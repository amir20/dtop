use chrono::Utc;
use ratatui::{
    Frame,
    layout::Constraint,
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};
use timeago::Formatter;

use crate::core::app_state::AppState;
use crate::core::types::{Container, ContainerState, HealthStatus, SortField, SortState};
use crate::ui::render::UiStyles;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Renders the container list view
pub fn render_container_list(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    app_state: &mut AppState,
    styles: &UiStyles,
    show_host_column: bool,
) {
    let width = area.width;

    // Determine if we should show progress bars based on terminal width
    let show_progress_bars = width >= 128;

    // Use pre-sorted list instead of sorting every frame
    let rows: Vec<Row> = app_state
        .sorted_container_keys
        .iter()
        .filter_map(|key| app_state.containers.get(key))
        .map(|c| create_container_row(c, styles, show_host_column, show_progress_bars))
        .collect();

    let header = create_header_row(styles, show_host_column, app_state.sort_state);
    let table = create_table(
        rows,
        header,
        app_state.sorted_container_keys.len(),
        styles,
        show_host_column,
        show_progress_bars,
    );

    f.render_stateful_widget(table, area, &mut app_state.table_state);
}

/// Creates a table row for a single container
fn create_container_row<'a>(
    container: &'a Container,
    styles: &UiStyles,
    show_host_column: bool,
    show_progress_bars: bool,
) -> Row<'a> {
    // Check if container is running
    let is_running = container.state == ContainerState::Running;

    // Only show stats for running containers
    let (cpu_bar, cpu_style) = if is_running {
        let display = if show_progress_bars {
            create_progress_bar(container.stats.cpu, 20)
        } else {
            format!("{:5.1}%", container.stats.cpu)
        };
        (display, get_percentage_style(container.stats.cpu, styles))
    } else {
        (String::new(), Style::default())
    };

    let (memory_bar, memory_style) = if is_running {
        let display = if show_progress_bars {
            create_progress_bar(container.stats.memory, 20)
        } else {
            format!("{:5.1}%", container.stats.memory)
        };
        (
            display,
            get_percentage_style(container.stats.memory, styles),
        )
    } else {
        (String::new(), Style::default())
    };

    let network_tx = if is_running {
        format_bytes_per_sec(container.stats.network_tx_bytes_per_sec)
    } else {
        String::new()
    };

    let network_rx = if is_running {
        format_bytes_per_sec(container.stats.network_rx_bytes_per_sec)
    } else {
        String::new()
    };

    // Format time elapsed since creation - show "N/A" for non-running containers
    let time_elapsed = if is_running {
        format_time_elapsed(container.created.as_ref())
    } else {
        "N/A".to_string()
    };

    // Get status icon and color (health takes priority over state)
    let (icon, icon_style) = get_status_icon(&container.state, &container.health);

    let mut cells = vec![
        Cell::from(container.id.as_str()),
        Cell::from(icon).style(icon_style),
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
        Cell::from(time_elapsed),
    ]);

    Row::new(cells)
}

/// Creates a text-based progress bar with percentage
fn create_progress_bar(percentage: f64, width: usize) -> String {
    // Clamp the bar visual to 100%, but display the actual percentage value
    let bar_percentage = percentage.clamp(0.0, 100.0);
    let filled_width = ((bar_percentage / 100.0) * width as f64).round() as usize;
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

/// Formats the time elapsed since container creation
fn format_time_elapsed(created: Option<&chrono::DateTime<Utc>>) -> String {
    match created {
        Some(created_time) => {
            let formatter = Formatter::new();
            let now = Utc::now();
            formatter.convert_chrono(*created_time, now)
        }
        None => "Unknown".to_string(),
    }
}

/// Returns the status icon and color based on container health (if available) or state
fn get_status_icon(state: &ContainerState, health: &Option<HealthStatus>) -> (String, Style) {
    // Prioritize health status if container has health checks configured
    if let Some(health_status) = health {
        return match health_status {
            HealthStatus::Healthy => ("✓".to_string(), Style::default().fg(Color::Green)),
            HealthStatus::Unhealthy => ("✖".to_string(), Style::default().fg(Color::Red)),
            HealthStatus::Starting => ("◐".to_string(), Style::default().fg(Color::Yellow)),
        };
    }

    // Use state-based icon if no health check is configured
    match state {
        ContainerState::Running => ("▶".to_string(), Style::default().fg(Color::Green)),
        ContainerState::Paused => ("⏸".to_string(), Style::default().fg(Color::Yellow)),
        ContainerState::Restarting => ("↻".to_string(), Style::default().fg(Color::Yellow)),
        ContainerState::Removing => ("↻".to_string(), Style::default().fg(Color::Yellow)),
        ContainerState::Exited => ("■".to_string(), Style::default().fg(Color::Red)),
        ContainerState::Dead => ("✖".to_string(), Style::default().fg(Color::Red)),
        ContainerState::Created => ("◆".to_string(), Style::default().fg(Color::Cyan)),
        ContainerState::Unknown => ("?".to_string(), Style::default().fg(Color::Gray)),
    }
}

/// Returns the appropriate style based on percentage value
fn get_percentage_style(value: f64, styles: &UiStyles) -> Style {
    if value > 80.0 {
        styles.high
    } else if value > 50.0 {
        styles.medium
    } else {
        styles.low
    }
}

/// Creates the table header row
fn create_header_row(
    styles: &UiStyles,
    show_host_column: bool,
    sort_state: SortState,
) -> Row<'static> {
    let sort_symbol = sort_state.direction.symbol();
    let sort_field = sort_state.field;

    let mut headers = vec![
        "ID".to_string(),
        "".to_string(), // Status icon column (no header text)
        if sort_field == SortField::Name {
            format!("Name {}", sort_symbol)
        } else {
            "Name".to_string()
        },
    ];

    if show_host_column {
        headers.push("Host".to_string());
    }

    headers.extend(vec![
        if sort_field == SortField::Cpu {
            format!("CPU % {}", sort_symbol)
        } else {
            "CPU %".to_string()
        },
        if sort_field == SortField::Memory {
            format!("Memory % {}", sort_symbol)
        } else {
            "Memory %".to_string()
        },
        "Net TX".to_string(),
        "Net RX".to_string(),
        if sort_field == SortField::Uptime {
            format!("Uptime {}", sort_symbol)
        } else {
            "Uptime".to_string()
        },
    ]);

    Row::new(headers).style(styles.header).bottom_margin(1)
}

/// Creates the complete table widget
fn create_table<'a>(
    rows: Vec<Row<'a>>,
    header: Row<'static>,
    container_count: usize,
    styles: &UiStyles,
    show_host_column: bool,
    show_progress_bars: bool,
) -> Table<'a> {
    let mut constraints = vec![
        Constraint::Length(12), // Container ID
        Constraint::Length(1),  // Status icon
        Constraint::Min(8),     // Name (minimum 8, flexible)
    ];

    if show_host_column {
        constraints.push(Constraint::Length(20)); // Host
    }

    // Adjust column widths based on whether progress bars are shown
    let cpu_mem_width = if show_progress_bars {
        28 // CPU/Memory progress bar (20 chars + " 100.0%")
    } else {
        7 // Just percentage (" 100.0%")
    };

    constraints.extend(vec![
        Constraint::Length(cpu_mem_width), // CPU
        Constraint::Length(cpu_mem_width), // Memory
        Constraint::Length(12),            // Network TX (1.23MB/s)
        Constraint::Length(12),            // Network RX (4.56MB/s)
        Constraint::Length(15),            // Uptime
    ]);

    Table::new(rows, constraints)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    "dtop v{} - {} containers ('?' for help, 'q' to quit)",
                    VERSION, container_count
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
