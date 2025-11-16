use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

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

/// Format JSON as colored ratatui Text with flattened key-value pairs
/// Returns Text with color-coded keys and values padded to multiples of 5 for alignment
pub fn format_json_as_text(json_value: &serde_json::Value) -> Text<'static> {
    // Flatten the JSON object into key-value pairs with type information
    let flattened = flatten_json("", json_value);

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
        let value_style = get_value_style(value_type);
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
                result.extend(flatten_json(&new_prefix, val));
            }
        }
        serde_json::Value::Array(arr) => {
            for (idx, val) in arr.iter().enumerate() {
                let new_prefix = format!("{}[{}]", prefix, idx);
                result.extend(flatten_json(&new_prefix, val));
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
