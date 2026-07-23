# Disk I/O Support Feature Design

## Overview

Add disk I/O (block I/O) monitoring to dtop, similar to what [ctop](https://github.com/bcicen/ctop) provides. This will display read/write bytes per second for each container.

## Docker Stats API

The Docker Stats API provides block I/O data via `blkio_stats.io_service_bytes_recursive`, which contains entries with:
- `major`/`minor`: device identifiers
- `op`: operation type ("Read", "Write", "Sync", "Async")
- `value`: cumulative bytes transferred

### Bollard Types (v0.21.0)

```rust
pub struct ContainerBlkioStats {
    pub io_service_bytes_recursive: Option<Vec<ContainerBlkioStatEntry>>,
    // ... other cgroups v1 fields (often null on cgroups v2)
}

pub struct ContainerBlkioStatEntry {
    pub major: Option<u64>,
    pub minor: Option<u64>,
    pub op: Option<String>,    // "Read", "Write", "Sync", "Async"
    pub value: Option<u64>,    // cumulative bytes
}
```

The `ContainerStatsResponse` struct has a `blkio_stats: Option<ContainerBlkioStats>` field.

### Known Limitation

Per [moby/moby#35352](https://github.com/moby/moby/issues/35352), blkio stats may return zeros on some systems (especially with certain cgroup configurations). The implementation should handle this gracefully by showing 0 B/s or hiding the columns when no data is available.

## Implementation Plan

### 1. Update `ContainerStats` struct (`src/core/types.rs`)

Add new fields for disk I/O rates:

```rust
#[derive(Clone, Debug, Default)]
pub struct ContainerStats {
    pub cpu: f64,
    pub memory: f64,
    pub memory_used_bytes: u64,
    pub memory_limit_bytes: u64,
    pub network_tx_bytes_per_sec: f64,
    pub network_rx_bytes_per_sec: f64,
    // NEW FIELDS:
    pub disk_read_bytes_per_sec: f64,
    pub disk_write_bytes_per_sec: f64,
}
```

### 2. Add Column variants (`src/core/types.rs`)

Add new column types to the `Column` enum:

```rust
pub enum Column {
    // ... existing columns ...
    DiskRead,
    DiskWrite,
}
```

Update associated methods:
- `label()` → "Disk R", "Disk W"
- `id()` → "disk_read", "disk_write"
- `from_id()` → handle new IDs: `"disk_read" => Some(Column::DiskRead)`, `"disk_write" => Some(Column::DiskWrite)`
- `all_default()` → add new columns at the end (after NetRx, before Uptime or after Restarts)
- `default_visible()` → return `false` for disk columns (add to existing match: `Column::Restarts | Column::Compose | Column::DiskRead | Column::DiskWrite`)
- `default_sort_direction()` → `Descending` for both
- `sort_label()` → "Disk Read", "Disk Write"
- `from_sort_str()` → add short aliases if desired (e.g., "dr"/"dw" or "disk_read"/"disk_write")

### 3. Implement blkio extraction in `src/docker/stats.rs`

Follow the existing pattern for network I/O:

```rust
/// Extracts total disk bytes (read, write) from container stats
fn extract_disk_bytes(stats: &ContainerStatsResponse) -> (Option<u64>, Option<u64>) {
    let blkio_stats = match &stats.blkio_stats {
        Some(bs) => bs,
        None => return (None, None),
    };

    let entries = match &blkio_stats.io_service_bytes_recursive {
        Some(e) => e,
        None => return (None, None),
    };

    let mut total_read = 0u64;
    let mut total_write = 0u64;

    for entry in entries {
        let value = entry.value.unwrap_or(0);
        match entry.op.as_deref() {
            Some("Read") => total_read += value,
            Some("Write") => total_write += value,
            _ => {}
        }
    }

    (Some(total_read), Some(total_write))
}

/// Calculates disk I/O rates in bytes per second
fn calculate_disk_rates(
    stats: &ContainerStatsResponse,
    prev_read: Option<u64>,
    prev_write: Option<u64>,
    prev_time: Option<Instant>,
) -> (f64, f64) {
    let (current_read, current_write) = extract_disk_bytes(stats);

    let (prev_read, prev_write, prev_time) = match (prev_read, prev_write, prev_time) {
        (Some(r), Some(w), Some(t)) => (r, w, t),
        _ => return (0.0, 0.0),
    };

    let (current_read, current_write) = match (current_read, current_write) {
        (Some(r), Some(w)) => (r, w),
        _ => return (0.0, 0.0),
    };

    let elapsed = prev_time.elapsed().as_secs_f64();
    if elapsed <= 0.0 {
        return (0.0, 0.0);
    }

    let read_delta = current_read.saturating_sub(prev_read) as f64;
    let write_delta = current_write.saturating_sub(prev_write) as f64;

    (read_delta / elapsed, write_delta / elapsed)
}
```

Update `stream_container_stats()` to:
1. Add tracking variables: `prev_disk_read`, `prev_disk_write` (reuse existing `prev_timestamp`)
2. Add smoothed values: `smoothed_disk_read`, `smoothed_disk_write`
3. Call `calculate_disk_rates()` and apply EMA smoothing using existing `ema()` helper (alpha=0.3)
4. Populate new `ContainerStats` fields

**Note:** The existing `prev_timestamp` is shared between network and disk I/O since both use the same stats poll interval.

### 4. Update UI rendering (`src/ui/container_list.rs`)

Add column rendering in `create_container_row()`:

```rust
Column::DiskRead => {
    if is_running {
        Cell::from(format_bytes_per_sec(container.stats.disk_read_bytes_per_sec))
    } else {
        Cell::from("")
    }
}
Column::DiskWrite => {
    if is_running {
        Cell::from(format_bytes_per_sec(container.stats.disk_write_bytes_per_sec))
    } else {
        Cell::from("")
    }
}
```

Add header labels in `create_header_row()`:
```rust
Column::DiskRead => "Disk R",
Column::DiskWrite => "Disk W",
```

Add column constraints in `create_table()`:
```rust
Column::DiskRead => Constraint::Length(12),
Column::DiskWrite => Constraint::Length(12),
```

### 5. Update sorting (`src/core/app_state/sorting.rs`)

Add comparison logic for the new columns in `sort_containers_internal()`, following the existing pattern for NetTx/NetRx:

```rust
Column::DiskRead => a
    .stats
    .disk_read_bytes_per_sec
    .total_cmp(&b.stats.disk_read_bytes_per_sec),
Column::DiskWrite => a
    .stats
    .disk_write_bytes_per_sec
    .total_cmp(&b.stats.disk_write_bytes_per_sec),
```

Use `total_cmp()` for deterministic float ordering (handles NaN consistently).

### 6. Update tests

#### Unit tests for stats extraction (`src/docker/stats.rs`)
- Add unit tests for `extract_disk_bytes()` with various blkio_stats scenarios
- Add unit tests for `calculate_disk_rates()` covering rate calculation and edge cases
- Test handling of None/empty blkio_stats (should return 0.0)

#### UI test helper (`src/ui/ui_tests.rs`)
Update `create_test_container()` helper to include disk I/O parameters:
```rust
fn create_test_container(
    // ... existing params ...
    disk_read: f64,
    disk_write: f64,
) -> Container {
    // ...
    stats: ContainerStats {
        // ... existing fields ...
        disk_read_bytes_per_sec: disk_read,
        disk_write_bytes_per_sec: disk_write,
    },
}
```

#### Column config tests (`src/core/types.rs`)
- Update `test_column_config_default_all_visible`: change assertion from 11 to 13 columns
- Add tests for new column IDs in `from_id()` and `id()` round-trip

#### Snapshot tests
- Run `cargo insta accept` to update column selector snapshot (will show 13 columns instead of 11)
- Existing container list snapshots should NOT change since disk columns are hidden by default

### 7. Update documentation

- Update `CLAUDE.md` with new columns and configuration options
- Update `config.example.yaml` if needed

## Files to Modify

| File | Changes |
|------|---------|
| `src/core/types.rs` | Add `disk_read_bytes_per_sec`, `disk_write_bytes_per_sec` to `ContainerStats`; add `Column::DiskRead`, `Column::DiskWrite` variants; update `label()`, `id()`, `from_id()`, `all_default()`, `default_visible()`, `default_sort_direction()`, `sort_label()`; update test assertion (11→13 columns) |
| `src/docker/stats.rs` | Add `extract_disk_bytes()` and `calculate_disk_rates()` functions; add tracking variables (`prev_disk_read`, `prev_disk_write`, `smoothed_disk_read`, `smoothed_disk_write`); reuse existing `prev_timestamp` and `ema()` helper |
| `src/ui/container_list.rs` | Add cell rendering in `create_container_row()`; add header labels ("Disk R", "Disk W") in `create_header_row()`; add `Constraint::Length(12)` in `create_table()` |
| `src/core/app_state/sorting.rs` | Add sort comparison using `total_cmp()` for `Column::DiskRead` and `Column::DiskWrite` |
| `src/ui/ui_tests.rs` | Update `create_test_container()` helper to include disk I/O parameters; run `cargo insta accept` for snapshots |
| `CLAUDE.md` | Document new columns and configuration options |

## Implementation Notes

### Shared Timestamp
The existing `prev_timestamp` variable in `stream_container_stats()` can be reused for disk I/O rate calculations since both network and disk stats come from the same Docker stats poll.

### EMA Smoothing
Reuse the existing `ema()` helper function with alpha=0.3 for consistent smoothing behavior across all rate metrics.

### Config Compatibility
- **Forward compatibility:** Old configs without disk columns will work fine - new columns appear hidden by default
- **Backward compatibility:** `Column::from_id()` returns `None` for unknown IDs, so new configs with disk columns will gracefully degrade on old versions

### Formatter Reuse
The existing `format_bytes_per_sec()` function in `src/ui/formatters.rs` can be reused directly for disk I/O rates.

## Implementation Order

The recommended order to minimize compilation errors during development:

1. **`src/core/types.rs`** — Add fields to `ContainerStats` first (breaks nothing), then add `Column` variants and all associated methods
2. **`src/docker/stats.rs`** — Add extraction and rate calculation functions, update streaming loop
3. **`src/ui/container_list.rs`** — Add rendering (cells, headers, constraints)
4. **`src/core/app_state/sorting.rs`** — Add sort comparisons
5. **`src/ui/ui_tests.rs`** — Update test helper, run `cargo insta accept`
6. **`src/core/types.rs` tests** — Fix column count assertion (11→13)
7. **Documentation** — Update CLAUDE.md

Run `cargo check` after each file to catch issues early. Run full `cargo test` after step 4.

## Effort Estimate

**Total: ~4-6 hours** for implementation and testing.

- Low complexity: existing patterns for network I/O are directly reusable
- Most changes are additive (new columns, new fields)
- Well-structured codebase with clear separation of concerns

## References

- [Docker Runtime Metrics Documentation](https://docs.docker.com/engine/containers/runmetrics/)
- [Datadog: How to Collect Docker Metrics](https://www.datadoghq.com/blog/how-to-collect-docker-metrics/)
- [moby/moby#35352 - blkio stats showing zeros](https://github.com/moby/moby/issues/35352)
- [ctop - Container metrics viewer](https://github.com/bcicen/ctop)
