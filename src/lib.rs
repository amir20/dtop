// Core modules
pub mod core {
    pub mod app_state;
    pub mod types;
}

// Docker-related modules
pub mod docker {
    pub mod connection;
    pub mod logs;
    pub mod stats;
}

// UI modules
pub mod ui {
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
pub use docker::{connection::*, logs::*, stats::*};
pub use ui::{input::*, render::*};
