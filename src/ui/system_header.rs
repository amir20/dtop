//! System stats header shown above the container list.
//!
//! A `System` line for the machine dtop runs on (sampled locally, see
//! `core::system_stats`), then per host: a `System` line for SSH hosts (read
//! from their `/proc`, see `docker::remote_stats`) and a `Docker` line. Docker
//! lines are built from what the API exposes: the host's capacity from
//! `docker info`, and usage summed over its running containers.

use std::collections::BTreeSet;
use std::fmt::Write;

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use crate::core::app_state::AppState;
use crate::core::app_state::system::DockerTotals;
use crate::core::types::{HostInfo, SystemStats};
use crate::ui::container_list::{clip_to_width, get_percentage_style, write_bar};
use crate::ui::formatters::{format_bytes_per_sec, write_bytes};
use crate::ui::render::UiStyles;

/// Bars narrower than this are dropped: a few cells of bar says nothing the
/// percentage next to it doesn't.
const MIN_BAR_WIDTH: usize = 8;

/// Rows the container list needs below the header to stay usable. On terminals
/// shorter than that the header is dropped rather than squeezing the list.
const MIN_LIST_ROWS: u16 = 8;

/// Labels longer than this are truncated so one long host name can't push every
/// line's metrics off screen.
const MAX_LABEL_WIDTH: usize = 24;

/// Width the network rates are padded to, e.g. `999.99MB/s`.
const RATE_WIDTH: usize = 10;

/// Width the `used/total` memory text is padded to, e.g. `1006M/11.7G`.
const MEMORY_TEXT_WIDTH: usize = 11;

/// How much optional detail the lines carry. When the terminal is too narrow,
/// detail is shed from the top down: uptime, load, swap, then the core count
/// (with the bars dropped before the core count).
#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct Detail(u8);

impl Detail {
    const MAX: Detail = Detail(4);
    /// Bars are only drawn while at least this much detail fits alongside them
    const MIN_WITH_BARS: Detail = Detail(1);

    fn cores(self) -> bool {
        self.0 >= 1
    }
    fn swap(self) -> bool {
        self.0 >= 2
    }
    fn load(self) -> bool {
        self.0 >= 3
    }
    fn uptime(self) -> bool {
        self.0 >= 4
    }
}

/// Number of rows the header occupies in an area of `area_height` (0 = hidden).
pub fn header_height(state: &AppState, area_height: u16) -> u16 {
    if !state.show_system_header {
        return 0;
    }
    let lines = usize::from(state.system_stats.is_some())
        + state.remote_system_stats.len()
        + state.host_info.len();
    let lines = lines as u16;
    if lines == 0 || area_height < lines + MIN_LIST_ROWS {
        0
    } else {
        lines
    }
}

/// One header line's data, gathered once per frame.
enum Entry<'a> {
    System(&'a SystemStats),
    Docker {
        host_id: &'a str,
        info: &'a HostInfo,
        totals: DockerTotals,
    },
}

impl Entry<'_> {
    fn label(&self) -> &str {
        match self {
            Entry::System(stats) => &stats.hostname,
            Entry::Docker { host_id, .. } => host_id,
        }
    }
}

/// The local machine and `local` host first, then each other host's machine and
/// containers together.
fn entries(state: &AppState) -> Vec<Entry<'_>> {
    let mut entries = Vec::new();
    if let Some(stats) = &state.system_stats {
        entries.push(Entry::System(stats));
    }

    // `local` first so its Docker line sits under the local machine's line
    let hosts: BTreeSet<(bool, &String)> = state
        .remote_system_stats
        .keys()
        .chain(state.host_info.keys())
        .map(|host_id| (host_id != "local", host_id))
        .collect();
    for (_, host_id) in hosts {
        if let Some(stats) = state.remote_system_stats.get(host_id) {
            entries.push(Entry::System(stats));
        }
        if let Some(info) = state.host_info.get(host_id) {
            entries.push(Entry::Docker {
                host_id,
                info,
                totals: state.docker_totals(host_id),
            });
        }
    }
    entries
}

/// Renders the header into `area`, which must be [`header_height`] rows tall.
///
/// The CPU and memory bars stretch so the widest line fills `area`. Every line
/// gets the same bar width, so the bars and the text after them stay aligned.
pub fn render_system_header(f: &mut Frame, area: Rect, state: &AppState, styles: &UiStyles) {
    let entries = entries(state);
    let label_width = entries
        .iter()
        .map(|e| e.label().width())
        .max()
        .unwrap_or(0)
        .min(MAX_LABEL_WIDTH);
    let width = usize::from(area.width);
    let build = |bar_width, detail| -> Vec<Line<'static>> {
        entries
            .iter()
            .map(|e| line(e, label_width, bar_width, detail, styles))
            .collect()
    };
    let text_width = |lines: &[Line]| lines.iter().map(Line::width).max().unwrap_or(0);

    // Lay out without bars to find how much room the text needs, then split
    // what's left between the two bars (each also takes a separating space).
    // Shed swap/load/uptime to make room for bars, but not the core count: the
    // bars only restate the percentage, so they go before it does.
    // Without bars, keep the most detail that fits, or the least detail, which
    // overflows the least.
    let mut fallback = None;
    for level in (0..=Detail::MAX.0).rev() {
        let detail = Detail(level);
        let lines = build(0, detail);
        let text_width = text_width(&lines);
        let bar_width = (width.saturating_sub(text_width) / 2).saturating_sub(1);
        if detail >= Detail::MIN_WITH_BARS && bar_width >= MIN_BAR_WIDTH {
            f.render_widget(Paragraph::new(build(bar_width, detail)), area);
            return;
        }
        if fallback.is_none() && text_width <= width {
            fallback = Some(lines);
        }
    }
    let lines = fallback.unwrap_or_else(|| build(0, Detail(0)));
    f.render_widget(Paragraph::new(lines), area);
}

fn line(
    entry: &Entry<'_>,
    label_width: usize,
    bar_width: usize,
    detail: Detail,
    styles: &UiStyles,
) -> Line<'static> {
    match entry {
        Entry::System(stats) => system_line(stats, label_width, bar_width, detail, styles),
        Entry::Docker {
            host_id,
            info,
            totals,
        } => docker_line(
            host_id,
            info,
            totals,
            label_width,
            bar_width,
            detail,
            styles,
        ),
    }
}

fn system_line(
    stats: &SystemStats,
    label_width: usize,
    bar_width: usize,
    detail: Detail,
    styles: &UiStyles,
) -> Line<'static> {
    let mut spans = label_spans("System", &stats.hostname, label_width, styles);
    spans.extend(cpu_spans(stats.cpu, stats.cpus, bar_width, detail, styles));
    spans.extend(memory_spans(
        stats.memory_used_bytes,
        stats.memory_total_bytes,
        bar_width,
        styles,
    ));
    spans.extend(network_spans(
        stats.net_tx_bytes_per_sec,
        stats.net_rx_bytes_per_sec,
        styles,
    ));

    let mut text = String::new();
    if detail.swap() && stats.swap_total_bytes > 0 {
        text.push_str("  Swap ");
        write_bytes(&mut text, stats.swap_used_bytes);
        text.push('/');
        write_bytes(&mut text, stats.swap_total_bytes);
    }
    let [one, five, fifteen] = stats.load_average;
    if detail.load() && (one > 0.0 || five > 0.0 || fifteen > 0.0) {
        let _ = write!(text, "  Load {one:.2} {five:.2} {fifteen:.2}");
    }
    if detail.uptime() {
        text.push_str("  Up ");
        write_uptime(&mut text, stats.uptime_secs);
    }
    spans.push(Span::raw(text));

    Line::from(spans)
}

#[allow(clippy::too_many_arguments)]
fn docker_line(
    host_id: &str,
    info: &HostInfo,
    totals: &DockerTotals,
    label_width: usize,
    bar_width: usize,
    detail: Detail,
    styles: &UiStyles,
) -> Line<'static> {
    let mut spans = label_spans("Docker", host_id, label_width, styles);
    spans.extend(cpu_spans(
        totals.cpu_percent(info),
        info.cpus as usize,
        bar_width,
        detail,
        styles,
    ));
    spans.extend(memory_spans(
        totals.memory_used_bytes,
        info.memory_total_bytes,
        bar_width,
        styles,
    ));
    spans.extend(network_spans(
        totals.net_tx_bytes_per_sec,
        totals.net_rx_bytes_per_sec,
        styles,
    ));
    spans.push(Span::raw(format!(
        "  {}/{} running",
        totals.running, totals.total
    )));

    Line::from(spans)
}

fn label_spans(
    kind: &'static str,
    label: &str,
    label_width: usize,
    styles: &UiStyles,
) -> Vec<Span<'static>> {
    let (label, width) = clip_to_width(label, label_width as u16);
    let padding = label_width - usize::from(width);
    vec![
        Span::styled(kind, styles.header),
        Span::raw(format!("  {label}{:padding$}  ", "")),
    ]
}

/// CPU usage as a share of the whole machine, with the core count it is
/// measured against so the percentage isn't mistaken for a single core's.
fn cpu_spans(
    cpu: f64,
    cores: usize,
    bar_width: usize,
    detail: Detail,
    styles: &UiStyles,
) -> Vec<Span<'static>> {
    let mut text = String::new();
    write_bar_if_wide(&mut text, cpu, bar_width);
    let _ = write!(text, "{cpu:5.1}%");
    let mut spans = vec![
        Span::styled("CPU ", styles.header),
        Span::styled(text, get_percentage_style(cpu, styles)),
    ];
    if detail.cores() {
        // Singular padded to the plural's width so lines stay aligned
        let unit = if cores == 1 { "core " } else { "cores" };
        spans.push(Span::raw(format!(" of {cores:>2} {unit}")));
    }
    spans
}

fn memory_spans(used: u64, total: u64, bar_width: usize, styles: &UiStyles) -> [Span<'static>; 2] {
    let percent = if total > 0 {
        used as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    let mut text = String::new();
    write_bar_if_wide(&mut text, percent, bar_width);
    let start = text.len();
    write_bytes(&mut text, used);
    text.push('/');
    write_bytes(&mut text, total);
    // Fixed width so the columns after it line up across lines (ASCII only, so
    // byte length is display width)
    let padding = MEMORY_TEXT_WIDTH.saturating_sub(text.len() - start);
    text.extend(std::iter::repeat_n(' ', padding));
    [
        Span::styled("  MEM ", styles.header),
        Span::styled(text, get_percentage_style(percent, styles)),
    ]
}

/// Container network rates summed (Docker lines) or interface rates (System line).
fn network_spans(tx: f64, rx: f64, styles: &UiStyles) -> [Span<'static>; 4] {
    [
        Span::styled("  TX ", styles.header),
        Span::raw(format!("{:<RATE_WIDTH$}", format_bytes_per_sec(tx))),
        Span::styled(" RX ", styles.header),
        Span::raw(format!("{:<RATE_WIDTH$}", format_bytes_per_sec(rx))),
    ]
}

fn write_bar_if_wide(buf: &mut String, percent: f64, bar_width: usize) {
    if bar_width == 0 {
        return;
    }
    let filled = ((percent.clamp(0.0, 100.0) / 100.0) * bar_width as f64).round() as usize;
    write_bar(buf, filled, bar_width - filled);
    buf.push(' ');
}

/// Formats an uptime as its two most significant units, e.g. `3d 4h` or `12m`.
fn write_uptime(buf: &mut String, secs: u64) {
    let (days, hours, minutes) = (secs / 86_400, secs % 86_400 / 3_600, secs % 3_600 / 60);
    let _ = if days > 0 {
        write!(buf, "{days}d {hours}h")
    } else if hours > 0 {
        write!(buf, "{hours}h {minutes}m")
    } else {
        write!(buf, "{minutes}m")
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_shows_two_most_significant_units() {
        let fmt = |secs| {
            let mut s = String::new();
            write_uptime(&mut s, secs);
            s
        };
        assert_eq!(fmt(59), "0m");
        assert_eq!(fmt(12 * 60), "12m");
        assert_eq!(fmt(4 * 3_600 + 12 * 60), "4h 12m");
        assert_eq!(fmt(3 * 86_400 + 4 * 3_600 + 59), "3d 4h");
    }
}
