//! Reports what dtop is doing to the terminal with the Program Status Protocol
//! (OSC 7501, <https://www.superlogical.com/rex/docs/build/program-status>), so a
//! terminal that supports it (e.g. Rex) can flag dtop's tab while you look elsewhere.
//! Terminals that don't know the sequence ignore it, so it is always sent.
//!
//! Records, each replaced whole by every report:
//! - root: `working` (with progress) while a bulk action runs, then `done` or
//!   `error` with its summary; `idle` before the first one.
//! - `host/<host>`: `error` while the host is disconnected.
//! - `container/<host>/<id>`: `error` while a running container is unhealthy.
//!
//! Only changes are written, and everything is cleared on quit, since dtop is
//! then no longer watching and `error` records outlive the process.

use std::collections::BTreeMap;

use crate::core::app_state::AppState;
use crate::core::types::{ContainerState, HealthStatus};

const APP: &str = "dtop";
/// Decoded limits are 2048 (`msg`) and 192 (`title`) bytes; stay well inside them
const MAX_MSG_CHARS: usize = 200;
const MAX_TITLE_CHARS: usize = 60;

/// One record's report body, keyed by its id ("" for the root record)
type Reports = BTreeMap<String, String>;

/// Remembers what the terminal was last told, so only changes are written
#[derive(Default)]
pub struct ProgramStatus {
    sent: Reports,
}

impl ProgramStatus {
    /// The escape sequences that bring the terminal up to date with `state`
    /// (empty when nothing changed)
    pub fn update(&mut self, state: &AppState) -> String {
        let wanted = reports(state);
        let mut out = String::new();
        for id in self.sent.keys().filter(|id| !wanted.contains_key(*id)) {
            push_osc(&mut out, &format!("state=clear:id={id}"));
        }
        for (id, body) in &wanted {
            if self.sent.get(id) != Some(body) {
                push_osc(&mut out, body);
            }
        }
        self.sent = wanted;
        out
    }

    /// Removes every record dtop made
    pub fn clear_all() -> String {
        let mut out = String::new();
        push_osc(&mut out, "state=clear");
        out
    }
}

fn push_osc(out: &mut String, body: &str) {
    out.push_str("\x1b]7501;");
    out.push_str(body);
    out.push_str("\x1b\\");
}

fn reports(state: &AppState) -> Reports {
    let mut reports = Reports::new();
    reports.insert(String::new(), root_report(state));

    for host in &state.disconnected_hosts {
        let id = format!("host/{}", id_segment(host));
        let body = report(
            "error",
            Some(&id),
            Some(host),
            Some("Disconnected, reconnecting"),
            None,
        );
        reports.insert(id, body);
    }

    for (key, container) in &state.containers {
        if container.state == ContainerState::Running
            && container.health == Some(HealthStatus::Unhealthy)
        {
            let id = format!(
                "container/{}/{}",
                id_segment(&key.host_id),
                id_segment(&key.container_id)
            );
            let title = if state.connected_hosts.len() > 1 {
                format!("{} ({})", container.name, key.host_id)
            } else {
                container.name.clone()
            };
            let body = report("error", Some(&id), Some(&title), Some("Unhealthy"), None);
            reports.insert(id, body);
        }
    }

    reports
}

fn root_report(state: &AppState) -> String {
    let batches = &state.bulk_progress;
    if !batches.is_empty() {
        let total: usize = batches.iter().map(|b| b.total).sum();
        let finished: usize = batches.iter().map(|b| b.total - b.pending.len()).sum();
        let progress = (finished * 100 / total.max(1)) as u8;
        let count = if total == 1 {
            "1 container".to_string()
        } else {
            format!("{total} containers")
        };
        let msg = match batches.as_slice() {
            [batch] => format!("{} {count}", batch.action.progress_words().0),
            _ => format!("Running actions on {count}"),
        };
        return report("working", None, None, Some(&msg), Some(progress));
    }
    match &state.last_bulk_outcome {
        Some((true, summary)) => report("error", None, None, Some(summary), None),
        Some((false, summary)) => report("done", None, None, Some(summary), None),
        None => report("idle", None, None, None, None),
    }
}

/// One report's body. Every report repeats `app`, since a report replaces its
/// record whole; child records inherit it from the root, but stating it is harmless.
fn report(
    state: &str,
    id: Option<&str>,
    title: Option<&str>,
    msg: Option<&str>,
    progress: Option<u8>,
) -> String {
    let mut body = format!("state={state}:app={APP}");
    if let Some(id) = id {
        body.push_str(":id=");
        body.push_str(id);
    }
    if let Some(progress) = progress {
        body.push_str(&format!(":progress={}", progress.min(100)));
    }
    if let Some(title) = title {
        body.push_str(":title=");
        body.push_str(&base64(&clean_text(title, MAX_TITLE_CHARS)));
    }
    if let Some(msg) = msg {
        body.push_str(":msg=");
        body.push_str(&base64(&clean_text(msg, MAX_MSG_CHARS)));
    }
    body
}

/// Single-line text without control characters (the terminal refuses the whole
/// report otherwise), shortened to `max` characters
fn clean_text(text: &str, max: usize) -> String {
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if let Some((cut, _)) = out.char_indices().nth(max) {
        out.truncate(cut);
        out.push('…');
    }
    out
}

/// An id segment: `[A-Za-z0-9_.+-]{1,32}`. Other characters (e.g. the `@` in
/// `user@host`) become `_`; a report with a bad id would be ignored.
fn id_segment(text: &str) -> String {
    let segment: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(32)
        .collect();
    if segment.is_empty() {
        "_".to_string()
    } else {
        segment
    }
}

/// Standard base64 with padding
fn base64(text: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app_state::BulkProgress;
    use crate::core::types::{
        AppEvent, Column, ColumnConfig, Container, ContainerAction, ContainerKey,
    };
    use std::collections::{HashMap, HashSet};
    use tokio::sync::mpsc;

    fn state() -> AppState {
        let (tx, _rx) = mpsc::channel(10);
        AppState::new(
            HashMap::new(),
            tx,
            false,
            Column::Uptime,
            None,
            ColumnConfig::default(),
            None,
        )
    }

    fn container(id: &str, host: &str) -> Container {
        Container {
            id: id.to_string(),
            name: format!("{id}-name"),
            state: ContainerState::Running,
            health: None,
            created: None,
            stats: Default::default(),
            host_id: host.to_string(),
            dozzle_url: None,
            restart_count: None,
            compose_project: None,
            image: None,
            ports: None,
        }
    }

    fn osc(body: &str) -> String {
        format!("\x1b]7501;{body}\x1b\\")
    }

    #[test]
    fn base64_matches_the_spec_examples() {
        assert_eq!(base64("Installing updates"), "SW5zdGFsbGluZyB1cGRhdGVz");
        assert_eq!(
            base64("Upgraded 12 packages"),
            "VXBncmFkZWQgMTIgcGFja2FnZXM="
        );
        assert_eq!(base64("Deploying v2.4.1"), "RGVwbG95aW5nIHYyLjQuMQ==");
        assert_eq!(base64(""), "");
    }

    #[test]
    fn starts_idle_and_only_writes_changes() {
        let state = state();
        let mut status = ProgramStatus::default();
        assert_eq!(status.update(&state), osc("state=idle:app=dtop"));
        assert_eq!(status.update(&state), "");
    }

    #[test]
    fn unhealthy_container_is_an_error_until_it_recovers() {
        let mut state = state();
        let mut status = ProgramStatus::default();
        state.handle_event(AppEvent::InitialContainerList(
            "user@box".to_string(),
            vec![container("abc123", "user@box")],
        ));
        status.update(&state);

        let key = ContainerKey::new("user@box".to_string(), "abc123".to_string());
        state.handle_event(AppEvent::ContainerHealthChanged(
            key.clone(),
            HealthStatus::Unhealthy,
        ));
        assert_eq!(
            status.update(&state),
            osc(&format!(
                "state=error:app=dtop:id=container/user_box/abc123:title={}:msg={}",
                base64("abc123-name"),
                base64("Unhealthy")
            ))
        );

        state.handle_event(AppEvent::ContainerHealthChanged(key, HealthStatus::Healthy));
        assert_eq!(
            status.update(&state),
            osc("state=clear:id=container/user_box/abc123")
        );
    }

    #[test]
    fn unhealthy_but_stopped_container_is_not_reported() {
        let mut state = state();
        let mut stopped = container("abc123", "local");
        stopped.state = ContainerState::Exited;
        stopped.health = Some(HealthStatus::Unhealthy);
        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![stopped],
        ));
        assert_eq!(
            ProgramStatus::default().update(&state),
            osc("state=idle:app=dtop")
        );
    }

    #[test]
    fn disconnected_host_is_an_error_until_it_reconnects() {
        let mut state = state();
        let mut status = ProgramStatus::default();
        status.update(&state);

        state.handle_event(AppEvent::HostDisconnected("user@box".to_string()));
        assert_eq!(
            status.update(&state),
            osc(&format!(
                "state=error:app=dtop:id=host/user_box:title={}:msg={}",
                base64("user@box"),
                base64("Disconnected, reconnecting")
            ))
        );

        state.handle_event(AppEvent::HostReconnected("user@box".to_string()));
        assert_eq!(status.update(&state), osc("state=clear:id=host/user_box"));
    }

    #[test]
    fn bulk_action_reports_progress_then_its_summary() {
        let mut state = state();
        let mut status = ProgramStatus::default();
        status.update(&state);

        let keys: Vec<_> = ["a", "b", "c", "d"]
            .iter()
            .map(|id| ContainerKey::new("local".to_string(), id.to_string()))
            .collect();
        state.bulk_progress.push(BulkProgress {
            action: ContainerAction::Stop,
            pending: keys[1..].iter().cloned().collect::<HashSet<_>>(),
            total: 4,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        });
        assert_eq!(
            status.update(&state),
            osc(&format!(
                "state=working:app=dtop:progress=25:msg={}",
                base64("Stopping 4 containers")
            ))
        );

        for key in &keys[1..] {
            state.handle_event(AppEvent::ActionSuccess(key.clone(), ContainerAction::Stop));
        }
        assert_eq!(
            status.update(&state),
            osc(&format!(
                "state=done:app=dtop:msg={}",
                base64("Stopped 4 containers")
            ))
        );
    }

    #[test]
    fn failed_bulk_action_is_an_error() {
        let mut state = state();
        let mut status = ProgramStatus::default();
        let key = ContainerKey::new("local".to_string(), "a".to_string());
        state.bulk_progress.push(BulkProgress {
            action: ContainerAction::Remove,
            pending: HashSet::from([key.clone()]),
            total: 2,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        });
        status.update(&state);

        state.handle_event(AppEvent::ActionError(
            key,
            ContainerAction::Remove,
            "conflict".to_string(),
        ));
        assert_eq!(
            status.update(&state),
            osc(&format!(
                "state=error:app=dtop:msg={}",
                base64("Removed 1 of 2 containers (1 failed: conflict)")
            ))
        );
    }

    #[test]
    fn text_is_single_line_and_bounded() {
        assert_eq!(clean_text("a\nb\t\x1bc", 50), "a b c");
        assert_eq!(clean_text("abcdef", 3), "abc…");
        assert_eq!(id_segment("user@host:22"), "user_host_22");
        assert_eq!(id_segment(""), "_");
        assert_eq!(id_segment(&"x".repeat(40)).len(), 32);
    }

    #[test]
    fn clear_all_removes_every_record() {
        assert_eq!(ProgramStatus::clear_all(), osc("state=clear"));
    }
}
