use ansi_to_tui::IntoText;
use bollard::query_parameters::LogsOptions;
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use ratatui::text::{Line, Span, Text};

use crate::core::types::{AppEvent, ContainerKey, EventSender};
use crate::docker::connection::DockerHost;
use crate::docker::json_formatter;
use crate::docker::timestamp_prefix::duplicate_timestamp;

/// Columns dtop's timestamp and the space after it take in a formatted line
pub const TIMESTAMP_COLUMNS: usize = 20;

/// A parsed log entry with timestamp and ANSI-parsed content
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    /// Parsed ANSI text ready for rendering, as the app printed it
    pub text: Text<'static>,
    /// `text` without the timestamp the app printed at the start of the line,
    /// when that repeats `timestamp` (see `timestamp_prefix`). Shown next to
    /// dtop's own timestamp, so the line carries the time once.
    pub without_own_timestamp: Option<Text<'static>>,
}

impl LogEntry {
    /// Format this log entry into a styled Line suitable for a ratatui Paragraph.
    ///
    /// With `show_timestamp`, dtop's timestamp leads and the app's duplicate of
    /// it stays hidden. Without, the line reads as the app printed it.
    /// `repeats_previous` dims the timestamp: the line shares its second with
    /// the one above, as the rows of a stack trace or a table do.
    pub fn format(&self, show_timestamp: bool, repeats_previous: bool) -> Line<'static> {
        use chrono::Local;
        use ratatui::style::{Color, Modifier, Style};

        const TIMESTAMP_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
        const REPEATED_STYLE: Style = Style::new().fg(Color::DarkGray);

        let mut line_spans = Vec::new();
        let mut text = &self.text;
        if show_timestamp {
            let local_timestamp = self.timestamp.with_timezone(&Local);
            let timestamp_str = local_timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
            debug_assert_eq!(timestamp_str.len() + 1, TIMESTAMP_COLUMNS);
            let style = if repeats_previous {
                REPEATED_STYLE
            } else {
                TIMESTAMP_STYLE
            };
            line_spans.push(Span::styled(timestamp_str, style));
            line_spans.push(Span::raw(" "));
            text = self.without_own_timestamp.as_ref().unwrap_or(text);
        }

        if let Some(text_line) = text.lines.first() {
            line_spans.extend(text_line.spans.iter().cloned());
        }

        Line::from(line_spans)
    }

    /// Whether this entry would show the same timestamp as `previous`
    pub fn repeats(&self, previous: Option<&LogEntry>) -> bool {
        previous.is_some_and(|p| p.timestamp.timestamp() == self.timestamp.timestamp())
    }

    /// Parse a Docker log line with RFC3339 timestamp
    /// Format: "2025-10-28T12:34:56.789Z message content"
    pub fn parse(log_line: &str) -> Option<Self> {
        // Find the first space which separates timestamp from message
        let space_idx = log_line.find(' ')?;
        let (timestamp_str, message) = log_line.split_at(space_idx);
        let message = message.trim();

        // Parse the timestamp (Docker uses RFC3339 format)
        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
            .ok()?
            .with_timezone(&Utc);

        // Try to detect and format JSON
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(message) {
            return Some(LogEntry {
                timestamp,
                text: json_formatter::format_json_as_text(&json_value),
                without_own_timestamp: None,
            });
        }

        // Not JSON, try ANSI parsing for colored text. Any coloring of the
        // duplicate timestamp is closed inside the cut, so what is left
        // parses on its own.
        let without_own_timestamp = duplicate_timestamp(message, timestamp.timestamp_millis())
            .map(|cut| ansi_text(&format!("{}{}", &message[..cut.start], &message[cut.end..])));

        Some(LogEntry {
            timestamp,
            text: ansi_text(message),
            without_own_timestamp,
        })
    }
}

fn ansi_text(message: &str) -> Text<'static> {
    let mut text = message
        .as_bytes()
        .into_text()
        .unwrap_or_else(|_| Text::from(message.to_string()));
    if message.contains('\t') {
        text.lines.iter_mut().for_each(expand_tabs);
    }
    text
}

/// Replaces each tab with spaces up to the next multiple of `TAB_WIDTH`
/// columns, counted from the start of the message. A terminal cell can't hold
/// a tab, so ratatui would draw nothing for one and run the fields together.
fn expand_tabs(line: &mut Line<'static>) {
    const TAB_WIDTH: usize = 8;
    let mut column = 0;
    for span in &mut line.spans {
        if !span.content.contains('\t') {
            column += span.width();
            continue;
        }
        let mut expanded = String::with_capacity(span.content.len() + TAB_WIDTH);
        for c in span.content.chars() {
            if c == '\t' {
                let spaces = TAB_WIDTH - column % TAB_WIDTH;
                expanded.extend(std::iter::repeat_n(' ', spaces));
                column += spaces;
            } else {
                expanded.push(c);
                column += unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }
        span.content = expanded.into();
    }
}

/// Fetches older logs for pagination using density-based adaptive algorithm
///
/// This function calculates log density from the existing batch (time span between
/// oldest and newest timestamps) to estimate an optimal time window for fetching
/// the next batch. If the initial window doesn't yield enough logs, it exponentially
/// expands the window until enough logs are found or max attempts is reached.
pub async fn fetch_older_logs(
    host: DockerHost,
    container_id: String,
    before_timestamp: DateTime<Utc>,
    newest_timestamp: DateTime<Utc>,
    container_created: Option<DateTime<Utc>>,
    batch_size: usize,
    tx: EventSender,
) {
    const EXPANSION_FACTOR: i32 = 2;
    const DENSITY_BUFFER: f64 = 1.2; // 20% buffer
    const FALLBACK_WINDOW_MINUTES: i64 = 5;
    const MAX_ATTEMPTS: u32 = 10;

    let key = ContainerKey::new(host.host_id.clone(), container_id.clone());

    // Calculate log density from existing batch to estimate optimal time window
    let time_span = newest_timestamp.signed_duration_since(before_timestamp);
    let initial_window_duration = if time_span.num_seconds() > 0 {
        // The existing batch spans this time, so to get another batch_size logs,
        // we need approximately the same time span, plus a 20% buffer
        let seconds_needed = (time_span.num_seconds() as f64 * DENSITY_BUFFER) as i64;
        chrono::Duration::seconds(seconds_needed)
    } else {
        // Fallback if we can't calculate density (single log or all same timestamp)
        chrono::Duration::minutes(FALLBACK_WINDOW_MINUTES)
    };

    tracing::debug!(
        "Log density calculation: time_span={}s, batch_size={}, estimated_window={}s",
        time_span.num_seconds(),
        batch_size,
        initial_window_duration.num_seconds()
    );

    let mut current_duration = initial_window_duration;
    let mut attempt = 0;

    loop {
        attempt += 1;
        if attempt > MAX_ATTEMPTS {
            tracing::warn!(
                "Reached max attempts ({}) fetching older logs, no older logs found",
                MAX_ATTEMPTS
            );
            let _ = tx
                .send(AppEvent::LogBatchPrepend(key, Vec::new(), false))
                .await;
            return;
        }
        let mut since_timestamp = before_timestamp - current_duration;
        let mut reached_container_start = false;

        // Check if we've gone past the container creation time
        // If so, clamp to container creation and mark this as the final fetch
        if let Some(created) = container_created
            && since_timestamp < created
        {
            tracing::debug!(
                "Window start ({}) is before container creation ({}), clamping to container start",
                since_timestamp,
                created
            );
            since_timestamp = created;
            reached_container_start = true;
        }

        let options = Some(LogsOptions {
            follow: false,
            stdout: true,
            stderr: true,
            timestamps: true,
            // NOTE: Bollard's LogsOptions uses i32 for timestamps, which will overflow in 2038.
            // This is a limitation of the bollard-stubs API. Track upstream for i64 support.
            since: since_timestamp.timestamp() as i32,
            // Docker only filters by whole seconds, so ask for the whole boundary
            // second and trim to the exact timestamp below. Stopping a second
            // early skipped every unloaded line that shared the oldest loaded
            // line's second — for a burst, all of the remaining history.
            until: (before_timestamp.timestamp() + 1) as i32,
            ..Default::default()
        });

        let mut log_stream = host.docker.logs(&container_id, options);
        let mut batch_logs = Vec::new();

        // Collect logs within the time window
        while let Some(log_result) = log_stream.next().await {
            match log_result {
                Ok(log_output) => {
                    let log_line = log_output.to_string().replace('\r', "");
                    // Strictly older: a distinct line stamped with exactly the
                    // boundary line's nanosecond would be skipped. Docker's
                    // nanosecond stamps make that vanishingly rare; accepted.
                    if let Some(log_entry) = LogEntry::parse(&log_line)
                        && log_entry.timestamp < before_timestamp
                    {
                        batch_logs.push(log_entry);
                    }
                }
                Err(_) => break,
            }
        }

        tracing::debug!(
            "Attempt {}: Fetched {} logs with {}s window (since {} until {})",
            attempt + 1,
            batch_logs.len(),
            current_duration.num_seconds(),
            since_timestamp,
            before_timestamp
        );

        // If we reached container start, return all logs we found (don't trim)
        if reached_container_start {
            // Still one page at most: the window reaching the container's
            // start can hold far more than a page (a burst at startup), and
            // whatever doesn't fit is more history, not the end of it
            let has_more = batch_logs.len() > batch_size;
            if has_more {
                batch_logs.drain(..batch_logs.len() - batch_size);
            }
            tracing::debug!(
                "Reached container start, returning {} logs (more: {has_more})",
                batch_logs.len()
            );
            let _ = tx
                .send(AppEvent::LogBatchPrepend(key, batch_logs, has_more))
                .await;
            return;
        }

        // Check if we have enough logs
        if batch_logs.len() >= batch_size {
            // Success: take the last batch_size logs (most recent)
            let start_idx = batch_logs.len() - batch_size;
            let logs = batch_logs.split_off(start_idx);
            let _ = tx.send(AppEvent::LogBatchPrepend(key, logs, true)).await;
            return;
        }

        // Keep expanding the window for next attempt
        current_duration = current_duration * EXPANSION_FACTOR;
    }
}

/// Streams logs from a container in real-time
/// Fetches recent logs initially (for pagination), then streams new logs line by line
pub async fn stream_container_logs(host: DockerHost, container_id: String, tx: EventSender) {
    let key = ContainerKey::new(host.host_id.clone(), container_id.clone());

    const INITIAL_BATCH_SIZE: usize = 1000;

    // Phase 1: Fetch initial batch (most recent 1000 logs)
    let historical_options = Some(LogsOptions {
        follow: false,                         // Don't follow, just get existing logs
        stdout: true,                          // Include stdout
        stderr: true,                          // Include stderr
        timestamps: true,                      // Include timestamps
        tail: format!("{INITIAL_BATCH_SIZE}"), // Get most recent N logs
        ..Default::default()
    });

    let mut historical_stream = host.docker.logs(&container_id, historical_options);
    let mut historical_logs = Vec::new();
    let mut last_timestamp: Option<DateTime<Utc>> = None;

    // Collect initial batch of logs
    while let Some(log_result) = historical_stream.next().await {
        match log_result {
            Ok(log_output) => {
                let log_line = log_output.to_string().replace('\r', "");
                if let Some(log_entry) = LogEntry::parse(&log_line) {
                    last_timestamp = Some(log_entry.timestamp);
                    historical_logs.push(log_entry);
                }
            }
            Err(_) => break,
        }
    }

    // Determine if there might be more historical logs
    // If we got a full batch, assume there might be more
    let has_more_history = historical_logs.len() >= INITIAL_BATCH_SIZE;

    // Send initial batch as LogBatchPrepend
    if !historical_logs.is_empty()
        && tx
            .send(AppEvent::LogBatchPrepend(
                key.clone(),
                historical_logs,
                has_more_history,
            ))
            .await
            .is_err()
    {
        return; // Channel closed
    }

    // Phase 2: Start streaming new logs from after the last timestamp
    let streaming_options = Some(LogsOptions {
        follow: true, // Stream logs in real-time
        stdout: true, // Include stdout
        stderr: true, // Include stderr
        timestamps: true,
        // NOTE: Bollard's LogsOptions uses i32 for timestamps (Y2038 limitation)
        // Whole seconds only, so this replays the rest of the last historical
        // log's second; those lines are skipped below (strictly newer, with the
        // same accepted equal-nanosecond caveat as `fetch_older_logs`).
        since: last_timestamp.map(|ts| ts.timestamp() as i32).unwrap_or(0),
        ..Default::default()
    });

    let mut log_stream = host.docker.logs(&container_id, streaming_options);

    while let Some(log_result) = log_stream.next().await {
        match log_result {
            Ok(log_output) => {
                let log_line = log_output.to_string().replace('\r', "");
                if let Some(log_entry) = LogEntry::parse(&log_line)
                    && last_timestamp.is_none_or(|last| log_entry.timestamp > last)
                    && tx
                        .send(AppEvent::LogLine(key.clone(), log_entry))
                        .await
                        .is_err()
                {
                    break; // Channel closed, stop streaming
                }
            }
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// The app's copy of Docker's timestamp is hidden behind dtop's, and comes
    /// back (with its own coloring) when dtop's is turned off
    #[test]
    fn test_duplicate_timestamp_shows_once() {
        let entry = LogEntry::parse(
            "2025-10-28T12:34:56.789Z \x1b[90m2025-10-28T12:34:56Z\x1b[0m INF Starting server",
        )
        .unwrap();

        let with = plain(&entry.format(true, false));
        assert!(with.ends_with(":56 INF Starting server"), "{with}");
        assert_eq!(with.matches("2025-10-2").count(), 1, "{with}");

        let without = entry.format(false, false);
        assert_eq!(plain(&without), "2025-10-28T12:34:56Z INF Starting server");
        assert_ne!(without.spans[0].style, ratatui::style::Style::default());
    }

    /// Tabs become spaces up to the next tab stop, also across colored spans
    #[test]
    fn test_tabs_expand_to_tab_stops() {
        let entry = LogEntry::parse(
            "2025-10-28T12:34:56.789Z a\t\x1b[34mINFO\x1b[0m\tshutting down\t{\"server\": \"http\"}",
        )
        .unwrap();
        assert_eq!(
            plain(&entry.text.lines[0]),
            "a       INFO    shutting down   {\"server\": \"http\"}"
        );
    }

    /// Keys stay in the order the app wrote them
    #[test]
    fn test_json_keeps_key_order() {
        let entry = LogEntry::parse(
            r#"2025-10-28T12:34:56.789Z {"time":"t","level":"debug","msg":"hi","args":{"z":1,"a":2}}"#,
        )
        .unwrap();
        let keys: Vec<_> = plain(&entry.text.lines[0])
            .split_whitespace()
            .map(|field| field.split('=').next().unwrap().to_string())
            .collect();
        assert_eq!(keys, ["time", "level", "msg", "args.z", "args.a"]);
    }

    /// A timestamp that isn't Docker's (here an event from hours earlier) stays
    #[test]
    fn test_unrelated_timestamp_is_kept() {
        let entry =
            LogEntry::parse("2025-10-28T12:34:56.789Z 2025-10-28T09:00:00Z job finished").unwrap();
        assert!(entry.without_own_timestamp.is_none());
        assert!(plain(&entry.format(true, false)).ends_with(" 2025-10-28T09:00:00Z job finished"));
        assert_eq!(
            plain(&entry.format(false, false)),
            "2025-10-28T09:00:00Z job finished"
        );
    }

    #[test]
    fn test_parse_log_entry_valid() {
        let log_line = "2025-10-28T12:34:56.789Z Hello world";
        let entry = LogEntry::parse(log_line).expect("Should parse valid log line");

        assert_eq!(entry.timestamp.format("%Y-%m-%d").to_string(), "2025-10-28");
        assert!(!entry.text.lines.is_empty());
    }

    #[test]
    fn test_parse_log_entry_with_multiple_spaces() {
        let log_line = "2025-10-28T12:34:56.789Z Message with   multiple spaces";
        let entry = LogEntry::parse(log_line).expect("Should parse log line with multiple spaces");

        assert!(!entry.text.lines.is_empty());
    }

    #[test]
    fn test_parse_log_entry_invalid_timestamp() {
        let log_line = "invalid-timestamp Message";
        let entry = LogEntry::parse(log_line);

        assert!(entry.is_none(), "Should return None for invalid timestamp");
    }

    #[test]
    fn test_parse_log_entry_no_space() {
        let log_line = "2025-10-28T12:34:56.789Z";
        let entry = LogEntry::parse(log_line);

        assert!(
            entry.is_none(),
            "Should return None when no space separator"
        );
    }

    #[test]
    fn test_parse_log_entry_empty_message() {
        let log_line = "2025-10-28T12:34:56.789Z ";
        let entry = LogEntry::parse(log_line).expect("Should parse log line with empty message");

        // Should parse successfully even with empty message (just check it exists)
        assert_eq!(entry.timestamp.format("%Y-%m-%d").to_string(), "2025-10-28");
    }

    #[test]
    fn test_parse_log_entry_with_json() {
        let log_line = r#"2025-10-28T12:34:56.789Z {"level":"info","message":"test log","timestamp":1234567890}"#;
        let entry = LogEntry::parse(log_line).expect("Should parse log line with JSON");

        assert_eq!(entry.timestamp.format("%Y-%m-%d").to_string(), "2025-10-28");
        // The text should be formatted as a single line (compact JSON)
        assert_eq!(
            entry.text.lines.len(),
            1,
            "JSON should be formatted on a single line"
        );
    }

    #[test]
    fn test_parse_log_entry_with_invalid_json() {
        let log_line = r#"2025-10-28T12:34:56.789Z {"invalid": json}"#;
        let entry = LogEntry::parse(log_line).expect("Should parse log line with invalid JSON");

        assert_eq!(entry.timestamp.format("%Y-%m-%d").to_string(), "2025-10-28");
        // Invalid JSON should be treated as plain text
        assert!(!entry.text.lines.is_empty());
    }

    #[test]
    fn test_parse_log_entry_with_nested_json() {
        let log_line = r#"2025-10-28T12:34:56.789Z {"user":{"name":"test","id":123},"action":"login","success":true}"#;
        let entry = LogEntry::parse(log_line).expect("Should parse log line with nested JSON");

        assert_eq!(entry.timestamp.format("%Y-%m-%d").to_string(), "2025-10-28");
        assert!(!entry.text.lines.is_empty());
    }

    #[test]
    fn test_json_formatting_flattened() {
        let log_line = r#"2025-10-28T12:34:56.789Z {"key":"value","another":"test"}"#;
        let entry = LogEntry::parse(log_line).expect("Should parse log line with JSON");

        // Convert the text to a plain string to check the flattened format
        let text_str = entry
            .text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        // The formatted JSON should be flattened: key=value  another=test
        assert!(
            text_str.contains("another=test"),
            "JSON should be flattened with key=value format. Got: '{text_str}'"
        );
        assert!(
            text_str.contains("key=value"),
            "JSON should be flattened with key=value format. Got: '{text_str}'"
        );
        // Check that fields are separated (should contain both keys)
        assert!(
            text_str.contains("another") && text_str.contains("key"),
            "JSON should contain all fields. Got: '{text_str}'"
        );
    }

    #[test]
    fn test_json_formatting_nested() {
        let log_line = r#"2025-10-28T12:34:56.789Z {"name":"Alice","age":30,"address":{"city":"Portland","zip":"97201"}}"#;
        let entry = LogEntry::parse(log_line).expect("Should parse log line with nested JSON");

        // Convert the text to a plain string
        let text_str = entry
            .text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Check for flattened nested keys using dot notation
        assert!(
            text_str.contains("name=Alice"),
            "Should contain flattened name field. Got: '{text_str}'"
        );
        assert!(
            text_str.contains("age=30"),
            "Should contain flattened age field. Got: '{text_str}'"
        );
        assert!(
            text_str.contains("address.city=Portland"),
            "Should contain flattened nested city field with dot notation. Got: '{text_str}'"
        );
        assert!(
            text_str.contains("address.zip=97201"),
            "Should contain flattened nested zip field with dot notation. Got: '{text_str}'"
        );
    }

    // Density calculation tests for pagination algorithm
    mod density_calculation_tests {
        use super::*;
        use chrono::Duration;

        #[test]
        fn test_density_calculation_normal_case() {
            // 1000 logs over 10 minutes should suggest ~12 minute window (10 * 1.2)
            let oldest = DateTime::parse_from_rfc3339("2025-10-28T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc);
            let newest = DateTime::parse_from_rfc3339("2025-10-28T12:10:00Z")
                .unwrap()
                .with_timezone(&Utc);

            let time_span = newest.signed_duration_since(oldest);
            assert_eq!(time_span.num_seconds(), 600); // 10 minutes

            let estimated_window = (time_span.num_seconds() as f64 * 1.2) as i64;
            assert_eq!(estimated_window, 720); // 12 minutes (600 * 1.2)
        }

        #[test]
        fn test_density_calculation_high_frequency() {
            // 1000 logs over 1 minute should suggest ~1.2 minute window
            let oldest = DateTime::parse_from_rfc3339("2025-10-28T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc);
            let newest = DateTime::parse_from_rfc3339("2025-10-28T12:01:00Z")
                .unwrap()
                .with_timezone(&Utc);

            let time_span = newest.signed_duration_since(oldest);
            assert_eq!(time_span.num_seconds(), 60); // 1 minute

            let estimated_window = (time_span.num_seconds() as f64 * 1.2) as i64;
            assert_eq!(estimated_window, 72); // 1.2 minutes (60 * 1.2)
        }

        #[test]
        fn test_density_calculation_low_frequency() {
            // 1000 logs over 100 minutes should suggest ~120 minute window
            let oldest = DateTime::parse_from_rfc3339("2025-10-28T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc);
            let newest = DateTime::parse_from_rfc3339("2025-10-28T13:40:00Z")
                .unwrap()
                .with_timezone(&Utc);

            let time_span = newest.signed_duration_since(oldest);
            assert_eq!(time_span.num_seconds(), 6000); // 100 minutes

            let estimated_window = (time_span.num_seconds() as f64 * 1.2) as i64;
            assert_eq!(estimated_window, 7200); // 120 minutes (6000 * 1.2)
        }

        #[test]
        fn test_density_calculation_zero_duration() {
            // All logs have same timestamp - should use fallback
            let oldest = DateTime::parse_from_rfc3339("2025-10-28T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc);
            let newest = oldest; // Same timestamp

            let time_span = newest.signed_duration_since(oldest);
            assert_eq!(time_span.num_seconds(), 0);

            // When time_span is 0, we should fallback to 5 minutes
            const FALLBACK_WINDOW_MINUTES: i64 = 5;
            let window = if time_span.num_seconds() > 0 {
                Duration::seconds((time_span.num_seconds() as f64 * 1.2) as i64)
            } else {
                Duration::minutes(FALLBACK_WINDOW_MINUTES)
            };

            assert_eq!(window.num_seconds(), 300); // 5 minutes
        }

        #[test]
        fn test_density_calculation_negative_duration() {
            // Timestamps in wrong order - should use fallback
            let oldest = DateTime::parse_from_rfc3339("2025-10-28T12:10:00Z")
                .unwrap()
                .with_timezone(&Utc);
            let newest = DateTime::parse_from_rfc3339("2025-10-28T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc);

            let time_span = newest.signed_duration_since(oldest);
            assert!(time_span.num_seconds() < 0);

            // Should fallback to 5 minutes
            const FALLBACK_WINDOW_MINUTES: i64 = 5;
            let window = if time_span.num_seconds() > 0 {
                Duration::seconds((time_span.num_seconds() as f64 * 1.2) as i64)
            } else {
                Duration::minutes(FALLBACK_WINDOW_MINUTES)
            };

            assert_eq!(window.num_seconds(), 300); // 5 minutes
        }

        #[test]
        fn test_exponential_expansion() {
            // Test that expansion factor doubles the window
            let initial_duration = Duration::minutes(5);
            const EXPANSION_FACTOR: i32 = 2;

            let expanded = initial_duration * EXPANSION_FACTOR;
            assert_eq!(expanded.num_seconds(), 600); // 10 minutes

            let expanded_again = expanded * EXPANSION_FACTOR;
            assert_eq!(expanded_again.num_seconds(), 1200); // 20 minutes
        }

        #[test]
        fn test_very_dense_logs() {
            // 1000 logs over 10 seconds (very chatty container)
            let oldest = DateTime::parse_from_rfc3339("2025-10-28T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc);
            let newest = DateTime::parse_from_rfc3339("2025-10-28T12:00:10Z")
                .unwrap()
                .with_timezone(&Utc);

            let time_span = newest.signed_duration_since(oldest);
            assert_eq!(time_span.num_seconds(), 10);

            let estimated_window = (time_span.num_seconds() as f64 * 1.2) as i64;
            assert_eq!(estimated_window, 12); // 12 seconds
        }

        #[test]
        fn test_very_sparse_logs() {
            // 1000 logs over 1 day (very quiet container)
            let oldest = DateTime::parse_from_rfc3339("2025-10-28T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc);
            let newest = DateTime::parse_from_rfc3339("2025-10-29T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc);

            let time_span = newest.signed_duration_since(oldest);
            assert_eq!(time_span.num_seconds(), 86400); // 24 hours

            let estimated_window = (time_span.num_seconds() as f64 * 1.2) as i64;
            assert_eq!(estimated_window, 103680); // 28.8 hours
        }
    }
}
