//! System stats header shown above the container list.
//!
//! A `System` line for the machine dtop runs on (sampled locally, see
//! `core::system_stats`), then per host: a `System` line for SSH hosts (read
//! from their `/proc`, see `docker::remote_stats`) and a `Docker` line. Docker
//! lines are built from what the API exposes: the host's capacity from
//! `docker info`, and usage summed over its running containers.

use std::fmt::Write;
use std::ops::Range;

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use crate::core::app_state::AppState;
use crate::core::app_state::system::DockerTotals;
use crate::core::types::{HostInfo, SystemStats};
use crate::ui::container_list::{clip_to_width, get_percentage_style};
use crate::ui::formatters::{write_bytes, write_bytes_per_sec};
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

/// Blank space standing in for the network column on lines without one, so
/// what follows lines up with the lines that have it.
const NETWORK_GAP: &str = "                             ";
const _: () = assert!(NETWORK_GAP.len() == "  TX ".len() + RATE_WIDTH + " RX ".len() + RATE_WIDTH);

/// Width the `used/total` memory text is padded to, e.g. `1006M/11.7G`.
const MEMORY_TEXT_WIDTH: usize = 11;

/// How much optional detail the lines carry. When the terminal is too narrow,
/// detail is shed from the top down: uptime, load, swap, then the core count,
/// then network (with the bars dropped before the core count).
#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct Detail(u8);

impl Detail {
    const MAX: Detail = Detail(5);
    /// Bars are only drawn while at least this much detail fits alongside them
    const MIN_WITH_BARS: Detail = Detail(2);

    fn network(self) -> bool {
        self.0 >= 1
    }
    fn cores(self) -> bool {
        self.0 >= 2
    }
    fn swap(self) -> bool {
        self.0 >= 3
    }
    fn load(self) -> bool {
        self.0 >= 4
    }
    fn uptime(self) -> bool {
        self.0 >= 5
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

/// Cells of bar glyphs kept ready to slice from; wider bars are capped.
const BAR_CAPACITY: usize = 512;

/// Filled and empty bar glyphs, built once so bars are borrowed slices rather
/// than a fresh `String` per bar per frame. Both glyphs are 3 bytes in UTF-8.
static BAR_FILLED: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| "█".repeat(BAR_CAPACITY));
static BAR_EMPTY: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| "░".repeat(BAR_CAPACITY));

/// One header line, formatted once per frame into a single buffer.
///
/// The spans drawn for it borrow from `buf`, and widths for the fitting search
/// are computed from the field ranges, so trying every detail level and drawing
/// the result costs one allocation per line rather than one per field per try.
struct HeaderText {
    kind: &'static str,
    buf: String,
    label: Range<usize>,
    /// Only System lines carry network: the sum of container counters would
    /// count traffic between containers (twice, as one's TX and the other's
    /// RX) that never touches the machine's interfaces, so a Docker line's
    /// "network" could exceed the machine's own.
    has_network: bool,
    cpu: Range<usize>,
    cpu_pct: f64,
    cores: Range<usize>,
    mem: Range<usize>,
    mem_pct: f64,
    tx: Range<usize>,
    rx: Range<usize>,
    swap: Range<usize>,
    load: Range<usize>,
    uptime: Range<usize>,
    running: Range<usize>,
}

/// Appends whatever `write` produces to `buf` and returns where it landed.
fn field(buf: &mut String, write: impl FnOnce(&mut String)) -> Range<usize> {
    let start = buf.len();
    write(buf);
    start..buf.len()
}

/// Pads `buf` with spaces so the text since `start` is `width` columns (ASCII).
fn pad_to(buf: &mut String, start: usize, width: usize) {
    let padding = width.saturating_sub(buf.len() - start);
    buf.extend(std::iter::repeat_n(' ', padding));
}

impl HeaderText {
    fn new(kind: &'static str, label: &str, label_width: usize) -> Self {
        let mut buf = String::with_capacity(160);
        let label = field(&mut buf, |buf| {
            let (label, width) = clip_to_width(label, label_width as u16);
            buf.push_str("  ");
            buf.push_str(label);
            buf.extend(std::iter::repeat_n(
                ' ',
                label_width - usize::from(width) + 2,
            ));
        });
        Self {
            kind,
            buf,
            label,
            has_network: false,
            cpu: 0..0,
            cpu_pct: 0.0,
            cores: 0..0,
            mem: 0..0,
            mem_pct: 0.0,
            tx: 0..0,
            rx: 0..0,
            swap: 0..0,
            load: 0..0,
            uptime: 0..0,
            running: 0..0,
        }
    }

    /// CPU as a share of the whole machine, with the core count it is measured
    /// against so the percentage isn't mistaken for a single core's.
    fn cpu(&mut self, cpu: f64, cores: usize) {
        self.cpu_pct = cpu;
        self.cpu = field(&mut self.buf, |buf| {
            let _ = write!(buf, "{cpu:5.1}%");
        });
        // A host that reported no CPU count gets no "of 0 cores"
        if cores > 0 {
            self.cores = field(&mut self.buf, |buf| {
                // Singular padded to the plural's width so lines stay aligned
                let unit = if cores == 1 { "core " } else { "cores" };
                let _ = write!(buf, " of {cores:>2} {unit}");
            });
        }
    }

    fn memory(&mut self, used: u64, total: u64) {
        self.mem_pct = if total > 0 {
            used as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        self.mem = field(&mut self.buf, |buf| {
            let start = buf.len();
            write_bytes(buf, used);
            buf.push('/');
            write_bytes(buf, total);
            // Fixed width so the columns after it line up across lines
            pad_to(buf, start, MEMORY_TEXT_WIDTH);
        });
    }

    /// Interface rates (System lines only, see `has_network`).
    fn network(&mut self, tx: f64, rx: f64) {
        self.has_network = true;
        for (range, rate) in [(&mut self.tx, tx), (&mut self.rx, rx)] {
            *range = field(&mut self.buf, |buf| {
                let start = buf.len();
                write_bytes_per_sec(buf, rate);
                pad_to(buf, start, RATE_WIDTH);
            });
        }
    }

    fn system(stats: &SystemStats, label_width: usize) -> Self {
        let mut text = Self::new("System", &stats.hostname, label_width);
        text.cpu(stats.cpu, stats.cpus);
        text.memory(stats.memory_used_bytes, stats.memory_total_bytes);
        text.network(stats.net_tx_bytes_per_sec, stats.net_rx_bytes_per_sec);
        if stats.swap_total_bytes > 0 {
            text.swap = field(&mut text.buf, |buf| {
                buf.push_str("  Swap ");
                write_bytes(buf, stats.swap_used_bytes);
                buf.push('/');
                write_bytes(buf, stats.swap_total_bytes);
            });
        }
        let [one, five, fifteen] = stats.load_average;
        if one > 0.0 || five > 0.0 || fifteen > 0.0 {
            text.load = field(&mut text.buf, |buf| {
                let _ = write!(buf, "  Load {one:.2} {five:.2} {fifteen:.2}");
            });
        }
        text.uptime = field(&mut text.buf, |buf| {
            buf.push_str("  Up ");
            write_uptime(buf, stats.uptime_secs);
        });
        text
    }

    fn docker(host_id: &str, info: &HostInfo, totals: &DockerTotals, label_width: usize) -> Self {
        let mut text = Self::new("Docker", host_id, label_width);
        text.cpu(totals.cpu_percent(info), info.cpus as usize);
        text.memory(totals.memory_used_bytes, info.memory_total_bytes);
        text.running = field(&mut text.buf, |buf| {
            let _ = write!(buf, "  {}/{} running", totals.running, totals.total);
        });
        text
    }

    /// Display width at `detail` with bars of `bar_width` cells (0 = no bars),
    /// matching exactly what [`HeaderText::line`] draws. `network_column` is
    /// whether the network column is drawn at all (on some line).
    fn width(&self, detail: Detail, bar_width: usize, network_column: bool) -> usize {
        let bar = if bar_width > 0 { bar_width + 1 } else { 0 };
        let mut width = self.kind.len()
            + self.buf[self.label.clone()].width()
            + "CPU ".len()
            + bar
            + self.cpu.len()
            + "  MEM ".len()
            + bar
            + self.mem.len()
            + self.running.len();
        if detail.cores() {
            width += self.cores.len();
        }
        if network_column {
            width += if self.has_network {
                "  TX ".len() + self.tx.len() + " RX ".len() + self.rx.len()
            } else {
                NETWORK_GAP.len()
            };
        }
        if detail.swap() {
            width += self.swap.len();
        }
        if detail.load() {
            width += self.load.len();
        }
        if detail.uptime() {
            width += self.uptime.len();
        }
        width
    }

    fn line(
        &self,
        detail: Detail,
        bar_width: usize,
        network_column: bool,
        styles: &UiStyles,
    ) -> Line<'_> {
        let text = |range: &Range<usize>| &self.buf[range.clone()];
        let cpu_style = get_percentage_style(self.cpu_pct, styles);
        let mem_style = get_percentage_style(self.mem_pct, styles);

        let mut spans = Vec::with_capacity(20);
        spans.push(Span::styled(self.kind, styles.header));
        spans.push(Span::raw(text(&self.label)));
        spans.push(Span::styled("CPU ", styles.header));
        push_bar(&mut spans, self.cpu_pct, bar_width, cpu_style);
        spans.push(Span::styled(text(&self.cpu), cpu_style));
        if detail.cores() {
            spans.push(Span::raw(text(&self.cores)));
        }
        spans.push(Span::styled("  MEM ", styles.header));
        push_bar(&mut spans, self.mem_pct, bar_width, mem_style);
        spans.push(Span::styled(text(&self.mem), mem_style));
        if network_column && self.has_network {
            spans.push(Span::styled("  TX ", styles.header));
            spans.push(Span::raw(text(&self.tx)));
            spans.push(Span::styled(" RX ", styles.header));
            spans.push(Span::raw(text(&self.rx)));
        } else if network_column {
            spans.push(Span::raw(NETWORK_GAP));
        }
        for (shown, range) in [
            (detail.swap(), &self.swap),
            (detail.load(), &self.load),
            (detail.uptime(), &self.uptime),
            (true, &self.running),
        ] {
            if shown && !range.is_empty() {
                spans.push(Span::raw(text(range)));
            }
        }
        Line::from(spans)
    }
}

/// Pushes a `bar_width`-cell bar and its trailing space, borrowing the glyphs.
fn push_bar<'a>(spans: &mut Vec<Span<'a>>, percent: f64, bar_width: usize, style: Style) {
    if bar_width == 0 {
        return;
    }
    let bar_width = bar_width.min(BAR_CAPACITY);
    let filled = ((percent.clamp(0.0, 100.0) / 100.0) * bar_width as f64).round() as usize;
    let glyph = "█".len();
    spans.push(Span::styled(&BAR_FILLED[..filled * glyph], style));
    spans.push(Span::styled(
        &BAR_EMPTY[..(bar_width - filled) * glyph],
        style,
    ));
    spans.push(Span::raw(" "));
}

/// Builds the header's lines: the local machine and the hosts on it first, then
/// each other host's machine and containers together. Host IDs come from two
/// sorted maps, merged in order without collecting them.
fn header_texts(state: &AppState) -> Vec<HeaderText> {
    let label_width = state
        .system_stats
        .iter()
        .map(|s| s.hostname.width())
        .chain(state.remote_system_stats.keys().map(|h| h.width()))
        .chain(state.host_info.keys().map(|h| h.width()))
        .max()
        .unwrap_or(0)
        .min(MAX_LABEL_WIDTH);

    let mut texts = Vec::with_capacity(
        usize::from(state.system_stats.is_some())
            + state.remote_system_stats.len()
            + state.host_info.len(),
    );
    if let Some(stats) = &state.system_stats {
        texts.push(HeaderText::system(stats, label_width));
    }

    let mut add_host = |host_id: &str| {
        if let Some(stats) = state.remote_system_stats.get(host_id) {
            texts.push(HeaderText::system(stats, label_width));
        }
        if let Some(info) = state.host_info.get(host_id) {
            let totals = state.docker_totals(host_id);
            texts.push(HeaderText::docker(host_id, info, &totals, label_width));
        }
    };

    // Hosts on this machine first so their Docker lines sit under the local
    // machine's line. Ids can be custom names, so this asks the host, not the id.
    let is_local = |host_id: &str| {
        state
            .connected_hosts
            .get(host_id)
            .is_some_and(|host| host.is_local)
    };
    for host_id in sorted_host_ids(state).filter(|id| is_local(id)) {
        add_host(host_id);
    }
    for host_id in sorted_host_ids(state).filter(|id| !is_local(id)) {
        add_host(host_id);
    }
    texts
}

/// Every host id in the header, sorted and deduplicated, merged from the two
/// sorted maps without collecting them.
fn sorted_host_ids(state: &AppState) -> impl Iterator<Item = &str> {
    let mut remote = state.remote_system_stats.keys().peekable();
    let mut docker = state.host_info.keys().peekable();
    std::iter::from_fn(move || {
        let next = match (remote.peek(), docker.peek()) {
            (Some(r), Some(d)) if r == d => {
                docker.next();
                remote.next()
            }
            (Some(r), Some(d)) if r < d => remote.next(),
            (Some(_), None) => remote.next(),
            (_, Some(_)) => docker.next(),
            (None, None) => None,
        };
        next.map(String::as_str)
    })
}

/// Renders the header into `area`, which must be [`header_height`] rows tall.
///
/// The CPU and memory bars stretch so the widest line fills `area`. Every line
/// gets the same bar width, so the bars and the text after them stay aligned.
pub fn render_system_header(f: &mut Frame, area: Rect, state: &AppState, styles: &UiStyles) {
    let texts = header_texts(state);
    let width = usize::from(area.width);
    let any_network = texts.iter().any(|t| t.has_network);
    let network_column = |detail: Detail| detail.network() && any_network;
    let text_width = |detail| {
        texts
            .iter()
            .map(|t| t.width(detail, 0, network_column(detail)))
            .max()
            .unwrap_or(0)
    };

    // Find how much room the text needs without bars, then split what's left
    // between the two bars (each also takes a separating space). Shed
    // swap/load/uptime to make room for bars, but not the core count or
    // network: the bars only restate the percentage, so they go before those
    // do. Without bars, keep the most detail that fits, or the least detail,
    // which overflows the least.
    let mut chosen = None;
    for level in (0..=Detail::MAX.0).rev() {
        let detail = Detail(level);
        let needed = text_width(detail);
        let bar_width = (width.saturating_sub(needed) / 2).saturating_sub(1);
        if detail >= Detail::MIN_WITH_BARS && bar_width >= MIN_BAR_WIDTH {
            chosen = Some((detail, bar_width));
            break;
        }
        if chosen.is_none() && needed <= width {
            chosen = Some((detail, 0));
        }
    }
    let (detail, bar_width) = chosen.unwrap_or((Detail(0), 0));

    let lines: Vec<Line> = texts
        .iter()
        .map(|text| text.line(detail, bar_width, network_column(detail), styles))
        .collect();
    f.render_widget(Paragraph::new(lines), area);
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
