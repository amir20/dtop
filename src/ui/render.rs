use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::core::app_state::AppState;
use crate::core::types::{ContainerKey, ViewState};

use crate::ui::action_menu::render_action_menu;
use crate::ui::container_list::render_container_list;
use crate::ui::help::render_help_popup;

/// Pre-allocated styles to avoid recreation every frame
pub struct UiStyles {
    pub high: Style,
    pub medium: Style,
    pub low: Style,
    pub header: Style,
    pub border: Style,
    pub selected: Style,
    pub search_bar: Style,
}

impl Default for UiStyles {
    fn default() -> Self {
        Self {
            high: Style::default().fg(Color::Red),
            medium: Style::default().fg(Color::Yellow),
            low: Style::default().fg(Color::Green),
            header: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            border: Style::default().fg(Color::White),
            selected: Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            search_bar: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        }
    }
}

/// Renders the main UI - either container list, log view, or action menu
pub fn render_ui(f: &mut Frame, state: &mut AppState, styles: &UiStyles) {
    let size = f.area();

    // Calculate the main area and search bar area
    // Show search bar if in SearchMode OR if there's an active filter
    let show_search_bar = state.view_state == ViewState::SearchMode
        || (!state.search_input.value().is_empty() && state.view_state == ViewState::ContainerList);

    let (main_area, search_area) = if show_search_bar {
        // Reserve bottom 1 line for search bar
        let main_height = size.height.saturating_sub(1);
        let main = ratatui::layout::Rect {
            x: size.x,
            y: size.y,
            width: size.width,
            height: main_height,
        };
        let search = ratatui::layout::Rect {
            x: size.x,
            y: size.y + main_height,
            width: size.width,
            height: 1,
        };
        (main, Some(search))
    } else {
        (size, None)
    };

    match &state.view_state {
        ViewState::ContainerList | ViewState::SearchMode => {
            // Calculate unique hosts to determine if host column should be shown
            let unique_hosts: std::collections::HashSet<_> =
                state.containers.keys().map(|key| &key.host_id).collect();
            let show_host_column = unique_hosts.len() > 1;

            render_container_list(f, main_area, state, styles, show_host_column);
        }
        ViewState::LogView(container_key) => {
            let container_key = container_key.clone();
            render_log_view(f, main_area, &container_key, state, styles);
        }
        ViewState::ActionMenu(_) => {
            // First render the container list in the background
            let unique_hosts: std::collections::HashSet<_> =
                state.containers.keys().map(|key| &key.host_id).collect();
            let show_host_column = unique_hosts.len() > 1;

            render_container_list(f, main_area, state, styles, show_host_column);

            // Then render the action menu on top
            render_action_menu(f, state, styles);
        }
    }

    // Render search bar if active
    if let Some(search_area) = search_area {
        render_search_bar(f, search_area, state, styles);
    }

    // Render help popup on top if shown
    if state.show_help {
        render_help_popup(f, styles);
    }

    // Render connection error notifications in top right corner
    render_error_notifications(f, state, styles);
}

/// Renders the log view for a specific container
fn render_log_view(
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

    // Calculate visible height (subtract 1 for top)
    let visible_height = size.height.saturating_sub(1) as usize;

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

    // Create log widget with scrolling using cached formatted text
    // We clone here, but this is still more efficient than creating individual spans
    let log_widget = Paragraph::new(state.formatted_log_text.clone())
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
        .wrap(Wrap { trim: false })
        .scroll((actual_scroll as u16, 0));

    f.render_widget(log_widget, size);
}

/// Renders the search bar at the bottom of the screen (vi-style)
fn render_search_bar(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    state: &AppState,
    styles: &UiStyles,
) {
    use ratatui::text::{Line, Span};

    // Determine if we're in search mode (editing) or filter mode (applied)
    let is_editing = state.view_state == ViewState::SearchMode;

    let search_text = if is_editing {
        // In search mode: show "/" prefix for editing
        format!("/{}", state.search_input.value())
    } else {
        // Filter applied: show "Filtering: " prefix
        format!("Filtering: {}", state.search_input.value())
    };

    // Create a paragraph with the search text using the search_bar style
    let search_widget = Paragraph::new(Line::from(vec![Span::styled(
        search_text,
        styles.search_bar,
    )]));

    f.render_widget(search_widget, area);

    // Only show cursor if we're in search mode (editing)
    if is_editing {
        // Set the cursor position for the input
        // Cursor should be after the '/' and the current input text
        let cursor_x = area.x + 1 + state.search_input.visual_cursor() as u16;
        let cursor_y = area.y;

        // Make cursor visible at the input position
        f.set_cursor_position((cursor_x, cursor_y));
    }
}

/// Renders connection error notifications in the top right corner
fn render_error_notifications(f: &mut Frame, state: &mut AppState, styles: &UiStyles) {
    // Clean up old errors (older than 10 seconds)
    state
        .connection_errors
        .retain(|_, (_, timestamp)| timestamp.elapsed().as_secs() < 10);

    if state.connection_errors.is_empty() {
        return;
    }

    let screen_area = f.area();

    // Stack errors vertically from the top
    let mut y_offset = 0;

    for (host_id, (error_msg, _)) in &state.connection_errors {
        // Shorten the error message if it's too long
        let display_msg = if error_msg.len() > 80 {
            format!("{}...", &error_msg[..77])
        } else {
            error_msg.clone()
        };

        let error_text = format!("✗ {}: {}", host_id, display_msg);
        let error_width = (error_text.len() + 4).min(80) as u16; // +4 for borders and padding
        let error_height = 3; // Border + text + border

        // Position in top right corner, stacked vertically
        let error_area = Rect {
            x: screen_area.width.saturating_sub(error_width),
            y: y_offset,
            width: error_width,
            height: error_height,
        };

        // Create error notification with red styling from UiStyles
        let error_widget = Paragraph::new(Line::from(vec![Span::styled(
            error_text,
            styles.high.add_modifier(Modifier::BOLD),
        )]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(styles.high),
        )
        .alignment(Alignment::Left);

        f.render_widget(error_widget, error_area);

        y_offset += error_height;
    }
}
