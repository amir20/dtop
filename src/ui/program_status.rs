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
//! - `logs/<host>/<id>`: `done` once lines matching the log view's search have
//!   streamed in, with their count. It lasts as long as the search does.
//!
//! Only changes are written, and everything is cleared on quit, since dtop is
//! then no longer watching and `error` records outlive the process.
//!
//! A bulk action's progress is also sent as OSC 9;4, the progress bar that
//! Windows Terminal, ConEmu, Ghostty and WezTerm show, but only to those: other
//! terminals (older iTerm2, kitty) read any OSC 9 as a desktop notification, and
//! would pop one up for every progress step. The same bar pulses (no
//! percentage) while the log view loads older lines.

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::core::app_state::AppState;
use crate::core::types::{ContainerState, HealthStatus};

const APP: &str = "dtop";
/// Decoded limits in bytes: 2048 (`msg`) and 192 (`title`). A report over
/// either is dropped whole, so text is cut to fit
const MAX_MSG_BYTES: usize = 600;
const MAX_TITLE_BYTES: usize = 120;

/// One record's report body, keyed by its id ("" for the root record)
type Reports = BTreeMap<String, String>;

/// Remembers what the terminal was last told, so only changes are written
#[derive(Default)]
pub struct ProgramStatus {
    sent: Reports,
    /// Whether to also send OSC 9;4 progress
    progress_bar: bool,
    /// The OSC 9;4 bar last sent (None: no bar shown)
    sent_progress: Option<ProgressBar>,
    /// [`fingerprint`] of the state last reported, to skip rebuilding the
    /// reports on the (usual) passes where nothing they show changed
    sent_fingerprint: Option<u64>,
}

impl ProgramStatus {
    /// A reporter for the terminal dtop runs in
    pub fn for_terminal() -> Self {
        Self {
            progress_bar: supports_progress_bar(|key| std::env::var(key).ok()),
            ..Self::default()
        }
    }

    /// The escape sequences that bring the terminal up to date with `state`
    /// (empty when nothing changed)
    pub fn update(&mut self, state: &AppState) -> String {
        let fingerprint = fingerprint(state);
        if self.sent_fingerprint == Some(fingerprint) {
            return String::new();
        }
        self.sent_fingerprint = Some(fingerprint);

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

        if self.progress_bar {
            // A bulk action's percentage wins over the log view's pulse
            let progress = bulk_progress(state)
                .map(|(_, percent)| ProgressBar::Percent(percent))
                .or_else(|| loading_older_logs(state).then_some(ProgressBar::Busy));
            if progress != self.sent_progress {
                match progress {
                    Some(ProgressBar::Percent(percent)) => {
                        out.push_str(&format!("\x1b]9;4;1;{percent}\x07"))
                    }
                    Some(ProgressBar::Busy) => out.push_str("\x1b]9;4;3\x07"),
                    None => out.push_str(CLEAR_PROGRESS_BAR),
                }
                self.sent_progress = progress;
            }
        }
        out
    }

    /// Removes every record dtop made, and its progress bar
    pub fn clear_all(&self) -> String {
        let mut out = String::new();
        push_osc(&mut out, "state=clear");
        if self.progress_bar {
            out.push_str(CLEAR_PROGRESS_BAR);
        }
        out
    }
}

const CLEAR_PROGRESS_BAR: &str = "\x1b]9;4;0\x07";

/// What the OSC 9;4 progress bar shows
#[derive(Clone, Copy, PartialEq, Eq)]
enum ProgressBar {
    Percent(u8),
    /// Working, with no known end (state 3, "indeterminate")
    Busy,
}

/// Whether the log view is waiting on a page of older lines
fn loading_older_logs(state: &AppState) -> bool {
    state
        .log_state
        .as_ref()
        .is_some_and(|log| log.fetching_older)
}

/// The log view's applied search and how many matching lines have arrived
/// since, once there are any
fn new_log_matches(state: &AppState) -> Option<(&crate::core::types::LogState, &str, usize)> {
    let log = state.log_state.as_ref()?;
    let search = log.search.as_ref()?;
    (search.new_matches > 0).then(|| (log, search.input.value(), search.new_matches))
}

/// Terminals known to draw OSC 9;4 as a progress bar (the same set cargo uses).
/// Off under tmux, which doesn't pass it through.
fn supports_progress_bar(env: impl Fn(&str) -> Option<String>) -> bool {
    if env("TMUX").is_some() {
        return false;
    }
    env("WT_SESSION").is_some()
        || env("ConEmuANSI").as_deref() == Some("ON")
        || env("TERM_PROGRAM")
            .is_some_and(|p| p.eq_ignore_ascii_case("ghostty") || p.eq_ignore_ascii_case("WezTerm"))
}

/// Container count and percent done across the running bulk actions
fn bulk_progress(state: &AppState) -> Option<(usize, u8)> {
    let batches = &state.bulk_progress;
    if batches.is_empty() {
        return None;
    }
    let total: usize = batches.iter().map(|b| b.total).sum();
    let finished: usize = batches.iter().map(|b| b.total - b.pending.len()).sum();
    Some((total, (finished * 100 / total.max(1)) as u8))
}

/// A hash of everything [`reports`] and [`bulk_progress`] read, cheap enough to
/// compute every pass (no allocation). Unhealthy containers are combined
/// order-independently, since `containers` is a `HashMap`.
fn fingerprint(state: &AppState) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.bulk_progress.len().hash(&mut hasher);
    for batch in &state.bulk_progress {
        std::mem::discriminant(&batch.action).hash(&mut hasher);
        batch.total.hash(&mut hasher);
        batch.pending.len().hash(&mut hasher);
    }
    state.last_bulk_outcome.hash(&mut hasher);
    state.disconnected_hosts.hash(&mut hasher);
    loading_older_logs(state).hash(&mut hasher);
    if let Some((log, query, count)) = new_log_matches(state) {
        log.container_key.hash(&mut hasher);
        query.hash(&mut hasher);
        count.hash(&mut hasher);
    }
    (state.connected_hosts.len() > 1).hash(&mut hasher);
    let mut unhealthy = 0u64;
    for (key, container) in &state.containers {
        if container.state == ContainerState::Running
            && container.health == Some(HealthStatus::Unhealthy)
        {
            let mut item = DefaultHasher::new();
            key.hash(&mut item);
            container.name.hash(&mut item);
            unhealthy = unhealthy.wrapping_add(item.finish());
        }
    }
    unhealthy.hash(&mut hasher);
    hasher.finish()
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

    if let Some((log, query, count)) = new_log_matches(state) {
        let key = &log.container_key;
        let id = format!(
            "logs/{}/{}",
            id_segment(&key.host_id),
            id_segment(&key.container_id)
        );
        let name = state.containers.get(key).map(|c| c.name.as_str());
        let title = format!("Logs: {}", name.unwrap_or(&key.container_id));
        let lines = if count == 1 { "line" } else { "lines" };
        let msg = format!("{count} new {lines} matching \"{query}\"");
        reports.insert(
            id.clone(),
            report("done", Some(&id), Some(&title), Some(&msg), None),
        );
    }

    reports
}

fn root_report(state: &AppState) -> String {
    if let Some((total, progress)) = bulk_progress(state) {
        let batches = &state.bulk_progress;
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
        body.push_str(&base64(clean_text(title, MAX_TITLE_BYTES)));
    }
    if let Some(msg) = msg {
        body.push_str(":msg=");
        body.push_str(&base64(clean_text(msg, MAX_MSG_BYTES)));
    }
    body
}

/// Single-line text without control characters (the terminal refuses the whole
/// report otherwise), cut on a character boundary to at most `max` bytes
fn clean_text(text: &str, max: usize) -> String {
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if out.len() > max {
        let budget = max - '…'.len_utf8();
        let cut = out
            .char_indices()
            .map(|(i, c)| i + c.len_utf8())
            .take_while(|&end| end <= budget)
            .last()
            .unwrap_or(0);
        out.truncate(cut);
        out.push('…');
    }
    out
}

/// An id segment: `[A-Za-z0-9_.+-]{1,32}`. Other characters (e.g. the `@` in
/// `user@host`) become `_`, since a report with a bad id would be ignored. When
/// that changes the text, a hash of the original is appended so that, e.g.,
/// `user@box` and `user_box` don't share (and clear) each other's record.
fn id_segment(text: &str) -> String {
    let valid = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '-');
    if !text.is_empty() && text.len() <= 32 && text.chars().all(valid) {
        return text.to_string();
    }
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let hash = format!("{:08x}", hasher.finish() as u32);
    let mut segment: String = text
        .chars()
        .map(|c| if valid(c) { c } else { '_' })
        .take(32 - 1 - hash.len())
        .collect();
    segment.push('.');
    segment.push_str(&hash);
    segment
}

fn base64(data: impl AsRef<[u8]>) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
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
            started: None,
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
                "state=error:app=dtop:id=container/{}/abc123:title={}:msg={}",
                id_segment("user@box"),
                base64("abc123-name"),
                base64("Unhealthy")
            ))
        );

        state.handle_event(AppEvent::ContainerHealthChanged(key, HealthStatus::Healthy));
        assert_eq!(
            status.update(&state),
            osc(&format!(
                "state=clear:id=container/{}/abc123",
                id_segment("user@box")
            ))
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
                "state=error:app=dtop:id=host/{}:title={}:msg={}",
                id_segment("user@box"),
                base64("user@box"),
                base64("Disconnected, reconnecting")
            ))
        );

        state.handle_event(AppEvent::HostReconnected("user@box".to_string()));
        assert_eq!(
            status.update(&state),
            osc(&format!("state=clear:id=host/{}", id_segment("user@box")))
        );
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
    fn bulk_action_whose_results_are_lost_stops_working() {
        let mut state = state();
        let mut status = ProgramStatus::default();
        state.bulk_progress.push(BulkProgress {
            action: ContainerAction::Stop,
            pending: HashSet::from([ContainerKey::new("local".to_string(), "a".to_string())]),
            total: 1,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now() - std::time::Duration::from_secs(120),
        });
        assert!(status.update(&state).contains("state=working"));

        // No event arrives; the event loop's periodic cleanup drops the batch
        crate::ui::render::cleanup_expired_errors(&mut state);
        assert_eq!(status.update(&state), osc("state=idle:app=dtop"));
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
    fn failed_single_container_bulk_action_names_it() {
        let mut state = state();
        let mut status = ProgramStatus::default();
        let key = ContainerKey::new("local".to_string(), "a".to_string());
        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![container("a", "local")],
        ));
        state.bulk_progress.push(BulkProgress {
            action: ContainerAction::Stop,
            pending: HashSet::from([key.clone()]),
            total: 1,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        });
        status.update(&state);

        state.handle_event(AppEvent::ActionError(
            key,
            ContainerAction::Stop,
            "timeout".to_string(),
        ));
        assert_eq!(
            status.update(&state),
            osc(&format!(
                "state=error:app=dtop:msg={}",
                base64("a-name: timeout")
            ))
        );
    }

    #[test]
    fn unhealthy_container_that_stops_is_cleared() {
        let mut state = state();
        let mut status = ProgramStatus::default();
        let mut sick = container("abc123", "local");
        sick.health = Some(HealthStatus::Unhealthy);
        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![sick],
        ));
        status.update(&state);

        let key = ContainerKey::new("local".to_string(), "abc123".to_string());
        state.handle_event(AppEvent::ContainerStateChanged(key, ContainerState::Exited));
        assert_eq!(
            status.update(&state),
            osc("state=clear:id=container/local/abc123")
        );
    }

    #[test]
    fn overlapping_bulk_actions_share_one_working_record() {
        let mut state = state();
        for (action, id) in [(ContainerAction::Stop, "a"), (ContainerAction::Remove, "b")] {
            state.bulk_progress.push(BulkProgress {
                action,
                pending: HashSet::from([ContainerKey::new("local".to_string(), id.to_string())]),
                total: 1,
                failed: 0,
                last_error: None,
                last_activity: std::time::Instant::now(),
            });
        }
        assert_eq!(
            ProgramStatus::default().update(&state),
            osc(&format!(
                "state=working:app=dtop:progress=0:msg={}",
                base64("Running actions on 2 containers")
            ))
        );
    }

    #[test]
    fn text_is_single_line_and_bounded() {
        assert_eq!(clean_text("a\nb\t\x1bc", 50), "a b c");
        assert_eq!(clean_text("abcdef", 6), "abcdef");
        assert_eq!(clean_text("abcdefg", 6), "abc…");
        // Bytes, not characters: 4-byte characters are never split
        let emoji = clean_text(&"🐳".repeat(60), MAX_TITLE_BYTES);
        assert!(emoji.len() <= MAX_TITLE_BYTES);
        assert!(emoji.ends_with("🐳…"));
        assert_eq!(id_segment("abc123"), "abc123");
        assert!(id_segment("user@host:22").starts_with("user_host_22."));
        assert_ne!(id_segment("user@box"), id_segment("user_box"));
        assert_ne!(id_segment(""), "");
        let long = id_segment(&"x".repeat(40));
        assert_eq!(long.len(), 32);
        assert_ne!(long, id_segment(&"x".repeat(41)));
    }

    #[test]
    fn progress_bar_follows_the_bulk_action() {
        let mut state = state();
        let mut status = ProgramStatus {
            progress_bar: true,
            ..ProgramStatus::default()
        };
        status.update(&state);

        let keys: Vec<_> = ["a", "b"]
            .iter()
            .map(|id| ContainerKey::new("local".to_string(), id.to_string()))
            .collect();
        state.bulk_progress.push(BulkProgress {
            action: ContainerAction::Stop,
            pending: keys.iter().cloned().collect(),
            total: 2,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        });
        assert!(status.update(&state).ends_with("\x1b]9;4;1;0\x07"));

        state.handle_event(AppEvent::ActionSuccess(
            keys[0].clone(),
            ContainerAction::Stop,
        ));
        assert!(status.update(&state).ends_with("\x1b]9;4;1;50\x07"));

        state.handle_event(AppEvent::ActionSuccess(
            keys[1].clone(),
            ContainerAction::Stop,
        ));
        assert!(status.update(&state).ends_with(CLEAR_PROGRESS_BAR));
        assert_eq!(status.update(&state), "");
    }

    #[test]
    fn progress_bar_is_off_by_default() {
        let mut state = state();
        let mut status = ProgramStatus::default();
        state.bulk_progress.push(BulkProgress {
            action: ContainerAction::Stop,
            pending: HashSet::from([ContainerKey::new("local".to_string(), "a".to_string())]),
            total: 1,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        });
        assert!(!status.update(&state).contains("]9;4"));
    }

    #[test]
    fn progress_bar_only_on_terminals_that_draw_it() {
        let env = |vars: &'static [(&'static str, &'static str)]| {
            move |key: &str| {
                vars.iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert!(supports_progress_bar(env(&[("WT_SESSION", "x")])));
        assert!(supports_progress_bar(env(&[("ConEmuANSI", "ON")])));
        assert!(supports_progress_bar(env(&[("TERM_PROGRAM", "ghostty")])));
        assert!(supports_progress_bar(env(&[("TERM_PROGRAM", "WezTerm")])));
        // OSC 9 is a notification in these
        assert!(!supports_progress_bar(env(&[(
            "TERM_PROGRAM",
            "iTerm.app"
        )])));
        assert!(!supports_progress_bar(env(&[("TERM", "xterm-kitty")])));
        // Rex gets OSC 7501, which carries the progress already
        assert!(!supports_progress_bar(env(&[("TERM_PROGRAM", "rex")])));
        assert!(!supports_progress_bar(env(&[
            ("TERM_PROGRAM", "ghostty"),
            ("TMUX", "/tmp/tmux")
        ])));
    }

    #[test]
    fn clear_all_removes_every_record() {
        assert_eq!(ProgramStatus::default().clear_all(), osc("state=clear"));
        let with_bar = ProgramStatus {
            progress_bar: true,
            ..ProgramStatus::default()
        };
        assert_eq!(
            with_bar.clear_all(),
            format!("{}{CLEAR_PROGRESS_BAR}", osc("state=clear"))
        );
    }

    fn log_line(msg: &str) -> crate::docker::logs::LogEntry {
        crate::docker::logs::LogEntry::parse(&format!("2025-10-29T10:15:30Z {msg}")).unwrap()
    }

    /// A state with nginx's log view open
    fn viewing_logs() -> (AppState, ContainerKey) {
        use crate::core::types::{LogState, ViewState};
        let mut state = state();
        let nginx = container("abc123", "local");
        let key = ContainerKey::new("local".to_string(), "abc123".to_string());
        state.containers.insert(key.clone(), nginx);
        state.view_state = ViewState::LogView(key.clone());
        state.log_state = Some(LogState::new(key.clone(), None));
        (state, key)
    }

    /// Lines that match the applied search as they arrive flag the tab, counted;
    /// lines already loaded and lines that don't match do not
    #[test]
    fn new_search_matches_are_reported_until_the_search_is_dropped() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let (mut state, key) = viewing_logs();
        let press = |state: &mut AppState, code| {
            state.handle_event(AppEvent::KeyInput(KeyEvent::new(code, KeyModifiers::NONE)));
        };
        let mut status = ProgramStatus::default();

        state.handle_event(AppEvent::LogLine(key.clone(), log_line("ERROR old")));
        for code in "/error".chars().map(KeyCode::Char).chain([KeyCode::Enter]) {
            press(&mut state, code);
        }
        state.handle_event(AppEvent::LogLine(key.clone(), log_line("all good")));
        assert!(!status.update(&state).contains("logs/"));

        state.handle_event(AppEvent::LogLine(key.clone(), log_line("ERROR new")));
        let id = "logs/local/abc123";
        let title = base64(format!("Logs: {}", state.containers[&key].name));
        assert_eq!(
            status.update(&state),
            osc(&format!(
                "state=done:app=dtop:id={id}:title={title}:msg={}",
                base64("1 new line matching \"error\"")
            ))
        );
        state.handle_event(AppEvent::LogLine(key.clone(), log_line("error again")));
        assert!(
            status
                .update(&state)
                .contains(&base64("2 new lines matching \"error\""))
        );

        press(&mut state, KeyCode::Esc);
        assert_eq!(status.update(&state), osc(&format!("state=clear:id={id}")));
    }

    #[test]
    fn progress_bar_pulses_while_older_logs_load() {
        let (mut state, _) = viewing_logs();
        let mut status = ProgramStatus {
            progress_bar: true,
            ..ProgramStatus::default()
        };
        status.update(&state);

        state.log_state.as_mut().unwrap().fetching_older = true;
        assert!(status.update(&state).ends_with("\x1b]9;4;3\x07"));
        state.log_state.as_mut().unwrap().fetching_older = false;
        assert!(status.update(&state).ends_with(CLEAR_PROGRESS_BAR));

        // Not sent to terminals that would read it as a notification
        let mut plain = ProgramStatus::default();
        state.log_state.as_mut().unwrap().fetching_older = true;
        assert!(!plain.update(&state).contains("]9;4"));
    }
}
