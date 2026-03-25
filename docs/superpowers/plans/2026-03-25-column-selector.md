# Column Selector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a popup dialog that lets users show/hide and reorder table columns, with optional config file persistence.

**Architecture:** New `Column` enum and `ColumnConfig` struct in `core/types.rs` define column metadata and visibility. A new `ViewState::ColumnSelector` variant and `columns.rs` handler module manage popup state. A new `ui/column_selector.rs` renders the popup. The container list rendering in `ui/container_list.rs` reads visible columns dynamically instead of hardcoding. Config persistence adds a `columns` field to `cli/config.rs`.

**Tech Stack:** Rust, Ratatui (TUI framework), serde/serde_yaml (config persistence)

---

### Task 1: Add Column enum and ColumnConfig struct

**Files:**
- Modify: `src/core/types.rs`

- [ ] **Step 1: Write tests for Column and ColumnConfig**

Add these tests at the bottom of the existing `tests` module in `src/core/types.rs`:

```rust
#[test]
fn test_column_label() {
    assert_eq!(Column::Status.label(), "Status Icon");
    assert_eq!(Column::Name.label(), "Name");
    assert_eq!(Column::Id.label(), "ID");
    assert_eq!(Column::Host.label(), "Host");
    assert_eq!(Column::Cpu.label(), "CPU %");
    assert_eq!(Column::Memory.label(), "Memory %");
    assert_eq!(Column::NetTx.label(), "Net TX");
    assert_eq!(Column::NetRx.label(), "Net RX");
    assert_eq!(Column::Uptime.label(), "Uptime");
}

#[test]
fn test_column_config_default_all_visible() {
    let config = ColumnConfig::default();
    assert_eq!(config.columns.len(), 9);
    assert!(config.columns.iter().all(|(_, visible)| *visible));
}

#[test]
fn test_column_config_visible_columns() {
    let mut config = ColumnConfig::default();
    config.columns[2] = (Column::Id, false); // Hide ID
    let visible = config.visible_columns();
    assert!(!visible.contains(&Column::Id));
    assert_eq!(visible.len(), 8);
}

#[test]
fn test_column_config_toggle() {
    let mut config = ColumnConfig::default();
    // Find the index of Id column
    let id_idx = config.columns.iter().position(|(c, _)| *c == Column::Id).unwrap();
    config.toggle(id_idx);
    assert!(!config.columns[id_idx].1);
    config.toggle(id_idx);
    assert!(config.columns[id_idx].1);
}

#[test]
fn test_column_config_toggle_name_is_noop() {
    let mut config = ColumnConfig::default();
    let name_idx = config.columns.iter().position(|(c, _)| *c == Column::Name).unwrap();
    config.toggle(name_idx);
    assert!(config.columns[name_idx].1); // Still visible
}

#[test]
fn test_column_config_move_up() {
    let mut config = ColumnConfig::default();
    // Move column at index 2 up to index 1
    config.move_up(2);
    assert_eq!(config.columns[1].0, Column::Id);
    assert_eq!(config.columns[2].0, Column::Name);
}

#[test]
fn test_column_config_move_up_at_zero_is_noop() {
    let mut config = ColumnConfig::default();
    let first = config.columns[0].0;
    config.move_up(0);
    assert_eq!(config.columns[0].0, first);
}

#[test]
fn test_column_config_move_down() {
    let mut config = ColumnConfig::default();
    let col_at_0 = config.columns[0].0;
    config.move_down(0);
    assert_eq!(config.columns[1].0, col_at_0);
}

#[test]
fn test_column_config_move_down_at_end_is_noop() {
    let mut config = ColumnConfig::default();
    let last_idx = config.columns.len() - 1;
    let last = config.columns[last_idx].0;
    config.move_down(last_idx);
    assert_eq!(config.columns[last_idx].0, last);
}

#[test]
fn test_column_config_has_changed() {
    let config1 = ColumnConfig::default();
    let mut config2 = ColumnConfig::default();
    assert!(!config1.has_changed(&config2));

    let id_idx = config2.columns.iter().position(|(c, _)| *c == Column::Id).unwrap();
    config2.toggle(id_idx);
    assert!(config1.has_changed(&config2));
}

#[test]
fn test_column_config_from_config_strings() {
    let strings = vec![
        "status".to_string(),
        "name".to_string(),
        "cpu".to_string(),
    ];
    let config = ColumnConfig::from_config_strings(&strings);
    let visible = config.visible_columns();
    assert_eq!(visible, vec![Column::Status, Column::Name, Column::Cpu]);
    // Hidden columns should still exist in the list
    assert_eq!(config.columns.len(), 9);
}

#[test]
fn test_column_config_to_config_strings() {
    let mut config = ColumnConfig::default();
    let id_idx = config.columns.iter().position(|(c, _)| *c == Column::Id).unwrap();
    config.toggle(id_idx); // Hide ID
    let strings = config.to_config_strings();
    assert!(!strings.contains(&"id".to_string()));
    assert!(strings.contains(&"name".to_string()));
}

#[test]
fn test_column_config_id() {
    assert_eq!(Column::Status.id(), "status");
    assert_eq!(Column::Name.id(), "name");
    assert_eq!(Column::Id.id(), "id");
    assert_eq!(Column::Host.id(), "host");
    assert_eq!(Column::Cpu.id(), "cpu");
    assert_eq!(Column::Memory.id(), "memory");
    assert_eq!(Column::NetTx.id(), "net_tx");
    assert_eq!(Column::NetRx.id(), "net_rx");
    assert_eq!(Column::Uptime.id(), "uptime");
}

#[test]
fn test_column_from_id() {
    assert_eq!(Column::from_id("status"), Some(Column::Status));
    assert_eq!(Column::from_id("name"), Some(Column::Name));
    assert_eq!(Column::from_id("id"), Some(Column::Id));
    assert_eq!(Column::from_id("host"), Some(Column::Host));
    assert_eq!(Column::from_id("cpu"), Some(Column::Cpu));
    assert_eq!(Column::from_id("memory"), Some(Column::Memory));
    assert_eq!(Column::from_id("net_tx"), Some(Column::NetTx));
    assert_eq!(Column::from_id("net_rx"), Some(Column::NetRx));
    assert_eq!(Column::from_id("uptime"), Some(Column::Uptime));
    assert_eq!(Column::from_id("invalid"), None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -- --test-threads=1 column`
Expected: Compilation errors (Column and ColumnConfig not defined)

- [ ] **Step 3: Implement Column enum and ColumnConfig struct**

Add the following above the existing `#[cfg(test)]` module in `src/core/types.rs`:

```rust
/// Available columns in the container list
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl Column {
    /// Returns the display label for the column selector popup
    pub fn label(self) -> &'static str {
        match self {
            Column::Status => "Status Icon",
            Column::Name => "Name",
            Column::Id => "ID",
            Column::Host => "Host",
            Column::Cpu => "CPU %",
            Column::Memory => "Memory %",
            Column::NetTx => "Net TX",
            Column::NetRx => "Net RX",
            Column::Uptime => "Uptime",
        }
    }

    /// Returns the config file identifier for this column
    pub fn id(self) -> &'static str {
        match self {
            Column::Status => "status",
            Column::Name => "name",
            Column::Id => "id",
            Column::Host => "host",
            Column::Cpu => "cpu",
            Column::Memory => "memory",
            Column::NetTx => "net_tx",
            Column::NetRx => "net_rx",
            Column::Uptime => "uptime",
        }
    }

    /// Parses a config file identifier into a Column
    pub fn from_id(id: &str) -> Option<Column> {
        match id {
            "status" => Some(Column::Status),
            "name" => Some(Column::Name),
            "id" => Some(Column::Id),
            "host" => Some(Column::Host),
            "cpu" => Some(Column::Cpu),
            "memory" => Some(Column::Memory),
            "net_tx" => Some(Column::NetTx),
            "net_rx" => Some(Column::NetRx),
            "uptime" => Some(Column::Uptime),
            _ => None,
        }
    }

    /// Returns all columns in default order
    pub fn all_default() -> Vec<Column> {
        vec![
            Column::Status,
            Column::Name,
            Column::Id,
            Column::Host,
            Column::Cpu,
            Column::Memory,
            Column::NetTx,
            Column::NetRx,
            Column::Uptime,
        ]
    }
}

/// Configuration for which columns are visible and their order
#[derive(Clone, Debug)]
pub struct ColumnConfig {
    /// Ordered list of all columns with visibility flag
    pub columns: Vec<(Column, bool)>,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            columns: Column::all_default().into_iter().map(|c| (c, true)).collect(),
        }
    }
}

impl ColumnConfig {
    /// Returns only visible columns in order
    pub fn visible_columns(&self) -> Vec<Column> {
        self.columns
            .iter()
            .filter(|(_, visible)| *visible)
            .map(|(col, _)| *col)
            .collect()
    }

    /// Toggles visibility of the column at the given index.
    /// No-op for the Name column (always visible).
    pub fn toggle(&mut self, index: usize) {
        if let Some((col, visible)) = self.columns.get_mut(index) {
            if *col != Column::Name {
                *visible = !*visible;
            }
        }
    }

    /// Moves the column at `index` up by one position.
    /// No-op if index is 0.
    pub fn move_up(&mut self, index: usize) {
        if index > 0 && index < self.columns.len() {
            self.columns.swap(index, index - 1);
        }
    }

    /// Moves the column at `index` down by one position.
    /// No-op if index is the last element.
    pub fn move_down(&mut self, index: usize) {
        if index + 1 < self.columns.len() {
            self.columns.swap(index, index + 1);
        }
    }

    /// Returns true if this config differs from another
    pub fn has_changed(&self, other: &ColumnConfig) -> bool {
        self.columns != other.columns
    }

    /// Creates a ColumnConfig from a list of column ID strings (from config file).
    /// Listed columns are visible in the given order; unlisted columns are hidden
    /// but appended at the end.
    pub fn from_config_strings(strings: &[String]) -> Self {
        let mut result: Vec<(Column, bool)> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Add listed columns as visible
        for s in strings {
            if let Some(col) = Column::from_id(s) {
                if seen.insert(col) {
                    result.push((col, true));
                }
            }
        }

        // Add remaining columns as hidden
        for col in Column::all_default() {
            if !seen.contains(&col) {
                result.push((col, false));
            }
        }

        Self { columns: result }
    }

    /// Converts visible columns to config file string representation
    pub fn to_config_strings(&self) -> Vec<String> {
        self.columns
            .iter()
            .filter(|(_, visible)| *visible)
            .map(|(col, _)| col.id().to_string())
            .collect()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -- --test-threads=1 column`
Expected: All column-related tests pass

- [ ] **Step 5: Commit**

```bash
git add src/core/types.rs
git commit -m "feat: add Column enum and ColumnConfig struct for column visibility"
```

---

### Task 2: Add ColumnSelector to ViewState and AppState

**Files:**
- Modify: `src/core/types.rs` (ViewState enum)
- Modify: `src/core/app_state/mod.rs` (AppState struct + new() + handle_key_input)

- [ ] **Step 1: Add ColumnSelector variant to ViewState**

In `src/core/types.rs`, add a new variant to the `ViewState` enum:

```rust
/// Current view state of the application
#[derive(Clone, Debug, PartialEq)]
pub enum ViewState {
    /// Viewing the container list
    ContainerList,
    /// Viewing logs for a specific container
    LogView(ContainerKey),
    /// Viewing action menu for a specific container
    ActionMenu(ContainerKey),
    /// Search mode active (editing search query)
    SearchMode,
    /// Column selector popup
    ColumnSelector,
}
```

- [ ] **Step 2: Add column state fields to AppState**

In `src/core/app_state/mod.rs`, add the following imports and fields.

Add to the imports at the top:

```rust
use crate::core::types::{
    AppEvent, Column, ColumnConfig, Container, ContainerKey, HostId, LogState, RenderAction,
    SortField, SortState, ViewState,
};
```

Add these fields to the `AppState` struct (after `search_input`):

```rust
    /// Column visibility and order configuration
    pub column_config: ColumnConfig,
    /// Snapshot of column config when popup opened (for change detection)
    pub column_config_snapshot: Option<ColumnConfig>,
    /// Selected row in column selector popup
    pub column_selector_state: ListState,
    /// Whether the save confirmation prompt is showing
    pub column_save_prompt: bool,
    /// Path to config file (for saving column config)
    pub config_path: Option<std::path::PathBuf>,
```

Update the `new()` method signature to accept `column_config` and `config_path`:

```rust
    pub fn new(
        connected_hosts: HashMap<String, DockerHost>,
        event_tx: mpsc::Sender<AppEvent>,
        show_all: bool,
        sort_field: SortField,
        column_config: ColumnConfig,
        config_path: Option<std::path::PathBuf>,
    ) -> Self {
```

And add the new fields to the `Self { ... }` initialization:

```rust
            column_config,
            column_config_snapshot: None,
            column_selector_state: ListState::default(),
            column_save_prompt: false,
            config_path,
```

- [ ] **Step 3: Add `F` key binding and ColumnSelector dispatch to handle_key_input**

In `handle_key_input` in `src/core/app_state/mod.rs`, add a new early return for ColumnSelector view state (after the SearchMode early return block, before the Ctrl modifiers block):

```rust
        // Column selector: handle its own keys
        if self.view_state == ViewState::ColumnSelector {
            return self.handle_column_selector_key(key);
        }
```

Add the `F` key binding in the main `match key.code` block (e.g., after the `'a'`/`'A'` line):

```rust
            KeyCode::Char('F') => self.handle_open_column_selector(),
```

- [ ] **Step 4: Fix compilation errors in callers of AppState::new**

Update `src/main.rs` in the `run_event_loop` function where `AppState::new` is called:

```rust
    let mut state = AppState::new(connected_hosts, tx, config.show_all, config.sort_field, config.column_config, config.config_path);
```

This requires `EventLoopConfig` to also carry `column_config` and `config_path`. Update the `EventLoopConfig` struct:

```rust
struct EventLoopConfig {
    icon_style: IconStyle,
    show_all: bool,
    sort_field: SortField,
    column_config: ColumnConfig,
    config_path: Option<std::path::PathBuf>,
}
```

Add `use core::types::ColumnConfig;` to main.rs imports.

Update the `EventLoopConfig` construction in `run_async`:

```rust
        EventLoopConfig {
            icon_style,
            show_all,
            sort_field,
            column_config,
            config_path: config_path_for_state,
        },
```

Before that block, build the column config from merged config:

```rust
    // Build column config from merged config
    let column_config = if let Some(ref cols) = merged_config.columns {
        ColumnConfig::from_config_strings(cols)
    } else {
        ColumnConfig::default()
    };

    // Save config path for potential column config persistence
    let config_path_for_state = if cli_provided { None } else { config_path };
```

Note: `config_path` variable is already defined in `run_async`. For the CLI-provided case it's `None`, for the config-loaded case it's `Some(path)`.

Update `run_event_loop` to pass these new fields:

```rust
    let mut state = AppState::new(
        connected_hosts,
        tx,
        config.show_all,
        config.sort_field,
        config.column_config,
        config.config_path,
    );
```

Also update `src/ui/ui_tests.rs` helper `create_test_app_state`:

```rust
    fn create_test_app_state() -> AppState {
        let (tx, _rx) = mpsc::channel(100);
        AppState::new(HashMap::new(), tx, false, SortField::Uptime, ColumnConfig::default(), None)
    }
```

Add `ColumnConfig` to the test imports:

```rust
    use crate::core::types::{
        Column, ColumnConfig, Container, ContainerKey, ContainerState, ContainerStats, SortField,
        ViewState,
    };
```

- [ ] **Step 5: Add `columns` field to Config struct**

In `src/cli/config.rs`, add the field to the `Config` struct:

```rust
    /// Column visibility and order configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
```

Also update **all existing test Config constructors** in `src/cli/config.rs` tests to include `columns: None`. There are many tests — add `columns: None` to every `Config { ... }` literal in the test module.

- [ ] **Step 6: Create stub for columns handler module**

Create `src/core/app_state/columns.rs` with stub methods so it compiles:

```rust
use crate::core::app_state::AppState;
use crate::core::types::{RenderAction, ViewState};

impl AppState {
    /// Opens the column selector popup
    pub(super) fn handle_open_column_selector(&mut self) -> RenderAction {
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }
        self.column_config_snapshot = Some(self.column_config.clone());
        self.view_state = ViewState::ColumnSelector;
        self.column_selector_state.select(Some(0));
        self.column_save_prompt = false;
        RenderAction::Render
    }

    /// Handles key events in the column selector popup
    pub(super) fn handle_column_selector_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> RenderAction {
        // Stub - will be implemented in Task 4
        RenderAction::None
    }
}
```

Add `mod columns;` to the module declarations in `src/core/app_state/mod.rs` (after `mod sorting;`).

- [ ] **Step 7: Handle ColumnSelector in render_ui and handle_cancel_action_menu**

In `src/ui/render.rs`, add the `ColumnSelector` case to the `match &state.view_state` block:

```rust
        ViewState::ColumnSelector => {
            // Render the container list in the background
            let unique_hosts: std::collections::HashSet<_> =
                state.containers.keys().map(|key| &key.host_id).collect();
            let show_host_column = unique_hosts.len() > 1;

            render_container_list(f, size, state, styles, show_host_column);

            // Column selector popup will be rendered in Task 3
        }
```

In `src/core/app_state/actions.rs`, add a `ColumnSelector` arm in `handle_cancel_action_menu`:

```rust
            ViewState::ColumnSelector => {
                // Handle in column selector handler
                return self.handle_column_selector_key(crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Esc,
                    crossterm::event::KeyModifiers::NONE,
                ));
            }
```

Add this arm before the existing `_ =>` arm.

- [ ] **Step 8: Verify compilation**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 9: Run existing tests**

Run: `cargo test`
Expected: All existing tests pass

- [ ] **Step 10: Commit**

```bash
git add src/core/types.rs src/core/app_state/mod.rs src/core/app_state/columns.rs src/main.rs src/ui/render.rs src/ui/ui_tests.rs src/cli/config.rs src/core/app_state/actions.rs
git commit -m "feat: add ColumnSelector view state and AppState fields"
```

---

### Task 3: Render the column selector popup

**Files:**
- Create: `src/ui/column_selector.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/ui/render.rs`

- [ ] **Step 1: Create the column selector rendering module**

Create `src/ui/column_selector.rs`:

```rust
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use crate::core::app_state::AppState;
use crate::ui::render::UiStyles;

/// Renders the column selector popup
pub fn render_column_selector(f: &mut Frame, state: &mut AppState, styles: &UiStyles) {
    let area = f.area();

    // Create a centered popup (50% width, 60% height)
    let popup_width = (area.width as f32 * 0.5).max(40.0) as u16;
    let popup_height = (area.height as f32 * 0.6).max(14.0) as u16;
    let popup_width = popup_width.min(area.width.saturating_sub(4));
    let popup_height = popup_height.min(area.height.saturating_sub(4));

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the background area
    f.render_widget(Clear, popup_area);

    // Title
    let title = if state.column_save_prompt {
        " Save to config? (y/n/esc) "
    } else {
        " Columns "
    };

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(styles.header)
        .style(Style::default().bg(Color::Black));

    f.render_widget(block, popup_area);

    // Inner area for content
    let inner_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    // Instruction line
    let instruction_area = Rect::new(inner_area.x, inner_area.y, inner_area.width, 1);
    let instruction = ratatui::widgets::Paragraph::new("  Re-order: <PageUp> / <PageDown>")
        .style(Style::default().fg(Color::Gray));
    f.render_widget(instruction, instruction_area);

    // List area (below instruction, above footer)
    let list_area = Rect::new(
        inner_area.x,
        inner_area.y + 2,
        inner_area.width,
        inner_area.height.saturating_sub(4),
    );

    // Build list items
    let list_items: Vec<ListItem> = state
        .column_config
        .columns
        .iter()
        .map(|(col, visible)| {
            let checkbox = if *visible { "[X]" } else { "[ ]" };
            let text = format!("  {:<30}{}", col.label(), checkbox);
            ListItem::new(text).style(Style::default().fg(Color::White))
        })
        .collect();

    let list = List::new(list_items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, list_area, &mut state.column_selector_state);

    // Footer
    let footer_y = popup_area.y + popup_area.height.saturating_sub(2);
    let footer_area = Rect::new(
        popup_area.x + 2,
        footer_y,
        popup_area.width.saturating_sub(4),
        1,
    );

    let footer_text = if state.column_save_prompt {
        "y: Save  n: Don't save  Esc: Cancel"
    } else {
        "Enter/Space: Toggle  Esc: Close  F: Close"
    };

    let footer = ratatui::widgets::Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);

    f.render_widget(footer, footer_area);
}
```

- [ ] **Step 2: Register the module in ui/mod.rs**

Add `pub mod column_selector;` to `src/ui/mod.rs`:

```rust
pub mod action_menu;
pub mod column_selector;
pub mod container_list;
pub mod formatters;
pub mod help;
pub mod icons;
pub mod input;
pub mod log_view;
pub mod render;
```

- [ ] **Step 3: Wire up rendering in render.rs**

In `src/ui/render.rs`, add the import at the top:

```rust
use crate::ui::column_selector::render_column_selector;
```

Update the `ViewState::ColumnSelector` arm to call the renderer:

```rust
        ViewState::ColumnSelector => {
            // Render the container list in the background
            let unique_hosts: std::collections::HashSet<_> =
                state.containers.keys().map(|key| &key.host_id).collect();
            let show_host_column = unique_hosts.len() > 1;

            render_container_list(f, size, state, styles, show_host_column);

            // Render column selector popup on top
            render_column_selector(f, state, styles);
        }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src/ui/column_selector.rs src/ui/mod.rs src/ui/render.rs
git commit -m "feat: add column selector popup rendering"
```

---

### Task 4: Implement column selector key handling

**Files:**
- Modify: `src/core/app_state/columns.rs`

- [ ] **Step 1: Implement the full key handler**

Replace the stub `handle_column_selector_key` in `src/core/app_state/columns.rs`:

```rust
use crate::core::app_state::AppState;
use crate::core::types::{ColumnConfig, RenderAction, ViewState};

impl AppState {
    /// Opens the column selector popup
    pub(super) fn handle_open_column_selector(&mut self) -> RenderAction {
        if self.view_state != ViewState::ContainerList {
            return RenderAction::None;
        }
        self.column_config_snapshot = Some(self.column_config.clone());
        self.view_state = ViewState::ColumnSelector;
        self.column_selector_state.select(Some(0));
        self.column_save_prompt = false;
        RenderAction::Render
    }

    /// Handles key events in the column selector popup
    pub(super) fn handle_column_selector_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> RenderAction {
        use crossterm::event::KeyCode;

        // If save prompt is showing, handle y/n/esc
        if self.column_save_prompt {
            return match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.save_column_config();
                    self.close_column_selector()
                }
                KeyCode::Char('n') | KeyCode::Char('N') => self.close_column_selector(),
                KeyCode::Esc => {
                    // Cancel close, go back to column selector
                    self.column_save_prompt = false;
                    RenderAction::Render
                }
                _ => RenderAction::None,
            };
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let current = self.column_selector_state.selected().unwrap_or(0);
                if current > 0 {
                    self.column_selector_state.select(Some(current - 1));
                }
                RenderAction::Render
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let current = self.column_selector_state.selected().unwrap_or(0);
                let max = self.column_config.columns.len().saturating_sub(1);
                if current < max {
                    self.column_selector_state.select(Some(current + 1));
                }
                RenderAction::Render
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(idx) = self.column_selector_state.selected() {
                    self.column_config.toggle(idx);
                }
                RenderAction::Render
            }
            KeyCode::PageUp => {
                if let Some(idx) = self.column_selector_state.selected() {
                    self.column_config.move_up(idx);
                    if idx > 0 {
                        self.column_selector_state.select(Some(idx - 1));
                    }
                }
                RenderAction::Render
            }
            KeyCode::PageDown => {
                if let Some(idx) = self.column_selector_state.selected() {
                    self.column_config.move_down(idx);
                    let max = self.column_config.columns.len().saturating_sub(1);
                    if idx < max {
                        self.column_selector_state.select(Some(idx + 1));
                    }
                }
                RenderAction::Render
            }
            KeyCode::Esc | KeyCode::Char('F') => {
                // Check if config has changed
                if let Some(ref snapshot) = self.column_config_snapshot {
                    if snapshot.has_changed(&self.column_config) {
                        // Show save prompt
                        self.column_save_prompt = true;
                        return RenderAction::Render;
                    }
                }
                self.close_column_selector()
            }
            _ => RenderAction::None,
        }
    }

    /// Closes the column selector and returns to container list
    fn close_column_selector(&mut self) -> RenderAction {
        self.view_state = ViewState::ContainerList;
        self.column_config_snapshot = None;
        self.column_selector_state.select(None);
        self.column_save_prompt = false;
        RenderAction::Render
    }

    /// Saves the current column config to the config file
    fn save_column_config(&self) {
        use std::path::PathBuf;

        let config_path = self.config_path.clone().unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
                .join("dtop")
                .join("config.yaml")
        });

        // Read existing config or create default
        let mut config: serde_yaml::Value = if config_path.exists() {
            let contents = match std::fs::read_to_string(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to read config file: {}", e);
                    return;
                }
            };
            match serde_yaml::from_str(&contents) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Failed to parse config file: {}", e);
                    return;
                }
            }
        } else {
            serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
        };

        // Update the columns key
        let columns_value: Vec<serde_yaml::Value> = self
            .column_config
            .to_config_strings()
            .into_iter()
            .map(serde_yaml::Value::String)
            .collect();

        if let serde_yaml::Value::Mapping(ref mut map) = config {
            map.insert(
                serde_yaml::Value::String("columns".to_string()),
                serde_yaml::Value::Sequence(columns_value),
            );
        }

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::error!("Failed to create config directory: {}", e);
                return;
            }
        }

        // Write config
        let yaml_string = match serde_yaml::to_string(&config) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to serialize config: {}", e);
                return;
            }
        };

        if let Err(e) = std::fs::write(&config_path, yaml_string) {
            tracing::error!("Failed to write config file: {}", e);
        } else {
            tracing::debug!("Saved column config to: {}", config_path.display());
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/core/app_state/columns.rs
git commit -m "feat: implement column selector key handling and config persistence"
```

---

### Task 5: Make container list rendering dynamic based on visible columns

**Files:**
- Modify: `src/ui/container_list.rs`

This is the most significant UI change. The container list currently hardcodes which columns to render. We need to make it read from `column_config.visible_columns()`.

- [ ] **Step 1: Refactor create_header_row to use visible columns**

Replace the `create_header_row` function:

```rust
/// Creates the table header row based on visible columns
fn create_header_row(
    styles: &UiStyles,
    visible_columns: &[Column],
    show_host_column: bool,
    sort_state: SortState,
) -> Row<'static> {
    let sort_symbol = sort_state.direction.symbol();
    let sort_field = sort_state.field;

    let headers: Vec<String> = visible_columns
        .iter()
        .filter(|col| **col != Column::Host || show_host_column)
        .map(|col| match col {
            Column::Status => "".to_string(),
            Column::Name => {
                if sort_field == SortField::Name {
                    format!("Name {}", sort_symbol)
                } else {
                    "Name".to_string()
                }
            }
            Column::Id => "ID".to_string(),
            Column::Host => "Host".to_string(),
            Column::Cpu => {
                if sort_field == SortField::Cpu {
                    format!("CPU % {}", sort_symbol)
                } else {
                    "CPU %".to_string()
                }
            }
            Column::Memory => {
                if sort_field == SortField::Memory {
                    format!("Memory % {}", sort_symbol)
                } else {
                    "Memory %".to_string()
                }
            }
            Column::NetTx => "Net TX".to_string(),
            Column::NetRx => "Net RX".to_string(),
            Column::Uptime => {
                if sort_field == SortField::Uptime {
                    format!("Created {}", sort_symbol)
                } else {
                    "Created".to_string()
                }
            }
        })
        .collect();

    Row::new(headers).style(styles.header).bottom_margin(1)
}
```

Add `use crate::core::types::Column;` to the imports at the top of `container_list.rs`.

- [ ] **Step 2: Refactor create_container_row to use visible columns**

Replace the `create_container_row` function:

```rust
/// Creates a table row for a single container based on visible columns
fn create_container_row<'a>(
    container: &'a Container,
    styles: &UiStyles,
    visible_columns: &[Column],
    show_host_column: bool,
    show_progress_bars: bool,
) -> Row<'a> {
    let is_running = container.state == ContainerState::Running;

    let cells: Vec<Cell> = visible_columns
        .iter()
        .filter(|col| **col != Column::Host || show_host_column)
        .map(|col| match col {
            Column::Id => Cell::from(container.id.as_str()),
            Column::Status => {
                let (icon, icon_style) =
                    get_status_icon(&container.state, &container.health, styles);
                Cell::from(icon).style(icon_style)
            }
            Column::Name => Cell::from(container.name.as_str()),
            Column::Host => Cell::from(container.host_id.as_str()),
            Column::Cpu => {
                if is_running {
                    let display = if show_progress_bars {
                        create_progress_bar(container.stats.cpu, 20)
                    } else {
                        format!("{:5.1}%", container.stats.cpu)
                    };
                    Cell::from(display).style(get_percentage_style(container.stats.cpu, styles))
                } else {
                    Cell::from(String::new())
                }
            }
            Column::Memory => {
                if is_running {
                    let display = if show_progress_bars {
                        create_memory_progress_bar(
                            container.stats.memory,
                            container.stats.memory_used_bytes,
                            container.stats.memory_limit_bytes,
                            20,
                        )
                    } else {
                        format!("{:5.1}%", container.stats.memory)
                    };
                    Cell::from(display).style(get_percentage_style(container.stats.memory, styles))
                } else {
                    Cell::from(String::new())
                }
            }
            Column::NetTx => {
                if is_running {
                    Cell::from(format_bytes_per_sec(
                        container.stats.network_tx_bytes_per_sec,
                    ))
                } else {
                    Cell::from(String::new())
                }
            }
            Column::NetRx => {
                if is_running {
                    Cell::from(format_bytes_per_sec(
                        container.stats.network_rx_bytes_per_sec,
                    ))
                } else {
                    Cell::from(String::new())
                }
            }
            Column::Uptime => {
                if is_running {
                    Cell::from(format_time_elapsed(container.created.as_ref()))
                } else {
                    Cell::from("N/A".to_string())
                }
            }
        })
        .collect();

    Row::new(cells)
}
```

- [ ] **Step 3: Refactor create_table to use visible columns for constraints**

Replace the `create_table` function:

```rust
/// Creates the complete table widget based on visible columns
fn create_table<'a>(
    rows: Vec<Row<'a>>,
    header: Row<'static>,
    container_count: usize,
    styles: &UiStyles,
    visible_columns: &[Column],
    show_host_column: bool,
    show_progress_bars: bool,
) -> Table<'a> {
    let cpu_width = if show_progress_bars { 28 } else { 7 };
    let mem_width = if show_progress_bars { 33 } else { 7 };

    let constraints: Vec<Constraint> = visible_columns
        .iter()
        .filter(|col| **col != Column::Host || show_host_column)
        .map(|col| match col {
            Column::Id => Constraint::Length(12),
            Column::Status => Constraint::Length(1),
            Column::Name => Constraint::Min(8),
            Column::Host => Constraint::Length(20),
            Column::Cpu => Constraint::Length(cpu_width),
            Column::Memory => Constraint::Length(mem_width),
            Column::NetTx => Constraint::Length(12),
            Column::NetRx => Constraint::Length(12),
            Column::Uptime => Constraint::Length(15),
        })
        .collect();

    Table::new(rows, constraints)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .padding(ratatui::widgets::Padding::proportional(1))
                .title(format!(
                    "dtop v{} - {} containers ('?' for help, 'q' to quit)",
                    VERSION, container_count
                ))
                .style(styles.border),
        )
        .row_highlight_style(styles.selected)
}
```

- [ ] **Step 4: Update render_container_list to pass visible columns**

Update the `render_container_list` function:

```rust
pub fn render_container_list(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    app_state: &mut AppState,
    styles: &UiStyles,
    show_host_column: bool,
) {
    let width = area.width;
    let show_progress_bars = width >= 128;

    app_state.sort_containers();

    let visible_columns = app_state.column_config.visible_columns();

    let rows: Vec<Row> = app_state
        .sorted_container_keys
        .iter()
        .filter_map(|key| app_state.containers.get(key))
        .map(|c| {
            create_container_row(
                c,
                styles,
                &visible_columns,
                show_host_column,
                show_progress_bars,
            )
        })
        .collect();

    let header = create_header_row(styles, &visible_columns, show_host_column, app_state.sort_state);
    let table = create_table(
        rows,
        header,
        app_state.sorted_container_keys.len(),
        styles,
        &visible_columns,
        show_host_column,
        show_progress_bars,
    );

    f.render_stateful_widget(table, area, &mut app_state.table_state);
}
```

- [ ] **Step 5: Verify compilation and run tests**

Run: `cargo build && cargo test`
Expected: Compiles and all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/ui/container_list.rs
git commit -m "feat: make container list rendering dynamic based on visible columns"
```

---

### Task 6: Update help popup with column selector shortcut

**Files:**
- Modify: `src/ui/help.rs`

- [ ] **Step 1: Add F key to the help text**

In `src/ui/help.rs`, update the Navigation section to include the `F` key. Change this line:

```rust
        Line::from(
            "  a           Show all containers         /      Filter         o      Open Dozzle",
        ),
```

To:

```rust
        Line::from(
            "  a           Show all containers         /      Filter         o      Open Dozzle",
        ),
        Line::from(
            "  F           Column visibility",
        ),
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/ui/help.rs
git commit -m "feat: add column selector shortcut to help popup"
```

---

### Task 7: Update snapshot tests

**Files:**
- Modify: `src/ui/ui_tests.rs`

- [ ] **Step 1: Add snapshot test for column selector popup**

Add this test to the `tests` module in `src/ui/ui_tests.rs`:

```rust
    #[test]
    fn test_column_selector_popup() {
        let mut state = create_test_app_state();

        // Add a container so the background list isn't empty
        let container = create_test_container("abc123def456", "nginx", "local", 25.0, 50.0, 1024.0, 2048.0);
        let key = ContainerKey::new("local".to_string(), "abc123def456".to_string());
        state.containers.insert(key, container);
        state.sort_containers();
        state.table_state.select(Some(0));

        // Open column selector
        state.view_state = ViewState::ColumnSelector;
        state.column_selector_state.select(Some(0));
        state.column_config_snapshot = Some(state.column_config.clone());

        let styles = UiStyles::default();
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_container_list_with_hidden_columns() {
        let mut state = create_test_app_state();

        // Hide ID and Net TX columns
        let id_idx = state.column_config.columns.iter().position(|(c, _)| *c == Column::Id).unwrap();
        state.column_config.toggle(id_idx);
        let net_tx_idx = state.column_config.columns.iter().position(|(c, _)| *c == Column::NetTx).unwrap();
        state.column_config.toggle(net_tx_idx);

        // Add containers
        let container = create_test_container("abc123def456", "nginx", "local", 25.0, 50.0, 1024.0, 2048.0);
        let key = ContainerKey::new("local".to_string(), "abc123def456".to_string());
        state.containers.insert(key, container);
        state.sort_containers();
        state.table_state.select(Some(0));

        let styles = UiStyles::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }
```

- [ ] **Step 2: Run snapshot tests and accept**

Run: `cargo insta test`
Then: `cargo insta accept`

- [ ] **Step 3: Verify all tests pass**

Run: `cargo test`
Expected: All tests pass including new snapshots

- [ ] **Step 4: Commit**

```bash
git add src/ui/ui_tests.rs src/ui/snapshots/
git commit -m "test: add snapshot tests for column selector and hidden columns"
```

---

### Task 8: Wire config columns through main.rs and test config round-trip

**Files:**
- Modify: `src/cli/config.rs` (tests)

- [ ] **Step 1: Add config deserialization test for columns**

Add these tests to the `tests` module in `src/cli/config.rs`:

```rust
    #[test]
    fn test_yaml_deserialization_with_columns() {
        let yaml = r#"
hosts:
  - host: local
columns:
  - status
  - name
  - cpu
  - memory
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.hosts.len(), 1);
        let columns = config.columns.unwrap();
        assert_eq!(columns, vec!["status", "name", "cpu", "memory"]);
    }

    #[test]
    fn test_yaml_deserialization_without_columns() {
        let yaml = r#"
hosts:
  - host: local
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.columns.is_none());
    }

    #[test]
    fn test_yaml_serialization_with_columns() {
        let config = Config {
            hosts: vec![HostConfig {
                host: "local".to_string(),
                dozzle: None,
                filter: None,
            }],
            icons: None,
            all: None,
            sort: None,
            columns: Some(vec!["name".to_string(), "cpu".to_string()]),
        };
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("columns:"));
        assert!(yaml.contains("- name"));
        assert!(yaml.contains("- cpu"));
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -- --test-threads=1 config`
Expected: All config tests pass

- [ ] **Step 3: Commit**

```bash
git add src/cli/config.rs
git commit -m "test: add config serialization tests for columns field"
```
