use ansi_to_tui::IntoText;
use bollard::query_parameters::LogsOptions;
use chrono::{DateTime, Utc};
use colored_json::{ColoredFormatter, CompactFormatter};
use futures_util::stream::StreamExt;
use ratatui::text::Text;

use crate::core::types::{AppEvent, ContainerKey, EventSender};
use crate::docker::connection::DockerHost;

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
        let formatted_message = Self::try_format_json(message.trim());
        let text = formatted_message
            .as_bytes()
            .into_text()
            .unwrap_or_else(|_| Text::from(message.to_string()));

        Some(LogEntry { timestamp, text })
    }

    /// Try to parse and format the message as JSON
    /// Returns the colorized JSON string if valid, otherwise returns the original message
    fn try_format_json(message: &str) -> String {
        // Try to parse as JSON to validate it
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(message) {
            // Create formatter with CompactFormatter for single-line output
            // Note: Creating the formatter is cheap (just struct initialization)
            // and to_colored_json_auto() consumes self, so we can't reuse it anyway
            let formatter = ColoredFormatter::new(CompactFormatter {});
            if let Ok(colored) = formatter.to_colored_json_auto(&json_value) {
                return colored;
            }
        }
        // If not valid JSON or colorization failed, return original message
        message.to_string()
    }
}

/// Streams logs from a container in real-time
/// Sends each log line as it arrives via the event channel
pub async fn stream_container_logs(host: DockerHost, container_id: String, tx: EventSender) {
    let key = ContainerKey::new(host.host_id.clone(), container_id.clone());

    // Configure log options to stream logs
    let options = Some(LogsOptions {
        follow: true,            // Stream logs in real-time
        stdout: true,            // Include stdout
        stderr: true,            // Include stderr
        tail: "100".to_string(), // Start with last 100 lines
        timestamps: true,        // Include timestamps
        ..Default::default()
    });

    let mut log_stream = host.docker.logs(&container_id, options);

    while let Some(log_result) = log_stream.next().await {
        match log_result {
            Ok(log_output) => {
                // Convert log output to string and strip carriage returns
                // Jellyfin and other apps use \r for progress updates, which causes artifacts
                let log_line = log_output.to_string().replace('\r', "");

                // Parse the log line into a LogEntry
                if let Some(log_entry) = LogEntry::parse(&log_line) {
                    // Send log entry event
                    if tx
                        .send(AppEvent::LogLine(key.clone(), log_entry))
                        .await
                        .is_err()
                    {
                        // Channel closed, stop streaming
                        break;
                    }
                }
                // If parsing fails, we skip this log line
            }
            Err(_) => {
                // Error reading logs, stop streaming
                break;
            }
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
}
