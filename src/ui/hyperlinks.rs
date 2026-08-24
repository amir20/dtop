//! OSC 8 hyperlinks: the [`Hyperlink`] widget and terminal support detection.
//!
//! Terminals that understand [OSC 8] turn escape-delimited text into a clickable
//! link. Terminals that don't generally *swallow* the sequence rather than print
//! it, so the failure mode is "no link" rather than "garbage" — but multiplexers
//! are the exception, so we sniff the environment and default to off when we
//! can't tell.
//!
//! [OSC 8]: https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda

use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::Widget;
use std::num::NonZeroU16;
use std::str::FromStr;
use unicode_width::UnicodeWidthChar;

/// Start of an OSC 8 hyperlink: `ESC ] 8 ; ;`
const OSC8_PREFIX: &str = "\u{1b}]8;;";
/// String Terminator: `ESC \`
const ST: &str = "\u{1b}\\";

/// When to render OSC 8 hyperlinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HyperlinkMode {
    /// Enable only when the terminal is recognized as supporting OSC 8.
    #[default]
    Auto,
    /// Always render hyperlinks, whatever the terminal looks like.
    Always,
    /// Never render hyperlinks.
    Never,
}

impl FromStr for HyperlinkMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "always" | "true" | "yes" | "on" => Ok(Self::Always),
            "never" | "false" | "no" | "off" => Ok(Self::Never),
            other => Err(format!(
                "invalid hyperlink mode '{other}' (expected auto, always, or never)"
            )),
        }
    }
}

impl HyperlinkMode {
    /// Resolves the mode against the current environment.
    pub fn enabled(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => detect_osc8_support(&EnvVars),
        }
    }
}

/// A clickable [OSC 8] hyperlink.
///
/// The label is written into the buffer as ordinary per-cell text, and only the
/// *first* and *last* cells carry the escape sequences — the opening sequence is
/// prepended to the first grapheme, the terminator appended to the last. Both
/// are tagged [`CellDiffOption::ForcedWidth`] with the grapheme's real width.
///
/// That last part is load-bearing. `Buffer::diff` derives how many terminal
/// columns a cell occupies from its symbol's *display width*, and an escape
/// sequence measures dozens of columns wide even though it prints nothing. Left
/// unforced, the diff advances by that bogus width and silently swallows every
/// following cell on the row — which, in a table, means the columns to the right
/// of the link stop being drawn. `ForcedWidth` is ratatui's escape hatch for
/// exactly this ("Escape sequences will have some computed width that does match
/// what is written to the screen").
///
/// Keeping the label as normal cell text also means the row still diffs, snapshots
/// and copy-pastes as its plain text.
///
/// [OSC 8]: https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
#[derive(Debug, Clone)]
pub struct Hyperlink<'a> {
    label: &'a str,
    url: String,
    style: Style,
}

impl<'a> Hyperlink<'a> {
    pub fn new(label: &'a str, url: String) -> Self {
        Self {
            label,
            url,
            style: Style::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

/// Rejects URLs that could break out of the OSC 8 sequence.
///
/// The URL is written verbatim between `ESC ] 8 ; ;` and the terminator, so a
/// control character in it would be forwarded straight to the terminal and could
/// close the sequence early and inject arbitrary escapes. `dozzle_url` is
/// free-form YAML from a config file that may have come from a shared or synced
/// location, so it is not inherently trustworthy.
///
/// Failing closed (no link) rather than stripping: a URL containing control
/// characters is malformed anyway, and silently rewriting it could point the
/// link somewhere the user did not configure.
fn is_safe_url(url: &str) -> bool {
    !url.is_empty() && !url.chars().any(char::is_control)
}

impl Widget for Hyperlink<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || !is_safe_url(&self.url) {
            return;
        }

        // Lay the label out cell by cell, dropping anything that doesn't fit.
        // Container names are ASCII per Docker's naming rules, but width-aware
        // placement keeps a wide grapheme from straddling the column edge.
        let max_x = area.x.saturating_add(area.width);
        let mut placed: Vec<(u16, char, u16)> = Vec::new();
        let mut x = area.x;
        for ch in self.label.chars() {
            let width = ch.width().unwrap_or(0) as u16;
            if width == 0 {
                continue;
            }
            if x.saturating_add(width) > max_x {
                break;
            }
            placed.push((x, ch, width));
            x += width;
        }

        let (Some(&(first_x, first_char, first_width)), Some(&(last_x, last_char, last_width))) =
            (placed.first(), placed.last())
        else {
            return;
        };

        for &(cell_x, ch, _) in &placed {
            if let Some(cell) = buf.cell_mut(Position::new(cell_x, area.y)) {
                cell.set_char(ch);
                cell.set_style(self.style);
            }
        }

        if first_x == last_x {
            // Single-cell label: the whole link lives in one cell.
            self.write_cell(
                buf,
                first_x,
                area.y,
                &format!("{OSC8_PREFIX}{}{ST}{first_char}{OSC8_PREFIX}{ST}", self.url),
                first_width,
            );
            return;
        }

        self.write_cell(
            buf,
            first_x,
            area.y,
            &format!("{OSC8_PREFIX}{}{ST}{first_char}", self.url),
            first_width,
        );
        self.write_cell(
            buf,
            last_x,
            area.y,
            &format!("{last_char}{OSC8_PREFIX}{ST}"),
            last_width,
        );
    }
}

impl Hyperlink<'_> {
    fn write_cell(&self, buf: &mut Buffer, x: u16, y: u16, symbol: &str, width: u16) {
        if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
            cell.set_symbol(symbol);
            cell.set_style(self.style);
            // Without this the diff would treat the escape sequence as visible
            // columns and skip the rest of the row.
            if let Some(width) = NonZeroU16::new(width) {
                cell.set_diff_option(CellDiffOption::ForcedWidth(width));
            }
        }
    }
}

/// Environment lookup, abstracted so the detection logic is testable without
/// mutating process-global state.
trait Env {
    fn get(&self, key: &str) -> Option<String>;

    fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

struct EnvVars;

impl Env for EnvVars {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// `TERM_PROGRAM` values for terminals that render OSC 8 hyperlinks.
///
/// Deliberately excludes `Apple_Terminal`: macOS Terminal.app has no OSC 8
/// support, so a link there is invisible and only costs us the underline.
const SUPPORTED_TERM_PROGRAMS: &[&str] = &[
    "iTerm.app", // iTerm2 >= 3.1
    "WezTerm",   // since 2018
    "ghostty",   // since 2024
    "vscode",    // integrated terminal (xterm.js)
    "Hyper",
    "rio",
    "Tabby",
    "WarpTerminal",
];

/// `TERM` values (or prefixes) for terminals that render OSC 8 hyperlinks.
const SUPPORTED_TERMS: &[&str] = &[
    "xterm-kitty", // kitty >= 0.19
    "wezterm",
    "foot",      // foot >= 1.7.0
    "contour",   // contour
    "alacritty", // alacritty >= 0.11
    "rio",
];

/// Minimum `VTE_VERSION` with OSC 8 support: VTE 0.50 reports as 5000.
const MIN_VTE_VERSION: u32 = 5000;

fn detect_osc8_support(env: &impl Env) -> bool {
    let term = env.get("TERM").unwrap_or_default();

    // A dumb terminal renders nothing fancy.
    if term == "dumb" || term.is_empty() {
        return false;
    }

    // Multiplexers only pass OSC 8 through when explicitly configured (tmux
    // >= 3.4 needs `set -ga terminal-features "*:hyperlinks"`), and we can't
    // detect that without shelling out. Stay off; `hyperlinks: always` is the
    // escape hatch.
    if env.has("TMUX") || term.starts_with("screen") || term.starts_with("tmux") {
        return false;
    }

    if let Some(term_program) = env.get("TERM_PROGRAM")
        && SUPPORTED_TERM_PROGRAMS
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&term_program))
    {
        return true;
    }

    if SUPPORTED_TERMS.iter().any(|t| term.starts_with(t)) {
        return true;
    }

    // Windows Terminal >= 1.4
    if env.has("WT_SESSION") {
        return true;
    }

    // Alacritty >= 0.12 exports this even when TERM is xterm-256color.
    if env.has("ALACRITTY_WINDOW_ID") {
        return true;
    }

    // Konsole has OSC 8 since 2020, though it's off by default there; the
    // sequence is ignored when disabled, so enabling costs nothing.
    if env.has("KONSOLE_VERSION") {
        return true;
    }

    // VTE-based terminals (GNOME Terminal, Console, Tilix, xfce4-terminal, ...)
    if let Some(vte) = env.get("VTE_VERSION")
        && vte.parse::<u32>().is_ok_and(|v| v >= MIN_VTE_VERSION)
    {
        return true;
    }

    false
}

#[cfg(test)]
mod widget_tests {
    use super::*;
    use ratatui::layout::Rect;

    fn render(label: &str, width: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        Hyperlink::new(label, "https://example.com/x".to_string())
            .render(Rect::new(2, 0, width, 1), &mut buf);
        buf
    }

    #[test]
    fn test_escapes_only_on_first_and_last_cell() {
        let buf = render("nginx", 10);
        assert_eq!(
            buf[(2, 0)].symbol(),
            "\u{1b}]8;;https://example.com/x\u{1b}\\n"
        );
        assert_eq!(buf[(3, 0)].symbol(), "g");
        assert_eq!(buf[(4, 0)].symbol(), "i");
        assert_eq!(buf[(5, 0)].symbol(), "n");
        assert_eq!(buf[(6, 0)].symbol(), "x\u{1b}]8;;\u{1b}\\");
        // Nothing outside the label is touched.
        assert_eq!(buf[(7, 0)].symbol(), " ");
        assert_eq!(buf[(1, 0)].symbol(), " ");
    }

    /// The escape-carrying cells MUST declare their real column width. Without
    /// this, `Buffer::diff` measures the escape sequence as visible columns and
    /// skips the rest of the row. See `Hyperlink`'s docs.
    #[test]
    fn test_escape_cells_force_their_real_width() {
        let buf = render("nginx", 10);
        let one = NonZeroU16::new(1).unwrap();
        assert_eq!(
            buf[(2, 0)].diff_option,
            CellDiffOption::ForcedWidth(one),
            "opening cell must force width 1"
        );
        assert_eq!(
            buf[(6, 0)].diff_option,
            CellDiffOption::ForcedWidth(one),
            "closing cell must force width 1"
        );
        // Plain label cells need no override.
        assert_eq!(buf[(3, 0)].diff_option, CellDiffOption::None);
    }

    #[test]
    fn test_single_cell_label_carries_both_sequences() {
        let buf = render("n", 1);
        assert_eq!(
            buf[(2, 0)].symbol(),
            "\u{1b}]8;;https://example.com/x\u{1b}\\n\u{1b}]8;;\u{1b}\\"
        );
        assert_eq!(
            buf[(2, 0)].diff_option,
            CellDiffOption::ForcedWidth(NonZeroU16::new(1).unwrap())
        );
    }

    #[test]
    fn test_label_is_clipped_to_the_area() {
        let buf = render("nginx", 3);
        assert!(buf[(2, 0)].symbol().ends_with('n'));
        assert_eq!(buf[(3, 0)].symbol(), "g");
        assert_eq!(buf[(4, 0)].symbol(), "i\u{1b}]8;;\u{1b}\\");
        assert_eq!(buf[(5, 0)].symbol(), " ", "must not write past the area");
    }

    #[test]
    fn test_wide_grapheme_not_split_across_the_edge() {
        // Two double-width chars need 4 columns; only 3 are available, so the
        // second must be dropped rather than straddle the boundary.
        let buf = render("日本", 3);
        // Only 日 fits, so this is the single-cell case: both sequences land in
        // the one cell, wrapped around the character.
        assert_eq!(
            buf[(2, 0)].symbol(),
            "\u{1b}]8;;https://example.com/x\u{1b}\\日\u{1b}]8;;\u{1b}\\"
        );
        assert_eq!(
            buf[(2, 0)].diff_option,
            CellDiffOption::ForcedWidth(NonZeroU16::new(2).unwrap()),
            "a wide grapheme must force width 2, not 1"
        );
        // 本 was dropped rather than straddling the boundary.
        assert_eq!(buf[(4, 0)].symbol(), " ");
    }

    /// The URL is emitted verbatim inside the escape sequence, so a control
    /// character in it could terminate the sequence early and inject arbitrary
    /// escapes into the terminal. Fail closed instead.
    #[test]
    fn test_url_with_control_characters_renders_no_link() {
        for hostile in [
            "https://example.com\u{1b}]0;pwned\u{7}",
            "https://example.com\u{7}",
            "https://exa\nmple.com",
            "https://example.com\u{1b}\\",
        ] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
            Hyperlink::new("nginx", hostile.to_string()).render(Rect::new(2, 0, 10, 1), &mut buf);
            for x in 0..20 {
                assert!(
                    !buf[(x, 0)].symbol().contains('\u{1b}'),
                    "no escape should be emitted for {hostile:?}"
                );
            }
            // Nothing is drawn at all, leaving whatever was already rendered
            // there (in practice the table's own plain-text name).
            assert_eq!(buf[(2, 0)].symbol(), " ");
        }
    }

    /// Control characters in a label are zero-width and get dropped during
    /// placement, so a label cannot inject escapes either.
    #[test]
    fn test_control_characters_in_label_are_dropped() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        Hyperlink::new("a\u{1b}[31mb", "https://example.com".to_string())
            .render(Rect::new(2, 0, 10, 1), &mut buf);
        let rendered: String = (2..12).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(
            !rendered.contains("\u{1b}["),
            "label escapes must not survive placement, got {rendered:?}"
        );
    }

    #[test]
    fn test_empty_inputs_render_nothing() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        Hyperlink::new("", "https://example.com".to_string())
            .render(Rect::new(0, 0, 10, 1), &mut buf);
        Hyperlink::new("nginx", String::new()).render(Rect::new(0, 0, 10, 1), &mut buf);
        Hyperlink::new("nginx", "https://example.com".to_string())
            .render(Rect::new(0, 0, 0, 1), &mut buf);
        for x in 0..10 {
            assert_eq!(buf[(x, 0)].symbol(), " ");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeEnv(HashMap<String, String>);

    impl FakeEnv {
        fn new(pairs: &[(&str, &str)]) -> Self {
            Self(
                pairs
                    .iter()
                    .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                    .collect(),
            )
        }
    }

    impl Env for FakeEnv {
        fn get(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
    }

    #[test]
    fn test_mode_parsing() {
        assert_eq!("auto".parse(), Ok(HyperlinkMode::Auto));
        assert_eq!("Always".parse(), Ok(HyperlinkMode::Always));
        assert_eq!("true".parse(), Ok(HyperlinkMode::Always));
        assert_eq!("never".parse(), Ok(HyperlinkMode::Never));
        assert_eq!("off".parse(), Ok(HyperlinkMode::Never));
        assert!("sometimes".parse::<HyperlinkMode>().is_err());
    }

    #[test]
    fn test_explicit_modes_ignore_environment() {
        assert!(HyperlinkMode::Always.enabled());
        assert!(!HyperlinkMode::Never.enabled());
    }

    #[test]
    fn test_detects_supported_term_programs() {
        for program in ["iTerm.app", "WezTerm", "ghostty", "vscode"] {
            let env = FakeEnv::new(&[("TERM", "xterm-256color"), ("TERM_PROGRAM", program)]);
            assert!(detect_osc8_support(&env), "{program} should be supported");
        }
    }

    #[test]
    fn test_detects_supported_terms() {
        for term in ["xterm-kitty", "wezterm", "foot-extra", "alacritty"] {
            let env = FakeEnv::new(&[("TERM", term)]);
            assert!(detect_osc8_support(&env), "{term} should be supported");
        }
    }

    #[test]
    fn test_detects_windows_terminal_and_alacritty_via_marker_vars() {
        let wt = FakeEnv::new(&[("TERM", "xterm-256color"), ("WT_SESSION", "abc")]);
        assert!(detect_osc8_support(&wt));

        let alacritty = FakeEnv::new(&[
            ("TERM", "xterm-256color"),
            ("ALACRITTY_WINDOW_ID", "1234567"),
        ]);
        assert!(detect_osc8_support(&alacritty));
    }

    #[test]
    fn test_vte_version_threshold() {
        let old = FakeEnv::new(&[("TERM", "xterm-256color"), ("VTE_VERSION", "4900")]);
        assert!(!detect_osc8_support(&old));

        let new = FakeEnv::new(&[("TERM", "xterm-256color"), ("VTE_VERSION", "5000")]);
        assert!(detect_osc8_support(&new));
    }

    #[test]
    fn test_multiplexers_are_off_by_default() {
        // tmux can't be probed for `terminal-features "*:hyperlinks"`, so even a
        // supported outer terminal stays off.
        let tmux = FakeEnv::new(&[
            ("TERM", "screen-256color"),
            ("TMUX", "/tmp/tmux-501/default,1,0"),
            ("TERM_PROGRAM", "iTerm.app"),
        ]);
        assert!(!detect_osc8_support(&tmux));

        let screen = FakeEnv::new(&[("TERM", "screen")]);
        assert!(!detect_osc8_support(&screen));
    }

    #[test]
    fn test_unknown_and_dumb_terminals_are_off() {
        assert!(!detect_osc8_support(&FakeEnv::new(&[("TERM", "dumb")])));
        assert!(!detect_osc8_support(&FakeEnv::new(&[])));
        assert!(!detect_osc8_support(&FakeEnv::new(&[(
            "TERM",
            "xterm-256color"
        )])));
        // Terminal.app has no OSC 8 support.
        assert!(!detect_osc8_support(&FakeEnv::new(&[
            ("TERM", "xterm-256color"),
            ("TERM_PROGRAM", "Apple_Terminal"),
        ])));
    }
}
