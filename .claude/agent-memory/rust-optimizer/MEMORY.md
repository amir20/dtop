# dtop Codebase Knowledge

## Architecture
- Event-driven TUI: Tokio runtime, mpsc channels, Ratatui rendering at 500ms
- Multi-host Docker monitoring via Bollard (SSH, TCP, TLS, local)
- One `container_manager` async task per host, one stats stream per container
- Keyboard input runs on a blocking OS thread (crossterm polling at 200ms)
- AppState is modular: actions, container_events, integrations, log_view, navigation, search, sorting

## Key Performance Patterns
- Styles pre-allocated in `UiStyles` (not per-frame)
- Sort throttled to every 3s (`SORT_THROTTLE_DURATION`), force_sort bypasses
- Log text parsed at arrival (ANSI, JSON), cached as `Text<'static>` in `LogEntry`
- Log view only formats visible lines (window slicing in `render_log_view`)
- Stats smoothed with EMA (alpha=0.3)

## Known Issues Identified (2026-02-15 full review)
1. **Keyboard worker sends duplicate events**: Every key sends SearchKeyEvent AND the specific event. In SearchMode, both get processed. The sort/navigation handlers guard with `view_state != ContainerList`, but keys like 'q' send Quit unconditionally -- typing 'q' in search exits the app.
2. **Sorting does double HashMap lookups**: `sort_containers_internal` iterates keys, calls `self.containers.get(key)` in filter, then clones keys, then looks up again in sort comparators -- each comparator does `self.containers.get().unwrap()`.
3. **Per-frame allocations in render**: `format!()` for progress bars, `String::new()` for empty fields, `Vec<Row>` collected per render. Header row creates `String` per column.
4. **`unwrap()` in sort comparators**: `self.containers.get(a).unwrap()` can panic if container removed between filter and sort (race with event processing).
5. **`LogEntry::clone()` on every log line**: `handle_log_line` clones the LogEntry (which contains `Text<'static>` with heap-allocated spans).
6. **`Formatter::new()` created per container per render**: `format_time_elapsed` creates a new `timeago::Formatter` on every call.
7. **`truncate_string` doesn't handle multi-byte chars**: byte slicing `&s[..max_len-1]` can panic on multi-byte UTF-8.

## File Layout
- `src/main.rs` - Entry, event loop, terminal setup
- `src/core/types.rs` - AppEvent, Container, ContainerKey, ViewState, SortField
- `src/core/app_state/mod.rs` - Central state, handle_event dispatch
- `src/core/app_state/sorting.rs` - sort_containers_internal with throttling
- `src/docker/connection.rs` - DockerHost, container_manager, connect_docker
- `src/docker/stats.rs` - stream_container_stats with EMA smoothing
- `src/docker/logs.rs` - LogEntry::parse, stream/fetch_older_logs
- `src/ui/render.rs` - render_ui, UiStyles, search/error overlays
- `src/ui/container_list.rs` - Table rendering with progress bars
- `src/ui/log_view.rs` - Log viewer with scrollbar + pagination
- `src/ui/input.rs` - keyboard_worker (blocking thread)
