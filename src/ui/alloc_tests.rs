//! Allocation-regression tests for the rendering hot path.
//!
//! `dtop` re-renders the whole UI on a timer (every 500ms) and on every event.
//! The container-list render is the hottest path, so we measure how many heap
//! allocations a single steady-state frame performs and assert an upper bound.
//! This both documents the current cost and prevents regressions.
//!
//! The measurement uses the test-only `CountingAllocator` (see
//! `src/alloc_counter.rs`) which counts allocations on the current thread.
//!
//! This module is already gated behind `#[cfg(test)]` at its declaration in
//! `src/ui/mod.rs`, so no inner `#[cfg(test)]` is needed.

mod tests {
    use crate::alloc_counter::count_allocations;
    use crate::core::app_state::AppState;
    use crate::core::types::{
        Column, ColumnConfig, Container, ContainerKey, ContainerState, ContainerStats,
    };
    use crate::ui::render::{UiStyles, render_ui};
    use chrono::Utc;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::HashMap;
    use tokio::sync::mpsc;

    fn make_container(i: usize, host: &str) -> Container {
        Container {
            id: format!("container{i:08}"),
            name: format!("service-{i}"),
            state: ContainerState::Running,
            health: None,
            created: Some(Utc::now() - chrono::Duration::hours(i as i64 + 1)),
            stats: ContainerStats {
                cpu: (i as f64 * 3.7) % 100.0,
                memory: (i as f64 * 5.1) % 100.0,
                memory_used_bytes: (i as u64 + 1) * 12_345_678,
                memory_limit_bytes: 2_000_000_000,
                network_tx_bytes_per_sec: (i as f64) * 1024.0,
                network_rx_bytes_per_sec: (i as f64) * 2048.0,
            },
            host_id: host.to_string(),
            dozzle_url: None,
            restart_count: Some(i as i64 % 4),
            compose_project: Some(format!("project-{}", i % 3)),
        }
    }

    fn build_state(count: usize, hosts: &[&str]) -> AppState {
        // Containers are assigned round-robin via `hosts[i % hosts.len()]`,
        // which would divide by zero on an empty slice.
        assert!(!hosts.is_empty(), "build_state requires at least one host");
        let (tx, _rx) = mpsc::channel(100);
        let mut state = AppState::new(
            HashMap::new(),
            tx,
            false,
            Column::Uptime,
            ColumnConfig::default(),
            None,
        );

        for i in 0..count {
            let host = hosts[i % hosts.len()];
            let container = make_container(i, host);
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
        }
        state.force_sort_containers();
        state.table_state.select(Some(0));
        state
    }

    /// Renders one steady-state frame and returns the allocation count.
    ///
    /// The first render is a warm-up: it lets `Terminal` allocate its double
    /// buffers and lets any lazily-initialized caches populate. We then measure
    /// a second identical render, which represents the steady-state cost.
    fn measure_render(state: &mut AppState, w: u16, h: u16) -> u64 {
        let styles = UiStyles::default();
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        // Warm-up render.
        terminal.draw(|f| render_ui(f, state, &styles)).unwrap();

        // Measured render (identical content, same terminal/buffers).
        count_allocations(|| {
            terminal.draw(|f| render_ui(f, state, &styles)).unwrap();
        })
    }

    // About the allocation bounds asserted below.
    //
    // These bounds count Rust-level calls into the global allocator, which is
    // a function of the code and the `ratatui`/`std` versions, not of the host
    // OS or system allocator — so they are stable across Linux/macOS for a
    // given toolchain, but expect to re-baseline them after a `ratatui` or Rust
    // upgrade changes the widget tree or `Vec`/format internals.
    //
    // Last measured (dtop's pinned `ratatui`, Rust on aarch64-darwin):
    //   single-host  = 1271 (bound 1300)
    //   multi-host   = 1365 (bound 1400)
    //   per-container = 41.5 (bound 42)
    // The headroom is deliberately tight so a real regression trips the test;
    // if a routine toolchain bump pushes a value just over, re-measure (the
    // `println!`s report the live numbers) and lift the bound to match.

    #[test]
    fn render_container_list_steady_state_allocations() {
        let mut state = build_state(30, &["local"]);
        let allocs = measure_render(&mut state, 140, 40);
        println!("single-host steady-state render (30 containers) allocations: {allocs}");

        // Upper bound guarding against regressions. The bulk of this number is
        // structural: ratatui's immediate-mode `Table`/`Row`/`Cell`/`Text`
        // widgets allocate ~2 Vecs per cell every frame (see
        // `render_marginal_allocations_per_container`). Per-frame constant
        // overhead (host-column detection, visible-column list) is now
        // allocation-free.
        assert!(
            allocs <= 1300,
            "steady-state render allocated {allocs} times (expected <= 1300)"
        );
    }

    #[test]
    fn render_multi_host_steady_state_allocations() {
        let mut state = build_state(30, &["local", "user@server1", "root@10.0.0.5"]);
        let allocs = measure_render(&mut state, 160, 40);
        println!("multi-host steady-state render (30 containers) allocations: {allocs}");

        assert!(
            allocs <= 1400,
            "multi-host steady-state render allocated {allocs} times (expected <= 1400)"
        );
    }

    /// Measures the marginal allocation cost of each additional rendered
    /// container by comparing renders at two container counts in a viewport
    /// tall enough to show them all.
    ///
    /// This isolates the per-row cost from the per-frame constant overhead and
    /// guards it against regression. The remaining per-row cost is dominated by
    /// ratatui's widget tree (one `Cell`/`Text`/`Line` per column per row);
    /// the per-frame constant overhead has been driven to zero.
    #[test]
    fn render_marginal_allocations_per_container() {
        let mut small = build_state(10, &["local"]);
        let mut large = build_state(80, &["local"]);

        let small_allocs = measure_render(&mut small, 140, 100);
        let large_allocs = measure_render(&mut large, 140, 100);

        let per_container = (large_allocs - small_allocs) as f64 / 70.0;
        println!(
            "render allocations: 10 containers={small_allocs}, 80 containers={large_allocs} \
             ({per_container:.1} allocs/container)"
        );

        assert!(
            per_container <= 42.0,
            "per-container render allocation cost regressed to {per_container:.1} (expected <= 42)"
        );
    }
}
