use crate::core::app_state::AppState;
use crate::core::types::ViewState;

impl AppState {
    pub(super) fn handle_select_previous(&mut self) -> bool {
        // Only handle in ContainerList view (not in ActionMenu or LogView)
        if self.view_state != ViewState::ContainerList {
            return false;
        }

        let container_count = self.containers.len();
        if container_count > 0 {
            let selected = self.table_state.selected().unwrap_or(0);
            if selected > 0 {
                self.table_state.select(Some(selected - 1));
            }
        }
        true // Force draw - selection changed
    }

    pub(super) fn handle_select_next(&mut self) -> bool {
        // Only handle in ContainerList view (not in ActionMenu or LogView)
        if self.view_state != ViewState::ContainerList {
            return false;
        }

        let container_count = self.containers.len();
        if container_count > 0 {
            let selected = self.table_state.selected().unwrap_or(0);
            if selected < container_count - 1 {
                self.table_state.select(Some(selected + 1));
            }
        }
        true // Force draw - selection changed
    }

    pub(super) fn handle_toggle_help(&mut self) -> bool {
        self.show_help = !self.show_help;
        true // Force redraw to show/hide popup
    }
}
