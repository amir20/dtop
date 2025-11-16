// Core modules
pub mod core {
    pub mod app_state;
    pub mod types;
}

// Docker-related modules
pub mod docker;

// UI modules
pub mod ui {
    pub mod action_menu;
    pub mod container_list;
    pub mod help;
    pub mod input;
    pub mod render;

    #[cfg(test)]
    mod ui_tests;
}

// CLI modules
pub mod cli {
    pub mod config;
    #[cfg(feature = "self-update")]
    pub mod update;
}

// Re-export commonly used items for convenience
pub use cli::config::*;
#[cfg(feature = "self-update")]
pub use cli::update::*;
pub use core::{app_state::AppState, types::*};
pub use docker::{actions::*, connection::*, logs::*, stats::*};
pub use ui::{input::*, render::*};
