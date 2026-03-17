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
