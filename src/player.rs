//! Test player implementation for `SteelMC`.
//!
//! This implementation wraps a real `steel_core::player::Player` with a
//! `FlintConnection` to enable testing of player interactions (like `use_item_on`)
//! without real network connections.

use std::any::Any;
use std::sync;
use std::sync::Arc;

use flint_core::test_spec::{GameMode, PlayerSlot};
use flint_core::{FlintPlayer, Item};
use glam::DVec3;
use rustc_hash::FxHashMap;
use steel_core::config::RuntimeConfig;
use steel_core::entity::Entity;
use steel_core::inventory::container::Container;
use steel_core::player::game_mode;
use steel_core::player::player_inventory::PlayerInventory;
use steel_core::player::{ClientInformation, GameProfile, Player, PlayerConnection};
use steel_core::server::Server;
use steel_core::world::{ClipBlockShape, ClipFluid, World};
use steel_registry::item_stack::ItemStack;
use steel_registry::{REGISTRY, RegistryExt};
use steel_utils::Identifier;
use steel_utils::types::{GameType, InteractionHand};
use uuid::Uuid;

use crate::convert::clip_to_block_hit;
use crate::test_connection;
use crate::test_connection::FlintConnection;

/// Test player implementation that wraps a real `Player`.
///
/// This provides inventory management and enables calling real game logic
/// like `use_item_on` through the underlying player.
pub struct SteelTestPlayer {
    /// The real player instance.
    player: Arc<Player>,
    /// The test connection (kept for event inspection).
    #[allow(dead_code)]
    connection: FlintConnection,
}

impl SteelTestPlayer {
    /// Creates a new test player in the given world.
    pub fn new(world: Arc<World>) -> Self {
        // Create a test connection
        let connection = FlintConnection::new();
        let test_conn = connection.clone(); // shares inner state via Arc

        // Create a dummy game profile
        let game_profile = GameProfile {
            id: Uuid::new_v4(),
            name: "TestPlayer".to_string(),
            properties: vec![],
            profile_actions: None,
        };

        // Create the player with our test connection
        let player_connection = Arc::new(PlayerConnection::Other(Box::new(connection)));
        let runtime_config = Arc::new(RuntimeConfig {
            max_players: 20,
            view_distance: 10,
            simulation_distance: 10,
            max_chained_neighbor_updates: -1,
            online_mode: false,
            encryption: false,
            motd: String::new(),
            use_favicon: false,
            favicon: String::new(),
            enforce_secure_chat: false,
            compression: None,
            server_links: None,
            allow_flight: true,
            auth_server: None,
            chat_spam_threshold_seconds: 10,
            command_spam_threshold_seconds: 10,
            chunk_generation_threads: None,
            profile_server: None,
            packet_workers: None,
            chunk_encoding_threads: None,
        });
        let player = Arc::new({
            let p = Player::new(
                game_profile,
                player_connection,
                world,
                sync::Weak::<Server>::new(),
                runtime_config,
                -1, // Negative entity ID for test players
                ClientInformation::default(),
            );
            // Mark as loaded so interactions work
            p.set_client_loaded(true);
            p
        });

        Self {
            player,
            connection: test_conn,
        }
    }

    /// Gets the connection's recorded events (for test assertions).
    #[allow(dead_code)]
    #[must_use]
    pub fn get_events(&self) -> Vec<test_connection::PlayerEvent> {
        self.connection.get_events()
    }

    /// Clears the connection's recorded events.
    #[allow(dead_code)]
    pub fn clear_events(&self) {
        self.connection.clear_events();
    }

    /// Returns a reference to the underlying player.
    #[allow(dead_code)]
    #[must_use]
    pub const fn player(&self) -> &Arc<Player> {
        &self.player
    }
}

/// Converts a Flint [`PlayerSlot`] to a Steel inventory slot index.
///
/// Flint uses semantic slot names (e.g., `Hotbar1`, `OffHand`, `Helmet`),
/// while Steel uses numeric indices. This function maps between the two:
/// - Hotbar slots 1-9 → indices 0-8
/// - `OffHand` → `PlayerInventory::SLOT_OFFHAND`
/// - Armor slots → indices 36-39 (boots to helmet)
const fn player_slot_to_index(slot: PlayerSlot) -> usize {
    match slot {
        PlayerSlot::Hotbar1 => 0,
        PlayerSlot::Hotbar2 => 1,
        PlayerSlot::Hotbar3 => 2,
        PlayerSlot::Hotbar4 => 3,
        PlayerSlot::Hotbar5 => 4,
        PlayerSlot::Hotbar6 => 5,
        PlayerSlot::Hotbar7 => 6,
        PlayerSlot::Hotbar8 => 7,
        PlayerSlot::Hotbar9 => 8,
        PlayerSlot::OffHand => PlayerInventory::SLOT_OFFHAND,
        PlayerSlot::Boots => 36,
        PlayerSlot::Leggings => 37,
        PlayerSlot::Chestplate => 38,
        PlayerSlot::Helmet => 39,
    }
}

/// Converts a Flint [`Item`] to a Steel [`ItemStack`].
///
/// Handles the `minecraft:` namespace prefix (strips it if present) and
/// looks up the item in the registry. Returns an empty stack if the item
/// is not found.
fn flint_item_to_stack(item: &Item) -> ItemStack {
    // Parse the item ID - may have "minecraft:" prefix
    let item_id = if item.id.starts_with("minecraft:") {
        &item.id[10..]
    } else {
        &item.id
    };

    let identifier = Identifier::vanilla(item_id.to_string());

    // Look up the item in the registry
    if let Some(item_ref) = REGISTRY.items.by_key(&identifier) {
        ItemStack::with_count(item_ref, i32::from(item.count))
    } else {
        tracing::warn!("Unknown item: {} - returning empty stack", item.id);
        ItemStack::empty()
    }
}

/// Converts a Steel [`ItemStack`] to a Flint [`Item`].
///
/// Returns `None` for empty stacks. Adds the `minecraft:` namespace prefix
/// to the item ID for consistency with Flint's expected format.
fn stack_to_flint_item(stack: &ItemStack, requested_data: Vec<String>) -> Option<Item> {
    if stack.is_empty() {
        return None;
    }

    let id = format!("minecraft:{}", stack.item.key.path);
    let mut map: FxHashMap<String, String> = FxHashMap::default();
    for key in requested_data {
        if let Some(data) = stack.get_effective_value_raw(&Identifier::vanilla(key.clone())) {
            if data.downcast_ref::<()>().is_some() {
                map.insert(key, String::new());
            } else if let Some(b) = data.downcast_ref::<bool>() {
                map.insert(key, b.to_string());
            } else if let Some(i) = data.downcast_ref::<i32>() {
                map.insert(key, i.to_string());
            } else if let Some(f) = data.downcast_ref::<f32>() {
                map.insert(key, f.to_string());
            }
            // TODO: handle other data types then needed
        }
    }
    Some(Item {
        id,
        count: stack.count.try_into().unwrap_or(1),
        data: map,
    })
}

impl FlintPlayer for SteelTestPlayer {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn set_slot(&mut self, slot: PlayerSlot, item: Option<&Item>) -> Result<(), anyhow::Error> {
        let index = player_slot_to_index(slot);
        let stack = item.map_or_else(ItemStack::empty, flint_item_to_stack);

        let mut inv = self.player.inventory.lock();
        inv.set_item(index, stack);
        Ok(())
    }

    fn get_slot(
        &mut self,
        slot: PlayerSlot,
        requested_data: Vec<String>,
    ) -> Result<Option<Item>, anyhow::Error> {
        let index = player_slot_to_index(slot);

        let inv = self.player.inventory.lock();
        let stack = inv.get_item(index);
        Ok(stack_to_flint_item(stack, requested_data))
    }

    fn select_hotbar(&mut self, slot: u8) -> Result<(), anyhow::Error> {
        if (1..=9).contains(&slot) {
            // Flint uses 1-9, Steel uses 0-8
            self.player.inventory.lock().set_selected_slot(slot - 1);
        }
        Ok(())
    }

    fn selected_hotbar(&self) -> u8 {
        // Steel uses 0-8, Flint uses 1-9
        self.player.inventory.lock().get_selected_slot() + 1
    }

    fn teleport(&mut self, pos: [f64; 3], rot: Option<[f32; 2]>) -> Result<(), anyhow::Error> {
        self.player
            .try_set_position(DVec3::new(pos[0], pos[1], pos[2]))?;
        self.player.set_rotation((rot.unwrap_or([0.0, 0.0])).into());
        Ok(())
    }

    fn interact(&mut self) -> Result<(), anyhow::Error> {
        let world = self.player.get_world();
        let (start, end) = self.player.get_ray_endpoints();
        let clip = world.clip(start, end, ClipBlockShape::Outline, ClipFluid::None);

        let hand = InteractionHand::MainHand;
        let result = if clip.is_miss() {
            game_mode::use_item(&self.player, &world, hand)
        } else {
            game_mode::use_item_on(&self.player, &world, hand, &clip_to_block_hit(clip))
        };

        if result.should_swing_server() {
            self.player.swing(hand, true);
        }
        self.player.broadcast_inventory_changes();

        tracing::debug!("interact({start:?} -> {end:?}) -> {result:?}");
        Ok(())
    }

    fn set_game_mode(&mut self, mode: GameMode) -> Result<(), anyhow::Error> {
        self.player.set_game_mode(match mode {
            GameMode::Survival => GameType::Survival,
            GameMode::Creative => GameType::Creative,
            GameMode::Adventure => GameType::Adventure,
            GameMode::Spectator => GameType::Spectator,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_test_registries;
    use crate::world::SteelTestWorld;
    use flint_core::FlintWorld;

    #[test]
    fn test_inventory() {
        init_test_registries();
        let mut world = SteelTestWorld::new();
        let mut player = world.create_player();

        let item = Item::new("minecraft:stone");
        player
            .set_slot(PlayerSlot::Hotbar1, Some(&item))
            .expect("TODO: panic message");

        let retrieved = player
            .get_slot(PlayerSlot::Hotbar1, vec![])
            .expect("get_slot failed")
            .expect("Slot not found");
        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn test_hotbar_selection() {
        init_test_registries();
        let mut world = SteelTestWorld::new();
        let mut player = world.create_player();

        // Default is slot 1
        assert_eq!(player.selected_hotbar(), 1);

        player.select_hotbar(5).expect("TODO: panic message");
        assert_eq!(player.selected_hotbar(), 5);

        // Out of range values should be ignored
        player.select_hotbar(0).expect("TODO: panic message");
        assert_eq!(player.selected_hotbar(), 5);

        player.select_hotbar(10).expect("TODO: panic message");
        assert_eq!(player.selected_hotbar(), 5);
    }
}
