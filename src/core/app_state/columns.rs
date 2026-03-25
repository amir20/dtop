use crate::core::app_state::AppState;
use crate::core::types::{RenderAction, ViewState};

impl AppState {
    pub(super) fn handle_open_column_selector(&mut self) -> RenderAction {
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }
        self.column_config_snapshot = Some(self.column_config.clone());
        self.view_state = ViewState::ColumnSelector;
        self.column_selector_state.select(Some(0));
        self.column_save_prompt = false;
        RenderAction::Render
    }

    pub(super) fn handle_column_selector_key(
        &mut self,
        _key: crossterm::event::KeyEvent,
    ) -> RenderAction {
        RenderAction::None
    }
}
