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
    Game, native_block_entity_draw::BlockEntityDraw,
    rustcraft_content as content,
    rustcraft_utility::{self as util, items, read_i64, read_json, spend, with_items, write_i64},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack, text::Text};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    utility::{
        Shown, Util,
        plan::{self, Plan},
        transit::Transit,
    },
};
use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const PLAN_KEY: &str = "plan";
const DOCK_KEY: &str = "dock_id";
pub(super) const FACADE_KEY: &str = "facade";
pub(super) const LOOK_KEY: &str = "facade_state";
pub(super) const FACADE: &str = "rustcraft:facade";
const FACADE_PLAIN: &str = "minecraft:smooth_stone";

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

pub(super) fn step(game: &mut Game<'_>, dimension: &str, level: &crate::game::rustcraft_server::LevelNet, pos: Pos, u: Util, cost: f64, _now: u64) -> Shown {
    // Slice 18: the projector and the building machine are rustcraft_projector's.
    match u {
        Util::Projector => crate::game::rustcraft_projector::projector_step(game, dimension, level, pos, cost),
        Util::Builder => crate::game::rustcraft_projector::build(game, dimension, level, pos, cost),
        _ => dock(game, dimension, level, pos, cost),
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
            let s = state();
            let partner = id.and_then(|id| s.transit.partner(id));
            match partner {
                // Wave 2 (ui.js `portal_dock`): «Пара · док в Незере · связан».
                Some(p) => {
                    let world = s.transit.worlds.get(&p).cloned();
                    drop(s);
                    match world {
                        Some(w) => util_line("rustcraft.util.portal_dock.paired_in", "Pair: a dock in %s · paired", &[world_name(&w)]),
                        None => util_line("rustcraft.util.portal_dock.paired", "Paired", &[]),
                    }
                }
                None => util_line("rustcraft.util.portal_dock.unpaired", "No pair: click two docks with the wrench", &[]),
            }
        }
    }
}

/// Wave 2: a world's name, «Незер», «Верхний мир», «Энд» (the game's own ids otherwise).
fn world_name(world: &str) -> Text {
    let key = format!("rustcraft.world.{}", world.rsplit(':').next().unwrap_or(world));
    util_line(&key, world, &[])
}

/// Wave 2 (ui.js `portal_dock`): the dock's first row, «Тот мир · загружен / выгружен», with its
/// fluid buffer «жидкость N из 16 вёдер» (a line: 27 slots leave no room for a bar in 240 px).
pub(super) fn dock_world_row(game: &Game<'_>, pos: Pos) -> Text {
    use rustcraft_rules::utility::transit::FLUID_ROOM;
    let id = dock_id(game, pos);
    let s = state();
    let partner = id.and_then(|id| s.transit.partner(id));
    let held = partner.map_or(0, |p| s.transit.fluid_held(p));
    let loaded = partner.map(|p| s.transit.loaded(p, now_ms()));
    drop(s);
    let world = match loaded {
        Some(true) => util_line("rustcraft.util.portal_dock.world_loaded", "The other world · loaded", &[]),
        Some(false) => util_line("rustcraft.util.portal_dock.world_unloaded", "The other world · not loaded", &[]),
        None => util_line("rustcraft.util.portal_dock.world_none", "The other world · no pair", &[]),
    };
    let buckets = |mb: u64| Text::literal(format!("{}", mb / 1000));
    let mut row = world;
    row.push(util_line(
        "rustcraft.util.portal_dock.fluid",
        " · fluid %s of %s buckets",
        &[buckets(held), buckets(FLUID_ROOM)],
    ));
    row
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn util_line(key: &str, fallback: &str, args: &[Text]) -> Text {
    crate::game::rustcraft_server::line(key, fallback, args)
}

// ---- The projector ----


/// A press in its window: slice 18's (rustcraft_projector).
pub(super) fn projector_press(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, i: usize) {
    crate::game::rustcraft_projector::projector_press(game, pos, entity, i);
}


/// On a client: slice 18's ghost (rustcraft_projector).
pub(super) fn draw_ghost(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) -> bool {
    crate::game::rustcraft_projector::draw_ghost(game, pos, entity, draw)
}


/// The projector's area for culling.
pub(super) fn ghost_bounds(bounds: &mut [f32; 6]) {
    let h = plan::HALF as f32;
    *bounds = [-h, 0.0, -h, h + 1.0, plan::HEIGHT as f32 + 1.0, h + 1.0];
}

// ---- The building machine ----


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

fn dock(game: &mut Game<'_>, dimension: &str, level: &crate::game::rustcraft_server::LevelNet, pos: Pos, cost: f64) -> Shown {
    let id = ensure_id(game, pos);
    let partner = {
        let mut s = state();
        load(game, &mut s);
        // Wave 2: the other dock knows this one's world is loaded, and which world it is.
        s.transit.saw(id, dimension, now_ms());
        s.transit.partner(id)
    };
    let Some(partner) = partner else {
        return Shown::Blocked("dock_unpaired");
    };
    if !by_portal(game, pos) {
        return Shown::Blocked("dock_no_portal");
    }
    let mut moved = dock_fluids(game, level, pos, id, partner, cost);
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
    // Slice 18 (stream 6): items wait and its pair's store has no room: «Буфер дока полон».
    let mut full = false;
    for slot in 0..18 {
        let Some(Some(st)) = slots.get(slot) else { continue };
        let max = crate::game::rustcraft_machines::max_stack(game, &st.item);
        let room = state().transit.room(partner, &st.item, max);
        let n = st.count.min(room);
        if n == 0 {
            full |= st.count > 0;
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
    // Wave 2: «Копит груз: другой мир выгружен».
    let holding = {
        let s = state();
        !s.transit.loaded(partner, now_ms()) && s.transit.holds_for(partner)
    };
    if full {
        Shown::Blocked("dock_full")
    } else if holding {
        Shown::Idle("dock_holding")
    } else if moved {
        Shown::Working
    } else {
        Shown::Idle("dock_idle")
    }
}

/// Wave 2 (guide `portal_dock`: «предметы и жидкости», «16 вёдер», «1 Е за ведро»): what came
/// from the pair goes into the tanks the dock unloads to; the fluid of the tanks it sends from
/// goes to the pair's store, as far as its 16 buckets go, 1 E a bucket. Whether anything moved.
fn dock_fluids(game: &mut Game<'_>, level: &crate::game::rustcraft_server::LevelNet, pos: Pos, id: u64, partner: u64, cost: f64) -> bool {
    use crate::game::rustcraft_fluids as fluids;
    let t = crate::game::rustcraft_server::tuning();
    let mut moved = false;
    let came = state().transit.take_fluids(id);
    let mut left = Vec::new();
    for (fluid, mb) in came {
        let put = fluids::dock_move(game, level, pos, &fluid, mb, true, t);
        moved |= put > 0;
        left.push((fluid, mb - put));
    }
    {
        let mut s = state();
        s.transit.put_back_fluids(id, left);
        s.dirty = true;
    }
    for (fluid, have) in fluids::dock_offer(game, level, pos, t) {
        let room = state().transit.fluid_room(partner);
        // Whole buckets, 1 E each.
        let buckets = have.min(room) / 1000;
        if buckets == 0 {
            continue;
        }
        if !spend(game, pos, cost * buckets as f64) {
            break;
        }
        let took = fluids::dock_move(game, level, pos, &fluid, buckets * 1000, false, t);
        let sent = state().transit.send_fluid(partner, &fluid, took);
        moved |= sent > 0;
    }
    moved
}

/// Wave 2 (the self-test's bench): the docks at `a` and `b` paired, as two wrench clicks pair
/// them.
pub(super) fn pair_docks(game: &mut Game<'_>, a: Pos, b: Pos) -> Result<(), String> {
    let (ia, ib) = (ensure_id(game, a), ensure_id(game, b));
    let mut s = state();
    load(game, &mut s);
    s.dirty = true;
    s.transit.click(0, ia);
    s.transit.click(0, ib).map(|_| ()).ok_or_else(|| "the docks did not pair".to_owned())
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
    // Slice 18: the wrench sets a projector's corners, and lifts it with its plan.
    if &*stack.item == content::WRENCH
        && let Some(dimension) = game.native_acting_player().map(|p| p.dimension)
        && crate::game::rustcraft_projector::on_wrench(game, player, &dimension, sneaking, pos)
    {
        return true;
    }
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
    } else if covered
        && game.items().get(item).is_some_and(|i| i.block.is_some())
        && !crate::game::rustcraft_server::is_ours(item)
        // s18/pipe, F2: only a whole block gives the look.
        && rustcraft_rules::housing::whole_block(item)
    {
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
