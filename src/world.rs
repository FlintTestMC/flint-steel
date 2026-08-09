//! Test world implementation using the real steel-core World.
//!
//! This module provides a test world that wraps the real `Arc<World>` from steel-core,
//! configured with RAM-only storage for instant chunk creation without disk I/O.

use std::io::Cursor;
use std::iter;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use flint_core::Block;
use flint_core::test_spec::EntityNbt;
use flint_core::{BlockPos as FlintBlockPos, FlintPlayer, FlintWorld};
use rustc_hash::FxHashMap;
use simdnbt::borrow::read_compound;
use steel_core::chunk::chunk_request::{ChunkRequestHandle, ChunkRequestState, ChunkTicketKind};
use steel_core::chunk::status::ChunkStatus;
use steel_core::level_data::WorldGenerationSettings;
use steel_core::world::{LevelReader, World, WorldConfig, WorldStorageConfig};
use steel_core::worldgen::{ChunkGeneratorType, EmptyChunkGenerator};
use steel_registry::vanilla_dimension_types::OVERWORLD;
use steel_utils::Identifier;
use steel_utils::locks::SyncMutex;
use steel_utils::nbt::parse_snbt_compound;
use steel_utils::types::{Difficulty, GameType};
use steel_utils::{BlockPos, ChunkPos, types::UpdateFlags};

use crate::convert::{flint_block_to_state_id, flint_pos_to_steel, state_id_to_block};
use crate::player::SteelTestPlayer;
use crate::runtime;

/// Test world implementation using the real steel-core World.
///
/// This wraps an `Arc<World>` configured with RAM-only storage:
/// - Chunks are created empty (all air) on-demand
/// - No disk I/O, no chunk generation delay
/// - Full block behavior system (neighbors, shapes, etc.)
/// - Real tick processing
pub struct SteelTestWorld {
    /// The underlying steel-core world.
    world: Arc<World>,
    /// Current tick count (for `FlintWorld` trait).
    tick: AtomicU64,
    /// Active chunk requests, keyed by chunk position.
    ///
    /// Each handle owns the chunk's tickets; retaining it for the world's
    /// lifetime keeps the chunk permanently loaded (it unloads on drop).
    chunk_requests: SyncMutex<FxHashMap<ChunkPos, ChunkRequestHandle>>,
}

impl SteelTestWorld {
    /// Creates a new test world with RAM-only storage.
    ///
    /// The world uses the overworld dimension type and starts with seed 0.
    /// All chunks are created empty on-demand.
    ///
    /// # Panic
    /// shouldn't panic only something is completely broken and then it is ok
    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn new() -> Self {
        let rt = runtime();

        let dim_id = Identifier::vanilla_static("overworld");

        // Create world with RAM-only storage
        let config = WorldConfig {
            storage: WorldStorageConfig::RamOnly,
            level_data_path: None,
            generator: Arc::new(ChunkGeneratorType::Empty(EmptyChunkGenerator::new())),
            generation_settings: WorldGenerationSettings {
                generator: Identifier::new("steel", "empty"),
                config: toml::Value::Table(toml::value::Table::new()),
                dimension_type: dim_id.clone(),
                min_y: OVERWORLD.min_y,
                height: OVERWORLD.height,
            },
            view_distance: 10,
            simulation_distance: 10,
            max_chained_neighbor_updates: -1,
            compression: None,
            is_flat: false,
            sea_level: 63,
            default_gamemode: GameType::Survival,
            difficulty: Difficulty::Normal,
        };

        let generation_pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .build()
                .expect("Failed to create rayon thread pool"),
        );

        // Block on async world creation
        let world = rt
            .block_on(async {
                World::new_with_config(rt.clone(), dim_id, &OVERWORLD, 0, config, generation_pool)
                    .await
            })
            .expect("Failed to create test world");

        Self {
            world,
            tick: AtomicU64::new(0),
            chunk_requests: SyncMutex::new(FxHashMap::default()),
        }
    }

    /// Gets a reference to the underlying steel-core world.
    #[must_use]
    pub const fn inner(&self) -> &Arc<World> {
        &self.world
    }

    /// Ensures the chunk containing the given block position is loaded.
    ///
    /// This is intended for testing only. It blocks until the chunk is loaded
    /// from storage. For RAM-only storage, this creates empty chunks on-demand.
    fn ensure_chunk_at(&self, pos: &BlockPos) {
        self.ensure_chunk(ChunkPos::new(pos.x() >> 4, pos.z() >> 4));
    }

    /// Ensures the chunk at `chunk_pos` is loaded, retaining its ticket handle.
    fn ensure_chunk(&self, chunk_pos: ChunkPos) {
        // Fast path: a retained handle that is already Ready means the chunk
        // is loaded at Full and its ticket is held — nothing to do.
        if let Some(handle) = self.chunk_requests.lock().get(&chunk_pos)
            && handle.poll() == ChunkRequestState::Ready
        {
            return;
        }

        // Retain the handle so the ticket stays alive for the world's
        // lifetime (the chunk would unload if the handle were dropped).
        let handle = self.drive_chunk_request(chunk_pos);
        self.chunk_requests.lock().insert(chunk_pos, handle);
    }

    /// Requests the chunk at `chunk_pos` and blocks until it reaches `Full`.
    ///
    /// `World::tick_game` does not drive chunk scheduling (in production that
    /// runs on a separate loop), so this drives scheduling itself via the
    /// `flint`-gated `ChunkMap::drive_scheduling_for_flint` hook.
    /// Scheduling must keep being driven until the center generation task is
    /// spawned: `ChunkGenerationTask::new` reads every neighbour holder in the
    /// generation radius and panics if one is missing, and those holders are
    /// only created by ticket propagation across multiple scheduling ticks.
    /// Once the task is spawned it self-drives sub-layers via `apply_step`.
    ///
    /// The returned handle owns the chunk's ticket and must be retained.
    ///
    /// # Panics
    /// Panics if the chunk does not reach `Full` within 30 seconds, or if the
    /// request becomes disallowed/cancelled. This is a test framework: a
    /// missing chunk silently corrupts every downstream assertion, so failing
    /// loudly is correct.
    fn drive_chunk_request(&self, chunk_pos: ChunkPos) -> ChunkRequestHandle {
        let chunk_map = &self.world.chunk_map;

        // Ticket-owned request: adds a ticket and lets the normal scheduling /
        // generation pipeline create the holder and generate it to Full.
        let handle =
            chunk_map.request_chunk(chunk_pos, ChunkStatus::Full, ChunkTicketKind::Command);

        let deadline = Instant::now() + Duration::from_secs(30);

        while Instant::now() < deadline {
            chunk_map.advance_scheduling();

            match handle.poll() {
                ChunkRequestState::Ready => return handle,
                ChunkRequestState::Cancelled => {
                    panic!("chunk {chunk_pos:?} request was cancelled before reaching Full")
                }
                ChunkRequestState::Pending { .. } => thread::sleep(Duration::from_millis(1)),
            }
        }

        panic!("chunk {chunk_pos:?} did not reach Full status within 30s");
    }
}

impl Default for SteelTestWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl FlintWorld for SteelTestWorld {
    fn do_tick(&mut self) -> Result<(), anyhow::Error> {
        let tick_count = self.tick.fetch_add(1, Ordering::SeqCst);

        // Run a real world tick
        // Note: For testing we run with `runs_normally = true`
        self.world.tick_game(tick_count, true);
        Ok(())
    }

    fn current_tick(&self) -> u64 {
        self.tick.load(Ordering::SeqCst)
    }

    fn get_time(&self) -> Result<u64, anyhow::Error> {
        Ok(self.world.game_time() as u64)
    }

    fn get_block(
        &self,
        pos: FlintBlockPos,
        requested_nbt: &[String],
    ) -> Result<Block, anyhow::Error> {
        let steel_pos = flint_pos_to_steel(pos);

        // Ensure the chunk is loaded (for RAM storage this creates empty chunks)
        self.ensure_chunk_at(&steel_pos);

        let state = self.world.get_block_state(steel_pos);
        let mut block = state_id_to_block(state);
        if let Some(entity) = self.world.get_block_entity(steel_pos) {
            block.nbt = Some(EntityNbt::from_string_values(iter::empty()));
        }
        Ok(block)
    }

    fn set_block(&mut self, pos: FlintBlockPos, block: &Block) -> Result<(), anyhow::Error> {
        let Some(state_id) = flint_block_to_state_id(block) else {
            tracing::warn!("Unknown block: {} - skipping placement", block.id);
            return Ok(());
        };

        let steel_pos = flint_pos_to_steel(pos);

        // Ensure the chunk is loaded before setting blocks
        self.ensure_chunk_at(&steel_pos);

        self.world
            .set_block(steel_pos, state_id, UpdateFlags::UPDATE_ALL);
        if let Some(nbt) = &block.nbt
            && let Some(entity) = self.world.get_block_entity(steel_pos)
        {
            let compound = parse_snbt_compound(&nbt.to_snbt())?;
            let mut bytes = Vec::new();
            compound.write(&mut bytes);
            let borrowed = read_compound(&mut Cursor::new(bytes.as_slice()))?;
            entity.load_additional(&borrowed);
            entity.set_changed();
        }
        Ok(())
    }

    fn create_player(&mut self) -> Box<dyn FlintPlayer> {
        Box::new(SteelTestPlayer::new(self.world.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_test_registries;

    #[test]
    fn test_world_creation() {
        init_test_registries();
        let world = SteelTestWorld::new();
        assert_eq!(world.current_tick(), 0);
    }

    #[test]
    fn test_world_tick() {
        init_test_registries();
        let mut world = SteelTestWorld::new();
        assert_eq!(world.current_tick(), 0);

        world.do_tick().expect("TODO: panic message");
        assert_eq!(world.current_tick(), 1);

        world.do_tick().expect("TODO: panic message");
        world.do_tick().expect("TODO: panic message");
        assert_eq!(world.current_tick(), 3);
    }

    #[test]
    fn test_get_air_by_default() {
        init_test_registries();
        let world = SteelTestWorld::new();
        let block = world.get_block([0, 64, 0], &[]).unwrap();
        // Empty chunks are filled with air (or void_air depending on implementation)
        assert!(
            block.id == "minecraft:air" || block.id == "minecraft:void_air",
            "Expected air or void_air, got: {}",
            block.id
        );
    }

    #[test]
    fn test_set_and_get_block() {
        init_test_registries();
        let mut world = SteelTestWorld::new();

        let stone = Block::new("minecraft:stone");
        world
            .set_block([0, 64, 0], &stone)
            .expect("TODO: panic message");

        let retrieved = world.get_block([0, 64, 0], &[]).unwrap();
        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn test_preload_region_loads_chunks_spanning_multiple_blocks() {
        init_test_registries();
        let mut world = SteelTestWorld::new();

        // Region spans chunk (0,0) and chunk (1,0): x=0..=20 crosses the x=16 chunk border.
        world
            .preload_region([[0, 60, 0], [20, 70, 0]])
            .expect("preload should succeed");

        // get_block/set_block must not have to drive a fresh chunk request afterwards.
        let stone = Block::new("minecraft:stone");
        world
            .set_block([18, 64, 0], &stone)
            .expect("set_block in preloaded chunk 1 should not need to load anything");
        let retrieved = world.get_block([18, 64, 0], &[]).unwrap();
        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn test_set_air_clears_block() {
        init_test_registries();
        let mut world = SteelTestWorld::new();

        // Place a block
        let stone = Block::new("minecraft:stone");
        world
            .set_block([0, 64, 0], &stone)
            .expect("TODO: panic message");

        let retrieved = world.get_block([0, 64, 0], &[]).unwrap();
        assert_eq!(retrieved.id, "minecraft:stone");

        // Remove with air
        let air = Block::new("minecraft:air");
        let _ = world.set_block([0, 64, 0], &air);

        let retrieved = world.get_block([0, 64, 0], &[]).unwrap();
        // Accept both air and void_air as valid "cleared" states
        assert!(
            retrieved.id == "minecraft:air" || retrieved.id == "minecraft:void_air",
            "Expected air or void_air, got: {}",
            retrieved.id
        );
    }
}
