//! Test execution engine.
//!
//! The `TestRunner` loads tests and executes them against a server adapter.

use crate::traits::{BlockData, FlintAdapter, FlintPlayer, FlintWorld, Item, PlayerSlot};
use flint_core::results::{
    ActionOutcome, AssertFailure, AssertionResult, InfoType, TestResult, TestSummary,
};
use flint_core::test_spec::{ActionType, Block, TestSpec};
use flint_core::timeline::TimelineAggregate;
use rustc_hash::FxHashMap;
use std::time::Instant;

/// Configuration for test execution
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TestRunConfig {
    /// Enable debug mode with breakpoints
    pub debug_enabled: bool,
    /// Run tests in parallel
    pub parallel: bool,
    /// Maximum parallel test worlds
    pub max_parallel_worlds: usize,
}

#[allow(dead_code)]
impl Default for TestRunConfig {
    fn default() -> Self {
        Self {
            debug_enabled: false,
            parallel: false,
            max_parallel_worlds: 4,
        }
    }
}

/// Test execution engine
pub struct TestRunner<'a, A: FlintAdapter> {
    adapter: &'a A,
    // is needed for later, to run multiple tests in parallel or have more configs
    // config: TestRunConfig,
}

impl<'a, A: FlintAdapter> TestRunner<'a, A> {
    pub fn new(adapter: &'a A) -> Self {
        Self { adapter }
    }

    /// Run a single test
    pub fn run_test(&self, spec: &TestSpec) -> TestResult {
        let start_time = Instant::now();
        let mut world = self.adapter.create_test_world();

        // Build timeline for single test (no offset)
        let tests_with_offsets = vec![(spec.clone(), [0i32, 0, 0])];
        let timeline = TimelineAggregate::from_tests(&tests_with_offsets);

        let mut result = TestResult::new(&spec.name);

        // Player is created on demand when player actions are used
        let mut player: Option<Box<dyn FlintPlayer>> = None;

        // Initialize player from config if present (advanced mode)
        if let Some(setup) = &spec.setup
            && let Some(player_config) = setup.player.as_ref()
        {
            let p = player.get_or_insert_with(|| world.create_player());

            // Set initial inventory
            for (slot_name, slot_config) in &player_config.inventory {
                let item = Item::with_count(&slot_config.item, slot_config.count);
                p.set_slot(*slot_name, Some(&item));
            }

            // Set initial hotbar selection
            p.select_hotbar(player_config.selected_hotbar);
        }

        // Execute timeline tick by tick
        for tick in 0..=timeline.max_tick {
            // Check for breakpoints (debug mode)
            // if self.config.debug_enabled && timeline.breakpoints.contains(&tick) {
            //     // TODO: Implement breakpoint pause mechanism
            //     // For now, just continue
            // }

            // Execute actions for this tick
            if let Some(actions) = timeline.timeline.get(&tick) {
                for (_test_idx, entry, _value_idx) in actions.iter() {
                    match self.execute_action(&mut *world, &mut player, &entry.action_type, tick) {
                        ActionOutcome::Action => {}
                        ActionOutcome::AssertPassed => {
                            result.add_assertion(AssertionResult::Success(tick));
                        }
                        ActionOutcome::AssertFailed(fail) => {
                            result.add_assertion(AssertionResult::Failure(fail));
                            result.success = false;
                            result.total_ticks = tick;
                            result.execution_time_ms = start_time.elapsed().as_millis() as u64;
                            return result;
                        }
                    }
                }
            }

            // Advance game tick
            world.do_tick();
        }

        result.total_ticks = timeline.max_tick;
        result.execution_time_ms = start_time.elapsed().as_millis() as u64;
        result
    }

    /// Run multiple tests
    pub fn run_tests(&self, specs: &[TestSpec]) -> TestSummary {
        // For now, run sequentially
        // TODO: Implement parallel execution
        let results: Vec<TestResult> = specs.iter().map(|spec| self.run_test(spec)).collect();
        TestSummary::from_results(results)
    }

    /// Execute a single action
    fn execute_action(
        &self,
        world: &mut dyn FlintWorld,
        player: &mut Option<Box<dyn FlintPlayer>>,
        action: &ActionType,
        _tick: u32,
    ) -> ActionOutcome {
        match action {
            ActionType::Place { pos, block } => {
                let pos = [pos[0], pos[1], pos[2]];
                world.set_block(pos, block);
                ActionOutcome::Action
            }

            ActionType::PlaceEach { blocks } => {
                for placement in blocks {
                    let pos = [placement.pos[0], placement.pos[1], placement.pos[2]];
                    world.set_block(pos, &placement.block);
                }
                ActionOutcome::Action
            }

            ActionType::Fill { region, with } => {
                // Flint handles fill by iterating set_block
                // Handle potentially inverted coordinates
                let min_x = region[0][0].min(region[1][0]);
                let max_x = region[0][0].max(region[1][0]);
                let min_y = region[0][1].min(region[1][1]);
                let max_y = region[0][1].max(region[1][1]);
                let min_z = region[0][2].min(region[1][2]);
                let max_z = region[0][2].max(region[1][2]);

                for x in min_x..=max_x {
                    for y in min_y..=max_y {
                        for z in min_z..=max_z {
                            world.set_block([x, y, z], with);
                        }
                    }
                }
                ActionOutcome::Action
            }

            ActionType::Remove { pos } => {
                let pos = [pos[0], pos[1], pos[2]];
                let air = Block {
                    id: "minecraft:air".to_string(),
                    properties: Default::default(),
                };
                world.set_block(pos, &air);
                ActionOutcome::Action
            }

            ActionType::Assert { checks } => {
                for check in checks {
                    let pos = [check.pos[0], check.pos[1], check.pos[2]];
                    let actual = world.get_block(pos);

                    if !block_matches(&actual, &check.is) {
                        // Convert expected Block properties to BlockData format
                        let expected_props: FxHashMap<String, String> = check
                            .is
                            .properties
                            .iter()
                            .filter_map(|(k, v)| {
                                let value = match v {
                                    serde_json::Value::String(s) => s.clone(),
                                    serde_json::Value::Bool(b) => b.to_string(),
                                    serde_json::Value::Number(n) => n.to_string(),
                                    _ => return None,
                                };
                                Some((k.clone(), value))
                            })
                            .collect();
                        return ActionOutcome::AssertFailed(AssertFailure {
                            tick: _tick,
                            error_message: format!(
                                "Block mismatch at {:?}: expected '{}', got '{}'",
                                pos,
                                BlockData::with_properties(&check.is.id, expected_props.clone()),
                                actual,
                            ),
                            position: pos,
                            execution_time_ms: None,
                            expected: InfoType::Block(
                                BlockData::with_properties(&check.is.id, expected_props.clone())
                                    .into_block(),
                            ),
                            actual: InfoType::Block(actual.into_block()),
                        });
                    }
                }
                ActionOutcome::AssertPassed
            }

            ActionType::UseItemOn { pos, face, item } => {
                // Create player on demand if not already created
                let p = player.get_or_insert_with(|| world.create_player());
                let pos = [pos[0], pos[1], pos[2]];

                // Simple mode: if item is specified, set it in hotbar1 and select it
                if let Some(item_id) = item {
                    let item = Item::new(item_id);
                    p.set_slot(PlayerSlot::Hotbar1, Some(&item));
                    p.select_hotbar(1);
                }

                p.use_item_on(pos, &face);
                ActionOutcome::Action
            }

            ActionType::SetSlot { slot, item, count } => {
                // Create player on demand if not already created
                let p = player.get_or_insert_with(|| world.create_player());
                if let Some(item_id) = item {
                    let item = Item::with_count(item_id, *count);
                    p.set_slot(*slot, Some(&item));
                } else {
                    p.set_slot(*slot, None);
                }
                ActionOutcome::Action
            }

            ActionType::SelectHotbar { slot } => {
                // Create player on demand if not already created
                let p = player.get_or_insert_with(|| world.create_player());
                p.select_hotbar(*slot);
                ActionOutcome::Action
            }
        }
    }
}

/// Check if actual block matches expected
fn block_matches(actual: &BlockData, expected: &Block) -> bool {
    // Check block ID
    if actual.id != expected.id {
        // Also try without minecraft: prefix
        let expected_id = if expected.id.starts_with("minecraft:") {
            &expected.id[10..]
        } else {
            &expected.id
        };
        let actual_id = if actual.id.starts_with("minecraft:") {
            &actual.id[10..]
        } else {
            &actual.id
        };
        if actual_id != expected_id {
            return false;
        }
    }

    // Check properties if specified
    for (key, expected_value) in &expected.properties {
        // Skip the "properties" key itself (nested format) - it's handled by Block::to_command
        if key == "properties" {
            continue;
        }

        // Convert expected value to string for comparison
        let expected_str = match expected_value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Null => continue, // Skip null values
            _ => continue,                       // Skip complex types (arrays, objects)
        };

        if let Some(actual_value) = actual.properties.get(key) {
            if actual_value != &expected_str {
                return false;
            }
        } else {
            // Property expected but not found in actual block - this is a mismatch
            return false;
        }
    }

    true
}
