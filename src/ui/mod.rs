pub mod action_menu;
pub mod app_logos;
mod app_logos_data;
pub mod column_selector;
pub mod container_list;
pub mod details;
pub mod formatters;
pub mod help;
pub mod hyperlinks;
pub mod icons;
pub mod input;
pub mod log_view;
pub mod panes;
pub mod program_status;
pub mod render;
pub mod sort_selector;
pub mod system_header;
pub mod terminal_probe;
pub mod window_title;

#[cfg(test)]
mod alloc_tests;
#[cfg(test)]
mod ui_tests;
