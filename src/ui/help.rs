use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::core::types::{ContainerState, HealthStatus};
use crate::ui::render::UiStyles;

/// Renders a centered help popup
pub fn render_help_popup(f: &mut Frame, styles: &UiStyles) {
    let area = f.area();

    // Create a centered popup (80% width, 50% height for compact layout)
    let popup_width = (area.width as f32 * 0.8) as u16;
    let popup_height = (area.height as f32 * 0.5) as u16;

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the background area first to prevent bleed-through
    f.render_widget(Clear, popup_area);

    // Render the popup block
    let block = Block::default()
        .title(" Help - Press ? or ESC to close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(styles.header)
        .style(Style::default().bg(Color::Black));

    f.render_widget(block, popup_area);

    // Create help content - compact layout
    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(
            "  ↑/↓, j/k    Navigate/scroll (1 line)    →/l    View logs      ←/h    Exit logs",
        ),
        Line::from(
            "  Enter       Action menu                 Esc    Close menu     ?      Toggle help",
        ),
        Line::from(
            "  a           Show all containers         /      Filter         o      Open Dozzle",
        ),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Log View Scrolling",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(
            "  g/Home      Top              Ctrl+U, b, PgUp    Page up     Ctrl+D, Space, PgDn  Page down",
        ),
        Line::from(
            "  G/End       Bottom",
        ),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Sorting",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  u/U         Uptime       n/N         Name           c/C             CPU"),
        Line::from(
            "  m/M         Memory       s           Cycle          (press again to toggle asc/desc)",
        ),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Status Icons",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                format!("{} ", styles.icons.health(&HealthStatus::Healthy)),
                Style::default().fg(Color::Green),
            ),
            Span::raw("Healthy  "),
            Span::styled(
                format!("{} ", styles.icons.health(&HealthStatus::Unhealthy)),
                Style::default().fg(Color::Red),
            ),
            Span::raw("Unhealthy  "),
            Span::styled(
                format!("{} ", styles.icons.health(&HealthStatus::Starting)),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("Starting  "),
            Span::styled(
                format!("{} ", styles.icons.state(&ContainerState::Running)),
                Style::default().fg(Color::Green),
            ),
            Span::raw("Running  "),
            Span::styled(
                format!("{} ", styles.icons.state(&ContainerState::Paused)),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("Paused  "),
            Span::styled(
                format!("{} ", styles.icons.state(&ContainerState::Exited)),
                Style::default().fg(Color::Red),
            ),
            Span::raw("Exited"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Colors",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Green", styles.low),
            Span::raw(" (0-50%)  "),
            Span::styled("Yellow", styles.medium),
            Span::raw(" (50-80%)  "),
            Span::styled("Red", styles.high),
            Span::raw(" (>80%)"),
        ]),
    ];

    // Calculate inner area (inside the border)
    let inner_area = Rect::new(
        popup_area.x + 2,
        popup_area.y + 2,
        popup_area.width.saturating_sub(4),
        popup_area.height.saturating_sub(3),
    );

    let paragraph = Paragraph::new(help_text)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner_area);
}
