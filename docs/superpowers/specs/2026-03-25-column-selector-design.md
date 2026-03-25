# Column Selector Design

## Summary

Add a popup dialog that lets users show/hide and reorder table columns in the container list view. Changes apply at runtime and can optionally be persisted to the config file.

## Trigger

Press `F` in the container list view to open the column selector popup.

## Available Columns

All existing columns are available in the selector:

| Column ID    | Default Label | Default Visible | Notes |
|-------------|---------------|-----------------|-------|
| `status`    | (icon)        | Yes             | Single-character state/health icon |
| `name`      | Name          | Yes             | **Cannot be hidden** — always visible |
| `id`        | ID            | Yes             | 12-char truncated container ID |
| `host`      | Host          | Yes             | Only rendered when multiple hosts connected (existing behavior preserved) |
| `cpu`       | CPU %         | Yes             | Progress bar + percentage |
| `memory`    | Memory %      | Yes             | Progress bar + used/limit |
| `net_tx`    | Net TX        | Yes             | Network transmit rate |
| `net_rx`    | Net RX        | Yes             | Network receive rate |
| `uptime`    | Uptime        | Yes             | Time since container creation |

## Popup UI

Centered popup, styled consistently with the existing help popup:

```
┌─ Columns ─────────────────────────────┐
│  Re-order: <PageUp> / <PageDown>      │
│                                       │
│  Status Icon              [X]         │
│  Name                     [X]         │
│  ID                       [X]         │
│  Host                     [X]         │
│  CPU %                    [X]         │
│  Memory %                 [X]         │
│  Net TX                   [X]         │
│  Net RX                   [X]         │
│  Uptime                   [X]         │
└───────────────────────────────────────┘
```

- Selected row highlighted (same highlight style as action menu)
- `[X]` = visible, `[ ]` = hidden
- "Name" row always shows `[X]` and toggling is a no-op

## Controls

| Key | Action |
|-----|--------|
| `↑` / `↓` (or `k` / `j`) | Navigate between columns |
| `Enter` or `Space` | Toggle column visibility |
| `PageUp` / `PageDown` | Reorder selected column up/down |
| `Esc` | Close popup (prompts to save if changes were made) |

## Save-on-Close Flow

When the user presses `Esc` and column config has changed from its state when the popup was opened:

1. A confirmation line appears at the bottom of the popup: `Save to config? (y/n/esc)`
2. `y` — persist to config file, close popup
3. `n` — keep changes for this session only, close popup
4. `Esc` — cancel close, return to column selector

If no changes were made, `Esc` closes immediately.

## Config File Format

New top-level `columns` key in the YAML config:

```yaml
columns:
  - status
  - name
  - id
  - cpu
  - memory
  - net_tx
  - net_rx
  # uptime and host omitted = hidden
```

- Column order in the list determines display order
- Omitted columns are hidden
- If `columns` key is absent, all columns are shown in default order
- The `host` column in config controls whether it *can* be shown; the existing multi-host logic still gates actual rendering

## Config Persistence

When saving, the application:

1. Reads the existing config file (or determines the write path as `~/.config/dtop/config.yaml` if no config exists)
2. Updates/adds only the `columns` key, preserving all other config
3. Writes the file back

If no config file exists and the user saves, create `~/.config/dtop/config.yaml` with just the `columns` key.

## Data Model Changes

### New Types (in `core/types.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    Status,
    Name,
    Id,
    Host,
    Cpu,
    Memory,
    NetTx,
    NetRx,
    Uptime,
}
```

A `Column` has a `label()` method returning the display name and a `default_order()` returning the default position.

### New Struct: `ColumnConfig`

```rust
pub struct ColumnConfig {
    /// Ordered list of all columns with visibility
    pub columns: Vec<(Column, bool)>,
}
```

Methods:
- `visible_columns(&self) -> Vec<Column>` — returns only visible columns in order
- `toggle(&mut self, index: usize)` — toggles visibility (no-op for Name)
- `move_up(&mut self, index: usize)` / `move_down(&mut self, index: usize)` — reorder
- `default() -> Self` — all columns visible in default order

### AppState Changes

Add to `AppState`:
- `column_config: ColumnConfig` — current column configuration
- `column_config_snapshot: Option<ColumnConfig>` — snapshot taken when popup opens (for change detection)

### ViewState Changes

Add new variant:
- `ViewState::ColumnSelector` — column selector popup is open

Add to track selector state:
- `column_selector_state: ListState` — selected row in the column selector
- `column_save_prompt: bool` — whether the save confirmation is showing

## Event Changes

New `AppEvent` variants:
- `ToggleColumnSelector` — `F` key pressed, open/close popup
- `ToggleColumn` — `Enter`/`Space` in popup, toggle selected column
- `MoveColumnUp` / `MoveColumnDown` — `PageUp`/`PageDown` in popup
- `ConfirmSaveColumns(bool)` — `y`/`n` response to save prompt

## UI Rendering

### New file: `src/ui/column_selector.rs`

Renders the column selector popup, similar to `help.rs`:
- 50% width, 60% height, centered
- Block border with cyan title "Columns"
- Instruction line: "Re-order: <PageUp> / <PageDown>"
- List of columns with highlight and checkbox
- Optional save prompt line at bottom

### Changes to `src/ui/container_list.rs`

- Read `column_config.visible_columns()` instead of hardcoded column list
- Build header row and data rows dynamically based on visible columns
- Column width constraints remain per-column but only applied for visible columns

### Changes to `src/ui/render.rs`

- Add rendering branch for `ViewState::ColumnSelector` (renders container list with popup overlay, same pattern as help)

## Input Handling

### Changes to `src/core/app_state/mod.rs`

Add key handling for `ViewState::ColumnSelector`:
- `↑/↓/k/j` — navigate
- `Enter/Space` — toggle
- `PageUp/PageDown` — reorder
- `Esc` — close (with save prompt if changed)
- `y/n` — when save prompt is showing

### New module: `src/core/app_state/columns.rs`

Handles column selector events, following the existing modular pattern (like `actions.rs`, `sorting.rs`).

## Testing

- Unit tests for `ColumnConfig`: toggle, reorder, visible_columns, move boundaries
- Unit tests for config serialization/deserialization of columns
- Snapshot tests for column selector popup rendering
- Snapshot tests for container list with hidden columns
