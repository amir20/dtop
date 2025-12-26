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
        let container_count = self.sorted_container_keys.len();
        if container_count == 0 {
            self.table_state.select(None);
        } else if let Some(selected) = self.table_state.selected()
            && selected >= container_count
        {
            self.table_state.select(Some(container_count - 1));
        } else if self.table_state.selected().is_none() && container_count > 0 {
            self.table_state.select(Some(0));
        }

        RenderAction::Render // Force redraw to hide search bar
    }

    pub(super) fn handle_search_key_event(
        &mut self,
        key_event: crossterm::event::KeyEvent,
    ) -> RenderAction {
        use crossterm::event::KeyCode;

        // Only process typing keys when in search mode
        // Enter and Escape are handled by handle_enter_pressed and handle_exit_log_view
        if self.view_state != ViewState::SearchMode {
            return RenderAction::None;
        }

        // Skip Enter and Escape - they're handled elsewhere
        if matches!(key_event.code, KeyCode::Enter | KeyCode::Esc) {
            return RenderAction::None;
        }

        // Manually handle key events to avoid crossterm version conflicts
        // tui-input depends on crossterm 0.28, but we use 0.29
        match key_event.code {
            KeyCode::Char(c) => {
                // Insert character at cursor position
                let current_value = self.search_input.value();
                let cursor = self.search_input.visual_cursor();
                let mut new_value = String::with_capacity(current_value.len() + 1);
                new_value.push_str(&current_value[..cursor]);
                new_value.push(c);
                new_value.push_str(&current_value[cursor..]);
                self.search_input = tui_input::Input::new(new_value).with_cursor(cursor + 1);
            }
            KeyCode::Backspace => {
                // Delete character before cursor
                let current_value = self.search_input.value();
                let cursor = self.search_input.visual_cursor();
                if cursor > 0 {
                    let mut new_value = String::with_capacity(current_value.len());
                    new_value.push_str(&current_value[..cursor - 1]);
                    new_value.push_str(&current_value[cursor..]);
                    self.search_input = tui_input::Input::new(new_value).with_cursor(cursor - 1);
                }
            }
            KeyCode::Delete => {
                // Delete character at cursor
                let current_value = self.search_input.value();
                let cursor = self.search_input.visual_cursor();
                if cursor < current_value.len() {
                    let mut new_value = String::with_capacity(current_value.len());
                    new_value.push_str(&current_value[..cursor]);
                    new_value.push_str(&current_value[cursor + 1..]);
                    self.search_input = tui_input::Input::new(new_value).with_cursor(cursor);
                }
            }
            KeyCode::Left => {
                // Move cursor left
                let cursor = self.search_input.visual_cursor();
                if cursor > 0 {
                    self.search_input = tui_input::Input::new(self.search_input.value().to_string())
                        .with_cursor(cursor - 1);
                }
            }
            KeyCode::Right => {
                // Move cursor right
                let current_value = self.search_input.value();
                let cursor = self.search_input.visual_cursor();
                if cursor < current_value.len() {
                    self.search_input = tui_input::Input::new(current_value.to_string())
                        .with_cursor(cursor + 1);
                }
            }
            KeyCode::Home => {
                // Move cursor to start
                self.search_input = tui_input::Input::new(self.search_input.value().to_string())
                    .with_cursor(0);
            }
            KeyCode::End => {
                // Move cursor to end
                let len = self.search_input.value().len();
                self.search_input = tui_input::Input::new(self.search_input.value().to_string())
                    .with_cursor(len);
            }
            _ => {
                // Ignore other keys
                return RenderAction::None;
            }
        }

        // Force immediate re-filter and sort as user types
        self.force_sort_containers();

        // Adjust selection after filtering
        let container_count = self.sorted_container_keys.len();
        if container_count == 0 {
            self.table_state.select(None);
        } else if let Some(selected) = self.table_state.selected()
            && selected >= container_count
        {
            // If current selection is out of bounds, select the last item
            self.table_state.select(Some(container_count - 1));
        } else if self.table_state.selected().is_none() && container_count > 0 {
            // If nothing is selected but we have containers, select the first one
            self.table_state.select(Some(0));
        }

        RenderAction::Render // Force redraw to show updated search text and filtered results
    }
}
