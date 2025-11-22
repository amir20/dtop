use crate::core::app_state::AppState;
use crate::core::types::{RenderAction, ViewState};

impl AppState {
    pub(super) fn handle_open_dozzle(&mut self) -> RenderAction {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // Don't open URLs in SSH sessions
        if self.is_ssh_session {
            return RenderAction::None;
        }

        // Get the selected container
        let Some(selected_idx) = self.table_state.selected() else {
            return RenderAction::None;
        };

        let Some(container_key) = self.sorted_container_keys.get(selected_idx) else {
            return RenderAction::None;
        };

        // Get the container and its Dozzle URL
        let Some(container) = self.containers.get(container_key) else {
            return RenderAction::None;
        };

        let Some(dozzle_url) = &container.dozzle_url else {
            return RenderAction::None;
        };

        // Build the full URL: {dozzle}/container/{containerId}
        let full_url = format!(
            "{}/container/{}",
            dozzle_url.trim_end_matches('/'),
            container_key.container_id
        );

        // Open the URL using the 'open' crate (cross-platform)
        let _ = open::that(&full_url);

        RenderAction::None // No need to force draw
    }
}
