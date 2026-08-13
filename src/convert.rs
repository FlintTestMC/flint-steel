//! Conversion utilities between Flint types and `SteelMC` types.

use anyhow::{Context, Result, anyhow, bail};
use flint_core::Block;
use rustc_hash::FxHashMap;
use steel_core::behavior::BlockHitResult;
use steel_core::world::ClipHitResult;
use steel_registry::{REGISTRY, RegistryExt};
use steel_utils::{BlockPos as SteelBlockPos, BlockStateId, Identifier};

/// Preserve the full registry key when crossing into Flint's string IDs.
pub(crate) fn registry_key_to_flint_id(key: &Identifier) -> String {
    key.to_string()
}

/// Convert a Flint block specification to a `SteelMC` `BlockStateId`.
///
/// Returns an error if the block ID is unknown or any property is invalid.
pub fn flint_block_to_state_id(block: &Block) -> Result<BlockStateId> {
    let identifier = block
        .id
        .parse::<Identifier>()
        .map_err(|error| anyhow!("invalid block identifier `{}`: {error}", block.id))?;

    // Properties are already String values in the new Block type
    let properties: Vec<(&str, &str)> = block
        .properties
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    // If no properties specified, return the block's default state
    if properties.is_empty() {
        let block_ref = REGISTRY
            .blocks
            .by_key(&identifier)
            .with_context(|| format!("unknown block `{identifier}`"))?;
        return Ok(REGISTRY.blocks.get_default_state_id(block_ref));
    }

    REGISTRY
        .blocks
        .state_id_from_properties(&identifier, &properties)
        .with_context(|| {
            let properties = properties
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("unknown block `{identifier}` or invalid properties [{properties}]")
        })
}

/// Convert a `SteelMC` `BlockStateId` to Flint `Block`.
pub fn state_id_to_block(state_id: BlockStateId) -> Result<Block> {
    let Some(block) = REGISTRY.blocks.by_state_id(state_id) else {
        bail!("unknown Steel block state ID {}", state_id.0);
    };

    let id = registry_key_to_flint_id(&block.key);

    // Get properties from the registry
    let props = REGISTRY.blocks.get_properties(state_id);
    #[allow(clippy::disallowed_types)]
    let properties: FxHashMap<String, String> = props
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    Ok(Block::with_properties(id, properties))
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

        let state_id = flint_block_to_state_id(&block).expect("stone should have a state ID");

        let retrieved = state_id_to_block(state_id).expect("state ID should resolve");
        assert_eq!(retrieved.id, "minecraft:stone");
    }

    #[test]
    fn test_air_block() {
        init_test_registries();
        let block = Block::new("minecraft:air");

        assert!(flint_block_to_state_id(&block).is_ok());
    }

    #[test]
    fn cave_air_is_not_reported_as_air() {
        init_test_registries();
        let cave_air = flint_block_to_state_id(&Block::new("minecraft:cave_air"))
            .expect("cave air should have a registered state");

        let retrieved = state_id_to_block(cave_air).expect("cave air state should resolve");
        assert_eq!(retrieved.id, "minecraft:cave_air");
        assert_ne!(retrieved.id, "minecraft:air");
    }

    #[test]
    fn test_block_without_prefix() {
        init_test_registries();
        let block = Block::new("stone");

        assert!(flint_block_to_state_id(&block).is_ok());
    }

    #[test]
    fn unknown_block_is_an_error() {
        init_test_registries();
        let error = flint_block_to_state_id(&Block::new("minecraft:not_a_real_block"))
            .expect_err("unknown block must not become air");

        assert!(error.to_string().contains("unknown block"));
    }

    #[test]
    fn invalid_block_property_is_an_error() {
        init_test_registries();
        let block = Block::with_properties(
            "minecraft:oak_log",
            [("axis".to_string(), "sideways".to_string())]
                .into_iter()
                .collect(),
        );

        assert!(flint_block_to_state_id(&block).is_err());
    }

    #[test]
    fn unknown_state_id_is_an_error() {
        init_test_registries();

        assert!(state_id_to_block(BlockStateId(u16::MAX)).is_err());
    }

    #[test]
    fn registry_identifier_preserves_non_vanilla_namespace() {
        let identifier = Identifier::new_static("example_mod", "custom_block");

        assert_eq!(
            registry_key_to_flint_id(&identifier),
            "example_mod:custom_block"
        );
    }
}
