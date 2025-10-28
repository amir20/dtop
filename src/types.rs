use tokio::sync::mpsc;

use crate::logs::LogEntry;

/// Host identifier for tracking which Docker host a container belongs to
pub type HostId = String;

/// Container metadata (static information)
#[derive(Clone, Debug)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub status: String,
    pub stats: ContainerStats,
    pub host_id: HostId,
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
