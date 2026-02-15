use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use crate::core::app_state::AppState;
use crate::core::types::{ContainerAction, ViewState};
use crate::ui::render::UiStyles;

/// Renders a centered action menu popup for a specific container
pub fn render_action_menu(f: &mut Frame, state: &mut AppState, styles: &UiStyles) {
    // Only render if we're in ActionMenu view
    let ViewState::ActionMenu(ref container_key) = state.view_state else {
        return;
    };

    // Get the container info
    let Some(container) = state.containers.get(container_key) else {
        return;
    };

    let area = f.area();

    // Create a centered popup (40% width, auto height based on actions)
    let available_actions = ContainerAction::available_for_state(&container.state);

    // If no actions available, don't show the menu
    if available_actions.is_empty() {
        return;
    }

    // Calculate height: title (3 lines) + actions + footer (2 lines) + padding
    let popup_height = (available_actions.len() as u16 + 6).min(area.height.saturating_sub(4));
    let popup_width = 40u16.min(area.width.saturating_sub(4));

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the background area first to prevent bleed-through
    f.render_widget(Clear, popup_area);

    // Create the title with container name
    let title = format!(
        " Actions: {} ({}) ",
        truncate_string(&container.name, 20),
        truncate_string(&container_key.host_id, 10)
    );

    // Render the popup block
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(styles.header)
        .style(Style::default().bg(Color::Black));

    // Calculate inner area for the list
    let inner_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    // Render the border first
    f.render_widget(block, popup_area);

    // Create list items from available actions
    let list_items: Vec<ListItem> = available_actions
        .iter()
        .map(|action| {
            let icon = styles.icons.action(*action);
            let text = format!(" {}  {}", icon, action.display_name());
            ListItem::new(text).style(Style::default().fg(Color::White))
        })
        .collect();

    // Create the list widget
    let list = List::new(list_items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    // Render the list with state
    f.render_stateful_widget(list, inner_area, &mut state.action_menu_state);

    // Render footer with keybindings
    let footer_y = popup_area.y + popup_area.height.saturating_sub(2);
    let footer_area = Rect::new(
        popup_area.x + 2,
        footer_y,
        popup_area.width.saturating_sub(4),
        1,
    );

    let footer_style = Style::default().fg(Color::Gray);
    let footer = ratatui::widgets::Paragraph::new("↑/↓: Navigate  Enter: Execute  Esc/←: Cancel")
        .style(footer_style)
        .alignment(Alignment::Center);

    f.render_widget(footer, footer_area);
}

/// Truncates a string to the specified character length, adding ellipsis if needed
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}
