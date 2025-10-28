use ratatui::widgets::TableState;
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::docker::DockerHost;
use crate::logs::{LogEntry, stream_container_logs};
use crate::types::{AppEvent, Container, ContainerKey, ViewState};

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
    /// Current container logs (only one container at a time)
    pub current_logs: Option<(ContainerKey, Vec<LogEntry>)>,
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
}

impl AppState {
    /// Creates a new AppState instance
    pub fn new(
        connected_hosts: HashMap<String, DockerHost>,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Self {
        Self {
            containers: HashMap::new(),
            sorted_container_keys: Vec::new(),
            should_quit: false,
            table_state: TableState::default(),
            view_state: ViewState::ContainerList,
            current_logs: None,
            log_scroll_offset: 0,
            is_at_bottom: true,
            log_stream_handle: None,
            connected_hosts,
            event_tx,
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

        // Sort by host_id first, then by container name
        self.sorted_container_keys.sort_by(|a, b| {
            let container_a = self.containers.get(a).unwrap();
            let container_b = self.containers.get(b).unwrap();

            match container_a.host_id.cmp(&container_b.host_id) {
                std::cmp::Ordering::Equal => container_a.name.cmp(&container_b.name),
                other => other,
            }
        });

        // Select first row if we have containers
        if !self.containers.is_empty() {
            self.table_state.select(Some(0));
        }

        true // Force draw - table structure changed
    }

    fn handle_container_created(&mut self, container: Container) -> bool {
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        let sort_key = (container.host_id.clone(), container.name.clone());
        self.containers.insert(key.clone(), container);

        // Insert into sorted position
        let insert_pos = self
            .sorted_container_keys
            .binary_search_by(|probe_key| {
                let probe_container = self.containers.get(probe_key).unwrap();
                let probe_sort_key = (&probe_container.host_id, &probe_container.name);
                probe_sort_key.cmp(&(&sort_key.0, &sort_key.1))
            })
            .unwrap_or_else(|pos| pos);
        self.sorted_container_keys.insert(insert_pos, key);

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

    fn handle_select_previous(&mut self) -> bool {
        let container_count = self.containers.len();
        if container_count > 0 {
            let selected = self.table_state.selected().unwrap_or(0);
            if selected > 0 {
                self.table_state.select(Some(selected - 1));
            }
        }
        true // Force draw - selection changed
    }

    fn handle_select_next(&mut self) -> bool {
        let container_count = self.containers.len();
        if container_count > 0 {
            let selected = self.table_state.selected().unwrap_or(0);
            if selected < container_count - 1 {
                self.table_state.select(Some(selected + 1));
            }
        }
        true // Force draw - selection changed
    }

    fn handle_enter_pressed(&mut self) -> bool {
        // Only handle Enter in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return false;
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

        // Initialize log storage for this container (clear any previous logs)
        self.current_logs = Some((container_key.clone(), Vec::new()));

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

    fn handle_exit_log_view(&mut self) -> bool {
        // Only handle Escape when in log view
        if !matches!(self.view_state, ViewState::LogView(_)) {
            return false;
        }

        // Stop log streaming
        if let Some(handle) = self.log_stream_handle.take() {
            handle.abort();
        }

        // Clear current logs
        self.current_logs = None;

        // Switch back to container list view
        self.view_state = ViewState::ContainerList;

        true // Force draw - view changed
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

        // Only scroll if we have logs
        if self.current_logs.is_some() {
            // Increment scroll offset
            self.log_scroll_offset = self.log_scroll_offset.saturating_add(1);

            // Will be clamped in UI and is_at_bottom will be recalculated there
            return true; // Force draw
        }

        false
    }

    fn handle_log_line(&mut self, key: ContainerKey, log_entry: LogEntry) -> bool {
        // Only add log line if we're currently viewing this container's logs
        if let Some((current_key, logs)) = &mut self.current_logs
            && current_key == &key
        {
            logs.push(log_entry);

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
