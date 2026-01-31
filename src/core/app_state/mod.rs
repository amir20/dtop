use ratatui::widgets::{ListState, TableState};
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::mpsc;
use tui_input::Input;

use crate::core::types::{
    AppEvent, Container, ContainerKey, HostId, LogState, RenderAction, SortField, SortState,
    ViewState,
};
use crate::docker::connection::DockerHost;

// Import all the event handler modules
mod actions;
mod container_events;
mod integrations;
mod log_view;
mod navigation;
mod search;
mod sorting;

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
    /// Log state for the currently viewed container (None if not viewing logs)
    pub log_state: Option<LogState>,
    /// Whether the user is at the bottom of the logs (for auto-scroll behavior)
    pub is_at_bottom: bool,
    /// Last known viewport height for page up/down calculations
    pub last_viewport_height: usize,
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
    /// Action menu list state for selection tracking
    pub action_menu_state: ListState,
    /// Search input widget
    pub search_input: Input,
    /// Connection errors to display (host_id -> (error_message, timestamp))
    pub connection_errors: HashMap<HostId, (String, Instant)>,
    /// Last time containers were sorted (for throttling)
    pub last_sort_time: Instant,
}

impl AppState {
    /// Creates a new AppState instance
    pub fn new(
        connected_hosts: HashMap<String, DockerHost>,
        event_tx: mpsc::Sender<AppEvent>,
        show_all: bool,
        sort_field: SortField,
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
            log_state: None,
            is_at_bottom: true,
            last_viewport_height: 20, // Default to 20 lines (will be updated on first render)
            connected_hosts,
            event_tx,
            is_ssh_session,
            show_help: false,
            sort_state: SortState::new(sort_field), // Use configured sort field with default direction
            show_all_containers: show_all,
            action_menu_state: ListState::default(), // Default to no selection
            search_input: Input::default(),
            connection_errors: HashMap::new(),
            last_sort_time: Instant::now(),
        }
    }

    /// Processes a single event and returns what action to take
    pub fn handle_event(&mut self, event: AppEvent) -> RenderAction {
        // Log stats and log lines at TRACE level since they're very frequent, everything else at DEBUG
        match &event {
            AppEvent::ContainerStat(_, _) => tracing::trace!("Handling stat update: {:?}", event),
            AppEvent::LogLine(_, _) => tracing::trace!("Handling log line: {:?}", event),

            _ => tracing::debug!("Handling event: {:?}", event),
        }

        match event {
            AppEvent::InitialContainerList(host_id, container_list) => {
                self.handle_initial_container_list(host_id, container_list)
            }
            AppEvent::ContainerCreated(container) => self.handle_container_created(container),
            AppEvent::ContainerDestroyed(key) => self.handle_container_destroyed(key),
            AppEvent::ContainerStateChanged(key, state) => {
                self.handle_container_state_changed(key, state)
            }
            AppEvent::ContainerStat(key, stats) => self.handle_container_stat(key, stats),
            AppEvent::ContainerHealthChanged(key, health) => {
                self.handle_container_health_changed(key, health)
            }
            AppEvent::Resize => RenderAction::Render, // Always redraw on resize
            AppEvent::Quit => {
                self.should_quit = true;
                RenderAction::None
            }
            AppEvent::SelectPrevious => self.handle_select_previous(),
            AppEvent::SelectNext => self.handle_select_next(),
            AppEvent::EnterPressed => self.handle_enter_pressed(),
            AppEvent::ExitLogView => self.handle_exit_log_view(),
            AppEvent::ShowLogView => self.handle_show_log_view(),
            AppEvent::ScrollUp => self.handle_scroll_up(),
            AppEvent::ScrollDown => self.handle_scroll_down(),
            AppEvent::ScrollToTop => self.handle_scroll_to_top(),
            AppEvent::ScrollToBottom => self.handle_scroll_to_bottom(),
            AppEvent::ScrollPageUp => self.handle_scroll_page_up(),
            AppEvent::ScrollPageDown => self.handle_scroll_page_down(),
            AppEvent::LogBatchPrepend(key, log_entries, has_more_history) => {
                self.handle_log_batch_prepend(key, log_entries, has_more_history)
            }
            AppEvent::LogLine(key, log_line) => self.handle_log_line(key, log_line),
            AppEvent::OpenDozzle => self.handle_open_dozzle(),
            AppEvent::ToggleHelp => self.handle_toggle_help(),
            AppEvent::CycleSortField => self.handle_cycle_sort_field(),
            AppEvent::SetSortField(field) => self.handle_set_sort_field(field),
            AppEvent::ToggleShowAll => self.handle_toggle_show_all(),
            AppEvent::CancelActionMenu => self.handle_cancel_action_menu(),
            AppEvent::SelectActionUp => self.handle_select_action_up(),
            AppEvent::SelectActionDown => self.handle_select_action_down(),
            AppEvent::ActionInProgress(key, action) => self.handle_action_in_progress(key, action),
            AppEvent::ActionSuccess(key, action) => self.handle_action_success(key, action),
            AppEvent::ActionError(key, action, error) => {
                self.handle_action_error(key, action, error)
            }
            AppEvent::EnterSearchMode => self.handle_enter_search_mode(),
            AppEvent::SearchKeyEvent(key_event) => self.handle_search_key_event(key_event),
            AppEvent::ConnectionError(host_id, error) => {
                self.handle_connection_error(host_id, error)
            }
            AppEvent::HostConnected(docker_host) => self.handle_host_connected(docker_host),
        }
    }

    /// Handles a connection error by storing it with a timestamp
    fn handle_connection_error(&mut self, host_id: HostId, error: String) -> RenderAction {
        // Store the error with current timestamp
        self.connection_errors
            .insert(host_id, (error, Instant::now()));

        // Remove errors older than 10 seconds
        self.connection_errors
            .retain(|_, (_, timestamp)| timestamp.elapsed().as_secs() < 10);

        RenderAction::Render // Redraw to show the error
    }

    /// Handles a new Docker host connection by adding it to the connected hosts
    fn handle_host_connected(&mut self, docker_host: DockerHost) -> RenderAction {
        use tracing::debug;

        let host_id = docker_host.host_id.clone();
        debug!("Adding host to connected_hosts: {}", host_id);
        self.connected_hosts.insert(host_id.clone(), docker_host);

        // Clear any connection error for this host
        self.connection_errors.remove(&host_id);

        RenderAction::None // No need to force redraw, container list will update via normal events
    }
}
