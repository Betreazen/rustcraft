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

// ---- Druses in chunks older than 0.18 (rustcraft_rules::druses) ----
//
// Worldgen places druses only in chunks generated with the mod's content. `native_chunk_load`
// (API 57) says whether a loaded chunk is new: a new one had worldgen and is only noted; an old
// overworld chunk that never had the pass gets it a few ticks later, from the level's tick (the
// hook says to change the world from a later tick: its light and entities may not be ready). The
// chunks done are kept in the world's `data/rustcraft_druse_chunks.json`.

const DONE_FILE: &str = "rustcraft_druse_chunks.json";
/// Only the overworld has druses (the biome modifier's `#minecraft:is_overworld`).
const OVERWORLD: &str = "minecraft:overworld";
/// The overworld's bottom (vanilla 26.3).
const OVERWORLD_MIN_Y: i32 = -64;
/// Old chunks given their pass in one tick, so a player flying over old land costs little.
const PER_TICK: usize = 2;
/// How often the done chunks are written when they changed, ticks.
const SAVE_EVERY: u64 = 200;

#[derive(Default)]
struct Retro {
    /// Chunks loaded and not yet looked at: the level, the chunk, whether it is new.
    queue: std::collections::VecDeque<(String, [i32; 2], bool)>,
    done: Option<rustcraft_rules::druses::Done>,
    dirty: bool,
    ticks: u64,
}

fn retro() -> std::sync::MutexGuard<'static, Retro> {
    static R: std::sync::LazyLock<std::sync::Mutex<Retro>> = std::sync::LazyLock::new(Default::default);
    R.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `native_chunk_load`: an overworld chunk came; looked at from the level's next ticks.
pub(super) fn on_chunk_load(dimension: &str, chunk: [i32; 2], new: bool) {
    if dimension == OVERWORLD {
        retro().queue.push_back((dimension.to_owned(), chunk, new));
    }
}

fn done_path(game: &Game<'_>) -> Option<std::path::PathBuf> {
    game.native_world_dir().map(|w| w.join("data").join(DONE_FILE))
}

fn load_done(game: &Game<'_>) -> rustcraft_rules::druses::Done {
    done_path(game)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_done(game: &Game<'_>, done: &rustcraft_rules::druses::Done) {
    let Some(path) = done_path(game) else { return };
    let written = serde_json::to_string(done)
        .map_err(std::io::Error::other)
        .and_then(|json| {
            let fresh = path.with_extension("json.tmp");
            path.parent().map_or(Ok(()), std::fs::create_dir_all)?;
            std::fs::write(&fresh, json)?;
            std::fs::rename(&fresh, &path)
        });
    if let Err(e) = written {
        log::warn!("rustcraft: the druse chunks' file was not saved: {e}");
    }
}

/// The level's tick: notes new chunks, gives old ones their pass, a few a tick, and saves.
pub(super) fn retro_tick(game: &mut Game<'_>, dimension: &str) {
    if dimension != OVERWORLD {
        return;
    }
    let mut r = retro();
    if r.done.is_none() {
        r.done = Some(load_done(game));
    }
    r.ticks += 1;
    let mut work = Vec::new();
    while work.len() < PER_TICK {
        let Some((level, chunk, new)) = r.queue.pop_front() else { break };
        let done = r.done.get_or_insert_with(Default::default);
        if done.has(&level, chunk) {
            continue;
        }
        if new {
            done.mark(&level, chunk);
            r.dirty = true;
        } else {
            work.push(chunk);
        }
    }
    let save = r.dirty && r.ticks.is_multiple_of(SAVE_EVERY);
    drop(r);
    for chunk in work {
        // A chunk that left again before its turn is looked at when it next loads.
        if !game.world.chunks.contains_key(&(chunk[0], chunk[1])) {
            continue;
        }
        let placed = retro_chunk(game, dimension, chunk, rustcraft_rules::druses::TRIES);
        if placed > 0 {
            log::debug!("rustcraft: {placed} druses in old chunk {chunk:?}");
        }
        let mut r = retro();
        r.done.get_or_insert_with(Default::default).mark(dimension, chunk);
        r.dirty = true;
    }
    if save {
        let done = retro().done.clone().unwrap_or_default();
        save_done(game, &done);
        retro().dirty = false;
    }
}

/// Gives chunk `chunk` its druses: `tries` tries as worldgen makes them, on natural floors only.
/// How many it placed.
pub(super) fn retro_chunk(game: &mut Game<'_>, dimension: &str, chunk: [i32; 2], tries: u32) -> u32 {
    use rustcraft_rules::druses::{cell_of, place_from, starts};
    let Some(state) = game.native_default_state(DRUSE) else {
        return 0;
    };
    let mut placed = 0;
    for start in starts(chunk, OVERWORLD_MIN_Y, tries) {
        let at = place_from(start, |p| cell_of(&game.native_block(p)));
        if let Some(at) = at {
            game.native_set_block(dimension, at, state, 2);
            placed += u32::from(game.native_block(at) == DRUSE);
        }
    }
    placed
}

/// The self-test: chunk `chunk` given its pass now with `tries` tries, unless it had one; how
/// many druses it placed.
pub(super) fn retro_for_selftest(game: &mut Game<'_>, dimension: &str, chunk: [i32; 2], tries: u32, fresh: bool) -> u32 {
    if retro_done(game, |d| {
        if fresh {
            d.forget(dimension, chunk);
        }
        d.has(dimension, chunk)
    }) {
        return 0;
    }
    let placed = retro_chunk(game, dimension, chunk, tries);
    retro_done(game, |d| d.mark(dimension, chunk));
    placed
}

/// Runs `f` on the done chunks, read from the world's file the first time.
fn retro_done<R>(game: &Game<'_>, f: impl FnOnce(&mut rustcraft_rules::druses::Done) -> R) -> R {
    let mut r = retro();
    if r.done.is_none() {
        r.done = Some(load_done(game));
    }
    f(r.done.get_or_insert_with(Default::default))
}
