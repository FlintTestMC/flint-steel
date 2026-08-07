//! Conversion utilities between Flint types and `SteelMC` types.

use flint_core::Block;
use rustc_hash::FxHashMap;
use steel_core::behavior::BlockHitResult;
use steel_core::world::ClipHitResult;
use steel_registry::{REGISTRY, RegistryExt};
use steel_utils::{BlockPos as SteelBlockPos, BlockStateId, Identifier};

/// Convert a Flint block specification to a `SteelMC` `BlockStateId`.
///
/// Returns `None` if the block ID is unknown or if any property is invalid.
pub fn flint_block_to_state_id(block: &Block) -> Option<BlockStateId> {
    // Parse the block ID - may have "minecraft:" prefix
    let block_id = if block.id.starts_with("minecraft:") {
        &block.id[10..]
    } else {
        &block.id
    };

    let identifier = Identifier::vanilla(block_id.to_string());

    // Properties are already String values in the new Block type
    let properties: Vec<(&str, &str)> = block
        .properties
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    // If no properties specified, return the block's default state
    if properties.is_empty() {
        let block_ref = REGISTRY.blocks.by_key(&identifier)?;
        return Some(REGISTRY.blocks.get_default_state_id(block_ref));
    }

    REGISTRY
        .blocks
        .state_id_from_properties(&identifier, &properties)
}

/// Convert a `SteelMC` `BlockStateId` to Flint `Block`.
pub fn state_id_to_block(state_id: BlockStateId) -> Block {
    let Some(block) = REGISTRY.blocks.by_state_id(state_id) else {
        return Block::new("minecraft:air");
    };

    let mut id = format!("minecraft:{}", block.key.path);

    if block.key.path == "void_air" || block.key.path == "cave_air" {
        id = "minecraft:air".to_owned();
    }

    // Get properties from the registry
    let props = REGISTRY.blocks.get_properties(state_id);
    #[allow(clippy::disallowed_types)]
    let properties: FxHashMap<String, String> = props
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    Block::with_properties(id, properties)
}

/// Convert Flint `BlockPos` to `SteelMC` `BlockPos`.
pub const fn flint_pos_to_steel(pos: flint_core::BlockPos) -> SteelBlockPos {
    SteelBlockPos::new(pos[0], pos[1], pos[2])
}

/// Convert a `SteelMC` [`ClipHitResult`] to a [`BlockHitResult`].
///
/// The two types carry identical data; `World::clip` returns the former while
/// the interaction API (`use_item_on`) takes the latter.
pub const fn clip_to_block_hit(clip: ClipHitResult) -> BlockHitResult {
    BlockHitResult {
        location: clip.location,
        direction: clip.direction,
        block_pos: clip.block_pos,
        miss: clip.miss,
        inside: clip.inside,
        world_border_hit: clip.world_border_hit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_test_registries;

    #[test]
    fn test_simple_block_conversion() {
        init_test_registries();
        let block = Block::new("minecraft:stone");

        let state_id = flint_block_to_state_id(&block);
        assert!(state_id.is_some(), "Stone should convert to valid state ID");

        let retrieved = state_id_to_block(state_id.expect("Valid state ID"));
        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn test_air_block() {
        init_test_registries();
        let block = Block::new("minecraft:air");

        let state_id = flint_block_to_state_id(&block);
        assert!(state_id.is_some(), "Air should convert to valid state ID");
    }

    #[test]
    fn test_block_without_prefix() {
        init_test_registries();
        let block = Block::new("stone");

        let state_id = flint_block_to_state_id(&block);
        assert!(state_id.is_some(), "Block without prefix should still work");
    }
}
