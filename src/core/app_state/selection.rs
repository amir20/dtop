use crate::core::app_state::AppState;
use crate::core::types::{ContainerAction, ContainerKey, RenderAction, ViewState};

impl AppState {
    /// Space: marks or unmarks the highlighted container, then moves down so
    /// a run of containers can be marked by holding Space.
    pub(super) fn handle_toggle_mark(&mut self) -> RenderAction {
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }
        let Some(selected) = self.table_state.selected() else {
            return RenderAction::None;
        };
        let Some(key) = self.sorted_container_keys.get(selected) else {
            return RenderAction::None;
        };
        if !self.marked_containers.remove(key) {
            self.marked_containers.insert(key.clone());
        }
        if selected + 1 < self.sorted_container_keys.len() {
            self.table_state.select(Some(selected + 1));
        }
        RenderAction::Render
    }

    /// Ctrl+A: marks every visible container, or unmarks them all if they
    /// already are. Not offered while typing a search, where keys go to the
    /// input; after Enter applies the filter it marks the matching rows.
    pub(super) fn handle_toggle_mark_all(&mut self) -> RenderAction {
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }
        let all_marked = self
            .sorted_container_keys
            .iter()
            .all(|key| self.marked_containers.contains(key));
        if all_marked {
            for key in &self.sorted_container_keys {
                self.marked_containers.remove(key);
            }
        } else {
            self.marked_containers
                .extend(self.sorted_container_keys.iter().cloned());
        }
        RenderAction::Render
    }

    /// Marked containers that are currently listed, in display order.
    ///
    /// Bulk actions only touch these: a container hidden by the search filter
    /// or by 'a' stays marked, but is not acted on behind the user's back.
    /// Running an action unmarks only the containers it was run on.
    pub fn marked_visible_keys(&self) -> Vec<ContainerKey> {
        self.sorted_container_keys
            .iter()
            .filter(|key| self.marked_containers.contains(*key))
            .cloned()
            .collect()
    }

    /// Actions the bulk menu offers for the marked, visible containers.
    pub fn bulk_actions(&self) -> Vec<ContainerAction> {
        ContainerAction::available_for_states(
            self.sorted_container_keys
                .iter()
                .filter(|key| self.marked_containers.contains(*key))
                .filter_map(|key| self.containers.get(key))
                .map(|c| &c.state),
        )
    }

    /// Drops marks for containers that no longer exist.
    pub(super) fn prune_marks(&mut self) {
        self.marked_containers
            .retain(|key| self.containers.contains_key(key));
    }
}
