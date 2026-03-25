use chrono::{DateTime, Utc};
use ratatui::text::Line;
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
        let state = match s.to_lowercase().as_str() {
            "running" => ContainerState::Running,
            "paused" => ContainerState::Paused,
            "restarting" => ContainerState::Restarting,
            "removing" => ContainerState::Removing,
            "exited" => ContainerState::Exited,
            "dead" => ContainerState::Dead,
            "created" => ContainerState::Created,
            _ => ContainerState::Unknown,
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
    /// A keyboard input event - dispatched by AppState based on view state
    KeyInput(crossterm::event::KeyEvent),
    /// Batch of historical logs to prepend (initial load AND pagination)
    /// bool indicates if there are more historical logs available before this batch
    LogBatchPrepend(ContainerKey, Vec<LogEntry>, bool),
    /// New log line received from streaming logs
    LogLine(ContainerKey, LogEntry),
    /// Action is in progress
    ActionInProgress(ContainerKey, ContainerAction),
    /// Action completed successfully
    ActionSuccess(ContainerKey, ContainerAction),
    /// Action failed with error
    ActionError(ContainerKey, ContainerAction, String),
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

impl std::str::FromStr for SortField {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "uptime" | "u" => Ok(SortField::Uptime),
            "name" | "n" => Ok(SortField::Name),
            "cpu" | "c" => Ok(SortField::Cpu),
            "memory" | "mem" | "m" => Ok(SortField::Memory),
            _ => Err(format!(
                "Invalid sort field '{}'. Valid options: uptime, name, cpu, memory",
                s
            )),
        }
    }
}

impl std::fmt::Display for SortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortField::Uptime => write!(f, "uptime"),
            SortField::Name => write!(f, "name"),
            SortField::Cpu => write!(f, "cpu"),
            SortField::Memory => write!(f, "memory"),
        }
    }
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

    /// Raw log entries with timestamps (used for progress calculation)
    pub log_entries: Vec<crate::docker::logs::LogEntry>,

    /// Pre-formatted lines for rendering (cached to avoid reformatting every frame)
    pub formatted_lines: Vec<Line<'static>>,

    /// Current scroll offset in visual lines (not entry count)
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
            formatted_lines: Vec::new(),
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

    /// Set log entries and rebuild the formatted lines cache.
    /// Used in tests and when bulk-replacing entries.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_entries(&mut self, entries: Vec<crate::docker::logs::LogEntry>) {
        self.formatted_lines = entries.iter().map(|e| e.format()).collect();
        self.log_entries = entries;
    }

    /// Calculate what percentage of log history the current visible page represents.
    /// Takes the entry index of the topmost visible log entry.
    /// 0% = viewing logs from container creation time (top), 100% = viewing current/newest logs (bottom)
    /// Returns None if we can't calculate (missing timestamps)
    pub fn calculate_progress(&self, visible_entry_index: usize) -> Option<f64> {
        let container_created = self.container_created_at?;
        let newest_loaded = self.newest_timestamp?;

        // Get the timestamp of the currently visible log entry
        let visible_timestamp = if visible_entry_index < self.log_entries.len() {
            self.log_entries[visible_entry_index].timestamp
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

/// Available columns in the container list
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Column {
    Status,
    Name,
    Id,
    Host,
    Cpu,
    Memory,
    NetTx,
    NetRx,
    Uptime,
}

impl Column {
    pub fn label(self) -> &'static str {
        match self {
            Column::Status => "Status Icon",
            Column::Name => "Name",
            Column::Id => "ID",
            Column::Host => "Host",
            Column::Cpu => "CPU %",
            Column::Memory => "Memory %",
            Column::NetTx => "Net TX",
            Column::NetRx => "Net RX",
            Column::Uptime => "Uptime",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Column::Status => "status",
            Column::Name => "name",
            Column::Id => "id",
            Column::Host => "host",
            Column::Cpu => "cpu",
            Column::Memory => "memory",
            Column::NetTx => "net_tx",
            Column::NetRx => "net_rx",
            Column::Uptime => "uptime",
        }
    }

    pub fn from_id(id: &str) -> Option<Column> {
        match id {
            "status" => Some(Column::Status),
            "name" => Some(Column::Name),
            "id" => Some(Column::Id),
            "host" => Some(Column::Host),
            "cpu" => Some(Column::Cpu),
            "memory" => Some(Column::Memory),
            "net_tx" => Some(Column::NetTx),
            "net_rx" => Some(Column::NetRx),
            "uptime" => Some(Column::Uptime),
            _ => None,
        }
    }

    pub fn all_default() -> Vec<Column> {
        vec![
            Column::Status, Column::Name, Column::Id, Column::Host,
            Column::Cpu, Column::Memory, Column::NetTx, Column::NetRx, Column::Uptime,
        ]
    }
}

#[derive(Clone, Debug)]
pub struct ColumnConfig {
    pub columns: Vec<(Column, bool)>,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            columns: Column::all_default().into_iter().map(|c| (c, true)).collect(),
        }
    }
}

impl ColumnConfig {
    pub fn visible_columns(&self) -> Vec<Column> {
        self.columns.iter().filter(|(_, visible)| *visible).map(|(col, _)| *col).collect()
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some((col, visible)) = self.columns.get_mut(index) {
            if *col != Column::Name {
                *visible = !*visible;
            }
        }
    }

    pub fn move_up(&mut self, index: usize) {
        if index > 0 && index < self.columns.len() {
            self.columns.swap(index, index - 1);
        }
    }

    pub fn move_down(&mut self, index: usize) {
        if index + 1 < self.columns.len() {
            self.columns.swap(index, index + 1);
        }
    }

    pub fn has_changed(&self, other: &ColumnConfig) -> bool {
        self.columns != other.columns
    }

    pub fn from_config_strings(strings: &[String]) -> Self {
        let mut result: Vec<(Column, bool)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for s in strings {
            if let Some(col) = Column::from_id(s) {
                if seen.insert(col) {
                    result.push((col, true));
                }
            }
        }
        for col in Column::all_default() {
            if !seen.contains(&col) {
                result.push((col, false));
            }
        }
        Self { columns: result }
    }

    pub fn to_config_strings(&self) -> Vec<String> {
        self.columns.iter().filter(|(_, visible)| *visible).map(|(col, _)| col.id().to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_field_from_str_full_names() {
        assert_eq!("uptime".parse::<SortField>().unwrap(), SortField::Uptime);
        assert_eq!("name".parse::<SortField>().unwrap(), SortField::Name);
        assert_eq!("cpu".parse::<SortField>().unwrap(), SortField::Cpu);
        assert_eq!("memory".parse::<SortField>().unwrap(), SortField::Memory);
    }

    #[test]
    fn test_sort_field_from_str_short_names() {
        assert_eq!("u".parse::<SortField>().unwrap(), SortField::Uptime);
        assert_eq!("n".parse::<SortField>().unwrap(), SortField::Name);
        assert_eq!("c".parse::<SortField>().unwrap(), SortField::Cpu);
        assert_eq!("m".parse::<SortField>().unwrap(), SortField::Memory);
    }

    #[test]
    fn test_sort_field_from_str_case_insensitive() {
        assert_eq!("UPTIME".parse::<SortField>().unwrap(), SortField::Uptime);
        assert_eq!("Name".parse::<SortField>().unwrap(), SortField::Name);
        assert_eq!("CPU".parse::<SortField>().unwrap(), SortField::Cpu);
        assert_eq!("Memory".parse::<SortField>().unwrap(), SortField::Memory);
        assert_eq!("MEM".parse::<SortField>().unwrap(), SortField::Memory);
    }

    #[test]
    fn test_sort_field_from_str_invalid() {
        assert!("invalid".parse::<SortField>().is_err());
        assert!("".parse::<SortField>().is_err());
        assert!("x".parse::<SortField>().is_err());
    }

    #[test]
    fn test_sort_field_display() {
        assert_eq!(SortField::Uptime.to_string(), "uptime");
        assert_eq!(SortField::Name.to_string(), "name");
        assert_eq!(SortField::Cpu.to_string(), "cpu");
        assert_eq!(SortField::Memory.to_string(), "memory");
    }

    #[test]
    fn test_sort_field_default_direction() {
        assert_eq!(
            SortField::Uptime.default_direction(),
            SortDirection::Descending
        );
        assert_eq!(
            SortField::Name.default_direction(),
            SortDirection::Ascending
        );
        assert_eq!(
            SortField::Cpu.default_direction(),
            SortDirection::Descending
        );
        assert_eq!(
            SortField::Memory.default_direction(),
            SortDirection::Descending
        );
    }

    #[test]
    fn test_sort_state_new() {
        let state = SortState::new(SortField::Name);
        assert_eq!(state.field, SortField::Name);
        assert_eq!(state.direction, SortDirection::Ascending);

        let state = SortState::new(SortField::Cpu);
        assert_eq!(state.field, SortField::Cpu);
        assert_eq!(state.direction, SortDirection::Descending);
    }

    #[test]
    fn test_column_label() {
        assert_eq!(Column::Status.label(), "Status Icon");
        assert_eq!(Column::Name.label(), "Name");
        assert_eq!(Column::Id.label(), "ID");
        assert_eq!(Column::Host.label(), "Host");
        assert_eq!(Column::Cpu.label(), "CPU %");
        assert_eq!(Column::Memory.label(), "Memory %");
        assert_eq!(Column::NetTx.label(), "Net TX");
        assert_eq!(Column::NetRx.label(), "Net RX");
        assert_eq!(Column::Uptime.label(), "Uptime");
    }

    #[test]
    fn test_column_config_default_all_visible() {
        let config = ColumnConfig::default();
        assert_eq!(config.columns.len(), 9);
        assert!(config.columns.iter().all(|(_, visible)| *visible));
    }

    #[test]
    fn test_column_config_visible_columns() {
        let mut config = ColumnConfig::default();
        config.columns[2] = (Column::Id, false);
        let visible = config.visible_columns();
        assert!(!visible.contains(&Column::Id));
        assert_eq!(visible.len(), 8);
    }

    #[test]
    fn test_column_config_toggle() {
        let mut config = ColumnConfig::default();
        let id_idx = config.columns.iter().position(|(c, _)| *c == Column::Id).unwrap();
        config.toggle(id_idx);
        assert!(!config.columns[id_idx].1);
        config.toggle(id_idx);
        assert!(config.columns[id_idx].1);
    }

    #[test]
    fn test_column_config_toggle_name_is_noop() {
        let mut config = ColumnConfig::default();
        let name_idx = config.columns.iter().position(|(c, _)| *c == Column::Name).unwrap();
        config.toggle(name_idx);
        assert!(config.columns[name_idx].1);
    }

    #[test]
    fn test_column_config_move_up() {
        let mut config = ColumnConfig::default();
        config.move_up(2);
        assert_eq!(config.columns[1].0, Column::Id);
        assert_eq!(config.columns[2].0, Column::Name);
    }

    #[test]
    fn test_column_config_move_up_at_zero_is_noop() {
        let mut config = ColumnConfig::default();
        let first = config.columns[0].0;
        config.move_up(0);
        assert_eq!(config.columns[0].0, first);
    }

    #[test]
    fn test_column_config_move_down() {
        let mut config = ColumnConfig::default();
        let col_at_0 = config.columns[0].0;
        config.move_down(0);
        assert_eq!(config.columns[1].0, col_at_0);
    }

    #[test]
    fn test_column_config_move_down_at_end_is_noop() {
        let mut config = ColumnConfig::default();
        let last_idx = config.columns.len() - 1;
        let last = config.columns[last_idx].0;
        config.move_down(last_idx);
        assert_eq!(config.columns[last_idx].0, last);
    }

    #[test]
    fn test_column_config_has_changed() {
        let config1 = ColumnConfig::default();
        let mut config2 = ColumnConfig::default();
        assert!(!config1.has_changed(&config2));
        let id_idx = config2.columns.iter().position(|(c, _)| *c == Column::Id).unwrap();
        config2.toggle(id_idx);
        assert!(config1.has_changed(&config2));
    }

    #[test]
    fn test_column_config_from_config_strings() {
        let strings = vec!["status".to_string(), "name".to_string(), "cpu".to_string()];
        let config = ColumnConfig::from_config_strings(&strings);
        let visible = config.visible_columns();
        assert_eq!(visible, vec![Column::Status, Column::Name, Column::Cpu]);
        assert_eq!(config.columns.len(), 9);
    }

    #[test]
    fn test_column_config_to_config_strings() {
        let mut config = ColumnConfig::default();
        let id_idx = config.columns.iter().position(|(c, _)| *c == Column::Id).unwrap();
        config.toggle(id_idx);
        let strings = config.to_config_strings();
        assert!(!strings.contains(&"id".to_string()));
        assert!(strings.contains(&"name".to_string()));
    }

    #[test]
    fn test_column_config_id() {
        assert_eq!(Column::Status.id(), "status");
        assert_eq!(Column::Name.id(), "name");
        assert_eq!(Column::Id.id(), "id");
        assert_eq!(Column::Host.id(), "host");
        assert_eq!(Column::Cpu.id(), "cpu");
        assert_eq!(Column::Memory.id(), "memory");
        assert_eq!(Column::NetTx.id(), "net_tx");
        assert_eq!(Column::NetRx.id(), "net_rx");
        assert_eq!(Column::Uptime.id(), "uptime");
    }

    #[test]
    fn test_column_from_id() {
        assert_eq!(Column::from_id("status"), Some(Column::Status));
        assert_eq!(Column::from_id("name"), Some(Column::Name));
        assert_eq!(Column::from_id("id"), Some(Column::Id));
        assert_eq!(Column::from_id("host"), Some(Column::Host));
        assert_eq!(Column::from_id("cpu"), Some(Column::Cpu));
        assert_eq!(Column::from_id("memory"), Some(Column::Memory));
        assert_eq!(Column::from_id("net_tx"), Some(Column::NetTx));
        assert_eq!(Column::from_id("net_rx"), Some(Column::NetRx));
        assert_eq!(Column::from_id("uptime"), Some(Column::Uptime));
        assert_eq!(Column::from_id("invalid"), None);
    }
}
