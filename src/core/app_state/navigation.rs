use crate::core::app_state::AppState;
use crate::core::types::{RenderAction, ViewState};

impl AppState {
    pub(super) fn handle_select_previous(&mut self) -> RenderAction {
        // Allow navigation in ContainerList and SearchMode
        if !matches!(
            self.view_state,
            ViewState::ContainerList | ViewState::SearchMode
        ) {
            return RenderAction::None;
        }

        let container_count = self.sorted_container_keys.len();
        if container_count > 0 {
            let selected = self.table_state.selected().unwrap_or(0);
            if selected > 0 {
                self.table_state.select(Some(selected - 1));
            }
        }
        RenderAction::Render // Force draw - selection changed
    }

    pub(super) fn handle_select_next(&mut self) -> RenderAction {
        // Allow navigation in ContainerList and SearchMode
        if !matches!(
            self.view_state,
            ViewState::ContainerList | ViewState::SearchMode
        ) {
            return RenderAction::None;
        }

        let container_count = self.sorted_container_keys.len();
        if container_count > 0 {
            let selected = self.table_state.selected().unwrap_or(0);
            if selected < container_count - 1 {
                self.table_state.select(Some(selected + 1));
            }
        }
        RenderAction::Render // Force draw - selection changed
    }

    pub(super) fn handle_toggle_help(&mut self) -> RenderAction {
        self.show_help = !self.show_help;
        RenderAction::Render // Force redraw to show/hide popup
    }

    /// Clamps the current table selection to be within the valid range of sorted container keys.
    /// Call this after filtering or removing containers to ensure the selection remains valid.
    pub fn clamp_selection(&mut self) {
        let container_count = self.sorted_container_keys.len();
        if container_count == 0 {
            self.table_state.select(None);
        } else if let Some(selected) = self.table_state.selected()
            && selected >= container_count
        {
            self.table_state.select(Some(container_count - 1));
        }
    }
}
