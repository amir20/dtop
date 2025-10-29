// Public modules for testing
pub mod app_state;
pub mod config;
pub mod docker;
pub mod input;
pub mod logs;
pub mod stats;
pub mod types;
pub mod ui;

// UI snapshot tests
#[cfg(test)]
mod ui_tests;
