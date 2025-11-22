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
        for container in container_list {
            let key = ContainerKey::new(host_id.clone(), container.id.clone());
            self.containers.insert(key.clone(), container);
            self.sorted_container_keys.push(key);
        }

        // Sort using current sort field
        self.sort_containers();

        // Select first row if we have containers
        if !self.containers.is_empty() {
            self.table_state.select(Some(0));
        }

        RenderAction::Render // Force draw - table structure changed
    }

    pub(super) fn handle_container_created(&mut self, container: Container) -> RenderAction {
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        self.containers.insert(key.clone(), container);
        self.sorted_container_keys.push(key);

        // Re-sort the entire list with current sort field
        self.sort_containers();

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
        let container_count = self.containers.len();
        if container_count == 0 {
            self.table_state.select(None);
        } else if let Some(selected) = self.table_state.selected()
            && selected >= container_count
        {
            self.table_state.select(Some(container_count - 1));
        }

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
