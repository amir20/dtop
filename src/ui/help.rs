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

    // Create a centered popup (60% width, 70% height)
    let popup_width = (area.width as f32 * 0.6) as u16;
    let popup_height = (area.height as f32 * 0.7) as u16;

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

    // Create help content
    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ↑/↓ or j/k  Navigate containers or scroll logs (1 line)"),
        Line::from("  Enter       Open action menu for container"),
        Line::from("  →/l         View logs for selected container"),
        Line::from("  ←/h         Exit log view"),
        Line::from("  Esc         Close action menu, search, or help"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Log View Scrolling",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  g           Scroll to top"),
        Line::from("  G           Scroll to bottom"),
        Line::from("  Ctrl+U / b  Page up"),
        Line::from("  Ctrl+D / Space  Page down"),
        Line::from("  o           Open container in Dozzle (if configured and available)"),
        Line::from("  a/A         Toggle showing all containers (including stopped)"),
        Line::from("  /           Filter containers by name, id or host"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Sorting",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  u/U         Sort by Created (press again to toggle asc/desc)"),
        Line::from("  n/N         Sort by Name (press again to toggle asc/desc)"),
        Line::from("  c/C         Sort by CPU usage (press again to toggle asc/desc)"),
        Line::from("  m/M         Sort by Memory usage (press again to toggle asc/desc)"),
        Line::from("  s           Cycle through sort fields"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Container Status Icons",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                format!("  {} ", styles.icons.health(&HealthStatus::Healthy)),
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
            Span::raw("Starting"),
        ]),
        Line::from(vec![
            Span::styled(
                format!("  {} ", styles.icons.state(&ContainerState::Running)),
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
        Line::from(vec![
            Span::styled(
                format!("  {} ", styles.icons.state(&ContainerState::Restarting)),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("Restarting  "),
            Span::styled(
                format!("{} ", styles.icons.state(&ContainerState::Created)),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("Created  "),
            Span::styled(
                format!("{} ", styles.icons.state(&ContainerState::Unknown)),
                Style::default().fg(Color::Gray),
            ),
            Span::raw("Unknown"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Resource Usage Colors",
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
