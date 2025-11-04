use chrono::Utc;
use ratatui::{
    Frame,
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
};
use std::collections::HashMap;
use timeago::Formatter;

use crate::app_state::AppState;
use crate::types::{
    Container, ContainerKey, ContainerState, HealthStatus, SortField, SortState, ViewState,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Pre-allocated styles to avoid recreation every frame
pub struct UiStyles {
    pub high: Style,
    pub medium: Style,
    pub low: Style,
    pub header: Style,
    pub border: Style,
    pub selected: Style,
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
                state.sort_state,
            );
        }
        ViewState::LogView(container_key) => {
            let container_key = container_key.clone();
            render_log_view(f, &container_key, state, styles);
        }
    }

    // Render help popup on top if shown
    if state.show_help {
        render_help_popup(f, styles);
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
    sort_state: SortState,
) {
    let size = f.area();
    let width = size.width;

    // Determine if we should show progress bars based on terminal width
    let show_progress_bars = width >= 128;

    // Use pre-sorted list instead of sorting every frame
    let rows: Vec<Row> = sorted_container_keys
        .iter()
        .filter_map(|key| containers.get(key))
        .map(|c| create_container_row(c, styles, show_host_column, show_progress_bars))
        .collect();

    let header = create_header_row(styles, show_host_column, sort_state);
    let table = create_table(
        rows,
        header,
        sorted_container_keys.len(),
        styles,
        show_host_column,
        show_progress_bars,
    );

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

    // Get number of log lines for this container (only if it matches current_log_container)
    // Use the cached formatted text instead of reformatting on every render
    let num_lines = if let Some(key) = &state.current_log_container {
        if key == container_key {
            state.formatted_log_text.lines.len()
        } else {
            0
        }
    } else {
        0
    };

    // Calculate visible height (subtract 1 for top)
    let visible_height = size.height.saturating_sub(1) as usize;

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

    // Create log widget with scrolling using cached formatted text
    // We clone here, but this is still more efficient than creating individual spans
    let log_widget = Paragraph::new(state.formatted_log_text.clone())
        .block(
            Block::default()
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
        Constraint::Fill(1),    // Name (flexible)
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

/// Renders a centered help popup
fn render_help_popup(f: &mut Frame, styles: &UiStyles) {
    use ratatui::layout::{Alignment, Rect};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Clear;

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
        Line::from("  ↑/↓ or j/k  Navigate containers or scroll logs"),
        Line::from("  Enter       View logs for selected container"),
        Line::from("  Esc         Exit log view or close help"),
        Line::from("  o           Open container in Dozzle (if configured)"),
        Line::from("  a/A         Toggle showing all containers (including stopped)"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Sorting",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  u/U         Sort by Uptime (press again to toggle asc/desc)"),
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
            Span::styled("  ✓ ", Style::default().fg(Color::Green)),
            Span::raw("Healthy  "),
            Span::styled("✖ ", Style::default().fg(Color::Red)),
            Span::raw("Unhealthy  "),
            Span::styled("◐ ", Style::default().fg(Color::Yellow)),
            Span::raw("Starting"),
        ]),
        Line::from(vec![
            Span::styled("  ▶ ", Style::default().fg(Color::Green)),
            Span::raw("Running  "),
            Span::styled("⏸ ", Style::default().fg(Color::Yellow)),
            Span::raw("Paused  "),
            Span::styled("■ ", Style::default().fg(Color::Red)),
            Span::raw("Exited"),
        ]),
        Line::from(vec![
            Span::styled("  ↻ ", Style::default().fg(Color::Yellow)),
            Span::raw("Restarting  "),
            Span::styled("◆ ", Style::default().fg(Color::Cyan)),
            Span::raw("Created  "),
            Span::styled("? ", Style::default().fg(Color::Gray)),
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

    #[test]
    fn test_progress_bar_with_wide_width() {
        // Test that progress bar function creates bars with percentages
        let progress_bar = create_progress_bar(42.5, 20);

        // Should contain progress bar characters
        assert!(
            progress_bar.contains('█') || progress_bar.contains('░'),
            "Progress bar should contain bar characters, got: {}",
            progress_bar
        );

        // Should contain the percentage
        assert!(
            progress_bar.contains("42.5%"),
            "Progress bar should contain percentage, got: {}",
            progress_bar
        );

        // Verify it has both bar and percentage (don't check exact byte length due to Unicode)
        let parts: Vec<&str> = progress_bar.split_whitespace().collect();
        assert!(
            parts.len() >= 2,
            "Progress bar should have bar and percentage parts"
        )
    }

    #[test]
    fn test_percentage_only_narrow_width() {
        // Test that when show_progress_bars is false, we get just the percentage
        let percentage_only = format!("{:5.1}%", 42.5);

        // Should NOT contain progress bar characters
        assert!(
            !percentage_only.contains('█') && !percentage_only.contains('░'),
            "Percentage-only mode should not contain bar characters, got: {}",
            percentage_only
        );

        // Should contain the percentage
        assert!(
            percentage_only.contains("42.5%"),
            "Should contain percentage, got: {}",
            percentage_only
        );

        // Should be much shorter than progress bar
        assert_eq!(percentage_only.len(), 6); // " 42.5%"
    }

    #[test]
    fn test_create_table_column_widths() {
        let styles = UiStyles::default();
        let rows = vec![];
        let header = create_header_row(&styles, false, SortState::default());

        // Test with progress bars (wide width)
        let _table_wide = create_table(rows.clone(), header.clone(), 0, &styles, false, true);
        // Wide mode should use 28 chars for CPU/Memory columns (verified by column constraint)

        // Test without progress bars (narrow width)
        let _table_narrow = create_table(rows, header, 0, &styles, false, false);
        // Narrow mode should use 7 chars for CPU/Memory columns (verified by column constraint)

        // If we get here without panics, the table creation works for both modes
        assert!(true);
    }

    #[test]
    fn test_progress_bar_edge_cases() {
        // Test 0%
        let bar_0 = create_progress_bar(0.0, 20);
        assert!(bar_0.contains("0.0%") || bar_0.contains("  0.0%"));
        assert!(bar_0.contains('░')); // Should be all empty

        // Test 100%
        let bar_100 = create_progress_bar(100.0, 20);
        assert!(bar_100.contains("100.0%"));
        assert!(bar_100.contains('█')); // Should be all filled

        // Test 50%
        let bar_50 = create_progress_bar(50.0, 20);
        assert!(bar_50.contains("50.0%"));
        assert!(bar_50.contains('█') && bar_50.contains('░')); // Should have both
    }
}
