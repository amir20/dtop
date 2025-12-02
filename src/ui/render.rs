use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::core::app_state::AppState;
use crate::core::types::ViewState;

use crate::ui::action_menu::render_action_menu;
use crate::ui::container_list::render_container_list;
use crate::ui::help::render_help_popup;
use crate::ui::icons::{IconStyle, Icons};
use crate::ui::log_view::render_log_view;

/// Pre-allocated styles to avoid recreation every frame
pub struct UiStyles {
    pub high: Style,
    pub medium: Style,
    pub low: Style,
    pub header: Style,
    pub border: Style,
    pub selected: Style,
    pub search_bar: Style,
    pub icons: Icons,
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
            border: Style::default(),
            selected: Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            search_bar: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            icons: Icons::default(),
        }
    }
}

impl UiStyles {
    /// Create UiStyles with a specific icon style
    pub fn with_icon_style(icon_style: IconStyle) -> Self {
        Self {
            icons: Icons::new(icon_style),
            ..Default::default()
        }
    }
}

/// Renders the main UI - either container list, log view, or action menu
pub fn render_ui(f: &mut Frame, state: &mut AppState, styles: &UiStyles) {
    let size = f.area();

    // Render main content
    match &state.view_state {
        ViewState::ContainerList | ViewState::SearchMode => {
            // Calculate unique hosts to determine if host column should be shown
            let unique_hosts: std::collections::HashSet<_> =
                state.containers.keys().map(|key| &key.host_id).collect();
            let show_host_column = unique_hosts.len() > 1;

            render_container_list(f, size, state, styles, show_host_column);
        }
        ViewState::LogView(container_key) => {
            let container_key = container_key.clone();
            render_log_view(f, size, &container_key, state, styles);
        }
        ViewState::ActionMenu(_) => {
            // First render the container list in the background
            let unique_hosts: std::collections::HashSet<_> =
                state.containers.keys().map(|key| &key.host_id).collect();
            let show_host_column = unique_hosts.len() > 1;

            render_container_list(f, size, state, styles, show_host_column);

            // Then render the action menu on top
            render_action_menu(f, state, styles);
        }
    }

    // Render search bar overlay if in SearchMode OR if there's an active filter
    let show_search_bar = state.view_state == ViewState::SearchMode
        || (!state.search_input.value().is_empty() && state.view_state == ViewState::ContainerList);

    if show_search_bar {
        let search_area = ratatui::layout::Rect {
            x: size.x,
            y: size.y + size.height.saturating_sub(1),
            width: size.width,
            height: 1,
        };
        render_search_bar(f, search_area, state, styles);
    }

    // Render help popup on top if shown
    if state.show_help {
        render_help_popup(f, styles);
    }

    // Render connection error notifications in top right corner
    render_error_notifications(f, state, styles);
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
        // Shorten the error message if it's too long and build error text directly
        let error_text = if error_msg.len() > 80 {
            format!("✗ {}: {}...", host_id, &error_msg[..77])
        } else {
            format!("✗ {}: {}", host_id, error_msg)
        };
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
