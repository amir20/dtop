use crate::core::app_state::AppState;
use crate::core::types::{RenderAction, ViewState};

impl AppState {
    pub(super) fn handle_enter_search_mode(&mut self) -> RenderAction {
        // Only allow entering search mode from ContainerList view
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // Activate search mode
        self.view_state = ViewState::SearchMode;

        // Clear any existing search input
        self.search_input.reset();

        RenderAction::Render // Force redraw to show search bar
    }

    pub(super) fn handle_exit_search_mode(&mut self) -> RenderAction {
        // Only handle if we're in search mode
        if self.view_state != ViewState::SearchMode {
            return RenderAction::None;
        }

        // Deactivate search mode
        self.view_state = ViewState::ContainerList;

        // Clear the search input
        self.search_input.reset();

        // Force immediate re-sort/filter when exiting search mode
        self.force_sort_containers();

        // Adjust selection after clearing filter
        self.clamp_selection();
        if self.table_state.selected().is_none() && !self.sorted_container_keys.is_empty() {
            self.table_state.select(Some(0));
        }

        RenderAction::Render // Force redraw to hide search bar
    }

    pub(super) fn handle_search_key_event(
        &mut self,
        key_event: crossterm::event::KeyEvent,
    ) -> RenderAction {
        // Only process typing keys when in search mode
        if self.view_state != ViewState::SearchMode {
            return RenderAction::None;
        }

        // Pass the key event to tui-input to handle character input, backspace, etc.
        use tui_input::backend::crossterm::EventHandler;
        self.search_input
            .handle_event(&crossterm::event::Event::Key(key_event));

        // Force immediate re-filter and sort as user types
        self.force_sort_containers();

        // Adjust selection after filtering
        self.clamp_selection();
        if self.table_state.selected().is_none() && !self.sorted_container_keys.is_empty() {
            self.table_state.select(Some(0));
        }

        RenderAction::Render // Force redraw to show updated search text and filtered results
    }
}
