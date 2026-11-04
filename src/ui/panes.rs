//! Opens Shell and Logs in a terminal pane beside dtop (`--panes`), instead of
//! taking over dtop's screen.
//!
//! The new pane, split to the right of dtop's, runs dtop again as
//! `dtop --host <spec> shell|logs <id>`. Going through dtop rather than the
//! docker CLI keeps every host type working (dtop's own SSH transport, TLS) and
//! gives the pane dtop's log view.
//!
//! Two backends, picked by where dtop runs:
//! - tmux (`$TMUX`, `$TMUX_PANE`): `tmux split-window`. tmux runs on the same
//!   machine as dtop, so this also works when dtop runs on a server over SSH.
//! - Rex (`$REX_SESSION`, `$REX_BLOCK`, `rex` on `PATH`): `rex split`. The `rex`
//!   CLI lives where the app does, so not over SSH.
//!
//! tmux wins when both are set: in tmux inside Rex, dtop's pane is a tmux pane.
//! Without either, dtop falls back to taking over its own screen.

use std::process::{Command, Stdio};

/// Where to put new panes, next to dtop's own
#[derive(Clone, Debug, PartialEq)]
pub enum Panes {
    Tmux { pane: String },
    Rex { session: String, block: String },
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

impl Panes {
    /// Panes are used when the flag is on and dtop runs in a tmux or Rex pane
    /// whose CLI is available
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
        let var = |key| env(key).filter(|value: &String| !value.is_empty());
        if var("TMUX").is_some()
            && let Some(pane) = var("TMUX_PANE")
            && has_program("tmux")
        {
            return Some(Self::Tmux { pane });
        }
        let session = var("REX_SESSION")?;
        let block = var("REX_BLOCK")?;
        has_program("rex").then_some(Self::Rex { session, block })
    }

    /// Opens `kind` for a container in a pane split right of dtop's. Returns
    /// `None` if it couldn't (dtop's own path unknown, or the split failed), so
    /// the caller falls back to the in-place view.
    pub fn open(
        &self,
        kind: PaneKind,
        host_spec: &str,
        container_id: &str,
        label: &str,
    ) -> Option<()> {
        let exe = std::env::current_exe().ok()?;
        let (program, args) =
            self.split_command(kind, &exe.to_string_lossy(), host_spec, container_id, label);
        // Both return as soon as the pane exists, so waiting costs a moment
        // and tells us whether to fall back
        let result = Command::new(program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output();
        match result {
            Ok(out) if out.status.success() => Some(()),
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!("{program} split failed: {}", stderr.trim());
                None
            }
            Err(e) => {
                tracing::warn!("{program} split failed: {e}");
                None
            }
        }
    }

    fn split_command(
        &self,
        kind: PaneKind,
        exe: &str,
        host_spec: &str,
        container_id: &str,
        label: &str,
    ) -> (&'static str, Vec<String>) {
        let (program, mut args): (_, Vec<&str>) = match self {
            // Several arguments after the command run it directly, without a shell
            Self::Tmux { pane } => ("tmux", vec!["split-window", "-h", "-t", pane, "--"]),
            Self::Rex { session, block } => (
                "rex",
                vec![
                    "split",
                    "-s",
                    session,
                    "-b",
                    block,
                    "--split=right",
                    "--label",
                    label,
                    "--",
                ],
            ),
        };
        args.extend([exe, "--host", host_spec, kind.subcommand(), container_id]);
        (program, args.into_iter().map(String::from).collect())
    }
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rex() -> Panes {
        Panes::Rex {
            session: "session:1".into(),
            block: "block:2".into(),
        }
    }

    fn tmux() -> Panes {
        Panes::Tmux { pane: "%3".into() }
    }

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            vars.iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    const REX: [(&str, &str); 2] = [("REX_SESSION", "session:1"), ("REX_BLOCK", "block:2")];
    const TMUX: [(&str, &str); 2] = [("TMUX", "/tmp/tmux-501/default,1,0"), ("TMUX_PANE", "%3")];

    #[test]
    fn detects_rex_only_when_enabled_with_its_cli() {
        assert_eq!(Panes::detect_with(true, env(&REX), |_| true), Some(rex()));
        assert_eq!(Panes::detect_with(false, env(&REX), |_| true), None);
        assert_eq!(Panes::detect_with(true, env(&[]), |_| true), None);
        assert_eq!(Panes::detect_with(true, env(&REX), |_| false), None);
    }

    #[test]
    fn detects_tmux_and_prefers_it_inside_rex() {
        assert_eq!(Panes::detect_with(true, env(&TMUX), |_| true), Some(tmux()));
        let both = [REX, TMUX].concat();
        assert_eq!(Panes::detect_with(true, env(&both), |_| true), Some(tmux()));
        // tmux missing from PATH: Rex is still there
        assert_eq!(
            Panes::detect_with(true, env(&both), |p| p != "tmux"),
            Some(rex())
        );
        // $TMUX without a pane id can't be targeted
        assert_eq!(
            Panes::detect_with(true, env(&[("TMUX", "x")]), |_| true),
            None
        );
    }

    #[test]
    fn rex_splits_right_of_dtop_running_dtop_on_the_same_host() {
        let (program, args) = rex().split_command(
            PaneKind::Shell,
            "/bin/dtop",
            "ssh://me@nas:2222",
            "abc123",
            "shell: web",
        );
        assert_eq!(program, "rex");
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
    }

    #[test]
    fn tmux_splits_dtops_pane_horizontally() {
        let (program, args) =
            tmux().split_command(PaneKind::Logs, "/bin/dtop", "local", "abc123", "logs: web");
        assert_eq!(program, "tmux");
        assert_eq!(
            args,
            [
                "split-window",
                "-h",
                "-t",
                "%3",
                "--",
                "/bin/dtop",
                "--host",
                "local",
                "logs",
                "abc123"
            ]
        );
    }
}
