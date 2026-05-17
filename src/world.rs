//! Test world implementation using the real steel-core World.
//!
//! This module provides a test world that wraps the real `Arc<World>` from steel-core,
//! configured with RAM-only storage for instant chunk creation without disk I/O.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use flint_core::Block;
use flint_core::{BlockPos as FlintBlockPos, FlintPlayer, FlintWorld};
use rustc_hash::FxHashMap;
use steel_core::chunk::chunk_access::ChunkStatus;
use steel_core::chunk::chunk_request::{ChunkRequestHandle, ChunkRequestState, ChunkTicketKind};
use steel_core::level_data::WorldGenerationSettings;
use steel_core::world::{World, WorldConfig, WorldStorageConfig};
use steel_core::worldgen::{ChunkGeneratorType, EmptyChunkGenerator};
use steel_registry::vanilla_dimension_types::OVERWORLD;
use steel_utils::Identifier;
use steel_utils::locks::SyncMutex;
use steel_utils::types::{Difficulty, GameType};
use steel_utils::{BlockPos, ChunkPos, types::UpdateFlags};
use tokio::time::timeout;

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
        let chunk_pos = ChunkPos::new(pos.x() >> 4, pos.z() >> 4);

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
    /// runs on a separate loop), so this drives `tick_scheduling` itself.
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

        let rt = runtime();
        let deadline = Instant::now() + Duration::from_secs(30);

        while Instant::now() < deadline {
            chunk_map.tick_scheduling();

            // The holder is created by ticket propagation inside
            // `tick_scheduling`; it may not exist on the first iterations.
            let Some(holder) = chunk_map
                .chunks
                .read_sync(&chunk_pos, |_, holder| holder.clone())
            else {
                continue;
            };

            // Race the real status-change notification against a short timeout
            // so we return the instant the chunk hits Full, while still
            // re-driving scheduling if it is not ready yet.
            match rt.block_on(async {
                timeout(
                    Duration::from_millis(1),
                    holder.await_chunk(ChunkStatus::Full),
                )
                .await
            }) {
                Ok(Some(_)) => return handle,
                Ok(None) => panic!(
                    "chunk {chunk_pos:?} request became disallowed or cancelled before reaching Full"
                ),
                Err(_) => {} // timed out waiting; re-drive scheduling
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
    fn do_tick(&mut self) {
        let tick_count = self.tick.fetch_add(1, Ordering::SeqCst);

        // Run a real world tick
        // Note: For testing we run with `runs_normally = true`
        self.world.tick_game(tick_count, true);
    }

    fn current_tick(&self) -> u64 {
        self.tick.load(Ordering::SeqCst)
    }

    fn get_block(&self, pos: FlintBlockPos) -> Block {
        let steel_pos = flint_pos_to_steel(pos);

        // Ensure the chunk is loaded (for RAM storage this creates empty chunks)
        self.ensure_chunk_at(&steel_pos);

        let state = self.world.get_block_state(steel_pos);
        state_id_to_block(state)
    }

    fn set_block(&mut self, pos: FlintBlockPos, block: &Block) {
        let Some(state_id) = flint_block_to_state_id(block) else {
            tracing::warn!("Unknown block: {} - skipping placement", block.id);
            return;
        };

        let steel_pos = flint_pos_to_steel(pos);

        // Ensure the chunk is loaded before setting blocks
        self.ensure_chunk_at(&steel_pos);

        // Use the real World::set_block which handles:
        // - Neighbor updates
        // - Shape updates
        // - Block behavior callbacks (on_place, etc.)
        self.world
            .set_block(steel_pos, state_id, UpdateFlags::UPDATE_ALL);
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

        world.do_tick();
        assert_eq!(world.current_tick(), 1);

        world.do_tick();
        world.do_tick();
        assert_eq!(world.current_tick(), 3);
    }

    #[test]
    fn test_get_air_by_default() {
        init_test_registries();
        let world = SteelTestWorld::new();
        let block = world.get_block([0, 64, 0]);
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
        world.set_block([0, 64, 0], &stone);

        let retrieved = world.get_block([0, 64, 0]);
        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn test_set_air_clears_block() {
        init_test_registries();
        let mut world = SteelTestWorld::new();

        // Place a block
        let stone = Block::new("minecraft:stone");
        world.set_block([0, 64, 0], &stone);

        let retrieved = world.get_block([0, 64, 0]);
        assert_eq!(retrieved.id, "minecraft:stone");

        // Remove with air
        let air = Block::new("minecraft:air");
        world.set_block([0, 64, 0], &air);

        let retrieved = world.get_block([0, 64, 0]);
        // Accept both air and void_air as valid "cleared" states
        assert!(
            retrieved.id == "minecraft:air" || retrieved.id == "minecraft:void_air",
            "Expected air or void_air, got: {}",
            retrieved.id
        );
    }
}
