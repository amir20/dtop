use ansi_to_tui::IntoText;
use bollard::query_parameters::LogsOptions;
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::core::types::{AppEvent, ContainerKey, EventSender};
use crate::docker::connection::DockerHost;

/// Represents the type of a JSON value for efficient styling
#[derive(Clone, Debug)]
enum JsonValueType {
    String(String),
    Number(String),
    Bool(bool),
    Null,
}

impl JsonValueType {
    /// Get the string representation of the value
    fn as_str(&self) -> &str {
        match self {
            JsonValueType::String(s) | JsonValueType::Number(s) => s,
            JsonValueType::Bool(true) => "true",
            JsonValueType::Bool(false) => "false",
            JsonValueType::Null => "null",
        }
    }
}

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
            Self::format_json_as_text(&json_value)
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

    /// Format JSON as colored ratatui Text with flattened key-value pairs
    /// Returns Text with color-coded keys and values padded to multiples of 5 for alignment
    fn format_json_as_text(json_value: &serde_json::Value) -> Text<'static> {
        // Flatten the JSON object into key-value pairs with type information
        let flattened = Self::flatten_json("", json_value);

        // Create colored spans for each key=value pair
        let mut spans = Vec::new();

        for (key, value_type) in flattened.iter() {
            // Key in cyan with bold
            spans.push(Span::styled(
                key.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

            // Equals sign in gray
            spans.push(Span::styled(
                "=".to_string(),
                Style::default().fg(Color::Gray),
            ));

            // Value color based on type (no parsing needed!)
            let value_style = Self::get_value_style(value_type);
            spans.push(Span::styled(value_type.as_str().to_string(), value_style));

            // Calculate padding to nearest multiple of 5, with at least 1 space
            // For example: length 13 -> pad to 15 (add 2 spaces)
            //              length 10 -> pad to 15 (add 5 spaces, not 0)
            let field_len = key.len() + 1 + value_type.as_str().len(); // +1 for "="
            let next_multiple = ((field_len / 5) + 1) * 5; // Always round up to next multiple
            let padding = next_multiple - field_len;
            spans.push(Span::raw(" ".repeat(padding)));
        }

        Text::from(Line::from(spans))
    }

    /// Determine the style for a value based on its type
    fn get_value_style(value_type: &JsonValueType) -> Style {
        match value_type {
            JsonValueType::Null => Style::default().fg(Color::DarkGray),
            JsonValueType::Bool(true) => Style::default().fg(Color::Green),
            JsonValueType::Bool(false) => Style::default().fg(Color::Red),
            JsonValueType::Number(_) => Style::default().fg(Color::Yellow),
            JsonValueType::String(_) => Style::default().fg(Color::White),
        }
    }

    /// Recursively flatten a JSON value into dot-notation key-value pairs
    /// Returns a vector of (key, value_type) tuples
    fn flatten_json(prefix: &str, value: &serde_json::Value) -> Vec<(String, JsonValueType)> {
        let mut result = Vec::new();

        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map {
                    let new_prefix = if prefix.is_empty() {
                        key.to_string()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    result.extend(Self::flatten_json(&new_prefix, val));
                }
            }
            serde_json::Value::Array(arr) => {
                for (idx, val) in arr.iter().enumerate() {
                    let new_prefix = format!("{}[{}]", prefix, idx);
                    result.extend(Self::flatten_json(&new_prefix, val));
                }
            }
            _ => {
                // Leaf value - capture type information
                let value_type = match value {
                    serde_json::Value::String(s) => JsonValueType::String(s.clone()),
                    serde_json::Value::Number(n) => JsonValueType::Number(n.to_string()),
                    serde_json::Value::Bool(b) => JsonValueType::Bool(*b),
                    serde_json::Value::Null => JsonValueType::Null,
                    _ => unreachable!(),
                };
                result.push((prefix.to_string(), value_type));
            }
        }

        result
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
}
