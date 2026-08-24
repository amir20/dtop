use crate::core::app_state::AppState;
use crate::core::types::{
    Column, Container, ContainerKey, ContainerState, HealthStatus, SortState,
};
use crate::ui::formatters::{format_bytes_per_sec, format_time_elapsed, write_bytes};
use crate::ui::hyperlinks::Hyperlink;
use crate::ui::render::UiStyles;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};
use std::collections::HashMap;
use unicode_width::UnicodeWidthChar;

/// `Table`'s default `column_spacing`. Mirrored here so hyperlink overlays land
/// on the same x-offsets the table computed.
const COLUMN_SPACING: u16 = 1;

/// Rows the table's vertical layout consumes before the first data row: the
/// header row plus its `bottom_margin(1)`.
const HEADER_ROWS: u16 = 2;

#[cfg(not(test))]
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Fixed in tests so UI snapshots don't shift columns when the real version
/// changes length (e.g. 0.7.9 -> 0.7.10).
#[cfg(test)]
const VERSION: &str = "X.X.X";

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

    // Track visible data rows for page up/down navigation.
    // Layout consumes: proportional(1) block padding (top+bottom = 2 rows) and
    // the header row plus its bottom_margin(1) = 2 rows.
    app_state.last_list_viewport_height = (area.height as usize).saturating_sub(4).max(1);

    app_state.sort_containers();

    // Refresh the reusable visible-columns buffer in place (no per-frame alloc),
    // then borrow it for the rest of the render.
    app_state.refresh_visible_columns();
    let visible_columns = &app_state.visible_columns_cache;

    let rows: Vec<Row> = app_state
        .sorted_container_keys
        .iter()
        .filter_map(|key| app_state.containers.get(key))
        .map(|c| {
            create_container_row(
                c,
                styles,
                visible_columns,
                show_host_column,
                show_progress_bars,
            )
        })
        .collect();

    let header = create_header_row(
        styles,
        visible_columns,
        show_host_column,
        app_state.sort_state,
    );
    let constraints = column_constraints(visible_columns, show_host_column, show_progress_bars);
    let block = list_block(app_state.sorted_container_keys.len(), styles);
    // Capture the table's rect before the block is moved into the table; the
    // hyperlink overlay is positioned relative to it.
    let table_area = block.inner(area);
    let table = create_table(rows, header, constraints.clone(), block, styles);

    f.render_stateful_widget(table, area, &mut app_state.table_state);

    if styles.hyperlinks {
        // `render_rows` writes the first visible index back into the state, so
        // this must read `offset()` *after* the table has rendered.
        render_dozzle_links(
            f,
            table_area,
            &constraints,
            visible_columns,
            show_host_column,
            &app_state.containers,
            &app_state.sorted_container_keys,
            app_state.table_state.offset(),
            styles,
        );
    }
}

/// Overlays OSC 8 hyperlinks on the Name column for containers whose host has a
/// Dozzle URL configured.
///
/// `Table` renders cells from `Text`, which has nowhere to carry an escape
/// sequence, so the link has to be drawn on top of the finished table. That
/// means recomputing where the Name column landed — done by replaying the same
/// `Layout` the table used rather than hand-rolling the arithmetic, so the two
/// can't drift apart.
///
/// Unlike the `o` key (which shells out to `open`), this works over SSH: the
/// escape sequence is interpreted by the user's local terminal emulator.
#[allow(clippy::too_many_arguments)]
fn render_dozzle_links(
    f: &mut Frame,
    table_area: Rect,
    constraints: &[Constraint],
    visible_columns: &[Column],
    show_host_column: bool,
    containers: &HashMap<ContainerKey, Container>,
    sorted_keys: &[ContainerKey],
    offset: usize,
    styles: &UiStyles,
) {
    if table_area.height <= HEADER_ROWS || table_area.width == 0 {
        return;
    }

    // Position of Name among the columns actually handed to the table.
    let Some(name_index) = visible_columns
        .iter()
        .filter(|col| **col != Column::Host || show_host_column)
        .position(|col| *col == Column::Name)
    else {
        return;
    };

    // Mirrors `Table::get_column_widths`. The selection column is zero-width
    // here because the table sets no `highlight_symbol`, so the columns start
    // at the left edge of the table area.
    let column_rects = Layout::horizontal(constraints)
        .flex(Flex::Start)
        .spacing(COLUMN_SPACING)
        .split(Rect::new(0, 0, table_area.width, 1));

    let Some(name_rect) = column_rects.get(name_index) else {
        return;
    };
    if name_rect.width == 0 {
        return;
    }

    let first_row_y = table_area.y + HEADER_ROWS;
    let bottom = table_area.bottom();

    for (row_index, key) in sorted_keys.iter().enumerate().skip(offset) {
        let y = first_row_y + (row_index - offset) as u16;
        if y >= bottom {
            break;
        }

        let Some(container) = containers.get(key) else {
            continue;
        };
        let Some(dozzle_url) = container.dozzle_url.as_deref() else {
            continue;
        };

        // Clip exactly like `Cell` does, so a linked name truncates the same
        // way an unlinked one does.
        let (label, label_width) = clip_to_width(&container.name, name_rect.width);
        if label_width == 0 {
            continue;
        }

        let url = format!(
            "{}/container/{}",
            dozzle_url.trim_end_matches('/'),
            key.container_id
        );

        f.render_widget(
            Hyperlink::new(label, url).style(styles.link),
            Rect::new(table_area.x + name_rect.x, y, label_width, 1),
        );
    }
}

/// Truncates `text` to at most `max_width` display columns, returning the
/// clipped text and its width.
fn clip_to_width(text: &str, max_width: u16) -> (&str, u16) {
    let max_width = max_width as usize;
    let mut width = 0usize;

    for (index, ch) in text.char_indices() {
        let char_width = ch.width().unwrap_or(0);
        if width + char_width > max_width {
            return (&text[..index], width as u16);
        }
        width += char_width;
    }

    (text, width as u16)
}

/// Creates a table row for a single container
fn create_container_row<'a>(
    container: &'a Container,
    styles: &'a UiStyles,
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
            Column::Compose => Cell::from(container.compose_project.as_deref().unwrap_or("")),
            Column::Cpu => {
                if is_running {
                    let display = if show_progress_bars {
                        create_progress_bar(container.stats.cpu, 20)
                    } else {
                        format!("{:5.1}%", container.stats.cpu)
                    };
                    Cell::from(display).style(get_percentage_style(container.stats.cpu, styles))
                } else {
                    Cell::from("")
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
                    Cell::from("")
                }
            }
            Column::Pids => {
                if is_running {
                    let pids = container.stats.pids_current;
                    // A limit of 0 means "no limit"; show "current/limit" when a
                    // limit is set so users can watch it against the cap.
                    let display = if container.stats.pids_limit > 0 {
                        format!("{}/{}", pids, container.stats.pids_limit)
                    } else {
                        pids.to_string()
                    };
                    Cell::from(display)
                } else {
                    Cell::from("")
                }
            }
            Column::NetTx => {
                if is_running {
                    Cell::from(format_bytes_per_sec(
                        container.stats.network_tx_bytes_per_sec,
                    ))
                } else {
                    Cell::from("")
                }
            }
            Column::NetRx => {
                if is_running {
                    Cell::from(format_bytes_per_sec(
                        container.stats.network_rx_bytes_per_sec,
                    ))
                } else {
                    Cell::from("")
                }
            }
            Column::DiskRead => {
                if is_running {
                    Cell::from(format_bytes_per_sec(
                        container.stats.disk_read_bytes_per_sec,
                    ))
                } else {
                    Cell::from("")
                }
            }
            Column::DiskWrite => {
                if is_running {
                    Cell::from(format_bytes_per_sec(
                        container.stats.disk_write_bytes_per_sec,
                    ))
                } else {
                    Cell::from("")
                }
            }
            Column::Uptime => {
                if is_running {
                    Cell::from(format_time_elapsed(container.created.as_ref()))
                } else {
                    Cell::from("N/A")
                }
            }
            Column::Restarts => Cell::from(
                container
                    .restart_count
                    .map(|c| c.to_string())
                    .unwrap_or_default(),
            ),
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
    let _ = write!(result, " {percentage:5.1}%");
    result
}

/// Creates a text-based progress bar with memory used/limit display
fn create_memory_progress_bar(percentage: f64, used: u64, limit: u64, width: usize) -> String {
    // Clamp the bar visual to 100%, but display the actual percentage value
    let bar_percentage = percentage.clamp(0.0, 100.0);
    let filled_width = ((bar_percentage / 100.0) * width as f64).round() as usize;
    let empty_width = width.saturating_sub(filled_width);

    let mut result = String::with_capacity(width * 3 + 20);
    write_bar(&mut result, filled_width, empty_width);
    // Format the byte values directly into `result` to avoid two intermediate
    // String allocations per row.
    result.push(' ');
    write_bytes(&mut result, used);
    result.push('/');
    write_bytes(&mut result, limit);
    result
}

/// Returns the status icon and color based on container health (if available) or state
fn get_status_icon<'a>(
    state: &ContainerState,
    health: &Option<HealthStatus>,
    styles: &'a UiStyles,
) -> (&'a str, Style) {
    // Prioritize health status if container has health checks configured
    if let Some(health_status) = health {
        let icon = styles.icons.health(health_status);
        let style = match health_status {
            HealthStatus::Healthy => Style::default().fg(Color::Green),
            HealthStatus::Unhealthy => Style::default().fg(Color::Red),
            HealthStatus::Starting => Style::default().fg(Color::Yellow),
        };
        return (icon, style);
    }

    // Use state-based icon if no health check is configured
    let icon = styles.icons.state(state);
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
    use std::borrow::Cow;

    let sort_symbol = sort_state.direction.symbol();
    let sort_field = sort_state.field;

    let headers: Vec<Cow<'static, str>> = visible_columns
        .iter()
        .filter(|col| **col != Column::Host || show_host_column)
        .map(|col| {
            let base_label = match col {
                Column::Status => "",
                Column::Name => "Name",
                Column::Id => "ID",
                Column::Host => "Host",
                Column::Compose => "Compose",
                Column::Cpu => "CPU %",
                Column::Memory => "Memory %",
                Column::Pids => "PIDs",
                Column::NetTx => "Net TX",
                Column::NetRx => "Net RX",
                Column::DiskRead => "Disk R",
                Column::DiskWrite => "Disk W",
                Column::Uptime => "Created",
                Column::Restarts => "Restarts",
            };
            if *col == sort_field && !base_label.is_empty() {
                Cow::Owned(format!("{base_label} {sort_symbol}"))
            } else {
                Cow::Borrowed(base_label)
            }
        })
        .collect();

    Row::new(headers).style(styles.header).bottom_margin(1)
}

/// Creates the complete table widget
fn create_table<'a>(
    rows: Vec<Row<'a>>,
    header: Row<'static>,
    constraints: Vec<Constraint>,
    block: Block<'static>,
    styles: &UiStyles,
) -> Table<'a> {
    Table::new(rows, constraints)
        .header(header)
        .block(block)
        .row_highlight_style(styles.selected)
}

/// Builds the column constraints for the table.
///
/// Shared with the hyperlink overlay, which re-runs the same layout to find
/// where the table put each column.
fn column_constraints(
    visible_columns: &[Column],
    show_host_column: bool,
    show_progress_bars: bool,
) -> Vec<Constraint> {
    let cpu_width = if show_progress_bars { 28 } else { 7 };
    let mem_width = if show_progress_bars { 33 } else { 7 };

    visible_columns
        .iter()
        .filter(|col| **col != Column::Host || show_host_column)
        .map(|col| match col {
            Column::Id => Constraint::Length(12),
            Column::Status => Constraint::Length(1),
            Column::Name => Constraint::Min(8),
            Column::Host => Constraint::Length(20),
            Column::Compose => Constraint::Length(20),
            Column::Cpu => Constraint::Length(cpu_width),
            Column::Memory => Constraint::Length(mem_width),
            Column::Pids => Constraint::Length(12),
            Column::NetTx => Constraint::Length(12),
            Column::NetRx => Constraint::Length(12),
            Column::DiskRead => Constraint::Length(12),
            Column::DiskWrite => Constraint::Length(12),
            Column::Uptime => Constraint::Length(15),
            Column::Restarts => Constraint::Length(10),
        })
        .collect()
}

/// Builds the block that wraps the table.
///
/// Returned rather than inlined so the caller can ask it for `inner()` — the
/// hyperlink overlay needs the exact rect the table rendered into, and the
/// title row plus proportional padding make that non-obvious to compute by hand.
fn list_block(container_count: usize, styles: &UiStyles) -> Block<'static> {
    Block::default()
        .borders(Borders::NONE)
        .padding(ratatui::widgets::Padding::proportional(1))
        .title(format!(
            "dtop v{VERSION} - {container_count} containers ('?' for help, 'q' to quit)"
        ))
        .style(styles.border)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_to_width_ascii() {
        assert_eq!(clip_to_width("nginx", 10), ("nginx", 5));
        assert_eq!(clip_to_width("nginx", 5), ("nginx", 5));
        assert_eq!(clip_to_width("nginx", 3), ("ngi", 3));
        assert_eq!(clip_to_width("nginx", 0), ("", 0));
        assert_eq!(clip_to_width("", 8), ("", 0));
    }

    #[test]
    fn test_clip_to_width_wide_chars() {
        // A double-width char must not be split across the column boundary.
        assert_eq!(clip_to_width("日本語", 6), ("日本語", 6));
        assert_eq!(clip_to_width("日本語", 5), ("日本", 4));
        assert_eq!(clip_to_width("日本語", 1), ("", 0));
    }

    /// The hyperlink overlay recomputes the Name column's x-offset by replaying
    /// `Table`'s layout. If ratatui ever changes `column_spacing`'s default or
    /// the flex behaviour, this pins the assumption.
    #[test]
    fn test_column_layout_matches_table_defaults() {
        // Render a bare table and check where it actually put column 1.
        let mut rendered = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 20, 3));
        let table = Table::new(
            vec![Row::new(vec!["ab", "cd"])],
            [Constraint::Length(4), Constraint::Length(4)],
        );
        ratatui::widgets::Widget::render(table, Rect::new(0, 0, 20, 3), &mut rendered);

        // Column 0 occupies x=0..4, then one spacing column, so column 1 starts
        // at x=5.
        assert_eq!(rendered[(0, 0)].symbol(), "a");
        assert_eq!(rendered[(5, 0)].symbol(), "c");

        // The overlay's replayed layout must agree with that.
        let rects = Layout::horizontal([Constraint::Length(4), Constraint::Length(4)])
            .flex(Flex::Start)
            .spacing(COLUMN_SPACING)
            .split(Rect::new(0, 0, 20, 1));
        assert_eq!(rects[0].x, 0);
        assert_eq!(rects[1].x, 5);
    }

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
