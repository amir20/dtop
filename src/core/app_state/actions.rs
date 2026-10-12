use std::collections::HashSet;
use std::time::Duration;

use crate::core::app_state::{AppState, BulkProgress};
use crate::core::types::{ContainerAction, ContainerKey, RenderAction, ViewState};

impl AppState {
    pub(super) fn handle_show_action_menu(&mut self) -> RenderAction {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // With containers marked, Enter acts on all of them instead
        if self
            .marked_containers
            .iter()
            .any(|key| self.sorted_container_keys.contains(key))
        {
            if self.bulk_actions().is_empty() {
                self.show_notification("No actions available for the selected containers");
                return RenderAction::Render;
            }
            self.view_state = ViewState::BulkActionMenu;
            self.action_menu_state.select(Some(0));
            self.armed_bulk_action = None;
            return RenderAction::Render;
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
            ViewState::ActionMenu(_) | ViewState::BulkActionMenu => {
                // Exit action menu
                self.armed_bulk_action = None;
            }
            ViewState::ContainerList if !self.marked_containers.is_empty() => {
                // Unmark everything
                self.marked_containers.clear();
                return RenderAction::Render;
            }
            ViewState::ColumnSelector => {
                // Switch back to container list view
                self.view_state = ViewState::ContainerList;
                self.column_selector_state.select(None);
                return RenderAction::Render;
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

    /// The action under the menu cursor, if a menu is open
    pub fn selected_menu_action(&self) -> Option<ContainerAction> {
        let index = self.action_menu_state.selected()?;
        self.menu_actions().get(index).copied()
    }

    /// Re-checks an open action menu after its containers changed.
    ///
    /// Closes it if nothing is left to offer, e.g. because its containers were
    /// removed (it would otherwise stay active but undrawn). Otherwise keeps
    /// the cursor on `previous`, the action it was on, since a list that lost
    /// an entry above it would leave the same index on a different action.
    /// If that action is gone, the cursor is clamped and an armed Remove is
    /// disarmed, so the second Enter can't confirm something else.
    pub(super) fn revalidate_action_menu(&mut self, previous: Option<ContainerAction>) {
        if !matches!(
            self.view_state,
            ViewState::ActionMenu(_) | ViewState::BulkActionMenu
        ) {
            return;
        }
        let actions = self.menu_actions();
        if actions.is_empty() {
            self.view_state = ViewState::ContainerList;
            self.action_menu_state.select(None);
            self.armed_bulk_action = None;
            return;
        }
        match previous.and_then(|action| actions.iter().position(|a| *a == action)) {
            Some(index) => self.action_menu_state.select(Some(index)),
            None => {
                let index = self
                    .action_menu_state
                    .selected()
                    .unwrap_or(0)
                    .min(actions.len() - 1);
                self.action_menu_state.select(Some(index));
                self.armed_bulk_action = None;
            }
        }
    }

    /// Actions listed by the open action menu, single or bulk
    pub fn menu_actions(&self) -> Vec<ContainerAction> {
        match &self.view_state {
            ViewState::ActionMenu(key) => self
                .containers
                .get(key)
                .map(|c| ContainerAction::available_for_state(&c.state))
                .unwrap_or_default(),
            ViewState::BulkActionMenu => self.bulk_actions(),
            _ => Vec::new(),
        }
    }

    pub(super) fn handle_select_action_up(&mut self) -> RenderAction {
        if self.menu_actions().is_empty() {
            return RenderAction::None;
        }

        // Move selection up
        let current = self.action_menu_state.selected().unwrap_or(0);
        if current > 0 {
            self.action_menu_state.select(Some(current - 1));
            self.armed_bulk_action = None;
            RenderAction::Render // Force draw
        } else {
            RenderAction::None
        }
    }

    pub(super) fn handle_select_action_down(&mut self) -> RenderAction {
        let action_count = self.menu_actions().len();
        if action_count == 0 {
            return RenderAction::None;
        }

        // Move selection down
        let current = self.action_menu_state.selected().unwrap_or(0);
        if current < action_count - 1 {
            self.action_menu_state.select(Some(current + 1));
            self.armed_bulk_action = None;
            RenderAction::Render // Force draw
        } else {
            RenderAction::None
        }
    }

    /// Runs the picked bulk action on every marked, visible container that
    /// supports it. Remove asks for a second Enter first.
    pub(super) fn handle_execute_bulk_action(&mut self) -> RenderAction {
        let Some(selected_idx) = self.action_menu_state.selected() else {
            return RenderAction::None;
        };
        let Some(action) = self.bulk_actions().get(selected_idx).copied() else {
            return RenderAction::None;
        };

        // The confirming Enter must land on the action that was armed
        let armed = self.armed_bulk_action.take();
        if armed.is_some_and(|armed| armed != action) {
            return RenderAction::Render;
        }
        if action == ContainerAction::Remove && armed.is_none() {
            self.armed_bulk_action = Some(action);
            return RenderAction::Render; // The menu footer asks to confirm
        }

        let targets: Vec<ContainerKey> = self
            .marked_visible_keys()
            .into_iter()
            .filter(|key| {
                self.containers.get(key).is_some_and(|c| {
                    ContainerAction::available_for_state(&c.state).contains(&action)
                })
            })
            .collect();

        let mut pending = HashSet::with_capacity(targets.len());
        let mut skipped = 0;
        for key in targets {
            let Some(host) = self.connected_hosts.get(&key.host_id) else {
                // Stays marked, so it can be retried once the host is back
                skipped += 1;
                continue;
            };
            let host = host.clone();
            let tx = self.event_tx.clone();
            self.marked_containers.remove(&key);
            pending.insert(key.clone());
            tokio::spawn(async move {
                crate::docker::actions::execute_container_action(host, key, action, tx).await;
            });
        }

        self.view_state = ViewState::ContainerList;
        self.action_menu_state.select(None);

        if pending.is_empty() {
            self.show_notification("No connected containers to act on");
            return RenderAction::Render;
        }

        // A container now in this batch reports here, not to an older one
        self.bulk_progress.retain_mut(|batch| {
            let before = batch.pending.len();
            batch.pending.retain(|key| !pending.contains(key));
            batch.total -= before - batch.pending.len();
            !batch.pending.is_empty()
        });

        let total = pending.len();
        let (doing, _) = action.progress_words();
        let skipped_note = if skipped > 0 {
            format!(" ({skipped} skipped, host disconnected)")
        } else {
            String::new()
        };
        self.show_action_notification(
            &format!("{doing} {}…{skipped_note}", containers_word(total)),
            Duration::from_secs(15),
            false,
        );
        self.bulk_progress.push(BulkProgress {
            action,
            pending,
            total,
            failed: 0,
            last_error: None,
            started: std::time::Instant::now(),
        });

        RenderAction::Render
    }

    /// Drops bulk batches whose results stopped arriving (e.g. a result event
    /// was lost), so they can't swallow later toasts for their containers.
    /// Docker's stop/restart timeout is 10s, so a minute is generous.
    fn prune_stale_bulk_progress(&mut self) {
        self.bulk_progress
            .retain(|batch| batch.started.elapsed() < BULK_RESULT_TIMEOUT);
    }

    /// Records a bulk member's result. Returns false if the container is not
    /// part of the running bulk action.
    fn record_bulk_result(
        &mut self,
        key: &ContainerKey,
        action: ContainerAction,
        error: Option<&str>,
    ) -> bool {
        self.prune_stale_bulk_progress();
        let Some(index) = self
            .bulk_progress
            .iter()
            .position(|p| p.action == action && p.pending.contains(key))
        else {
            return false;
        };
        let progress = &mut self.bulk_progress[index];
        progress.pending.remove(key);
        if let Some(error) = error {
            progress.failed += 1;
            progress.last_error = Some(error.to_string());
            // Re-mark it, so retrying the failures is Enter away
            if self.containers.contains_key(key) {
                self.marked_containers.insert(key.clone());
            }
        }
        if progress.pending.is_empty() {
            let (_, done) = action.progress_words();
            let (total, failed) = (progress.total, progress.failed);
            let last_error = progress.last_error.take();
            self.bulk_progress.swap_remove(index);
            if failed > 0 && total == 1 {
                // The container's own error toast already says what happened
            } else if failed == 0 {
                self.show_action_notification(
                    &format!("{done} {}", containers_word(total)),
                    Duration::from_secs(2),
                    false,
                );
            } else {
                // There is one notification slot, so earlier failures' toasts
                // are already gone; keep the latest reason in the summary
                let reason = match last_error {
                    Some(error) if failed == 1 => format!(": {error}"),
                    Some(error) => format!(", last: {error}"),
                    None => String::new(),
                };
                self.show_action_notification(
                    &format!(
                        "{done} {} of {total} containers ({failed} failed{reason})",
                        total - failed
                    ),
                    Duration::from_secs(6),
                    true,
                );
            }
        }
        true
    }

    pub(super) fn handle_execute_action(&mut self) -> RenderAction {
        if self.view_state == ViewState::BulkActionMenu {
            return self.handle_execute_bulk_action();
        }

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

    /// The container's name for notifications. Remembered when the action
    /// starts, since a removed container can be gone from the list (via its
    /// destroy event) before the action's result arrives; the ID is the last
    /// resort.
    fn action_target_name(&self, key: &ContainerKey) -> String {
        self.action_names
            .get(key)
            .or_else(|| self.containers.get(key).map(|c| &c.name))
            .cloned()
            .unwrap_or_else(|| key.container_id.clone())
    }

    /// Shows background action progress, unless an unexpired error is up:
    /// routine news ("Stopped x") shouldn't replace an error before it can be
    /// read. A newer error still replaces an older one.
    fn show_action_notification(&mut self, message: &str, duration: Duration, is_error: bool) {
        let showing_error = self.notification_is_error
            && self
                .notification
                .as_ref()
                .is_some_and(|(_, expiry)| std::time::Instant::now() < *expiry);
        if showing_error && !is_error {
            return;
        }
        self.show_notification_for(message, duration, is_error);
    }

    pub(super) fn handle_action_in_progress(
        &mut self,
        key: ContainerKey,
        action: ContainerAction,
    ) -> RenderAction {
        // Stop and restart wait up to 10s for the container to exit, so say
        // something is happening. The result normally replaces it; the expiry
        // only matters if none arrives (e.g. the host dropped mid-action).
        let name = self.action_target_name(&key);
        // Bulk members share the "Stopping N containers…" toast shown at launch
        self.prune_stale_bulk_progress();
        let in_bulk = self
            .bulk_progress
            .iter()
            .any(|p| p.action == action && p.pending.contains(&key));
        if !in_bulk {
            let (doing, _) = action.progress_words();
            self.show_action_notification(
                &format!("{doing} {name}…"),
                Duration::from_secs(15),
                false,
            );
        }
        self.action_names.insert(key, name);
        RenderAction::Render
    }

    pub(super) fn handle_action_success(
        &mut self,
        key: ContainerKey,
        action: ContainerAction,
    ) -> RenderAction {
        // The row itself is updated by the Docker event for the change
        if self.record_bulk_result(&key, action, None) {
            self.action_names.remove(&key);
            return RenderAction::Render;
        }
        let (_, done) = action.progress_words();
        let message = format!("{done} {}", self.action_target_name(&key));
        self.action_names.remove(&key);
        self.show_action_notification(&message, Duration::from_secs(2), false);
        RenderAction::Render
    }

    pub(super) fn handle_action_error(
        &mut self,
        key: ContainerKey,
        action: ContainerAction,
        error: String,
    ) -> RenderAction {
        tracing::warn!(
            "{} failed for {}: {}",
            action.display_name(),
            key.container_id,
            error
        );
        // Docker errors can span lines; the toast has room for one
        let error = error.lines().next().unwrap_or_default();
        let message = format!(
            "Failed to {} {}: {error}",
            action.display_name().to_lowercase(),
            self.action_target_name(&key)
        );
        self.action_names.remove(&key);
        self.show_action_notification(&message, Duration::from_secs(6), true);
        // Each failure gets its own toast; after the last result of a bulk
        // action, its summary replaces it.
        self.record_bulk_result(&key, action, Some(error));
        RenderAction::Render
    }
}

/// How long a bulk batch waits for its results before it is dropped
const BULK_RESULT_TIMEOUT: Duration = Duration::from_secs(60);

/// "1 container" / "3 containers"
fn containers_word(count: usize) -> String {
    if count == 1 {
        "1 container".to_string()
    } else {
        format!("{count} containers")
    }
}
