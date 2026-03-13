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
fn wrapped_line_height(line: &Line, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    let line_width = line.width();
    if line_width <= width {
        return 1;
    }
    line_width.div_ceil(width)
}

/// Format a log entry into a Line with timestamp and ANSI-parsed content
fn format_log_entry(log_entry: &LogEntry) -> Line<'static> {
    let local_timestamp = log_entry.timestamp.with_timezone(&Local);
    let timestamp_str = local_timestamp.format("%Y-%m-%d %H:%M:%S").to_string();

    let mut line_spans = vec![Span::styled(timestamp_str, TIMESTAMP_STYLE), Span::raw(" ")];

    if let Some(text_line) = log_entry.text.lines.first() {
        line_spans.extend(text_line.spans.iter().cloned());
    }

    Line::from(line_spans)
}

/// Find which entry index contains a given visual line offset.
fn entry_index_for_visual_line(entries: &[LogEntry], visual_line: usize, width: usize) -> usize {
    let mut accumulated = 0;
    for (i, entry) in entries.iter().enumerate() {
        let line = format_log_entry(entry);
        let rows = wrapped_line_height(&line, width);
        if accumulated + rows > visual_line {
            return i;
        }
        accumulated += rows;
    }
    entries.len().saturating_sub(1)
}

/// Renders the log view for a specific container
pub fn render_log_view(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    container_key: &ContainerKey,
    state: &mut AppState,
    styles: &UiStyles,
) {
    let Some(log_state) = &mut state.log_state else {
        return;
    };

    if &log_state.container_key != container_key {
        return;
    }

    let container_name = state
        .containers
        .get(container_key)
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");

    // Calculate visible height (subtract 2 for top and bottom border)
    let visible_height = area.height.saturating_sub(2) as usize;

    // Inner width for wrap calculations (subtract 2 for left/right border)
    let inner_width = area.width.saturating_sub(2) as usize;

    // Store viewport dimensions for scroll calculations
    state.last_viewport_height = visible_height;
    state.last_viewport_width = inner_width;

    // Format all log entries
    let all_lines: Vec<Line> = log_state.log_entries.iter().map(format_log_entry).collect();

    // Calculate total visual lines (accounting for wrapping)
    let total_rows: usize = all_lines
        .iter()
        .map(|line| wrapped_line_height(line, inner_width))
        .sum();

    // Max scroll: enough so that the last visual line is at the bottom of the viewport
    let max_scroll = total_rows.saturating_sub(visible_height);

    // Determine actual scroll offset (in visual lines)
    let actual_scroll = if state.is_at_bottom {
        max_scroll
    } else {
        log_state.scroll_offset.min(max_scroll)
    };

    // Update is_at_bottom based on actual position
    state.is_at_bottom = actual_scroll >= max_scroll;

    // Update scroll offset to actual (for proper clamping)
    log_state.scroll_offset = actual_scroll;

    // Find which entry is at the current scroll position (for progress calculation)
    let visible_entry_index =
        entry_index_for_visual_line(&log_state.log_entries, actual_scroll, inner_width);

    // Determine status indicator
    let status_indicator = if log_state.fetching_older {
        "[Loading...]".to_string()
    } else if state.is_at_bottom {
        "[LIVE]".to_string()
    } else if let Some(progress) = log_state.calculate_progress(visible_entry_index) {
        if log_state.has_more_history || progress > 0.0 {
            format!("[{:.0}%]", progress)
        } else {
            "[0%]".to_string()
        }
    } else {
        String::new()
    };

    let all_text = Text::from(all_lines);

    // Use Paragraph::scroll() to handle visual-line-level scrolling natively
    let log_widget = Paragraph::new(all_text)
        .block(
            Block::default()
                .title(format!(
                    "Logs: {} ({}) - Press ESC to return {}",
                    container_name, container_key.host_id, status_indicator
                ))
                .style(styles.border),
        )
        .wrap(Wrap { trim: false })
        .scroll((actual_scroll as u16, 0));

    f.render_widget(log_widget, area);

    // Render scrollbar on the right side
    let mut scrollbar_state = ScrollbarState::default()
        .content_length(total_rows)
        .viewport_content_length(visible_height)
        .position(actual_scroll);

    let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);

    f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}
