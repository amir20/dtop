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

impl std::str::FromStr for IconStyle {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "unicode" => Ok(IconStyle::Unicode),
            "nerd" => Ok(IconStyle::Nerd),
            _ => Err(format!(
                "Invalid icon style: '{}'. Use 'unicode' or 'nerd'",
                s
            )),
        }
    }
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
            },
            IconStyle::Nerd => match action {
                ContainerAction::Start => "\u{f04b}",   // nf-fa-play
                ContainerAction::Stop => "\u{f04d}",    // nf-fa-stop
                ContainerAction::Restart => "\u{f01e}", // nf-fa-refresh
                ContainerAction::Remove => "\u{f1f8}",  // nf-fa-trash
                ContainerAction::Shell => "\u{f120}",   // nf-fa-terminal
            },
        }
    }
}
