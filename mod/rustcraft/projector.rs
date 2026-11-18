// Slice 18 (stream 5): the holo-projector and the building machine as the guide says them
// (`projector`, `builder`; rustcraft_rules::utility::plan decides).
//
// The projector:
// - The wrench on the projector arms it for that player; the next two wrench clicks on blocks are
//   the corners of its area, at most 32 × 32 × 32 (without corners it reads 9 × 9 and 8 up).
//   «Запомнить» reads every block of the area with its state into the block entity, packed
//   (`plan_box`, `plan_palette`, `plan_runs`).
// - Its window turns the plan a quarter, mirrors it, shows or hides the ghost, forgets it, and
//   «В руку» gives the player a stack of the first missing block from the storage on the
//   projector's magistral. Its rows name what is missing and how much of it the storage holds.
// - While the ghost shows it spends 4 E/s; without energy the ghost goes out (`ghost_lit`).
// - The sneaking wrench lifts it into an item that keeps its plan (custom data
//   `rustcraft:plan`); placed again, the plan is back.
// - Clients draw the ghost from the update tag: each missing cell's block, smaller, where the
//   world has air, and red glass where another block stands in a cell of the plan.
//
// The building machine: beside a projector, two blocks a second into the plan's next missing
// cells bottom up, with their states, taken from the storage on its magistral (or its own slots);
// a cell with another block is skipped and named. Builder's experience once a cell (BU1).
use crate::game::{
    Game, native_block_entity_draw::BlockEntityDraw, rustcraft_base as base,
    rustcraft_server::{LevelNet, line, tell},
    rustcraft_storage as storage,
    rustcraft_utility::{self as util, CUSTOM_DATA, items, read_i64, read_json, spend, write_i64, write_json},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack, text::Text};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    reports::Report,
    utility::{
        Shown, Util,
        plan::{self, Area, Built, Cell, Packed, Plan},
    },
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const BOX_KEY: &str = "plan_box";
const PALETTE_KEY: &str = "plan_palette";
const RUNS_KEY: &str = "plan_runs";
const TURN_KEY: &str = "plan_turn";
const MIRROR_KEY: &str = "plan_mirror";
const AREA_KEY: &str = "plan_area";
const OLD_PLAN_KEY: &str = "plan";
const GHOST_KEY: &str = "ghost";
const LIT_KEY: &str = "ghost_lit";
const BUILT_KEY: &str = "built";
/// The custom data mark of a lifted projector's item.
const ITEM_MARK: &str = "rustcraft:plan";
const PROJECTOR: &str = "rustcraft:projector";
/// The keys clients draw the ghost from.
const SYNCED: [&str; 7] = [BOX_KEY, PALETTE_KEY, RUNS_KEY, TURN_KEY, MIRROR_KEY, GHOST_KEY, LIT_KEY];
/// The ghost's blocks are drawn this big, about their cell's middle.
const GHOST_SCALE: f32 = 0.6;
/// The most ghost blocks a projector draws in a frame.
const GHOST_MAX: usize = 512;
/// The window's toggles.
pub(super) const TOGGLES: [&str; 6] = ["remember", "ghost", "clear", "turn", "mirror", "hand"];

#[derive(Default)]
struct State {
    /// A player's armed projector and the corners clicked so far.
    arming: HashMap<u128, (String, Pos, Vec<Pos>)>,
    /// What each building machine skipped and lacked on its last step: (skipped cells, lacking).
    builder: HashMap<Pos, (usize, Vec<(String, u32)>)>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The client's unpacked plans, by projector and its packed form's size, so a frame does not
/// unpack 32 000 cells.
static PLANS: LazyLock<Mutex<HashMap<Pos, (usize, Plan)>>> = LazyLock::new(Mutex::default);

// ---- The plan in NBT ----

fn ints(nbt: &Compound, key: &str) -> Vec<i32> {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::IntArray(v)) => v.clone(),
        _ => Vec::new(),
    }
}

fn read_packed(nbt: &Compound) -> Option<Packed> {
    let b = ints(nbt, BOX_KEY);
    if b.len() != 6 {
        return None;
    }
    let palette = match nbt.get(&JavaString::from(PALETTE_KEY)) {
        Some(Tag::List(l)) => l
            .iter()
            .filter_map(|t| match t {
                Tag::String(s) => Some(s.to_string_lossy()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(Packed { min: [b[0], b[1], b[2]], size: [b[3], b[4], b[5]], palette, runs: ints(nbt, RUNS_KEY) })
}

fn write_packed(nbt: &mut Compound, p: &Packed) {
    let b = vec![p.min[0], p.min[1], p.min[2], p.size[0], p.size[1], p.size[2]];
    nbt.insert(JavaString::from(BOX_KEY), Tag::IntArray(b));
    let palette = p.palette.iter().map(|s| Tag::String(JavaString::from(s.as_str()))).collect();
    nbt.insert(JavaString::from(PALETTE_KEY), Tag::List(palette));
    nbt.insert(JavaString::from(RUNS_KEY), Tag::IntArray(p.runs.clone()));
    nbt.remove(&JavaString::from(OLD_PLAN_KEY));
}

/// The plan as remembered (before its turn), the slice-12 JSON plan of an older world too.
fn source_plan(nbt: &Compound) -> Plan {
    match read_packed(nbt) {
        Some(p) => Plan::unpack(&p),
        None => read_json(nbt, OLD_PLAN_KEY),
    }
}

fn turn_of(nbt: &Compound) -> (u8, bool) {
    ((read_i64(nbt, TURN_KEY).rem_euclid(4)) as u8, read_i64(nbt, MIRROR_KEY) != 0)
}

/// The plan as it stands now: remembered, turned and mirrored.
pub(super) fn plan_of(nbt: &Compound) -> Plan {
    let (q, m) = turn_of(nbt);
    let p = source_plan(nbt);
    if q == 0 && !m { p } else { p.transformed(q, m) }
}

/// `native_block_entity_update_tag`: clients get the plan and the ghost's switches.
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id != PROJECTOR {
        return;
    }
    for key in SYNCED {
        if let Some(v) = entity.nbt.get(&JavaString::from(key)) {
            tag.get_or_insert_with(Compound::new).insert(JavaString::from(key), v.clone());
        }
    }
}

// ---- The world ----

fn offset(pos: Pos, o: [i32; 3]) -> Pos {
    [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]]
}

fn empty(b: &str) -> bool {
    matches!(b, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "minecraft:water" | "minecraft:lava")
}

/// The block id at `at`; none for air and fluids.
fn block_at(game: &Game<'_>, at: Pos) -> Option<String> {
    let b = game.native_block(at);
    (!empty(&b)).then_some(b)
}

/// The block's state at `at` with the properties a plan keeps.
fn state_at(game: &Game<'_>, at: Pos) -> Option<String> {
    let id = block_at(game, at)?;
    let props: Vec<(String, String)> = plan::PROPERTIES
        .iter()
        .filter_map(|k| game.native_block_property(at, k).map(|v| ((*k).to_owned(), v)))
        .collect();
    Some(plan::state_string(&id, &props))
}

/// A plan's state string into the world at `at`: its block's default state, then each kept
/// property (`native_set_block_property`; `native_state_with_property` is a level hook's, not
/// the game's). False when the block is unknown or the world refuses it.
fn place_state(game: &mut Game<'_>, dimension: &str, at: Pos, state: &str) -> bool {
    let Some(s) = game.native_default_state(plan::state_id(state)) else {
        return false;
    };
    if !game.native_set_block(dimension, at, s, 3) {
        return false;
    }
    for (k, v) in plan::state_props(state) {
        game.native_set_block_property(at, &k, &v);
    }
    true
}

// ---- The projector ----

/// Each tick a projector steps: 4 E/s while its ghost shows; without it the ghost goes out. Wave 2
/// (guide `projector` diagnoses): a ghost with no storage on the projector's magistral says
/// «Нет склада на магистрали» (it still shows: the blocks can come by hand).
pub(super) fn projector_step(game: &mut Game<'_>, dimension: &str, level: &crate::game::rustcraft_server::LevelNet, pos: Pos, cost: f64) -> Shown {
    let Some(e) = game.native_mod_block_entity(pos) else {
        return Shown::Idle("idle");
    };
    let empty_plan = read_packed(&e.nbt).is_none_or(|p| p.runs.is_empty()) && source_plan(&e.nbt).cells.is_empty();
    let on = read_i64(&e.nbt, GHOST_KEY) != 0;
    let lit = read_i64(&e.nbt, LIT_KEY) != 0;
    drop(e);
    if empty_plan {
        return Shown::Idle("projector_empty");
    }
    if !on {
        set_lit(game, pos, lit, false);
        return Shown::Idle("projector_hidden");
    }
    let powered = spend(game, pos, cost);
    set_lit(game, pos, lit, powered);
    if !powered {
        return Shown::NoEnergy;
    }
    if storage::storage_by(dimension, pos, &level.topology).is_none() {
        return Shown::Idle("projector_no_storage");
    }
    Shown::Working
}

fn set_lit(game: &mut Game<'_>, pos: Pos, was: bool, now: bool) {
    if was != now {
        game.native_with_mod_block_entity(pos, |e| write_i64(&mut e.nbt, LIT_KEY, i64::from(now)));
        game.native_block_entity_sync(pos);
    }
}

/// A press in its window: remember, ghost, forget, turn, mirror, «В руку».
pub(super) fn projector_press(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, i: usize) {
    match TOGGLES.get(i).copied() {
        Some("remember") => {
            let a = ints(&entity.nbt, AREA_KEY);
            let area = if a.len() == 6 { Area { min: [a[0], a[1], a[2]], max: [a[3], a[4], a[5]] } } else { Area::default() };
            let p = Plan::read_area(area, |o| {
                (o != [0, 0, 0]).then(|| state_at(game, offset(pos, o))).flatten()
                    .filter(|b| !crate::game::rustcraft_server::is_ours(plan::state_id(b)))
            });
            write_packed(&mut entity.nbt, &p.pack());
            write_i64(&mut entity.nbt, TURN_KEY, 0);
            write_i64(&mut entity.nbt, MIRROR_KEY, 0);
            write_i64(&mut entity.nbt, GHOST_KEY, 1);
        }
        Some("ghost") => {
            let on = read_i64(&entity.nbt, GHOST_KEY) != 0;
            write_i64(&mut entity.nbt, GHOST_KEY, i64::from(!on));
        }
        Some("clear") => {
            write_packed(&mut entity.nbt, &Packed::default());
            write_i64(&mut entity.nbt, GHOST_KEY, 0);
        }
        Some("turn") => {
            let q = read_i64(&entity.nbt, TURN_KEY);
            write_i64(&mut entity.nbt, TURN_KEY, (q + 1).rem_euclid(4));
        }
        Some("mirror") => {
            let m = read_i64(&entity.nbt, MIRROR_KEY) != 0;
            write_i64(&mut entity.nbt, MIRROR_KEY, i64::from(!m));
        }
        Some("hand") => to_hand(game, pos, entity),
        _ => {}
    }
    game.native_block_entity_sync(pos);
}

/// The storage on the magistral of the block at `pos`, in the level `dimension` (none while
/// the level is not at hand).
fn storage_of(dimension: &str, pos: Pos) -> Option<u32> {
    crate::game::rustcraft_server::with_level(dimension, |l| storage::storage_by(dimension, pos, &l.topology)).flatten()
}

/// «В руку»: a stack of the first missing block from the storage on the projector's magistral
/// into the pressing player's inventory.
fn to_hand(game: &mut Game<'_>, pos: Pos, entity: &ModBlockEntity) {
    let Some(player) = crate::game::rustcraft_server::acting(game) else { return };
    let Some(id) = storage_of(&player.dimension, pos) else {
        tell(game, player.uuid, line("rustcraft.util.projector_no_storage", "No storage on the magistral", &[]));
        return;
    };
    let plan = plan_of(&entity.nbt);
    let missing = Plan::counts(plan.missing(|o| block_at(game, offset(pos, o))));
    let Some(item) = missing
        .keys()
        .find(|k| storage::stored_count(&player.dimension, id, k) > 0)
        .cloned()
    else {
        tell(game, player.uuid, line("rustcraft.util.projector_none_stored", "The storage holds none of the missing blocks", &[]));
        return;
    };
    let max = crate::game::rustcraft_machines::max_stack(game, &item);
    let want = missing.get(&item).copied().unwrap_or(1).min(max);
    storage::give_from(game, &player.dimension, id, &item, want, player.uuid);
}

/// Its window's rows: what it remembers and what is missing, with the storage's counts.
/// Wave 2: the storage comes from `level` (the window's tick has the level out of the server's
/// map, and the self-test's bench holds that map while it reads a window: asking the map again
/// found no storage, or stood still for good).
pub(super) fn projector_rows(game: &Game<'_>, dimension: &str, level: &crate::game::rustcraft_server::LevelNet, pos: Pos) -> Vec<Text> {
    let Some(e) = game.native_mod_block_entity(pos) else { return Vec::new() };
    let plan = plan_of(&e.nbt);
    let (q, m) = turn_of(&e.nbt);
    drop(e);
    if plan.cells.is_empty() {
        return vec![line("rustcraft.util.projector_empty", "No saved building", &[]), Text::literal("")];
    }
    let first = line(
        "rustcraft.util.projector.cells2",
        "Remembers %s blocks · turned %s° %s",
        &[
            Text::literal(plan.cells.len().to_string()),
            Text::literal((u32::from(q) * 90).to_string()),
            if m { line("rustcraft.util.projector.mirrored", "mirrored", &[]) } else { Text::literal("") },
        ],
    );
    let missing = Plan::counts(plan.missing(|o| block_at(game, offset(pos, o))));
    let id = storage::storage_by(dimension, pos, &level.topology);
    let parts: Vec<String> = missing
        .iter()
        .take(3)
        .map(|(k, n)| {
            let name = k.rsplit(':').next().unwrap_or(k);
            match id {
                Some(id) => format!("{name} ×{n} ({})", storage::stored_count(dimension, id, k)),
                None => format!("{name} ×{n}"),
            }
        })
        .collect();
    let second = if missing.is_empty() {
        line("rustcraft.util.projector.complete", "Nothing missing", &[])
    } else {
        line("rustcraft.util.projector.missing", "Missing (in storage): %s", &[Text::literal(parts.join(", "))])
    };
    vec![first, second]
}

// ---- The wrench: corners, lifting, placing ----

/// `native_use_item_on` with the wrench: on a projector, arm it (or, sneaking, lift it with its
/// plan); while armed, a block is a corner. True when it took the click.
pub(super) fn on_wrench(game: &mut Game<'_>, player: u128, dimension: &str, sneaking: bool, pos: Pos) -> bool {
    let on_projector = game.native_block(pos) == PROJECTOR;
    if on_projector && sneaking {
        lift(game, player, dimension, pos);
        return true;
    }
    if on_projector {
        state().arming.insert(player, (dimension.to_owned(), pos, Vec::new()));
        tell(game, player, line("rustcraft.util.projector.arm", "Click two corners of the area with the wrench (at most 32 a side)", &[]));
        return true;
    }
    let Some((dim, projector, mut corners)) = state().arming.remove(&player) else {
        return false;
    };
    if dim != dimension {
        return false;
    }
    corners.push(pos);
    if corners.len() < 2 {
        state().arming.insert(player, (dim, projector, corners));
        tell(game, player, line("rustcraft.util.projector.corner1", "First corner set: now the second", &[]));
        return true;
    }
    match Area::of(corners[0], corners[1], projector) {
        Some(a) => {
            let flat = vec![a.min[0], a.min[1], a.min[2], a.max[0], a.max[1], a.max[2]];
            game.native_with_mod_block_entity(projector, |e| {
                e.nbt.insert(JavaString::from(AREA_KEY), Tag::IntArray(flat));
            });
            let s = a.size();
            let size = format!("{}×{}×{}", s[0], s[1], s[2]);
            tell(game, player, line("rustcraft.util.projector.area", "Area %s set: press «Remember»", &[Text::literal(size)]));
        }
        None => tell(game, player, line("rustcraft.util.projector.too_big", "The area is bigger than 32 a side", &[])),
    }
    true
}

/// The sneaking wrench on a projector: it comes out as an item keeping its plan (wave 2: the
/// wrench keeps the projector, rustcraft_rules::net::wrench::KEPT, so this lift is its own).
fn lift(game: &mut Game<'_>, player: u128, dimension: &str, pos: Pos) {
    let Some(e) = game.native_mod_block_entity(pos) else { return };
    let mut mark = Compound::new();
    for key in [BOX_KEY, PALETTE_KEY, RUNS_KEY, TURN_KEY, MIRROR_KEY, AREA_KEY] {
        if let Some(v) = e.nbt.get(&JavaString::from(key)) {
            mark.insert(JavaString::from(key), v.clone());
        }
    }
    drop(e);
    let Some(air) = game.native_default_state("minecraft:air") else { return };
    if !game.native_set_block(dimension, pos, air, 3) {
        return;
    }
    let mut stack = ItemStack::new(PROJECTOR, 1);
    if !mark.is_empty() {
        let mut data = Compound::new();
        data.insert(JavaString::from(ITEM_MARK), Tag::Compound(mark));
        stack.data_mut().set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
    }
    if let Some(rest) = game.native_give_stack(player, stack) {
        let nbt = Compound::from([(JavaString::from("Item"), horizon_client::save::item_to_nbt(&rest))]);
        let at = pos.map(|c| f64::from(c) + 0.5);
        if game.native_summon(dimension, "minecraft:item", at, nbt).is_none() {
            crate::game::rustcraft_debug::journal_warn!("rustcraft: the lifted projector at {pos:?} was lost");
        }
    }
}

/// `native_place_block` of a projector item that keeps a plan: the plan into the new block.
pub(super) fn on_place(game: &mut Game<'_>, stack: &ItemStack, pos: Pos) {
    if &*stack.item != PROJECTOR {
        return;
    }
    let Some(Tag::Compound(mark)) = stack.custom_data().and_then(|d| d.get(&JavaString::from(ITEM_MARK))).cloned() else {
        return;
    };
    game.native_with_mod_block_entity(pos, |e| {
        for (k, v) in mark {
            e.nbt.insert(k, v);
        }
        write_i64(&mut e.nbt, GHOST_KEY, 1);
    });
    game.native_block_entity_sync(pos);
}

// ---- The ghost on a client ----

/// The plan of a projector as a client draws it, unpacked once for each packed form.
fn client_plan(pos: Pos, nbt: &Compound) -> Plan {
    let p = read_packed(nbt).unwrap_or_default();
    let (q, m) = turn_of(nbt);
    let stamp = p.runs.len() * 31 + p.palette.len() * 7 + usize::from(q) * 3 + usize::from(m) + p.size[1] as usize * 101;
    let mut cache = PLANS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((s, plan)) = cache.get(&pos)
        && *s == stamp
    {
        return plan.clone();
    }
    let plan = Plan::unpack(&p);
    let plan = if q == 0 && !m { plan } else { plan.transformed(q, m) };
    cache.insert(pos, (stamp, plan.clone()));
    plan
}

/// On a client: each missing cell's block, smaller, where the world has air; red glass in a cell
/// another block took. Only while the ghost shows and the projector has energy.
pub(super) fn draw_ghost(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) -> bool {
    if read_i64(&entity.nbt, GHOST_KEY) == 0 || read_i64(&entity.nbt, LIT_KEY) == 0 {
        return false;
    }
    let plan = client_plan(pos, &entity.nbt);
    let mut drawn = 0;
    for cell in plan.missing(|o| block_at(game, offset(pos, o))) {
        if drawn >= GHOST_MAX {
            break;
        }
        let there = block_at(game, offset(pos, cell.at));
        draw.push();
        if there.is_some() {
            // Another block in the plan's cell: red, a little bigger than the block.
            draw.translate([cell.at[0] as f32 - 0.01, cell.at[1] as f32 - 0.01, cell.at[2] as f32 - 0.01]);
            draw.scale([1.02; 3]);
            game.native_draw_block(draw, "minecraft:red_stained_glass", &[]);
        } else {
            let k = (1.0 - GHOST_SCALE) / 2.0;
            draw.translate([cell.at[0] as f32 + k, cell.at[1] as f32 + k, cell.at[2] as f32 + k]);
            draw.scale([GHOST_SCALE; 3]);
            let props = plan::state_props(&cell.block);
            let refs: Vec<(&str, &str)> = props.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            game.native_draw_block(draw, cell.id(), &refs);
        }
        draw.pop();
        drawn += 1;
    }
    true
}

/// The projector's culling box: its plan's box (32 a side at most).
pub(super) fn ghost_bounds(entity: &ModBlockEntity, bounds: &mut [f32; 6]) {
    let b = ints(&entity.nbt, BOX_KEY);
    if b.len() != 6 {
        let h = plan::HALF as f32;
        *bounds = [-h, 0.0, -h, h + 1.0, plan::HEIGHT as f32 + 1.0, h + 1.0];
        return;
    }
    // Any turn keeps the box within its widest half-diagonal round the projector.
    let r = [b[0].abs(), b[2].abs(), (b[0] + b[3]).abs(), (b[2] + b[5]).abs()].into_iter().max().unwrap_or(0) as f32 + 1.0;
    *bounds = [-r, b[1] as f32, -r, r + 1.0, (b[1] + b[4]) as f32 + 1.0, r + 1.0];
}

// ---- The building machine ----

/// Two blocks a second into the plan's next missing cells, from the storage on its magistral or
/// its own slots.
pub(super) fn build(game: &mut Game<'_>, dimension: &str, level: &LevelNet, pos: Pos, cost: f64) -> Shown {
    let Some(projector) = util::neighbour_of(game, pos, Util::Projector.spec().block) else {
        return Shown::Blocked("builder_no_projector");
    };
    let Some(pe) = game.native_mod_block_entity(projector) else {
        return Shown::Blocked("builder_no_projector");
    };
    let plan = plan_of(&pe.nbt);
    drop(pe);
    if plan.cells.is_empty() {
        return Shown::Blocked("builder_no_plan");
    }
    let id = storage::storage_by(dimension, pos, &level.topology);
    let slots = items(game, pos);
    let has = |item: &str| {
        id.is_some_and(|id| storage::stored_count(dimension, id, item) > 0)
            || slots.iter().flatten().any(|s| s.count > 0 && &*s.item == item)
    };
    let missing: Vec<&Cell> = plan.missing(|o| block_at(game, offset(projector, o))).collect();
    let skipped = missing.iter().filter(|c| block_at(game, offset(projector, c.at)).is_some()).count();
    let free: Vec<&Cell> = missing.iter().copied().filter(|c| block_at(game, offset(projector, c.at)).is_none()).collect();
    let lacking: Vec<(String, u32)> = Plan::counts(free.iter().copied().filter(|c| !has(c.id())))
        .into_iter()
        .collect();
    state().builder.insert(pos, (skipped, lacking.clone()));
    let Some(cell) = free.iter().find(|c| has(c.id())).map(|c| (*c).clone()) else {
        return if missing.is_empty() {
            Shown::Idle("builder_done")
        } else if free.is_empty() {
            Shown::Blocked("builder_foreign")
        } else {
            Shown::Blocked("builder_no_blocks")
        };
    };
    if game.native_default_state(cell.id()).is_none() {
        return Shown::Blocked("builder_no_blocks");
    }
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    // From the storage first, else from its own slots.
    let from_storage = id.is_some_and(|id| storage::take_from(dimension, id, cell.id(), 1) == 1);
    let at = offset(projector, cell.at);
    if !place_state(game, dimension, at, &cell.block) {
        if from_storage && let Some(id) = id {
            storage::restore_into(dimension, id, cell.id(), 1);
        }
        return Shown::Blocked("builder_no_blocks");
    }
    if from_storage && let Some(id) = id {
        // rustcraft_debug: the building machine's block out of the storage.
        let route = format!("storage#{id} -> rustcraft:builder({})", rustcraft_rules::debug::pos_text(pos));
        crate::game::rustcraft_debug::flow(dimension, &route, cell.id(), 1);
    }
    let pays = game
        .native_with_mod_block_entity(pos, |e| {
            if !from_storage
                && let Some(slot) = e.items.iter().position(|s| s.as_ref().is_some_and(|s| *s.item == *cell.id()))
            {
                util::take_one(&mut e.items, slot);
            }
            let mut built: Built = read_json(&e.nbt, BUILT_KEY);
            let pays = built.pays(cell.at);
            write_json(&mut e.nbt, BUILT_KEY, &built);
            pays
        })
        .unwrap_or(false);
    let block = cell.id().to_owned();
    match base::owner_at(game, pos) {
        Some(o) if pays => base::report(Report::Build { owner: o, dimension: dimension.to_owned(), pos: at, block }),
        _ => base::report(Report::Place { dimension: dimension.to_owned(), pos: at, block }),
    }
    Shown::Working
}

/// The building machine's window: «слой N из M», its progress, what the storage lacks, the cells
/// it skipped.
pub(super) fn builder_sheet(game: &Game<'_>, pos: Pos, s: &mut rustcraft_rules::machines::sheet::Sheet) {
    let Some(projector) = util::neighbour_of(game, pos, Util::Projector.spec().block) else {
        return;
    };
    let Some(pe) = game.native_mod_block_entity(projector) else { return };
    let plan = plan_of(&pe.nbt);
    drop(pe);
    let missing: Vec<&Cell> = plan.missing(|o| block_at(game, offset(projector, o))).collect();
    let layers = plan.layers();
    let now = missing.first().map_or(layers.len(), |c| layers.iter().position(|y| *y == c.at[1]).unwrap_or(0) + 1);
    let (skipped, lacking) = state().builder.get(&pos).cloned().unwrap_or_default();
    let lack: Vec<String> = lacking.iter().take(2).map(|(k, n)| format!("{} ×{n}", k.rsplit(':').next().unwrap_or(k))).collect();
    let first = line(
        "rustcraft.util.builder.layer",
        "Layer %s of %s · skipped cells: %s",
        &[Text::literal(now.to_string()), Text::literal(layers.len().to_string()), Text::literal(skipped.to_string())],
    );
    let second = if lack.is_empty() {
        line("rustcraft.util.builder.has_all", "The storage has every block", &[])
    } else {
        line("rustcraft.util.builder.lacks", "Not in storage: %s", &[Text::literal(lack.join(", "))])
    };
    s.rows = vec![base::v(&first), base::v(&second)];
    let total = plan.cells.len().max(1);
    let done = plan.cells.len() - missing.len();
    s.bars.push(rustcraft_rules::machines::sheet::Bar {
        label: base::v(&line("rustcraft.util.builder.progress", "Built", &[])),
        value: base::v(&Text::literal(format!("{done} / {}", plan.cells.len()))),
        share: done as f64 / total as f64,
        energy: false,
        colour: None,
    });
}
