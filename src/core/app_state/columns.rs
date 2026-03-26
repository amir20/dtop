use std::path::PathBuf;

use crate::core::app_state::AppState;
use crate::core::types::{RenderAction, ViewState};

impl AppState {
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
            KeyCode::Esc | KeyCode::Char('c') => self.handle_close_column_selector(),
            _ => RenderAction::None,
        }
    }

    /// Handles closing the column selector, showing save prompt if config changed
    pub(super) fn handle_close_column_selector(&mut self) -> RenderAction {
        if let Some(ref snapshot) = self.column_config_snapshot
            && *snapshot != self.column_config
        {
            self.column_save_prompt = true;
            return RenderAction::Render;
        }
        self.close_column_selector()
    }

    fn close_column_selector(&mut self) -> RenderAction {
        self.view_state = ViewState::ContainerList;
        self.column_config_snapshot = None;
        self.column_selector_state.select(None);
        self.column_save_prompt = false;
        RenderAction::Render
    }

    fn save_column_config(&self) {
        let config_path = self.config_path.clone().unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
                .join("dtop")
                .join("config.yaml")
        });

        let column_strings = self.column_config.to_config_strings();

        // Use spawn_blocking to avoid blocking the async event loop
        tokio::task::spawn_blocking(move || {
            write_column_config(&config_path, &column_strings);
        });
    }
}

/// Writes column configuration to the config file (blocking I/O, run off main thread)
fn write_column_config(config_path: &std::path::Path, column_strings: &[String]) {
    let mut config: serde_yaml::Value = if config_path.exists() {
        let contents = match std::fs::read_to_string(config_path) {
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

    let columns_value: Vec<serde_yaml::Value> = column_strings
        .iter()
        .map(|s| serde_yaml::Value::String(s.clone()))
        .collect();

    if let serde_yaml::Value::Mapping(ref mut map) = config {
        map.insert(
            serde_yaml::Value::String("columns".to_string()),
            serde_yaml::Value::Sequence(columns_value),
        );
    }

    if let Some(parent) = config_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        tracing::error!("Failed to create config directory: {}", e);
        return;
    }

    let yaml_string = match serde_yaml::to_string(&config) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to serialize config: {}", e);
            return;
        }
    };

    if let Err(e) = std::fs::write(config_path, yaml_string) {
        tracing::error!("Failed to write config file: {}", e);
    } else {
        tracing::debug!("Saved column config to: {}", config_path.display());
    }
}
