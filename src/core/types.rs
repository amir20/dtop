use chrono::{DateTime, Utc};
use std::str::FromStr;
use tokio::sync::mpsc;

use crate::docker::logs::LogEntry;

/// Host identifier for tracking which Docker host a container belongs to
pub type HostId = String;

/// Container state as reported by Docker
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContainerState {
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Created,
    Unknown,
}

/// Container health status from Docker health checks
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Starting,
}

impl FromStr for ContainerState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_lower = s.to_lowercase();
        let state = if s_lower.contains("running") {
            ContainerState::Running
        } else if s_lower.contains("paused") {
            ContainerState::Paused
        } else if s_lower.contains("restarting") {
            ContainerState::Restarting
        } else if s_lower.contains("removing") {
            ContainerState::Removing
        } else if s_lower.contains("exited") {
            ContainerState::Exited
        } else if s_lower.contains("dead") {
            ContainerState::Dead
        } else if s_lower.contains("created") {
            ContainerState::Created
        } else {
            ContainerState::Unknown
        };
        Ok(state)
    }
}

impl FromStr for HealthStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_lower = s.to_lowercase();
        if s_lower.contains("healthy") && !s_lower.contains("unhealthy") {
            Ok(HealthStatus::Healthy)
        } else if s_lower.contains("unhealthy") {
            Ok(HealthStatus::Unhealthy)
        } else if s_lower.contains("starting") {
            Ok(HealthStatus::Starting)
        } else {
            Err(()) // Return error for unknown/no health status
        }
    }
}

/// Container metadata (static information)
#[derive(Clone, Debug)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub state: ContainerState,
    pub health: Option<HealthStatus>, // None if container has no health check configured
    pub created: Option<DateTime<Utc>>, // When the container was created
    pub stats: ContainerStats,
    pub host_id: HostId,
    pub dozzle_url: Option<String>,
}

/// Container runtime statistics (updated frequently)
#[derive(Clone, Debug, Default)]
pub struct ContainerStats {
    pub cpu: f64,
    pub memory: f64,
    /// Memory used in bytes
    pub memory_used_bytes: u64,
    /// Memory limit in bytes
    pub memory_limit_bytes: u64,
    /// Network transmit rate in bytes per second
    pub network_tx_bytes_per_sec: f64,
    /// Network receive rate in bytes per second
    pub network_rx_bytes_per_sec: f64,
}

/// Unique key for identifying containers across multiple hosts
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ContainerKey {
    pub host_id: HostId,
    pub container_id: String,
}

impl ContainerKey {
    pub fn new(host_id: HostId, container_id: String) -> Self {
        Self {
            host_id,
            container_id,
        }
    }
}

#[derive(Debug)]
pub enum AppEvent {
    /// Initial list of containers when app starts for a specific host
    InitialContainerList(HostId, Vec<Container>),
    /// A new container was created/started (host_id is in the Container)
    ContainerCreated(Container),
    /// A container was stopped/destroyed on a specific host
    ContainerDestroyed(ContainerKey),
    /// A container's state changed (e.g., from Running to Exited)
    ContainerStateChanged(ContainerKey, ContainerState),
    /// Stats update for an existing container on a specific host
    ContainerStat(ContainerKey, ContainerStats),
    /// Health status changed for a container
    ContainerHealthChanged(ContainerKey, HealthStatus),
    /// User requested to quit
    Quit,
    /// Terminal was resized
    Resize,
    /// Move selection up
    SelectPrevious,
    /// Move selection down
    SelectNext,
    /// User pressed Enter key
    EnterPressed,
    /// User pressed Escape to exit log view
    ExitLogView,
    /// User pressed right arrow to show log view
    ShowLogView,
    /// User scrolled up in log view
    ScrollUp,
    /// User scrolled down in log view
    ScrollDown,
    /// User scrolled to top of log view (g)
    ScrollToTop,
    /// User scrolled to bottom of log view (G)
    ScrollToBottom,
    /// User scrolled page up in log view (Ctrl+U, b)
    ScrollPageUp,
    /// User scrolled page down in log view (Ctrl+D, Space)
    ScrollPageDown,
    /// Batch of historical log lines loaded at once (no UI updates per line)
    /// DEPRECATED: Use LogBatchPrepend instead
    #[allow(dead_code)]
    LogBatch(ContainerKey, Vec<LogEntry>),
    /// Batch of historical logs to prepend (initial load AND pagination)
    /// bool indicates if there are more historical logs available before this batch
    LogBatchPrepend(ContainerKey, Vec<LogEntry>, bool),
    /// Request to fetch older logs (user scrolled to top)
    /// Generated internally, not constructed directly
    #[allow(dead_code)]
    RequestOlderLogs,
    /// New log line received from streaming logs
    LogLine(ContainerKey, LogEntry),
    /// User pressed 'o' to open Dozzle
    OpenDozzle,
    /// User pressed '?' to toggle help
    ToggleHelp,
    /// User pressed 's' to cycle sort field
    CycleSortField,
    /// User pressed a key to set a specific sort field
    SetSortField(SortField),
    /// User pressed 'a' to toggle showing all containers (including stopped)
    ToggleShowAll,
    /// User pressed left arrow or Esc to cancel action menu
    CancelActionMenu,
    /// Navigate up in action menu
    SelectActionUp,
    /// Navigate down in action menu
    SelectActionDown,
    /// Action is in progress
    ActionInProgress(ContainerKey, ContainerAction),
    /// Action completed successfully
    ActionSuccess(ContainerKey, ContainerAction),
    /// Action failed with error
    ActionError(ContainerKey, ContainerAction, String),
    /// User pressed '/' to enter search mode
    EnterSearchMode,
    /// Key event for search input (passed to tui-input)
    SearchKeyEvent(crossterm::event::KeyEvent),
    /// Connection to a Docker host failed
    ConnectionError(HostId, String),
    /// A new Docker host has successfully connected
    HostConnected(crate::docker::connection::DockerHost),
}

pub type EventSender = mpsc::Sender<AppEvent>;

/// Action to take after processing an event
#[derive(Clone, Debug, PartialEq)]
pub enum RenderAction {
    /// Don't render
    None,
    /// Normal render
    Render,
    /// Start a shell session for a container
    StartShell(ContainerKey),
}

/// Current view state of the application
#[derive(Clone, Debug, PartialEq)]
pub enum ViewState {
    /// Viewing the container list
    ContainerList,
    /// Viewing logs for a specific container
    LogView(ContainerKey),
    /// Viewing action menu for a specific container
    ActionMenu(ContainerKey),
    /// Search mode active (editing search query)
    SearchMode,
}

/// Available actions for containers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Remove,
    Shell,
}

impl ContainerAction {
    /// Returns the display name for this action
    pub fn display_name(self) -> &'static str {
        match self {
            ContainerAction::Start => "Start",
            ContainerAction::Stop => "Stop",
            ContainerAction::Restart => "Restart",
            ContainerAction::Remove => "Remove",
            ContainerAction::Shell => "Shell",
        }
    }

    /// Returns all available actions for a given container state
    pub fn available_for_state(state: &ContainerState) -> Vec<ContainerAction> {
        match state {
            ContainerState::Running => vec![
                ContainerAction::Shell,
                ContainerAction::Stop,
                ContainerAction::Restart,
                ContainerAction::Remove,
            ],
            ContainerState::Paused => vec![ContainerAction::Stop, ContainerAction::Remove],
            ContainerState::Exited | ContainerState::Created | ContainerState::Dead => {
                vec![ContainerAction::Start, ContainerAction::Remove]
            }
            ContainerState::Restarting | ContainerState::Removing => vec![],
            ContainerState::Unknown => vec![],
        }
    }
}

/// Sort direction
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    /// Toggles the sort direction
    pub fn toggle(self) -> Self {
        match self {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        }
    }

    /// Returns the display symbol for this direction
    pub fn symbol(self) -> &'static str {
        match self {
            SortDirection::Ascending => "▲",
            SortDirection::Descending => "▼",
        }
    }
}

/// Combined sort state (field + direction)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SortState {
    pub field: SortField,
    pub direction: SortDirection,
}

impl SortState {
    /// Creates a new SortState with the default direction for the field
    pub fn new(field: SortField) -> Self {
        Self {
            field,
            direction: field.default_direction(),
        }
    }
}

impl Default for SortState {
    fn default() -> Self {
        Self::new(SortField::Uptime)
    }
}

/// Sort field for container list
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortField {
    /// Sort by creation time
    Uptime,
    /// Sort by container name
    Name,
    /// Sort by CPU usage
    Cpu,
    /// Sort by memory usage
    Memory,
}

impl SortField {
    /// Cycles to the next sort field
    pub fn next(self) -> Self {
        match self {
            SortField::Uptime => SortField::Name,
            SortField::Name => SortField::Cpu,
            SortField::Cpu => SortField::Memory,
            SortField::Memory => SortField::Uptime,
        }
    }

    /// Returns the default sort direction for this field
    pub fn default_direction(self) -> SortDirection {
        match self {
            SortField::Name => SortDirection::Ascending,
            SortField::Uptime => SortDirection::Descending, // Newest first
            SortField::Cpu => SortDirection::Descending,    // Highest first
            SortField::Memory => SortDirection::Descending, // Highest first
        }
    }
}

/// Log state for the currently viewed container
#[derive(Debug)]
pub struct LogState {
    /// Which container these logs are for
    pub container_key: ContainerKey,

    /// Raw log entries with timestamps (used for progress calculation and formatting)
    pub log_entries: Vec<crate::docker::logs::LogEntry>,

    /// Current scroll offset (line number from top)
    pub scroll_offset: usize,

    /// Handle to the log streaming task (for cancellation)
    pub stream_handle: Option<tokio::task::JoinHandle<()>>,

    /// Timestamp of the oldest log currently loaded (for pagination cursor)
    pub oldest_timestamp: Option<DateTime<Utc>>,

    /// Timestamp of the newest log (for progress bar calculation)
    pub newest_timestamp: Option<DateTime<Utc>>,

    /// Whether there are more logs to fetch before oldest_timestamp
    pub has_more_history: bool,

    /// Total number of logs loaded so far
    pub total_loaded: usize,

    /// Timestamp when the container was created (for progress bar calculation)
    pub container_created_at: Option<DateTime<Utc>>,

    /// Track if we're currently fetching older logs (prevent duplicate requests)
    pub fetching_older: bool,
}

impl LogState {
    /// Create a new LogState for a container
    pub fn new(container_key: ContainerKey, container_created_at: Option<DateTime<Utc>>) -> Self {
        Self {
            container_key,
            log_entries: Vec::new(),
            scroll_offset: 0,
            stream_handle: None,
            oldest_timestamp: None,
            newest_timestamp: None,
            has_more_history: false,
            total_loaded: 0,
            container_created_at,
            fetching_older: false,
        }
    }

    /// Calculate what percentage of log history the current visible page represents
    /// 0% = viewing logs from container creation time (top), 100% = viewing current/newest logs (bottom)
    /// Returns None if we can't calculate (missing timestamps)
    pub fn calculate_progress(&self, visible_line_index: usize) -> Option<f64> {
        let container_created = self.container_created_at?;
        let newest_loaded = self.newest_timestamp?;

        // Get the timestamp of the currently visible log entry
        let visible_timestamp = if visible_line_index < self.log_entries.len() {
            self.log_entries[visible_line_index].timestamp
        } else if !self.log_entries.is_empty() {
            // If index is out of range, use the last entry
            self.log_entries.last()?.timestamp
        } else {
            return None;
        };

        // Calculate time range from container creation to newest log
        let total_duration = (newest_loaded - container_created).num_seconds() as f64;

        // Avoid division by zero
        if total_duration <= 0.0 {
            return Some(100.0);
        }

        // Calculate how far the visible timestamp is from container creation
        let visible_offset = (visible_timestamp - container_created).num_seconds() as f64;

        // Percentage: how far through the log history we are
        // 0% = at container creation (visible_timestamp = container_created)
        // 100% = at newest logs (visible_timestamp = newest_loaded)
        let percentage = (visible_offset / total_duration) * 100.0;

        Some(percentage.clamp(0.0, 100.0))
    }
}
