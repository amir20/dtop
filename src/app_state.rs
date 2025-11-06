use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::TableState;
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::docker::DockerHost;
use crate::logs::{LogEntry, stream_container_logs};
use crate::types::{
    AppEvent, Container, ContainerKey, SortDirection, SortField, SortState, ViewState,
};

/// Style for log timestamps (yellow + bold)
const TIMESTAMP_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);

/// Application state that manages all runtime data
pub struct AppState {
    /// All containers indexed by (host_id, container_id)
    pub containers: HashMap<ContainerKey, Container>,
    /// Pre-sorted list of container keys for efficient rendering
    pub sorted_container_keys: Vec<ContainerKey>,
    /// Whether the application should quit
    pub should_quit: bool,
    /// Table selection state
    pub table_state: TableState,
    /// Current view (container list or log view)
    pub view_state: ViewState,
    /// Currently viewed container key (for log view)
    pub current_log_container: Option<ContainerKey>,
    /// Cached formatted log text (to avoid reformatting on every render)
    pub formatted_log_text: Text<'static>,
    /// Current scroll position (number of lines scrolled from top)
    pub log_scroll_offset: usize,
    /// Whether the user is at the bottom of the logs (for auto-scroll behavior)
    pub is_at_bottom: bool,
    /// Handle to the currently running log stream task
    pub log_stream_handle: Option<tokio::task::JoinHandle<()>>,
    /// Connected Docker hosts for log streaming
    pub connected_hosts: HashMap<String, DockerHost>,
    /// Event sender for spawning log streams
    pub event_tx: mpsc::Sender<AppEvent>,
    /// Whether the app is running in an SSH session
    pub is_ssh_session: bool,
    /// Whether the help popup is currently shown
    pub show_help: bool,
    /// Current sort state (field + direction)
    pub sort_state: SortState,
    /// Whether to show all containers (including stopped ones)
    pub show_all_containers: bool,
    /// Currently selected host filter (None = all hosts)
    pub selected_host_filter: Option<String>,
    /// Host selection state (for navigating host list)
    pub host_selection_state: TableState,
}

impl AppState {
    /// Creates a new AppState instance
    pub fn new(
        connected_hosts: HashMap<String, DockerHost>,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Self {
        // Detect if running in SSH session
        let is_ssh_session = std::env::var("SSH_CLIENT").is_ok()
            || std::env::var("SSH_TTY").is_ok()
            || std::env::var("SSH_CONNECTION").is_ok();

        Self {
            containers: HashMap::new(),
            sorted_container_keys: Vec::new(),
            should_quit: false,
            table_state: TableState::default(),
            view_state: ViewState::ContainerList,
            current_log_container: None,
            formatted_log_text: Text::default(),
            log_scroll_offset: 0,
            is_at_bottom: true,
            log_stream_handle: None,
            connected_hosts,
            event_tx,
            is_ssh_session,
            show_help: false,
            sort_state: SortState::default(), // Default to Uptime descending
            show_all_containers: false,       // Default to showing only running containers
            selected_host_filter: None,       // Show all hosts by default
            host_selection_state: TableState::default(),
        }
    }

    /// Processes a single event and returns whether UI should be redrawn
    pub fn handle_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::InitialContainerList(host_id, container_list) => {
                self.handle_initial_container_list(host_id, container_list)
            }
            AppEvent::ContainerCreated(container) => self.handle_container_created(container),
            AppEvent::ContainerDestroyed(key) => self.handle_container_destroyed(key),
            AppEvent::ContainerStat(key, stats) => self.handle_container_stat(key, stats),
            AppEvent::ContainerHealthChanged(key, health) => {
                self.handle_container_health_changed(key, health)
            }
            AppEvent::Resize => true, // Always redraw on resize
            AppEvent::Quit => {
                self.should_quit = true;
                false
            }
            AppEvent::SelectPrevious => self.handle_select_previous(),
            AppEvent::SelectNext => self.handle_select_next(),
            AppEvent::EnterPressed => self.handle_enter_pressed(),
            AppEvent::ExitLogView => self.handle_exit_log_view(),
            AppEvent::ScrollUp => self.handle_scroll_up(),
            AppEvent::ScrollDown => self.handle_scroll_down(),
            AppEvent::LogLine(key, log_line) => self.handle_log_line(key, log_line),
            AppEvent::OpenDozzle => self.handle_open_dozzle(),
            AppEvent::ToggleHelp => self.handle_toggle_help(),
            AppEvent::CycleSortField => self.handle_cycle_sort_field(),
            AppEvent::SetSortField(field) => self.handle_set_sort_field(field),
            AppEvent::ToggleShowAll => self.handle_toggle_show_all(),
            AppEvent::ShowHostSelection => self.handle_show_host_selection(),
        }
    }

    fn handle_initial_container_list(
        &mut self,
        host_id: String,
        container_list: Vec<Container>,
    ) -> bool {
        for container in container_list {
            let key = ContainerKey::new(host_id.clone(), container.id.clone());
            self.containers.insert(key.clone(), container);
            self.sorted_container_keys.push(key);
        }

        // Sort using current sort field
        self.sort_containers();

        // Select first row if we have containers
        if !self.containers.is_empty() {
            self.table_state.select(Some(0));
        }

        true // Force draw - table structure changed
    }

    fn handle_container_created(&mut self, container: Container) -> bool {
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        self.containers.insert(key.clone(), container);
        self.sorted_container_keys.push(key);

        // Re-sort the entire list with current sort field
        self.sort_containers();

        // Select first row if this is the first container
        if self.containers.len() == 1 {
            self.table_state.select(Some(0));
        }

        true // Force draw - table structure changed
    }

    fn handle_container_destroyed(&mut self, key: ContainerKey) -> bool {
        self.containers.remove(&key);
        self.sorted_container_keys.retain(|k| k != &key);

        // Adjust selection if needed
        let container_count = self.containers.len();
        if container_count == 0 {
            self.table_state.select(None);
        } else if let Some(selected) = self.table_state.selected()
            && selected >= container_count
        {
            self.table_state.select(Some(container_count - 1));
        }

        true // Force draw - table structure changed
    }

    fn handle_container_stat(
        &mut self,
        key: ContainerKey,
        stats: crate::types::ContainerStats,
    ) -> bool {
        if let Some(container) = self.containers.get_mut(&key) {
            container.stats = stats;
        }
        false // No force draw - just stats update
    }

    fn handle_container_health_changed(
        &mut self,
        key: ContainerKey,
        health: crate::types::HealthStatus,
    ) -> bool {
        if let Some(container) = self.containers.get_mut(&key) {
            container.health = Some(health);
        }
        true // Force draw - health status changed (visible in UI)
    }

    fn handle_select_previous(&mut self) -> bool {
        match self.view_state {
            ViewState::ContainerList => {
                let container_count = self.sorted_container_keys.len();
                if container_count > 0 {
                    let selected = self.table_state.selected().unwrap_or(0);
                    if selected > 0 {
                        self.table_state.select(Some(selected - 1));
                    }
                }
                true // Force draw - selection changed
            }
            ViewState::HostSelection => {
                let host_count = self.connected_hosts.len() + 1; // +1 for "All Hosts"
                if host_count > 0 {
                    let selected = self.host_selection_state.selected().unwrap_or(0);
                    if selected > 0 {
                        self.host_selection_state.select(Some(selected - 1));
                    }
                }
                true // Force draw - selection changed
            }
            ViewState::LogView(_) => false, // Handled by scroll_up
        }
    }

    fn handle_select_next(&mut self) -> bool {
        match self.view_state {
            ViewState::ContainerList => {
                let container_count = self.sorted_container_keys.len();
                if container_count > 0 {
                    let selected = self.table_state.selected().unwrap_or(0);
                    if selected < container_count - 1 {
                        self.table_state.select(Some(selected + 1));
                    }
                }
                true // Force draw - selection changed
            }
            ViewState::HostSelection => {
                let host_count = self.connected_hosts.len() + 1; // +1 for "All Hosts"
                if host_count > 0 {
                    let selected = self.host_selection_state.selected().unwrap_or(0);
                    if selected < host_count - 1 {
                        self.host_selection_state.select(Some(selected + 1));
                    }
                }
                true // Force draw - selection changed
            }
            ViewState::LogView(_) => false, // Handled by scroll_down
        }
    }

    fn handle_enter_pressed(&mut self) -> bool {
        match self.view_state {
            ViewState::ContainerList => {
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
                        stream_container_logs(host_clone, container_id, tx_clone).await;
                    });

                    self.log_stream_handle = Some(handle);
                }

                true // Force draw - view changed
            }
            ViewState::HostSelection => {
                // Get selected host
                let Some(selected_idx) = self.host_selection_state.selected() else {
                    return false;
                };

                // Get sorted list of hosts
                let mut hosts: Vec<String> = self
                    .connected_hosts
                    .keys()
                    .cloned()
                    .collect();
                hosts.sort();

                // Determine which host was selected
                // Index 0 = "All Hosts" (None), index 1+ = specific hosts
                let selected_host = if selected_idx == 0 {
                    None
                } else {
                    hosts.get(selected_idx - 1).cloned()
                };

                // Handle the host selection
                self.handle_select_host(selected_host)
            }
            ViewState::LogView(_) => false,
        }
    }

    fn handle_exit_log_view(&mut self) -> bool {
        // If help is shown, close it first
        if self.show_help {
            self.show_help = false;
            return true; // Force redraw
        }

        // Handle Escape based on current view
        match &self.view_state {
            ViewState::LogView(_) => {
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
            ViewState::HostSelection => {
                // Exit host selection and return to container list
                self.view_state = ViewState::ContainerList;
                true // Force draw - view changed
            }
            ViewState::ContainerList => false,
        }
    }

    fn handle_scroll_up(&mut self) -> bool {
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

    fn handle_scroll_down(&mut self) -> bool {
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

    fn handle_log_line(&mut self, key: ContainerKey, log_entry: LogEntry) -> bool {
        // Only add log line if we're currently viewing this container's logs
        if let Some(current_key) = &self.current_log_container
            && current_key == &key
        {
            // Format the new log entry with timestamp and append to cached text
            let timestamp_str = log_entry.timestamp.format("%Y-%m-%d %H:%M:%S").to_string();

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

    fn handle_open_dozzle(&mut self) -> bool {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return false;
        }

        // Don't open URLs in SSH sessions
        if self.is_ssh_session {
            return false;
        }

        // Get the selected container
        let Some(selected_idx) = self.table_state.selected() else {
            return false;
        };

        let Some(container_key) = self.sorted_container_keys.get(selected_idx) else {
            return false;
        };

        // Get the container and its Dozzle URL
        let Some(container) = self.containers.get(container_key) else {
            return false;
        };

        let Some(dozzle_url) = &container.dozzle_url else {
            return false;
        };

        // Build the full URL: {dozzle}/container/{containerId}
        let full_url = format!(
            "{}/container/{}",
            dozzle_url.trim_end_matches('/'),
            container_key.container_id
        );

        // Open the URL using the 'open' crate (cross-platform)
        let _ = open::that(&full_url);

        false // No need to force draw
    }

    fn handle_toggle_help(&mut self) -> bool {
        self.show_help = !self.show_help;
        true // Force redraw to show/hide popup
    }

    fn handle_cycle_sort_field(&mut self) -> bool {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return false;
        }

        // Cycle to next sort field with default direction
        self.sort_state = SortState::new(self.sort_state.field.next());

        // Re-sort the container list
        self.sort_containers();

        true // Force redraw - sort order changed
    }

    fn handle_set_sort_field(&mut self, field: SortField) -> bool {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return false;
        }

        // If same field, toggle direction; otherwise use default direction
        if self.sort_state.field == field {
            self.sort_state.direction = self.sort_state.direction.toggle();
        } else {
            self.sort_state = SortState::new(field);
        }

        // Re-sort the container list
        self.sort_containers();

        true // Force redraw - sort order changed
    }

    fn handle_toggle_show_all(&mut self) -> bool {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return false;
        }

        // Toggle the show_all_containers flag
        self.show_all_containers = !self.show_all_containers;

        // Re-sort/filter the container list
        self.sort_containers();

        // Adjust selection if needed after filtering
        let container_count = self.sorted_container_keys.len();
        if container_count == 0 {
            self.table_state.select(None);
        } else if let Some(selected) = self.table_state.selected()
            && selected >= container_count
        {
            self.table_state.select(Some(container_count - 1));
        }

        true // Force redraw - visibility changed
    }

    fn handle_show_host_selection(&mut self) -> bool {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return false;
        }

        // Switch to HostSelection view
        self.view_state = ViewState::HostSelection;

        // Select current host filter in the list
        // Index 0 = "All Hosts", index 1+ = specific hosts
        let selected_idx = if let Some(ref filter) = self.selected_host_filter {
            // Find the index of the current filter
            let mut hosts: Vec<String> = self.connected_hosts.keys().cloned().collect();
            hosts.sort();
            hosts.iter().position(|h| h == filter).map(|i| i + 1).unwrap_or(0)
        } else {
            0 // "All Hosts" is selected
        };

        self.host_selection_state.select(Some(selected_idx));

        true // Force redraw
    }

    fn handle_select_host(&mut self, host_id: Option<String>) -> bool {
        // Set the host filter
        self.selected_host_filter = host_id;

        // Return to container list view
        self.view_state = ViewState::ContainerList;

        // Re-sort/filter containers with new host filter
        self.sort_containers();

        // Reset selection to first container
        if !self.sorted_container_keys.is_empty() {
            self.table_state.select(Some(0));
        } else {
            self.table_state.select(None);
        }

        true // Force redraw
    }

    /// Sorts the container keys based on the current sort field and direction
    fn sort_containers(&mut self) {
        use crate::types::ContainerState;

        // Rebuild sorted_container_keys from containers, filtering by running state and host if needed
        self.sorted_container_keys = self
            .containers
            .keys()
            .filter(|key| {
                // Filter by running state
                let state_match = if self.show_all_containers {
                    true // Show all containers
                } else {
                    // Only show running containers
                    self.containers
                        .get(key)
                        .map(|c| c.state == ContainerState::Running)
                        .unwrap_or(false)
                };

                // Filter by host
                let host_match = if let Some(ref filter_host) = self.selected_host_filter {
                    &key.host_id == filter_host
                } else {
                    true // Show all hosts
                };

                state_match && host_match
            })
            .cloned()
            .collect();

        let direction = self.sort_state.direction;

        match self.sort_state.field {
            SortField::Uptime => {
                self.sorted_container_keys.sort_by(|a, b| {
                    let container_a = self.containers.get(a).unwrap();
                    let container_b = self.containers.get(b).unwrap();

                    // First by host_id
                    match container_a.host_id.cmp(&container_b.host_id) {
                        std::cmp::Ordering::Equal => {
                            // Then by creation time
                            let ord = match (&container_a.created, &container_b.created) {
                                (Some(a_time), Some(b_time)) => a_time.cmp(b_time),
                                (Some(_), None) => std::cmp::Ordering::Greater,
                                (None, Some(_)) => std::cmp::Ordering::Less,
                                (None, None) => std::cmp::Ordering::Equal,
                            };
                            // Reverse if descending
                            if direction == SortDirection::Descending {
                                ord.reverse()
                            } else {
                                ord
                            }
                        }
                        other => other,
                    }
                });
            }
            SortField::Name => {
                self.sorted_container_keys.sort_by(|a, b| {
                    let container_a = self.containers.get(a).unwrap();
                    let container_b = self.containers.get(b).unwrap();

                    // First by host_id
                    match container_a.host_id.cmp(&container_b.host_id) {
                        std::cmp::Ordering::Equal => {
                            let ord = container_a.name.cmp(&container_b.name);
                            // Reverse if descending
                            if direction == SortDirection::Descending {
                                ord.reverse()
                            } else {
                                ord
                            }
                        }
                        other => other,
                    }
                });
            }
            SortField::Cpu => {
                self.sorted_container_keys.sort_by(|a, b| {
                    let container_a = self.containers.get(a).unwrap();
                    let container_b = self.containers.get(b).unwrap();

                    // First by host_id
                    match container_a.host_id.cmp(&container_b.host_id) {
                        std::cmp::Ordering::Equal => {
                            let ord = container_a
                                .stats
                                .cpu
                                .partial_cmp(&container_b.stats.cpu)
                                .unwrap_or(std::cmp::Ordering::Equal);
                            // Reverse if descending
                            if direction == SortDirection::Descending {
                                ord.reverse()
                            } else {
                                ord
                            }
                        }
                        other => other,
                    }
                });
            }
            SortField::Memory => {
                self.sorted_container_keys.sort_by(|a, b| {
                    let container_a = self.containers.get(a).unwrap();
                    let container_b = self.containers.get(b).unwrap();

                    // First by host_id
                    match container_a.host_id.cmp(&container_b.host_id) {
                        std::cmp::Ordering::Equal => {
                            let ord = container_a
                                .stats
                                .memory
                                .partial_cmp(&container_b.stats.memory)
                                .unwrap_or(std::cmp::Ordering::Equal);
                            // Reverse if descending
                            if direction == SortDirection::Descending {
                                ord.reverse()
                            } else {
                                ord
                            }
                        }
                        other => other,
                    }
                });
            }
        }
    }
}
