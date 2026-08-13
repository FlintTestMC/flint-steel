//! Flint testing framework integration for `SteelMC`.
//!
//! This crate provides implementations of the Flint traits (`FlintAdapter`, `FlintWorld`,
//! `FlintPlayer`) that allow running automated tests against the `SteelMC` server.
//!
//! # Architecture
//!
//! This integration uses Steel's production `World` and gameplay code:
//! - `SteelTestWorld` owns Steel's isolated RAM-only `InMemoryWorld` harness
//! - `SteelTestPlayer` uses the real block/item behavior system
//! - Steel owns player attachment, chunks, ticking, and runtime shutdown
//!
//! Unsupported operations return errors instead of being silently ignored.
//!
//! # Example
//!
//! ```ignore
//! use std::{path::Path, sync::Arc};
//! use flint_core::{TestLoader, TestRunner};
//! use flint_steel::SteelAdapter;
//!
//! # fn run() -> anyhow::Result<()> {
//! let root = Path::new("./tests");
//! let loader = TestLoader::unindexed(root, true)?;
//! let paths = loader.collect_all_test_files()?;
//! let specs = loader.load_specs(&paths, false)?;
//! let _summary = TestRunner::new(Arc::new(SteelAdapter::new())).run_tests(&specs);
//! # Ok(())
//! # }
//! ```

mod adapter;
mod convert;
mod player;
mod world;

pub use adapter::SteelAdapter;
pub use player::SteelTestPlayer;
pub use world::SteelTestWorld;

/// Re-export flint types for convenience
pub use flint_core::{TestLoader, TestRunner};

use steel_core::bootstrap::init_globals_once;

/// Initialize the `SteelMC` registry and behaviors for testing.
///
/// `SteelTestWorld::new` also performs this idempotent initialization. This
/// entry point remains useful to tests that exercise conversion code without
/// creating a world.
pub fn init() {
    init_globals_once();
}

/// Test helper to initialize registries (for use in test modules)
#[cfg(test)]
pub(crate) fn init_test_registries() {
    init();
}
