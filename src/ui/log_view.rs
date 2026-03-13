use chrono::Local;
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::core::app_state::AppState;
use crate::core::types::ContainerKey;
use crate::docker::logs::LogEntry;

use super::render::UiStyles;

/// Style for log timestamps (yellow + bold)
const TIMESTAMP_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);

/// Calculate how many terminal rows a Line occupies when wrapped to the given width.
/// Uses unicode display width for accuracy with non-ASCII characters.
fn wrapped_line_height(line: &Line, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    let line_width = line.width();
    if line_width <= width {
        return 1;
    }
    // Ceiling division: how many rows this line wraps into
    (line_width + width - 1) / width
}

/// Format a log entry into a Line with timestamp and ANSI-parsed content
fn format_log_entry(log_entry: &LogEntry) -> Line<'static> {
    let local_timestamp = log_entry.timestamp.with_timezone(&Local);
    let timestamp_str = local_timestamp.format("%Y-%m-%d %H:%M:%S").to_string();

    // Create a line with timestamp + ANSI-parsed content
    let mut line_spans = vec![Span::styled(timestamp_str, TIMESTAMP_STYLE), Span::raw(" ")];

    // Append all spans from the ANSI-parsed text (should be a single line)
    if let Some(text_line) = log_entry.text.lines.first() {
        line_spans.extend(text_line.spans.iter().cloned());
    }

    Line::from(line_spans)
}

/// Renders the log view for a specific container
pub fn render_log_view(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    container_key: &ContainerKey,
    state: &mut AppState,
    styles: &UiStyles,
) {
    let size = area;

    let Some(log_state) = &mut state.log_state else {
        return; // No logs to display
    };

    // Verify we're viewing the correct container
    if &log_state.container_key != container_key {
        return;
    }

    // Get container info
    let container_name = state
        .containers
        .get(container_key)
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");

    // Get number of log entries
    let num_lines = log_state.log_entries.len();

    // Calculate visible height (subtract 2 for top and bottom border)
    let visible_height = size.height.saturating_sub(2) as usize;

    // Inner width available for text content (used for wrap height calculations)
    let inner_width = size.width as usize;

    // Store viewport height for page up/down calculations
    state.last_viewport_height = visible_height;

    // Calculate max scroll accounting for line wrapping.
    // Iterate backwards from the end to find how many entries fill the viewport.
    let max_scroll = {
        let mut rows_from_bottom = 0;
        let mut entries_from_bottom = 0;
        for entry in log_state.log_entries.iter().rev() {
            let line = format_log_entry(entry);
            let rows = wrapped_line_height(&line, inner_width);
            if rows_from_bottom + rows > visible_height && entries_from_bottom > 0 {
                break;
            }
            rows_from_bottom += rows;
            entries_from_bottom += 1;
            if rows_from_bottom >= visible_height {
                break;
            }
        }
        num_lines.saturating_sub(entries_from_bottom)
    };

    // Determine actual scroll offset
    let actual_scroll = if state.is_at_bottom {
        // Auto-scroll to bottom
        max_scroll
    } else {
        // Use manual scroll position, but clamp to max
        log_state.scroll_offset.min(max_scroll)
    };

    // Update is_at_bottom based on actual position
    state.is_at_bottom = actual_scroll >= max_scroll;

    // Update scroll offset to actual (for proper clamping)
    log_state.scroll_offset = actual_scroll;

    // Format visible entries, accounting for line wrapping.
    // Iterate forward from scroll position, accumulating wrapped row heights,
    // and stop when the viewport is full.
    let visible_start = actual_scroll;
    let mut visible_lines: Vec<Line> = Vec::new();
    let mut total_rows = 0;
    let mut visible_end = visible_start;

    if visible_start < num_lines {
        for entry in &log_state.log_entries[visible_start..] {
            let line = format_log_entry(entry);
            let rows = wrapped_line_height(&line, inner_width);
            if total_rows + rows > visible_height && !visible_lines.is_empty() {
                break;
            }
            total_rows += rows;
            visible_lines.push(line);
            visible_end += 1;
            if total_rows >= visible_height {
                break;
            }
        }
    }

    let visible_text = Text::from(visible_lines);

    // Determine status indicator - show only one of: [Loading...], [LIVE], or [XX%]
    let status_indicator = if log_state.fetching_older {
        // Show loading indicator when fetching older logs
        "[Loading...]".to_string()
    } else if state.is_at_bottom {
        // At bottom in auto-scroll mode, show LIVE
        "[LIVE]".to_string()
    } else if let Some(progress) = log_state.calculate_progress(actual_scroll) {
        // Not at bottom, show progress percentage
        if log_state.has_more_history || progress > 0.0 {
            format!("[{:.0}%]", progress)
        } else {
            // At the very beginning (0%)
            "[0%]".to_string()
        }
    } else {
        String::new()
    };

    // Create log widget with only visible text, no scroll needed since we pre-sliced
    let log_widget = Paragraph::new(visible_text)
        .block(
            Block::default()
                .title(format!(
                    "Logs: {} ({}) - Press ESC to return {}",
                    container_name, container_key.host_id, status_indicator
                ))
                .style(styles.border),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(log_widget, size);

    // Render scrollbar on the right side
    let mut scrollbar_state = ScrollbarState::default()
        .content_length(num_lines)
        .viewport_content_length(visible_height)
        .position(visible_end);

    let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);

    f.render_stateful_widget(scrollbar, size, &mut scrollbar_state);
}
