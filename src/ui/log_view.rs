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

    // Store viewport height for page up/down calculations
    state.last_viewport_height = visible_height;

    // Calculate max scroll position (first line that can be at top of viewport)
    // If we have 100 lines and can show 20, max_scroll is 80 (lines 80-99 visible)
    let max_scroll = num_lines.saturating_sub(visible_height);

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

    // Only format the visible portion of log entries for performance
    // Calculate visible range based on scroll position and viewport height
    let visible_start = actual_scroll;
    let visible_end = (actual_scroll + visible_height).min(num_lines);

    // Format only the visible log entries into lines
    let visible_lines: Vec<_> = if visible_start < log_state.log_entries.len() {
        log_state.log_entries[visible_start..visible_end]
            .iter()
            .map(format_log_entry)
            .collect()
    } else {
        vec![]
    };

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
