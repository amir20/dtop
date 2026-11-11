use crate::core::app_state::AppState;
use crate::core::log_search::LogSearch;
use crate::core::types::{ContainerKey, LogState, RenderAction, ViewState};
use crate::docker::logs::{LogEntry, fetch_older_logs};
use crate::ui::panes::PaneKind;

impl AppState {
    pub(super) fn handle_enter_pressed(&mut self) -> RenderAction {
        // Handle Enter based on current view state
        match self.view_state {
            ViewState::SearchMode => {
                // Apply filter and return to ContainerList view
                self.view_state = ViewState::ContainerList;
                RenderAction::Render // Force redraw to show filter bar
            }
            ViewState::ContainerList => {
                // Show action menu for selected container
                self.handle_show_action_menu()
            }
            ViewState::ActionMenu(_) | ViewState::BulkActionMenu => {
                // Execute selected action
                self.handle_execute_action()
            }
            _ => {
                // Ignore Enter in other views
                RenderAction::None
            }
        }
    }

    pub(super) fn handle_show_log_view(&mut self) -> RenderAction {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // Get the selected container
        let Some(selected_idx) = self.table_state.selected() else {
            return RenderAction::None;
        };

        let Some(container_key) = self.sorted_container_keys.get(selected_idx).cloned() else {
            return RenderAction::None;
        };

        if let Some(action) = self.open_in_pane(PaneKind::Logs, &container_key) {
            return action;
        }

        self.open_log_view(&container_key)
    }

    /// `dtop logs <id>`: opens the log view once the container is listed
    pub(super) fn open_pending_logs(&mut self) -> Option<RenderAction> {
        let wanted = self.logs_only.as_deref()?;
        if self.quit_with_log_view {
            return None;
        }
        let key = self
            .containers
            .keys()
            .find(|key| {
                key.container_id.starts_with(wanted) || wanted.starts_with(&key.container_id)
            })?
            .clone();
        self.quit_with_log_view = true;
        Some(self.open_log_view(&key))
    }

    fn open_log_view(&mut self, container_key: &ContainerKey) -> RenderAction {
        // Get container creation time for progress calculation
        let container_created_at = self.containers.get(container_key).and_then(|c| c.created);

        // Create new log state for this container
        let mut new_log_state = LogState::new(container_key.clone(), container_created_at);
        new_log_state.set_show_timestamps(self.show_log_timestamps);

        // Start streaming logs for this container
        if let Some(host) = self.connected_hosts.get(&container_key.host_id) {
            let host_clone = host.clone();
            let container_id = container_key.container_id.clone();
            let tx_clone = self.event_tx.clone();

            let handle = tokio::spawn(async move {
                use crate::docker::logs::stream_container_logs;
                stream_container_logs(host_clone, container_id, tx_clone).await;
            });

            new_log_state.stream_handle = Some(handle);
        }

        // Set the log state
        self.log_state = Some(new_log_state);

        // Reset scroll state - start at bottom
        self.is_at_bottom = true;

        // Switch to log view
        self.view_state = ViewState::LogView(container_key.clone());

        RenderAction::Render // Force draw - view changed
    }

    pub(super) fn handle_exit_log_view(&mut self) -> RenderAction {
        // Only handle in LogView
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        // Stop log streaming and cleanup log state
        if let Some(mut state) = self.log_state.take()
            && let Some(handle) = state.stream_handle.take()
        {
            handle.abort();
        }

        // `dtop logs <id>` exists only for this log view
        if self.quit_with_log_view {
            self.should_quit = true;
        }

        // Switch back to container list view
        self.view_state = ViewState::ContainerList;

        RenderAction::Render // Force draw - view changed
    }

    /// Handles 't' in the log view: show or hide dtop's timestamp on each line
    pub(super) fn handle_toggle_log_timestamps(&mut self) -> RenderAction {
        self.show_log_timestamps = !self.show_log_timestamps;
        if let Some(state) = &mut self.log_state {
            state.set_show_timestamps(self.show_log_timestamps);
        }
        RenderAction::Render
    }

    /// Whether a log search query is being typed
    pub(super) fn log_search_editing(&self) -> bool {
        matches!(self.view_state, ViewState::LogView(_))
            && !self.show_help
            && self
                .log_state
                .as_ref()
                .and_then(|state| state.search.as_ref())
                .is_some_and(|search| search.editing)
    }

    /// Handles '/' in the log view: start typing a search
    pub(super) fn handle_start_log_search(&mut self) -> RenderAction {
        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };
        state.search = Some(LogSearch {
            editing: true,
            ..Default::default()
        });
        RenderAction::Render
    }

    /// A key while the query is typed: Enter applies it and jumps to the
    /// newest match, Esc drops the search, anything else edits the text
    pub(super) fn handle_log_search_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> RenderAction {
        use crossterm::event::KeyCode;
        use tui_input::backend::crossterm::EventHandler;

        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };
        let Some(search) = &mut state.search else {
            return RenderAction::None;
        };
        match key.code {
            KeyCode::Esc => state.search = None,
            KeyCode::Enter if search.input.value().is_empty() => state.search = None,
            KeyCode::Enter => {
                search.editing = false;
                return self.handle_log_search_jump(false);
            }
            _ => {
                search
                    .input
                    .handle_event(&crossterm::event::Event::Key(key));
            }
        }
        RenderAction::Render
    }

    /// Handles 'n' (`forward`, towards newer lines) and 'N' in the log view:
    /// scroll to the next line that holds the search text
    pub(super) fn handle_log_search_jump(&mut self, forward: bool) -> RenderAction {
        // Rows of context kept above the line jumped to
        const CONTEXT_ROWS: usize = 2;

        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }
        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };
        let Some(search) = &state.search else {
            return RenderAction::None;
        };
        let first_jump = search.current.is_none();

        let Some(index) = state.find_match(forward) else {
            let message = if first_jump {
                format!("No matches for \"{}\"", search.input.value())
            } else if forward {
                "No newer matches".to_string()
            } else {
                "No older matches".to_string()
            };
            self.show_notification(&message);
            return RenderAction::Render;
        };
        if let Some(search) = &mut state.search {
            search.current = Some(index);
        }

        // Leave the view where it is when the line is already on screen
        // While following the bottom, `scroll_offset` is only brought up to
        // date by the next draw, and lines may have arrived since the last one
        let total = state.total_rows(self.last_viewport_width);
        let top = if self.is_at_bottom {
            total.saturating_sub(self.last_viewport_height)
        } else {
            state.scroll_offset
        };
        let row = state.rows_before(index);
        let on_screen = top..top + self.last_viewport_height;
        if !on_screen.contains(&row) {
            state.scroll_offset = row.saturating_sub(CONTEXT_ROWS);
            self.is_at_bottom = false;
        }
        RenderAction::Render
    }

    pub(super) fn handle_scroll_up(&mut self) -> RenderAction {
        // Only handle scroll in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };

        // Scroll up (decrease offset)
        if state.scroll_offset > 0 {
            state.scroll_offset = state.scroll_offset.saturating_sub(1);
            self.is_at_bottom = false; // User scrolled away from bottom

            // Check if we're near the top (within threshold) - trigger pagination
            const SCROLL_THRESHOLD: usize = 10; // Lines from top to trigger pagination
            if state.scroll_offset <= SCROLL_THRESHOLD {
                // Trigger pagination request
                self.handle_request_older_logs();
            }

            return RenderAction::Render; // Force draw
        }

        RenderAction::None
    }

    pub(super) fn handle_scroll_down(&mut self) -> RenderAction {
        // Only handle scroll in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };

        // Increment scroll offset
        state.scroll_offset = state.scroll_offset.saturating_add(1);

        // Will be clamped in UI and is_at_bottom will be recalculated there
        RenderAction::Render // Force draw
    }

    pub(super) fn handle_scroll_to_top(&mut self) -> RenderAction {
        // Only handle in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };

        // Scroll to top
        state.scroll_offset = 0;
        self.is_at_bottom = false;

        // Trigger pagination since we're at the top
        self.handle_request_older_logs();

        RenderAction::Render
    }

    pub(super) fn handle_scroll_to_bottom(&mut self) -> RenderAction {
        // Only handle in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        // Set is_at_bottom - the actual offset will be calculated in render
        self.is_at_bottom = true;
        RenderAction::Render
    }

    pub(super) fn handle_scroll_page_up(&mut self) -> RenderAction {
        // Only handle in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };

        // Scroll up by half page (similar to vim's Ctrl+U)
        // We'll use the last known viewport height stored in AppState
        let page_size = self.last_viewport_height / 2;
        state.scroll_offset = state.scroll_offset.saturating_sub(page_size);
        self.is_at_bottom = false;

        // Check if we're near the top - trigger pagination
        const SCROLL_THRESHOLD: usize = 10;
        if state.scroll_offset <= SCROLL_THRESHOLD {
            self.handle_request_older_logs();
        }

        RenderAction::Render
    }

    pub(super) fn handle_scroll_page_down(&mut self) -> RenderAction {
        // Only handle in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };

        // Scroll down by half page (similar to vim's Ctrl+D)
        // We'll use the last known viewport height stored in AppState
        let page_size = self.last_viewport_height / 2;
        state.scroll_offset = state.scroll_offset.saturating_add(page_size);
        // Will be clamped in UI and is_at_bottom will be recalculated there
        RenderAction::Render
    }

    pub(super) fn handle_log_line(
        &mut self,
        key: ContainerKey,
        log_entry: LogEntry,
    ) -> RenderAction {
        // Only add log line if we're currently viewing this container's logs
        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };

        if state.container_key != key {
            return RenderAction::None;
        }

        // Update newest timestamp for progress calculation
        state.newest_timestamp = Some(log_entry.timestamp);
        state.push_entry(log_entry);

        // Picked up by the regular 500ms draw. Forcing a draw per line made a
        // chatty container redraw as fast as lines arrived.
        RenderAction::None
    }

    pub(super) fn handle_log_batch_prepend(
        &mut self,
        key: ContainerKey,
        log_entries: Vec<LogEntry>,
        has_more_history: bool,
    ) -> RenderAction {
        // Only process if viewing this container
        let Some(state) = &mut self.log_state else {
            return RenderAction::None;
        };

        if state.container_key != key {
            return RenderAction::None;
        }

        // Check if this is the initial load
        let is_initial_load = state.total_loaded == 0;

        tracing::debug!(
            "Received log batch: {} entries, has_more_history: {}, is_initial_load: {}, total_loaded: {}",
            log_entries.len(),
            has_more_history,
            is_initial_load,
            state.total_loaded
        );

        // Extract timestamps before moving log_entries
        let oldest = log_entries.first().map(|e| e.timestamp);
        let newest = log_entries.last().map(|e| e.timestamp);
        let num_entries = log_entries.len();

        let visual_lines_prepended = state.prepend_entries(log_entries, self.last_viewport_width);

        state.oldest_timestamp = oldest;
        state.has_more_history = has_more_history;
        state.total_loaded += num_entries;
        state.fetching_older = false;

        // Update newest timestamp if this is the first batch (initial load)
        if is_initial_load {
            state.newest_timestamp = newest;
        }

        // Adjust scroll offset to maintain visual position during pagination.
        // scroll_offset is in visual lines, so compute how many visual lines
        // the prepended entries occupy.
        if !is_initial_load {
            state.scroll_offset += visual_lines_prepended;
        }

        RenderAction::Render
    }

    pub(super) fn handle_request_older_logs(&mut self) -> RenderAction {
        let Some(state) = &mut self.log_state else {
            tracing::debug!("No log state, skipping pagination request");
            return RenderAction::None;
        };

        // Check if we're already fetching or no more history
        if state.fetching_older {
            tracing::debug!("Already fetching older logs, skipping");
            return RenderAction::None;
        }

        if !state.has_more_history {
            tracing::debug!("No more history available, skipping pagination");
            return RenderAction::None;
        }

        let Some(oldest_ts) = state.oldest_timestamp else {
            tracing::debug!("No oldest timestamp, skipping pagination");
            return RenderAction::None;
        };

        let Some(newest_ts) = state.newest_timestamp else {
            tracing::debug!("No newest timestamp, skipping pagination");
            return RenderAction::None;
        };

        tracing::debug!(
            "Requesting older logs before timestamp: {}, total_loaded: {}",
            oldest_ts,
            state.total_loaded
        );

        // Mark as fetching to prevent duplicate requests
        state.fetching_older = true;

        // Spawn task to fetch older logs (using density-based pagination)
        let key = state.container_key.clone();
        if let Some(host) = self.connected_hosts.get(&key.host_id) {
            let host_clone = host.clone();
            let container_id = key.container_id.clone();
            let container_created = self.containers.get(&key).and_then(|c| c.created);
            let tx_clone = self.event_tx.clone();

            tokio::spawn(async move {
                fetch_older_logs(
                    host_clone,
                    container_id,
                    oldest_ts,
                    newest_ts,
                    container_created,
                    1000,
                    tx_clone,
                )
                .await;
            });
        }

        RenderAction::None // Don't render yet, wait for LogBatchPrepend
    }
}

#[cfg(test)]
mod logs_only_tests {
    use super::*;
    use crate::core::types::{AppEvent, Column, ColumnConfig, Container, ContainerState};
    use std::collections::HashMap;
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

    fn container(id: &str, state: ContainerState) -> Container {
        Container {
            id: id.to_string(),
            name: id.to_string(),
            state,
            health: None,
            created: None,
            started: None,
            stats: Default::default(),
            host_id: "nas".to_string(),
            dozzle_url: None,
            restart_count: None,
            compose_project: None,
            image: None,
            ports: None,
        }
    }

    /// `dtop logs <id>` opens that container's log view once it is listed, even
    /// when it is stopped and show-all is off, and leaving the view quits
    #[test]
    fn logs_only_opens_the_container_then_quits_with_the_view() {
        let mut state = state();
        state.logs_only = Some("abc123".to_string());
        state.handle_event(AppEvent::InitialContainerList(
            "nas".to_string(),
            vec![
                container("fff000aaa111", ContainerState::Running),
                container("abc123def456", ContainerState::Exited),
            ],
        ));
        let key = ContainerKey::new("nas".to_string(), "abc123def456".to_string());
        assert_eq!(state.view_state, ViewState::LogView(key));
        assert!(!state.should_quit);

        state.handle_exit_log_view();
        assert!(state.should_quit);
    }

    /// Without `logs_only`, leaving the log view goes back to the list
    #[test]
    fn leaving_the_log_view_normally_does_not_quit() {
        let mut state = state();
        state.handle_event(AppEvent::InitialContainerList(
            "nas".to_string(),
            vec![container("abc123def456", ContainerState::Running)],
        ));
        assert_eq!(state.view_state, ViewState::ContainerList);
        state.view_state = ViewState::LogView(ContainerKey::new(
            "nas".to_string(),
            "abc123def456".to_string(),
        ));
        state.handle_exit_log_view();
        assert!(!state.should_quit);
    }

    /// 't' in the log view hides dtop's timestamp on loaded and later lines,
    /// leaves the stats header alone, and carries over to the next log view
    #[test]
    fn t_toggles_timestamps_in_the_log_view() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let line = |state: &AppState, i: usize| -> String {
            let lines = &state.log_state.as_ref().unwrap().formatted_lines;
            lines[i].spans.iter().map(|s| s.content.as_ref()).collect()
        };
        let entry = |msg: &str| LogEntry::parse(&format!("2025-10-29T10:15:30Z {msg}")).unwrap();

        let mut state = state();
        state.handle_event(AppEvent::InitialContainerList(
            "nas".to_string(),
            vec![container("abc123def456", ContainerState::Running)],
        ));
        let key = ContainerKey::new("nas".to_string(), "abc123def456".to_string());
        state.open_log_view(&key);
        state.handle_log_line(key.clone(), entry("first"));
        assert_ne!(line(&state, 0), "first");

        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::Char('t'),
            KeyModifiers::NONE,
        )));
        assert!(state.show_system_header);
        assert_eq!(line(&state, 0), "first");
        state.handle_log_line(key.clone(), entry("second"));
        assert_eq!(line(&state, 1), "second");

        state.handle_exit_log_view();
        state.open_log_view(&key);
        state.handle_log_line(key, entry("third"));
        assert_eq!(line(&state, 0), "third");
    }

    /// '/' types a query (keys are text, not commands), Enter jumps to the
    /// newest match, n/N walk the matches, and Esc drops the search before a
    /// second Esc leaves the view
    #[test]
    fn search_walks_the_matching_lines() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let entry = |second: usize, msg: &str| {
            LogEntry::parse(&format!("2025-10-29T10:15:{second:02}Z {msg}")).unwrap()
        };

        let mut state = state();
        state.handle_event(AppEvent::InitialContainerList(
            "nas".to_string(),
            vec![container("abc123def456", ContainerState::Running)],
        ));
        let key = ContainerKey::new("nas".to_string(), "abc123def456".to_string());
        state.open_log_view(&key);
        state.last_viewport_height = 5;
        for i in 0..40 {
            let msg = if i % 10 == 3 { "ERROR boom" } else { "ok" };
            state.handle_log_line(key.clone(), entry(i, msg));
        }
        // A match in the bottom rows is on screen already, so nothing moves
        state.handle_log_line(key.clone(), entry(40, "visible line"));
        press_keys(&mut state, "/visible");
        assert!(state.is_at_bottom);
        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        let press = |state: &mut AppState, code: KeyCode| {
            state.handle_event(AppEvent::KeyInput(KeyEvent::new(code, KeyModifiers::NONE)));
        };
        let current = |state: &AppState| {
            let log = state.log_state.as_ref().unwrap();
            log.search.as_ref().and_then(|search| search.current)
        };

        // 'q' and 'n' are part of the query here, not quit and next
        press(&mut state, KeyCode::Char('/'));
        for c in "qn".chars() {
            press(&mut state, KeyCode::Char(c));
        }
        assert!(!state.should_quit);
        press(&mut state, KeyCode::Backspace);
        press(&mut state, KeyCode::Backspace);
        for c in "error".chars() {
            press(&mut state, KeyCode::Char(c));
        }
        assert_eq!(current(&state), None);

        // Enter lands on the newest match and scrolls it into view: following
        // the bottom, rows 35..40 are showing, whatever the last draw recorded
        press(&mut state, KeyCode::Enter);
        assert_eq!(current(&state), Some(33));
        assert!(!state.is_at_bottom);
        assert_eq!(state.log_state.as_ref().unwrap().scroll_offset, 31);

        press(&mut state, KeyCode::Char('N'));
        assert_eq!(current(&state), Some(23));
        assert_eq!(state.log_state.as_ref().unwrap().scroll_offset, 21);
        press(&mut state, KeyCode::Char('n'));
        assert_eq!(current(&state), Some(33));
        // Nothing newer: stays put and says so
        press(&mut state, KeyCode::Char('n'));
        assert_eq!(current(&state), Some(33));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("No newer matches")
        );

        // Older lines loaded above shift the entry the search is on
        state.handle_log_batch_prepend(
            key.clone(),
            vec![entry(0, "older"), entry(0, "older")],
            false,
        );
        assert_eq!(current(&state), Some(35));

        press(&mut state, KeyCode::Esc);
        assert!(state.log_state.as_ref().unwrap().search.is_none());
        assert!(matches!(state.view_state, ViewState::LogView(_)));
        press(&mut state, KeyCode::Esc);
        assert_eq!(state.view_state, ViewState::ContainerList);
    }

    /// Types `text` and presses Enter
    fn press_keys(state: &mut AppState, text: &str) {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        for code in text.chars().map(KeyCode::Char).chain([KeyCode::Enter]) {
            state.handle_event(AppEvent::KeyInput(KeyEvent::new(code, KeyModifiers::NONE)));
        }
    }

    #[test]
    fn search_without_a_match_says_so() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = state();
        state.handle_event(AppEvent::InitialContainerList(
            "nas".to_string(),
            vec![container("abc123def456", ContainerState::Running)],
        ));
        let key = ContainerKey::new("nas".to_string(), "abc123def456".to_string());
        state.open_log_view(&key);
        state.handle_log_line(
            key,
            LogEntry::parse("2025-10-29T10:15:30Z all good").unwrap(),
        );
        for code in [KeyCode::Char('/'), KeyCode::Char('x'), KeyCode::Enter] {
            state.handle_event(AppEvent::KeyInput(KeyEvent::new(code, KeyModifiers::NONE)));
        }
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("No matches for \"x\"")
        );
    }
}
