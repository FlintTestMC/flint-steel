//! Flint world implementation over Steel's isolated in-memory test harness.

use std::io::Cursor;
use std::sync::Arc;

use anyhow::{Context, bail};
use flint_core::Block;
use flint_core::test_spec::{EntityNbt, GameruleValue, Weather, WorldConfig as FlintWorldConfig};
use flint_core::traits::EntityState;
use flint_core::{BlockPos as FlintBlockPos, FlintPlayer, FlintWorld};
use simdnbt::borrow::read_compound;
use simdnbt::owned::NbtTag;
use steel_core::test_harness::InMemoryWorld;
use steel_core::world::LevelReader;
use steel_registry::vanilla_timelines;
use steel_utils::Identifier;
use steel_utils::locks::SyncMutex;
use steel_utils::nbt::{parse_nbt_path, parse_snbt_compound, to_canonical_snbt};
use steel_utils::{ChunkPos, types::UpdateFlags};
use uuid::Uuid;

use crate::convert::{flint_block_to_state_id, flint_pos_to_steel, state_id_to_block};
use crate::player::SteelTestPlayer;

/// Test world implementation using the real steel-core World.
///
/// The harness owns a real Steel world configured with RAM-only storage:
/// - Chunks are created empty (all air) on-demand
/// - No disk I/O, no chunk generation delay
/// - Full block behavior system (neighbors, shapes, etc.)
/// - Real tick processing
pub struct SteelTestWorld {
    harness: Arc<SyncMutex<InMemoryWorld>>,
}

impl SteelTestWorld {
    /// Creates a new test world with RAM-only storage.
    ///
    /// The world uses the overworld dimension type and starts with seed 0.
    /// All chunks are created empty on-demand.
    ///
    pub fn new() -> Result<Self, anyhow::Error> {
        let mut world = Self {
            harness: Arc::new(SyncMutex::new(InMemoryWorld::new()?)),
        };
        world.configure_world(&FlintWorldConfig::default())?;
        Ok(world)
    }

    pub(crate) fn create_test_player(&self) -> Result<SteelTestPlayer, anyhow::Error> {
        let test_player =
            self.harness
                .lock()
                .create_player(Uuid::from_u128(1), "FlintPlayer", -1)?;
        Ok(SteelTestPlayer::new(test_player, Arc::clone(&self.harness)))
    }
}

fn resolve_daytime(value: &str) -> Result<u32, anyhow::Error> {
    if let Ok(ticks) = value.parse::<u32>() {
        return Ok(ticks);
    }

    let key = value
        .parse::<Identifier>()
        .map_err(|error| anyhow::anyhow!("invalid time marker `{value}`: {error}"))?;
    let marker = vanilla_timelines::DAY
        .time_markers
        .iter()
        .find(|marker| marker.key == key && marker.show_in_commands == Some(true))
        .with_context(|| format!("unknown Overworld day time marker `{key}`"))?;
    u32::try_from(marker.ticks)
        .with_context(|| format!("time marker `{key}` has invalid tick {}", marker.ticks))
}

fn gamerule_json_value(value: &GameruleValue) -> serde_json::Value {
    match value {
        GameruleValue::Bool(value) => serde_json::Value::Bool(*value),
        GameruleValue::Integer(value) => serde_json::Value::Number((*value).into()),
        GameruleValue::String(value) => serde_json::Value::String(value.clone()),
    }
}

impl FlintWorld for SteelTestWorld {
    fn configure_world(&mut self, config: &FlintWorldConfig) -> Result<(), anyhow::Error> {
        if config.weather != Weather::Clear {
            bail!(
                "flint-steel MVP supports only clear weather, got {}",
                config.weather
            );
        }

        let harness = self.harness.lock();
        harness.set_daytime(resolve_daytime(&config.time)?)?;
        for (key, value) in &config.gamerules {
            let key = key.parse::<Identifier>().map_err(|error| {
                anyhow::anyhow!("invalid game rule identifier `{key}`: {error}")
            })?;
            harness.set_game_rule(&key, &gamerule_json_value(value))?;
        }
        Ok(())
    }

    fn do_tick(&mut self) -> Result<(), anyhow::Error> {
        let _ = self.harness.lock().tick_once()?;
        Ok(())
    }

    fn current_tick(&self) -> u64 {
        self.harness.lock().current_tick()
    }

    fn get_time(&self) -> Result<u64, anyhow::Error> {
        Ok(u64::from(self.harness.lock().daytime()?))
    }

    fn get_block(
        &self,
        pos: FlintBlockPos,
        requested_nbt: &[String],
    ) -> Result<Block, anyhow::Error> {
        let steel_pos = flint_pos_to_steel(pos);
        let harness = self.harness.lock();
        if !harness.world().is_in_valid_bounds(steel_pos) {
            bail!(
                "block position [{}, {}, {}] is outside the Steel world bounds",
                pos[0],
                pos[1],
                pos[2]
            );
        }

        harness.ensure_chunk(ChunkPos::from_block_pos(steel_pos))?;

        let state = harness.world().get_block_state(steel_pos);
        let mut block = state_id_to_block(state)?;
        if requested_nbt.is_empty() {
            return Ok(block);
        }

        let Some(entity) = harness.world().get_block_entity(steel_pos) else {
            return Ok(block);
        };
        let full_nbt = NbtTag::Compound(entity.save_with_full_metadata());
        let mut values = Vec::with_capacity(requested_nbt.len());
        for requested_path in requested_nbt {
            let path = parse_nbt_path(requested_path)
                .with_context(|| format!("invalid requested block NBT path `{requested_path}`"))?;
            let matches = path.get(&full_nbt);
            match matches.as_slice() {
                [] => {}
                [value] => {
                    let value = to_canonical_snbt(value).with_context(|| {
                        format!("could not encode block NBT path `{requested_path}`")
                    })?;
                    values.push((requested_path.clone(), value));
                }
                _ => bail!(
                    "block NBT path `{requested_path}` selected {} values; Flint expects one",
                    matches.len()
                ),
            }
        }
        block.nbt = Some(EntityNbt::from_string_values(values));
        Ok(block)
    }

    fn set_block(&mut self, pos: FlintBlockPos, block: &Block) -> Result<(), anyhow::Error> {
        let state_id = flint_block_to_state_id(block)?;

        let steel_pos = flint_pos_to_steel(pos);
        let harness = self.harness.lock();
        if !harness.world().is_in_valid_bounds(steel_pos) {
            bail!(
                "block position [{}, {}, {}] is outside the Steel world bounds",
                pos[0],
                pos[1],
                pos[2]
            );
        }

        harness.ensure_chunk(ChunkPos::from_block_pos(steel_pos))?;

        let changed = harness
            .world()
            .set_block(steel_pos, state_id, UpdateFlags::UPDATE_ALL);
        if !changed && harness.world().get_block_state(steel_pos) != state_id {
            bail!(
                "Steel rejected block `{}` at [{}, {}, {}]",
                block.id,
                pos[0],
                pos[1],
                pos[2]
            );
        }
        if let Some(nbt) = &block.nbt {
            let Some(entity) = harness.world().get_block_entity(steel_pos) else {
                bail!(
                    "block `{}` at [{}, {}, {}] has NBT but no block entity",
                    block.id,
                    pos[0],
                    pos[1],
                    pos[2]
                );
            };
            let compound = parse_snbt_compound(&nbt.to_snbt())?;
            let mut bytes = Vec::new();
            compound.write(&mut bytes);
            let borrowed = read_compound(&mut Cursor::new(bytes.as_slice()))?;
            entity.load_additional(&borrowed);
            entity.set_changed();
        }
        Ok(())
    }

    fn summon_entity(
        &mut self,
        alias: &str,
        entity_type: &str,
        _pos: [f64; 3],
        _nbt: Option<&EntityNbt>,
    ) -> Result<(), anyhow::Error> {
        bail!(
            "entity actions are not supported by flint-steel yet: cannot summon `{entity_type}` as `{alias}`"
        )
    }

    fn teleport_entity(
        &mut self,
        alias: &str,
        _pos: [f64; 3],
        _rot: Option<[f32; 2]>,
    ) -> Result<(), anyhow::Error> {
        bail!("entity actions are not supported by flint-steel yet: cannot teleport `{alias}`")
    }

    fn get_entity(
        &self,
        alias: &str,
        _requested_nbt: &[String],
    ) -> Result<Vec<EntityState>, anyhow::Error> {
        bail!("entity assertions are not supported by flint-steel yet: cannot read `{alias}`")
    }

    fn find_entity(
        &self,
        entity_type: &str,
        _requested_nbt: &[String],
    ) -> Result<Vec<EntityState>, anyhow::Error> {
        bail!("entity assertions are not supported by flint-steel yet: cannot find `{entity_type}`")
    }

    fn create_player(&mut self) -> Result<Box<dyn FlintPlayer>, anyhow::Error> {
        Ok(Box::new(self.create_test_player()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_test_registries;

    #[test]
    fn test_world_creation() {
        init_test_registries();
        let world = SteelTestWorld::new().expect("test world should initialize");
        assert_eq!(world.current_tick(), 0);
        assert_eq!(world.get_time().expect("daytime should be readable"), 1_000);
    }

    #[test]
    fn test_world_tick() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");
        assert_eq!(world.current_tick(), 0);

        world.do_tick().expect("first tick should succeed");
        assert_eq!(world.current_tick(), 1);

        world.do_tick().expect("second tick should succeed");
        world.do_tick().expect("third tick should succeed");
        assert_eq!(world.current_tick(), 3);
        assert_eq!(
            world.get_time().expect("daytime should be readable"),
            1_000,
            "the canonical Flint config disables time advancement"
        );
    }

    #[test]
    fn test_get_air_by_default() {
        init_test_registries();
        let world = SteelTestWorld::new().expect("test world should initialize");
        let block = world
            .get_block([0, 64, 0], &[])
            .expect("default block should be readable");
        assert_eq!(block.id, "minecraft:air");
    }

    #[test]
    fn test_set_and_get_block() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");

        let stone = Block::new("minecraft:stone");
        world
            .set_block([0, 64, 0], &stone)
            .expect("stone should be placeable");

        let retrieved = world
            .get_block([0, 64, 0], &[])
            .expect("placed block should be readable");
        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn test_set_and_get_across_chunk_boundary() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");

        let stone = Block::new("minecraft:stone");
        world
            .set_block([18, 64, 0], &stone)
            .expect("set_block should load the destination chunk");
        let retrieved = world
            .get_block([18, 64, 0], &[])
            .expect("placed block should be readable");
        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn unknown_block_does_not_silently_pass() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");

        assert!(
            world
                .set_block([0, 64, 0], &Block::new("minecraft:not_a_real_block"))
                .is_err()
        );
    }

    #[test]
    fn rejected_block_position_does_not_silently_pass() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");

        assert!(
            world
                .set_block([0, 10_000, 0], &Block::new("minecraft:stone"))
                .is_err()
        );
    }

    #[test]
    fn placing_the_existing_state_is_a_valid_no_op() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");

        world
            .set_block([0, 64, 0], &Block::new("minecraft:air"))
            .expect("placing the existing state should succeed");
    }

    #[test]
    fn unsupported_entity_action_is_an_error() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");

        assert!(
            world
                .summon_entity("test", "minecraft:pig", [0.0, 64.0, 0.0], None)
                .is_err()
        );
    }

    #[test]
    fn test_set_air_clears_block() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");

        // Place a block
        let stone = Block::new("minecraft:stone");
        world
            .set_block([0, 64, 0], &stone)
            .expect("stone should be placeable");

        let retrieved = world
            .get_block([0, 64, 0], &[])
            .expect("placed block should be readable");
        assert_eq!(retrieved.id, "minecraft:stone");

        // Remove with air
        let air = Block::new("minecraft:air");
        world
            .set_block([0, 64, 0], &air)
            .expect("air should clear the block");

        let retrieved = world
            .get_block([0, 64, 0], &[])
            .expect("cleared block should be readable");
        assert_eq!(retrieved.id, "minecraft:air");
    }
}
