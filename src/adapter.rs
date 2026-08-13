//! Flint adapter implementation for Steel.

use flint_core::{FlintAdapter, FlintWorld, ServerInfo};

use crate::world::SteelTestWorld;

/// Adapter for running Flint tests against an in-memory Steel world.
#[derive(Clone)]
pub struct SteelAdapter {
    info: ServerInfo,
}

impl SteelAdapter {
    /// Creates an adapter for Steel's compiled Minecraft version.
    #[must_use]
    pub fn new() -> Self {
        Self {
            info: ServerInfo {
                minecraft_version: steel_utils::MC_VERSION.to_string(),
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
    fn create_test_world(&self) -> Result<Box<dyn FlintWorld>, anyhow::Error> {
        Ok(Box::new(SteelTestWorld::new()?))
    }

    fn server_info(&self) -> ServerInfo {
        self.info.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use flint_core::test_spec::{
        ActionType, AssertType, BlockCheck, BlockSpec, TickSpec, TimelineEntry,
    };
    use flint_core::{Block, TestRunner, TestSpec, TestSpecLoadResult};

    use super::*;

    fn block_contract_spec(name: &str, expected: &str) -> TestSpec {
        TestSpec {
            flint_version: None,
            name: name.to_string(),
            description: None,
            tags: vec!["adapter-contract".to_string()],
            minecraft_ids: vec!["minecraft:stone".to_string()],
            dependencies: Vec::new(),
            setup: None,
            timeline: vec![
                TimelineEntry {
                    at: TickSpec::Single(0),
                    action_type: ActionType::Place {
                        pos: [0, 64, 0],
                        block: Block::new("minecraft:stone"),
                    },
                },
                TimelineEntry {
                    at: TickSpec::Single(1),
                    action_type: ActionType::Assert {
                        checks: vec![AssertType::Block(BlockCheck {
                            pos: [0, 64, 0],
                            is: BlockSpec::Single(Block::new(expected)),
                        })],
                    },
                },
            ],
            breakpoints: Vec::new(),
        }
    }

    #[test]
    fn server_info_uses_steel_minecraft_version() {
        assert_eq!(
            SteelAdapter::new().server_info().minecraft_version,
            steel_utils::MC_VERSION
        );
    }

    #[test]
    fn block_contract_has_a_green_control_and_a_red_control() {
        let specs = vec![
            TestSpecLoadResult::Loaded(block_contract_spec(
                "correct block assertion",
                "minecraft:stone",
            )),
            TestSpecLoadResult::Loaded(block_contract_spec(
                "deliberately wrong block assertion",
                "minecraft:dirt",
            )),
        ];
        let summary = TestRunner::new(Arc::new(SteelAdapter::new())).run_tests(&specs);

        assert_eq!(summary.total_tests, 2, "both controls must be loaded");
        assert_eq!(summary.skipped_tests, 0, "controls must not be skipped");
        assert_eq!(summary.passed_tests + summary.failed_tests, 2);

        let positive = &summary.results[0];
        assert!(positive.success, "positive control failed: {positive:?}");
        assert_eq!(positive.total_assertions(), 1);

        let negative = &summary.results[1];
        assert!(
            !negative.success,
            "negative control passed, so the adapter is false-green"
        );
        assert_eq!(negative.failed_count(), 1);
    }
}
