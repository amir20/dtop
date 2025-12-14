use crate::core::app_state::AppState;
use crate::core::types::{ContainerState, RenderAction, SortDirection, SortField, ViewState};
use std::time::Duration;

/// Minimum time between sorts to avoid re-sorting on every frame
const SORT_THROTTLE_DURATION: Duration = Duration::from_secs(3);

impl AppState {
    pub(super) fn handle_cycle_sort_field(&mut self) -> RenderAction {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // Cycle to next sort field with default direction
        self.sort_state = crate::core::types::SortState::new(self.sort_state.field.next());

        // Force immediate re-sort when user changes sort field
        self.force_sort_containers();

        RenderAction::Render // Force redraw - sort order changed
    }

    pub(super) fn handle_set_sort_field(&mut self, field: SortField) -> RenderAction {
        // Only handle in ContainerList view
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }

        // If same field, toggle direction; otherwise use default direction
        if self.sort_state.field == field {
            self.sort_state.direction = self.sort_state.direction.toggle();
        } else {
            self.sort_state = crate::core::types::SortState::new(field);
        }

        // Force immediate re-sort when user changes sort field
        self.force_sort_containers();

        RenderAction::Render // Force redraw - sort order changed
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
        let container_count = self.sorted_container_keys.len();
        if container_count == 0 {
            self.table_state.select(None);
        } else if let Some(selected) = self.table_state.selected()
            && selected >= container_count
        {
            self.table_state.select(Some(container_count - 1));
        }

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

        // Rebuild sorted_container_keys from containers, filtering by running state and search term
        self.sorted_container_keys = self
            .containers
            .keys()
            .filter(|key| {
                // First filter by running state
                let passes_state_filter = if self.show_all_containers {
                    true // Show all containers
                } else {
                    // Only show running containers
                    self.containers
                        .get(key)
                        .map(|c| c.state == ContainerState::Running)
                        .unwrap_or(false)
                };

                if !passes_state_filter {
                    return false;
                }

                // Then filter by search term if present
                if has_search_filter {
                    if let Some(container) = self.containers.get(key) {
                        // Search in name, id, and host_id (case-insensitive)
                        let name_matches = container.name.to_lowercase().contains(&search_filter);
                        let id_matches = container.id.to_lowercase().contains(&search_filter);
                        let host_matches =
                            container.host_id.to_lowercase().contains(&search_filter);

                        name_matches || id_matches || host_matches
                    } else {
                        false
                    }
                } else {
                    true // No search filter, include container
                }
            })
            .cloned()
            .collect();

        let direction = self.sort_state.direction;

        match self.sort_state.field {
            SortField::Uptime => {
                self.sorted_container_keys.sort_by(|a, b| {
                    let container_a = self.containers.get(a).unwrap();
                    let container_b = self.containers.get(b).unwrap();

                    // First by host_id
                    match container_a.host_id.cmp(&container_b.host_id) {
                        std::cmp::Ordering::Equal => {
                            // Then by creation time
                            let ord = match (&container_a.created, &container_b.created) {
                                (Some(a_time), Some(b_time)) => a_time.cmp(b_time),
                                (Some(_), None) => std::cmp::Ordering::Greater,
                                (None, Some(_)) => std::cmp::Ordering::Less,
                                (None, None) => std::cmp::Ordering::Equal,
                            };
                            // Reverse if descending
                            if direction == SortDirection::Descending {
                                ord.reverse()
                            } else {
                                ord
                            }
                        }
                        other => other,
                    }
                });
            }
            SortField::Name => {
                self.sorted_container_keys.sort_by(|a, b| {
                    let container_a = self.containers.get(a).unwrap();
                    let container_b = self.containers.get(b).unwrap();

                    // First by host_id
                    match container_a.host_id.cmp(&container_b.host_id) {
                        std::cmp::Ordering::Equal => {
                            let ord = container_a.name.cmp(&container_b.name);
                            // Reverse if descending
                            if direction == SortDirection::Descending {
                                ord.reverse()
                            } else {
                                ord
                            }
                        }
                        other => other,
                    }
                });
            }
            SortField::Cpu => {
                self.sorted_container_keys.sort_by(|a, b| {
                    let container_a = self.containers.get(a).unwrap();
                    let container_b = self.containers.get(b).unwrap();

                    // First by host_id
                    match container_a.host_id.cmp(&container_b.host_id) {
                        std::cmp::Ordering::Equal => {
                            let ord = container_a
                                .stats
                                .cpu
                                .partial_cmp(&container_b.stats.cpu)
                                .unwrap_or(std::cmp::Ordering::Equal);
                            // Reverse if descending
                            if direction == SortDirection::Descending {
                                ord.reverse()
                            } else {
                                ord
                            }
                        }
                        other => other,
                    }
                });
            }
            SortField::Memory => {
                self.sorted_container_keys.sort_by(|a, b| {
                    let container_a = self.containers.get(a).unwrap();
                    let container_b = self.containers.get(b).unwrap();

                    // First by host_id
                    match container_a.host_id.cmp(&container_b.host_id) {
                        std::cmp::Ordering::Equal => {
                            let ord = container_a
                                .stats
                                .memory
                                .partial_cmp(&container_b.stats.memory)
                                .unwrap_or(std::cmp::Ordering::Equal);
                            // Reverse if descending
                            if direction == SortDirection::Descending {
                                ord.reverse()
                            } else {
                                ord
                            }
                        }
                        other => other,
                    }
                });
            }
        }
    }
}
