use ratatui::{
    Frame,
    text::{Line, Text},
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::core::app_state::AppState;
use crate::core::types::ContainerKey;

use super::render::UiStyles;

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

/// Find the entry index and sub-line offset for a given visual line position.
/// Returns (entry_index, sub_line_offset) where sub_line_offset is the number
/// of visual lines into the entry that the scroll position falls.
fn find_visible_start(lines: &[Line], visual_line: usize, width: usize) -> (usize, usize) {
    let mut accumulated = 0;
    for (i, line) in lines.iter().enumerate() {
        let rows = wrapped_line_height(line, width);
        if accumulated + rows > visual_line {
            return (i, visual_line - accumulated);
        }
        accumulated += rows;
    }
    // Past the end — return last entry with no sub-offset
    (lines.len().saturating_sub(1), 0)
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

    // Use cached formatted lines (maintained by event handlers)
    let all_lines = &log_state.formatted_lines;

    // Calculate total visual lines — O(n) but cheap (no allocations, just width comparisons)
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

    // Find the first visible entry and sub-line offset within it.
    // This also gives us the entry index for progress calculation.
    let (first_entry_idx, sub_line_offset) =
        find_visible_start(all_lines, actual_scroll, inner_width);

    // Determine status indicator
    let status_indicator = if log_state.fetching_older {
        "[Loading...]".to_string()
    } else if state.is_at_bottom {
        "[LIVE]".to_string()
    } else if let Some(progress) = log_state.calculate_progress(first_entry_idx) {
        if log_state.has_more_history || progress > 0.0 {
            format!("[{:.0}%]", progress)
        } else {
            "[0%]".to_string()
        }
    } else {
        String::new()
    };

    // Collect only the visible slice of lines — O(viewport) instead of O(n).
    // We need enough lines to fill visible_height + sub_line_offset (to account
    // for the partial first entry that gets scrolled past).
    let needed_rows = visible_height + sub_line_offset;
    let mut visible_lines: Vec<Line> = Vec::new();
    let mut rows_collected = 0;

    for line in &all_lines[first_entry_idx..] {
        let rows = wrapped_line_height(line, inner_width);
        visible_lines.push(line.clone());
        rows_collected += rows;
        if rows_collected >= needed_rows {
            break;
        }
    }

    let visible_text = Text::from(visible_lines);

    // Paragraph::scroll() only needs the sub-line offset within the first entry,
    // since we already sliced to the visible window.
    let log_widget = Paragraph::new(visible_text)
        .block(
            Block::default()
                .title(format!(
                    "Logs: {} ({}) - Press ESC to return {}",
                    container_name, container_key.host_id, status_indicator
                ))
                .style(styles.border),
        )
        .wrap(Wrap { trim: false })
        .scroll((sub_line_offset as u16, 0));

    f.render_widget(log_widget, area);

    // Render scrollbar on the right side
    let mut scrollbar_state = ScrollbarState::default()
        .content_length(total_rows)
        .viewport_content_length(visible_height)
        .position(actual_scroll + visible_height);

    let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);

    f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}
