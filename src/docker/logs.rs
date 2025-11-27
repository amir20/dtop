use ansi_to_tui::IntoText;
use bollard::query_parameters::LogsOptions;
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use ratatui::text::Text;

use crate::core::types::{AppEvent, ContainerKey, EventSender};
use crate::docker::connection::DockerHost;
use crate::docker::json_formatter;

/// A parsed log entry with timestamp and ANSI-parsed content
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    /// Parsed ANSI text ready for rendering
    pub text: Text<'static>,
}

impl LogEntry {
    /// Parse a Docker log line with RFC3339 timestamp
    /// Format: "2025-10-28T12:34:56.789Z message content"
    pub fn parse(log_line: &str) -> Option<Self> {
        // Find the first space which separates timestamp from message
        let space_idx = log_line.find(' ')?;
        let (timestamp_str, message) = log_line.split_at(space_idx);

        // Parse the timestamp (Docker uses RFC3339 format)
        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
            .ok()?
            .with_timezone(&Utc);

        // Try to detect and format JSON
        let text = if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(message.trim())
        {
            json_formatter::format_json_as_text(&json_value)
        } else {
            // Not JSON, try ANSI parsing for colored text
            message
                .trim()
                .as_bytes()
                .into_text()
                .unwrap_or_else(|_| Text::from(message.to_string()))
        };

        Some(LogEntry { timestamp, text })
    }
}

/// Fetches older logs for pagination
/// Fetches logs before a specific timestamp
pub async fn fetch_older_logs(
    host: DockerHost,
    container_id: String,
    before_timestamp: DateTime<Utc>,
    batch_size: usize,
    tx: EventSender,
) {
    let key = ContainerKey::new(host.host_id.clone(), container_id.clone());

    // Fetch logs before the given timestamp
    // Note: We use 'until' to get logs before the timestamp, but we can't use 'tail' with it
    // because 'tail' gets the LAST N logs from the entire log history, not from the filtered set.
    // So we fetch all logs up to the timestamp, then take the last N ourselves.
    let options = Some(LogsOptions {
        follow: false,
        stdout: true,
        stderr: true,
        timestamps: true,
        until: (before_timestamp.timestamp() - 1) as i32, // -1 to exclude the boundary
        ..Default::default()
    });

    let mut log_stream = host.docker.logs(&container_id, options);
    let mut all_logs = Vec::new();

    // Collect all logs up to the 'until' timestamp
    while let Some(log_result) = log_stream.next().await {
        match log_result {
            Ok(log_output) => {
                let log_line = log_output.to_string().replace('\r', "");
                if let Some(log_entry) = LogEntry::parse(&log_line) {
                    all_logs.push(log_entry);
                }
            }
            Err(_) => break,
        }
    }

    tracing::debug!(
        "Fetched {} logs before timestamp {}, taking last {}",
        all_logs.len(),
        before_timestamp,
        batch_size
    );

    // Take only the last N logs (most recent before the timestamp)
    let has_more_history = all_logs.len() > batch_size;
    let logs = if all_logs.len() > batch_size {
        // Take the last batch_size logs
        all_logs.split_off(all_logs.len() - batch_size)
    } else {
        all_logs
    };

    tracing::debug!(
        "Sending {} logs, has_more_history: {}",
        logs.len(),
        has_more_history
    );

    // Send the batch (even if empty, to signal completion)
    let _ = tx
        .send(AppEvent::LogBatchPrepend(key, logs, has_more_history))
        .await;
}

/// Streams logs from a container in real-time
/// Fetches recent logs initially (for pagination), then streams new logs line by line
pub async fn stream_container_logs(host: DockerHost, container_id: String, tx: EventSender) {
    let key = ContainerKey::new(host.host_id.clone(), container_id.clone());

    const INITIAL_BATCH_SIZE: usize = 1000;

    // Phase 1: Fetch initial batch (most recent 1000 logs)
    let historical_options = Some(LogsOptions {
        follow: false,                           // Don't follow, just get existing logs
        stdout: true,                            // Include stdout
        stderr: true,                            // Include stderr
        timestamps: true,                        // Include timestamps
        tail: format!("{}", INITIAL_BATCH_SIZE), // Get most recent N logs
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
        since: last_timestamp.map(|ts| ts.timestamp() as i32).unwrap_or(0), // Start after last historical log
        ..Default::default()
    });

    let mut log_stream = host.docker.logs(&container_id, streaming_options);

    while let Some(log_result) = log_stream.next().await {
        match log_result {
            Ok(log_output) => {
                let log_line = log_output.to_string().replace('\r', "");
                if let Some(log_entry) = LogEntry::parse(&log_line)
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
            "JSON should be flattened with key=value format. Got: '{}'",
            text_str
        );
        assert!(
            text_str.contains("key=value"),
            "JSON should be flattened with key=value format. Got: '{}'",
            text_str
        );
        // Check that fields are separated (should contain both keys)
        assert!(
            text_str.contains("another") && text_str.contains("key"),
            "JSON should contain all fields. Got: '{}'",
            text_str
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
            "Should contain flattened name field. Got: '{}'",
            text_str
        );
        assert!(
            text_str.contains("age=30"),
            "Should contain flattened age field. Got: '{}'",
            text_str
        );
        assert!(
            text_str.contains("address.city=Portland"),
            "Should contain flattened nested city field with dot notation. Got: '{}'",
            text_str
        );
        assert!(
            text_str.contains("address.zip=97201"),
            "Should contain flattened nested zip field with dot notation. Got: '{}'",
            text_str
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
