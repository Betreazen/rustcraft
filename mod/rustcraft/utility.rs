// Slice 12: the last blocks of the guide on the server (rustcraft_rules::utility decides). Each is
// a machine with a buffer of energy it fills from the net (rustcraft_machines, as the charge dock
// and the turret), a sheet window and slots; every `step_ticks` (shorter with speed modules) it
// does one step of its work if its buffer holds the step's energy.
//
// - The trash (UT1–UT7): burns what comes into its intake slots, which take only what its filter
//   slots name, 8 items a second (×2 a speed module).
// - The disintegrator (DS1–DS7): one crafted item every 5 s into a share of its crafting
//   recipe's ingredients (the game's own recipes), fractions kept in its block entity.
// - The charger and the charge plate: charged items (custom data `rustcraft:charge` under
//   `rustcraft:charge_max`) fill from the buffer. The plate's players are found but no worn item is
//   charged yet: the mod's charged gear is not made.
// - The repair stand: a quarter of an item's durability back for a unit of its material.
// - The kitchen: the food recipes its toggles turn on, from the game's crafting recipes.
// - The brewer: the game's brewing recipes, bottle by bottle, a blaze powder for 20 brews.
// - The experience condenser: its toggles give or take a level of the pressing player.
// - The enchanting machine: an item and lapis with the experience of a condenser beside it, as
//   the enchanting table with 15 bookshelves offers it, the level its toggles choose.
// - The archaeology table: a suspicious block and a brush into a find of the table its toggle
//   chooses (spec 24, ST2), and the sand or gravel back; `brush` to Path-of-craft-tree only for a
//   block found in the world (ST5).
// - The living vein: ore, or with the stratum module suspicious sand or gravel, in a free cell
//   beside it, told as `place` (X6). A suspicious block it grows is remembered by its cell
//   (rustcraft_rules::utility::grown, the world's `data/rustcraft_grown.json`) until a player
//   breaks or places there; the grab (rustcraft_workers) takes it whole and its item carries custom
//   data `rustcraft:grown`, so the table sends no `brush` for it (ST4, ST5).
// - Spec 26: the wrench with sneaking lifts suspicious sand or gravel out of the world
//   (rustcraft_rules::utility::found). Its item carries custom data `rustcraft:found`: the
//   dimension, the cell, and the loot table and seed its brushable block entity held (or the find
//   a brush stroke already rolled), read from the level's block entities (`BlockEntity::Brushable`,
//   SRC/crates/horizon-client-render/src/chunk/block_entity.rs:324-332). Placed again it gets that
//   loot back; the archaeology table brushes it with that loot table and tells `brush`, as for any
//   block found in the world. A grown block lifted stays grown, and placed again its cell is grown.
// The farm machines, the projector and builder and the portal dock are rustcraft_farm and
// rustcraft_builder; this module runs their ticks too.
use crate::game::{
    Game, rustcraft_anim as anim, rustcraft_base as base, rustcraft_builder as builder,
    rustcraft_content::Machine, rustcraft_farm as farm, rustcraft_machines as machines,
    rustcraft_mining as mining,
    rustcraft_server::{LevelNet, line, tell},
};
use horizon_client::{
    containers::{BlockEntity, ModBlockEntity},
    items::ItemStack,
    save,
    text::Text,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos},
    machines::{
        sheet::{Bar, Kind as SheetKind, Press, Sheet, Status as SheetStatus, Toggle, severity_code},
        style::Severity,
    },
    modules::{self, Module},
    reports::Report,
    tuning::Tuning,
    utility::{
        self as rules, ARCHEO_TABLES, KITCHEN_RECIPES, Shown, Util, charge, disassembly,
        found::{self, Found, Lift},
        grown::{self, Grown},
        repair, xp,
    },
};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

pub(super) const CUSTOM_DATA: &str = "minecraft:custom_data";
const TOGGLES_KEY: &str = "util_toggles";
const KEPT_KEY: &str = "util_kept";
const XP_KEY: &str = "util_xp";
const FUEL_KEY: &str = "util_fuel";
/// The brewer's brews a blaze powder gives, as the brewing stand's.
const BREWS_PER_POWDER: i32 = 20;

#[derive(Default)]
struct State {
    ticks: HashMap<String, u64>,
    shown: HashMap<(String, Pos), Shown>,
    seed: u64,
    /// The game's crafting recipes as the disintegrator reads them, made at the first need.
    recipes: Option<Vec<disassembly::Recipe>>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// ST5: the cells the vein grew a suspicious block in, as the world's file keeps them.
#[derive(Default)]
struct GrownFile {
    path: Option<PathBuf>,
    cells: Grown,
    dirty: bool,
}

static GROWN: LazyLock<Mutex<GrownFile>> = LazyLock::new(Mutex::default);

/// The grown cells, read from the world's file the first time.
fn grown_cells(game: &Game<'_>) -> MutexGuard<'static, GrownFile> {
    let mut g = GROWN.lock().unwrap_or_else(PoisonError::into_inner);
    if g.path.is_none()
        && let Some(world) = game.native_world_dir()
    {
        let path = world.join("data").join("rustcraft_grown.json");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match serde_json::from_str(&text) {
                Ok(cells) => g.cells = cells,
                Err(e) => log::warn!("rustcraft: {} does not read: {e}", path.display()),
            }
        }
        g.path = Some(path);
    }
    g
}

/// Writes the grown cells when they changed, whole under another name first.
pub(super) fn save_grown(game: &Game<'_>) {
    let mut g = grown_cells(game);
    if !g.dirty {
        return;
    }
    let Some(path) = g.path.clone() else { return };
    let Ok(json) = serde_json::to_string(&g.cells) else {
        return;
    };
    g.dirty = false;
    drop(g);
    let fresh = path.with_extension("json.tmp");
    let done = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&fresh, json))
        .and_then(|()| std::fs::rename(&fresh, &path));
    if let Err(e) = done {
        log::warn!("rustcraft: the grown cells were not saved: {e}");
    }
}

/// ST5: whether the vein grew the block at `pos`.
pub(super) fn grown_at(game: &Game<'_>, dimension: &str, pos: Pos) -> bool {
    grown_cells(game).cells.contains(dimension, pos)
}

/// The block at `pos` changed or was taken: it is no grown block any more.
pub(super) fn forget_grown(game: &Game<'_>, dimension: &str, pos: Pos) {
    let mut g = grown_cells(game);
    if g.cells.forget(dimension, pos) {
        g.dirty = true;
    }
}

/// Whether `stack` is a block the vein grew (custom data `rustcraft:grown`).
pub(super) fn is_grown(stack: &ItemStack) -> bool {
    stack
        .custom_data()
        .is_some_and(|d| d.get(&JavaString::from(grown::MARK)).is_some())
}

/// A stack of the item a worker's hold names `name`: a grown block's with its mark.
pub(super) fn hold_stack(name: &str, count: u32) -> ItemStack {
    let (item, marked) = grown::from_hold(name);
    let mut stack = ItemStack::new(item, count);
    if marked {
        let mut data = stack.custom_data().cloned().unwrap_or_default();
        data.insert(JavaString::from(grown::MARK), Tag::Byte(1));
        stack
            .data_mut()
            .set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
    }
    stack
}

/// The name a worker's hold keeps `stack` under: its item, marked when it was grown.
pub(super) fn hold_name(stack: &ItemStack) -> String {
    grown::hold_name(&stack.item, is_grown(stack))
}

/// A random number 0..1 (xorshift; the machines' chances need no better).
pub(super) fn random() -> f64 {
    let mut s = state();
    if s.seed == 0 {
        s.seed = 0x9E37_79B9_7F4A_7C15
            ^ std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64);
    }
    let mut x = s.seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    s.seed = x;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// The window's title in English, the content mod's language files name it for each client.
pub(super) fn title(u: Util) -> &'static str {
    match u {
        Util::Trash => "Trash",
        Util::Disintegrator => "Disintegrator",
        Util::Charger => "Charger",
        Util::ChargePlate => "Charge Plate",
        Util::Repair => "Repair Stand",
        Util::Kitchen => "Kitchen",
        Util::Fishery => "Fishing Station",
        Util::Feeder => "Feeder",
        Util::Shearer => "Shearer",
        Util::Apiary => "Apiary Collector",
        Util::Brewer => "Brewing Vat",
        Util::XpTank => "Experience Condenser",
        Util::Enchanter => "Enchanting Machine",
        Util::Archeo => "Archaeology Table",
        Util::Projector => "Holo-Projector",
        Util::Builder => "Building Machine",
        Util::PortalDock => "Portal Dock",
        Util::Vein => "Living Vein",
    }
}

// ---- Helpers the three modules share ----

pub(super) fn items(game: &Game<'_>, pos: Pos) -> Vec<Option<ItemStack>> {
    game.native_mod_block_entity(pos).map(|e| e.items.clone()).unwrap_or_default()
}

pub(super) fn with_items<R>(game: &mut Game<'_>, pos: Pos, f: impl FnOnce(&mut Vec<Option<ItemStack>>) -> R) -> Option<R> {
    game.native_with_mod_block_entity(pos, |e| f(&mut e.items))
}

/// Takes one from slot `slot`.
pub(super) fn take_one(slots: &mut [Option<ItemStack>], slot: usize) {
    if let Some(Some(s)) = slots.get_mut(slot) {
        s.count = s.count.saturating_sub(1);
        if s.count == 0 {
            slots[slot] = None;
        }
    }
}

/// Puts `stack` into the slots of `range`; what does not fit comes back.
pub(super) fn put(game: &mut Game<'_>, pos: Pos, range: std::ops::Range<usize>, stack: ItemStack) -> Option<ItemStack> {
    let max = machines::max_stack(game, &stack.item);
    with_items(game, pos, move |items| {
        if items.len() < range.end {
            items.resize(range.end, None);
        }
        mining::store(&mut items[range], stack, max)
    })
    .flatten()
}

/// Whether all of `stacks` fit into the slots of `range` now.
pub(super) fn fits(game: &Game<'_>, pos: Pos, range: std::ops::Range<usize>, stacks: &[ItemStack]) -> bool {
    let mut slots = items(game, pos);
    if slots.len() < range.end {
        slots.resize(range.end, None);
    }
    let mut part = slots[range].to_vec();
    stacks.iter().all(|s| {
        let max = machines::max_stack(game, &s.item);
        mining::store(&mut part, s.clone(), max).is_none()
    })
}

pub(super) fn energy(game: &Game<'_>, pos: Pos) -> f64 {
    machines::buffer_energy(game, pos)
}

/// Spends `cost` E of the buffer; false, and nothing spent, when it holds less.
pub(super) fn spend(game: &mut Game<'_>, pos: Pos, cost: f64) -> bool {
    let e = energy(game, pos);
    if e + 1e-9 < cost {
        return false;
    }
    machines::set_buffer_energy(game, pos, e - cost)
}

/// (ticks between steps, energy a step) after the modules: speed ×2 work for ×2.5 energy, economy
/// ×0.85 work for ×0.6 energy, each in full (A7, A9; `rustcraft_rules::modules::factors`).
pub(super) fn paced(game: &Game<'_>, pos: Pos, u: Util) -> (u64, f64) {
    let slots = base::modules_at(game, pos);
    let t = crate::game::rustcraft_server::tuning();
    let ticks = modules::paced_ticks(rules::step_ticks(u), &slots, t);
    let cost = rules::step_energy(u) * modules::factors(&slots, t).1;
    (ticks, cost)
}

pub(super) fn show(game: &mut Game<'_>, dimension: &str, pos: Pos, shown: Shown) {
    anim::show_on(game, pos, shown.working());
    state().shown.insert((dimension.to_owned(), pos), shown);
}

pub(super) fn shown_at(dimension: &str, pos: Pos) -> Option<Shown> {
    state().shown.get(&(dimension.to_owned(), pos)).cloned()
}

fn toggles(nbt: &Compound) -> Vec<bool> {
    match nbt.get(&JavaString::from(TOGGLES_KEY)) {
        Some(Tag::ByteArray(b)) => b.iter().map(|v| *v != 0).collect(),
        _ => Vec::new(),
    }
}

fn write_toggles(nbt: &mut Compound, t: &[bool]) {
    nbt.insert(
        JavaString::from(TOGGLES_KEY),
        Tag::ByteArray(t.iter().map(|v| u8::from(*v)).collect()),
    );
}

/// The toggles of the block at `pos`, `n` of them, all off when none were pressed.
pub(super) fn toggles_at(game: &Game<'_>, pos: Pos, n: usize) -> Vec<bool> {
    let mut t = game.native_mod_block_entity(pos).map(|e| toggles(&e.nbt)).unwrap_or_default();
    t.resize(n, false);
    t
}

/// The one toggle on of a choice (the archaeology table's table, the enchanting level): the
/// first by default.
fn choice(game: &Game<'_>, pos: Pos, n: usize) -> usize {
    toggles_at(game, pos, n).iter().position(|t| *t).unwrap_or(0)
}

pub(super) fn read_i64(nbt: &Compound, key: &str) -> i64 {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        _ => 0,
    }
}

pub(super) fn write_i64(nbt: &mut Compound, key: &str, v: i64) {
    nbt.insert(JavaString::from(key), Tag::Long(v));
}

pub(super) fn read_json<T: serde::de::DeserializeOwned + Default>(nbt: &Compound, key: &str) -> T {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::String(s)) => serde_json::from_str(&s.to_string_lossy()).unwrap_or_default(),
        _ => T::default(),
    }
}

pub(super) fn write_json<T: serde::Serialize>(nbt: &mut Compound, key: &str, v: &T) {
    let json = serde_json::to_string(v).unwrap_or_default();
    nbt.insert(JavaString::from(key), Tag::String(JavaString::from(json.as_str())));
}

/// The block entity of the block at `pos` beside `at`, on a face.
pub(super) fn neighbour_of(game: &Game<'_>, at: Pos, block: &str) -> Option<Pos> {
    Face::ALL
        .iter()
        .map(|f| {
            let o = f.offset();
            [at[0] + o[0], at[1] + o[1], at[2] + o[2]]
        })
        .find(|p| game.native_block(*p) == block)
}

fn stack(item: &str, count: u32) -> ItemStack {
    ItemStack::new(item, count)
}

// ---- The tick ----

/// Each slice-12 machine of the level.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &LevelNet, t: &Tuning) {
    let mine: Vec<(Pos, Util)> = level
        .machines
        .iter()
        .filter_map(|(p, m)| match m {
            Machine::Util(u) => Some((*p, *u)),
            _ => None,
        })
        .collect();
    if mine.is_empty() {
        return;
    }
    let now = {
        let mut s = state();
        let n = s.ticks.entry(dimension.to_owned()).or_default();
        *n += 1;
        *n
    };
    for (pos, u) in mine {
        let (every, cost) = paced(game, pos, u);
        if !now.is_multiple_of(every) {
            continue;
        }
        let shown = match u {
            Util::Trash => trash(game, pos, t),
            Util::Disintegrator => disintegrate(game, pos, cost),
            Util::Charger => charge_slots(game, pos, every),
            Util::ChargePlate => plate(game, dimension, pos),
            Util::Repair => repair_one(game, pos, cost),
            Util::Kitchen => cook(game, pos, cost),
            Util::Brewer => brew(game, pos, cost),
            Util::XpTank => xp_tank_shown(game, pos),
            Util::Enchanter => enchant(game, pos, cost),
            Util::Archeo => brush(game, pos, cost),
            Util::Fishery | Util::Feeder | Util::Shearer | Util::Apiary => {
                farm::step(game, dimension, pos, u, cost, now, t)
            }
            Util::Projector | Util::Builder | Util::PortalDock => {
                builder::step(game, dimension, pos, u, cost, now)
            }
            Util::Vein => grow(game, dimension, pos, cost),
        };
        show(game, dimension, pos, shown);
    }
    builder::save_transit(game);
}

fn owner(game: &Game<'_>, pos: Pos) -> Option<u128> {
    base::owner_at(game, pos)
}

// ---- The trash ----

fn trash(game: &mut Game<'_>, pos: Pos, _t: &Tuning) -> Shown {
    let slots = items(game, pos);
    if slots.iter().take(9).all(|s| s.as_ref().is_none_or(|s| s.count == 0)) {
        return Shown::Blocked("trash_no_filter");
    }
    let speed = modules::count(&base::modules_at(game, pos), Module::Speed);
    let mut batch = 8u32 << speed.min(3);
    let per = paced(game, pos, Util::Trash).1;
    let filter: Vec<&str> = slots.iter().take(9).flatten().map(|s| &*s.item).collect();
    let mut burned = 0;
    for slot in 9..18 {
        let Some(Some(s)) = slots.get(slot) else { continue };
        if !filter.contains(&&*s.item) {
            continue;
        }
        let n = s.count.min(batch);
        if n == 0 || !spend(game, pos, per * f64::from(n)) {
            break;
        }
        with_items(game, pos, |items| {
            if let Some(Some(s)) = items.get_mut(slot) {
                s.count -= n.min(s.count);
                if s.count == 0 {
                    items[slot] = None;
                }
            }
        });
        burned += n;
        batch -= n;
        if batch == 0 {
            break;
        }
    }
    if burned > 0 {
        Shown::Working
    } else if slots.iter().skip(9).any(|s| s.is_some()) {
        Shown::NoEnergy
    } else {
        Shown::Idle("trash_empty")
    }
}

// ---- The disintegrator ----

fn crafting_recipes(game: &Game<'_>) -> Vec<disassembly::Recipe> {
    use horizon_client::crafting::Recipe;
    let first = |i: &horizon_client::crafting::Ingredient| -> Option<String> {
        i.order.first().cloned().or_else(|| i.items.iter().min().cloned())
    };
    let mut out = Vec::new();
    for (_, recipe) in &game.recipes().crafting {
        let (cells, result): (Vec<Option<String>>, _) = match recipe {
            Recipe::Shaped { cells, result, .. } => (cells.iter().map(|c| c.as_ref().and_then(first)).collect(), result),
            Recipe::Shapeless { ingredients, result } => (ingredients.iter().map(|i| first(i)).collect(), result),
            Recipe::Special(_) => continue,
        };
        let Some(item) = result.item.as_ref() else { continue };
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for c in cells.into_iter().flatten() {
            *counts.entry(c).or_default() += 1;
        }
        if counts.is_empty() {
            continue;
        }
        out.push(disassembly::Recipe {
            result: item.to_string(),
            result_count: result.count.max(1),
            ingredients: counts.into_iter().collect(),
        });
    }
    out
}

fn charged(game: &Game<'_>, s: &ItemStack) -> bool {
    battery_of(game, s).is_some_and(|b| b.charge > 0.0)
}

fn disintegrate(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let Some(input) = items(game, pos).first().cloned().flatten() else {
        return Shown::Idle("dis_empty");
    };
    if state().recipes.is_none() {
        let r = crafting_recipes(game);
        state().recipes = Some(r);
    }
    let is_charged = charged(game, &input);
    let recipe = {
        let s = state();
        let all = s.recipes.as_deref().unwrap_or_default();
        disassembly::recipe_for(&input.item, all, is_charged).cloned()
    };
    let Ok(recipe) = recipe else {
        return Shown::Blocked("dis_refused");
    };
    let max_damage = game.items().get(&input.item).map_or(0, |i| i.max_damage);
    let durability = if max_damage > 0 {
        f64::from(max_damage.saturating_sub(input.damage)) / f64::from(max_damage)
    } else {
        1.0
    };
    let percent = disassembly::share_percent(modules::count(&base::modules_at(game, pos), Module::Yield) as usize);
    let entity = game.native_mod_block_entity(pos);
    let mut kept: BTreeMap<String, u32> = entity.as_ref().map(|e| read_json(&e.nbt, KEPT_KEY)).unwrap_or_default();
    let out = disassembly::take_apart(&recipe, percent, durability, &mut kept);
    let stacks: Vec<ItemStack> = out.iter().map(|(i, n)| stack(i, *n)).collect();
    if !fits(game, pos, 1..10, &stacks) {
        return Shown::Blocked("output_full");
    }
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    game.native_with_mod_block_entity(pos, |e| {
        take_one(&mut e.items, 0);
        write_json(&mut e.nbt, KEPT_KEY, &kept);
    });
    for s in stacks {
        put(game, pos, 1..10, s);
    }
    Shown::Working
}

// ---- The charger and the plate ----

// Slice 13: the batteries are the gear's (rustcraft_gear keeps them under each piece's number).
fn battery_of(game: &Game<'_>, s: &ItemStack) -> Option<charge::Battery> {
    crate::game::rustcraft_gear::battery(game, s)
}

fn charge_slots(game: &mut Game<'_>, pos: Pos, every: u64) -> Shown {
    let slots = items(game, pos);
    let held: Vec<(usize, charge::Battery)> = (0..4)
        .filter_map(|i| slots.get(i).cloned().flatten().and_then(|s| battery_of(game, &s).map(|b| (i, b))))
        .collect();
    if held.is_empty() {
        return Shown::Idle("charge_empty");
    }
    let offer = energy(game, pos).min(Util::Charger.spec().draw * every as f64 / 20.0);
    let batteries: Vec<_> = held.iter().map(|(_, b)| *b).collect();
    let (after, used) = charge::share(&batteries, offer);
    if used <= 0.0 && held.iter().any(|(_, b)| !b.full()) {
        return Shown::NoEnergy;
    }
    spend(game, pos, used);
    for ((slot, _), b) in held.iter().zip(&after) {
        if let Some(s) = slots.get(*slot).cloned().flatten() {
            crate::game::rustcraft_gear::charge(&s, b.charge);
        }
    }
    // Full ones go to the output slots, for the net.
    for ((slot, _), b) in held.iter().zip(&after) {
        if b.full()
            && let Some(s) = items(game, pos).get(*slot).cloned().flatten()
            && put(game, pos, 4..8, s).is_none()
        {
            with_items(game, pos, |items| items[*slot] = None);
        }
    }
    Shown::Working
}

fn plate(game: &mut Game<'_>, dimension: &str, pos: Pos) -> Shown {
    let below = [pos[0], pos[1] - 1, pos[2]];
    if game.native_block(below) != crate::game::rustcraft_content::HOUSING {
        return Shown::Blocked("plate_no_housing");
    }
    let standing: Vec<u128> = game
        .native_server_players()
        .into_iter()
        .filter(|p| {
            p.dimension == dimension
                && p.position[0].floor() as i32 == pos[0]
                && p.position[2].floor() as i32 == pos[2]
                && (p.position[1] - f64::from(pos[1])).abs() < 1.2
        })
        .map(|p| p.uuid)
        .collect();
    if standing.is_empty() {
        return Shown::Idle("plate_empty");
    }
    // W3: worn armour and both hands, each in turn.
    use horizon_world::player_inventory::EquipmentSlot as E;
    let mut offer = energy(game, pos).min(Util::ChargePlate.spec().draw);
    let mut used = 0.0;
    for uuid in standing {
        for slot in [E::Head, E::Chest, E::Legs, E::Feet, E::MainHand, E::OffHand] {
            let Some(st) = game.native_player_equipment(uuid, slot) else { continue };
            let Some(b) = battery_of(game, &st) else { continue };
            let (after, took) = b.fill(offer);
            if took > 0.0 {
                crate::game::rustcraft_gear::charge(&st, after.charge);
                offer -= took;
                used += took;
            }
        }
    }
    spend(game, pos, used);
    if used > 0.0 { Shown::Working } else { Shown::Idle("plate_full") }
}

// ---- The repair stand ----

fn repair_one(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let slots = items(game, pos);
    let Some(tool) = slots.first().cloned().flatten() else {
        return Shown::Idle("repair_empty");
    };
    let max = game.items().get(&tool.item).map_or(0, |i| i.max_damage);
    if max == 0 || tool.damage == 0 {
        // Whole: to the output slot.
        if put(game, pos, 5..6, tool).is_none() {
            with_items(game, pos, |items| items[0] = None);
            return Shown::Working;
        }
        return Shown::Blocked("output_full");
    }
    let Some(material) = repair::material_of(&tool.item) else {
        return Shown::Blocked("repair_no_material");
    };
    let Some(slot) = (1..5).find(|i| slots.get(*i).cloned().flatten().is_some_and(|s| material.contains(&&*s.item))) else {
        return Shown::Blocked("repair_no_material");
    };
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    let damage = repair::repaired(tool.damage, max);
    with_items(game, pos, |items| {
        take_one(items, slot);
        if let Some(Some(t)) = items.get_mut(0) {
            t.damage = damage;
        }
    });
    if let Some(o) = owner(game, pos) {
        base::report(Report::Counted { kind: "repair", owner: o, count: 1 });
    }
    Shown::Working
}

// ---- The kitchen ----

/// The game's crafting recipe for `result`: (its id, the ingredients it needs, the stack made).
fn food_recipe(game: &Game<'_>, result: &str) -> Option<(String, Vec<(String, u32)>, ItemStack)> {
    use horizon_client::crafting::Recipe;
    let first = |i: &horizon_client::crafting::Ingredient| -> Option<String> {
        i.order.first().cloned().or_else(|| i.items.iter().min().cloned())
    };
    for (name, recipe) in &game.recipes().crafting {
        let (cells, made): (Vec<Option<String>>, _) = match recipe {
            Recipe::Shaped { cells, result, .. } => (cells.iter().map(|c| c.as_ref().and_then(first)).collect(), result),
            Recipe::Shapeless { ingredients, result } => (ingredients.iter().map(|i| first(i)).collect(), result),
            Recipe::Special(_) => continue,
        };
        if made.item.as_deref() != Some(result) {
            continue;
        }
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for c in cells.into_iter().flatten() {
            *counts.entry(c).or_default() += 1;
        }
        return Some((name.clone(), counts.into_iter().collect(), stack(result, made.count.max(1))));
    }
    None
}

fn count_of(slots: &[Option<ItemStack>], range: std::ops::Range<usize>, item: &str) -> u32 {
    slots
        .iter()
        .skip(range.start)
        .take(range.end - range.start)
        .flatten()
        .filter(|s| &*s.item == item)
        .map(|s| s.count)
        .sum()
}

/// Takes `n` of `item` from the slots of `range`.
fn take(slots: &mut [Option<ItemStack>], range: std::ops::Range<usize>, item: &str, mut n: u32) {
    for slot in range {
        if n == 0 {
            break;
        }
        if let Some(Some(s)) = slots.get_mut(slot)
            && &*s.item == item
        {
            let k = n.min(s.count);
            s.count -= k;
            n -= k;
            if s.count == 0 {
                slots[slot] = None;
            }
        }
    }
}

fn cook(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let on = toggles_at(game, pos, KITCHEN_RECIPES.len());
    if !on.iter().any(|t| *t) {
        return Shown::Idle("kitchen_none");
    }
    let slots = items(game, pos);
    for (i, result) in KITCHEN_RECIPES.iter().enumerate() {
        if !on[i] {
            continue;
        }
        let Some((id, need, made)) = food_recipe(game, result) else { continue };
        if !need.iter().all(|(item, n)| count_of(&slots, 0..9, item) >= *n) {
            continue;
        }
        if !fits(game, pos, 9..13, std::slice::from_ref(&made)) {
            return Shown::Blocked("output_full");
        }
        if !spend(game, pos, cost) {
            return Shown::NoEnergy;
        }
        with_items(game, pos, |items| {
            for (item, n) in &need {
                take(items, 0..9, item, *n);
            }
        });
        let count = made.count;
        put(game, pos, 9..13, made);
        if let Some(o) = owner(game, pos) {
            base::report(Report::Craft { owner: o, recipe: id, item: (*result).to_owned(), count, crafts: 1 });
        }
        return Shown::Working;
    }
    Shown::Idle("kitchen_missing")
}

// ---- The brewer ----

fn brew(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let slots = items(game, pos);
    let Some(reagent) = slots.get(3).cloned().flatten() else {
        return Shown::Idle("brew_no_reagent");
    };
    let bottles: Vec<(usize, ItemStack)> = (0..3).filter_map(|i| slots.get(i).cloned().flatten().map(|s| (i, s))).collect();
    if bottles.is_empty() {
        return Shown::Idle("brew_no_bottles");
    }
    let made: Vec<(usize, ItemStack)> = bottles
        .iter()
        .filter_map(|(i, b)| game.stations().brewing.find(b, &reagent).map(|r| (*i, r.output.clone())))
        .collect();
    if made.is_empty() {
        return Shown::Blocked("brew_no_recipe");
    }
    let fuel = game.native_mod_block_entity(pos).map_or(0, |e| read_i64(&e.nbt, FUEL_KEY));
    let has_powder = slots.get(4).cloned().flatten().is_some_and(|s| s.count > 0);
    if fuel <= 0 && !has_powder {
        return Shown::Blocked("brew_no_fuel");
    }
    let outputs: Vec<ItemStack> = made.iter().map(|(_, s)| s.clone()).collect();
    if !fits(game, pos, 5..8, &outputs) {
        return Shown::Blocked("output_full");
    }
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    game.native_with_mod_block_entity(pos, |e| {
        let mut f = read_i64(&e.nbt, FUEL_KEY);
        if f <= 0 {
            take_one(&mut e.items, 4);
            f = i64::from(BREWS_PER_POWDER);
        }
        write_i64(&mut e.nbt, FUEL_KEY, f - 1);
        take_one(&mut e.items, 3);
        for (i, _) in &made {
            e.items[*i] = None;
        }
    });
    for s in outputs {
        put(game, pos, 5..8, s);
    }
    if let Some(o) = owner(game, pos) {
        base::report(Report::Counted { kind: "brew", owner: o, count: 1 });
    }
    Shown::Working
}

// ---- The experience condenser and the enchanting machine ----

pub(super) fn xp_of(game: &Game<'_>, pos: Pos) -> u32 {
    game.native_mod_block_entity(pos).map_or(0, |e| read_i64(&e.nbt, XP_KEY).clamp(0, i64::from(xp::CAPACITY)) as u32)
}

fn set_xp(nbt: &mut Compound, points: u32) {
    write_i64(nbt, XP_KEY, i64::from(points.min(xp::CAPACITY)));
}

fn xp_tank_shown(game: &mut Game<'_>, pos: Pos) -> Shown {
    let points = xp_of(game, pos);
    anim::show_level(game, pos, f64::from(points) / f64::from(xp::CAPACITY), 0);
    if points > 0 { Shown::Working } else { Shown::Idle("xp_empty") }
}

/// A press in the condenser's window: give one level, give all, take one, take ten.
fn xp_press(game: &mut Game<'_>, entity: &mut ModBlockEntity, i: usize) {
    let player = game.native_acting_id();
    let stored = read_i64(&entity.nbt, XP_KEY).clamp(0, i64::from(xp::CAPACITY)) as u32;
    let mut level = 0u32;
    game.native_as_player(player, |g| level = g.experience.level.max(0) as u32);
    match i {
        0 | 1 => {
            let levels = if i == 0 { 1 } else { level };
            let room = xp::CAPACITY - stored;
            let mut n = levels.min(level);
            while n > 0 && xp::give(level, n) > room {
                n -= 1;
            }
            if n == 0 {
                return;
            }
            let points = xp::give(level, n);
            if game.native_give_levels(player, -(n as i32)) {
                set_xp(&mut entity.nbt, stored + points);
            }
        }
        _ => {
            let want = if i == 2 { 1 } else { 10 };
            let points = (level..level + want).map(xp::to_next).sum::<u32>().min(stored);
            if points > 0 && game.native_give_experience(player, points) {
                set_xp(&mut entity.nbt, stored - points);
            }
        }
    }
}

fn enchant(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let slots = items(game, pos);
    let Some(item) = slots.first().cloned().flatten() else {
        return Shown::Idle("enchant_empty");
    };
    let Some(tank) = neighbour_of(game, pos, Util::XpTank.spec().block) else {
        return Shown::Blocked("enchant_no_tank");
    };
    let tier = choice(game, pos, 3);
    let need = tier as u32 + 1;
    let lapis = slots.get(1).cloned().flatten().map_or(0, |s| s.count);
    if lapis < need {
        return Shown::Blocked("enchant_no_lapis");
    }
    let seed = (random() * f64::from(i32::MAX)) as i32;
    let e = &game.stations().enchantments;
    let offers = horizon_client::stations::enchant_offers(e, game.items(), seed, 15, Some(&item));
    let level_cost = offers.costs[tier];
    if level_cost <= 0 {
        return Shown::Blocked("enchant_not_enchantable");
    }
    let Some(enchanted) = horizon_client::stations::apply_enchantment(e, game.items(), seed, tier, level_cost, &item) else {
        return Shown::Blocked("enchant_not_enchantable");
    };
    let points = xp::enchant_points(level_cost as u32, need);
    if xp_of(game, tank) < points {
        return Shown::Blocked("enchant_no_xp");
    }
    if !fits(game, pos, 2..3, std::slice::from_ref(&enchanted)) {
        return Shown::Blocked("output_full");
    }
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    game.native_with_mod_block_entity(tank, |t| {
        let now = read_i64(&t.nbt, XP_KEY).max(0) as u32;
        set_xp(&mut t.nbt, now.saturating_sub(points));
    });
    with_items(game, pos, |items| {
        items[0] = None;
        for _ in 0..need {
            take_one(items, 1);
        }
    });
    put(game, pos, 2..3, enchanted);
    if let Some(o) = owner(game, pos) {
        base::report(Report::Counted { kind: "enchant", owner: o, count: need });
    }
    Shown::Working
}

// ---- The archaeology table ----

fn brush(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let slots = items(game, pos);
    let Some(block) = slots.first().cloned().flatten() else {
        return Shown::Idle("archeo_empty");
    };
    let Some(brush) = slots.get(1).cloned().flatten() else {
        return Shown::Blocked("archeo_no_brush");
    };
    let table = ARCHEO_TABLES[choice(game, pos, ARCHEO_TABLES.len())];
    let back = if &*block.item == "minecraft:suspicious_gravel" { "minecraft:gravel" } else { "minecraft:sand" };
    if !fits(game, pos, 2..11, &[stack(back, 1), stack("minecraft:stone", 1)]) {
        return Shown::Blocked("output_full");
    }
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    let was_grown = is_grown(&block);
    // Spec 26: a lifted block brushes with the loot it was found with.
    let remembered = found_of(&block);
    let rolled = remembered.as_ref().and_then(|(_, item)| item.clone());
    let table = found::brush_table(remembered.as_ref().map(|(f, _)| f), table).to_owned();
    let finds = match rolled {
        Some(item) => vec![item],
        None => game.sequence_loot(&table, &table, Some(&brush)),
    };
    let max_damage = game.items().get(&brush.item).map_or(0, |i| i.max_damage);
    with_items(game, pos, |items| {
        take_one(items, 0);
        if let Some(Some(b)) = items.get_mut(1) {
            b.damage += 1;
            if max_damage > 0 && b.damage >= max_damage {
                items[1] = None;
            }
        }
    });
    put(game, pos, 2..11, stack(back, 1));
    for f in finds {
        put(game, pos, 2..11, f);
    }
    // ST5: a block the vein grew gives its find but no digger's experience.
    if let Some(r) = grown::brush_report(owner(game, pos), was_grown) {
        base::report(r);
    }
    Shown::Working
}

// ---- Lifting suspicious blocks with the wrench (spec 26) ----

/// What the brushable block entity at a cell holds: its loot table, its seed and the find a brush
/// stroke already rolled.
type Brushed = (Option<String>, Option<i64>, Option<ItemStack>);

fn brushable_at(game: &Game<'_>, pos: Pos) -> Option<Brushed> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2]))? {
        BlockEntity::Brushable(b) => Some((
            match &b.loot_table {
                Some(Tag::String(t)) => Some(t.to_string_lossy()),
                _ => None,
            },
            match &b.loot_seed {
                Some(Tag::Long(seed)) => Some(*seed),
                _ => None,
            },
            b.item.clone(),
        )),
        _ => None,
    }
}

/// The self-test's structure: the brushable block at `pos` holds the loot table `table`.
pub(super) fn set_brush_loot(game: &mut Game<'_>, pos: Pos, table: &str) -> bool {
    match game.world.block_entities.get_mut(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Brushable(b)) => {
            b.loot_table = Some(Tag::String(JavaString::from(table)));
            b.loot_seed = None;
            b.item = None;
            true
        }
        _ => false,
    }
}

/// The item a lift gives of the suspicious `block`: marked grown, or with where it was found.
fn lifted_stack(block: &str, lift: &Lift, rolled: Option<&ItemStack>) -> ItemStack {
    let mut stack = ItemStack::new(block, 1);
    let mut data = Compound::default();
    match lift {
        Lift::Grown => {
            data.insert(JavaString::from(grown::MARK), Tag::Byte(1));
        }
        Lift::Found(f) => {
            let text = |s: &str| Tag::String(JavaString::from(s));
            let mut mark = Compound::default();
            let dimension = text(f.dimension.as_str());
            mark.insert(JavaString::from(found::DIMENSION), dimension);
            mark.insert(JavaString::from(found::POS), Tag::IntArray(f.pos.to_vec()));
            if let Some(table) = &f.loot_table {
                mark.insert(JavaString::from(found::LOOT_TABLE), text(table.as_str()));
            }
            if let Some(seed) = f.seed {
                mark.insert(JavaString::from(found::SEED), Tag::Long(seed));
            }
            if let Some(item) = rolled {
                mark.insert(JavaString::from(found::ITEM), save::item_to_nbt(item));
            }
            data.insert(JavaString::from(found::MARK), Tag::Compound(mark));
        }
    }
    stack
        .data_mut()
        .set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
    stack
}

/// Where a lifted block was found, from its item's custom data `rustcraft:found`, and the find a
/// brush stroke rolled before it was lifted.
fn found_of(stack: &ItemStack) -> Option<(Found, Option<ItemStack>)> {
    let data = stack.custom_data()?;
    let Some(Tag::Compound(mark)) = data.get(&JavaString::from(found::MARK)) else {
        return None;
    };
    let field = |key: &str| mark.get(&JavaString::from(key));
    let text = |key: &str| match field(key) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    let pos = match field(found::POS) {
        Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
        _ => [0; 3],
    };
    let seed = match field(found::SEED) {
        Some(Tag::Long(seed)) => Some(*seed),
        _ => None,
    };
    let rolled = field(found::ITEM).and_then(save::item_from_nbt);
    let found = Found {
        dimension: text(found::DIMENSION).unwrap_or_default(),
        pos,
        loot_table: text(found::LOOT_TABLE),
        seed,
        rolled: rolled.is_some(),
    };
    Some((found, rolled))
}

/// What a lifted block's item says of it: grown, or found where; none for an item with neither
/// mark.
pub(super) fn marks_of(stack: &ItemStack) -> Option<Lift> {
    if is_grown(stack) {
        return Some(Lift::Grown);
    }
    found_of(stack).map(|(f, _)| Lift::Found(f))
}

/// Spec 26: the suspicious block at `pos` of `dimension` lifted out of the world: its item,
/// marked grown (ST5) or with where it was found and its loot, and what that says. None when it is
/// no suspicious block or the world does not let it go.
pub(super) fn lift(game: &mut Game<'_>, dimension: &str, pos: Pos) -> Option<(ItemStack, Lift)> {
    let block = game.native_block(pos);
    if !grown::is_suspicious(&block) {
        return None;
    }
    let (table, seed, rolled) = brushable_at(game, pos).unwrap_or_default();
    let was_grown = grown_at(game, dimension, pos);
    let lift = found::lift(dimension, pos, was_grown, table, seed, rolled.is_some());
    let air = game.native_default_state("minecraft:air")?;
    if !game.native_set_block(dimension, pos, air, 3) {
        return None;
    }
    forget_grown(game, dimension, pos);
    Some((lifted_stack(&block, &lift, rolled.as_ref()), lift))
}

/// `native_use_item_on` of the wrench with sneaking on suspicious sand or gravel: the block is
/// lifted into the player's inventory; what does not fit drops where it stood.
pub(super) fn lift_with_wrench(game: &mut Game<'_>, uuid: u128, dimension: &str, pos: Pos) {
    let Some((stack, _)) = lift(game, dimension, pos) else {
        return;
    };
    let Some(rest) = game.native_give_stack(uuid, stack) else {
        return;
    };
    // `/summon`'s compound with the stack's components, marks and all.
    let nbt = Compound::from([(JavaString::from("Item"), save::item_to_nbt(&rest))]);
    let at = pos.map(|c| f64::from(c) + 0.5);
    if game
        .native_summon(dimension, "minecraft:item", at, nbt)
        .is_none()
    {
        log::warn!("rustcraft: the lifted {} at {pos:?} was lost", rest.item);
    }
}

/// `native_place_block` of suspicious sand or gravel, after the cell's grown mark was forgotten:
/// a grown one's cell is grown again, so a lift and a place do not wash its mark off (ST5); a
/// lifted one's block entity gets its loot back.
pub(super) fn on_place_suspicious(
    game: &mut Game<'_>,
    dimension: &str,
    stack: &ItemStack,
    pos: Pos,
) {
    if !grown::is_suspicious(&stack.item) {
        return;
    }
    if is_grown(stack) {
        let mut g = grown_cells(game);
        g.cells.grew(dimension, pos);
        g.dirty = true;
        return;
    }
    let Some((f, rolled)) = found_of(stack) else {
        return;
    };
    let set = match game.world.block_entities.get_mut(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Brushable(b)) => {
            b.loot_table = f.loot_table.as_deref().map(|t| Tag::String(JavaString::from(t)));
            b.loot_seed = f.loot_table.as_ref().and(f.seed).map(Tag::Long);
            b.item = if b.loot_table.is_none() { rolled } else { None };
            true
        }
        _ => false,
    };
    if set {
        game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
    }
}

// ---- The living vein (slice 13) ----

fn grow(game: &mut Game<'_>, dimension: &str, pos: Pos, cost: f64) -> Shown {
    use rustcraft_rules::utility::vein;
    let slots = items(game, pos);
    let mods = base::modules_at(game, pos);
    let stratum = modules::count(&mods, Module::Stratum) > 0;
    // Slice 18: the stratum takes the rock its window chose (sand or gravel).
    let chosen_rock = vein::stratum_rock(choice(game, pos, vein::STRATUM_ROCKS.len()));
    let wanted = |item: &str| if stratum { item == chosen_rock } else { vein::is_stone(item) };
    let Some(slot) = slots.iter().position(|s| s.as_ref().is_some_and(|s| s.count > 0 && wanted(&s.item))) else {
        return Shown::Idle("vein_no_rock");
    };
    let rock = slots[slot].as_ref().map(|s| s.item.to_string()).unwrap_or_default();
    let Some(cell) = vein::cells()
        .into_iter()
        .map(|c| [pos[0] + c[0], pos[1] + c[1], pos[2] + c[2]])
        .find(|p| {
            matches!(game.native_block(*p).as_str(), "minecraft:air" | "minecraft:cave_air")
                // A suspicious block falls where nothing holds it up, and leaves its mark behind.
                && (!stratum || grown::holds_up(&game.native_block([p[0], p[1] - 1, p[2]])))
        })
    else {
        return Shown::Blocked("vein_no_room");
    };
    let rare = modules::count(&mods, Module::Rare);
    let pick = modules::count(&mods, Module::Pick) > 0;
    let (ore, cost) = if stratum {
        // ST3: slower and dearer; `paced` gave the ore's numbers.
        if !state().ticks.values().any(|n| n.is_multiple_of(vein::STRATUM_TICKS)) {
            return Shown::Working;
        }
        (vein::stratum(&rock).unwrap_or("minecraft:suspicious_sand"), cost / vein::BLOCK_ENERGY * vein::STRATUM_ENERGY)
    } else if pick {
        (vein::PICKS[choice(game, pos, vein::PICKS.len())], cost * vein::PICK_FACTOR)
    } else {
        (vein::ore(random(), rare), cost)
    };
    let Some(state_id) = game.native_default_state(ore) else {
        return Shown::Blocked("vein_no_room");
    };
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    game.native_set_block(dimension, cell, state_id, 3);
    with_items(game, pos, |items| take_one(items, slot));
    if grown::is_suspicious(ore) {
        let mut g = grown_cells(game);
        g.cells.grew(dimension, cell);
        g.dirty = true;
    }
    base::report(Report::Place { dimension: dimension.to_owned(), pos: cell, block: ore.to_owned() });
    Shown::Working
}

// ---- Windows and diagnoses ----

fn shown_text(u: Util, s: &Shown) -> Text {
    let key = format!("rustcraft.util.{}", s.key());
    let fallback = match s {
        Shown::Working => "Works",
        Shown::NoEnergy => "No energy",
        Shown::Idle(_) => "Nothing to do",
        Shown::Blocked(_) => "Stands",
    };
    let _ = u;
    line(&key, fallback, &[])
}

fn severity(s: &Shown) -> Severity {
    match s {
        Shown::Working => Severity::Ok,
        Shown::Idle(_) => Severity::Warn,
        Shown::NoEnergy | Shown::Blocked(_) => Severity::Bad,
    }
}

/// What a player looking at it reads (A4).
pub(super) fn diagnosis(dimension: &str, pos: Pos, u: Util) -> Text {
    shown_at(dimension, pos).map_or_else(
        || line(&format!("block.rustcraft.{}", u.spec().block.trim_start_matches("rustcraft:")), title(u), &[]),
        |s| shown_text(u, &s),
    )
}

fn toggle_names(u: Util) -> Vec<(String, String)> {
    match u {
        Util::Kitchen => KITCHEN_RECIPES.iter().map(|r| ((*r).to_owned(), (*r).to_owned())).collect(),
        Util::Archeo => ARCHEO_TABLES
            .iter()
            .map(|t| ((*t).to_owned(), "minecraft:suspicious_sand".to_owned()))
            .collect(),
        Util::Enchanter => (1..=3).map(|l| (format!("level{l}"), "minecraft:lapis_lazuli".to_owned())).collect(),
        Util::XpTank => ["give1", "give_all", "take1", "take10"]
            .iter()
            .map(|k| ((*k).to_owned(), "minecraft:experience_bottle".to_owned()))
            .collect(),
        Util::Vein => rustcraft_rules::utility::vein::PICKS
            .iter()
            .map(|o| ((*o).to_owned(), (*o).to_owned()))
            .collect(),
        Util::Projector => ["remember", "ghost", "clear"]
            .iter()
            .map(|k| ((*k).to_owned(), "minecraft:light_blue_stained_glass".to_owned()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Its window: what it does, its energy, its toggles and its slots.
pub(super) fn sheet(game: &Game<'_>, dimension: &str, pos: Pos, u: Util, s: &mut Sheet) {
    let short = u.spec().block.trim_start_matches("rustcraft:");
    s.rows.push(base::v(&line(&format!("rustcraft.util.{short}.what"), title(u), &[])));
    s.rows.push(base::v(&match u {
        Util::XpTank => line(
            "rustcraft.util.xp_tank.stored",
            "Holds %s points of %s",
            &[Text::literal(xp_of(game, pos).to_string()), Text::literal(xp::CAPACITY.to_string())],
        ),
        Util::Projector | Util::Builder | Util::PortalDock => builder::row(game, pos, u),
        _ => line(
            "rustcraft.util.step",
            "A step every %s s",
            &[Text::literal(format!("{:.1}", paced(game, pos, u).0 as f64 / 20.0))],
        ),
    }));
    let spec = u.spec();
    let e = energy(game, pos);
    s.bars.push(Bar {
        label: base::v(&line("rustcraft.util.energy", "Energy", &[])),
        value: base::v(&Text::literal(format!("{e:.0} / {:.0} E", spec.buffer))),
        share: (e / spec.buffer).clamp(0.0, 1.0),
        energy: true,
        colour: None,
    });
    let on = toggles_at(game, pos, spec.toggles);
    // Slice 18: the vein's window offers sand or gravel with the stratum module (guide
    // `mod_stratum`), the ores with the ore choice module (`mod_pick`), nothing without either.
    let names = if u == Util::Vein {
        let mods = base::modules_at(game, pos);
        if modules::count(&mods, Module::Stratum) > 0 {
            rustcraft_rules::utility::vein::STRATUM_ROCKS.iter().map(|r| ((*r).to_owned(), (*r).to_owned())).collect()
        } else if modules::count(&mods, Module::Pick) > 0 {
            toggle_names(u)
        } else {
            Vec::new()
        }
    } else {
        toggle_names(u)
    };
    let shown_n = names.len();
    s.toggles = names
        .into_iter()
        .into_iter()
        .enumerate()
        .map(|(i, (key, icon))| Toggle {
            icon,
            name: base::v(&line(&format!("rustcraft.util.{short}.toggle.{}", key.replace([':', '/'], "_")), &key, &[])),
            on: on.get(i).copied().unwrap_or(false) || (matches!(u, Util::Archeo | Util::Enchanter | Util::Vein) && i == 0 && !on.iter().take(shown_n).any(|t| *t)),
            hint: base::v(&line(&format!("rustcraft.util.{short}.hint"), "Click to turn on or off", &[])),
        })
        .collect();
    let shown = shown_at(dimension, pos).unwrap_or(Shown::Idle("idle"));
    s.status = Some(SheetStatus {
        severity: severity_code(severity(&shown)),
        text: base::v(&shown_text(u, &shown)),
    });
}

/// A press in a slice-12 machine's window (`native_menu_button_v2`); the block entity is out of
/// the world during the call, so it is written to `entity`.
pub(super) fn on_button(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, id: i32) {
    let Some(Machine::Util(u)) = Machine::of(&entity.id) else {
        return;
    };
    let Some(Press::Toggle(i)) = Press::of(id) else {
        return;
    };
    let n = u.spec().toggles;
    if i >= n {
        return;
    }
    match u {
        Util::XpTank => xp_press(game, entity, i),
        Util::Projector => builder::projector_press(game, pos, entity, i),
        // A choice: this one on, the rest off.
        Util::Archeo | Util::Enchanter | Util::Vein => {
            let mut t = vec![false; n];
            t[i] = true;
            write_toggles(&mut entity.nbt, &t);
        }
        _ => {
            let mut t = toggles(&entity.nbt);
            t.resize(n, false);
            t[i] = !t[i];
            write_toggles(&mut entity.nbt, &t);
        }
    }
}

/// The sheet kind of a slice-12 machine.
pub(super) fn sheet_kind(u: Util) -> SheetKind {
    SheetKind::Util(u)
}

/// Says why a machine stands to a player (the projector's and the dock's clicks).
pub(super) fn say(game: &mut Game<'_>, player: u128, key: &str, fallback: &str, args: &[Text]) {
    tell(game, player, line(key, fallback, args));
}

/// Forgets what it showed of the blocks in an area that was taken away.
pub(super) fn forget_area(dimension: &str, min: Pos, max: Pos) {
    state().shown.retain(|(d, p), _| {
        d != dimension || !(0..3).all(|i| p[i] >= min[i] && p[i] <= max[i])
    });
}
