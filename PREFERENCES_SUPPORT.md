# Preferences Persistence Feature Design

## Overview

Add the ability to save and reset user preferences in dtop. Uses a simple, non-intrusive approach that preserves existing keyboard shortcuts and adds two new actions: save (`Ctrl-S`) and reset (`Ctrl-R`).

## Design Goals

1. **Keep existing flows** - `s` for sort, `c` for columns, `a` for toggle remain unchanged
2. **Explicit save** - Changes only persist when user explicitly saves
3. **Minimal UI changes** - No new popups, just key bindings + feedback
4. **Discoverable via help** - Enhanced help popup (`?`) shows save/reset options
5. **Familiar conventions** - `Ctrl-S` to save is universal across applications

## New Key Bindings

| Key | Action | Context |
|-----|--------|---------|
| `Ctrl-S` | Save all preferences to config file | Container list view |
| `Ctrl-R` | Reset all preferences to defaults | Container list view |

## Preferences to Persist

| Preference | Type | Config Key | Default | Current Toggle |
|------------|------|------------|---------|----------------|
| Column visibility | `Vec<String>` | `columns` | All except Restarts, Compose, DiskRead, DiskWrite | `c` key |
| Column order | (implicit) | `columns` | ID, Status, Name, Host, ... | `c` key |
| Sort field | `String` | `sort` | `uptime` | `s` key |
| Sort direction | `String` | `sort_direction` | `desc` | `s` key |
| Show stopped containers | `bool` | `all` | `false` | `a` key |

## User Workflow

### Saving Preferences
1. User modifies settings during session (`c`, `s`, `a` keys)
2. User presses `Ctrl-S`
3. Brief notification appears: "Preferences saved to ~/.config/dtop/config.yaml"
4. Notification auto-dismisses after 2 seconds

### Resetting Preferences
1. User presses `Ctrl-R`
2. Confirmation prompt: "Reset all preferences to defaults? (y/n)"
3. If confirmed:
   - All preferences reset to defaults (in memory)
   - Notification: "Preferences reset to defaults"
4. User can then `Ctrl-S` to persist the reset, or continue with defaults for session only

## Config File Format

```yaml
# ~/.config/dtop/config.yaml
hosts:
  - host: local
  - host: ssh://user@server1

# User preferences (saved via Ctrl-S)
columns:
  - id
  - status
  - name
  - cpu
  - memory
  - net_tx
  - net_rx
  - uptime
sort: uptime
sort_direction: desc
all: false
```

## Updated Help Popup

Add new section to help popup (`?` key):

```
┌──────────────────── Help ─────────────────────┐
│                                               │
│  Navigation                                   │
│    ↑/↓         Select container               │
│    PageUp/Dn   Page up/down                   │
│    Home/End    First/last container           │
│                                               │
│  Views                                        │
│    →/l         View logs                      │
│    Enter       Action menu                    │
│    /           Search                         │
│                                               │
│  Settings                                     │
│    c           Column selector                │
│    s           Sort selector                  │
│    a           Toggle show all                │
│                                               │
│  Preferences                                  │
│    Ctrl-S      Save preferences to config     │
│    Ctrl-R      Reset preferences to defaults  │
│                                               │
│  Other                                        │
│    ?           Toggle this help               │
│    q/Ctrl-C    Quit                           │
│                                               │
└───────────────────────────────────────────────┘
```

## Implementation Plan

### 1. Add sort_direction to Config (`src/cli/config.rs`)

```rust
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Config {
    // ... existing fields ...
    pub sort_direction: Option<String>,  // "asc" or "desc"
}
```

### 2. Add notification state (`src/core/app_state/mod.rs`)

```rust
pub struct AppState {
    // ... existing fields ...
    pub notification: Option<(String, Instant)>,  // (message, show_until)
}
```

### 3. Create preferences handlers (`src/core/app_state/preferences.rs`)

```rust
impl AppState {
    /// Saves all preferences to config file (Ctrl-S)
    pub fn handle_save_preferences(&mut self) -> RenderAction {
        // Determine config path
        let config_path = self.config_path.clone()
            .unwrap_or_else(|| default_config_path());
        
        // Collect current preferences
        let columns = self.column_config.to_config_strings();
        let sort = self.sort_state.field.id().to_string();
        let sort_direction = match self.sort_state.direction {
            SortDirection::Ascending => "asc",
            SortDirection::Descending => "desc",
        };
        let all = self.show_all_containers;
        
        // Save async
        save_preferences(config_path, columns, sort, sort_direction, all);
        
        // Show notification
        self.show_notification("Preferences saved");
        RenderAction::Render
    }

    /// Resets all preferences to defaults (Ctrl-R)
    pub fn handle_reset_preferences(&mut self) -> RenderAction {
        self.column_config = ColumnConfig::default();
        self.sort_state = SortState::default();
        self.show_all_containers = false;
        self.force_sort_containers();
        
        self.show_notification("Preferences reset to defaults");
        RenderAction::Render
    }
    
    fn show_notification(&mut self, message: &str) {
        self.notification = Some((
            message.to_string(),
            Instant::now() + Duration::from_secs(2),
        ));
    }
}
```

### 4. Update input handling (`src/core/app_state/mod.rs`)

In the key event dispatcher for ContainerList view:

```rust
KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    self.handle_save_preferences()
}
KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    // Show confirmation first
    self.handle_reset_preferences_confirm()
}
```

### 5. Add notification rendering (`src/ui/render.rs`)

Render notification in top-right or bottom of screen:

```rust
if let Some((message, until)) = &app_state.notification {
    if Instant::now() < *until {
        // Render notification banner
        render_notification(f, area, message);
    } else {
        app_state.notification = None;
    }
}
```

### 6. Generalize config writing (`src/core/app_state/preferences.rs`)

Refactor existing `write_column_config()` to `write_preferences()`:

```rust
fn write_preferences(
    path: PathBuf,
    columns: Vec<String>,
    sort: String,
    sort_direction: String,
    all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read existing config to preserve hosts, icons, etc.
    let mut config = read_existing_config(&path)?;
    
    // Update preference keys
    config["columns"] = columns.into();
    config["sort"] = sort.into();
    config["sort_direction"] = sort_direction.into();
    config["all"] = all.into();
    
    // Write back
    write_config(&path, &config)?;
    Ok(())
}
```

### 7. Update help popup (`src/ui/help.rs`)

Add "Preferences" section with Ctrl-S and Ctrl-R.

### 8. Handle reset confirmation

Add a simple confirmation state:

```rust
pub reset_confirm_pending: bool,
```

When `Ctrl-R` pressed, set `reset_confirm_pending = true` and show "Reset to defaults? (y/n)" in notification area. `y` confirms, `n`/`Esc` cancels.

## Files to Modify

| File | Changes |
|------|---------|
| `src/cli/config.rs` | Add `sort_direction` field to Config |
| `src/core/types.rs` | Add notification types if needed |
| `src/core/app_state/mod.rs` | Add `notification` field, wire up Ctrl-S/Ctrl-R |
| `src/core/app_state/preferences.rs` | **New file** - Save/reset handlers |
| `src/core/app_state/columns.rs` | Remove `save_column_config()`, `write_column_config()`, `column_save_prompt` logic; simplify column selector close |
| `src/ui/render.rs` | Render notification banner; remove column save prompt rendering |
| `src/ui/help.rs` | Add Preferences section |
| `CLAUDE.md` | Document new key bindings |

## Edge Cases

### No Config File Path
When dtop is launched with CLI arguments only:
- `config_path` is `None`
- `Ctrl-S` creates `~/.config/dtop/config.yaml`
- First save shows: "Created ~/.config/dtop/config.yaml"

### Config File Doesn't Exist
- `Ctrl-S` creates parent directories if needed
- Creates new file with only preference keys (no hosts)

### Column Selector Save Prompt - REMOVED

The current column selector has its own "Save changes? (y/n)" prompt. This will be **removed** for consistency.

**Rationale**: With `Ctrl-S` as the unified save mechanism, having a separate save prompt in the column selector would be confusing:
- Users might think "I pressed `y` in column selector, why didn't my sort preference save?"
- Two different save mechanisms with different scopes is inconsistent

**New behavior**:
- Column selector closes immediately on `Esc` or `c`
- Changes stay in memory for the current session
- User presses `Ctrl-S` when ready to persist all preferences
- Clean mental model: make changes freely, then `Ctrl-S` to save everything

## Effort Estimate

**Total: ~4-5 hours**

- Preference handlers: 1-2 hours
- Config writing refactor: 1 hour
- Notification UI: 1 hour
- Help popup update: 30 minutes
- Testing: 30 minutes

## Future Enhancements

- **Undo** - `Ctrl-Z` to undo last preference change
- **Config file selector** - Choose which config file to save to
- **Preferences diff** - Show what changed before saving
