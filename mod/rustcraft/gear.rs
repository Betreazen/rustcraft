// Slice 13: the player's gear on the server (rustcraft_rules::gear decides).
//
// - Every gear piece gets a number in its custom data the first tick it is carried
//   (`native_inventory_tick`, as a backpack does); what it holds — its charge, its player modules,
//   its technical enchantments — is kept under that number in the world's file
//   `data/rustcraft_gear.json`.
// - A sneaking right click with a piece opens its window (`native_open_menu_with_input`, the
//   backpack's way): its module sockets, then the Capacity and the Thrift book slots. Closing it
//   keeps the modules of its piece and up to three books each; the rest goes back to the player.
// - Twice a second, for each player of the level: the worn pieces' modules work on their charge —
//   night vision (switched by its key) and jump boost as the game's effects, the run module +50 %
//   speed for 5 s from the start of a sprint (an attribute modifier), the magnet (switched by its
//   key) pulls what lies within 6 blocks toward the player every 2 ticks for the game's pickup
//   (slice 18), the jetpack lets the player fly while the chestplate holds charge
//   (`native_set_flight`). What the damage hook needs is noted for it (it runs inside the hit,
//   with no game): the blade in hand, the damper, the shock absorber.
// - The damage hook: a charged blade strikes and pays 25 E (the fourth tier one point harder), an
//   empty one strikes as a hand; the shock absorber takes fall damage for 25 E a point; the damper
//   softens hits for 5 s after one by 20 %. Their spending is applied at the next tick.
// - The rifle: a right click shoots straight along the look, 64 blocks, the first creature on the
//   line, stopped by a solid block; 16 damage of the arrow's type from the player, so the base
//   game and Path-of-craft-tree count it as a bow's; 400 E, a shot every 1.25 s.
// - Chargers: `battery` and `charge` give the charger and the plate a piece's battery.
// - Slice 14, the harpoon (G5): a right click hooks the first solid block looked at within 20
//   blocks and pulls the player there at 14 blocks a second on the zipline's invisible seat
//   (rustcraft_monorail::zipline, method b2): onto the block when the space over it is free, else
//   to the cell in front of the face it hit; 100 E a shot from its 4 000 E battery. A mob met
//   first is pulled to the player, a big one pulls the player (rustcraft_zipline).
// - The zipline's ropes and anchors are rustcraft_zipline's; a use here pays their charge.
// - W6: a blade or shield of the next tier is made from the previous one by a transmute recipe
//   (`minecraft:crafting_transmute`, horizon-client/src/crafting.rs `Template::over`: the input's
//   component patch goes onto the result), so the new piece keeps the old one's number, and with
//   it its charge and its books.
use crate::game::{
    Game,
    native_player_state::NativeEffect,
    rustcraft_server::{line, tell},
    rustcraft_zipline as zipline,
};
use horizon_client::{items::ItemStack, text::Text};
use horizon_world::{
    nbt::{JavaString, Tag},
    player_inventory::EquipmentSlot,
};
use rustcraft_rules::{
    gear::{self as rules, Gear, Held, Kind, Module, Piece, shot},
    ray,
    traversal::ANCHOR,
    utility::charge::Battery,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const ID_KEY: &str = "rustcraft:gear_id";
const CUSTOM_DATA: &str = "minecraft:custom_data";
const EVERY: u64 = 10;
const SECONDS: f64 = EVERY as f64 / 20.0;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Store {
    next: u64,
    held: BTreeMap<u64, Held>,
}

/// What the damage hook knows of a player, from the last tick.
#[derive(Clone, Copy, Default)]
struct View {
    /// The blade in the main hand: its number, tier and whether it holds a hit's charge.
    blade: Option<(u64, u8, bool)>,
    absorber: Option<(u64, f64)>,
    damper: Option<(u64, f64)>,
}

#[derive(Default)]
struct State {
    path: Option<PathBuf>,
    store: Store,
    dirty: bool,
    ticks: HashMap<String, u64>,
    views: HashMap<i32, View>,
    /// Charge the damage hook spent, to take at the next tick.
    spent: Vec<(u64, f64)>,
    /// Ticks of the server when each player was last hurt (the damper).
    hurt: HashMap<i32, u64>,
    now: u64,
    /// Players the jetpack lets fly.
    flying: HashSet<u128>,
    /// The next tick each player's rifle may shoot.
    rifle_ready: HashMap<u128, u64>,
    /// Open gear windows: the player's window id and the piece's number and kind.
    open: HashMap<u128, (i32, u64, Gear)>,
    opening: Vec<(u128, u64, Gear)>,
    /// The dash: each player's last input flags and the tick forward last went down.
    input: HashMap<u128, (i32, u64)>,
    /// Players holding up a charged energy shield, by entity id: the shield's number.
    blocking: HashMap<i32, (u64, u8)>,
    /// Slice 18: the tick each player began to sprint (the run module's burst), the players
    /// whose burst modifier is on, and the players whose magnet pulls.
    sprint: HashMap<u128, u64>,
    running: HashSet<u128>,
    magnets: HashSet<u128>,
    /// Players whose leftover burst modifier (from a logout or a restart mid-burst) was taken
    /// off since they came on.
    cleaned: HashSet<u128>,
}

/// The client's side of the dash: a push the server asked for, applied at the window's next tick.
static PUSH: LazyLock<Mutex<Option<[f32; 3]>>> = LazyLock::new(Mutex::default);
const DASH_CHANNEL: &str = "rustcraft:dash";

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn load(s: &mut State, game: &Game<'_>) {
    if s.path.is_some() {
        return;
    }
    let Some(world) = game.native_world_dir() else {
        return;
    };
    let path = world.join("data").join("rustcraft_gear.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        match serde_json::from_str(&text) {
            Ok(st) => s.store = st,
            Err(e) => log::warn!("rustcraft: {} does not read: {e}", path.display()),
        }
    }
    s.path = Some(path);
}

fn save(s: &mut State) {
    if !s.dirty {
        return;
    }
    let Some(path) = s.path.clone() else { return };
    let Ok(json) = serde_json::to_string(&s.store) else { return };
    s.dirty = false;
    let fresh = path.with_extension("json.tmp");
    let done = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&fresh, json))
        .and_then(|()| std::fs::rename(&fresh, &path));
    if let Err(e) = done {
        log::warn!("rustcraft: the gear store was not saved: {e}");
    }
}

/// The piece's number, if it has one yet.
pub(super) fn id_of(stack: &ItemStack) -> Option<u64> {
    match stack.custom_data()?.get(&JavaString::from(ID_KEY))? {
        Tag::Long(v) => u64::try_from(*v).ok(),
        _ => None,
    }
}

fn set_id(stack: &mut ItemStack, id: u64) {
    let mut data = stack.custom_data().cloned().unwrap_or_default();
    data.insert(JavaString::from(ID_KEY), Tag::Long(id.min(i64::MAX as u64) as i64));
    stack.data_mut().set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
}

fn held_of(s: &State, stack: &ItemStack) -> Option<(u64, Gear, Held)> {
    let g = Gear::of(&stack.item)?;
    let id = id_of(stack)?;
    Some((id, g, s.store.held.get(&id).cloned().unwrap_or_default()))
}

// ---- The chargers' side ----

/// A piece's battery, for the charger and the plate.
pub(super) fn battery(game: &Game<'_>, stack: &ItemStack) -> Option<Battery> {
    let mut s = state();
    load(&mut s, game);
    let (_, g, h) = held_of(&s, stack)?;
    let cap = rules::charger_capacity(g, &h)?;
    Some(Battery { charge: h.charge.min(cap), capacity: cap })
}

/// A charger's fill: the piece's charge goes up to `charge_now`, within its charger battery, and
/// never down.
pub(super) fn charge(stack: &ItemStack, charge_now: f64) {
    let mut s = state();
    let Some(id) = id_of(stack) else { return };
    let Some(g) = Gear::of(&stack.item) else { return };
    let h = s.store.held.entry(id).or_default();
    let Some(cap) = rules::charger_capacity(g, h) else { return };
    rules::add_charge(h, cap, charge_now);
    s.dirty = true;
}

/// A new zipline comes with its 16 charges; batteries come empty.
fn needs_full(game: &mut Game<'_>, slot: i32) -> f64 {
    game.native_carried_stack(slot)
        .and_then(|s| Gear::of(&s.item))
        .filter(|g| g.kind == Kind::Zipline)
        .map_or(0.0, |_| rules::ZIP_CHARGES)
}

// ---- Hooks ----

/// `native_inventory_tick`: a gear piece without a number gets one.
pub(super) fn on_inventory_tick(game: &mut Game<'_>, slot: i32) {
    let (gear, numbered) = match game.native_carried_stack(slot) {
        Some(s) => (Gear::of(&s.item), id_of(s).is_some()),
        None => return,
    };
    let Some(g) = gear else { return };
    if numbered {
        show_bar(game, slot, g);
        return;
    }
    let id = {
        let mut s = state();
        load(&mut s, game);
        s.store.next += 1;
        let id = s.store.next;
        let full = needs_full(game, slot);
        s.store.held.insert(id, Held { charge: full, ..Held::default() });
        s.dirty = true;
        save(&mut s);
        id
    };
    if let Some(stack) = game.native_carried_stack(slot) {
        set_id(stack, id);
    }
}

// ---- Slice 18: the charge bar under the icon ----

/// The battery a piece's bar is measured against: its charger battery, the zipline's charges.
fn shown_capacity(g: Gear, h: &Held) -> f64 {
    rules::charger_capacity(g, h).unwrap_or_else(|| g.base_capacity())
}

fn bar_of(stack: &ItemStack) -> Option<u8> {
    match stack.custom_data()?.get(&JavaString::from(rules::BAR_KEY))? {
        Tag::Byte(v) => u8::try_from(*v).ok(),
        _ => None,
    }
}

/// A carried piece's bar width, written into its custom data when it changed.
fn show_bar(game: &mut Game<'_>, slot: i32, g: Gear) {
    let width = {
        let s = state();
        let Some(stack) = game.native_carried_stack(slot) else { return };
        let Some(id) = id_of(stack) else { return };
        let h = s.store.held.get(&id).cloned().unwrap_or_default();
        let w = rules::bar(h.charge, shown_capacity(g, &h));
        if bar_of(stack) == Some(w) {
            return;
        }
        w
    };
    if let Some(stack) = game.native_carried_stack(slot) {
        let mut data = stack.custom_data().cloned().unwrap_or_default();
        data.insert(JavaString::from(rules::BAR_KEY), Tag::Byte(width as i8));
        stack.data_mut().set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
    }
}

/// `native_item_bar` on a client: a gear piece's bar from its custom data.
pub(super) fn item_bar(stack: &ItemStack, bar: &mut Option<(i32, u32)>) {
    if let Some(w) = bar_of(stack) {
        *bar = Some((i32::from(w), rules::bar_rgb(w)));
    }
}

/// `native_use_item`: a sneaking right click with a piece opens its window; the rifle shoots.
pub(super) fn on_use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed {
        return;
    }
    let Some(g) = Gear::of(&stack.item) else { return };
    let Some(player) = game.native_acting_player() else { return };
    if player.sneaking {
        *claimed = true;
        match id_of(stack) {
            Some(id) => state().opening.push((player.uuid, id, g)),
            None => tell(game, player.uuid, line("rustcraft.gear.not_ready", "The piece is getting ready, try again", &[])),
        }
        return;
    }
    if g.kind == Kind::Rifle {
        *claimed = true;
        shoot(game, &player, stack);
    }
    if g.kind == Kind::Zipline {
        *claimed = true;
        zip(game, &player, stack);
    }
    if g.kind == Kind::Harpoon {
        *claimed = true;
        harpoon(game, &player, stack);
    }
}

/// Blocks a rope, a harpoon or a shot passes through.
fn see_through(block: &str) -> bool {
    matches!(
        block,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:water" | "minecraft:short_grass" | "minecraft:tall_grass"
    )
}

/// How far from `eye` a shot or a hook meets the block `p`: to its middle less half a block.
fn wall_at(eye: [f64; 3], p: [i32; 3]) -> f64 {
    let c = p.map(|v| f64::from(v) + 0.5);
    ((c[0] - eye[0]).powi(2) + (c[1] - eye[1]).powi(2) + (c[2] - eye[2]).powi(2)).sqrt() - 0.5
}

fn is_air(game: &Game<'_>, pos: [i32; 3]) -> bool {
    matches!(game.native_block(pos).as_str(), "minecraft:air" | "minecraft:cave_air")
}

/// Takes `cost` from the piece `id`'s charge: false, with the player told, when it holds less.
fn pay(game: &mut Game<'_>, player: u128, id: u64, cost: f64, empty: Text) -> bool {
    let mut s = state();
    let h = s.store.held.entry(id).or_default();
    if !rules::spend(h, cost) {
        drop(s);
        tell(game, player, empty);
        return false;
    }
    s.dirty = true;
    true
}

fn refund(id: u64, cost: f64) {
    let mut s = state();
    s.store.held.entry(id).or_default().charge += cost;
    s.dirty = true;
}

/// G5: the harpoon hooks the first mob or solid block looked at within 20 blocks. A mob is pulled
/// to the player, a big one pulls the player to it (rustcraft_zipline::hook_mob); a block pulls
/// the player onto it when there is room over it (two blocks of air), else to the cell in front
/// of the face it hit. 100 E a shot (less with Thrift), given back when nothing moves.
fn harpoon(game: &mut Game<'_>, player: &horizon_client::mod_api::NativeServerPlayer, stack: &ItemStack) {
    let Some(id) = id_of(stack) else { return };
    if crate::game::rustcraft_monorail::riding(player.uuid) {
        return;
    }
    let eye = [player.position[0], player.position[1] + f64::from(player.eye_height), player.position[2]];
    let dir = ray::look_direction(player.yaw, player.pitch);
    let path = ray::blocks_along(eye, dir, rules::HARPOON_RANGE);
    let hit_at = path.iter().skip(1).position(|p| !see_through(game.native_block(*p).as_str())).map(|i| i + 1);
    let wall = hit_at.map_or(f64::INFINITY, |i| wall_at(eye, path[i]));
    let empty = || line("rustcraft.gear.empty", "No charge: put it on a charger", &[]);
    let cost = state().store.held.get(&id).map_or(rules::HARPOON_SHOT, |h| rules::cost(rules::HARPOON_SHOT, h));
    if let Some((mob, w, h, at)) = zipline::harpoon_target(game, player, wall) {
        if !pay(game, player.uuid, id, cost, empty()) {
            return;
        }
        if !zipline::hook_mob(game, Some(player), &player.dimension, player.position, mob, w, h, at) {
            refund(id, cost);
            tell(game, player.uuid, line("rustcraft.harpoon.no_pull", "No room to pull it", &[]));
        }
        return;
    }
    let Some(i) = hit_at else {
        tell(game, player.uuid, line("rustcraft.harpoon.far", "Nothing to hook within 20 blocks", &[]));
        return;
    };
    let hit = path[i];
    let top = [hit[0], hit[1] + 1, hit[2]];
    let top_free = is_air(game, top) && is_air(game, [hit[0], hit[1] + 2, hit[2]]);
    // The cell the ray came from; none when that is the player's own.
    let before = (i >= 2).then(|| path[i - 1]);
    let before_free = before.is_some_and(|b| see_through(game.native_block(b).as_str()));
    let Some(anchor) = rules::harpoon_anchor(hit, before, top_free, before_free) else {
        tell(game, player.uuid, line("rustcraft.harpoon.no_room", "No room to pull you there", &[]));
        return;
    };
    if !pay(game, player.uuid, id, cost, empty()) {
        return;
    }
    let to = [f64::from(anchor[0]) + 0.5, f64::from(anchor[1]), f64::from(anchor[2]) + 0.5];
    let entity = player.entity_id as u32;
    let points = [player.position, to];
    if !crate::game::rustcraft_monorail::zipline(game, player.uuid, entity, &player.dimension, &points, anchor, rules::HARPOON_SPEED, true) {
        refund(id, cost);
    }
}

/// 12.5 Z1–Z3: a string or an anchor in the other hand gives a charge back; else a rope within
/// reach is ridden free, or a new one is tied to the top of the block looked at within 32 blocks
/// for a charge and ridden (rustcraft_zipline).
fn zip(game: &mut Game<'_>, player: &horizon_client::mod_api::NativeServerPlayer, stack: &ItemStack) {
    let Some(id) = id_of(stack) else { return };
    let off = game.native_player_equipment(player.uuid, EquipmentSlot::OffHand);
    if let Some(refill) = off.map(|s| s.item.to_string()).filter(|i| i == "minecraft:string" || i == ANCHOR) {
        let full = state().store.held.get(&id).is_some_and(|h| h.charge >= rules::ZIP_CHARGES);
        if !full && game.native_take_items(&refill, 1) > 0 {
            let mut s = state();
            let h = s.store.held.entry(id).or_default();
            h.charge = (h.charge + 1.0).min(rules::ZIP_CHARGES);
            s.dirty = true;
        }
        return;
    }
    let zipline::Plan::Tie { start, far } = zipline::plan(game, player) else {
        return;
    };
    if !pay(game, player.uuid, id, 1.0, line("rustcraft.zipline.empty", "No charges: a string or an anchor gives one back", &[])) {
        return;
    }
    if !zipline::tie_and_ride(game, player, start, far) {
        refund(id, 1.0);
    }
}

fn shoot(game: &mut Game<'_>, player: &horizon_client::mod_api::NativeServerPlayer, stack: &ItemStack) {
    let now = state().now;
    if state().rifle_ready.get(&player.uuid).is_some_and(|t| now < *t) {
        return;
    }
    let Some(id) = id_of(stack) else { return };
    let cost = {
        let mut s = state();
        let h = s.store.held.entry(id).or_default();
        let cost = rules::cost(rules::RIFLE_SHOT, h);
        if !rules::spend(h, cost) {
            drop(s);
            tell(game, player.uuid, line("rustcraft.gear.empty", "No charge: put it on a charger", &[]));
            return;
        }
        s.dirty = true;
        s.rifle_ready.insert(player.uuid, now + rules::RIFLE_COOLDOWN_TICKS);
        cost
    };
    let _ = cost;
    let eye = [player.position[0], player.position[1] + f64::from(player.eye_height), player.position[2]];
    let dir = ray::look_direction(player.yaw, player.pitch);
    let wall = ray::blocks_along(eye, dir, rules::RIFLE_RANGE)
        .into_iter()
        .skip(1)
        .find(|p| !see_through(game.native_block(*p).as_str()))
        .map_or(f64::INFINITY, |p| wall_at(eye, p));
    let me = player.entity_id as u32;
    let targets: Vec<shot::Target> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.alive() && m.id != me)
        .map(|m| {
            let (w, h) = (f64::from(m.width()) / 2.0, f64::from(m.height()));
            let p = m.position;
            shot::Target { id: m.id, lo: [p.x - w, p.y, p.z - w], hi: [p.x + w, p.y + h, p.z + w] }
        })
        .collect();
    let hit = shot::hit(eye, dir, &targets, wall, rules::RIFLE_RANGE);
    // Slice 18: the shot's beam, from the muzzle to the creature hit, or to the wall, or its range.
    let end = hit
        .and_then(|id| targets.iter().find(|t| t.id == id))
        .map(|t| [(t.lo[0] + t.hi[0]) / 2.0, (t.lo[1] + t.hi[1]) / 2.0, (t.lo[2] + t.hi[2]) / 2.0])
        .unwrap_or_else(|| {
            let d = wall.min(rules::RIFLE_RANGE);
            [eye[0] + dir[0] * d, eye[1] + dir[1] * d, eye[2] + dir[2] * d]
        });
    let muzzle = [eye[0] + dir[0] * 0.8, eye[1] - 0.15 + dir[1] * 0.8, eye[2] + dir[2] * 0.8];
    let options = horizon_client::particles::ParticleOptions::named("minecraft:electric_spark".to_owned());
    for p in rustcraft_rules::aim::beam(muzzle, end, rules::BEAM_STEP) {
        game.with_level(|_, level, _| level.native_send_particles(&options, None, p, [0.0; 3], 0.0, 1));
    }
    if let Some(target) = hit {
        game.native_hurt_entity(&player.dimension, target, rules::RIFLE_DAMAGE, "minecraft:arrow", Some(me));
    }
}

/// `native_menu_input_closed`: a piece's window keeps its modules and books.
pub(super) fn on_menu_input_closed(uuid: u128, window: i32, slots: &mut [Option<ItemStack>]) {
    let mut s = state();
    let Some(&(open, id, g)) = s.open.get(&uuid) else { return };
    if open != window {
        return;
    }
    s.open.remove(&uuid);
    let view: Vec<Option<(String, u32)>> = slots
        .iter()
        .map(|x| x.as_ref().filter(|x| x.count > 0).map(|x| (x.item.to_string(), x.count)))
        .collect();
    let (modules, capacity, thrift) = rules::from_window(g, &view);
    // What stays in the piece is taken out of the window; the rest goes back to the player.
    let sockets = g.sockets();
    for (i, slot) in slots.iter_mut().enumerate() {
        let Some(st) = slot.as_mut() else { continue };
        let keep = if i < sockets {
            modules.iter().any(|m| *m == *st.item)
        } else {
            let level = if i == sockets { capacity } else { thrift };
            level > 0 && rules::window_takes(g, i, &st.item, 1)
        };
        if !keep {
            continue;
        }
        if i < sockets {
            *slot = None;
        } else {
            let level = u32::from(if i == sockets { capacity } else { thrift });
            st.count -= level.min(st.count);
            if st.count == 0 {
                *slot = None;
            }
        }
    }
    let h = s.store.held.entry(id).or_default();
    h.modules = modules;
    h.capacity = capacity;
    h.thrift = thrift;
    s.dirty = true;
    save(&mut s);
}

/// `native_player_logged_out`: an open window is forgotten, flight given back.
pub(super) fn on_logout(uuid: u128) {
    let mut s = state();
    s.open.remove(&uuid);
    s.flying.remove(&uuid);
    s.sprint.remove(&uuid);
    s.running.remove(&uuid);
    s.magnets.remove(&uuid);
    s.cleaned.remove(&uuid);
}

// ---- The dash ----

/// `native_player_input`: two presses of forward with the dash module in the leggings throw the
/// player forward; the push is the client's own (`native_push_player`), so the server sends it.
pub(super) fn on_player_input(game: &mut Game<'_>, player: u128, flags: i32) {
    let dash = {
        let mut s = state();
        let now = s.now;
        let (before, last) = s.input.get(&player).copied().unwrap_or((0, 0));
        if rules::moves::sprint_started(before, flags) {
            s.sprint.insert(player, now);
        } else if flags & rules::moves::SPRINT == 0 {
            s.sprint.remove(&player);
        }
        let pressed = rules::forward_pressed(before, flags);
        let twice = pressed && last > 0 && now.saturating_sub(last) <= rules::DASH_WINDOW_TICKS;
        s.input.insert(player, (flags, if pressed && !twice { now } else if twice { 0 } else { last }));
        twice
    };
    if !dash {
        return;
    }
    let Some(legs) = game.native_player_equipment(player, EquipmentSlot::Legs) else { return };
    let paid = {
        let mut s = state();
        load(&mut s, game);
        match held_of(&s, &legs) {
            Some((id, Gear { kind: Kind::Armor(Piece::Leggings), .. }, h)) if h.has(Module::Dash) => {
                let cost = rules::cost(rules::DASH_COST, &h);
                let e = s.store.held.entry(id).or_default();
                let ok = rules::spend(e, cost);
                s.dirty |= ok;
                ok
            }
            _ => false,
        }
    };
    if !paid {
        return;
    }
    let Some(p) = game.native_server_players().into_iter().find(|p| p.uuid == player) else { return };
    let push = rules::dash_push(p.yaw);
    let bytes: Vec<u8> = push.iter().flat_map(|v| v.to_le_bytes()).collect();
    if let Err(e) = game.native_net_send_to_player(player, DASH_CHANNEL, &bytes) {
        log::warn!("rustcraft: the dash was not sent: {e}");
    }
}

/// `native_net_from_server` on a client: the server's dash, a gear window's view.
pub(super) fn on_net(channel: &str, data: &[u8]) {
    if channel == rules::window::CHANNEL {
        *WINDOW.lock().unwrap_or_else(PoisonError::into_inner) = serde_json::from_slice(data).ok();
        return;
    }
    if channel != DASH_CHANNEL || data.len() != 12 {
        return;
    }
    let f = |i: usize| f32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
    *PUSH.lock().unwrap_or_else(PoisonError::into_inner) = Some([f(0), f(4), f(8)]);
}

// ---- Slice 18: the gear window's charge and modules, on the client ----

/// The view of the gear window the server opened last.
static WINDOW: Mutex<Option<rules::window::View>> = Mutex::new(None);
const PANEL_GAP: f32 = 4.0;
const PANEL_WIDTH: f32 = 168.0;
const LINE: f32 = 11.0;

/// `native_screen_render_post`: beside a gear window's sockets, its charge bar and a line a
/// module (guide `armor`).
pub(super) fn draw_window(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    use crate::game::rustcraft_screens as kit;
    if screen.title.and_then(|t| t.translation_key()) != Some(rules::window::TITLE_KEY) {
        return;
    }
    let Some(v) = WINDOW.lock().unwrap_or_else(PoisonError::into_inner).clone() else { return };
    let o = [screen.rect[0] + screen.rect[2] + PANEL_GAP, screen.rect[1]];
    let height = 38.0 + LINE * v.modules.len() as f32;
    kit::background(game, batch, o, [PANEL_WIDTH, height]);
    kit::label(game, batch, &kit::tr("rustcraft.gear.window.charge", "Charge"), o, [8.0, 7.0]);
    let share = if v.capacity > 0.0 { v.charge / v.capacity } else { 0.0 };
    kit::bar(game, batch, [o[0] + 8.0, o[1] + 17.0, PANEL_WIDTH - 16.0, 5.0], share, kit::ENERGY);
    let amount = kit::tr_with(
        "rustcraft.gear.window.amount",
        "%s of %s E",
        &[rules::window::grouped(v.charge), rules::window::grouped(v.capacity)],
    );
    kit::text(game, batch, &amount, o, [8.0, 25.0], kit::VALUE);
    for (i, (item, st)) in v.modules.iter().enumerate() {
        let name = kit::tr(&format!("item.{}", item.replace(':', ".")), item);
        let key = Module::of(item)
            .and_then(rules::window::key_of)
            .map(|k| rules::window::key_label(&game.native_mod_key("rustcraft", k.name(), k.default_key())));
        let state = match (st, key) {
            (rules::window::State::On, Some(k)) => kit::tr_with("rustcraft.gear.window.on", "on · key %s", &[k]),
            (rules::window::State::Off, Some(k)) => kit::tr_with("rustcraft.gear.window.off", "off · key %s", &[k]),
            _ => kit::tr("rustcraft.gear.window.auto", "works by itself"),
        };
        let line = format!("{name}: {state}");
        let colour = if *st == rules::window::State::Off { kit::TEXT } else { kit::TEXT_ON };
        kit::text(game, batch, &kit::fit(game, &line, PANEL_WIDTH - 16.0), o, [8.0, 38.0 + LINE * i as f32], colour);
    }
}

/// Slice 18: whether this window's player aims the rifle (in the main hand, its use key held).
static AIMING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `native_compute_fov_modifier` on a client: aiming the rifle zooms twice.
pub(super) fn fov(modifier: &mut f32) {
    if AIMING.load(std::sync::atomic::Ordering::Relaxed) {
        *modifier = rules::zoom(*modifier);
    }
}

/// The window's tick: a dash waiting is pushed; whether the rifle is aimed.
pub(super) fn client_tick(game: &mut Game<'_>) {
    let rifle = game.native_held().is_some_and(|(item, _, _)| item == "rustcraft:rifle");
    let aiming = rifle && game.native_playing() && game.native_mapping("key.use").is_some_and(|(down, _)| down);
    AIMING.store(aiming, std::sync::atomic::Ordering::Relaxed);
    let push = PUSH.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(p) = push {
        game.native_push_player(p);
    }
}

// ---- The shield ----

/// `native_living_entity_use_item_start`: an energy shield without a block's charge does not go
/// up (W2); a charged one is noted as held up.
pub(super) fn on_use_start(stack: &ItemStack, entity: Option<i32>, canceled: &mut bool) {
    let Some(g) = Gear::of(&stack.item) else { return };
    if g.kind != Kind::Shield {
        return;
    }
    let Some(id) = id_of(stack) else { return };
    let mut s = state();
    let charge = s.store.held.get(&id).map_or(0.0, |h| h.charge);
    if charge < rules::SHIELD_PER_DAMAGE {
        *canceled = true;
        return;
    }
    if let Some(e) = entity {
        s.blocking.insert(e, (id, g.tier));
    }
}

/// `native_living_entity_use_item_stop` and the end of a use: the shield goes down.
pub(super) fn on_use_stop(entity: Option<i32>) {
    if let Some(e) = entity {
        state().blocking.remove(&e);
    }
}

// ---- Slice 18: the fourth tier's shield sends projectiles back ----

/// `native_hears_projectile_impacts`: the server keeps its flying projectiles for the hook below.
pub(super) fn hears_projectiles(hears: &mut bool) {
    *hears = true;
}

/// How fast a projectile flies back, blocks a tick (an arrow from a full bow: 3).
const BACK_SPEED: f64 = 1.6;

/// `native_projectile_impact`: a projectile meeting a player who holds up a charged fourth-tier
/// energy shield in front of it is cancelled and a new one of its kind flies back at its shooter,
/// shot by the shield's bearer (guide `eshield`).
#[allow(clippy::too_many_arguments)]
pub(super) fn on_projectile_impact(
    game: &mut Game<'_>,
    dimension: &str,
    projectile: u32,
    kind: &str,
    owner: u32,
    entity: Option<u32>,
    entity_kind: &str,
    at: [f64; 3],
    canceled: &mut bool,
) {
    if *canceled || entity_kind != "minecraft:player" || owner == 0 {
        return;
    }
    let Some(target) = entity.and_then(|e| i32::try_from(e).ok()) else { return };
    let Some((_, tier)) = state().blocking.get(&target).copied() else { return };
    let Some(p) = game.native_server_players().into_iter().find(|p| p.entity_id == target) else { return };
    let shooter = game
        .mobs
        .iter()
        .find(|m| m.id == owner)
        .map(|m| [m.position.x, m.position.y + f64::from(m.height()) * 0.6, m.position.z])
        .or_else(|| {
            game.native_server_players()
                .into_iter()
                .find(|q| q.entity_id as u32 == owner)
                .map(|q| [q.position[0], q.position[1] + 1.2, q.position[2]])
        });
    let Some(shooter) = shooter else { return };
    if !rules::shield::reflects(tier, p.yaw, p.position, shooter) {
        return;
    }
    *canceled = true;
    game.native_remove_entity(dimension, projectile);
    let motion = rules::shield::back_motion(at, shooter, BACK_SPEED);
    let start = [at[0] + motion[0] * 0.5, at[1] + motion[1] * 0.5, at[2] + motion[2] * 0.5];
    let words = [(p.uuid >> 96) as u32, (p.uuid >> 64) as u32, (p.uuid >> 32) as u32, p.uuid as u32].map(|w| w as i32);
    let nbt = horizon_world::nbt::Compound::from([
        (JavaString::from("Motion"), Tag::List(motion.iter().map(|v| Tag::Double(v.to_bits())).collect())),
        (JavaString::from("Owner"), Tag::IntArray(words.to_vec())),
        (JavaString::from("pickup"), Tag::Byte(0)),
    ]);
    if game.native_summon(dimension, kind, start, nbt).is_none() {
        log::debug!("rustcraft: a {kind} sent back by a shield did not spawn");
    }
}

// ---- The damage hook ----

/// `native_living_incoming_damage`: the blade, the shock absorber and the damper.
pub(super) fn on_incoming_damage(entity: i32, kind: &str, damage_type: &str, causing: i32, amount: &mut f32, canceled: &mut bool) {
    let mut s = state();
    let now = s.now;
    if causing != 0
        && let Some((id, tier, charged)) = s.views.get(&causing).and_then(|v| v.blade)
        && matches!(damage_type, "minecraft:player_attack")
    {
        if charged {
            *amount += rules::blade_bonus(tier);
            let cost = s.store.held.get(&id).map_or(rules::BLADE_HIT, |h| rules::cost(rules::BLADE_HIT, h));
            s.spent.push((id, cost));
        } else {
            *amount = rules::HAND_DAMAGE;
        }
    }
    if kind != "minecraft:player" {
        return;
    }
    // W8: a hit from someone on a held-up energy shield costs 10 E a point, at least 10 E.
    if causing != 0
        && let Some(&(id, _)) = s.blocking.get(&entity)
    {
        let cost = (f64::from(*amount) * rules::SHIELD_PER_DAMAGE).max(rules::SHIELD_PER_DAMAGE);
        let cost = s.store.held.get(&id).map_or(cost, |h| rules::cost(cost, h));
        s.spent.push((id, cost));
    }
    let view = s.views.get(&entity).copied().unwrap_or_default();
    if damage_type == "minecraft:fall"
        && let Some((id, charge_now)) = view.absorber
    {
        let cost = f64::from(*amount) * rules::ABSORB_PER_DAMAGE;
        if charge_now >= cost {
            s.spent.push((id, cost));
            *canceled = true;
            return;
        }
    }
    if let Some((id, charge_now)) = view.damper
        && s.hurt.get(&entity).is_some_and(|t| now < t + rules::DAMPER_TICKS)
        && charge_now >= rules::DAMPER_COST
    {
        *amount *= rules::DAMPER_SHARE;
        s.spent.push((id, rules::DAMPER_COST));
    }
    s.hurt.insert(entity, now);
}

// ---- The tick ----

fn effect(game: &mut Game<'_>, uuid: u128, id: &str, amplifier: u8) {
    let e = NativeEffect { id: id.to_owned(), amplifier, duration: 260, ambient: true, particles: false };
    game.native_add_effect(uuid, &e);
}

/// Twice a second, for the players of this level: their gear works.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    let now = {
        let mut s = state();
        load(&mut s, game);
        let n = s.ticks.entry(dimension.to_owned()).or_default();
        *n += 1;
        let n = *n;
        s.now = s.now.max(n);
        for (id, cost) in std::mem::take(&mut s.spent) {
            let h = s.store.held.entry(id).or_default();
            h.charge = (h.charge - cost).max(0.0);
            s.dirty = true;
        }
        n
    };
    open_windows(game);
    keys(game);
    if now.is_multiple_of(rules::moves::MAGNET_EVERY) {
        pull(game, dimension);
    }
    if !now.is_multiple_of(EVERY) {
        return;
    }
    for p in game.native_server_players() {
        if p.dimension != dimension {
            continue;
        }
        let worn: Vec<(EquipmentSlot, ItemStack)> = [
            EquipmentSlot::Head,
            EquipmentSlot::Chest,
            EquipmentSlot::Legs,
            EquipmentSlot::Feet,
            EquipmentSlot::MainHand,
        ]
        .into_iter()
        .filter_map(|slot| game.native_player_equipment(p.uuid, slot).map(|st| (slot, st)))
        .collect();
        let compass = worn
            .iter()
            .any(|(slot, st)| *slot == EquipmentSlot::MainHand && *st.item == *crate::game::rustcraft_explore::COMPASS);
        crate::game::rustcraft_explore::player(game, &p, compass);
        player_tick(game, &p, &worn);
    }
    if now.is_multiple_of(200) {
        save(&mut state());
        crate::game::rustcraft_explore::save(game);
        crate::game::rustcraft_utility::save_grown(game);
    }
}

// ---- Slice 18: the magnet's pull and the run burst ----

const RUN_MODIFIER: &str = "rustcraft:pm_run";

/// The run module's burst: +50 % speed as a movement-speed modifier while it holds.
fn run_burst(game: &mut Game<'_>, p: &horizon_client::mod_api::NativeServerPlayer, on: bool) {
    let (had, clean) = {
        let mut s = state();
        (s.running.contains(&p.uuid), s.cleaned.insert(p.uuid))
    };
    if clean && !had {
        game.native_run_command(&format!("attribute {} minecraft:movement_speed modifier remove {RUN_MODIFIER}", p.name));
    }
    if on == had {
        return;
    }
    let name = &p.name;
    if on {
        game.native_run_command(&format!(
            "attribute {name} minecraft:movement_speed modifier add {RUN_MODIFIER} {} add_multiplied_total",
            rules::moves::RUN_BOOST
        ));
        state().running.insert(p.uuid);
    } else {
        game.native_run_command(&format!("attribute {name} minecraft:movement_speed modifier remove {RUN_MODIFIER}"));
        state().running.remove(&p.uuid);
    }
}

/// Each `MAGNET_EVERY` ticks: the items in a working magnet's reach come a step nearer its
/// player; the game's own pickup takes them when they arrive.
fn pull(game: &mut Game<'_>, dimension: &str) {
    if state().magnets.is_empty() {
        return;
    }
    for p in game.native_server_players() {
        if p.dimension != dimension || !state().magnets.contains(&p.uuid) {
            continue;
        }
        let to = [p.position[0], p.position[1] + 0.5, p.position[2]];
        game.native_run_command(&rules::moves::pull_command(dimension, to, rules::MAGNET_REACH));
    }
}

// ---- Slice 18: the gear's keys ----

/// The keys pressed since the last tick (rustcraft_gear_keys): the magnet and night vision switch
/// in the worn piece holding them, the backpack key opens the first backpack carried.
fn keys(game: &mut Game<'_>) {
    for (uuid, key) in crate::game::rustcraft_gear_keys::take() {
        let Some(m) = key.module() else {
            crate::game::rustcraft_backpack::open_by_key(game, uuid);
            continue;
        };
        let slot = if m.piece() == Piece::Helmet { EquipmentSlot::Head } else { EquipmentSlot::Chest };
        let on = game.native_player_equipment(uuid, slot).and_then(|st| {
            let mut s = state();
            load(&mut s, game);
            let id = id_of(&st)?;
            Gear::of(&st.item)?;
            let h = s.store.held.get_mut(&id)?;
            if !h.has(m) {
                return None;
            }
            let on = rules::keys::toggle(h, m);
            s.dirty = true;
            Some(on)
        });
        let magnet = m == Module::Magnet;
        let said = match (on, magnet) {
            (Some(true), true) => line("rustcraft.gear.magnet_on", "Magnet on", &[]),
            (Some(false), true) => line("rustcraft.gear.magnet_off", "Magnet off", &[]),
            (Some(true), false) => line("rustcraft.gear.night_on", "Night vision on", &[]),
            (Some(false), false) => line("rustcraft.gear.night_off", "Night vision off", &[]),
            (None, true) => line("rustcraft.gear.no_magnet", "No magnet module in the chestplate", &[]),
            (None, false) => line("rustcraft.gear.no_night", "No night vision module in the helmet", &[]),
        };
        if m == Module::Night && on == Some(false) {
            game.native_remove_effect(uuid, "minecraft:night_vision");
        }
        game.native_action_bar(uuid, said);
    }
}

fn player_tick(game: &mut Game<'_>, p: &horizon_client::mod_api::NativeServerPlayer, worn: &[(EquipmentSlot, ItemStack)]) {
    let pieces: Vec<(EquipmentSlot, u64, Gear, Held)> = {
        let s = state();
        worn.iter()
            .filter_map(|(slot, st)| held_of(&s, st).map(|(id, g, h)| (*slot, id, g, h)))
            .collect()
    };
    let suit = pieces.iter().any(|(slot, _, g, h)| {
        *slot == EquipmentSlot::Chest && matches!(g.kind, Kind::Armor(Piece::Chestplate)) && h.has(Module::Capacity)
    });
    let mut view = View::default();
    let mut wants_flight = false;
    let (mut wants_run, mut wants_magnet) = (false, false);
    for (slot, id, g, h) in &pieces {
        let h = h.clone();
        if matches!(g.kind, Kind::Armor(_)) && h.suit != suit {
            let mut s = state();
            s.store.held.entry(*id).or_default().suit = suit;
            s.dirty = true;
        }
        if *slot == EquipmentSlot::MainHand {
            if g.kind == Kind::Blade {
                view.blade = Some((*id, g.tier, h.charge >= rules::cost(rules::BLADE_HIT, &h)));
            }
            continue;
        }
        let Kind::Armor(_) = g.kind else { continue };
        let mut drain = 0.0;
        for m in Module::ALL.into_iter().filter(|m| h.has(*m)) {
            let need = rules::cost(m.drain(), &h) * SECONDS;
            if h.charge < need.max(1e-9) && m.drain() > 0.0 {
                continue;
            }
            match m {
                Module::Night | Module::Magnet if !rules::keys::works(&h, m) => {}
                Module::Night => {
                    effect(game, p.uuid, "minecraft:night_vision", 0);
                    drain += need;
                }
                Module::Jump => {
                    effect(game, p.uuid, "minecraft:jump_boost", 1);
                    drain += need;
                }
                Module::Run => {
                    let s = state();
                    if rules::moves::run_boost(s.sprint.get(&p.uuid).copied(), s.now) {
                        wants_run = true;
                        drain += need;
                    }
                }
                Module::Magnet => {
                    wants_magnet = true;
                    drain += need;
                }
                Module::Jetpack => {
                    wants_flight = true;
                    if p.flying {
                        drain += need;
                    }
                }
                Module::Absorber => view.absorber = Some((*id, h.charge)),
                Module::Damper => view.damper = Some((*id, h.charge)),
                Module::Dash | Module::Capacity => {}
            }
        }
        if drain > 0.0 {
            let mut s = state();
            let e = s.store.held.entry(*id).or_default();
            e.charge = (e.charge - drain).max(0.0);
            s.dirty = true;
        }
    }
    state().views.insert(p.entity_id, view);
    run_burst(game, p, wants_run);
    if wants_magnet {
        state().magnets.insert(p.uuid);
    } else {
        state().magnets.remove(&p.uuid);
    }
    // The jetpack: flight while the chestplate holds charge, outside creative and spectator.
    let survival = matches!(p.game_mode, 0 | 2);
    let had = state().flying.contains(&p.uuid);
    if survival && wants_flight && !had {
        game.native_set_flight(p.uuid, true, false);
        game.native_set_flying_speed(p.uuid, Some(0.03));
        state().flying.insert(p.uuid);
    } else if had && (!wants_flight || !survival) {
        game.native_set_flight(p.uuid, false, false);
        game.native_set_flying_speed(p.uuid, None);
        state().flying.remove(&p.uuid);
    }
}

/// The windows asked for open: a piece's modules and books.
fn open_windows(game: &mut Game<'_>) {
    let opening = std::mem::take(&mut state().opening);
    for (uuid, id, g) in opening {
        let items: Vec<Option<ItemStack>> = {
            let s = state();
            if s.open.values().any(|(_, open, _)| *open == id) {
                continue;
            }
            let h = s.store.held.get(&id).cloned().unwrap_or_default();
            let mut items: Vec<Option<ItemStack>> = (0..g.sockets())
                .map(|i| h.modules.get(i).map(|m| ItemStack::new(m, 1)))
                .collect();
            items.push((h.capacity > 0).then(|| ItemStack::new(rules::BOOK_CAPACITY, u32::from(h.capacity))));
            items.push((h.thrift > 0).then(|| ItemStack::new(rules::BOOK_THRIFT, u32::from(h.thrift))));
            items.resize(9, None);
            items
        };
        let title = line(
            "rustcraft.gear.window",
            "Modules: %s · books: Capacity, Thrift",
            &[Text::literal(g.sockets().to_string())],
        );
        let used = rules::window_slots(g);
        if let Some(window) = game.native_open_menu_with_input(uuid, 1, title, items, 0..used) {
            state().open.insert(uuid, (window, id, g));
            // Slice 18: the charge and the modules' lines, drawn beside the sockets.
            let view = rules::window::view(g, &state().store.held.get(&id).cloned().unwrap_or_default());
            match serde_json::to_vec(&view) {
                Ok(bytes) => {
                    if let Err(e) = game.native_net_send_to_player(uuid, rules::window::CHANNEL, &bytes) {
                        log::warn!("rustcraft: the gear window's view was not sent: {e}");
                    }
                }
                Err(e) => log::warn!("rustcraft: the gear window's view does not write: {e}"),
            }
        }
    }
}

/// What the self-test reads: a piece's held data.
pub(super) fn held(stack: &ItemStack) -> Option<Held> {
    let s = state();
    held_of(&s, stack).map(|(_, _, h)| h)
}

/// What the self-test sets: a piece's held data.
pub(super) fn set_held(stack: &ItemStack, h: Held) {
    let mut s = state();
    if let Some(id) = id_of(stack) {
        s.store.held.insert(id, h);
        s.dirty = true;
    }
}

/// The self-test gives a piece a number of its own.
pub(super) fn number(stack: &mut ItemStack) {
    let id = {
        let mut s = state();
        s.store.next += 1;
        s.store.next
    };
    set_id(stack, id);
}
