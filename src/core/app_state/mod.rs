use ratatui::widgets::{ListState, TableState};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::Instant;
use tokio::sync::mpsc;
use tui_input::Input;

use crate::core::types::{
    AppEvent, Column, ColumnConfig, Container, ContainerAction, ContainerKey, HostId, HostInfo,
    LogState, RenderAction, SortDirection, SortState, SystemStats, ViewState,
};
use crate::docker::connection::DockerHost;

// Import all the event handler modules
mod actions;
mod columns;
mod container_events;
mod integrations;
mod log_view;
mod navigation;
mod preferences;
mod search;
mod selection;
pub mod sorting;
pub mod system;

/// A bulk action whose per-container results are still arriving
pub(crate) struct BulkProgress {
    pub action: ContainerAction,
    /// Containers whose result has not arrived yet
    pub pending: HashSet<ContainerKey>,
    pub total: usize,
    pub failed: usize,
    /// Reason for the latest failure, for the summary
    pub last_error: Option<String>,
    /// When the batch was launched or last heard from; stale batches are dropped
    pub last_activity: Instant,
}

/// Application state that manages all runtime data
pub struct AppState {
    /// All containers indexed by (host_id, container_id)
    pub containers: HashMap<ContainerKey, Container>,
    /// Pre-sorted list of container keys for efficient rendering
    pub sorted_container_keys: Vec<ContainerKey>,
    /// Reusable buffer of currently visible columns, refreshed each frame in
    /// place (clear + extend) so rendering does not allocate a new `Vec` per
    /// frame. Internal render scratch space — read only after calling
    /// [`AppState::refresh_visible_columns`], hence `pub(crate)`.
    pub(crate) visible_columns_cache: Vec<Column>,
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
    /// Last known viewport inner width for visual line calculations
    pub last_viewport_width: usize,
    /// Last known number of visible container rows (for container list page up/down)
    pub last_list_viewport_height: usize,
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
    pub column_config: ColumnConfig,
    pub column_selector_state: ListState,
    pub config_path: Option<std::path::PathBuf>,
    /// Sort selector list state for selection tracking
    pub sort_selector_state: ListState,
    /// Connection errors to display (host_id -> (error_message, timestamp))
    pub connection_errors: HashMap<HostId, (String, Instant)>,
    /// Hosts whose connection dropped and that are being retried in the background.
    /// A `BTreeSet` so the rendered banners keep a stable order.
    pub disconnected_hosts: BTreeSet<HostId>,
    /// Capacity of each Docker host, shown in the system stats header.
    /// A `BTreeMap` so the header lines keep a stable order.
    pub host_info: BTreeMap<HostId, HostInfo>,
    /// Latest sample of the local machine's metrics (None until the first
    /// sample arrives, or when no `local` host is configured)
    pub system_stats: Option<SystemStats>,
    /// Latest whole-machine metrics for SSH hosts, keyed by host
    pub remote_system_stats: BTreeMap<HostId, SystemStats>,
    /// Whether the system stats header is shown ('t' toggles). Change it with
    /// [`AppState::set_show_system_header`] so the samplers follow.
    pub show_system_header: bool,
    /// Mirror of `show_system_header` shared with the stats samplers, which
    /// idle while the header is hidden
    pub header_visible: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Last time containers were sorted (for throttling)
    pub last_sort_time: Instant,
    /// Notification message to display (message, expiry time)
    pub notification: Option<(String, Instant)>,
    /// Whether `notification` reports a failure (drawn in red)
    pub notification_is_error: bool,
    /// Names of containers with an action in flight, for its result toast
    pub(crate) action_names: HashMap<ContainerKey, String>,
    /// Containers marked with Space (or Ctrl+A) for a bulk action
    pub marked_containers: HashSet<ContainerKey>,
    /// Bulk actions still in flight, each reported as one summary toast.
    /// Several can overlap, e.g. Stop on some containers, then Remove on others.
    pub(crate) bulk_progress: Vec<BulkProgress>,
    /// The bulk action picked once and awaiting a confirming Enter (Remove)
    pub armed_bulk_action: Option<ContainerAction>,
    /// The containers the armed action was shown to hit; a confirming Enter
    /// only runs it if they haven't changed
    pub(crate) armed_bulk_targets: HashSet<ContainerKey>,
    /// Whether a reset confirmation is pending
    pub reset_confirm_pending: bool,
}

impl AppState {
    /// Creates a new AppState instance
    pub fn new(
        connected_hosts: HashMap<String, DockerHost>,
        event_tx: mpsc::Sender<AppEvent>,
        show_all: bool,
        sort_field: Column,
        sort_direction: Option<SortDirection>,
        column_config: ColumnConfig,
        config_path: Option<std::path::PathBuf>,
    ) -> Self {
        // Detect if running in SSH session
        let is_ssh_session = std::env::var("SSH_CLIENT").is_ok()
            || std::env::var("SSH_TTY").is_ok()
            || std::env::var("SSH_CONNECTION").is_ok();

        Self {
            containers: HashMap::new(),
            sorted_container_keys: Vec::new(),
            visible_columns_cache: Vec::new(),
            should_quit: false,
            table_state: TableState::default(),
            view_state: ViewState::ContainerList,
            log_state: None,
            is_at_bottom: true,
            last_viewport_height: 20, // Default to 20 lines (will be updated on first render)
            last_viewport_width: 80,  // Default width (will be updated on first render)
            last_list_viewport_height: 20, // Default visible rows (updated on first render)
            connected_hosts,
            event_tx,
            is_ssh_session,
            show_help: false,
            sort_state: SortState::new_with_direction(sort_field, sort_direction), // Use configured sort field and direction
            show_all_containers: show_all,
            action_menu_state: ListState::default(), // Default to no selection
            search_input: Input::default(),
            column_config,
            column_selector_state: ListState::default(),
            config_path,
            sort_selector_state: ListState::default(),
            connection_errors: HashMap::new(),
            disconnected_hosts: BTreeSet::new(),
            host_info: BTreeMap::new(),
            system_stats: None,
            remote_system_stats: BTreeMap::new(),
            show_system_header: true,
            header_visible: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            last_sort_time: Instant::now(),
            notification: None,
            notification_is_error: false,
            action_names: HashMap::new(),
            marked_containers: HashSet::new(),
            bulk_progress: Vec::new(),
            armed_bulk_action: None,
            armed_bulk_targets: HashSet::new(),
            reset_confirm_pending: false,
        }
    }

    /// Returns true if containers span more than one Docker host.
    ///
    /// This is used every frame to decide whether to show the "Host" column.
    /// It iterates keys with an early return instead of collecting into a
    /// `HashSet`, so it performs no heap allocation.
    pub fn has_multiple_hosts(&self) -> bool {
        let mut first: Option<&str> = None;
        for key in self.containers.keys() {
            match first {
                None => first = Some(key.host_id.as_str()),
                Some(h) if h != key.host_id.as_str() => return true,
                _ => {}
            }
        }
        false
    }

    /// Rebuilds the cached list of visible columns in place.
    ///
    /// Reuses the existing `Vec`'s capacity (`clear` + `extend`), so after the
    /// first frame this does not allocate even though it runs every frame.
    pub(crate) fn refresh_visible_columns(&mut self) {
        self.visible_columns_cache.clear();
        self.visible_columns_cache.extend(
            self.column_config
                .columns
                .iter()
                .filter(|(_, visible)| *visible)
                .map(|(col, _)| *col),
        );
    }

    /// Processes a single event and returns what action to take
    pub fn handle_event(&mut self, event: AppEvent) -> RenderAction {
        // Log stats and log lines at TRACE level since they're very frequent, everything else at DEBUG
        match &event {
            AppEvent::ContainerStat(_, _) => tracing::trace!("Handling stat update: {:?}", event),
            AppEvent::LogLine(_, _) => tracing::trace!("Handling log line: {:?}", event),
            _ => tracing::debug!("Handling event: {:?}", event),
        }

        // Container changes can empty or shrink an open action menu
        let changes_containers = matches!(
            event,
            AppEvent::InitialContainerList(..)
                | AppEvent::ContainerDestroyed(_)
                | AppEvent::ContainerStateChanged(..)
                | AppEvent::ContainerCreated(_)
        );
        let menu_action = if changes_containers {
            self.selected_menu_action()
        } else {
            None
        };
        let action = self.dispatch_event(event);
        if changes_containers {
            self.revalidate_action_menu(menu_action);
        }
        action
    }

    fn dispatch_event(&mut self, event: AppEvent) -> RenderAction {
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
            AppEvent::ContainerRestartCount(key, count) => {
                self.handle_container_restart_count(key, count)
            }
            AppEvent::Resize => RenderAction::Render,
            AppEvent::Quit => {
                self.should_quit = true;
                RenderAction::None
            }
            AppEvent::KeyInput(key_event) => self.handle_key_input(key_event),
            AppEvent::LogBatchPrepend(key, log_entries, has_more_history) => {
                self.handle_log_batch_prepend(key, log_entries, has_more_history)
            }
            AppEvent::LogLine(key, log_line) => self.handle_log_line(key, log_line),
            AppEvent::ActionInProgress(key, action) => self.handle_action_in_progress(key, action),
            AppEvent::ActionSuccess(key, action) => self.handle_action_success(key, action),
            AppEvent::ActionError(key, action, error) => {
                self.handle_action_error(key, action, error)
            }
            AppEvent::ConnectionError(host_id, error) => {
                self.handle_connection_error(host_id, error)
            }
            AppEvent::HostConnected(docker_host) => self.handle_host_connected(docker_host),
            AppEvent::HostDisconnected(host_id) => self.handle_host_disconnected(host_id),
            AppEvent::HostReconnected(host_id) => self.handle_host_reconnected(host_id),
            AppEvent::HostInfo(host_id, info) => self.handle_host_info(host_id, info),
            AppEvent::SystemStats(stats) => self.handle_system_stats(stats),
            AppEvent::RemoteSystemStats(host_id, stats) => {
                self.handle_remote_system_stats(host_id, stats)
            }
        }
    }

    /// Dispatches a keyboard input to the appropriate handler based on current view state.
    /// This ensures exactly one action per keypress, eliminating duplicate/conflicting events.
    fn handle_key_input(&mut self, key: crossterm::event::KeyEvent) -> RenderAction {
        use crossterm::event::{KeyCode, KeyModifiers};

        // Search mode: most keys go to search input handler
        if self.view_state == ViewState::SearchMode {
            return match key.code {
                KeyCode::Enter => self.handle_enter_pressed(),
                KeyCode::Esc => self.handle_cancel_action_menu(),
                KeyCode::Up => self.handle_select_previous(),
                KeyCode::Down => self.handle_select_next(),
                _ => self.handle_search_key_event(key),
            };
        }

        if self.view_state == ViewState::ColumnSelector {
            return self.handle_column_selector_key(key);
        }

        if self.view_state == ViewState::SortSelector {
            return self.handle_sort_selector_key(key);
        }

        // Ctrl modifiers
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('u') => match &self.view_state {
                    ViewState::ContainerList | ViewState::SearchMode => self.handle_page_up(),
                    _ => self.handle_scroll_page_up(),
                },
                KeyCode::Char('d') => match &self.view_state {
                    ViewState::ContainerList | ViewState::SearchMode => self.handle_page_down(),
                    _ => self.handle_scroll_page_down(),
                },
                KeyCode::Char('s') => self.handle_save_preferences(),
                KeyCode::Char('r') => self.handle_reset_preferences_prompt(),
                KeyCode::Char('a') => self.handle_toggle_mark_all(),
                _ => RenderAction::None,
            };
        }

        // Handle reset confirmation if pending
        if self.reset_confirm_pending {
            return match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.handle_reset_preferences_confirm(),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.reset_confirm_pending = false;
                    self.notification = None;
                    RenderAction::Render
                }
                _ => RenderAction::None,
            };
        }

        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                RenderAction::None
            }
            KeyCode::Char('/') => self.handle_enter_search_mode(),
            KeyCode::Char('?') => self.handle_toggle_help(),
            KeyCode::Up | KeyCode::Char('k') => match &self.view_state {
                ViewState::ContainerList => self.handle_select_previous(),
                ViewState::LogView(_) => self.handle_scroll_up(),
                ViewState::ActionMenu(_) | ViewState::BulkActionMenu => {
                    self.handle_select_action_up()
                }
                // SearchMode is handled by the early return above; fallback defensively
                ViewState::SearchMode => self.handle_select_previous(),
                // ColumnSelector/SortSelector handled by early returns above
                ViewState::ColumnSelector | ViewState::SortSelector => RenderAction::None,
            },
            KeyCode::Down | KeyCode::Char('j') => match &self.view_state {
                ViewState::ContainerList => self.handle_select_next(),
                ViewState::LogView(_) => self.handle_scroll_down(),
                ViewState::ActionMenu(_) | ViewState::BulkActionMenu => {
                    self.handle_select_action_down()
                }
                ViewState::SearchMode => self.handle_select_next(),
                // ColumnSelector/SortSelector handled by early returns above
                ViewState::ColumnSelector | ViewState::SortSelector => RenderAction::None,
            },
            KeyCode::PageUp => match &self.view_state {
                ViewState::ContainerList | ViewState::SearchMode => self.handle_page_up(),
                _ => self.handle_scroll_page_up(),
            },
            KeyCode::PageDown => match &self.view_state {
                ViewState::ContainerList | ViewState::SearchMode => self.handle_page_down(),
                _ => self.handle_scroll_page_down(),
            },
            KeyCode::Home => match &self.view_state {
                ViewState::ContainerList | ViewState::SearchMode => self.handle_select_first(),
                _ => self.handle_scroll_to_top(),
            },
            KeyCode::End => match &self.view_state {
                ViewState::ContainerList | ViewState::SearchMode => self.handle_select_last(),
                _ => self.handle_scroll_to_bottom(),
            },
            KeyCode::Enter => self.handle_enter_pressed(),
            KeyCode::Esc => self.handle_cancel_action_menu(),
            KeyCode::Char('o') => self.handle_open_dozzle(),
            KeyCode::Char('s') => self.handle_open_sort_selector(),
            KeyCode::Char('a') | KeyCode::Char('A') => self.handle_toggle_show_all(),
            KeyCode::Char('c') => self.handle_open_column_selector(),
            KeyCode::Char('t') => self.handle_toggle_system_header(),
            KeyCode::Right | KeyCode::Char('l') => self.handle_show_log_view(),
            KeyCode::Left | KeyCode::Char('h') => match &self.view_state {
                // The menu footers offer ← to cancel
                ViewState::ActionMenu(_) | ViewState::BulkActionMenu => {
                    self.handle_cancel_action_menu()
                }
                _ => self.handle_exit_log_view(),
            },
            KeyCode::Char('g') => self.handle_scroll_to_top(),
            KeyCode::Char('G') => self.handle_scroll_to_bottom(),
            KeyCode::Char(' ') => match &self.view_state {
                ViewState::ContainerList => self.handle_toggle_mark(),
                _ => self.handle_scroll_page_down(),
            },
            KeyCode::Char('b') => self.handle_scroll_page_up(),
            KeyCode::Char('p') => self.handle_toggle_mark_project(),
            _ => RenderAction::None,
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
        self.disconnected_hosts.remove(&host_id);

        RenderAction::None // No need to force redraw, container list will update via normal events
    }

    /// Marks a host as disconnected so the UI can show that it is being retried.
    ///
    /// The host's containers are left in place: they are stale, but wiping them
    /// would blank the list, and the next successful poll re-synchronizes them.
    fn handle_host_disconnected(&mut self, host_id: HostId) -> RenderAction {
        // Unlike the containers, whose rows the banner marks as stale, the
        // System line would keep showing a dead host's last sample as live.
        // The sampler brings it back once the host answers again.
        self.remote_system_stats.remove(&host_id);
        // Results from this host may never arrive; a bulk action waiting on
        // them would otherwise linger and swallow later toasts for its keys
        self.bulk_progress
            .retain(|p| !p.pending.iter().any(|key| key.host_id == host_id));
        self.disconnected_hosts.insert(host_id);
        RenderAction::Render // Force draw - show the reconnecting banner
    }

    /// Clears the disconnected marker once a host is reachable again.
    fn handle_host_reconnected(&mut self, host_id: HostId) -> RenderAction {
        self.disconnected_hosts.remove(&host_id);
        self.connection_errors.remove(&host_id);
        RenderAction::Render // Force draw - remove the reconnecting banner
    }
}
