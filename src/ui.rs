use ratatui::{
    Frame,
    layout::Constraint,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
};
use std::collections::HashMap;

use crate::app_state::AppState;
use crate::types::{Container, ContainerKey, ViewState};

/// Pre-allocated styles to avoid recreation every frame
pub struct UiStyles {
    pub high: Style,
    pub medium: Style,
    pub low: Style,
    pub header: Style,
    pub border: Style,
    pub selected: Style,
    pub timestamp: Style,
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
            timestamp: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        }
    }
}

/// Renders the main UI - either container list or log view
pub fn render_ui(f: &mut Frame, state: &mut AppState, styles: &UiStyles) {
    match &state.view_state {
        ViewState::ContainerList => {
            // Calculate unique hosts to determine if host column should be shown
            let unique_hosts: std::collections::HashSet<_> =
                state.containers.keys().map(|key| &key.host_id).collect();
            let show_host_column = unique_hosts.len() > 1;

            render_container_list(
                f,
                &state.containers,
                &state.sorted_container_keys,
                styles,
                &mut state.table_state,
                show_host_column,
            );
        }
        ViewState::LogView(container_key) => {
            let container_key = container_key.clone();
            render_log_view(f, &container_key, state, styles);
        }
    }
}

/// Renders the container list view
fn render_container_list(
    f: &mut Frame,
    containers: &HashMap<ContainerKey, Container>,
    sorted_container_keys: &[ContainerKey],
    styles: &UiStyles,
    table_state: &mut TableState,
    show_host_column: bool,
) {
    let size = f.area();

    // Use pre-sorted list instead of sorting every frame
    let rows: Vec<Row> = sorted_container_keys
        .iter()
        .filter_map(|key| containers.get(key))
        .map(|c| create_container_row(c, styles, show_host_column))
        .collect();

    let header = create_header_row(styles, show_host_column);
    let table = create_table(rows, header, containers.len(), styles, show_host_column);

    f.render_stateful_widget(table, size, table_state);
}

/// Renders the log view for a specific container
fn render_log_view(
    f: &mut Frame,
    container_key: &ContainerKey,
    state: &mut AppState,
    styles: &UiStyles,
) {
    let size = f.area();

    // Get container info
    let container_name = state
        .containers
        .get(container_key)
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");

    // Get logs for this container (only if it matches current_logs)
    let (log_lines, num_lines) = if let Some((key, logs)) = &state.current_logs {
        if key == container_key {
            // Format log entries into styled lines with colored timestamps
            let lines: Vec<Line> = logs
                .iter()
                .map(|entry| {
                    let timestamp_str = entry.timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
                    Line::from(vec![
                        Span::styled(timestamp_str, styles.timestamp),
                        Span::raw(" "),
                        Span::raw(entry.message.clone()),
                    ])
                })
                .collect();
            let num_lines = lines.len();
            (lines, num_lines)
        } else {
            (Vec::new(), 0)
        }
    } else {
        (Vec::new(), 0)
    };

    // Calculate visible height (subtract 2 for borders)
    let visible_height = size.height.saturating_sub(2) as usize;

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

    // Create log widget with scrolling
    let log_widget = Paragraph::new(log_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
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

/// Creates a table row for a single container
fn create_container_row<'a>(
    container: &'a Container,
    styles: &UiStyles,
    show_host_column: bool,
) -> Row<'a> {
    let cpu_bar = create_progress_bar(container.stats.cpu, 20);
    let cpu_style = get_percentage_style(container.stats.cpu, styles);

    let memory_bar = create_progress_bar(container.stats.memory, 20);
    let memory_style = get_percentage_style(container.stats.memory, styles);

    let network_tx = format_bytes_per_sec(container.stats.network_tx_bytes_per_sec);
    let network_rx = format_bytes_per_sec(container.stats.network_rx_bytes_per_sec);

    let mut cells = vec![
        Cell::from(container.id.as_str()),
        Cell::from(container.name.as_str()),
    ];

    if show_host_column {
        cells.push(Cell::from(container.host_id.as_str()));
    }

    cells.extend(vec![
        Cell::from(cpu_bar).style(cpu_style),
        Cell::from(memory_bar).style(memory_style),
        Cell::from(network_tx),
        Cell::from(network_rx),
        Cell::from(container.status.as_str()),
    ]);

    Row::new(cells)
}

/// Creates a text-based progress bar with percentage
fn create_progress_bar(percentage: f64, width: usize) -> String {
    let percentage = percentage.clamp(0.0, 100.0);
    let filled_width = ((percentage / 100.0) * width as f64).round() as usize;
    let empty_width = width.saturating_sub(filled_width);

    let bar = format!("{}{}", "█".repeat(filled_width), "░".repeat(empty_width));

    format!("{} {:5.1}%", bar, percentage)
}

/// Formats bytes per second into a human-readable string (KB/s, MB/s, GB/s)
fn format_bytes_per_sec(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_per_sec >= GB {
        format!("{:.2}GB/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.2}MB/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.1}KB/s", bytes_per_sec / KB)
    } else {
        format!("{:.0}B/s", bytes_per_sec)
    }
}

/// Returns the appropriate style based on percentage value
pub fn get_percentage_style(value: f64, styles: &UiStyles) -> Style {
    if value > 80.0 {
        styles.high
    } else if value > 50.0 {
        styles.medium
    } else {
        styles.low
    }
}

/// Creates the table header row
fn create_header_row(styles: &UiStyles, show_host_column: bool) -> Row<'static> {
    let mut headers = vec!["ID", "Name"];

    if show_host_column {
        headers.push("Host");
    }

    headers.extend(vec!["CPU %", "Memory %", "Net TX", "Net RX", "Status"]);

    Row::new(headers).style(styles.header).bottom_margin(1)
}

/// Creates the complete table widget
fn create_table<'a>(
    rows: Vec<Row<'a>>,
    header: Row<'static>,
    container_count: usize,
    styles: &UiStyles,
    show_host_column: bool,
) -> Table<'a> {
    let mut constraints = vec![
        Constraint::Length(12), // Container ID
        Constraint::Fill(1),    // Name (flexible)
    ];

    if show_host_column {
        constraints.push(Constraint::Length(20)); // Host
    }

    constraints.extend(vec![
        Constraint::Length(28), // CPU progress bar (20 chars + " 100.0%")
        Constraint::Length(28), // Memory progress bar (20 chars + " 100.0%")
        Constraint::Length(12), // Network TX (1.23MB/s)
        Constraint::Length(12), // Network RX (4.56MB/s)
        Constraint::Length(15), // Status
    ]);

    Table::new(rows, constraints)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    "Docker Container CPU Monitor - {} containers (↑/↓ to navigate, 'q' to quit)",
                    container_count
                ))
                .style(styles.border),
        )
        .row_highlight_style(styles.selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentage_style_thresholds() {
        let styles = UiStyles::default();

        // Test low threshold (green)
        let low_style = get_percentage_style(30.0, &styles);
        assert_eq!(low_style.fg, Some(Color::Green));

        // Test medium threshold (yellow)
        let medium_style = get_percentage_style(65.0, &styles);
        assert_eq!(medium_style.fg, Some(Color::Yellow));

        // Test high threshold (red)
        let high_style = get_percentage_style(85.0, &styles);
        assert_eq!(high_style.fg, Some(Color::Red));

        // Test boundary cases
        assert_eq!(get_percentage_style(50.0, &styles).fg, Some(Color::Green));
        assert_eq!(get_percentage_style(50.1, &styles).fg, Some(Color::Yellow));
        assert_eq!(get_percentage_style(80.0, &styles).fg, Some(Color::Yellow));
        assert_eq!(get_percentage_style(80.1, &styles).fg, Some(Color::Red));
    }
    #[test]
    fn test_color_coding_boundaries() {
        let styles = UiStyles::default();

        // Test exact boundary values
        assert_eq!(
            get_percentage_style(0.0, &styles).fg,
            Some(Color::Green),
            "0% should be green"
        );
        assert_eq!(
            get_percentage_style(50.0, &styles).fg,
            Some(Color::Green),
            "50% should be green"
        );
        assert_eq!(
            get_percentage_style(50.1, &styles).fg,
            Some(Color::Yellow),
            "50.1% should be yellow"
        );
        assert_eq!(
            get_percentage_style(80.0, &styles).fg,
            Some(Color::Yellow),
            "80% should be yellow"
        );
        assert_eq!(
            get_percentage_style(80.1, &styles).fg,
            Some(Color::Red),
            "80.1% should be red"
        );
        assert_eq!(
            get_percentage_style(100.0, &styles).fg,
            Some(Color::Red),
            "100% should be red"
        );
    }
}
