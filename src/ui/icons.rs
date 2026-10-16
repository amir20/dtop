//! Icon sets for Unicode and Nerd Font rendering

use crate::core::types::{ContainerAction, ContainerState, HealthStatus};

/// Style of icons to display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconStyle {
    /// Standard Unicode icons (default, works everywhere)
    #[default]
    Unicode,
    /// Nerd Font icons (requires Nerd Font installed)
    Nerd,
}

/// Which icon style to use, as configured. `Auto` is resolved once at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconMode {
    /// Nerd Font icons on terminals that ship them built in, Unicode elsewhere.
    #[default]
    Auto,
    Fixed(IconStyle),
}

impl std::str::FromStr for IconMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(IconMode::Auto),
            "unicode" => Ok(IconMode::Fixed(IconStyle::Unicode)),
            "nerd" => Ok(IconMode::Fixed(IconStyle::Nerd)),
            _ => Err(format!(
                "Invalid icon style: '{s}'. Use 'auto', 'unicode' or 'nerd'"
            )),
        }
    }
}

impl IconMode {
    /// Resolves `Auto` by asking the terminal what it is.
    ///
    /// Must run in raw mode, before anything else reads stdin, or the reply is
    /// either echoed or eaten as keystrokes.
    pub fn resolve(self) -> IconStyle {
        match self {
            IconMode::Fixed(style) => style,
            IconMode::Auto => {
                let env = |key: &str| std::env::var(key).ok();
                if in_multiplexer(&env) {
                    // A probe would name tmux/screen, not the terminal behind it,
                    // and a session can be reattached from a different terminal.
                    return IconStyle::Unicode;
                }
                let builtin = match probe_xtversion() {
                    Some(name) => has_builtin_nerd_font(&name),
                    None => env_has_builtin_nerd_font(&env),
                };
                if builtin {
                    IconStyle::Nerd
                } else {
                    IconStyle::Unicode
                }
            }
        }
    }
}

/// How long to wait for the terminal's replies. One round trip over SSH can
/// take 200ms+, and only terminals that answer neither query ever hit this.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

fn in_multiplexer(env: &impl Fn(&str) -> Option<String>) -> bool {
    let term = env("TERM").unwrap_or_default();
    env("TMUX").is_some()
        || env("STY").is_some()
        || term.starts_with("screen")
        || term.starts_with("tmux")
}

/// Whether a terminal, named by its XTVERSION reply, draws Nerd Font icons
/// without the user installing a Nerd Font.
fn has_builtin_nerd_font(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("ghostty") || lower.starts_with("wezterm") {
        return true;
    }
    // kitty replies `kitty(0.36.4)`; built-in Nerd Font symbols came in 0.36.
    if let Some(version) = lower.strip_prefix("kitty(") {
        let mut parts = version
            .split(['.', ')'])
            .map(|p| p.parse::<u32>().unwrap_or(0));
        let major = parts.next().unwrap_or(0);
        let minor = parts.next().unwrap_or(0);
        return (major, minor) >= (0, 36);
    }
    false
}

/// Fallback when the terminal doesn't answer XTVERSION. These variables don't
/// survive SSH (`TERM` is often rewritten too), so this only helps locally.
fn env_has_builtin_nerd_font(env: &impl Fn(&str) -> Option<String>) -> bool {
    let term_program = env("TERM_PROGRAM").unwrap_or_default();
    term_program.eq_ignore_ascii_case("ghostty")
        || term_program.eq_ignore_ascii_case("WezTerm")
        || env("TERM").is_some_and(|t| t == "xterm-ghostty")
}

/// Extracts the terminal name from an XTVERSION reply (`ESC P > | name ESC \`).
fn parse_xtversion(buf: &[u8]) -> Option<String> {
    let start = buf.windows(4).position(|w| w == b"\x1bP>|")? + 4;
    let rest = &buf[start..];
    let end = rest.iter().position(|&b| b == 0x1b || b == 0x07)?;
    Some(String::from_utf8_lossy(&rest[..end]).into_owned())
}

/// Whether `buf` ends with a primary device attributes reply (`ESC [ ? ... c`).
/// Every terminal answers DA1, and replies come back in order, so once it is
/// here there is no XTVERSION reply still on its way.
fn has_da1_reply(buf: &[u8]) -> bool {
    buf.windows(3)
        .rposition(|w| w == b"\x1b[?")
        .is_some_and(|i| buf[i + 3..].contains(&b'c'))
}

/// Asks the terminal its name with XTVERSION, followed by DA1 as a sentinel so
/// a terminal that ignores XTVERSION costs one round trip, not the timeout.
/// The query travels in-band, so this works when dtop runs over SSH.
#[cfg(unix)]
fn probe_xtversion() -> Option<String> {
    use std::io::{IsTerminal, Read, Write};
    use std::os::fd::AsRawFd;

    let mut stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    if !stdin.is_terminal() || !stdout.is_terminal() {
        return None;
    }
    stdout.write_all(b"\x1b[>0q\x1b[c").ok()?;
    stdout.flush().ok()?;

    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 256];
    while !has_da1_reply(&buf) {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let mut fds = libc::pollfd {
            fd: stdin.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd, and the count matches.
        let ready = unsafe { libc::poll(&mut fds, 1, remaining.as_millis() as libc::c_int) };
        if ready <= 0 {
            break;
        }
        match stdin.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    parse_xtversion(&buf)
}

#[cfg(not(unix))]
fn probe_xtversion() -> Option<String> {
    None
}

impl std::fmt::Display for IconStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IconStyle::Unicode => write!(f, "unicode"),
            IconStyle::Nerd => write!(f, "nerd"),
        }
    }
}

/// Icon provider that returns appropriate icons based on style
#[derive(Debug, Clone)]
pub struct Icons {
    style: IconStyle,
}

impl Default for Icons {
    fn default() -> Self {
        Self {
            style: IconStyle::Unicode,
        }
    }
}

impl Icons {
    pub fn new(style: IconStyle) -> Self {
        Self { style }
    }

    /// Get icon for container state
    pub fn state(&self, state: &ContainerState) -> &'static str {
        match self.style {
            IconStyle::Unicode => match state {
                ContainerState::Running => "▶",
                ContainerState::Paused => "⏸",
                ContainerState::Restarting => "↻",
                ContainerState::Removing => "↻",
                ContainerState::Exited => "■",
                ContainerState::Dead => "✖",
                ContainerState::Created => "◆",
                ContainerState::Unknown => "?",
            },
            IconStyle::Nerd => match state {
                ContainerState::Running => "\u{f04b}",    // nf-fa-play
                ContainerState::Paused => "\u{f04c}",     // nf-fa-pause
                ContainerState::Restarting => "\u{f01e}", // nf-fa-refresh
                ContainerState::Removing => "\u{f01e}",   // nf-fa-refresh
                ContainerState::Exited => "\u{f04d}",     // nf-fa-stop
                ContainerState::Dead => "\u{f00d}",       // nf-fa-times
                ContainerState::Created => "\u{f067}",    // nf-fa-plus
                ContainerState::Unknown => "\u{f128}",    // nf-fa-question
            },
        }
    }

    /// Get icon for health status
    pub fn health(&self, status: &HealthStatus) -> &'static str {
        match self.style {
            IconStyle::Unicode => match status {
                HealthStatus::Healthy => "✓",
                HealthStatus::Unhealthy => "✖",
                HealthStatus::Starting => "◐",
            },
            IconStyle::Nerd => match status {
                HealthStatus::Healthy => "\u{f00c}",   // nf-fa-check
                HealthStatus::Unhealthy => "\u{f00d}", // nf-fa-times
                HealthStatus::Starting => "\u{f110}",  // nf-fa-spinner
            },
        }
    }

    /// Get icon for container action
    pub fn action(&self, action: ContainerAction) -> &'static str {
        match self.style {
            IconStyle::Unicode => match action {
                ContainerAction::Start => "▶",
                ContainerAction::Stop => "■",
                ContainerAction::Restart => "↻",
                ContainerAction::Remove => "✕",
                ContainerAction::Shell => ">_",
                ContainerAction::Details => "ℹ",
            },
            IconStyle::Nerd => match action {
                ContainerAction::Start => "\u{f04b}",   // nf-fa-play
                ContainerAction::Stop => "\u{f04d}",    // nf-fa-stop
                ContainerAction::Restart => "\u{f01e}", // nf-fa-refresh
                ContainerAction::Remove => "\u{f1f8}",  // nf-fa-trash
                ContainerAction::Shell => "\u{f120}",   // nf-fa-terminal
                ContainerAction::Details => "\u{f05a}", // nf-fa-info_circle
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn test_mode_parsing() {
        assert_eq!("auto".parse(), Ok(IconMode::Auto));
        assert_eq!("Nerd".parse(), Ok(IconMode::Fixed(IconStyle::Nerd)));
        assert_eq!("unicode".parse(), Ok(IconMode::Fixed(IconStyle::Unicode)));
        assert!("emoji".parse::<IconMode>().is_err());
    }

    #[test]
    fn test_fixed_mode_does_not_probe() {
        assert_eq!(IconMode::Fixed(IconStyle::Nerd).resolve(), IconStyle::Nerd);
        assert_eq!(
            IconMode::Fixed(IconStyle::Unicode).resolve(),
            IconStyle::Unicode
        );
    }

    #[test]
    fn test_parse_xtversion() {
        // Captured from Ghostty over SSH: XTVERSION reply, then DA1.
        let reply = b"\x1bP>|ghostty 1.3.2-main+b1d9492cf\x1b\\\x1b[?62;22;52c";
        assert!(has_da1_reply(reply));
        assert_eq!(
            parse_xtversion(reply).as_deref(),
            Some("ghostty 1.3.2-main+b1d9492cf")
        );

        // A terminal that ignores XTVERSION answers DA1 alone.
        let da1_only = b"\x1b[?1;2c";
        assert!(has_da1_reply(da1_only));
        assert_eq!(parse_xtversion(da1_only), None);

        // XTVERSION arrived but DA1 hasn't yet: keep reading.
        assert!(!has_da1_reply(b"\x1bP>|WezTerm 20240203\x1b\\"));
        assert!(!has_da1_reply(b"\x1bP>|WezTerm 20240203\x1b\\\x1b[?62;2"));
    }

    #[test]
    fn test_builtin_nerd_font_terminals() {
        assert!(has_builtin_nerd_font("ghostty 1.3.2-main+b1d9492cf"));
        assert!(has_builtin_nerd_font("WezTerm 20240203-110809-5046fc22"));
        assert!(has_builtin_nerd_font("kitty(0.36.4)"));
        assert!(has_builtin_nerd_font("kitty(1.0.0)"));
        assert!(!has_builtin_nerd_font("kitty(0.35.2)"));
        assert!(!has_builtin_nerd_font("iTerm2 3.5.0"));
        assert!(!has_builtin_nerd_font("tmux 3.4"));
        assert!(!has_builtin_nerd_font("XTerm(390)"));
    }

    #[test]
    fn test_env_fallback() {
        assert!(env_has_builtin_nerd_font(&env(&[(
            "TERM_PROGRAM",
            "ghostty"
        )])));
        assert!(env_has_builtin_nerd_font(&env(&[(
            "TERM_PROGRAM",
            "WezTerm"
        )])));
        assert!(env_has_builtin_nerd_font(&env(&[(
            "TERM",
            "xterm-ghostty"
        )])));
        assert!(!env_has_builtin_nerd_font(&env(&[
            ("TERM", "xterm-256color"),
            ("TERM_PROGRAM", "iTerm.app"),
        ])));
    }

    #[test]
    fn test_multiplexer_detection() {
        assert!(in_multiplexer(&env(&[(
            "TMUX",
            "/tmp/tmux-501/default,1,0"
        )])));
        assert!(in_multiplexer(&env(&[("STY", "1234.pts-0.box")])));
        assert!(in_multiplexer(&env(&[("TERM", "screen-256color")])));
        assert!(in_multiplexer(&env(&[("TERM", "tmux-256color")])));
        assert!(!in_multiplexer(&env(&[("TERM", "xterm-ghostty")])));
    }
}
