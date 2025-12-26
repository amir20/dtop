//! TUI input library supporting multiple backends.
//!
//! See examples in the [GitHub repo](https://github.com/sayanarijit/tui-input/tree/main/examples).

mod core;

pub mod backend;
pub use core::{Input, InputRequest, StateChanged};
