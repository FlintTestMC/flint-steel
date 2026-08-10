//! Flint testing framework integration for `SteelMC`.

mod adapter;
mod convert;
mod player;
/// Test connection implementation for Flint tests.
pub mod test_connection;
mod world;

pub use adapter::SteelAdapter;
pub use player::SteelTestPlayer;
pub use world::SteelTestWorld;

/// Re-export flint types for convenience
pub use flint_core::{TestLoader, TestRunner};

use std::sync::{Arc, OnceLock};
use steel_core::behavior::init_behaviors;
use steel_core::block_entity::init_block_entities;
use steel_core::entity::init_entities;
use steel_registry::init_vanilla_registry;
use tokio::runtime;
use tokio::runtime::Runtime;

/// Global runtime for flint tests.
static FLINT_RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

/// Initialize the `SteelMC` registry and behaviors for testing.
///
/// This must be called before creating any test worlds or adapters.
/// It's safe to call multiple times - subsequent calls are no-ops.
pub fn init() {
    init_vanilla_registry();
    init_behaviors();
    init_block_entities();
    init_entities();

    // Initialize runtime
    init_runtime();
}

/// Initialize the Tokio runtime for async operations.
fn init_runtime() {
    let _ = FLINT_RUNTIME.get_or_init(|| {
        Arc::new(
            runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("Failed to create Flint runtime"),
        )
    });
}

/// Gets the shared Tokio runtime for flint tests.
pub(crate) fn runtime() -> Arc<Runtime> {
    init_runtime();
    FLINT_RUNTIME
        .get()
        .expect("Runtime not initialized")
        .clone()
}

/// Test helper to initialize registries (for use in test modules)
#[cfg(test)]
pub(crate) fn init_test_registries() {
    init();
}
