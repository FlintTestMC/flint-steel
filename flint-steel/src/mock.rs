//! Mock adapter for testing flint-steel without a real server.
//!
//! This module provides a simple in-memory implementation of the Flint traits
//! that can be used for unit testing and development.

use rustc_hash::FxHashMap;
use crate::traits::{
    Block, BlockData, BlockFace, BlockPos, FlintAdapter, FlintPlayer, FlintWorld, Item, PlayerSlot,
    ServerInfo,
};

/// Mock adapter for testing
pub struct MockAdapter;

impl MockAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FlintAdapter for MockAdapter {
    fn create_test_world(&self) -> Box<dyn FlintWorld> {
        Box::new(MockWorld::new())
    }

    fn server_info(&self) -> ServerInfo {
        ServerInfo {
            minecraft_version: "1.21".to_string(),
        }
    }
}

/// Mock world that stores blocks in a HashMap
pub struct MockWorld {
    blocks: FxHashMap<BlockPos, BlockData>,
    tick: u64,
}

impl MockWorld {
    pub fn new() -> Self {
        Self {
            blocks: FxHashMap::default(),
            tick: 0,
        }
    }

    /// Get all blocks in the world (for debugging/testing)
    pub fn all_blocks(&self) -> &FxHashMap<BlockPos, BlockData> {
        &self.blocks
    }
}

impl Default for MockWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl FlintWorld for MockWorld {
    fn do_tick(&mut self) {
        self.tick += 1;
    }

    fn current_tick(&self) -> u64 {
        self.tick
    }

    fn get_block(&self, pos: BlockPos) -> BlockData {
        self.blocks
            .get(&pos)
            .cloned()
            .unwrap_or_else(|| BlockData::new("minecraft:air"))
    }

    fn set_block(&mut self, pos: BlockPos, block: &Block) {
        let data = BlockData {
            id: block.id.clone(),
            properties: block
                .properties
                .iter()
                .map(|(k, v)| {
                    let value = match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    };
                    (k.clone(), value)
                })
                .collect(),
        };
        self.blocks.insert(pos, data);
    }

    fn create_player(&mut self) -> Box<dyn FlintPlayer> {
        Box::new(MockPlayer::new())
    }
}

/// Mock player with inventory
pub struct MockPlayer {
    slots: FxHashMap<PlayerSlot, Item>,
    selected_hotbar: u8,
}

impl MockPlayer {
    pub fn new() -> Self {
        Self {
            slots: FxHashMap::default(),
            selected_hotbar: 1,
        }
    }
}

impl Default for MockPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl FlintPlayer for MockPlayer {
    fn set_slot(&mut self, slot: PlayerSlot, item: Option<&Item>) {
        if let Some(item) = item {
            self.slots.insert(slot, item.clone());
        } else {
            self.slots.remove(&slot);
        }
    }

    fn get_slot(&self, slot: PlayerSlot) -> Option<Item> {
        self.slots.get(&slot).cloned()
    }

    fn select_hotbar(&mut self, slot: u8) {
        if (1..=9).contains(&slot) {
            self.selected_hotbar = slot;
        }
    }

    fn selected_hotbar(&self) -> u8 {
        self.selected_hotbar
    }

    fn use_item_on(&mut self, _pos: BlockPos, _face: &BlockFace) {
        // Mock implementation - does nothing
        // A real server would process the item use interaction
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Block;
    use serde_json::json;

    // ==========================================================================
    // MockAdapter Tests
    // ==========================================================================

    #[test]
    fn test_adapter_new() {
        let adapter = MockAdapter::new();
        let info = adapter.server_info();
        assert_eq!(info.minecraft_version, "1.21");
    }

    #[test]
    fn test_adapter_default() {
        let adapter = MockAdapter::new();
        let info = adapter.server_info();
        assert_eq!(info.minecraft_version, "1.21");
    }

    #[test]
    fn test_adapter_creates_world() {
        let adapter = MockAdapter::new();
        let world = adapter.create_test_world();
        assert_eq!(world.current_tick(), 0);
    }

    // ==========================================================================
    // MockWorld Tests
    // ==========================================================================

    #[test]
    fn test_world_new() {
        let world = MockWorld::new();
        assert_eq!(world.current_tick(), 0);
        assert!(world.all_blocks().is_empty());
    }

    #[test]
    fn test_world_default() {
        let world = MockWorld::default();
        assert_eq!(world.current_tick(), 0);
    }

    #[test]
    fn test_world_do_tick() {
        let mut world = MockWorld::new();
        assert_eq!(world.current_tick(), 0);

        world.do_tick();
        assert_eq!(world.current_tick(), 1);

        world.do_tick();
        world.do_tick();
        assert_eq!(world.current_tick(), 3);
    }

    #[test]
    fn test_world_get_block_returns_air_for_unset() {
        let world = MockWorld::new();
        let block = world.get_block([0, 64, 0]);
        assert_eq!(block.id, "minecraft:air");
        assert!(block.properties.is_empty());
    }

    #[test]
    fn test_world_set_and_get_block_simple() {
        let mut world = MockWorld::new();
        let stone = Block {
            id: "minecraft:stone".to_string(),
            properties: Default::default(),
        };

        world.set_block([0, 64, 0], &stone);
        let retrieved = world.get_block([0, 64, 0]);

        assert_eq!(retrieved.id, "minecraft:stone");
        assert!(retrieved.properties.is_empty());
    }

    #[test]
    fn test_world_set_and_get_block_with_string_properties() {
        let mut world = MockWorld::new();
        let mut properties = FxHashMap::default();
        properties.insert("facing".to_string(), json!("north"));
        properties.insert("half".to_string(), json!("top"));

        let stairs = Block {
            id: "minecraft:oak_stairs".to_string(),
            properties,
        };

        world.set_block([1, 65, 2], &stairs);
        let retrieved = world.get_block([1, 65, 2]);

        assert_eq!(retrieved.id, "minecraft:oak_stairs");
        assert_eq!(retrieved.properties.get("facing"), Some(&"north".to_string()));
        assert_eq!(retrieved.properties.get("half"), Some(&"top".to_string()));
    }

    #[test]
    fn test_world_set_and_get_block_with_bool_properties() {
        let mut world = MockWorld::new();
        let mut properties = FxHashMap::default();
        properties.insert("powered".to_string(), json!(true));
        properties.insert("lit".to_string(), json!(false));

        let lamp = Block {
            id: "minecraft:redstone_lamp".to_string(),
            properties,
        };

        world.set_block([0, 0, 0], &lamp);
        let retrieved = world.get_block([0, 0, 0]);

        assert_eq!(retrieved.id, "minecraft:redstone_lamp");
        assert_eq!(retrieved.properties.get("powered"), Some(&"true".to_string()));
        assert_eq!(retrieved.properties.get("lit"), Some(&"false".to_string()));
    }

    #[test]
    fn test_world_set_and_get_block_with_number_properties() {
        let mut world = MockWorld::new();
        let mut properties = FxHashMap::default();
        properties.insert("delay".to_string(), json!(2));
        properties.insert("facing".to_string(), json!("south"));

        let repeater = Block {
            id: "minecraft:repeater".to_string(),
            properties,
        };

        world.set_block([5, 64, 5], &repeater);
        let retrieved = world.get_block([5, 64, 5]);

        assert_eq!(retrieved.id, "minecraft:repeater");
        assert_eq!(retrieved.properties.get("delay"), Some(&"2".to_string()));
        assert_eq!(retrieved.properties.get("facing"), Some(&"south".to_string()));
    }

    #[test]
    fn test_world_overwrite_block() {
        let mut world = MockWorld::new();
        let pos = [10, 64, 10];

        let stone = Block {
            id: "minecraft:stone".to_string(),
            properties: Default::default(),
        };
        world.set_block(pos, &stone);
        assert_eq!(world.get_block(pos).id, "minecraft:stone");

        let dirt = Block {
            id: "minecraft:dirt".to_string(),
            properties: Default::default(),
        };
        world.set_block(pos, &dirt);
        assert_eq!(world.get_block(pos).id, "minecraft:dirt");
    }

    #[test]
    fn test_world_multiple_positions() {
        let mut world = MockWorld::new();

        let stone = Block {
            id: "minecraft:stone".to_string(),
            properties: Default::default(),
        };
        let dirt = Block {
            id: "minecraft:dirt".to_string(),
            properties: Default::default(),
        };
        let grass = Block {
            id: "minecraft:grass_block".to_string(),
            properties: Default::default(),
        };

        world.set_block([0, 0, 0], &stone);
        world.set_block([1, 1, 1], &dirt);
        world.set_block([2, 2, 2], &grass);

        assert_eq!(world.get_block([0, 0, 0]).id, "minecraft:stone");
        assert_eq!(world.get_block([1, 1, 1]).id, "minecraft:dirt");
        assert_eq!(world.get_block([2, 2, 2]).id, "minecraft:grass_block");
        assert_eq!(world.get_block([3, 3, 3]).id, "minecraft:air");
    }

    #[test]
    fn test_world_negative_coordinates() {
        let mut world = MockWorld::new();
        let stone = Block {
            id: "minecraft:stone".to_string(),
            properties: Default::default(),
        };

        world.set_block([-100, -64, -100], &stone);
        let retrieved = world.get_block([-100, -64, -100]);

        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn test_world_create_player() {
        let mut world = MockWorld::new();
        let player = world.create_player();
        assert_eq!(player.selected_hotbar(), 1);
    }

    #[test]
    fn test_world_all_blocks() {
        let mut world = MockWorld::new();
        let stone = Block {
            id: "minecraft:stone".to_string(),
            properties: Default::default(),
        };

        world.set_block([0, 0, 0], &stone);
        world.set_block([1, 1, 1], &stone);

        let blocks = world.all_blocks();
        assert_eq!(blocks.len(), 2);
        assert!(blocks.contains_key(&[0, 0, 0]));
        assert!(blocks.contains_key(&[1, 1, 1]));
    }

    // ==========================================================================
    // MockPlayer Tests
    // ==========================================================================

    #[test]
    fn test_player_new() {
        let player = MockPlayer::new();
        assert_eq!(player.selected_hotbar(), 1);
    }

    #[test]
    fn test_player_default() {
        let player = MockPlayer::default();
        assert_eq!(player.selected_hotbar(), 1);
    }

    #[test]
    fn test_player_select_hotbar_valid() {
        let mut player = MockPlayer::new();

        for slot in 1..=9 {
            player.select_hotbar(slot);
            assert_eq!(player.selected_hotbar(), slot);
        }
    }

    #[test]
    fn test_player_select_hotbar_invalid_zero() {
        let mut player = MockPlayer::new();
        player.select_hotbar(5);
        assert_eq!(player.selected_hotbar(), 5);

        player.select_hotbar(0);
        // Should remain unchanged
        assert_eq!(player.selected_hotbar(), 5);
    }

    #[test]
    fn test_player_select_hotbar_invalid_too_high() {
        let mut player = MockPlayer::new();
        player.select_hotbar(5);
        assert_eq!(player.selected_hotbar(), 5);

        player.select_hotbar(10);
        // Should remain unchanged
        assert_eq!(player.selected_hotbar(), 5);
    }

    #[test]
    fn test_player_set_and_get_slot() {
        let mut player = MockPlayer::new();
        let item = Item::new("minecraft:diamond_sword");

        player.set_slot(PlayerSlot::Hotbar1, Some(&item));
        let retrieved = player.get_slot(PlayerSlot::Hotbar1);

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "minecraft:diamond_sword");
        assert_eq!(retrieved.count, 1);
    }

    #[test]
    fn test_player_get_empty_slot() {
        let player = MockPlayer::new();
        let retrieved = player.get_slot(PlayerSlot::Hotbar1);
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_player_set_slot_with_count() {
        let mut player = MockPlayer::new();
        let item = Item::with_count("minecraft:cobblestone", 64);

        player.set_slot(PlayerSlot::Hotbar2, Some(&item));
        let retrieved = player.get_slot(PlayerSlot::Hotbar2).unwrap();

        assert_eq!(retrieved.id, "minecraft:cobblestone");
        assert_eq!(retrieved.count, 64);
    }

    #[test]
    fn test_player_clear_slot() {
        let mut player = MockPlayer::new();
        let item = Item::new("minecraft:stick");

        player.set_slot(PlayerSlot::Hotbar3, Some(&item));
        assert!(player.get_slot(PlayerSlot::Hotbar3).is_some());

        player.set_slot(PlayerSlot::Hotbar3, None);
        assert!(player.get_slot(PlayerSlot::Hotbar3).is_none());
    }

    #[test]
    fn test_player_multiple_slots() {
        let mut player = MockPlayer::new();

        let sword = Item::new("minecraft:diamond_sword");
        let pickaxe = Item::new("minecraft:diamond_pickaxe");
        let food = Item::with_count("minecraft:cooked_beef", 32);

        player.set_slot(PlayerSlot::Hotbar1, Some(&sword));
        player.set_slot(PlayerSlot::Hotbar2, Some(&pickaxe));
        player.set_slot(PlayerSlot::Hotbar9, Some(&food));

        assert_eq!(player.get_slot(PlayerSlot::Hotbar1).unwrap().id, "minecraft:diamond_sword");
        assert_eq!(player.get_slot(PlayerSlot::Hotbar2).unwrap().id, "minecraft:diamond_pickaxe");
        assert_eq!(player.get_slot(PlayerSlot::Hotbar9).unwrap().id, "minecraft:cooked_beef");
        assert_eq!(player.get_slot(PlayerSlot::Hotbar9).unwrap().count, 32);
    }

    #[test]
    fn test_player_overwrite_slot() {
        let mut player = MockPlayer::new();

        let sword = Item::new("minecraft:iron_sword");
        player.set_slot(PlayerSlot::Hotbar1, Some(&sword));
        assert_eq!(player.get_slot(PlayerSlot::Hotbar1).unwrap().id, "minecraft:iron_sword");

        let better_sword = Item::new("minecraft:diamond_sword");
        player.set_slot(PlayerSlot::Hotbar1, Some(&better_sword));
        assert_eq!(player.get_slot(PlayerSlot::Hotbar1).unwrap().id, "minecraft:diamond_sword");
    }

    #[test]
    fn test_player_use_item_on_does_not_panic() {
        let mut player = MockPlayer::new();
        // use_item_on is a no-op in mock, but should not panic
        player.use_item_on([0, 64, 0], &BlockFace::Top);
        player.use_item_on([-100, 0, 100], &BlockFace::Bottom);
    }

    // ==========================================================================
    // Integration Tests
    // ==========================================================================

    #[test]
    fn test_adapter_world_player_integration() {
        let adapter = MockAdapter::new();
        let mut world = adapter.create_test_world();

        // Place some blocks
        let stone = Block {
            id: "minecraft:stone".to_string(),
            properties: Default::default(),
        };
        world.set_block([0, 64, 0], &stone);

        // Advance ticks
        world.do_tick();
        world.do_tick();

        // Create player and set inventory
        let mut player = world.create_player();
        let item = Item::new("minecraft:honeycomb");
        player.set_slot(PlayerSlot::Hotbar1, Some(&item));
        player.select_hotbar(1);

        // Use item
        player.use_item_on([0, 64, 0], &BlockFace::Top);

        // Verify state
        assert_eq!(world.current_tick(), 2);
        assert_eq!(world.get_block([0, 64, 0]).id, "minecraft:stone");
        assert_eq!(player.get_slot(PlayerSlot::Hotbar1).unwrap().id, "minecraft:honeycomb");
    }
}
