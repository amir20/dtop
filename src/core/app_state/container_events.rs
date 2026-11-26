use crate::core::app_state::AppState;
use crate::core::types::{
    Container, ContainerKey, ContainerState, ContainerStats, HealthStatus, RenderAction,
};
use chrono::{DateTime, Utc};

impl AppState {
    pub(super) fn handle_initial_container_list(
        &mut self,
        host_id: String,
        container_list: Vec<Container>,
    ) -> RenderAction {
        // This is the authoritative list for the host. Drop anything we still hold
        // for it first, since the same event is re-sent to re-synchronize after the
        // host reconnects and containers may have been removed in the meantime.
        self.containers.retain(|key, _| key.host_id != host_id);

        for container in container_list {
            let key = ContainerKey::new(host_id.clone(), container.id.clone());
            self.containers.insert(key, container);
        }

        self.prune_marks();

        // Force immediate sort when loading the container list. This rebuilds
        // `sorted_container_keys` and keeps the cursor on the selected container:
        // a resync resets the host's stats to zero, so under a stats-based sort
        // the list can come back in a different order.
        self.force_sort_containers();
        self.ensure_selection();

        RenderAction::Render // Force draw - table structure changed
    }

    pub(super) fn handle_container_created(&mut self, container: Container) -> RenderAction {
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        self.containers.insert(key, container);

        // Force immediate sort when new container is added (rebuilds the keys)
        self.force_sort_containers();

        // The container may be filling an empty (possibly filtered) list, in
        // which case the selection was cleared when the list emptied out.
        self.ensure_selection();

        RenderAction::Render // Force draw - table structure changed
    }

    pub(super) fn handle_container_destroyed(&mut self, key: ContainerKey) -> RenderAction {
        self.containers.remove(&key);
        self.marked_containers.remove(&key);

        // Rebuilds the keys, keeping the cursor on the selected container when
        // it survives (a row above it going away would shift it)
        self.force_sort_containers();
        self.ensure_selection();

        RenderAction::Render // Force draw - table structure changed
    }

    pub(super) fn handle_container_state_changed(
        &mut self,
        key: ContainerKey,
        state: ContainerState,
    ) -> RenderAction {
        if let Some(container) = self.containers.get_mut(&key) {
            container.state = state;
            return RenderAction::Render; // Force draw - state changed
        }
        RenderAction::None
    }

    pub(super) fn handle_container_stat(
        &mut self,
        key: ContainerKey,
        stats: ContainerStats,
    ) -> RenderAction {
        if let Some(container) = self.containers.get_mut(&key) {
            container.stats = stats;
        }
        RenderAction::None // No force draw - just stats update
    }

    pub(super) fn handle_container_health_changed(
        &mut self,
        key: ContainerKey,
        health: HealthStatus,
    ) -> RenderAction {
        if let Some(container) = self.containers.get_mut(&key) {
            container.health = Some(health);
        }
        RenderAction::Render // Force draw - health status changed (visible in UI)
    }

    /// Applies a backfilled restart count and start time.
    ///
    /// These arrive one per container right after startup, so this deliberately
    /// does not force a draw: the periodic render picks the values up, and the
    /// sort throttle settles the ordering when sorting by `Restarts` or `Uptime`.
    pub(super) fn handle_container_inspected(
        &mut self,
        key: ContainerKey,
        restart_count: Option<i64>,
        started: Option<DateTime<Utc>>,
    ) -> RenderAction {
        if let Some(container) = self.containers.get_mut(&key) {
            if restart_count.is_some() {
                container.restart_count = restart_count;
            }
            if started.is_some() {
                container.started = started;
            }
        }
        RenderAction::None
    }
}
