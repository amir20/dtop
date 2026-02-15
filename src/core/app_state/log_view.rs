use crate::core::app_state::AppState;
use crate::core::types::{ContainerKey, LogState, RenderAction, ViewState};
use crate::docker::logs::{LogEntry, fetch_older_logs};

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
            ViewState::ActionMenu(_) => {
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

        let Some(container_key) = self.sorted_container_keys.get(selected_idx) else {
            return RenderAction::None;
        };

        // Get container creation time for progress calculation
        let container_created_at = self.containers.get(container_key).and_then(|c| c.created);

        // Create new log state for this container
        let mut new_log_state = LogState::new(container_key.clone(), container_created_at);

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

        // Switch back to container list view
        self.view_state = ViewState::ContainerList;

        RenderAction::Render // Force draw - view changed
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

        // Extract timestamp before moving log_entry
        let timestamp = log_entry.timestamp;

        // Store the raw log entry (already owned, no clone needed)
        state.log_entries.push(log_entry);

        // Update newest timestamp for progress calculation
        state.newest_timestamp = Some(timestamp);

        RenderAction::Render
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

        // Prepend raw log entries to the beginning
        let mut new_entries = log_entries;
        new_entries.append(&mut state.log_entries);
        state.log_entries = new_entries;

        state.oldest_timestamp = oldest;
        state.has_more_history = has_more_history;
        state.total_loaded += num_entries;
        state.fetching_older = false;

        // Update newest timestamp if this is the first batch (initial load)
        if is_initial_load {
            state.newest_timestamp = newest;
        }

        // Adjust scroll offset to maintain visual position during pagination
        // Only adjust if this is NOT the initial load (initial load should start at bottom)
        if !is_initial_load {
            state.scroll_offset += num_entries;
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
