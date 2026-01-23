//! Flint-Steel: Abstract white-box testing framework for Minecraft servers.
//!
//! This library provides traits that Rust Minecraft servers implement to enable
//! automated testing. Flint-steel drives the server tick-by-tick for deterministic
//! testing.
//!
//! # Server Integration
//!
//! Servers implement two main traits:
//! - [`FlintAdapter`] - Creates test worlds
//! - [`FlintWorld`] - Block and tick operations
//! - [`FlintPlayer`] - Player inventory and item interactions (optional)
//!
//! # Example
//!
//! ```ignore
//! use flint_steel::{FlintAdapter, TestRunner, TestRunConfig, TestSelector, TestFilter};
//!
//! // Server implements FlintAdapter
//! let adapter = MyServerAdapter::new();
//!
//! // Load and filter tests
//! let selector = TestSelector::new("./tests").unwrap();
//!
//! // Run all tests
//! let specs = selector.load_tests(&TestFilter::all()).unwrap();
//!
//! // Or filter by tags
//! let specs = selector.load_tests(&TestFilter::by_tags(["redstone"])).unwrap();
//!
//! // Or filter by name pattern
//! let specs = selector.load_tests(&TestFilter::by_patterns(["copper_*"])).unwrap();
//!
//! // Or run a single test by name
//! let spec = selector.load_test_by_name("copper_waxing").unwrap();
//!
//! // Execute tests
//! let runner = TestRunner::new(&adapter, TestRunConfig::default());
//! let summary = runner.run_tests(&specs);
//!
//! println!("Passed: {}/{}", summary.passed_tests, summary.total_tests);
//! ```

pub mod filter;
pub mod mock;
pub mod runner;
pub mod traits;

// Re-export main types for convenience
pub use filter::{TestFilter, TestSelector};
pub use mock::{MockAdapter, MockPlayer, MockWorld};
pub use runner::{TestRunConfig, TestRunner};
pub use traits::{BlockPos, FlintAdapter, FlintPlayer, FlintWorld, Item, ServerInfo};

// Re-export flint-core types commonly used with this library
pub use flint_core::loader::TestLoader;
pub use flint_core::test_spec::{Block, TestSpec};
