use crate::core::app_state::AppState;
use crate::core::types::{Column, Container, ContainerState, HealthStatus, SortField, SortState};
use crate::ui::formatters::{format_bytes, format_bytes_per_sec, format_time_elapsed};
use crate::ui::render::UiStyles;
use ratatui::{
    Frame,
    layout::Constraint,
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

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
    let show_progress_bars = width >= 128;

    app_state.sort_containers();

    let visible_columns = app_state.column_config.visible_columns();

    let rows: Vec<Row> = app_state
        .sorted_container_keys
        .iter()
        .filter_map(|key| app_state.containers.get(key))
        .map(|c| {
            create_container_row(
                c,
                styles,
                &visible_columns,
                show_host_column,
                show_progress_bars,
            )
        })
        .collect();

    let header = create_header_row(
        styles,
        &visible_columns,
        show_host_column,
        app_state.sort_state,
    );
    let table = create_table(
        rows,
        header,
        app_state.sorted_container_keys.len(),
        styles,
        &visible_columns,
        show_host_column,
        show_progress_bars,
    );

    f.render_stateful_widget(table, area, &mut app_state.table_state);
}

/// Creates a table row for a single container
fn create_container_row<'a>(
    container: &'a Container,
    styles: &UiStyles,
    visible_columns: &[Column],
    show_host_column: bool,
    show_progress_bars: bool,
) -> Row<'a> {
    let is_running = container.state == ContainerState::Running;

    let cells: Vec<Cell> = visible_columns
        .iter()
        .filter(|col| **col != Column::Host || show_host_column)
        .map(|col| match col {
            Column::Id => Cell::from(container.id.as_str()),
            Column::Status => {
                let (icon, icon_style) =
                    get_status_icon(&container.state, &container.health, styles);
                Cell::from(icon).style(icon_style)
            }
            Column::Name => Cell::from(container.name.as_str()),
            Column::Host => Cell::from(container.host_id.as_str()),
            Column::Cpu => {
                if is_running {
                    let display = if show_progress_bars {
                        create_progress_bar(container.stats.cpu, 20)
                    } else {
                        format!("{:5.1}%", container.stats.cpu)
                    };
                    Cell::from(display).style(get_percentage_style(container.stats.cpu, styles))
                } else {
                    Cell::from(String::new())
                }
            }
            Column::Memory => {
                if is_running {
                    let display = if show_progress_bars {
                        create_memory_progress_bar(
                            container.stats.memory,
                            container.stats.memory_used_bytes,
                            container.stats.memory_limit_bytes,
                            20,
                        )
                    } else {
                        format!("{:5.1}%", container.stats.memory)
                    };
                    Cell::from(display).style(get_percentage_style(container.stats.memory, styles))
                } else {
                    Cell::from(String::new())
                }
            }
            Column::NetTx => {
                if is_running {
                    Cell::from(format_bytes_per_sec(
                        container.stats.network_tx_bytes_per_sec,
                    ))
                } else {
                    Cell::from(String::new())
                }
            }
            Column::NetRx => {
                if is_running {
                    Cell::from(format_bytes_per_sec(
                        container.stats.network_rx_bytes_per_sec,
                    ))
                } else {
                    Cell::from(String::new())
                }
            }
            Column::Uptime => {
                if is_running {
                    Cell::from(format_time_elapsed(container.created.as_ref()))
                } else {
                    Cell::from("N/A".to_string())
                }
            }
        })
        .collect();

    Row::new(cells)
}

/// Writes the progress bar characters (filled + empty) into the given String buffer
fn write_bar(buf: &mut String, filled_width: usize, empty_width: usize) {
    for _ in 0..filled_width {
        buf.push('█');
    }
    for _ in 0..empty_width {
        buf.push('░');
    }
}

/// Creates a text-based progress bar with percentage
fn create_progress_bar(percentage: f64, width: usize) -> String {
    use std::fmt::Write;
    // Clamp the bar visual to 100%, but display the actual percentage value
    let bar_percentage = percentage.clamp(0.0, 100.0);
    let filled_width = ((bar_percentage / 100.0) * width as f64).round() as usize;
    let empty_width = width.saturating_sub(filled_width);

    // Pre-allocate: each bar char is 3 bytes (UTF-8), plus " 100.0%" suffix
    let mut result = String::with_capacity(width * 3 + 8);
    write_bar(&mut result, filled_width, empty_width);
    let _ = write!(result, " {:5.1}%", percentage);
    result
}

/// Creates a text-based progress bar with memory used/limit display
fn create_memory_progress_bar(percentage: f64, used: u64, limit: u64, width: usize) -> String {
    use std::fmt::Write;
    // Clamp the bar visual to 100%, but display the actual percentage value
    let bar_percentage = percentage.clamp(0.0, 100.0);
    let filled_width = ((bar_percentage / 100.0) * width as f64).round() as usize;
    let empty_width = width.saturating_sub(filled_width);

    let mut result = String::with_capacity(width * 3 + 20);
    write_bar(&mut result, filled_width, empty_width);
    let _ = write!(result, " {}/{}", format_bytes(used), format_bytes(limit));
    result
}

/// Returns the status icon and color based on container health (if available) or state
fn get_status_icon(
    state: &ContainerState,
    health: &Option<HealthStatus>,
    styles: &UiStyles,
) -> (String, Style) {
    // Prioritize health status if container has health checks configured
    if let Some(health_status) = health {
        let icon = styles.icons.health(health_status).to_string();
        let style = match health_status {
            HealthStatus::Healthy => Style::default().fg(Color::Green),
            HealthStatus::Unhealthy => Style::default().fg(Color::Red),
            HealthStatus::Starting => Style::default().fg(Color::Yellow),
        };
        return (icon, style);
    }

    // Use state-based icon if no health check is configured
    let icon = styles.icons.state(state).to_string();
    let style = match state {
        ContainerState::Running => Style::default().fg(Color::Green),
        ContainerState::Paused => Style::default().fg(Color::Yellow),
        ContainerState::Restarting => Style::default().fg(Color::Yellow),
        ContainerState::Removing => Style::default().fg(Color::Yellow),
        ContainerState::Exited => Style::default().fg(Color::Red),
        ContainerState::Dead => Style::default().fg(Color::Red),
        ContainerState::Created => Style::default().fg(Color::Cyan),
        ContainerState::Unknown => Style::default().fg(Color::Gray),
    };
    (icon, style)
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
    visible_columns: &[Column],
    show_host_column: bool,
    sort_state: SortState,
) -> Row<'static> {
    let sort_symbol = sort_state.direction.symbol();
    let sort_field = sort_state.field;

    let headers: Vec<String> = visible_columns
        .iter()
        .filter(|col| **col != Column::Host || show_host_column)
        .map(|col| match col {
            Column::Status => "".to_string(),
            Column::Name => {
                if sort_field == SortField::Name {
                    format!("Name {}", sort_symbol)
                } else {
                    "Name".to_string()
                }
            }
            Column::Id => "ID".to_string(),
            Column::Host => "Host".to_string(),
            Column::Cpu => {
                if sort_field == SortField::Cpu {
                    format!("CPU % {}", sort_symbol)
                } else {
                    "CPU %".to_string()
                }
            }
            Column::Memory => {
                if sort_field == SortField::Memory {
                    format!("Memory % {}", sort_symbol)
                } else {
                    "Memory %".to_string()
                }
            }
            Column::NetTx => "Net TX".to_string(),
            Column::NetRx => "Net RX".to_string(),
            Column::Uptime => {
                if sort_field == SortField::Uptime {
                    format!("Created {}", sort_symbol)
                } else {
                    "Created".to_string()
                }
            }
        })
        .collect();

    Row::new(headers).style(styles.header).bottom_margin(1)
}

/// Creates the complete table widget
fn create_table<'a>(
    rows: Vec<Row<'a>>,
    header: Row<'static>,
    container_count: usize,
    styles: &UiStyles,
    visible_columns: &[Column],
    show_host_column: bool,
    show_progress_bars: bool,
) -> Table<'a> {
    let cpu_width = if show_progress_bars { 28 } else { 7 };
    let mem_width = if show_progress_bars { 33 } else { 7 };

    let constraints: Vec<Constraint> = visible_columns
        .iter()
        .filter(|col| **col != Column::Host || show_host_column)
        .map(|col| match col {
            Column::Id => Constraint::Length(12),
            Column::Status => Constraint::Length(1),
            Column::Name => Constraint::Min(8),
            Column::Host => Constraint::Length(20),
            Column::Cpu => Constraint::Length(cpu_width),
            Column::Memory => Constraint::Length(mem_width),
            Column::NetTx => Constraint::Length(12),
            Column::NetRx => Constraint::Length(12),
            Column::Uptime => Constraint::Length(15),
        })
        .collect();

    Table::new(rows, constraints)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .padding(ratatui::widgets::Padding::proportional(1))
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
    fn test_create_memory_progress_bar_format() {
        let bar = create_memory_progress_bar(50.0, 512 * 1024 * 1024, 1024 * 1024 * 1024, 20);
        assert!(bar.contains("512M/1G"));
        assert!(bar.contains("██████████")); // 50% filled = 10 blocks
    }

    #[test]
    fn test_create_memory_progress_bar_zero() {
        let bar = create_memory_progress_bar(0.0, 0, 1024 * 1024 * 1024, 20);
        assert!(bar.contains("0B/1G"));
        assert!(bar.starts_with("░░░░░░░░░░░░░░░░░░░░")); // All empty
    }

    #[test]
    fn test_create_memory_progress_bar_full() {
        let bar = create_memory_progress_bar(100.0, 1024 * 1024 * 1024, 1024 * 1024 * 1024, 20);
        assert!(bar.contains("1G/1G"));
        assert!(bar.starts_with("████████████████████")); // All filled
    }

    #[test]
    fn test_create_memory_progress_bar_clamps_over_100() {
        // Bar visual should clamp at 100% even if percentage > 100
        let bar = create_memory_progress_bar(150.0, 1536 * 1024 * 1024, 1024 * 1024 * 1024, 20);
        assert!(bar.starts_with("████████████████████")); // Still fully filled
    }

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
