//! State behind the system stats header: host capacity, machine metrics, and
//! the header's visibility.

use crate::core::app_state::AppState;
use crate::core::types::{ContainerState, HostId, HostInfo, RenderAction, SystemStats};

/// Usage summed over one host's containers.
#[derive(Debug, Default, PartialEq)]
pub struct DockerTotals {
    /// Sum of container CPU percentages, where 100% is one core
    pub cpu: f64,
    pub memory_used_bytes: u64,
    pub running: usize,
    pub total: usize,
}

impl DockerTotals {
    /// Container CPU scaled to the host's capacity, 0-100
    pub fn cpu_percent(&self, info: &HostInfo) -> f64 {
        self.cpu / f64::from(info.cpus.max(1))
    }
}

impl AppState {
    /// Sums the usage of `host_id`'s containers.
    pub fn docker_totals(&self, host_id: &str) -> DockerTotals {
        let mut totals = DockerTotals::default();
        for container in self.containers.values() {
            if container.host_id != host_id {
                continue;
            }
            totals.total += 1;
            // Stopped containers can keep their last stats sample; don't count it
            if container.state == ContainerState::Running {
                totals.running += 1;
                totals.cpu += container.stats.cpu;
                totals.memory_used_bytes += container.stats.memory_used_bytes;
            }
        }
        totals
    }

    pub(super) fn handle_host_info(&mut self, host_id: HostId, info: HostInfo) -> RenderAction {
        self.host_info.insert(host_id, info);
        RenderAction::Render
    }

    pub(super) fn handle_system_stats(&mut self, stats: SystemStats) -> RenderAction {
        self.system_stats = Some(stats);
        // Picked up by the next periodic draw, like container stats
        RenderAction::None
    }

    pub(super) fn handle_remote_system_stats(
        &mut self,
        host_id: HostId,
        stats: SystemStats,
    ) -> RenderAction {
        self.remote_system_stats.insert(host_id, stats);
        RenderAction::None
    }

    /// Shows or hides the header, pausing or resuming the samplers with it.
    ///
    /// Hiding drops the machine stats: the samplers stop updating them, so on
    /// re-show they'd be presented as live however old they were. The lines
    /// come back with the first fresh samples.
    pub fn set_show_system_header(&mut self, show: bool) {
        if !show {
            self.system_stats = None;
            self.remote_system_stats.clear();
        }
        self.show_system_header = show;
        self.header_visible
            .store(show, std::sync::atomic::Ordering::Relaxed);
    }

    /// Handles 't': show or hide the system stats header
    pub(super) fn handle_toggle_system_header(&mut self) -> RenderAction {
        self.set_show_system_header(!self.show_system_header);
        RenderAction::Render
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Column, ColumnConfig, Container, ContainerStats};
    use std::collections::HashMap;
    use tokio::sync::mpsc;

    fn state() -> AppState {
        let (tx, _rx) = mpsc::channel(10);
        AppState::new(
            HashMap::new(),
            tx,
            false,
            Column::Uptime,
            None,
            ColumnConfig::default(),
            None,
        )
    }

    fn container(id: &str, host: &str, state: ContainerState, cpu: f64) -> Container {
        Container {
            id: id.to_string(),
            name: id.to_string(),
            state,
            health: None,
            created: None,
            stats: ContainerStats {
                cpu,
                memory_used_bytes: 100,
                network_rx_bytes_per_sec: 10.0,
                network_tx_bytes_per_sec: 20.0,
                ..Default::default()
            },
            host_id: host.to_string(),
            dozzle_url: None,
            restart_count: None,
            compose_project: None,
            image: None,
        }
    }

    /// A disconnected host's System line must not keep showing its last sample.
    #[test]
    fn disconnect_drops_the_hosts_machine_stats() {
        use crate::core::types::AppEvent;

        let mut state = state();
        let stats = SystemStats {
            hostname: "h".to_string(),
            ..Default::default()
        };
        state.handle_event(AppEvent::RemoteSystemStats("h".to_string(), stats.clone()));
        state.handle_event(AppEvent::RemoteSystemStats("other".to_string(), stats));

        state.handle_event(AppEvent::HostDisconnected("h".to_string()));

        assert!(!state.remote_system_stats.contains_key("h"));
        assert!(state.remote_system_stats.contains_key("other"));
    }

    #[test]
    fn docker_totals_count_only_running_containers_of_the_host() {
        let mut state = state();
        for c in [
            container("a", "local", ContainerState::Running, 150.0),
            container("b", "local", ContainerState::Running, 50.0),
            container("c", "local", ContainerState::Exited, 99.0),
            container("d", "other", ContainerState::Running, 70.0),
        ] {
            state.handle_container_created(c);
        }

        let totals = state.docker_totals("local");

        assert_eq!(
            totals,
            DockerTotals {
                cpu: 200.0,
                memory_used_bytes: 200,
                running: 2,
                total: 3,
            }
        );
        let info = HostInfo {
            cpus: 8,
            memory_total_bytes: 0,
        };
        assert_eq!(totals.cpu_percent(&info), 25.0);
    }
}
