//! Column selector handlers
//!
//! Column changes are kept in memory until the user explicitly saves
//! with Ctrl-S (handled in preferences.rs).

use crate::core::app_state::AppState;
use crate::core::types::{RenderAction, ViewState};

impl AppState {
    pub(super) fn handle_open_column_selector(&mut self) -> RenderAction {
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }
        self.view_state = ViewState::ColumnSelector;
        self.column_selector_state.select(Some(0));
        RenderAction::Render
    }

    pub(super) fn handle_column_selector_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> RenderAction {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let current = self.column_selector_state.selected().unwrap_or(0);
                if current > 0 {
                    self.column_selector_state.select(Some(current - 1));
                }
                RenderAction::Render
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let current = self.column_selector_state.selected().unwrap_or(0);
                let max = self.column_config.columns.len().saturating_sub(1);
                if current < max {
                    self.column_selector_state.select(Some(current + 1));
                }
                RenderAction::Render
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(idx) = self.column_selector_state.selected() {
                    self.column_config.toggle(idx);
                }
                RenderAction::Render
            }
            KeyCode::PageUp => {
                if let Some(idx) = self.column_selector_state.selected() {
                    self.column_config.move_up(idx);
                    if idx > 0 {
                        self.column_selector_state.select(Some(idx - 1));
                    }
                }
                RenderAction::Render
            }
            KeyCode::PageDown => {
                if let Some(idx) = self.column_selector_state.selected() {
                    self.column_config.move_down(idx);
                    let max = self.column_config.columns.len().saturating_sub(1);
                    if idx < max {
                        self.column_selector_state.select(Some(idx + 1));
                    }
                }
                RenderAction::Render
            }
            KeyCode::Esc | KeyCode::Char('c') => self.close_column_selector(),
            _ => RenderAction::None,
        }
    }

    fn close_column_selector(&mut self) -> RenderAction {
        self.view_state = ViewState::ContainerList;
        self.column_selector_state.select(None);
        RenderAction::Render
    }
}
