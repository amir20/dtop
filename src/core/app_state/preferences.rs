//! Preferences save and reset handlers (Ctrl-S, Ctrl-R)

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::core::app_state::AppState;
use crate::core::types::{ColumnConfig, RenderAction, SortDirection, SortState, ViewState};

impl AppState {
    /// Shows a notification message that auto-dismisses after 2 seconds
    pub fn show_notification(&mut self, message: &str) {
        self.notification = Some((message.to_string(), Instant::now() + Duration::from_secs(2)));
    }

    /// Clears the notification if it has expired. Also cancels a pending reset
    /// confirmation so a stray later `y` cannot trigger a reset with no prompt shown.
    pub fn clear_expired_notification(&mut self) {
        if let Some((_, expiry)) = &self.notification
            && Instant::now() > *expiry
        {
            self.notification = None;
            self.reset_confirm_pending = false;
        }
    }

    /// Handles Ctrl-S: Save all preferences to config file
    pub fn handle_save_preferences(&mut self) -> RenderAction {
        // Only allow in container list view
        if !matches!(
            self.view_state,
            ViewState::ContainerList | ViewState::SearchMode
        ) {
            return RenderAction::None;
        }

        // Determine config path
        let config_path = self.config_path.clone().unwrap_or_else(default_config_path);

        // Collect current preferences
        let columns = self.column_config.to_config_strings();
        let sort = self.sort_state.field.id().to_string();
        let sort_direction = match self.sort_state.direction {
            SortDirection::Ascending => "asc".to_string(),
            SortDirection::Descending => "desc".to_string(),
        };
        let all = self.show_all_containers;

        // Build display path for notification (shorten home dir to ~)
        let display_path = config_path
            .to_str()
            .map(|s| {
                if let Some(home) = dirs::home_dir()
                    && let Some(home_str) = home.to_str()
                    && s.starts_with(home_str)
                {
                    return format!("~{}", &s[home_str.len()..]);
                }
                s.to_string()
            })
            .unwrap_or_else(|| "config file".to_string());

        // Perform synchronous write - preferences save is user-initiated and should
        // complete before showing result. The write is fast (<1ms typically).
        match write_preferences(&config_path, columns, sort, sort_direction, all) {
            Ok(()) => {
                self.show_notification(&format!("Saved to {}", display_path));
            }
            Err(e) => {
                tracing::error!("Failed to save preferences: {}", e);
                self.show_notification(&format!("Save failed: {}", e));
            }
        }

        RenderAction::Render
    }

    /// Handles Ctrl-R: Show reset confirmation prompt
    pub fn handle_reset_preferences_prompt(&mut self) -> RenderAction {
        // Only allow in container list view
        if !matches!(
            self.view_state,
            ViewState::ContainerList | ViewState::SearchMode
        ) {
            return RenderAction::None;
        }

        self.reset_confirm_pending = true;
        self.notification = Some((
            "Reset all preferences to defaults? (y/n)".to_string(),
            Instant::now() + Duration::from_secs(30), // Long timeout for confirmation
        ));
        RenderAction::Render
    }

    /// Handles 'y' confirmation for reset
    pub fn handle_reset_preferences_confirm(&mut self) -> RenderAction {
        self.reset_confirm_pending = false;

        // Reset all preferences to defaults
        self.column_config = ColumnConfig::default();
        self.sort_state = SortState::default();
        self.show_all_containers = false;

        // Force re-sort with new settings
        self.force_sort_containers();

        self.show_notification("Preferences reset to defaults");
        RenderAction::Render
    }
}

/// Returns the default config path: ~/.config/dtop/config.yaml
/// Falls back to current directory only if home directory cannot be determined.
fn default_config_path() -> PathBuf {
    dirs::home_dir()
        .map(|home| home.join(".config").join("dtop").join("config.yaml"))
        .unwrap_or_else(|| {
            tracing::warn!(
                "Could not determine home directory, using current directory for config"
            );
            PathBuf::from(".dtop.yaml")
        })
}

/// Writes preferences to the config file, preserving other keys (hosts, icons, etc.)
///
/// Uses atomic write (write to temp file, then rename) to prevent corruption on crash.
/// Returns an error if the existing config file contains invalid YAML rather than
/// silently replacing it.
fn write_preferences(
    path: &PathBuf,
    columns: Vec<String>,
    sort: String,
    sort_direction: String,
    all: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use serde_yaml::Value;
    use std::fs;

    // Read existing config or create empty mapping
    let mut config: Value = if path.exists() {
        let contents = fs::read_to_string(path)?;
        // Don't silently discard invalid YAML - return error so user knows
        serde_yaml::from_str(&contents).map_err(|e| {
            format!(
                "Config file contains invalid YAML: {}. Please fix or delete the file.",
                e
            )
        })?
    } else {
        Value::Mapping(Default::default())
    };

    // Ensure we have a mapping
    let mapping = config
        .as_mapping_mut()
        .ok_or("Config file is not a YAML mapping (expected key: value format)")?;

    // Update preference keys
    mapping.insert(
        Value::String("columns".to_string()),
        Value::Sequence(columns.into_iter().map(Value::String).collect()),
    );
    mapping.insert(Value::String("sort".to_string()), Value::String(sort));
    mapping.insert(
        Value::String("sort_direction".to_string()),
        Value::String(sort_direction),
    );
    mapping.insert(Value::String("all".to_string()), Value::Bool(all));

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Atomic write: write to temp file first, then rename
    // This prevents corruption if the process crashes mid-write
    let temp_path = path.with_extension("yaml.tmp");
    let yaml = serde_yaml::to_string(&config)?;
    fs::write(&temp_path, &yaml)?;

    // Rename is atomic on Unix; on Windows it's still safer than direct write
    fs::rename(&temp_path, path).or_else(|_| {
        // Fallback for systems where rename fails (e.g., cross-device)
        fs::write(path, &yaml)?;
        let _ = fs::remove_file(&temp_path); // Clean up temp file
        Ok::<(), std::io::Error>(())
    })?;

    tracing::info!("Preferences saved to {:?}", path);
    Ok(())
}
