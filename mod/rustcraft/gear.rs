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
//   night vision, jump boost and speed as the game's effects, the magnet picks up what lies within
//   6 blocks, the jetpack lets the player fly while the chestplate holds charge
//   (`native_set_flight`). What the damage hook needs is noted for it (it runs inside the hit,
//   with no game): the blade in hand, the damper, the shock absorber.
// - The damage hook: a charged blade strikes and pays 25 E (the fourth tier one point harder), an
//   empty one strikes as a hand; the shock absorber takes fall damage for 25 E a point; the damper
//   softens hits for 5 s after one by 20 %. Their spending is applied at the next tick.
// - The rifle: a right click shoots straight along the look, 64 blocks, the first creature on the
//   line, stopped by a solid block; 16 damage of the arrow's type from the player, so the base
//   game and Path-of-craft-tree count it as a bow's; 400 E, a shot every 1.25 s.
// - Chargers: `battery` and `charge` give the charger and the plate a piece's battery.
use crate::game::{
    Game,
    native_player_state::NativeEffect,
    rustcraft_server::{line, tell},
};
use horizon_client::{items::ItemStack, text::Text};
use horizon_world::{
    nbt::{JavaString, Tag},
    player_inventory::EquipmentSlot,
};
use rustcraft_rules::{
    gear::{self as rules, Gear, Held, Kind, Module, Piece, shot},
    ray,
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
}

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
    let cap = rules::capacity(g, &h, false);
    Some(Battery { charge: h.charge.min(cap), capacity: cap })
}

/// Sets a piece's charge.
pub(super) fn charge(stack: &ItemStack, charge_now: f64) {
    let mut s = state();
    let Some(id) = id_of(stack) else { return };
    s.store.held.entry(id).or_default().charge = charge_now.max(0.0);
    s.dirty = true;
}

// ---- Hooks ----

/// `native_inventory_tick`: a gear piece without a number gets one.
pub(super) fn on_inventory_tick(game: &mut Game<'_>, slot: i32) {
    let needs = game
        .native_carried_stack(slot)
        .is_some_and(|s| Gear::of(&s.item).is_some() && id_of(s).is_none());
    if !needs {
        return;
    }
    let id = {
        let mut s = state();
        load(&mut s, game);
        s.store.next += 1;
        let id = s.store.next;
        s.store.held.insert(id, Held::default());
        s.dirty = true;
        save(&mut s);
        id
    };
    if let Some(stack) = game.native_carried_stack(slot) {
        set_id(stack, id);
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
        .find(|p| {
            let b = game.native_block(*p);
            !matches!(b.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:water" | "minecraft:short_grass" | "minecraft:tall_grass")
        })
        .map_or(f64::INFINITY, |p| {
            let c = p.map(|v| f64::from(v) + 0.5);
            ((c[0] - eye[0]).powi(2) + (c[1] - eye[1]).powi(2) + (c[2] - eye[2]).powi(2)).sqrt() - 0.5
        });
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
    if let Some(target) = shot::hit(eye, dir, &targets, wall, rules::RIFLE_RANGE) {
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
    for (slot, id, g, h) in &pieces {
        let cap = rules::capacity(*g, h, suit);
        let mut h = h.clone();
        h.charge = h.charge.min(cap);
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
                Module::Night => {
                    effect(game, p.uuid, "minecraft:night_vision", 0);
                    drain += need;
                }
                Module::Jump => {
                    effect(game, p.uuid, "minecraft:jump_boost", 1);
                    drain += need;
                }
                Module::Run => {
                    effect(game, p.uuid, "minecraft:speed", 1);
                    drain += need;
                }
                Module::Magnet => {
                    if magnet(game, p) {
                        drain += need;
                    }
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
            e.charge = (e.charge.min(cap) - drain).max(0.0);
            s.dirty = true;
        }
    }
    state().views.insert(p.entity_id, view);
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

/// The magnet: what lies within reach goes into the player's inventory. True when it took any.
fn magnet(game: &mut Game<'_>, p: &horizon_client::mod_api::NativeServerPlayer) -> bool {
    let r = rules::MAGNET_REACH;
    let at = p.position;
    let items = game.native_item_entities(&p.dimension, [at[0] - r, at[1] - r, at[2] - r], [at[0] + r, at[1] + r, at[2] + r]);
    let mut took = false;
    for e in items {
        if e.pickup_delay > 0 {
            continue;
        }
        let left = game.native_give_stack(p.uuid, e.stack.clone());
        let left_count = left.as_ref().map_or(0, |l| l.count);
        if left_count == e.stack.count {
            continue;
        }
        let mut rest = e.stack.clone();
        rest.count = left_count;
        game.native_set_item_entity(&p.dimension, e.id, rest);
        took = true;
    }
    took
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
