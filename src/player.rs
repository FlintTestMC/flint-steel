//! Test player implementation for `SteelMC`.
//!
//! This implementation retains Steel's test-player owner so player attachment,
//! interaction, connection recording, and teardown use Steel's harness lifecycle.

use std::any::Any;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use flint_core::test_spec::{GameMode, PlayerSlot};
use flint_core::{FlintPlayer, Item};
use glam::DVec3;
use rustc_hash::FxHashMap;
use steel_core::entity::Entity;
use steel_core::inventory::container::Container;
use steel_core::player::Player;
use steel_core::player::game_mode;
use steel_core::player::player_inventory::PlayerInventory;
use steel_core::test_harness::{InMemoryWorld, RecordedConnectionEvent, TestPlayer};
use steel_core::world::{ClipBlockShape, ClipFluid};
use steel_registry::data_components::DataComponentPatch;
use steel_registry::item_stack::ItemStack;
use steel_registry::{REGISTRY, RegistryExt};
use steel_utils::ChunkPos;
use steel_utils::Identifier;
use steel_utils::locks::SyncMutex;
use steel_utils::nbt::{parse_snbt, to_canonical_snbt};
use steel_utils::types::{GameType, InteractionHand};

use crate::convert::{clip_to_block_hit, registry_key_to_flint_id};

/// Test player implementation that wraps a real `Player`.
///
/// This provides inventory management and enables calling real game logic
/// like `use_item_on` through the underlying player.
pub struct SteelTestPlayer {
    test_player: TestPlayer,
    harness: Arc<SyncMutex<InMemoryWorld>>,
}

impl SteelTestPlayer {
    /// Wraps a player attached by Steel's test harness.
    #[must_use]
    pub(crate) fn new(test_player: TestPlayer, harness: Arc<SyncMutex<InMemoryWorld>>) -> Self {
        test_player.player().set_client_loaded(true);
        Self {
            test_player,
            harness,
        }
    }

    /// Gets the connection's recorded events (for test assertions).
    #[allow(dead_code)]
    #[must_use]
    pub fn get_events(&self) -> Vec<RecordedConnectionEvent> {
        self.test_player.connection().events()
    }

    /// Clears the connection's recorded events.
    #[allow(dead_code)]
    pub fn clear_events(&self) {
        self.test_player.connection().clear();
    }

    /// Returns a reference to the underlying player.
    #[allow(dead_code)]
    #[must_use]
    pub const fn player(&self) -> &Arc<Player> {
        self.test_player.player()
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
/// Looks up the item and data components in Steel's registries.
fn flint_item_to_stack(item: &Item) -> Result<ItemStack> {
    if item.count == 0 {
        if item.id != "air" && item.id != "minecraft:air" {
            bail!("non-air item `{}` cannot have a count of zero", item.id);
        }
        if !item.data.is_empty() {
            bail!("empty air item cannot contain component data");
        }
        return Ok(ItemStack::empty());
    }

    let identifier = item
        .id
        .parse::<Identifier>()
        .map_err(|error| anyhow!("invalid item identifier `{}`: {error}", item.id))?;
    let item_ref = REGISTRY
        .items
        .by_key(&identifier)
        .with_context(|| format!("unknown item `{identifier}`"))?;

    let mut patch = DataComponentPatch::new();
    for (key, value) in &item.data {
        let identifier = key
            .parse::<Identifier>()
            .map_err(|error| anyhow!("invalid item component identifier `{key}`: {error}"))?;
        let entry = REGISTRY
            .data_components
            .by_key(&identifier)
            .with_context(|| format!("unknown item component `{identifier}`"))?;
        if !entry.is_persistent() {
            bail!("item component `{identifier}` has no persistent codec");
        }
        let tag = parse_snbt(value)
            .with_context(|| format!("invalid SNBT for item component `{identifier}`"))?;
        let component = entry.read_nbt_owned(&tag).with_context(|| {
            format!("value `{value}` is invalid for item component `{identifier}`")
        })?;
        if !patch.set_raw(identifier.clone(), component) {
            bail!("could not apply item component `{identifier}`");
        }
    }

    let stack = ItemStack::with_count_and_patch(item_ref, i32::from(item.count), patch);
    stack
        .validate_strict()
        .with_context(|| format!("invalid stack for item `{identifier}`"))?;
    Ok(stack)
}

/// Converts a Steel [`ItemStack`] to a Flint [`Item`].
///
/// Returns `None` for empty stacks and preserves the full registry namespace.
fn stack_to_flint_item(stack: &ItemStack, requested_data: Vec<String>) -> Result<Option<Item>> {
    if stack.is_empty() {
        return Ok(None);
    }

    let id = registry_key_to_flint_id(&stack.item.key);
    let mut map: FxHashMap<String, String> = FxHashMap::default();
    for key in requested_data {
        let identifier = key
            .parse::<Identifier>()
            .map_err(|error| anyhow!("invalid requested item component `{key}`: {error}"))?;
        let entry = REGISTRY
            .data_components
            .by_key(&identifier)
            .with_context(|| format!("unknown requested item component `{identifier}`"))?;
        let Some(data) = stack.get_effective_value_raw(&identifier) else {
            continue;
        };
        let tag = entry
            .write_nbt(data)
            .with_context(|| format!("could not encode item component `{identifier}`"))?;
        let value = to_canonical_snbt(&tag)
            .with_context(|| format!("could not render item component `{identifier}` as SNBT"))?;
        map.insert(key, value);
    }
    let count = stack
        .count()
        .try_into()
        .with_context(|| format!("item stack count {} does not fit Flint's u8", stack.count()))?;
    Ok(Some(Item {
        id,
        count,
        data: map,
    }))
}

impl FlintPlayer for SteelTestPlayer {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn set_slot(&mut self, slot: PlayerSlot, item: Option<&Item>) -> Result<(), anyhow::Error> {
        let index = player_slot_to_index(slot);
        let stack = match item {
            Some(item) => flint_item_to_stack(item)?,
            None => ItemStack::empty(),
        };

        let mut inv = self.player().inventory.lock();
        inv.set_item(index, stack);
        Ok(())
    }

    fn get_slot(
        &mut self,
        slot: PlayerSlot,
        requested_data: Vec<String>,
    ) -> Result<Option<Item>, anyhow::Error> {
        let index = player_slot_to_index(slot);

        let inv = self.player().inventory.lock();
        let stack = inv.get_item(index);
        stack_to_flint_item(stack, requested_data)
    }

    fn select_hotbar(&mut self, slot: u8) -> Result<(), anyhow::Error> {
        if !(1..=9).contains(&slot) {
            bail!("invalid hotbar slot {slot}; expected a value from 1 through 9");
        }
        // Flint uses 1-9, Steel uses 0-8
        self.player().inventory.lock().set_selected_slot(slot - 1);
        Ok(())
    }

    fn selected_hotbar(&self) -> u8 {
        // Steel uses 0-8, Flint uses 1-9
        self.player().inventory.lock().get_selected_slot() + 1
    }

    fn teleport(&mut self, pos: [f64; 3], rot: Option<[f32; 2]>) -> Result<(), anyhow::Error> {
        let pos = DVec3::new(pos[0], pos[1], pos[2]);
        self.harness
            .lock()
            .ensure_entity_chunk(ChunkPos::from_entity_pos(pos))?;
        let (yaw, pitch) = rot.map_or_else(|| self.player().rotation(), Into::into);
        self.player().teleport(pos, yaw, pitch)?;
        Ok(())
    }

    fn interact(&mut self) -> Result<(), anyhow::Error> {
        let world = self.player().get_world();
        let (start, end) = self.player().get_ray_endpoints();
        let clip = world.clip(start, end, ClipBlockShape::Outline, ClipFluid::None);

        let hand = InteractionHand::MainHand;
        let result = if clip.is_miss() {
            game_mode::use_item(self.player(), &world, hand)
        } else {
            game_mode::use_item_on(self.player(), &world, hand, &clip_to_block_hit(clip))
        };

        if result.should_swing_server() {
            self.player().swing(hand, true);
        }
        self.player().broadcast_inventory_changes();

        Ok(())
    }

    fn set_game_mode(&mut self, mode: GameMode) -> Result<(), anyhow::Error> {
        self.player().set_game_mode(match mode {
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
    use crate::convert::flint_block_to_state_id;
    use crate::init_test_registries;
    use crate::world::SteelTestWorld;
    use flint_core::{Block, FlintWorld};
    use steel_registry::vanilla_items;
    use steel_utils::{BlockPos, types::UpdateFlags};

    #[test]
    fn test_inventory() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");
        let mut player = world.create_player().expect("test player should attach");

        let item = Item::new("minecraft:stone");
        player
            .set_slot(PlayerSlot::Hotbar1, Some(&item))
            .expect("stone should fit in the first hotbar slot");

        let retrieved = player
            .get_slot(PlayerSlot::Hotbar1, vec![])
            .expect("get_slot failed")
            .expect("Slot not found");
        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn test_hotbar_selection() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");
        let mut player = world.create_player().expect("test player should attach");

        // Default is slot 1
        assert_eq!(player.selected_hotbar(), 1);

        player
            .select_hotbar(5)
            .expect("valid hotbar slot should be selectable");
        assert_eq!(player.selected_hotbar(), 5);

        assert!(player.select_hotbar(0).is_err());
        assert_eq!(player.selected_hotbar(), 5);

        assert!(player.select_hotbar(10).is_err());
        assert_eq!(player.selected_hotbar(), 5);
    }

    #[test]
    fn unknown_item_does_not_silently_clear_the_slot() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");
        let mut player = world.create_player().expect("test player should attach");

        assert!(
            player
                .set_slot(
                    PlayerSlot::Hotbar1,
                    Some(&Item::new("minecraft:not_a_real_item"))
                )
                .is_err()
        );
    }

    #[test]
    fn invalid_stack_count_is_an_error() {
        init_test_registries();

        assert!(flint_item_to_stack(&Item::with_count("minecraft:stone", 65)).is_err());
    }

    #[test]
    fn empty_air_with_component_data_is_an_error() {
        init_test_registries();
        let mut world = SteelTestWorld::new().expect("test world should initialize");
        let mut player = world.create_player().expect("test player should attach");
        player
            .set_slot(PlayerSlot::Hotbar1, Some(&Item::new("minecraft:stone")))
            .expect("stone should fit in the first hotbar slot");
        let item = Item::with_data_and_count(
            "minecraft:air",
            0,
            [("minecraft:not_a_component".to_string(), "1".to_string())]
                .into_iter()
                .collect(),
        );

        let error = player
            .set_slot(PlayerSlot::Hotbar1, Some(&item))
            .expect_err("component data on an empty item must not be discarded");
        assert!(error.to_string().contains("cannot contain component data"));
        let retained = player
            .get_slot(PlayerSlot::Hotbar1, Vec::new())
            .expect("retained slot should be readable")
            .expect("failed input must not clear the existing stack");
        assert_eq!(retained.id, "minecraft:stone");
    }

    #[test]
    fn oversized_steel_count_is_an_error() {
        init_test_registries();
        let stack = ItemStack::with_count(&vanilla_items::STONE, 256);

        assert!(stack_to_flint_item(&stack, Vec::new()).is_err());
    }

    #[test]
    fn requested_item_component_round_trips_through_steel_codec() {
        init_test_registries();
        let item = Item::with_data_and_count(
            "minecraft:diamond_pickaxe",
            1,
            [("minecraft:damage".to_string(), "7".to_string())]
                .into_iter()
                .collect(),
        );

        let stack = flint_item_to_stack(&item).expect("valid component should decode");
        let retrieved = stack_to_flint_item(&stack, vec!["minecraft:damage".to_string()])
            .expect("valid component should encode")
            .expect("stack should not be empty");

        assert_eq!(
            retrieved.data.get("minecraft:damage"),
            Some(&"7".to_string())
        );
    }

    #[test]
    fn teleport_without_rotation_preserves_existing_rotation() {
        init_test_registries();
        let world = SteelTestWorld::new().expect("test world should initialize");
        let mut player = world
            .create_test_player()
            .expect("test player should attach");
        player.player().set_rotation((42.0, -12.0));

        player
            .teleport([2.0, 70.0, 3.0], None)
            .expect("teleport should succeed");

        assert_eq!(player.player().rotation(), (42.0, -12.0));
    }

    #[test]
    fn cross_chunk_teleport_loads_destination_and_updates_player_chunk_state() {
        init_test_registries();
        let world = SteelTestWorld::new().expect("test world should initialize");
        let mut player = world
            .create_test_player()
            .expect("test player should attach");
        let destination = BlockPos::new(160, 70, 0);
        let destination_chunk = ChunkPos::from_block_pos(destination);
        let stone = flint_block_to_state_id(&Block::new("minecraft:stone"))
            .expect("stone should have a registered state");
        assert!(
            !player
                .player()
                .get_world()
                .set_block(destination, stone, UpdateFlags::UPDATE_ALL),
            "the distant destination should begin unloaded"
        );

        player
            .teleport([160.5, 70.0, 0.5], None)
            .expect("cross-chunk teleport should load and enter its destination");

        assert_eq!(player.player().position(), DVec3::new(160.5, 70.0, 0.5));
        assert_eq!(*player.player().last_chunk_pos.lock(), destination_chunk);
        assert!(
            player
                .player()
                .get_world()
                .chunk_map
                .tickable_full_chunk_positions()
                .contains(&destination_chunk),
            "teleport must make the destination entity-ticking"
        );
        for x_offset in -2..=2 {
            for z_offset in -2..=2 {
                let halo = ChunkPos::new(
                    destination_chunk.0.x + x_offset,
                    destination_chunk.0.y + z_offset,
                );
                assert!(
                    player
                        .player()
                        .get_world()
                        .chunk_map
                        .with_full_chunk(halo, |_| ())
                        .is_some(),
                    "teleport destination halo is missing full chunk {halo:?}"
                );
            }
        }
        assert!(
            player
                .player()
                .get_world()
                .set_block(destination, stone, UpdateFlags::UPDATE_ALL),
            "teleport must make the destination chunk available to gameplay"
        );
    }
}
