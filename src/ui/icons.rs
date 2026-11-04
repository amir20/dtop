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
    /// Resolves `Auto` from the terminal's XTVERSION name (`terminal_probe`).
    pub fn resolve(self, probed_name: Option<&str>) -> IconStyle {
        match self {
            IconMode::Fixed(style) => style,
            IconMode::Auto => {
                let env = |key: &str| std::env::var(key).ok();
                let builtin = if in_multiplexer(&env) {
                    // A probe would name tmux/screen, not the terminal behind it.
                    // tmux (>= 3.3) asks the outer terminal itself on attach, so
                    // use its answer; screen has nothing like it. Decided once:
                    // a later reattach from another terminal isn't followed.
                    env("TMUX").is_some()
                        && tmux_client_termtype().is_some_and(|name| has_builtin_nerd_font(&name))
                } else {
                    match probed_name {
                        Some(name) => has_builtin_nerd_font(name),
                        None => env_has_builtin_nerd_font(&env),
                    }
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
    // Rex renders with Ghostty's engine and bundles Symbols Nerd Font.
    // It replies `rex <version>`; don't match names that merely start "rex".
    if lower == "rex" || lower.starts_with("rex ") {
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
        || term_program.eq_ignore_ascii_case("rex")
        || env("TERM").is_some_and(|t| t == "xterm-ghostty")
}

/// The outer terminal's XTVERSION answer, as tmux recorded it when the client
/// attached (`#{client_termtype}`, empty before tmux 3.3 or if it didn't answer).
fn tmux_client_termtype() -> Option<String> {
    let mut child = std::process::Command::new("tmux")
        .args(["display-message", "-p", "#{client_termtype}"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    // tmux answers from memory, but don't let a wedged server hold up startup
    let deadline = std::time::Instant::now() + super::terminal_probe::PROBE_TIMEOUT;
    while child.try_wait().ok()?.is_none() {
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let output = child.wait_with_output().ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && !name.is_empty()).then_some(name)
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
    fn test_fixed_mode_ignores_the_probe() {
        assert_eq!(
            IconMode::Fixed(IconStyle::Nerd).resolve(Some("XTerm(390)")),
            IconStyle::Nerd
        );
        assert_eq!(
            IconMode::Fixed(IconStyle::Unicode).resolve(Some("ghostty 1.3.2")),
            IconStyle::Unicode
        );
    }

    #[test]
    fn test_builtin_nerd_font_terminals() {
        assert!(has_builtin_nerd_font("ghostty 1.3.2-main+b1d9492cf"));
        assert!(has_builtin_nerd_font("WezTerm 20240203-110809-5046fc22"));
        // Captured from Rex: `\x1bP>|rex 0.0.1793645657-gd018d2\x1b\\`
        assert!(has_builtin_nerd_font("rex 0.0.1793645657-gd018d2"));
        assert!(!has_builtin_nerd_font("rexterm 1.0"));
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
        assert!(env_has_builtin_nerd_font(&env(&[("TERM_PROGRAM", "rex")])));
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
