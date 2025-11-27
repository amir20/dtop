use chrono::Local;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::core::app_state::AppState;
use crate::core::types::{ContainerKey, RenderAction, ViewState};
use crate::docker::logs::LogEntry;

/// Style for log timestamps (yellow + bold)
const TIMESTAMP_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);

impl AppState {
    /// Format a log entry into a Line with timestamp and ANSI-parsed content
    fn format_log_entry(log_entry: &LogEntry) -> Line<'static> {
        let local_timestamp = log_entry.timestamp.with_timezone(&Local);
        let timestamp_str = local_timestamp.format("%Y-%m-%d %H:%M:%S").to_string();

        // Create a line with timestamp + ANSI-parsed content
        let mut line_spans = vec![Span::styled(timestamp_str, TIMESTAMP_STYLE), Span::raw(" ")];

        // Append all spans from the ANSI-parsed text (should be a single line)
        if let Some(text_line) = log_entry.text.lines.first() {
            line_spans.extend(text_line.spans.iter().cloned());
        }

        Line::from(line_spans)
    }

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

        // Switch to log view
        self.view_state = ViewState::LogView(container_key.clone());

        // Set the current log container and clear cached text
        self.current_log_container = Some(container_key.clone());
        self.formatted_log_text = Text::default();

        // Reset scroll state - start at bottom
        self.log_scroll_offset = 0;
        self.is_at_bottom = true;

        // Stop any existing log stream
        if let Some(handle) = self.log_stream_handle.take() {
            handle.abort();
        }

        // Start streaming logs for this container
        if let Some(host) = self.connected_hosts.get(&container_key.host_id) {
            let host_clone = host.clone();
            let container_id = container_key.container_id.clone();
            let tx_clone = self.event_tx.clone();

            let handle = tokio::spawn(async move {
                use crate::docker::logs::stream_container_logs;
                stream_container_logs(host_clone, container_id, tx_clone).await;
            });

            self.log_stream_handle = Some(handle);
        }

        RenderAction::Render // Force draw - view changed
    }

    pub(super) fn handle_exit_log_view(&mut self) -> RenderAction {
        // Only handle in LogView
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        // Stop log streaming
        if let Some(handle) = self.log_stream_handle.take() {
            handle.abort();
        }

        // Clear current log container and formatted text
        self.current_log_container = None;
        self.formatted_log_text = Text::default();

        // Switch back to container list view
        self.view_state = ViewState::ContainerList;

        RenderAction::Render // Force draw - view changed
    }

    pub(super) fn handle_scroll_up(&mut self) -> RenderAction {
        // Only handle scroll in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        // Scroll up (decrease offset)
        if self.log_scroll_offset > 0 {
            self.log_scroll_offset = self.log_scroll_offset.saturating_sub(1);
            self.is_at_bottom = false; // User scrolled away from bottom
            return RenderAction::Render; // Force draw
        }

        RenderAction::None
    }

    pub(super) fn handle_scroll_down(&mut self) -> RenderAction {
        // Only handle scroll in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        // Only scroll if we have a log container
        if self.current_log_container.is_some() {
            // Increment scroll offset
            self.log_scroll_offset = self.log_scroll_offset.saturating_add(1);

            // Will be clamped in UI and is_at_bottom will be recalculated there
            return RenderAction::Render; // Force draw
        }

        RenderAction::None
    }

    pub(super) fn handle_scroll_to_top(&mut self) -> RenderAction {
        // Only handle in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        // Scroll to top
        self.log_scroll_offset = 0;
        self.is_at_bottom = false;
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

        // Scroll up by half page (similar to vim's Ctrl+U)
        // We'll use the last known viewport height stored in AppState
        let page_size = self.last_viewport_height / 2;
        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(page_size);
        self.is_at_bottom = false;
        RenderAction::Render
    }

    pub(super) fn handle_scroll_page_down(&mut self) -> RenderAction {
        // Only handle in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return RenderAction::None;
        }

        // Scroll down by half page (similar to vim's Ctrl+D)
        // We'll use the last known viewport height stored in AppState
        let page_size = self.last_viewport_height / 2;
        self.log_scroll_offset = self.log_scroll_offset.saturating_add(page_size);
        // Will be clamped in UI and is_at_bottom will be recalculated there
        RenderAction::Render
    }

    pub(super) fn handle_log_batch(
        &mut self,
        key: ContainerKey,
        log_entries: Vec<LogEntry>,
    ) -> RenderAction {
        // Only add logs if we're currently viewing this container's logs
        if let Some(current_key) = &self.current_log_container
            && current_key == &key
        {
            // Process all log entries at once
            for log_entry in log_entries {
                let formatted_line = Self::format_log_entry(&log_entry);
                self.formatted_log_text.lines.push(formatted_line);
            }

            // Render once after processing all logs
            return RenderAction::Render;
        }

        // Ignore log batch for containers we're not viewing
        RenderAction::None
    }

    pub(super) fn handle_log_line(
        &mut self,
        key: ContainerKey,
        log_entry: LogEntry,
    ) -> RenderAction {
        // Only add log line if we're currently viewing this container's logs
        if let Some(current_key) = &self.current_log_container
            && current_key == &key
        {
            let formatted_line = Self::format_log_entry(&log_entry);
            self.formatted_log_text.lines.push(formatted_line);

            // Only auto-scroll if user is at the bottom
            if self.is_at_bottom {
                // Scroll will be updated to show bottom in UI
            }

            return RenderAction::Render; // Force draw - new log line for currently viewed container
        }

        // Ignore log lines for containers we're not viewing
        RenderAction::None
    }
}
