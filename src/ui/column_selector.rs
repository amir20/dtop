use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use crate::core::app_state::AppState;
use crate::ui::render::UiStyles;

/// Renders the column selector popup
pub fn render_column_selector(f: &mut Frame, state: &mut AppState, styles: &UiStyles) {
    let area = f.area();

    let popup_width = (area.width as f32 * 0.5).max(40.0) as u16;
    let popup_height = (area.height as f32 * 0.6).max(14.0) as u16;
    let popup_width = popup_width.min(area.width.saturating_sub(4));
    let popup_height = popup_height.min(area.height.saturating_sub(4));

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let title = if state.column_save_prompt {
        " Save to config? (y/n/esc) "
    } else {
        " Columns "
    };

    let block = Block::default()
        .title(title)
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

    let instruction_area = Rect::new(inner_area.x, inner_area.y, inner_area.width, 1);
    let instruction = ratatui::widgets::Paragraph::new("  Re-order: <PageUp> / <PageDown>")
        .style(Style::default().fg(Color::Gray));
    f.render_widget(instruction, instruction_area);

    let list_area = Rect::new(
        inner_area.x,
        inner_area.y + 2,
        inner_area.width,
        inner_area.height.saturating_sub(4),
    );

    let list_items: Vec<ListItem> = state
        .column_config
        .columns
        .iter()
        .map(|(col, visible)| {
            let checkbox = if *visible { "[X]" } else { "[ ]" };
            let text = format!("  {:<30}{}", col.label(), checkbox);
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

    f.render_stateful_widget(list, list_area, &mut state.column_selector_state);

    let footer_y = popup_area.y + popup_area.height.saturating_sub(2);
    let footer_area = Rect::new(
        popup_area.x + 2,
        footer_y,
        popup_area.width.saturating_sub(4),
        1,
    );

    let footer_text = if state.column_save_prompt {
        "y: Save  n: Don't save  Esc: Cancel"
    } else {
        "Enter/Space: Toggle  Esc: Close  v: Close"
    };

    let footer = ratatui::widgets::Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);

    f.render_widget(footer, footer_area);
}
