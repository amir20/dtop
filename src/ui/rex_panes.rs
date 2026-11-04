//! Opens Shell and Logs in a Rex pane beside dtop (`--panes`), instead of
//! taking over dtop's screen.
//!
//! Rex sets `REX_SESSION` and `REX_BLOCK` in every pane it opens; with those and
//! the `rex` CLI on `PATH`, `rex split` opens a pane to the right of dtop's that
//! runs dtop again, as `dtop --host <spec> shell|logs <id>`. Going through dtop
//! rather than the docker CLI keeps every host type working (dtop's own SSH
//! transport, TLS) and gives the pane dtop's log view. Off without Rex (e.g. over
//! SSH, where Rex's CLI can't reach the app), so dtop falls back to taking over
//! its own screen.

use std::process::{Command, Stdio};

/// Where to put new panes: dtop's own Rex session and pane
#[derive(Clone, Debug, PartialEq)]
pub struct RexPanes {
    session: String,
    block: String,
}

/// What to open in the pane
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaneKind {
    Shell,
    Logs,
}

impl PaneKind {
    fn subcommand(self) -> &'static str {
        match self {
            PaneKind::Shell => "shell",
            PaneKind::Logs => "logs",
        }
    }
}

impl RexPanes {
    /// Panes are used when the flag is on and dtop runs in a Rex pane with the
    /// `rex` CLI available
    pub fn detect(enabled: bool) -> Option<Self> {
        Self::detect_with(enabled, |key| std::env::var(key).ok(), on_path)
    }

    fn detect_with(
        enabled: bool,
        env: impl Fn(&str) -> Option<String>,
        has_program: impl Fn(&str) -> bool,
    ) -> Option<Self> {
        if !enabled {
            return None;
        }
        let session = env("REX_SESSION").filter(|s| !s.is_empty())?;
        let block = env("REX_BLOCK").filter(|s| !s.is_empty())?;
        has_program("rex").then_some(Self { session, block })
    }

    /// Opens `kind` for a container in a pane split right of dtop's. Returns
    /// `None` if it couldn't (dtop's own path unknown, or `rex split` failed),
    /// so the caller falls back to the in-place view.
    pub fn open(
        &self,
        kind: PaneKind,
        host_spec: &str,
        container_id: &str,
        label: &str,
    ) -> Option<()> {
        let exe = std::env::current_exe().ok()?;
        let args = self.split_args(kind, &exe.to_string_lossy(), host_spec, container_id, label);
        // `rex split` returns as soon as the pane exists, so waiting on it
        // costs a moment and tells us whether to fall back
        let result = Command::new("rex")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output();
        match result {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                tracing::warn!(
                    "rex split failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                );
                return None;
            }
            Err(e) => {
                tracing::warn!("rex split failed: {e}");
                return None;
            }
        }
        Some(())
    }

    fn split_args(
        &self,
        kind: PaneKind,
        exe: &str,
        host_spec: &str,
        container_id: &str,
        label: &str,
    ) -> Vec<String> {
        [
            "split",
            "-s",
            &self.session,
            "-b",
            &self.block,
            "--split=right",
            "--label",
            label,
            "--",
            exe,
            "--host",
            host_spec,
            kind.subcommand(),
            container_id,
        ]
        .map(String::from)
        .to_vec()
    }
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rex() -> RexPanes {
        RexPanes {
            session: "session:1".into(),
            block: "block:2".into(),
        }
    }

    fn env(key: &str) -> Option<String> {
        match key {
            "REX_SESSION" => Some("session:1".into()),
            "REX_BLOCK" => Some("block:2".into()),
            _ => None,
        }
    }

    #[test]
    fn detects_only_when_enabled_in_rex_with_its_cli() {
        assert_eq!(RexPanes::detect_with(true, env, |_| true), Some(rex()));
        assert_eq!(RexPanes::detect_with(false, env, |_| true), None);
        assert_eq!(RexPanes::detect_with(true, |_| None, |_| true), None);
        assert_eq!(RexPanes::detect_with(true, env, |_| false), None);
    }

    #[test]
    fn splits_right_of_dtop_running_dtop_on_the_same_host() {
        let args = rex().split_args(
            PaneKind::Shell,
            "/bin/dtop",
            "ssh://me@nas:2222",
            "abc123",
            "shell: web",
        );
        assert_eq!(
            args,
            [
                "split",
                "-s",
                "session:1",
                "-b",
                "block:2",
                "--split=right",
                "--label",
                "shell: web",
                "--",
                "/bin/dtop",
                "--host",
                "ssh://me@nas:2222",
                "shell",
                "abc123"
            ]
        );
        let logs = rex().split_args(PaneKind::Logs, "/bin/dtop", "local", "abc123", "logs: web");
        assert_eq!(logs[10..], ["--host", "local", "logs", "abc123"]);
    }
}
