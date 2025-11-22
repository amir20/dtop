use crate::core::app_state::AppState;
use crate::core::types::{ContainerAction, ContainerKey, RenderAction, ViewState};

impl AppState {
    pub(super) fn handle_show_action_menu(&mut self) -> RenderAction {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // Get the selected container
        let Some(selected_idx) = self.table_state.selected() else {
            return RenderAction::None;
        };

        let Some(container_key) = self.sorted_container_keys.get(selected_idx) else {
            return RenderAction::None;
        };

        // Switch to action menu view
        self.view_state = ViewState::ActionMenu(container_key.clone());

        // Reset action menu selection to first item
        self.action_menu_state.select(Some(0));

        RenderAction::Render // Force draw - view changed
    }

    pub(super) fn handle_cancel_action_menu(&mut self) -> RenderAction {
        // If help is shown, close it first
        if self.show_help {
            self.show_help = false;
            return RenderAction::Render; // Force redraw
        }

        // Handle Escape based on current view state
        match self.view_state {
            ViewState::SearchMode => {
                // Exit search mode and clear filter
                return self.handle_exit_search_mode();
            }
            ViewState::LogView(_) => {
                // Exit log view
                return self.handle_exit_log_view();
            }
            ViewState::ActionMenu(_) => {
                // Exit action menu
            }
            _ => {
                // Ignore Escape in other views
                return RenderAction::None;
            }
        }

        // Switch back to container list view
        self.view_state = ViewState::ContainerList;

        // Clear action menu selection
        self.action_menu_state.select(None);

        RenderAction::Render // Force draw - view changed
    }

    pub(super) fn handle_select_action_up(&mut self) -> RenderAction {
        // Only handle in action menu view
        let ViewState::ActionMenu(ref container_key) = self.view_state else {
            return RenderAction::None;
        };

        // Get the container to determine available actions
        let Some(container) = self.containers.get(container_key) else {
            return RenderAction::None;
        };

        let available_actions = ContainerAction::available_for_state(&container.state);

        if available_actions.is_empty() {
            return RenderAction::None;
        }

        // Move selection up
        let current = self.action_menu_state.selected().unwrap_or(0);
        if current > 0 {
            self.action_menu_state.select(Some(current - 1));
            RenderAction::Render // Force draw
        } else {
            RenderAction::None
        }
    }

    pub(super) fn handle_select_action_down(&mut self) -> RenderAction {
        // Only handle in action menu view
        let ViewState::ActionMenu(ref container_key) = self.view_state else {
            return RenderAction::None;
        };

        // Get the container to determine available actions
        let Some(container) = self.containers.get(container_key) else {
            return RenderAction::None;
        };

        let available_actions = ContainerAction::available_for_state(&container.state);

        if available_actions.is_empty() {
            return RenderAction::None;
        }

        // Move selection down
        let current = self.action_menu_state.selected().unwrap_or(0);
        if current < available_actions.len() - 1 {
            self.action_menu_state.select(Some(current + 1));
            RenderAction::Render // Force draw
        } else {
            RenderAction::None
        }
    }

    pub(super) fn handle_execute_action(&mut self) -> RenderAction {
        // Only handle in action menu view
        let ViewState::ActionMenu(ref container_key) = self.view_state else {
            return RenderAction::None;
        };

        // Get the selected action
        let Some(selected_idx) = self.action_menu_state.selected() else {
            return RenderAction::None;
        };

        // Get the container to determine available actions
        let Some(container) = self.containers.get(container_key) else {
            return RenderAction::None;
        };

        let available_actions = ContainerAction::available_for_state(&container.state);

        let Some(&action) = available_actions.get(selected_idx) else {
            return RenderAction::None;
        };

        // Get the Docker host for this container
        let Some(host) = self.connected_hosts.get(&container_key.host_id) else {
            // Silently fail if host not found
            return RenderAction::None;
        };

        // Handle Shell action specially - it needs to take over the terminal
        if action == ContainerAction::Shell {
            let container_key_clone = container_key.clone();

            // Close the action menu immediately
            self.view_state = ViewState::ContainerList;
            self.action_menu_state.select(None);

            return RenderAction::StartShell(container_key_clone);
        }

        // Spawn async task to execute the action
        let host_clone = host.clone();
        let container_key_clone = container_key.clone();
        let tx_clone = self.event_tx.clone();

        tokio::spawn(async move {
            crate::docker::actions::execute_container_action(
                host_clone,
                container_key_clone,
                action,
                tx_clone,
            )
            .await;
        });

        // Close the action menu immediately
        self.view_state = ViewState::ContainerList;
        self.action_menu_state.select(None);

        RenderAction::Render // Force draw
    }

    pub(super) fn handle_action_in_progress(
        &mut self,
        _key: ContainerKey,
        _action: ContainerAction,
    ) -> RenderAction {
        // TODO: Could show a loading indicator in the UI in the future
        // For now, just let Docker events update the container state
        RenderAction::None // Don't force redraw for progress events
    }

    pub(super) fn handle_action_success(
        &mut self,
        _key: ContainerKey,
        _action: ContainerAction,
    ) -> RenderAction {
        // TODO: Could show a success toast/notification in the UI in the future
        // The container state will be updated by Docker events
        // so we don't need to manually update it here
        RenderAction::None // Don't force redraw - Docker events will trigger updates
    }

    pub(super) fn handle_action_error(
        &mut self,
        _key: ContainerKey,
        _action: ContainerAction,
        _error: String,
    ) -> RenderAction {
        // TODO: Could show an error toast/notification in the UI in the future
        // For now, silently fail - the container state won't change on error
        RenderAction::None // Don't force redraw for error messages
    }
}
