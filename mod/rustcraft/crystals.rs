// Slice 18: the crystal shard, the druses and growing crystals (guide `crystal`, `shard`, `grow`;
// spec E5, CR2). rustcraft_rules::grow decides how a seed grows; this module declares the blocks
// and the item, and turns a shard clicked onto the top of a tank of nutrient solution into a seed.
//
// - The shard `rustcraft:crystal_shard`: the ingredient of the crystal, the solar panel and the
//   battery, and of most recipes of the guide.
// - The druse `rustcraft:druse`: a stone block with blue crystals growing out of it (model `geode`).
//   It lies on cave floors underground: the content mod's `worldgen/feature/druse.json`, its
//   placed feature and a NeoForge biome modifier adding it to every overworld biome
//   (SRC/docs/mods.md:458-485). Its loot table gives two to four shards (itself with silk touch).
// - The seed `rustcraft:seed`, stage 0..2 in its state: a shard standing on a tank. It is a sink of
//   rustcraft_power (the fluid tick fills it from the tank under it, as a crystal structure is
//   filled), and grows from what it holds (rustcraft_power::grow_seeds). Its loot table gives two
//   to four shards at stage 2 and its one shard before.
use crate::game::{Game, rustcraft_content as content, rustcraft_server as server};
use horizon_mods::content::{Block, BlockProperties, BlockShapes, Item, RenderShape, StateProperty};
use horizon_world::nbt::Compound;
use rustcraft_rules::{
    geom::Pos,
    grow::{self, DRUSE, SEED, SHARD, STAGE, STAGES},
};
use std::collections::BTreeMap;

/// The seed's outline at each stage: the small crystal, the half-grown druse, the grown one.
const SEED_BOXES: [[f64; 6]; 3] = [
    [0.3125, 0.0, 0.3125, 0.6875, 0.5, 0.6875],
    [0.1875, 0.0, 0.1875, 0.8125, 0.75, 0.8125],
    [0.0625, 0.0, 0.0625, 0.9375, 0.9375, 0.9375],
];

/// `native_blocks`: the druse and the seed (rustcraft_content declares the rest).
pub(super) fn declare_blocks(blocks: &mut Vec<Block>) {
    blocks.push(Block {
        id: DRUSE.into(),
        template: "minecraft:amethyst_block".into(),
        properties: BlockProperties {
            hardness: Some(1.5),
            explosion_resistance: Some(1.5),
            light: Some(4),
            sounds: Some("minecraft:amethyst_cluster".into()),
            occludes: Some(false),
            ..BlockProperties::default()
        },
        item: true,
        ..Block::default()
    });
    let seed_shape = || BlockShapes {
        property: STAGE.to_owned(),
        properties: Vec::new(),
        values: STAGES
            .iter()
            .zip(SEED_BOXES)
            .map(|(s, b)| ((*s).to_owned(), vec![b]))
            .collect(),
        default: None,
    };
    blocks.push(Block {
        id: SEED.into(),
        template: "minecraft:amethyst_block".into(),
        properties: BlockProperties {
            hardness: Some(0.5),
            explosion_resistance: Some(0.5),
            light: Some(3),
            sounds: Some("minecraft:amethyst_cluster".into()),
            occludes: Some(false),
            render_shape: Some(RenderShape::Model),
            ..BlockProperties::default()
        },
        shapes: Some(seed_shape()),
        collision_shapes: Some(seed_shape()),
        state_properties: BTreeMap::from([(
            STAGE.to_owned(),
            StateProperty::Enum {
                values: STAGES.map(str::to_owned).to_vec(),
            },
        )]),
        default_state: BTreeMap::from([(STAGE.to_owned(), STAGES[0].to_owned())]),
        item: false,
        ..Block::default()
    });
}

/// `native_items`: the crystal shard.
pub(super) fn declare_items(items: &mut Vec<Item>) {
    items.push(Item {
        id: SHARD.into(),
        template: "minecraft:amethyst_shard".into(),
        ..Item::default()
    });
}

/// `native_block_entity`: a seed keeps its growth and solution in one, and the server's rescan
/// finds it by it.
pub(super) fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_none() && block == SEED {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

/// A right click with a shard on the top face of a tank holding nutrient solution: the shard
/// stands there as a seed (CR2), if the cell above is free.
pub(super) fn on_right_click_block(
    game: &mut Game<'_>,
    pos: Pos,
    face: [i32; 3],
    canceled: &mut bool,
) {
    if *canceled || game.native_block(pos) != content::TANK {
        return;
    }
    let Some((held, _, _)) = game.native_held() else {
        return;
    };
    if held != SHARD {
        return;
    }
    let Some(player) = crate::game::rustcraft_server::acting(game) else {
        return;
    };
    let fluid = server::with_level(&player.dimension, |level| {
        crate::game::rustcraft_fluids::group_at(game, level, pos, server::tuning())
            .and_then(|g| g.fluid.map(|f| f.to_string()))
    })
    .flatten();
    if !grow::takes_seed(content::TANK, fluid.as_deref(), face == [0, 1, 0]) {
        return;
    }
    let above = [pos[0], pos[1] + 1, pos[2]];
    if !rustcraft_rules::mining::free(&game.native_block(above)) {
        return;
    }
    *canceled = true;
    if !place_seed(game, &player.dimension, above) {
        return;
    }
    game.native_take_items(SHARD, 1);
}

/// Sets a seed of stage 0 at `pos`; whether it stands there.
pub(super) fn place_seed(game: &mut Game<'_>, dimension: &str, pos: Pos) -> bool {
    let Some(state) = game.native_default_state(SEED) else {
        return false;
    };
    game.native_set_block(dimension, pos, state, 3);
    server::mark_level_stale(dimension);
    game.native_block(pos) == SEED
}

/// Shows stage `stage` on the seed at `pos`, if it shows another.
pub(super) fn set_stage(game: &mut Game<'_>, pos: Pos, stage: u8) {
    let want = STAGES[usize::from(stage.min(2))];
    if game.native_block_property(pos, STAGE).as_deref() != Some(want) {
        game.native_set_block_property(pos, STAGE, want);
    }
}
