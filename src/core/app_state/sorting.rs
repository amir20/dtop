use crate::core::app_state::AppState;
use crate::core::types::{
    ContainerState, RenderAction, SortDirection, SortField, SortState, ViewState,
};
use std::time::Duration;

/// Minimum time between sorts to avoid re-sorting on every frame
const SORT_THROTTLE_DURATION: Duration = Duration::from_secs(3);

/// All sort fields in display order
const SORT_FIELDS: [SortField; 4] = [
    SortField::Uptime,
    SortField::Name,
    SortField::Cpu,
    SortField::Memory,
];

impl AppState {
    /// Opens the sort selector popup
    pub(super) fn handle_open_sort_selector(&mut self) -> RenderAction {
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // Pre-select the currently active sort field
        let current_idx = SORT_FIELDS
            .iter()
            .position(|f| *f == self.sort_state.field)
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
                let max = SORT_FIELDS.len().saturating_sub(1);
                if current < max {
                    self.sort_selector_state.select(Some(current + 1));
                }
                RenderAction::Render
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(idx) = self.sort_selector_state.selected()
                    && let Some(&field) = SORT_FIELDS.get(idx)
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

        match self.sort_state.field {
            SortField::Uptime => {
                key_container_pairs.sort_by(|(_, a), (_, b)| match a.host_id.cmp(&b.host_id) {
                    std::cmp::Ordering::Equal => {
                        let ord = match (&a.created, &b.created) {
                            (Some(a_time), Some(b_time)) => a_time.cmp(b_time),
                            (Some(_), None) => std::cmp::Ordering::Greater,
                            (None, Some(_)) => std::cmp::Ordering::Less,
                            (None, None) => std::cmp::Ordering::Equal,
                        };
                        if direction == SortDirection::Descending {
                            ord.reverse()
                        } else {
                            ord
                        }
                    }
                    other => other,
                });
            }
            SortField::Name => {
                key_container_pairs.sort_by(|(_, a), (_, b)| match a.host_id.cmp(&b.host_id) {
                    std::cmp::Ordering::Equal => {
                        let ord = a.name.cmp(&b.name);
                        if direction == SortDirection::Descending {
                            ord.reverse()
                        } else {
                            ord
                        }
                    }
                    other => other,
                });
            }
            SortField::Cpu => {
                key_container_pairs.sort_by(|(_, a), (_, b)| match a.host_id.cmp(&b.host_id) {
                    std::cmp::Ordering::Equal => {
                        let ord = a
                            .stats
                            .cpu
                            .partial_cmp(&b.stats.cpu)
                            .unwrap_or(std::cmp::Ordering::Equal);
                        if direction == SortDirection::Descending {
                            ord.reverse()
                        } else {
                            ord
                        }
                    }
                    other => other,
                });
            }
            SortField::Memory => {
                key_container_pairs.sort_by(|(_, a), (_, b)| match a.host_id.cmp(&b.host_id) {
                    std::cmp::Ordering::Equal => {
                        let ord = a
                            .stats
                            .memory
                            .partial_cmp(&b.stats.memory)
                            .unwrap_or(std::cmp::Ordering::Equal);
                        if direction == SortDirection::Descending {
                            ord.reverse()
                        } else {
                            ord
                        }
                    }
                    other => other,
                });
            }
        }

        // Extract sorted keys
        self.sorted_container_keys = key_container_pairs
            .into_iter()
            .map(|(key, _)| key.clone())
            .collect();
    }
}

/// Returns the sort fields in display order (used by the UI renderer)
pub fn sort_fields() -> &'static [SortField; 4] {
    &SORT_FIELDS
}
