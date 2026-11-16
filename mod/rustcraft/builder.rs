// Slice 12: the holo-projector, the building machine, the portal dock and the facade.
//
// - The projector (spec 22): its window remembers the blocks of its area (9 × 9, 8 up, from
//   rustcraft_rules::utility::plan) into its block entity, shows or hides their ghost and clears the
//   plan. Clients get the plan in its update tag (rustcraft_anim) and draw each missing block of it
//   smaller, where the world has air, from the block entity (`native_draw_block`), as SoulsCraft's
//   altar draws its ghost.
// - The building machine (BU1): beside a projector, a block a second from its slots into the
//   plan's next missing cell (`native_set_block`, the block's default state); the first time a cell
//   is built Path-of-craft-tree hears `build`, then `place` only.
// - The portal dock (PD1–PD4): an id of its own; two wrench clicks pair two docks, in any worlds.
//   Each second what the net put into its first 18 slots goes, 1 E an item, to the store of the
//   server's on the way to its pair (rustcraft_rules::utility::transit), and what waits for it
//   comes into its last 9, which the net takes from. The store is a file of the world
//   (`data/rustcraft_transit.json`), so nothing is lost while the other world sleeps (PD3).
// - The facade (spec 15, F1–F3, F6): a click with it on a housing or a node hides it as smooth
//   stone; a click with a whole block gives it that look (the block is kept and given back); a
//   sneaking click with the wrench takes the facade and its block off. Clients draw the look over
//   the housing from its block entity. Not made: F4's full collision and F5's see-through panels
//   while a wrench is held.
use crate::game::{
    Game, native_block_entity_draw::BlockEntityDraw, rustcraft_base as base,
    rustcraft_content as content,
    rustcraft_utility::{self as util, items, read_i64, read_json, spend, with_items, write_i64, write_json},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack, text::Text};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    reports::Report,
    utility::{
        Shown, Util,
        plan::{self, Built, Plan},
        transit::Transit,
    },
};
use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const PLAN_KEY: &str = "plan";
const GHOST_KEY: &str = "ghost";
const BUILT_KEY: &str = "built";
const DOCK_KEY: &str = "dock_id";
pub(super) const FACADE_KEY: &str = "facade";
pub(super) const LOOK_KEY: &str = "facade_state";
pub(super) const FACADE: &str = "rustcraft:facade";
const FACADE_PLAIN: &str = "minecraft:smooth_stone";
/// The ghost's blocks are drawn this big, about their cell's middle.
const GHOST_SCALE: f32 = 0.6;
/// The most ghost blocks a projector draws in a frame.
const GHOST_MAX: usize = 256;

#[derive(Default)]
struct State {
    path: Option<PathBuf>,
    transit: Transit,
    dirty: bool,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn step(game: &mut Game<'_>, dimension: &str, pos: Pos, u: Util, cost: f64, _now: u64) -> Shown {
    match u {
        Util::Projector => projector_shown(game, pos),
        Util::Builder => build(game, dimension, pos, cost),
        _ => dock(game, pos, cost),
    }
}

/// Its window's second line.
pub(super) fn row(game: &Game<'_>, pos: Pos, u: Util) -> Text {
    match u {
        Util::Projector => {
            let n = game.native_mod_block_entity(pos).map_or(0, |e| read_json::<Plan>(&e.nbt, PLAN_KEY).cells.len());
            util_line("rustcraft.util.projector.cells", "Remembers %s blocks", &[Text::literal(n.to_string())])
        }
        Util::Builder => util_line("rustcraft.util.builder.next", "Builds its projector's plan beside it", &[]),
        _ => {
            let id = dock_id(game, pos);
            let paired = id.is_some_and(|id| state().transit.partner(id).is_some());
            if paired {
                util_line("rustcraft.util.portal_dock.paired", "Paired", &[])
            } else {
                util_line("rustcraft.util.portal_dock.unpaired", "No pair: click two docks with the wrench", &[])
            }
        }
    }
}

fn util_line(key: &str, fallback: &str, args: &[Text]) -> Text {
    crate::game::rustcraft_server::line(key, fallback, args)
}

// ---- The projector ----

fn projector_shown(game: &mut Game<'_>, pos: Pos) -> Shown {
    let Some(e) = game.native_mod_block_entity(pos) else {
        return Shown::Idle("idle");
    };
    let plan: Plan = read_json(&e.nbt, PLAN_KEY);
    if plan.cells.is_empty() {
        return Shown::Idle("projector_empty");
    }
    let on = read_i64(&e.nbt, GHOST_KEY) != 0;
    if on { Shown::Working } else { Shown::Idle("projector_hidden") }
}

fn block_at(game: &Game<'_>, at: Pos) -> Option<String> {
    let b = game.native_block(at);
    (!matches!(b.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "minecraft:water" | "minecraft:lava"))
        .then_some(b)
}

fn offset(pos: Pos, o: [i32; 3]) -> Pos {
    [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]]
}

/// A press in its window: remember, ghost on or off, clear.
pub(super) fn projector_press(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, i: usize) {
    match i {
        0 => {
            let p = Plan::read(|o| block_at(game, offset(pos, o)).filter(|b| !crate::game::rustcraft_server::is_ours(b)));
            write_json(&mut entity.nbt, PLAN_KEY, &p);
            write_i64(&mut entity.nbt, GHOST_KEY, 1);
        }
        1 => {
            let on = read_i64(&entity.nbt, GHOST_KEY) != 0;
            write_i64(&mut entity.nbt, GHOST_KEY, i64::from(!on));
        }
        _ => {
            write_json(&mut entity.nbt, PLAN_KEY, &Plan::default());
            write_i64(&mut entity.nbt, GHOST_KEY, 0);
        }
    }
    game.native_block_entity_sync(pos);
}

/// On a client: the plan's missing blocks, smaller, where the world has air.
pub(super) fn draw_ghost(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) -> bool {
    if read_i64(&entity.nbt, GHOST_KEY) == 0 {
        return false;
    }
    let plan: Plan = read_json(&entity.nbt, PLAN_KEY);
    let mut drawn = 0;
    for cell in plan.missing(|o| block_at(game, offset(pos, o))) {
        if drawn >= GHOST_MAX {
            break;
        }
        if block_at(game, offset(pos, cell.at)).is_some() {
            continue;
        }
        draw.push();
        let k = (1.0 - GHOST_SCALE) / 2.0;
        draw.translate([cell.at[0] as f32 + k, cell.at[1] as f32 + k, cell.at[2] as f32 + k]);
        draw.scale([GHOST_SCALE; 3]);
        game.native_draw_block(draw, &cell.block, &[]);
        draw.pop();
        drawn += 1;
    }
    true
}

/// The projector's area for culling.
pub(super) fn ghost_bounds(bounds: &mut [f32; 6]) {
    let h = plan::HALF as f32;
    *bounds = [-h, 0.0, -h, h + 1.0, plan::HEIGHT as f32 + 1.0, h + 1.0];
}

// ---- The building machine ----

fn build(game: &mut Game<'_>, dimension: &str, pos: Pos, cost: f64) -> Shown {
    let Some(projector) = util::neighbour_of(game, pos, Util::Projector.spec().block) else {
        return Shown::Blocked("builder_no_projector");
    };
    let Some(pe) = game.native_mod_block_entity(projector) else {
        return Shown::Blocked("builder_no_projector");
    };
    let plan: Plan = read_json(&pe.nbt, PLAN_KEY);
    if plan.cells.is_empty() {
        return Shown::Blocked("builder_no_plan");
    }
    let slots = items(game, pos);
    let next = plan
        .missing(|o| block_at(game, offset(projector, o)))
        .find(|c| {
            block_at(game, offset(projector, c.at)).is_none()
                && slots.iter().flatten().any(|s| s.count > 0 && *s.item == *c.block)
        })
        .cloned();
    let Some(cell) = next else {
        let done = plan.missing(|o| block_at(game, offset(projector, o))).next().is_none();
        return if done { Shown::Idle("builder_done") } else { Shown::Blocked("builder_no_blocks") };
    };
    let Some(state_id) = game.native_default_state(&cell.block) else {
        return Shown::Blocked("builder_no_blocks");
    };
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    let at = offset(projector, cell.at);
    game.native_set_block(dimension, at, state_id, 3);
    let pays = game
        .native_with_mod_block_entity(pos, |e| {
            if let Some(slot) = e.items.iter().position(|s| s.as_ref().is_some_and(|s| *s.item == *cell.block)) {
                util::take_one(&mut e.items, slot);
            }
            let mut built: Built = read_json(&e.nbt, BUILT_KEY);
            let pays = built.pays(cell.at);
            write_json(&mut e.nbt, BUILT_KEY, &built);
            pays
        })
        .unwrap_or(false);
    match base::owner_at(game, pos) {
        Some(o) if pays => base::report(Report::Build {
            owner: o,
            dimension: dimension.to_owned(),
            pos: at,
            block: cell.block,
        }),
        _ => base::report(Report::Place {
            dimension: dimension.to_owned(),
            pos: at,
            block: cell.block,
        }),
    }
    Shown::Working
}

// ---- The portal dock ----

fn load(game: &Game<'_>, s: &mut State) {
    if s.path.is_some() {
        return;
    }
    let Some(world) = game.native_world_dir() else {
        return;
    };
    let path = world.join("data").join("rustcraft_transit.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        match serde_json::from_str(&text) {
            Ok(t) => s.transit = t,
            Err(e) => log::warn!("rustcraft: {} does not read: {e}", path.display()),
        }
    }
    s.path = Some(path);
}

/// Writes the store when it changed, whole under another name first.
pub(super) fn save_transit(game: &Game<'_>) {
    let mut s = state();
    load(game, &mut s);
    if !s.dirty {
        return;
    }
    let Some(path) = s.path.clone() else { return };
    let Ok(json) = serde_json::to_string(&s.transit) else { return };
    s.dirty = false;
    drop(s);
    let fresh = path.with_extension("json.tmp");
    let done = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&fresh, json))
        .and_then(|()| std::fs::rename(&fresh, &path));
    if let Err(e) = done {
        log::warn!("rustcraft: the portal docks' store was not saved: {e}");
    }
}

fn dock_id(game: &Game<'_>, pos: Pos) -> Option<u64> {
    let id = game.native_mod_block_entity(pos).map(|e| read_i64(&e.nbt, DOCK_KEY))?;
    (id != 0).then_some(id as u64)
}

/// Its id, made at its first tick.
fn ensure_id(game: &mut Game<'_>, pos: Pos) -> u64 {
    if let Some(id) = dock_id(game, pos) {
        return id;
    }
    let id = ((util::random() * 9.0e15) as u64).max(1);
    game.native_with_mod_block_entity(pos, |e| write_i64(&mut e.nbt, DOCK_KEY, id as i64));
    id
}

fn by_portal(game: &Game<'_>, pos: Pos) -> bool {
    (-1..=1).any(|dx| (-1..=1).any(|dy| (-1..=1).any(|dz| game.native_block([pos[0] + dx, pos[1] + dy, pos[2] + dz]) == "minecraft:nether_portal")))
}

fn dock(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let id = ensure_id(game, pos);
    let partner = {
        let mut s = state();
        load(game, &mut s);
        s.transit.partner(id)
    };
    let Some(partner) = partner else {
        return Shown::Blocked("dock_unpaired");
    };
    if !by_portal(game, pos) {
        return Shown::Blocked("dock_no_portal");
    }
    let mut moved = false;
    // What waits for this dock comes in.
    let waiting = state().transit.take(id);
    let mut back = Vec::new();
    for (item, count) in waiting {
        let left = util::put(game, pos, 18..27, ItemStack::new(&item, count));
        if let Some(l) = left {
            back.push((item, l.count));
        } else {
            moved = true;
        }
    }
    {
        let mut s = state();
        s.transit.put_back(id, back);
        s.dirty = true;
    }
    // What the net gave it goes.
    let slots = items(game, pos);
    for slot in 0..18 {
        let Some(Some(st)) = slots.get(slot) else { continue };
        let max = crate::game::rustcraft_machines::max_stack(game, &st.item);
        let room = state().transit.room(partner, &st.item, max);
        let n = st.count.min(room);
        if n == 0 {
            continue;
        }
        if !spend(game, pos, cost * f64::from(n)) {
            return Shown::NoEnergy;
        }
        state().transit.send(partner, &st.item, n, max);
        with_items(game, pos, |items| {
            if let Some(Some(s)) = items.get_mut(slot) {
                s.count -= n.min(s.count);
                if s.count == 0 {
                    items[slot] = None;
                }
            }
        });
        moved = true;
    }
    if moved { Shown::Working } else { Shown::Idle("dock_idle") }
}

/// A wrench click on a portal dock: pairs it with the dock clicked before.
pub(super) fn wrench_dock(game: &mut Game<'_>, player: u128, pos: Pos) {
    let id = ensure_id(game, pos);
    let paired = {
        let mut s = state();
        load(game, &mut s);
        s.dirty = true;
        s.transit.click(player, id)
    };
    match paired {
        Some(_) => util::say(game, player, "rustcraft.util.portal_dock.now_paired", "The docks are paired", &[]),
        None => util::say(game, player, "rustcraft.util.portal_dock.first", "Now click its pair with the wrench", &[]),
    }
}

// ---- The facade ----

pub(super) fn has_facade(nbt: &Compound) -> bool {
    read_i64(nbt, FACADE_KEY) != 0
}

pub(super) fn look(nbt: &Compound) -> Option<String> {
    match nbt.get(&JavaString::from(LOOK_KEY)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// A click with `stack` on a housing or node at `pos`: claims it when it was the facade's.
pub(super) fn on_use_item_on(game: &mut Game<'_>, player: u128, sneaking: bool, stack: &ItemStack, pos: Pos) -> bool {
    let block = game.native_block(pos);
    if block != content::HOUSING && block != content::NODE {
        return false;
    }
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return false;
    };
    let item: &str = &stack.item;
    let covered = has_facade(&entity.nbt);
    if item == FACADE && !covered {
        game.native_with_mod_block_entity(pos, |e| write_i64(&mut e.nbt, FACADE_KEY, 1));
        game.native_take_items(FACADE, 1);
        // s18/pipe, F4: the hidden housing is a whole block (the node is one already).
        game.native_set_block_property(pos, rustcraft_rules::housing::COVERED, "true");
    } else if item == content::WRENCH && sneaking && covered {
        let old = look(&entity.nbt);
        game.native_with_mod_block_entity(pos, |e| {
            e.nbt.remove(&JavaString::from(FACADE_KEY));
            e.nbt.remove(&JavaString::from(LOOK_KEY));
        });
        give(game, player, FACADE);
        if let Some(o) = old {
            give(game, player, &o);
        }
        game.native_set_block_property(pos, rustcraft_rules::housing::COVERED, "false");
    } else if covered && game.items().get(item).is_some_and(|i| i.block.is_some()) && !crate::game::rustcraft_server::is_ours(item) {
        let old = look(&entity.nbt);
        game.native_with_mod_block_entity(pos, |e| {
            e.nbt.insert(JavaString::from(LOOK_KEY), Tag::String(JavaString::from(item)));
        });
        game.native_take_items(item, 1);
        if let Some(o) = old {
            give(game, player, &o);
        }
    } else {
        return false;
    }
    game.native_block_entity_sync(pos);
    true
}

fn give(game: &mut Game<'_>, _player: u128, item: &str) {
    let _ = game.native_give_items(item, 1);
}

/// On a client: the housing's look over it.
pub(super) fn draw_facade(game: &Game<'_>, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) -> bool {
    // s18/pipe, F5: with the wrench in hand every facade is see-through; drawn each frame (not
    // kept) so that taking the wrench shows it at once.
    if !has_facade(&entity.nbt) || crate::game::rustcraft_pipe18::wrench_in_hand(game) {
        return false;
    }
    let look = look(&entity.nbt).unwrap_or_else(|| FACADE_PLAIN.to_owned());
    if !game.native_draw_block(draw, &look, &[]) {
        game.native_draw_block(draw, FACADE_PLAIN, &[]);
    }
    true
}
