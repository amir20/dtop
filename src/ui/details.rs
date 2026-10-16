use chrono::{DateTime, Local, Utc};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::core::app_state::AppState;
use crate::core::types::{ContainerDetails, ViewState};
use crate::ui::render::UiStyles;

/// Width of the field-name column, so values line up
const LABEL_WIDTH: usize = 11;

/// Renders the details popup for the container in `ViewState::Details`
pub fn render_details(f: &mut Frame, state: &AppState, styles: &UiStyles) {
    let ViewState::Details(ref key) = state.view_state else {
        return;
    };
    let name = state
        .containers
        .get(key)
        .map_or(key.container_id.as_str(), |c| c.name.as_str());

    let lines = match &state.container_details {
        None => vec![Line::from(Span::styled(
            "Loading…",
            Style::default().fg(Color::Gray),
        ))],
        Some(Err(error)) => vec![Line::from(Span::styled(
            format!("Could not inspect: {error}"),
            Style::default().fg(Color::Red),
        ))],
        Some(Ok(details)) => detail_lines(details),
    };

    let area = f.area();
    // Borders (2), a blank line above the footer and the footer itself (2)
    let popup_height = (lines.len() as u16 + 4).min(area.height.saturating_sub(2));
    let popup_width = 80u16.min(area.width.saturating_sub(4));
    let popup_area = Rect::new(
        area.width.saturating_sub(popup_width) / 2,
        area.height.saturating_sub(popup_height) / 2,
        popup_width,
        popup_height,
    );

    f.render_widget(Clear, popup_area);
    let block = Block::default()
        .title(format!(" Details: {name} ({}) ", key.host_id))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(styles.header)
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let body = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        height: inner.height.saturating_sub(2),
        ..inner
    };
    f.render_widget(Paragraph::new(lines), body);

    let footer = Rect {
        y: inner.y + inner.height.saturating_sub(1),
        height: 1,
        ..body
    };
    f.render_widget(
        Paragraph::new("Esc/←: Close").style(Style::default().fg(Color::Gray)),
        footer,
    );
}

fn detail_lines(details: &ContainerDetails) -> Vec<Line<'static>> {
    let warn = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let mut lines = vec![
        field("Image", vec![Span::raw(details.image.clone())]),
        field("Command", vec![Span::raw(details.command.clone())]),
        field("Created", vec![Span::raw(time(details.created))]),
        field("Started", vec![Span::raw(time(details.started))]),
    ];

    if let Some(finished) = details.finished {
        let code = details.exit_code.unwrap_or(0);
        let mut value = vec![Span::styled(
            code.to_string(),
            if code != 0 { warn } else { Style::default() },
        )];
        if details.oom_killed {
            value.push(Span::styled(" (OOM killed)", warn));
        }
        value.push(Span::raw(format!(" at {}", time(Some(finished)))));
        lines.push(field("Last exit", value));
    }
    if let Some(error) = &details.error {
        lines.push(field("Error", vec![Span::styled(error.clone(), warn)]));
    }

    let restarts = match details.restart_count {
        0 => String::new(),
        1 => ", restarted once".to_string(),
        n => format!(", restarted {n} times"),
    };
    lines.push(field(
        "Restart",
        vec![Span::raw(format!("{}{restarts}", details.restart_policy))],
    ));

    list_field(&mut lines, "Ports", &details.ports);
    list_field(&mut lines, "Mounts", &details.mounts);
    list_field(&mut lines, "Networks", &details.networks);
    lines.push(field(
        "Labels",
        vec![Span::raw(details.label_count.to_string())],
    ));
    lines
}

fn field(label: &str, value: Vec<Span<'static>>) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{label:<LABEL_WIDTH$}"),
        Style::default().fg(Color::Cyan),
    )];
    spans.extend(value);
    Line::from(spans)
}

/// One value per line, the first beside the label and the rest indented under it
fn list_field(lines: &mut Vec<Line<'static>>, label: &str, values: &[String]) {
    let Some((first, rest)) = values.split_first() else {
        lines.push(field(label, vec![Span::raw("—")]));
        return;
    };
    lines.push(field(label, vec![Span::raw(first.clone())]));
    for value in rest {
        lines.push(field("", vec![Span::raw(value.clone())]));
    }
}

fn time(value: Option<DateTime<Utc>>) -> String {
    value.map_or_else(
        || "—".to_string(),
        |t| {
            t.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn last_exit_shows_code_and_oom_only_after_a_stop() {
        let mut details = ContainerDetails::default();
        assert!(
            !text(&detail_lines(&details))
                .iter()
                .any(|l| l.starts_with("Last exit"))
        );

        details.finished = Some(Utc::now());
        details.exit_code = Some(137);
        details.oom_killed = true;
        let lines = text(&detail_lines(&details));
        let exit = lines.iter().find(|l| l.starts_with("Last exit")).unwrap();
        assert!(
            exit.starts_with("Last exit  137 (OOM killed) at "),
            "{exit}"
        );
    }
}
