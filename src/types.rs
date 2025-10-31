use chrono::{DateTime, Utc};
use std::str::FromStr;
use tokio::sync::mpsc;

use crate::logs::LogEntry;

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

pub enum AppEvent {
    /// Initial list of containers when app starts for a specific host
    InitialContainerList(HostId, Vec<Container>),
    /// A new container was created/started (host_id is in the Container)
    ContainerCreated(Container),
    /// A container was stopped/destroyed on a specific host
    ContainerDestroyed(ContainerKey),
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
    /// User scrolled up in log view
    ScrollUp,
    /// User scrolled down in log view
    ScrollDown,
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
}

pub type EventSender = mpsc::Sender<AppEvent>;

/// Current view state of the application
#[derive(Clone, Debug, PartialEq)]
pub enum ViewState {
    /// Viewing the container list
    ContainerList,
    /// Viewing logs for a specific container
    LogView(ContainerKey),
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
    /// Sort by creation time (uptime)
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
