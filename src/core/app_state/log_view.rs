use chrono::Local;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::core::app_state::AppState;
use crate::core::types::{ContainerKey, ViewState};
use crate::docker::logs::LogEntry;

/// Style for log timestamps (yellow + bold)
const TIMESTAMP_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);

impl AppState {
    pub(super) fn handle_enter_pressed(&mut self) -> bool {
        // Handle Enter based on current view state
        match self.view_state {
            ViewState::SearchMode => {
                // Apply filter and return to ContainerList view
                self.view_state = ViewState::ContainerList;
                return true; // Force redraw to show filter bar
            }
            ViewState::ContainerList => {
                // Open logs for selected container
            }
            _ => {
                // Ignore Enter in other views
                return false;
            }
        }

        // Get the selected container
        let Some(selected_idx) = self.table_state.selected() else {
            return false;
        };

        let Some(container_key) = self.sorted_container_keys.get(selected_idx) else {
            return false;
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

        true // Force draw - view changed
    }

    pub(super) fn handle_exit_log_view(&mut self) -> bool {
        // If help is shown, close it first
        if self.show_help {
            self.show_help = false;
            return true; // Force redraw
        }

        // Handle Escape based on current view state
        match self.view_state {
            ViewState::SearchMode => {
                // Exit search mode and clear filter
                return self.handle_exit_search_mode();
            }
            ViewState::LogView(_) => {
                // Exit log view
            }
            _ => {
                // Ignore Escape in other views
                return false;
            }
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

        true // Force draw - view changed
    }

    pub(super) fn handle_scroll_up(&mut self) -> bool {
        // Only handle scroll in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return false;
        }

        // Scroll up (decrease offset)
        if self.log_scroll_offset > 0 {
            self.log_scroll_offset = self.log_scroll_offset.saturating_sub(1);
            self.is_at_bottom = false; // User scrolled away from bottom
            return true; // Force draw
        }

        false
    }

    pub(super) fn handle_scroll_down(&mut self) -> bool {
        // Only handle scroll in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return false;
        }

        // Only scroll if we have a log container
        if self.current_log_container.is_some() {
            // Increment scroll offset
            self.log_scroll_offset = self.log_scroll_offset.saturating_add(1);

            // Will be clamped in UI and is_at_bottom will be recalculated there
            return true; // Force draw
        }

        false
    }

    pub(super) fn handle_log_line(&mut self, key: ContainerKey, log_entry: LogEntry) -> bool {
        // Only add log line if we're currently viewing this container's logs
        if let Some(current_key) = &self.current_log_container
            && current_key == &key
        {
            // Format the new log entry with timestamp in local timezone and append to cached text
            let local_timestamp = log_entry.timestamp.with_timezone(&Local);
            let timestamp_str = local_timestamp.format("%Y-%m-%d %H:%M:%S").to_string();

            // Create a line with timestamp + ANSI-parsed content
            let mut line_spans = vec![Span::styled(timestamp_str, TIMESTAMP_STYLE), Span::raw(" ")];

            // Append all spans from the ANSI-parsed text (should be a single line)
            if let Some(text_line) = log_entry.text.lines.first() {
                line_spans.extend(text_line.spans.iter().cloned());
            }

            // Add the formatted line to our cached text
            self.formatted_log_text.lines.push(Line::from(line_spans));

            // Only auto-scroll if user is at the bottom
            if self.is_at_bottom {
                // Scroll will be updated to show bottom in UI
            }

            return true; // Force draw - new log line for currently viewed container
        }

        // Ignore log lines for containers we're not viewing
        false
    }
}
