use crate::core::app_state::AppState;
use crate::core::types::{
    Container, ContainerKey, ContainerState, ContainerStats, HealthStatus, RenderAction,
};

impl AppState {
    pub(super) fn handle_initial_container_list(
        &mut self,
        host_id: String,
        container_list: Vec<Container>,
    ) -> RenderAction {
        // Remember *which container* is selected, not just its row. A resync resets
        // the host's stats to zero, so under a stats-based sort every row compares
        // equal and the list can come back in a different order — holding the row
        // index would silently move the cursor onto a different container, and the
        // next Enter would open the action menu on it.
        let selected_key = self
            .table_state
            .selected()
            .and_then(|index| self.sorted_container_keys.get(index))
            .cloned();

        // This is the authoritative list for the host. Drop anything we still hold
        // for it first, since the same event is re-sent to re-synchronize after the
        // host reconnects and containers may have been removed in the meantime.
        self.containers.retain(|key, _| key.host_id != host_id);

        for container in container_list {
            let key = ContainerKey::new(host_id.clone(), container.id.clone());
            self.containers.insert(key, container);
        }

        // Force immediate sort when loading the container list
        // (this also rebuilds `sorted_container_keys`)
        self.force_sort_containers();

        match selected_key.and_then(|key| self.index_of_container(&key)) {
            // The selected container is still listed — follow it to its new row.
            Some(index) => self.table_state.select(Some(index)),
            // It went away (or nothing was selected): fall back to the first row,
            // keeping any existing selection in range.
            None => {
                if self.table_state.selected().is_none() {
                    if !self.sorted_container_keys.is_empty() {
                        self.table_state.select(Some(0));
                    }
                } else {
                    self.clamp_selection();
                }
            }
        }

        RenderAction::Render // Force draw - table structure changed
    }

    /// Returns the row index of a container in the currently sorted list.
    fn index_of_container(&self, key: &ContainerKey) -> Option<usize> {
        self.sorted_container_keys.iter().position(|k| k == key)
    }

    pub(super) fn handle_container_created(&mut self, container: Container) -> RenderAction {
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        let is_new = !self.containers.contains_key(&key);
        self.containers.insert(key.clone(), container);

        // Only add to sorted keys if this is a genuinely new container
        // (avoid duplicates during restarts where container already exists)
        if is_new {
            self.sorted_container_keys.push(key);
        }

        // Force immediate sort when new container is added
        self.force_sort_containers();

        // Select first row if this is the first container
        if self.containers.len() == 1 {
            self.table_state.select(Some(0));
        }

        RenderAction::Render // Force draw - table structure changed
    }

    pub(super) fn handle_container_destroyed(&mut self, key: ContainerKey) -> RenderAction {
        self.containers.remove(&key);
        self.sorted_container_keys.retain(|k| k != &key);

        // Adjust selection if needed
        self.clamp_selection();

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
}
