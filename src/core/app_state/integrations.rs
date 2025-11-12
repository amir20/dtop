use crate::core::app_state::AppState;
use crate::core::types::ViewState;

impl AppState {
    pub(super) fn handle_open_dozzle(&mut self) -> bool {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return false;
        }

        // Don't open URLs in SSH sessions
        if self.is_ssh_session {
            return false;
        }

        // Get the selected container
        let Some(selected_idx) = self.table_state.selected() else {
            return false;
        };

        let Some(container_key) = self.sorted_container_keys.get(selected_idx) else {
            return false;
        };

        // Get the container and its Dozzle URL
        let Some(container) = self.containers.get(container_key) else {
            return false;
        };

        let Some(dozzle_url) = &container.dozzle_url else {
            return false;
        };

        // Build the full URL: {dozzle}/container/{containerId}
        let full_url = format!(
            "{}/container/{}",
            dozzle_url.trim_end_matches('/'),
            container_key.container_id
        );

        // Open the URL using the 'open' crate (cross-platform)
        let _ = open::that(&full_url);

        false // No need to force draw
    }
}
