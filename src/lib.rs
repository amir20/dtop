// Test-only allocation counting allocator, used by allocation-regression tests.
#[cfg(test)]
mod alloc_counter;

#[cfg(test)]
#[global_allocator]
static GLOBAL_ALLOC: alloc_counter::CountingAllocator = alloc_counter::CountingAllocator;

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
