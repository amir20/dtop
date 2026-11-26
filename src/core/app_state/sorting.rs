use crate::core::app_state::AppState;
use crate::core::types::{
    Column, Container, ContainerState, ContainerStats, HealthStatus, RenderAction, SortDirection,
    SortState, ViewState,
};
use chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::time::Duration;

/// Minimum time between sorts to avoid re-sorting on every frame
const SORT_THROTTLE_DURATION: Duration = Duration::from_secs(3);

/// Ranks a container by how much attention it needs, for the Status sort.
/// Lower is worse, so the default (ascending) direction lists problems first.
///
/// Health only counts while the container runs: an exited container keeps its
/// last health status, which no longer says anything about it.
/// The Uptime column's sort key: when a running container last started.
/// Stopped containers have no uptime and sort as `None`, like an unknown start.
fn uptime_since(container: &Container) -> Option<DateTime<Utc>> {
    container
        .started
        .filter(|_| container.state == ContainerState::Running)
}

/// Compares a stats column. Stats of a container that isn't running are stale
/// (the list blanks them), so it sorts as no value, like `uptime_since`.
fn cmp_live_stats(
    a: &Container,
    b: &Container,
    cmp: impl FnOnce(&ContainerStats, &ContainerStats) -> Ordering,
) -> Ordering {
    let live = |c: &Container| c.state == ContainerState::Running;
    match (live(a), live(b)) {
        (true, true) => cmp(&a.stats, &b.stats),
        (a_live, b_live) => a_live.cmp(&b_live),
    }
}

fn status_severity(container: &Container) -> u8 {
    match container.state {
        ContainerState::Running => match container.health {
            Some(HealthStatus::Unhealthy) => 0,
            Some(HealthStatus::Starting) => 4,
            Some(HealthStatus::Healthy) | None => 5,
        },
        ContainerState::Restarting => 1,
        ContainerState::Dead => 2,
        ContainerState::Exited => 3,
        ContainerState::Removing => 6,
        ContainerState::Paused => 7,
        ContainerState::Created => 8,
        ContainerState::Unknown => 9,
    }
}

/// Orders two containers for the list: by the sort field in its direction,
/// then by host so the order is stable.
pub fn compare_containers(a: &Container, b: &Container, sort: SortState) -> Ordering {
    let ord = match sort.field {
        // `Option`'s ordering already places `None` before `Some`, which
        // matches the previous hand-written match exactly.
        Column::Uptime => uptime_since(a).cmp(&uptime_since(b)),
        Column::Created => a.created.cmp(&b.created),
        Column::Name => a.name.cmp(&b.name),
        Column::Id => a.id.cmp(&b.id),
        Column::Host => a.host_id.cmp(&b.host_id),
        Column::Compose => a.compose_project.cmp(&b.compose_project),
        Column::Image => a.image.cmp(&b.image),
        Column::Ports => a.ports.cmp(&b.ports),
        // `total_cmp` gives a deterministic total order over floats
        // (including NaN) without needing to unwrap `partial_cmp`.
        Column::Cpu => cmp_live_stats(a, b, |a, b| a.cpu.total_cmp(&b.cpu)),
        Column::Memory => cmp_live_stats(a, b, |a, b| a.memory.total_cmp(&b.memory)),
        Column::Pids => cmp_live_stats(a, b, |a, b| a.pids_current.cmp(&b.pids_current)),
        Column::NetTx => cmp_live_stats(a, b, |a, b| {
            a.network_tx_bytes_per_sec
                .total_cmp(&b.network_tx_bytes_per_sec)
        }),
        Column::NetRx => cmp_live_stats(a, b, |a, b| {
            a.network_rx_bytes_per_sec
                .total_cmp(&b.network_rx_bytes_per_sec)
        }),
        Column::DiskRead => cmp_live_stats(a, b, |a, b| {
            a.disk_read_bytes_per_sec
                .total_cmp(&b.disk_read_bytes_per_sec)
        }),
        Column::DiskWrite => cmp_live_stats(a, b, |a, b| {
            a.disk_write_bytes_per_sec
                .total_cmp(&b.disk_write_bytes_per_sec)
        }),
        Column::Status => status_severity(a).cmp(&status_severity(b)),
        Column::Restarts => a.restart_count.cmp(&b.restart_count),
    };
    let ord = if sort.direction == SortDirection::Descending {
        ord.reverse()
    } else {
        ord
    };
    // Tiebreakers for a stable order (the containers come from a HashMap)
    ord.then_with(|| a.host_id.cmp(&b.host_id))
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.id.cmp(&b.id))
}

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

        // Adjust selection if needed after filtering. Toggling back can refill a
        // list that emptied out (and so cleared the selection), so this has to
        // restore the cursor rather than only clamp it.
        self.ensure_selection();

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

        // Rows are selected by index, so remember which container is selected
        // and follow it to its new row
        let selected_key = self
            .table_state
            .selected()
            .and_then(|index| self.sorted_container_keys.get(index))
            .cloned();

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

        let sort_state = self.sort_state;
        key_container_pairs.sort_by(|(_, a), (_, b)| compare_containers(a, b, sort_state));

        // Extract sorted keys
        self.sorted_container_keys = key_container_pairs
            .into_iter()
            .map(|(key, _)| key.clone())
            .collect();

        match selected_key.and_then(|key| self.sorted_container_keys.iter().position(|k| *k == key))
        {
            Some(index) => self.table_state.select(Some(index)),
            // It went away (or nothing was selected): keep the row in range
            None => self.clamp_selection(),
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::core::types::{AppEvent, ColumnConfig, ContainerKey};
    use std::collections::HashMap;
    use tokio::sync::mpsc;

    pub(in crate::core::app_state) fn state(sort: Column) -> AppState {
        let (tx, _rx) = mpsc::channel(10);
        AppState::new(
            HashMap::new(),
            tx,
            false,
            sort,
            None,
            ColumnConfig::default(),
            None,
        )
    }

    pub(in crate::core::app_state) fn container(
        id: &str,
        state: ContainerState,
        cpu: f64,
    ) -> Container {
        Container {
            id: id.to_string(),
            name: id.to_string(),
            state,
            health: None,
            created: None,
            started: None,
            stats: ContainerStats {
                cpu,
                ..Default::default()
            },
            host_id: "local".to_string(),
            dozzle_url: None,
            restart_count: None,
            compose_project: None,
            image: None,
            ports: None,
        }
    }

    fn selected(state: &AppState) -> Option<&str> {
        let index = state.table_state.selected()?;
        Some(&state.sorted_container_keys[index].container_id)
    }

    /// The cursor stays on its container when another one sorts above it
    #[test]
    fn selection_follows_the_container_when_the_list_resorts() {
        let mut state = state(Column::Name);
        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![
                container("api", ContainerState::Running, 0.0),
                container("web", ContainerState::Running, 0.0),
            ],
        ));
        state.table_state.select(Some(0));
        assert_eq!(selected(&state), Some("api"));

        state.handle_event(AppEvent::ContainerCreated(container(
            "aaa",
            ContainerState::Running,
            0.0,
        )));
        assert_eq!(selected(&state), Some("api"));

        // A row above it going away doesn't move it either
        state.handle_event(AppEvent::ContainerDestroyed(ContainerKey::new(
            "local".to_string(),
            "aaa".to_string(),
        )));
        assert_eq!(selected(&state), Some("api"));
    }

    /// A stopped container's last stats don't put it above running ones
    #[test]
    fn stats_sorts_ignore_stopped_containers() {
        let running = container("run", ContainerState::Running, 5.0);
        let stopped = container("old", ContainerState::Exited, 90.0);
        let sort = SortState::new(Column::Cpu); // descending
        assert_eq!(compare_containers(&running, &stopped, sort), Ordering::Less);
        let ascending = SortState {
            direction: SortDirection::Ascending,
            ..sort
        };
        assert_eq!(
            compare_containers(&running, &stopped, ascending),
            Ordering::Greater
        );
    }

    /// Equal sort keys on one host fall back to name, then id
    #[test]
    fn ties_break_by_name_then_id() {
        let sort = SortState::new(Column::Cpu);
        let a = container("b-id", ContainerState::Running, 1.0);
        let mut b = container("a-id", ContainerState::Running, 1.0);
        b.name = "zeta".to_string();
        assert_eq!(compare_containers(&a, &b, sort), Ordering::Less);
        b.name = a.name.clone();
        assert_eq!(compare_containers(&a, &b, sort), Ordering::Greater);
    }

    /// Pressing '/' again starts a fresh search with the old filter lifted
    #[test]
    fn reentering_search_clears_the_applied_filter() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = state(Column::Name);
        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![
                container("api", ContainerState::Running, 0.0),
                container("web", ContainerState::Running, 0.0),
            ],
        ));
        let press = |state: &mut AppState, code| {
            state.handle_event(AppEvent::KeyInput(KeyEvent::new(code, KeyModifiers::NONE)));
        };
        press(&mut state, KeyCode::Char('/'));
        press(&mut state, KeyCode::Char('w'));
        press(&mut state, KeyCode::Enter);
        assert_eq!(state.sorted_container_keys.len(), 1);

        press(&mut state, KeyCode::Char('/'));
        assert_eq!(state.sorted_container_keys.len(), 2);
        assert!(state.table_state.selected().is_some());
    }
}
