use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use crate::core::app_state::AppState;
use crate::ui::render::UiStyles;

/// Renders the sort selector popup
pub fn render_sort_selector(f: &mut Frame, state: &mut AppState, styles: &UiStyles) {
    let area = f.area();

    let visible_columns = state.column_config.visible_columns();
    let item_count = visible_columns.len() as u16;

    // Dynamic popup height based on visible column count
    let popup_width = 36u16.min(area.width.saturating_sub(4));
    let popup_height = (item_count + 5).min(area.height.saturating_sub(4)); // border(2) + items + footer(1) + padding(2)

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Sort By ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(styles.header)
        .style(Style::default().bg(Color::Black));

    f.render_widget(block, popup_area);

    let inner_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    let list_area = Rect::new(
        inner_area.x,
        inner_area.y,
        inner_area.width,
        inner_area.height.saturating_sub(2),
    );

    let list_items: Vec<ListItem> = visible_columns
        .iter()
        .map(|col| {
            let is_active = state.sort_state.field == *col;
            let indicator = if is_active {
                state.sort_state.direction.symbol()
            } else {
                " "
            };
            let text = format!("  {:<20} {}", col.sort_label(), indicator);
            ListItem::new(text).style(Style::default().fg(Color::White))
        })
        .collect();

    let list = List::new(list_items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, list_area, &mut state.sort_selector_state);

    let footer_y = popup_area.y + popup_area.height.saturating_sub(2);
    let footer_area = Rect::new(
        popup_area.x + 2,
        footer_y,
        popup_area.width.saturating_sub(4),
        1,
    );

    let footer = ratatui::widgets::Paragraph::new("Enter: Select  Esc: Close")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);

    f.render_widget(footer, footer_area);
}
