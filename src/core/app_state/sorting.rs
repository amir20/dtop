use crate::core::app_state::AppState;
use crate::core::types::{
    Column, ContainerState, RenderAction, SortDirection, SortState, ViewState,
};
use std::time::Duration;

/// Minimum time between sorts to avoid re-sorting on every frame
const SORT_THROTTLE_DURATION: Duration = Duration::from_secs(3);

impl AppState {
    /// Opens the sort selector popup
    pub(super) fn handle_open_sort_selector(&mut self) -> RenderAction {
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // Pre-select the currently active sort field from visible columns
        let visible = self.column_config.visible_columns();
        let current_idx = visible
            .iter()
            .position(|c| *c == self.sort_state.field)
            .unwrap_or(0);

        self.view_state = ViewState::SortSelector;
        self.sort_selector_state.select(Some(current_idx));
        RenderAction::Render
    }

    /// Handles key events while in the sort selector popup
    pub(super) fn handle_sort_selector_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> RenderAction {
        use crossterm::event::KeyCode;

        let visible = self.column_config.visible_columns();
        let count = visible.len();

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let current = self.sort_selector_state.selected().unwrap_or(0);
                if current > 0 {
                    self.sort_selector_state.select(Some(current - 1));
                }
                RenderAction::Render
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let current = self.sort_selector_state.selected().unwrap_or(0);
                let max = count.saturating_sub(1);
                if current < max {
                    self.sort_selector_state.select(Some(current + 1));
                }
                RenderAction::Render
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(idx) = self.sort_selector_state.selected()
                    && let Some(&field) = visible.get(idx)
                {
                    if self.sort_state.field == field {
                        // Same field: toggle direction
                        self.sort_state.direction = self.sort_state.direction.toggle();
                    } else {
                        // Different field: set with default direction
                        self.sort_state = SortState::new(field);
                    }
                    self.force_sort_containers();
                }
                RenderAction::Render
            }
            KeyCode::Esc | KeyCode::Char('s') => self.close_sort_selector(),
            _ => RenderAction::None,
        }
    }

    fn close_sort_selector(&mut self) -> RenderAction {
        self.view_state = ViewState::ContainerList;
        self.sort_selector_state.select(None);
        RenderAction::Render
    }

    pub(super) fn handle_toggle_show_all(&mut self) -> RenderAction {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // Toggle the show_all_containers flag
        self.show_all_containers = !self.show_all_containers;

        // Force immediate re-sort/filter when user toggles visibility
        self.force_sort_containers();

        // Adjust selection if needed after filtering
        self.clamp_selection();

        RenderAction::Render // Force redraw - visibility changed
    }

    /// Sorts the container keys based on the current sort field and direction
    /// If force is false, will only sort if enough time has passed since last sort
    pub fn sort_containers(&mut self) {
        self.sort_containers_internal(false);
    }

    /// Forces an immediate sort regardless of throttle duration
    pub fn force_sort_containers(&mut self) {
        self.sort_containers_internal(true);
    }

    /// Internal sorting implementation with throttling control
    fn sort_containers_internal(&mut self, force: bool) {
        // Check if we should skip sorting due to throttle (unless forced)
        if !force && self.last_sort_time.elapsed() < SORT_THROTTLE_DURATION {
            return;
        }

        // Update last sort time
        self.last_sort_time = std::time::Instant::now();
        // Get the search filter (case-insensitive)
        let search_filter = self.search_input.value().to_lowercase();
        let has_search_filter = !search_filter.is_empty();

        // Collect (key, container) pairs to avoid repeated HashMap lookups during sort
        let mut key_container_pairs: Vec<_> = self
            .containers
            .iter()
            .filter(|(_, container)| {
                // First filter by running state
                if !self.show_all_containers && container.state != ContainerState::Running {
                    return false;
                }

                // Then filter by search term if present
                if has_search_filter {
                    let name_matches = container.name.to_lowercase().contains(&search_filter);
                    let id_matches = container.id.to_lowercase().contains(&search_filter);
                    let host_matches = container.host_id.to_lowercase().contains(&search_filter);
                    name_matches || id_matches || host_matches
                } else {
                    true
                }
            })
            .collect();

        let direction = self.sort_state.direction;
        let sort_field = self.sort_state.field;

        key_container_pairs.sort_by(|(_, a), (_, b)| match a.host_id.cmp(&b.host_id) {
            std::cmp::Ordering::Equal => {
                let ord = match sort_field {
                    Column::Uptime => match (&a.created, &b.created) {
                        (Some(a_time), Some(b_time)) => a_time.cmp(b_time),
                        (Some(_), None) => std::cmp::Ordering::Greater,
                        (None, Some(_)) => std::cmp::Ordering::Less,
                        (None, None) => std::cmp::Ordering::Equal,
                    },
                    Column::Name => a.name.cmp(&b.name),
                    Column::Id => a.id.cmp(&b.id),
                    Column::Host => a.host_id.cmp(&b.host_id),
                    Column::Compose => a.compose_project.cmp(&b.compose_project),
                    Column::Cpu => a
                        .stats
                        .cpu
                        .partial_cmp(&b.stats.cpu)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    Column::Memory => a
                        .stats
                        .memory
                        .partial_cmp(&b.stats.memory)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    Column::NetTx => a
                        .stats
                        .network_tx_bytes_per_sec
                        .partial_cmp(&b.stats.network_tx_bytes_per_sec)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    Column::NetRx => a
                        .stats
                        .network_rx_bytes_per_sec
                        .partial_cmp(&b.stats.network_rx_bytes_per_sec)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    Column::Status => {
                        let a_state = format!("{:?}", a.state);
                        let b_state = format!("{:?}", b.state);
                        a_state.cmp(&b_state)
                    }
                    Column::Restarts => a.restart_count.cmp(&b.restart_count),
                };
                if direction == SortDirection::Descending {
                    ord.reverse()
                } else {
                    ord
                }
            }
            other => other,
        });

        // Extract sorted keys
        self.sorted_container_keys = key_container_pairs
            .into_iter()
            .map(|(key, _)| key.clone())
            .collect();
    }
}
