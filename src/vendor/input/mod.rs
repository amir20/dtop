//! TUI input library supporting multiple backends.
//!
//! Vendored from: <https://github.com/sayanarijit/tui-input>
//! License: MIT (see LICENSE file in this directory)
//! Copyright (c) 2021 Arijit Basu

mod core;

pub mod backend;
pub use core::{Input, InputRequest, StateChanged};
