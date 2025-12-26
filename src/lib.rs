// Core modules
pub mod core {
    pub mod app_state;
    pub mod types;
}

// Docker-related modules
pub mod docker;

// UI modules
pub mod ui;

// CLI modules
pub mod cli {
    pub mod config;
    #[cfg(feature = "self-update")]
    pub mod update;
}

// Vendored dependencies
pub mod vendor {
    pub mod input;
}

// Re-export commonly used items for convenience
pub use cli::config::*;
#[cfg(feature = "self-update")]
pub use cli::update::*;
pub use core::{app_state::AppState, types::*};
pub use docker::{actions::*, connection::*, logs::*, stats::*};
pub use ui::{input::*, render::*};
pub use vendor::input::Input as VendoredInput;
