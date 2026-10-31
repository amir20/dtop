---
name: perf-review-2026-10
description: Open performance findings from the 2026-10-17 full hot-path review of dtop (log view, render windowing, cgroup fallback)
metadata:
  type: project
---
Findings not yet fixed as of 2026-10-17 (verify before re-reporting):
1. Log view: `LogLine` returns RenderAction::Render -> chatty container bypasses 500ms throttle; render does O(n) `Line::width()` over all formatted_lines twice per frame (`ui/log_view.rs` total_rows + find_visible_start); `LogState` buffers unbounded; `log_entries` keeps full `Text` though only timestamps are read (`calculate_progress`).
2. Container list builds Row/Cells for every container, not just viewport (`container_list.rs` rows collect) -> ~42 allocs per off-screen row per frame.
3. `docker/stats.rs` cgroup v2 disk fallback: spawn_blocking + fs probe per container per tick, no negative cache, runs even though Disk columns are hidden by default.
**Why:** to avoid re-deriving and to track whether they were fixed.
**How to apply:** check these sites first on future perf reviews; drop entries once fixed.
