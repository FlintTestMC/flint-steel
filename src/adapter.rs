//! Flint adapter implementation for `SteelMC`.
//!
//! This module provides the [`SteelAdapter`] which implements the `FlintAdapter` trait,
//! allowing the Flint testing framework to create test worlds using the real steel-core
//! World implementation.

use flint_core::{FlintAdapter, FlintWorld, ServerInfo};

use crate::world::SteelTestWorld;

/// Adapter for running Flint tests against `SteelMC`.
///
/// This adapter creates test worlds that use the real steel-core World
/// with RAM-only storage for instant chunk creation.
#[derive(Clone)]
pub struct SteelAdapter {
    /// Server info for identification
    info: ServerInfo,
}

impl SteelAdapter {
    /// Creates a new Steel adapter.
    ///
    /// Note: You must call `steel_flint::init()` before creating an adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            info: ServerInfo {
                minecraft_version: "1.21.11".to_string(),
            },
        }
    }
}

impl Default for SteelAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FlintAdapter for SteelAdapter {
    fn create_test_world(&self) -> Box<dyn FlintWorld> {
        Box::new(SteelTestWorld::new())
    }

    fn server_info(&self) -> ServerInfo {
        self.info.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_test_registries;
    use crate::{TestLoader, TestRunner};
    use dotenvy::dotenv;
    use flint_core::results::TestSummary;
    use flint_core::utils::get_test_path;
    use std::env::var;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use steel_core::behavior::{BLOCK_BEHAVIORS, ITEM_BEHAVIORS};
    use steel_registry::{REGISTRY, RegistryExt};

    #[derive(serde::Deserialize, Default)]
    struct FlintConfig {
        filter: Option<FilterConfig>,
    }

    #[derive(serde::Deserialize, Default)]
    struct FilterConfig {
        tags: Option<rustc_hash::FxHashMap<String, bool>>,
        implemented_only: Option<bool>,
        test: Option<String>,
        pattern: Option<String>,
    }

    fn load_config() -> FlintConfig {
        fs::read_to_string("flint.toml")
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn init_env() {
        dotenv().ok();
    }

    /// Collects test file paths based on environment variables and `flint.toml`.
    /// Priority: `FLINT_TEST` > `FLINT_PATTERN` > `FLINT_TAGS` > `implemented_only` > `filter.tags` > all
    fn collect_filtered_paths(loader: &TestLoader, cfg: &FlintConfig) -> Vec<PathBuf> {
        let filter = cfg.filter.as_ref();

        // Single test by name: env > toml
        let test_name = var("FLINT_TEST")
            .ok()
            .or_else(|| filter.and_then(|f| f.test.clone()));
        if let Some(name) = test_name {
            println!("Running single test: {name}");
            return loader
                .collect_all_test_files()
                .unwrap_or_default()
                .into_iter()
                .filter(|p| {
                    p.file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| n == name)
                })
                .collect();
        }

        // Tag filtering: env > implemented_only > toml tags
        let tags = var("FLINT_TAGS")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .collect::<Vec<_>>()
            })
            .or_else(|| {
                if filter.and_then(|f| f.implemented_only).unwrap_or(false) {
                    let mut ids = get_implemented_items();
                    ids.extend(get_implemented_blocks());
                    Some(ids)
                } else {
                    filter.and_then(|f| {
                        f.tags.as_ref().map(|map| {
                            map.iter()
                                .filter_map(|(k, &v)| v.then(|| k.clone()))
                                .collect()
                        })
                    })
                }
            });
        if let Some(tags) = tags {
            println!("Running tests with tags: {}", tags.join(", "));
            return loader.collect_by_tags(&tags).unwrap_or_default();
        }

        // Pattern matching: env > toml
        let pattern = var("FLINT_PATTERN")
            .ok()
            .or_else(|| filter.and_then(|f| f.pattern.clone()));
        if let Some(p) = pattern {
            println!("Running tests matching pattern: {p}");
            return loader
                .collect_all_test_files()
                .unwrap_or_default()
                .into_iter()
                .filter(|path| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|name| matches_pattern(name, &p))
                })
                .collect();
        }

        // Default: all tests
        println!("Running all flint tests");
        loader.collect_all_test_files().unwrap_or_default()
    }

    /// Simple glob pattern matching (supports * wildcard)
    fn matches_pattern(name: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            return name.starts_with(prefix);
        }
        if let Some(suffix) = pattern.strip_prefix('*') {
            return name.ends_with(suffix);
        }
        name == pattern
    }
    fn save_summary(summary: &TestSummary) {
        let path = PathBuf::from("log/flint_summary.json");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("TODO: panic message");
        }
        fs::write(&path, summary.create_ci_output(true))
            .expect("failed to write flint_summary.json");
        println!("Summary saved to {}", path.display());
    }

    fn get_implemented_blocks() -> Vec<String> {
        let mut registered_block_ids: Vec<String> = Vec::new();
        for (id, behavior) in BLOCK_BEHAVIORS.get_behaviors().iter().enumerate() {
            if !behavior.type_name().ends_with("DefaultBlockBehavior") {
                registered_block_ids.push(REGISTRY.blocks.by_id(id).unwrap().key.to_string());
            }
        }
        registered_block_ids
    }
    fn get_implemented_items() -> Vec<String> {
        let mut registered_block_ids: Vec<String> = Vec::new();
        for (id, behavior) in ITEM_BEHAVIORS.get_behaviors().iter().enumerate() {
            if !behavior.type_name().ends_with("DefaultItemBehavior")
                && !behavior.type_name().ends_with("BlockItemBehavior")
            {
                registered_block_ids.push(REGISTRY.items.by_id(id).unwrap().key.to_string());
            }
        }
        registered_block_ids
    }

    #[test]
    fn test_things() {
        init_env();
        let _config = load_config();
        let test_path = PathBuf::from(get_test_path());
        let mut loader = TestLoader::new(&test_path, true).unwrap_or_else(|_| panic!("Test"));
        loader
            .verify_and_rebuild_index()
            .expect("TODO: panic message");
    }

    #[test]
    fn test_run_flint_selected() {
        init_test_registries();
        init_env();

        // Load the fence test
        let cfg = load_config();
        let test_path = PathBuf::from(get_test_path());
        let loader = TestLoader::new(&test_path, true)
            .unwrap_or_else(|e| panic!("error while loading test files: {e}"));
        let paths = collect_filtered_paths(&loader, &cfg);
        let specs = loader.load_specs(&paths, false).unwrap();

        // Create adapter and runner
        let adapter = SteelAdapter::new();
        let runner = TestRunner::new(Arc::new(adapter));

        // Run the test
        let summary = runner.run_tests(&specs);
        summary.print_concise_summary();
        save_summary(&summary);
        assert_eq!(summary.failed_tests, 0, "Not all flint tests passed!");
    }

    #[test]
    fn test_run_all_flint_benchmarks() {
        init_test_registries();
        init_env();

        let test_dir = PathBuf::from(get_test_path());
        assert!(
            test_dir.exists(),
            "FlintBenchmark tests directory not found, skipping"
        );

        let loader = TestLoader::new(&test_dir, true)
            .unwrap_or_else(|e| panic!("error while loading test files: {e}"));
        let paths = loader
            .collect_all_test_files()
            .unwrap_or_else(|e| panic!("error while loading test files: {e}"));
        assert!(
            !paths.is_empty(),
            "No test files matched the filter criteria in {}",
            test_dir.to_str().unwrap_or("NONE")
        );

        println!("Found {} test(s) to run", paths.len());

        let specs = loader.load_specs(&paths, false).unwrap();

        let adapter = SteelAdapter::new();
        let runner = TestRunner::new(Arc::new(adapter));
        let summary = runner.run_tests(&specs);
        summary.print_concise_summary();
        save_summary(&summary);
        assert_eq!(summary.failed_tests, 0, "No tests were run");
    }
}
