use ratatui::{
    Frame,
    widgets::{Block, Paragraph, Wrap},
};

use crate::core::app_state::AppState;
use crate::core::types::ContainerKey;

use super::render::UiStyles;

/// Renders the log view for a specific container
pub fn render_log_view(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    container_key: &ContainerKey,
    state: &mut AppState,
    styles: &UiStyles,
) {
    let size = area;

    // Get container info
    let container_name = state
        .containers
        .get(container_key)
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");

    // Get number of log lines for this container (only if it matches current_log_container)
    // Use the cached formatted text instead of reformatting on every render
    let num_lines = if let Some(key) = &state.current_log_container {
        if key == container_key {
            state.formatted_log_text.lines.len()
        } else {
            0
        }
    } else {
        0
    };

    // Calculate visible height (subtract 1 for top border)
    let visible_height = size.height.saturating_sub(1) as usize;

    // Store viewport height for page up/down calculations
    state.last_viewport_height = visible_height;

    // Calculate max scroll position
    let max_scroll = if num_lines > visible_height {
        num_lines.saturating_sub(visible_height)
    } else {
        0
    };

    // Determine actual scroll offset
    let actual_scroll = if state.is_at_bottom {
        // Auto-scroll to bottom
        max_scroll
    } else {
        // Use manual scroll position, but clamp to max
        state.log_scroll_offset.min(max_scroll)
    };

    // Update is_at_bottom based on actual position
    state.is_at_bottom = actual_scroll >= max_scroll;

    // Update scroll offset to actual (for proper clamping)
    state.log_scroll_offset = actual_scroll;

    // Only clone the visible portion of logs to improve scrolling performance
    // Calculate visible range based on scroll position and terminal height
    let visible_start = actual_scroll;
    let visible_end = (actual_scroll + size.height as usize).min(num_lines);

    // Extract only the visible lines (cheap slice + small clone vs full clone)
    let visible_lines = if visible_start < state.formatted_log_text.lines.len() {
        state.formatted_log_text.lines[visible_start..visible_end].to_vec()
    } else {
        vec![]
    };

    let visible_text = ratatui::text::Text::from(visible_lines);

    // Create log widget with only visible text, no scroll needed since we pre-sliced
    let log_widget = Paragraph::new(visible_text)
        .block(
            Block::default()
                .title(format!(
                    "Logs: {} ({}) - Press ESC to return {}",
                    container_name,
                    container_key.host_id,
                    if state.is_at_bottom {
                        "[AUTO]"
                    } else {
                        "[MANUAL]"
                    }
                ))
                .style(styles.border),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(log_widget, size);
}
