//! Core traits that server implementations must provide.
//!
//! Servers implement `FlintAdapter` to create test worlds, and `FlintWorld`/`FlintPlayer`
//! to provide the actual block and player operations.

use std::fmt::{Display, Formatter};

/// Position in world coordinates [x, y, z]
pub type BlockPos = [i32; 3];

/// An item that can be held or placed in a slot
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Item identifier, e.g., "minecraft:honeycomb"
    pub id: String,
    /// Stack count (default 1)
    pub count: u8,
}

impl Item {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            count: 1,
        }
    }

    pub fn with_count(id: impl Into<String>, count: u8) -> Self {
        Self {
            id: id.into(),
            count,
        }
    }
}

/// Block data returned from get_block
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockData {
    /// Block identifier, e.g., "minecraft:stone"
    pub id: String,
    /// Block state properties, e.g., {"powered": "true", "facing": "north"}
    pub properties: FxHashMap<String, String>,
}
impl Display for BlockData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)?;

        if !self.properties.is_empty() {
            write!(f, "[")?;

            let mut properties: Vec<_> = self.properties.iter().collect();
            properties.sort_by_key(|(key, _)| *key);

            for (i, (key, value)) in properties.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{}={}", key, value)?;
            }

            write!(f, "]")?;
        }

        Ok(())
    }
}

impl BlockData {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            properties: FxHashMap::default(),
        }
    }

    pub fn with_properties(id: impl Into<String>, properties: FxHashMap<String, String>) -> Self {
        Self {
            id: id.into(),
            properties,
        }
    }

    /// Check if this block is air
    pub fn is_air(&self) -> bool {
        self.id == "minecraft:air" || self.id == "air"
    }
}

/// Server metadata
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub minecraft_version: String,
}

/// Block specification from test file (used for set_block)
/// Re-exported from flint_core for convenience
pub use flint_core::test_spec::Block;
pub(crate) use flint_core::test_spec::{BlockFace, PlayerSlot};
use rustc_hash::FxHashMap;
use serde::Serialize;
// =============================================================================
// Core Traits
// =============================================================================

/// Main adapter trait - server implements this to create test worlds
pub trait FlintAdapter: Send + Sync {
    /// Create a new disposable in-memory test world
    fn create_test_world(&self) -> Box<dyn FlintWorld>;

    /// Server metadata for logging
    fn server_info(&self) -> ServerInfo;
}

/// World operations - server implements this
///
/// This is the minimal interface servers must provide.
/// Flint handles fill/clear by iterating `set_block()`.
pub trait FlintWorld: Send + Sync {
    /// Execute exactly one game tick
    fn do_tick(&mut self);

    /// Get current tick count
    fn current_tick(&self) -> u64;

    /// Get block at position
    fn get_block(&self, pos: BlockPos) -> BlockData;

    /// Set block at position (with neighbor updates)
    fn set_block(&mut self, pos: BlockPos, block: &Block);

    /// Create a simulated player in this world
    ///
    /// Only called when tests use `use_item_on` or player-related actions.
    /// Pure block tests (place, fill, assert) don't need a player.
    fn create_player(&mut self) -> Box<dyn FlintPlayer>;
}

/// Player operations - server implements this
///
/// Hybrid model: Server owns the player entity, but flint can:
/// - Manipulate inventory slots directly
/// - Select hotbar slots
/// - Trigger item use actions
pub trait FlintPlayer: Send + Sync {
    /// Set item in a slot (None = empty/clear the slot)
    fn set_slot(&mut self, slot: PlayerSlot, item: Option<&Item>);

    /// Get item from a slot (None if empty)
    fn get_slot(&self, slot: PlayerSlot) -> Option<Item>;

    /// Select which hotbar slot is active (1-9)
    fn select_hotbar(&mut self, slot: u8);

    /// Get currently selected hotbar slot (1-9)
    fn selected_hotbar(&self) -> u8;

    /// Use the item in the active hotbar slot on a block face
    ///
    /// This tests the server's actual interaction logic.
    fn use_item_on(&mut self, pos: BlockPos, face: &BlockFace);
}
