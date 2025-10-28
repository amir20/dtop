use bollard::query_parameters::LogsOptions;
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;

use crate::docker::DockerHost;
use crate::types::{AppEvent, ContainerKey, EventSender};

/// A parsed log entry with timestamp and message
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub message: String,
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

        Some(LogEntry {
            timestamp,
            message: message.trim().to_string(),
        })
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
                // Convert log output to string
                let log_line = log_output.to_string();

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

        assert_eq!(entry.message, "Hello world");
        assert_eq!(entry.timestamp.format("%Y-%m-%d").to_string(), "2025-10-28");
    }

    #[test]
    fn test_parse_log_entry_with_multiple_spaces() {
        let log_line = "2025-10-28T12:34:56.789Z Message with   multiple spaces";
        let entry = LogEntry::parse(log_line).expect("Should parse log line with multiple spaces");

        assert_eq!(entry.message, "Message with   multiple spaces");
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

        assert_eq!(entry.message, "");
    }
}
