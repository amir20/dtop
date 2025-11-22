use crate::core::app_state::AppState;
use crate::core::types::{RenderAction, ViewState};

impl AppState {
    pub(super) fn handle_select_previous(&mut self) -> RenderAction {
        // Only handle in ContainerList view (not in ActionMenu or LogView)
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        let container_count = self.containers.len();
        if container_count > 0 {
            let selected = self.table_state.selected().unwrap_or(0);
            if selected > 0 {
                self.table_state.select(Some(selected - 1));
            }
        }
        RenderAction::Render // Force draw - selection changed
    }

    pub(super) fn handle_select_next(&mut self) -> RenderAction {
        // Only handle in ContainerList view (not in ActionMenu or LogView)
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        let container_count = self.containers.len();
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
}
