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
        charging, tiered,
        style::Severity,
    },
    modules::{self, Module},
    reports::Report,
    tuning::Tuning,
    utility::{
        self as rules, KITCHEN_RECIPES, Shown, Util, charge, disassembly,
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
use rustcraft_rules::storage::modules::CAPACITY;
/// The brewer's brews a blaze powder gives, as the brewing stand's.
const BREWS_PER_POWDER: i32 = 20;

#[derive(Default)]
struct State {
    ticks: HashMap<String, u64>,
    shown: HashMap<(String, Pos), Shown>,
    /// Slice 18: the milk (held, room) of the tank each kitchen and shearer touched last step.
    milk: HashMap<Pos, (u64, u64)>,
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

/// Slice 18, wave 2: the find table a grown block's item carries (custom data
/// `rustcraft:grown_table`).
fn grown_table_of(stack: &ItemStack) -> Option<String> {
    match stack.custom_data()?.get(&JavaString::from(grown::TABLE_KEY))? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

/// Slice 18, wave 2: the hold name a worker's grab keeps the grown block at `pos` under, with
/// the find table its vein chose (`grown::hold_name_with`).
pub(super) fn grown_hold_name(game: &Game<'_>, pos: Pos, block: &str) -> String {
    let table = brushable_at(game, pos).and_then(|(t, _, _)| t);
    grown::hold_name_with(block, table.as_deref())
}

/// A stack of the item a worker's hold names `name`: a grown block's with its mark (and its
/// find table).
pub(super) fn hold_stack(name: &str, count: u32) -> ItemStack {
    let (item, marked, table) = grown::from_hold_table(name);
    let mut stack = ItemStack::new(item, count);
    if marked {
        let mut data = stack.custom_data().cloned().unwrap_or_default();
        data.insert(JavaString::from(grown::MARK), Tag::Byte(1));
        if let Some(t) = table {
            data.insert(JavaString::from(grown::TABLE_KEY), Tag::String(JavaString::from(t)));
        }
        stack
            .data_mut()
            .set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
    }
    stack
}

/// The name a worker's hold keeps `stack` under: its item, marked when it was grown.
pub(super) fn hold_name(stack: &ItemStack) -> String {
    if is_grown(stack) {
        return grown::hold_name_with(&stack.item, grown_table_of(stack).as_deref());
    }
    grown::hold_name(&stack.item, false)
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
    // Slice 18: its tier's step (the disintegrator's 5 / 3 s) and its work mode (UT7).
    let tier = crate::game::rustcraft_tiers::tier_at(game, pos);
    // Wave 2: the repair stand and the kitchen work faster a tier above their first.
    let base_ticks = rules::tier_ticks(u, tiered::step_ticks(u, tier), tier, t.tier_speed_step);
    let ticks = modules::paced_ticks(base_ticks, &slots, t);
    let cost = rules::step_energy(u) * modules::factors(&slots, t).1;
    // Slice 18: the living vein keeps its own work mode (rustcraft_mining).
    if u == Util::Vein {
        return crate::game::rustcraft_mining::vein_paced(game, pos, ticks, cost);
    }
    let mode = machines::entity(game, pos).map_or(Default::default(), |e| machines::read_work(&e.nbt));
    tiered::in_mode(ticks, cost, mode)
}

pub(super) fn show(game: &mut Game<'_>, dimension: &str, pos: Pos, shown: Shown) {
    anim::show_on(game, pos, shown.working());
    // Slice 18 (guide `charger`, `charge_plate`): they glow while they charge: their furnace
    // template's `lit` gives the block light, and clients draw the coils' pulse (`draw_glow`).
    if matches!(Util::of(&game.native_block(pos)), Some(Util::Charger | Util::ChargePlate)) {
        let lit = if shown.working() { "true" } else { "false" };
        if game.native_block_property(pos, "lit").as_deref() != Some(lit) {
            game.native_set_block_property(pos, "lit", lit);
        }
    }
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
        // Wave 2: a tiered craft machine's block entity keeps its tier for the socket presses.
        if now.is_multiple_of(KITCHEN_STASH_EVERY) {
            keep_tier(game, pos);
        }
        // Slice 18 (cross): the kitchen's inputs go into its hidden cells between its steps too,
        // so its slots keep taking while it waits.
        if u == Util::Kitchen && now.is_multiple_of(KITCHEN_STASH_EVERY) {
            kitchen_stash(game, pos);
        }
        // Slice 18 (last): the brewer's reagents and powder likewise.
        if u == Util::Brewer && now.is_multiple_of(KITCHEN_STASH_EVERY) {
            brew_stash(game, pos);
        }
        let (every, cost) = paced(game, pos, u);
        if !now.is_multiple_of(every) {
            continue;
        }
        let shown = match u {
            Util::Trash => trash(game, pos, t),
            Util::Disintegrator => disintegrate(game, pos, cost),
            Util::Charger => charge_slots(game, dimension, pos, every),
            Util::ChargePlate => plate(game, dimension, pos, every),
            Util::Repair => repair_one(game, pos, cost),
            Util::Kitchen => cook(game, dimension, level, pos, cost, t),
            Util::Brewer => brew(game, dimension, pos, cost),
            Util::XpTank => xp_tank_shown(game, pos),
            Util::Enchanter => enchant(game, pos, cost),
            Util::Archeo => brush(game, pos, cost),
            Util::Fishery | Util::Feeder | Util::Shearer | Util::Apiary => {
                farm::step(game, dimension, level, pos, u, cost, now, t)
            }
            Util::Projector | Util::Builder | Util::PortalDock => {
                builder::step(game, dimension, level, pos, u, cost, now)
            }
            Util::Vein => grow(game, dimension, pos, cost),
        };
        show(game, dimension, pos, shown);
    }
    builder::save_transit(game);
}

/// Wave 2: the block entity's copy of a tiered machine's tier
/// (rustcraft_rules::machines::sockets::TIER_KEY), which the socket presses read.
fn keep_tier(game: &mut Game<'_>, pos: Pos) {
    let block = game.native_block(pos);
    if rustcraft_rules::tiers::family_of_block(&block).is_none() {
        return;
    }
    let tier = i32::from(crate::game::rustcraft_tiers::tier_of(game, pos, &block));
    let key = rustcraft_rules::machines::sockets::TIER_KEY;
    let saved = game.native_mod_block_entity(pos).map(|e| machines::read_int(&e.nbt, key));
    if saved.is_some_and(|s| s != Some(tier)) {
        game.native_with_mod_block_entity(pos, |e| machines::write_int(&mut e.nbt, key, tier));
    }
}

/// Slice 18: the module items in a machine's sockets (`modules` in its compound), the ones the
/// mod's module list does not know yet among them (the capacity module).
pub(super) fn module_items(entity: &ModBlockEntity) -> Vec<String> {
    match entity.nbt.get(&JavaString::from("modules")) {
        Some(Tag::List(list)) => list
            .iter()
            .filter_map(|t| match t {
                Tag::String(s) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// How many modules `id` the machine at `pos` holds.
pub(super) fn module_count(game: &Game<'_>, pos: Pos, id: &str) -> u32 {
    game.native_mod_block_entity(pos)
        .map_or(0, |e| module_items(&e).iter().filter(|i| i.as_str() == id).count() as u32)
}

fn owner(game: &Game<'_>, pos: Pos) -> Option<u128> {
    base::owner_at(game, pos)
}

// ---- The trash ----

/// UT7: what the trash burns a step on the first tier.
const TRASH_BATCH: u32 = 8;

fn trash(game: &mut Game<'_>, pos: Pos, _t: &Tuning) -> Shown {
    // Slice 18, wave 2 (guide `trash`): its joints' filters decide what it takes and burns; it
    // keeps a copy for its slot rule (`trash_filters`, asked without the world).
    let filters = joint_filters(game, pos);
    let json = tiered::filters_to_json(&filters);
    let kept = machines::entity(game, pos).and_then(|e| match e.nbt.get(&JavaString::from(TRASH_FILTERS_KEY)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    });
    if kept.as_deref() != Some(json.as_str()) {
        game.native_with_mod_block_entity(pos, |e| {
            e.nbt.insert(JavaString::from(TRASH_FILTERS_KEY), Tag::String(JavaString::from(json.as_str())));
        });
    }
    retire_filter_cells(game, pos);
    if !filters.iter().any(|f| !f.allow_only || !f.items.is_empty()) {
        return Shown::Blocked("trash_no_filter");
    }
    let slots = items(game, pos);
    // Slice 18: 8 / 16 / 32 / 64 a second by tier; a speed module shortens the step (paced), so it
    // doubles the rate once, not twice.
    let mut batch = tiered::trash_batch(crate::game::rustcraft_tiers::tier_at(game, pos));
    let per = paced(game, pos, Util::Trash).1;
    let mut burned = 0;
    let mut waiting = false;
    for slot in TRASH_BURN {
        let Some(Some(s)) = slots.get(slot) else { continue };
        if !tiered::trash_passes(&filters, &s.item) {
            continue;
        }
        waiting = true;
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
        note_burnt(&game.native_dimension().to_owned(), pos, n);
        batch -= n;
        if batch == 0 {
            break;
        }
    }
    if burned > 0 {
        Shown::Working
    } else if waiting {
        Shown::NoEnergy
    } else {
        Shown::Idle("trash_empty")
    }
}

/// Slice 18, wave 2: the block entity key of the trash's copy of its joints' filters.
const TRASH_FILTERS_KEY: &str = "trash_jf";
/// Slice 18, wave 2: the cells it burns from (0..9 were its own filter cells, now hidden).
const TRASH_BURN: std::ops::Range<usize> = 9..18;

/// What the trash `entity` takes, by its copy of its joints' filters.
pub(super) fn trash_filters(entity: &ModBlockEntity) -> Vec<rustcraft_rules::net::items::Filter> {
    match entity.nbt.get(&JavaString::from(TRASH_FILTERS_KEY)) {
        Some(Tag::String(s)) => tiered::filters_from_json(&s.to_string_lossy()),
        _ => Vec::new(),
    }
}

/// Slice 18, wave 2: its old filter cells are hidden now; what a world saved in them moves to the
/// cells it burns from as room allows.
fn retire_filter_cells(game: &mut Game<'_>, pos: Pos) {
    if !items(game, pos).iter().take(TRASH_BURN.start).any(Option::is_some) {
        return;
    }
    with_items(game, pos, |items| {
        for from in 0..TRASH_BURN.start {
            if items.get(from).is_none_or(Option::is_none) {
                continue;
            }
            if let Some(to) = TRASH_BURN.clone().find(|&i| items.get(i).is_some_and(Option::is_none)) {
                items[to] = items[from].take();
            }
        }
    });
}

/// Slice 18: the filters of the joints of the housings beside the trash at `pos` that face it.
fn joint_filters(game: &Game<'_>, pos: Pos) -> Vec<rustcraft_rules::net::items::Filter> {
    let mut out = Vec::new();
    for face in Face::ALL {
        let o = face.offset();
        let housing = [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]];
        if game.native_block(housing) != crate::game::rustcraft_content::HOUSING {
            continue;
        }
        let Some(e) = machines::entity(game, housing) else {
            continue;
        };
        let joints = crate::game::rustcraft_window::read_joints(&e.nbt);
        for slot in 0..4 {
            if let Some(s) = joints.get(face.opposite(), slot) {
                out.push(s.filter.clone());
            }
        }
    }
    out
}

/// Slice 18 (guide `trash`): a trash placed beside a housing, or a housing beside a trash, gets
/// the trash's joint settings where the housing has none: an empty allow list at the lowest
/// priority (rustcraft_rules::machines::tiered::trash_joint).
pub(super) fn on_placed(game: &mut Game<'_>, pos: Pos) {
    let here = game.native_block(pos);
    let housing = crate::game::rustcraft_content::HOUSING;
    let trash = Util::Trash.spec().block;
    for face in Face::ALL {
        let o = face.offset();
        let there = [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]];
        let (h, toward) = if here == trash && game.native_block(there) == housing {
            (there, face.opposite())
        } else if here == housing && game.native_block(there) == trash {
            (pos, face)
        } else {
            continue;
        };
        machines::change_entity(game, h, false, |e| {
            let mut joints = crate::game::rustcraft_window::read_joints(&e.nbt);
            for slot in 0..4 {
                if joints.get(toward, slot).is_none() {
                    joints = joints.with(toward, slot, tiered::trash_joint());
                }
            }
            crate::game::rustcraft_window::write_joints(&mut e.nbt, &joints);
        });
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

fn charge_slots(game: &mut Game<'_>, dimension: &str, pos: Pos, every: u64) -> Shown {
    let slots = items(game, pos);
    let held: Vec<(usize, charge::Battery)> = (0..4)
        .filter_map(|i| slots.get(i).cloned().flatten().and_then(|s| battery_of(game, &s).map(|b| (i, b))))
        .collect();
    // Slice 18 (guide `charger`): its speed module charges faster, its economy module draws less
    // of its buffer an E given (rustcraft_rules::machines::charging).
    let t = crate::game::rustcraft_server::tuning();
    let secs = every as f64 / 20.0;
    let factors = modules::factors(&base::modules_at(game, pos), t);
    // A8: the starsteel charger charges faster than the crystallite one.
    let tier_speed = rustcraft_rules::tiers::family_of_block(Util::Charger.spec().block).map_or(1.0, |f| {
        rustcraft_rules::tiers::speed(f, crate::game::rustcraft_tiers::tier_at(game, pos), t.tier_speed_step)
    });
    let (mut offer, per) = charging::charger_offer(
        Util::Charger.spec().draw * tier_speed,
        secs,
        factors,
        energy(game, pos),
    );
    let mut given = 0.0;
    let mut needs = false;
    if !held.is_empty() {
        let batteries: Vec<_> = held.iter().map(|(_, b)| *b).collect();
        needs = batteries.iter().any(|b| !b.full());
        let (after, used) = charge::share(&batteries, offer);
        offer -= used;
        given += used;
        for ((slot, _), b) in held.iter().zip(&after) {
            if let Some(s) = slots.get(*slot).cloned().flatten() {
                crate::game::rustcraft_gear::charge(&s, b.charge);
            }
        }
        // Full ones go to the hidden output slots for the net, when a housing beside it takes
        // them (slice 18, wave 2: `ui.js` shows the four «Броня» cells only).
        let net = beside_housing(game, pos);
        for ((slot, _), b) in held.iter().zip(&after) {
            if charging::to_output(b.full(), net)
                && let Some(s) = items(game, pos).get(*slot).cloned().flatten()
                && put(game, pos, CHARGER_ARMOUR..2 * CHARGER_ARMOUR, s).is_none()
            {
                with_items(game, pos, |items| items[*slot] = None);
            }
        }
    }
    if !beside_housing(game, pos) {
        // No net to take them: what waits in the hidden output comes back where it is seen.
        with_items(game, pos, |items| {
            for from in CHARGER_ARMOUR..2 * CHARGER_ARMOUR {
                if items.get(from).is_none_or(Option::is_none) {
                    continue;
                }
                if let Some(to) = (0..CHARGER_ARMOUR).find(|&i| items.get(i).is_some_and(Option::is_none)) {
                    items[to] = items[from].take();
                }
            }
        });
    }
    // «…или встань на него в снаряжении»: the gear of a player standing on it.
    let standing = standing_on(game, dimension, pos);
    for uuid in &standing {
        let (took, wanted) = charge_worn(game, *uuid, offer);
        offer -= took;
        given += took;
        needs |= wanted;
    }
    spend(game, pos, given * per);
    match charging::charged(!held.is_empty() || !standing.is_empty(), needs, given) {
        charging::Charged::Working => Shown::Working,
        charging::Charged::NoEnergy => Shown::NoEnergy,
        charging::Charged::Full | charging::Charged::Empty if !held.is_empty() || !standing.is_empty() => {
            Shown::Idle("plate_full")
        }
        _ => Shown::Idle("charge_empty"),
    }
}

/// Slice 18, wave 2: whether a housing of the net touches the block at `pos`.
fn beside_housing(game: &Game<'_>, pos: Pos) -> bool {
    Face::ALL.iter().any(|f| {
        let o = f.offset();
        game.native_block([pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]]) == crate::game::rustcraft_content::HOUSING
    })
}

/// The players standing on the block at `pos` (their feet in the cell above it, or on a thin
/// plate in its own cell).
fn standing_on(game: &Game<'_>, dimension: &str, pos: Pos) -> Vec<u128> {
    game.native_server_players()
        .into_iter()
        .filter(|p| {
            p.dimension == dimension
                && p.position[0].floor() as i32 == pos[0]
                && p.position[2].floor() as i32 == pos[2]
                && (p.position[1] - f64::from(pos[1])) > -0.01
                && (p.position[1] - f64::from(pos[1])) < 1.2
        })
        .map(|p| p.uuid)
        .collect()
}

/// W3: charges a player's worn armour (the jetpack with the chestplate) and both hands from
/// `offer` E, each in turn: (what it took, whether anything wanted charge).
fn charge_worn(game: &mut Game<'_>, uuid: u128, mut offer: f64) -> (f64, bool) {
    use horizon_world::player_inventory::EquipmentSlot as E;
    let mut took_all = 0.0;
    let mut wanted = false;
    for slot in [E::Head, E::Chest, E::Legs, E::Feet, E::MainHand, E::OffHand] {
        let Some(st) = game.native_player_equipment(uuid, slot) else { continue };
        let Some(b) = battery_of(game, &st) else { continue };
        wanted |= !b.full();
        let (after, took) = b.fill(offer);
        if took > 0.0 {
            crate::game::rustcraft_gear::charge(&st, after.charge);
            offer -= took;
            took_all += took;
        }
    }
    (took_all, wanted)
}

fn plate(game: &mut Game<'_>, dimension: &str, pos: Pos, every: u64) -> Shown {
    let below = [pos[0], pos[1] - 1, pos[2]];
    if game.native_block(below) != crate::game::rustcraft_content::HOUSING {
        return Shown::Blocked("plate_no_housing");
    }
    let standing = standing_on(game, dimension, pos);
    if standing.is_empty() {
        return Shown::Idle("plate_empty");
    }
    // Slice 18 (guide `charge_plate`): 128 E/s for each player on it, from its buffer.
    let secs = every as f64 / 20.0;
    let mut buffer = energy(game, pos);
    let mut used = 0.0;
    let mut needs = false;
    for uuid in standing {
        let offer = charging::plate_offer(buffer, secs);
        let (took, wanted) = charge_worn(game, uuid, offer);
        buffer -= took;
        used += took;
        needs |= wanted;
    }
    spend(game, pos, used);
    match charging::charged(true, needs, used) {
        charging::Charged::Working => Shown::Working,
        charging::Charged::NoEnergy => Shown::NoEnergy,
        _ => Shown::Idle("plate_full"),
    }
}

// ---- The repair stand ----

fn repair_one(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    use rustcraft_rules::utility::{REPAIR_MATERIAL, REPAIR_OUT, REPAIR_TOOLS, repair_material_end};
    let slots = items(game, pos);
    // Slice 18: three cells to repair, worked in turn.
    let Some((at, tool)) = REPAIR_TOOLS
        .clone()
        .find_map(|i| slots.get(i).cloned().flatten().map(|s| (i, s)))
    else {
        return Shown::Idle("repair_empty");
    };
    let max = game.items().get(&tool.item).map_or(0, |i| i.max_damage);
    if max == 0 || tool.damage == 0 {
        // Whole: to the output slots.
        if put(game, pos, REPAIR_OUT, tool).is_none() {
            with_items(game, pos, |items| items[at] = None);
            return Shown::Working;
        }
        return Shown::Blocked("output_full");
    }
    let Some(material) = repair::material_of(&tool.item) else {
        return Shown::Blocked("repair_no_material");
    };
    let end = repair_material_end(module_count(game, pos, CAPACITY));
    let Some(slot) = (REPAIR_MATERIAL.start..end)
        .find(|i| slots.get(*i).cloned().flatten().is_some_and(|s| material.contains(&&*s.item)))
    else {
        return Shown::Blocked("repair_no_material");
    };
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    let damage = repair::repaired(tool.damage, max);
    with_items(game, pos, |items| {
        take_one(items, slot);
        if let Some(Some(t)) = items.get_mut(at) {
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

// Slice 18 (guide `kitchen`): the pages, each a dish, in turn; milk from the tank it touches.
const PAGES_KEY: &str = "kitchen_pages";
const NEXT_PAGE_KEY: &str = "kitchen_next";

fn kitchen_pages(nbt: &Compound) -> Vec<Option<usize>> {
    let mut p: Vec<Option<usize>> = read_json(nbt, PAGES_KEY);
    p.resize(rustcraft_rules::utility::kitchen::PAGES_MAX, None);
    p
}

/// A press on a page's toggle: its dish becomes the next of the list (a page the kitchen has
/// no capacity module for stays as it is).
fn kitchen_press(game: &Game<'_>, pos: Pos, entity: &mut ModBlockEntity, i: usize) {
    use rustcraft_rules::utility::kitchen;
    let _ = (game, pos);
    let usable = kitchen::pages(capacity_of(entity));
    if i >= usable {
        return;
    }
    let mut pages = kitchen_pages(&entity.nbt);
    pages[i] = kitchen::cycle(pages[i]);
    write_json(&mut entity.nbt, PAGES_KEY, &pages);
}

fn capacity_of(entity: &ModBlockEntity) -> u32 {
    module_items(entity).iter().filter(|i| i.as_str() == CAPACITY).count() as u32
}

/// Slice 18 (cross): the kitchen's hidden input cells (guide `mod_stack`: each input holds 256
/// with a stack module), its saved compound's key.
pub(super) const KITCHEN_CELLS_KEY: &str = "kitchen_cells";
/// How often, in ticks, the kitchen's inputs go into its cells between its steps.
const KITCHEN_STASH_EVERY: u64 = 4;

/// Slice 18 (cross): the kitchen's inputs moved into or out of its hidden cells as its stack
/// module says (rustcraft_rules::utility::kitchen::stash), written back; the cells after it.
/// Only plain stacks (no damage, no components) go into a cell.
fn kitchen_stash(game: &mut Game<'_>, pos: Pos) -> Vec<rustcraft_rules::machines::slots::Slot> {
    use rustcraft_rules::machines::slots::Slot;
    use rustcraft_rules::utility::kitchen;
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return Vec::new();
    };
    let modules = base::modules_at(game, pos);
    let stacked = rustcraft_rules::modules::stack_tier(&modules) > 0;
    // Wave 2: and two hidden cells a capacity module.
    let queue = rustcraft_rules::utility::queue_cells(Util::Kitchen, &modules);
    let cells_before: Vec<Slot> = read_json(&entity.nbt, KITCHEN_CELLS_KEY);
    if !stacked && queue == 0 && cells_before.iter().all(Option::is_none) {
        return cells_before;
    }
    let plain = |s: &ItemStack| s.damage == 0 && s.extra.is_none();
    let visible_before: Vec<Slot> = (0..kitchen::INPUTS)
        .map(|k| {
            entity.items.get(k).cloned().flatten().filter(|s| plain(s) && s.count > 0).map(|s| (s.item.to_string(), s.count))
        })
        .collect();
    let mut visible = visible_before.clone();
    let mut cells = cells_before.clone();
    let max = |i: &str| machines::max_stack(game, i);
    kitchen::stash(&mut visible, &mut cells, stacked, queue, |i| rustcraft_rules::modules::machine_stack(&modules, max(i)), &max);
    if visible == visible_before && cells == cells_before {
        return cells;
    }
    let written = cells.clone();
    game.native_with_mod_block_entity(pos, move |e| {
        for (k, (now, was)) in visible.iter().zip(&visible_before).enumerate() {
            if now != was && let Some(slot) = e.items.get_mut(k) {
                *slot = now.as_ref().map(|(i, n)| ItemStack::new(i, *n));
            }
        }
        write_json(&mut e.nbt, KITCHEN_CELLS_KEY, &written);
    });
    cells
}

fn cook(game: &mut Game<'_>, dimension: &str, level: &LevelNet, pos: Pos, cost: f64, t: &Tuning) -> Shown {
    use rustcraft_rules::utility::kitchen::{self, BUCKET_MB, MILK, MILK_BUCKET};
    // Slice 18 (cross): with a stack module each input holds 256, the rest in hidden cells.
    let mut cells = kitchen_stash(game, pos);
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return Shown::Idle("kitchen_none");
    };
    let pages = kitchen_pages(&entity.nbt);
    let usable = kitchen::pages(capacity_of(&entity));
    let next = read_i64(&entity.nbt, NEXT_PAGE_KEY).max(0) as usize;
    drop(entity);
    // An order's dish first, whatever the pages say.
    let ordered = crate::game::rustcraft_orders::job_output_at(dimension, pos);
    let wanted: Vec<(Option<usize>, &str)> = ordered
        .iter()
        .map(|d| (None, d.as_str()))
        .chain(kitchen::in_turn(&pages, usable, next).into_iter().map(|(p, d)| (Some(p), KITCHEN_RECIPES[d])))
        .collect();
    if wanted.is_empty() {
        return Shown::Idle("kitchen_none");
    }
    let slots = items(game, pos);
    let mut no_milk = false;
    for (page, result) in wanted {
        let Some((id, need, made)) = food_recipe(game, result) else { continue };
        let milk = kitchen::milk_buckets(&need);
        let tank = crate::game::rustcraft_fluids::touching_amount(game, level, pos, MILK, t);
        state().milk.insert(pos, tank);
        let in_tank = tank.0;
        let from_tank = milk > 0 && in_tank >= u64::from(milk) * BUCKET_MB;
        let items_need: Vec<(String, u32)> = need
            .iter()
            .filter(|(i, _)| !(from_tank && i == MILK_BUCKET))
            .cloned()
            .collect();
        let have = |item: &str| count_of(&slots, 0..9, item) + rustcraft_rules::machines::slots::count_in(&cells, item);
        if !items_need.iter().all(|(item, n)| have(item.as_str()) >= *n) {
            no_milk |= milk > 0 && !from_tank;
            continue;
        }
        // Milk buckets brought as items leave their empty buckets with the dish.
        let buckets = if from_tank { 0 } else { milk };
        let mut out = vec![made.clone()];
        if buckets > 0 {
            out.push(stack("minecraft:bucket", buckets));
        }
        if !fits(game, pos, 9..13, &out) {
            return Shown::Blocked("output_full");
        }
        if !spend(game, pos, cost) {
            return Shown::NoEnergy;
        }
        if from_tank {
            crate::game::rustcraft_fluids::take_touching(game, level, pos, MILK, u64::from(milk) * BUCKET_MB, t);
        }
        // The hidden cells first, then the slots.
        let cells_was = cells.clone();
        // w2: what the slots still give after the cells (take_from returns what is left to take).
        let from_slots = kitchen::take_ingredients(&mut cells, &items_need);
        let cells_now = cells.clone();
        game.native_with_mod_block_entity(pos, |e| {
            for ((item, _), n) in items_need.iter().zip(&from_slots) {
                take(&mut e.items, 0..9, item, *n);
            }
            if cells_now != cells_was {
                write_json(&mut e.nbt, KITCHEN_CELLS_KEY, &cells_now);
            }
            if let Some(p) = page {
                write_i64(&mut e.nbt, NEXT_PAGE_KEY, (p as i64 + 1) % usable.max(1) as i64);
            }
        });
        let count = made.count;
        for s in out {
            put(game, pos, 9..13, s);
        }
        if let Some(o) = owner(game, pos) {
            base::report(Report::Craft { owner: o, recipe: id, item: result.to_owned(), count, crafts: 1 });
        }
        return Shown::Working;
    }
    if no_milk {
        return Shown::Blocked("kitchen_no_milk");
    }
    Shown::Idle("kitchen_missing")
}

// ---- The brewer ----

// Slice 18 (guide `brewer`): pages of an effect and a kind; the vat brews each batch through its
// page's whole chain by itself.
const BREW_PAGES_KEY: &str = "brew_pages";
/// The page the bottles in the vat are brewed for, and the next page to start.
const BREW_PAGE_KEY: &str = "brew_page";
const BREW_NEXT_KEY: &str = "brew_next";
/// The page the kind buttons set.
const BREW_SEL_KEY: &str = "brew_sel";

fn brew_pages(nbt: &Compound) -> Vec<rustcraft_rules::utility::brew::Page> {
    let mut p: Vec<rustcraft_rules::utility::brew::Page> = read_json(nbt, BREW_PAGES_KEY);
    p.resize(rustcraft_rules::utility::brew::PAGES_MAX, Default::default());
    p
}

/// A press in the vat's window: a page's button turns its effect to the next and chooses it; a
/// kind button sets the chosen page's kind.
fn brew_press(entity: &mut ModBlockEntity, i: usize) {
    use rustcraft_rules::utility::brew::{self, Kind, PAGES_MAX};
    let usable = brew::pages(capacity_of(entity));
    let mut pages = brew_pages(&entity.nbt);
    if i < PAGES_MAX {
        if i >= usable {
            return;
        }
        pages[i].effect = brew::cycle_effect(pages[i].effect);
        write_i64(&mut entity.nbt, BREW_SEL_KEY, i as i64);
    } else if let Some(kind) = Kind::ALL.get(i - PAGES_MAX) {
        let sel = (read_i64(&entity.nbt, BREW_SEL_KEY).max(0) as usize).min(usable - 1);
        pages[sel].kind = *kind;
    }
    write_json(&mut entity.nbt, BREW_PAGES_KEY, &pages);
}

/// Slice 18 (last, guide `mod_stack`): the brewer's hidden cells, its saved compound's keys: the
/// reagents' and, apart, the blaze powder's (rustcraft_rules::utility::brew::STASH_GROUPS).
pub(super) const BREW_CELLS_KEY: &str = "brew_cells";
pub(super) const BREW_POWDER_CELLS_KEY: &str = "brew_powder_cells";

/// Slice 18 (last): the brewer's reagents and blaze powder moved into or out of their hidden
/// cells as its stack module says (rustcraft_rules::machines::queue::stash, a group each),
/// written back; the reagents' cells and the powder's after it. Only plain stacks go in.
fn brew_stash(game: &mut Game<'_>, pos: Pos) -> [Vec<rustcraft_rules::machines::slots::Slot>; 2] {
    use rustcraft_rules::machines::slots::Slot;
    use rustcraft_rules::utility::brew::STASH_GROUPS;
    let keys = [BREW_CELLS_KEY, BREW_POWDER_CELLS_KEY];
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return [Vec::new(), Vec::new()];
    };
    let modules = base::modules_at(game, pos);
    let stacked = rustcraft_rules::modules::stack_tier(&modules) > 0;
    // Wave 2: and two hidden reagent cells a capacity module.
    let queue = rustcraft_rules::utility::queue_cells(Util::Brewer, &modules);
    let before: [Vec<Slot>; 2] = keys.map(|k| read_json(&entity.nbt, k));
    if !stacked && queue == 0 && before.iter().all(|c| c.iter().all(Option::is_none)) {
        return before;
    }
    let plain = |s: &ItemStack| s.damage == 0 && s.extra.is_none();
    let visible_before: Vec<Vec<Slot>> = STASH_GROUPS
        .iter()
        .map(|g| {
            g.clone()
                .map(|k| entity.items.get(k).cloned().flatten().filter(|s| plain(s) && s.count > 0).map(|s| (s.item.to_string(), s.count)))
                .collect()
        })
        .collect();
    let mut visible = visible_before.clone();
    let mut cells = before.clone();
    let max = |i: &str| machines::max_stack(game, i);
    for (g, (v, c)) in visible.iter_mut().zip(cells.iter_mut()).enumerate() {
        let open = rustcraft_rules::utility::brew::group_open(g, stacked, queue);
        rustcraft_rules::utility::kitchen::stash_open(v, c, open, |i| rustcraft_rules::modules::machine_stack(&modules, max(i)), &max);
    }
    if visible == visible_before && cells == before {
        return cells;
    }
    let written = cells.clone();
    game.native_with_mod_block_entity(pos, move |e| {
        for (g, (now, was)) in STASH_GROUPS.iter().zip(visible.iter().zip(&visible_before)) {
            for (k, (n, w)) in g.clone().zip(now.iter().zip(was)) {
                if n != w && let Some(slot) = e.items.get_mut(k) {
                    *slot = n.as_ref().map(|(i, c)| ItemStack::new(i, *c));
                }
            }
        }
        for (key, c) in keys.iter().zip(&written) {
            write_json(&mut e.nbt, key, c);
        }
    });
    cells
}

/// Takes one of `item` from the first cell holding it; whether there was one.
fn take_from_cells(cells: &mut [rustcraft_rules::machines::slots::Slot], item: &str) -> bool {
    for cell in cells.iter_mut() {
        if let Some((i, n)) = cell
            && i == item
            && *n > 0
        {
            *n -= 1;
            if *n == 0 {
                *cell = None;
            }
            return true;
        }
    }
    false
}

fn brew(game: &mut Game<'_>, dimension: &str, pos: Pos, cost: f64) -> Shown {
    use rustcraft_rules::utility::{
        BREW_BOTTLES, BREW_FUEL, BREW_INGREDIENTS, BREW_OUT,
        brew::{self, Step},
    };
    let _ = dimension;
    // Slice 18 (last): with a stack module the reagents and the powder hold 256, the rest in
    // hidden cells.
    let [reagent_cells, powder_cells] = brew_stash(game, pos);
    let slots = items(game, pos);
    let bottles: Vec<(usize, ItemStack)> = BREW_BOTTLES
        .clone()
        .filter_map(|i| slots.get(i).cloned().flatten().map(|s| (i, s)))
        .collect();
    crate::game::rustcraft_anim::show_level_cached(game, pos, bottles.len() as f64 / 3.0, 0);
    if bottles.is_empty() {
        return Shown::Idle("brew_no_bottles");
    }
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return Shown::Idle("brew_no_bottles");
    };
    let pages = brew_pages(&entity.nbt);
    let usable = brew::pages(capacity_of(&entity));
    let page_now = read_i64(&entity.nbt, BREW_PAGE_KEY);
    let next = read_i64(&entity.nbt, BREW_NEXT_KEY).max(0) as usize;
    let fuel = read_i64(&entity.nbt, FUEL_KEY);
    drop(entity);
    // The batch's page, or the next page in turn with an effect.
    let page_at = if page_now > 0 && (page_now as usize) <= usable && pages[page_now as usize - 1].effect.is_some() {
        page_now as usize - 1
    } else {
        let Some(p) = (0..usable).map(|k| (next + k) % usable).find(|p| pages[*p].effect.is_some()) else {
            return Shown::Idle("brew_none");
        };
        p
    };
    let chain = brew::chain(pages[page_at]);
    let applies = |i: &str| {
        let r = ItemStack::new(i, 1);
        bottles.iter().any(|(_, b)| game.stations().brewing.find(b, &r).is_some())
    };
    let has = |i: &str| {
        count_of(&slots, BREW_INGREDIENTS, i) > 0 || rustcraft_rules::machines::slots::count_in(&reagent_cells, i) > 0
    };
    match brew::next_step(&chain, applies, has) {
        Step::Done => {
            let done: Vec<ItemStack> = bottles.iter().map(|(_, b)| b.clone()).collect();
            if !fits(game, pos, BREW_OUT, &done) {
                return Shown::Blocked("output_full");
            }
            game.native_with_mod_block_entity(pos, |e| {
                for (i, _) in &bottles {
                    e.items[*i] = None;
                }
                write_i64(&mut e.nbt, BREW_PAGE_KEY, 0);
                write_i64(&mut e.nbt, BREW_NEXT_KEY, ((page_at + 1) % usable.max(1)) as i64);
            });
            for s in done {
                put(game, pos, BREW_OUT, s);
            }
            Shown::Working
        }
        Step::Missing(_) => Shown::Blocked("brew_no_reagent"),
        Step::Brew(ingredient) => {
            let has_powder = slots.get(BREW_FUEL).cloned().flatten().is_some_and(|s| s.count > 0)
                || rustcraft_rules::machines::slots::count_in(&powder_cells, "minecraft:blaze_powder") > 0;
            if fuel <= 0 && !has_powder {
                return Shown::Blocked("brew_no_fuel");
            }
            if !spend(game, pos, cost) {
                return Shown::NoEnergy;
            }
            let r = ItemStack::new(ingredient, 1);
            let made: Vec<(usize, ItemStack)> = bottles
                .iter()
                .filter_map(|(i, b)| game.stations().brewing.find(b, &r).map(|x| (*i, x.output.clone())))
                .collect();
            game.native_with_mod_block_entity(pos, |e| {
                let mut f = read_i64(&e.nbt, FUEL_KEY);
                // Slice 18 (last): from the hidden cells first, then the slots.
                if f <= 0 {
                    let mut powder = powder_cells.clone();
                    if take_from_cells(&mut powder, "minecraft:blaze_powder") {
                        write_json(&mut e.nbt, BREW_POWDER_CELLS_KEY, &powder);
                    } else {
                        take_one(&mut e.items, BREW_FUEL);
                    }
                    f = i64::from(BREWS_PER_POWDER);
                }
                write_i64(&mut e.nbt, FUEL_KEY, f - 1);
                let mut reagents = reagent_cells.clone();
                if take_from_cells(&mut reagents, ingredient) {
                    write_json(&mut e.nbt, BREW_CELLS_KEY, &reagents);
                } else {
                    take(&mut e.items, BREW_INGREDIENTS, ingredient, 1);
                }
                for (i, s) in &made {
                    e.items[*i] = Some(s.clone());
                }
                write_i64(&mut e.nbt, BREW_PAGE_KEY, page_at as i64 + 1);
            });
            if let Some(o) = owner(game, pos) {
                base::report(Report::Counted { kind: "brew", owner: o, count: 1 });
            }
            Shown::Working
        }
    }
}

/// The vat's window: its page's potion and kind, the step it is on; the fuel bar.
fn brew_sheet(game: &Game<'_>, pos: Pos, s: &mut Sheet) {
    use rustcraft_rules::utility::brew::{self, EFFECTS};
    let Some(entity) = game.native_mod_block_entity(pos) else { return };
    let pages = brew_pages(&entity.nbt);
    let usable = brew::pages(capacity_of(&entity));
    let sel = (read_i64(&entity.nbt, BREW_SEL_KEY).max(0) as usize).min(usable - 1);
    let fuel = read_i64(&entity.nbt, FUEL_KEY).max(0);
    drop(entity);
    let page = pages[sel];
    let row = match page.effect.and_then(|e| EFFECTS.get(e)) {
        Some((name, _)) => line(
            "rustcraft.util.brewer.page",
            "Page %s: %s, %s",
            &[
                Text::literal((sel + 1).to_string()),
                line(&format!("rustcraft.util.brewer.effect.{name}"), name, &[]),
                line(&format!("rustcraft.util.brewer.kind.{}", page.kind.name()), page.kind.name(), &[]),
            ],
        ),
        None => line("rustcraft.util.brewer.no_page", "Click a page to choose its potion", &[]),
    };
    let fuel_line = line(
        "rustcraft.util.brewer.brews",
        "Blaze powder: %s brews left",
        &[Text::literal(fuel.to_string())],
    );
    s.rows = vec![base::v(&row), base::v(&fuel_line)];
}

// ---- The experience condenser and the enchanting machine ----

pub(super) fn xp_of(game: &Game<'_>, pos: Pos) -> u32 {
    game.native_mod_block_entity(pos).map_or(0, |e| read_i64(&e.nbt, XP_KEY).clamp(0, i64::from(xp::CAPACITY)) as u32)
}

/// Slice 18 (K6): `points` into the condenser at `tank`, as far as it has room.
pub(super) fn add_xp(game: &mut Game<'_>, tank: Pos, points: u32) {
    game.native_with_mod_block_entity(tank, |t| {
        let now = read_i64(&t.nbt, XP_KEY).max(0) as u32;
        set_xp(&mut t.nbt, now.saturating_add(points));
    });
}

fn set_xp(nbt: &mut Compound, points: u32) {
    write_i64(nbt, XP_KEY, i64::from(points.min(xp::CAPACITY)));
}

fn xp_tank_shown(game: &mut Game<'_>, pos: Pos) -> Shown {
    let points = xp_of(game, pos);
    anim::show_level(game, pos, f64::from(points) / f64::from(xp::CAPACITY), 0);
    if points >= xp::CAPACITY {
        Shown::Blocked("xp_full")
    } else if points > 0 {
        Shown::Working
    } else {
        Shown::Idle("xp_empty")
    }
}

/// A press in the condenser's window (slice 18): give or take one level, ten or all; 2 E each.
fn xp_press(game: &mut Game<'_>, entity: &mut ModBlockEntity, i: usize) {
    use rustcraft_rules::utility::xp::XpPress;
    let Some(press) = xp::PRESSES.get(i).copied() else { return };
    let player = game.native_acting_id();
    let stored = read_i64(&entity.nbt, XP_KEY).clamp(0, i64::from(xp::CAPACITY)) as u32;
    let energy = machines::read_f64(&entity.nbt, machines::BUFFER_KEY);
    if energy + 1e-9 < xp::PRESS_ENERGY {
        say(game, player, "rustcraft.util.no_energy", "No energy", &[]);
        return;
    }
    let mut level = 0u32;
    game.native_as_player(player, |g| level = g.experience.level.max(0) as u32);
    let done = match press {
        XpPress::Give(amount) => {
            let (n, points) = xp::give_levels(level, amount, xp::CAPACITY - stored);
            if n == 0 {
                if stored >= xp::CAPACITY {
                    say(game, player, "rustcraft.util.xp_full", "The condenser is full", &[]);
                }
                return;
            }
            game.native_give_levels(player, -(n as i32)) && {
                set_xp(&mut entity.nbt, stored + points);
                true
            }
        }
        XpPress::Take(amount) => {
            let points = xp::take_points(level, amount, stored);
            points > 0
                && game.native_give_experience(player, points)
                && {
                    set_xp(&mut entity.nbt, stored - points);
                    true
                }
        }
    };
    if done {
        machines::write_f64(&mut entity.nbt, machines::BUFFER_KEY, energy - xp::PRESS_ENERGY);
    }
}

const ENCHANT_LEVEL_KEY: &str = "enchant_level";

/// The enchanting level the machine's window chose (30 until one is chosen).
fn enchant_level(game: &Game<'_>, pos: Pos) -> u32 {
    game.native_mod_block_entity(pos)
        .map(|e| read_i64(&e.nbt, ENCHANT_LEVEL_KEY))
        .filter(|l| *l > 0)
        .map_or(xp::LEVEL_MAX, |l| (l as u32).clamp(xp::LEVEL_MIN, xp::LEVEL_MAX))
}

fn enchant(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let slots = items(game, pos);
    let Some(item) = slots.first().cloned().flatten() else {
        return Shown::Idle("enchant_empty");
    };
    let Some(tank) = neighbour_of(game, pos, Util::XpTank.spec().block) else {
        return Shown::Blocked("enchant_no_tank");
    };
    // Slice 18: the level the player chose, 1–30; its price by tens.
    let level = enchant_level(game, pos);
    let need = xp::enchant_cost(level);
    let tier = need as usize - 1;
    let lapis = slots.get(1).cloned().flatten().map_or(0, |s| s.count);
    if lapis < need {
        return Shown::Blocked("enchant_no_lapis");
    }
    if !xp::enough_for(xp_of(game, tank), level) {
        return Shown::Blocked("enchant_no_xp");
    }
    let seed = (random() * f64::from(i32::MAX)) as i32;
    let e = &game.stations().enchantments;
    let offers = horizon_client::stations::enchant_offers(e, game.items(), seed, 15, Some(&item));
    if offers.costs.iter().all(|c| *c <= 0) {
        return Shown::Blocked("enchant_not_enchantable");
    }
    let Some(enchanted) = horizon_client::stations::apply_enchantment(e, game.items(), seed, tier, level as i32, &item) else {
        return Shown::Blocked("enchant_not_enchantable");
    };
    let points = xp::enchant_points(level, need);
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
    // Slice 18: a block that remembers no place brushes by its kind's usual table.
    let table = found::default_table(&block.item);
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
    // Slice 18, wave 2: a grown block brushes by the find table its vein chose.
    let table = grown_table_of(&block).unwrap_or(table);
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
fn lifted_stack(block: &str, lift: &Lift, rolled: Option<&ItemStack>, table: Option<&str>) -> ItemStack {
    let mut stack = ItemStack::new(block, 1);
    let mut data = Compound::default();
    match lift {
        Lift::Grown => {
            data.insert(JavaString::from(grown::MARK), Tag::Byte(1));
            // Slice 18, wave 2 (guide `mod_stratum`): the find table the vein's window chose.
            if let Some(t) = table.filter(|t| !t.is_empty()) {
                data.insert(JavaString::from(grown::TABLE_KEY), Tag::String(JavaString::from(t)));
            }
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
    let grown_table = table.clone();
    let lift = found::lift(dimension, pos, was_grown, table, seed, rolled.is_some());
    let air = game.native_default_state("minecraft:air")?;
    if !game.native_set_block(dimension, pos, air, 3) {
        return None;
    }
    forget_grown(game, dimension, pos);
    Some((lifted_stack(&block, &lift, rolled.as_ref(), grown_table.as_deref()), lift))
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
    // Slice 18: the stratum takes the rock its window chose (sand or gravel); wave 2: and the find
    // table its block gets.
    let picked = toggles_at(game, pos, Util::Vein.spec().toggles);
    let chosen_rock = vein::stratum_rock(vein::stratum_choice(&picked).0);
    let chosen_table = vein::stratum_table(&picked);
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
        set_brush_loot(game, cell, chosen_table);
        let mut g = grown_cells(game);
        g.cells.grew(dimension, cell);
        g.dirty = true;
    }
    base::report(Report::Place { dimension: dimension.to_owned(), pos: cell, block: ore.to_owned() });
    // Slice 18: «Выращено всего» (rustcraft_mining).
    crate::game::rustcraft_mining::vein_grew(game, pos);
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
        Util::Brewer => {
            let pages = (1..=rustcraft_rules::utility::brew::PAGES_MAX).map(|p| (format!("page{p}"), "minecraft:glass_bottle".to_owned()));
            let kinds = [
                ("normal", "minecraft:potion"),
                ("long", "minecraft:redstone"),
                ("strong", "minecraft:glowstone_dust"),
                ("splash", "minecraft:gunpowder"),
                ("lingering", "minecraft:dragon_breath"),
            ]
            .map(|(k, i)| (k.to_owned(), i.to_owned()));
            pages.chain(kinds).collect()
        }
        // Slice 18: a toggle a page (its icons are set by the sheet from the page's dish).
        Util::Kitchen => (1..=rustcraft_rules::utility::kitchen::PAGES_MAX)
            .map(|p| (format!("page{p}"), "minecraft:paper".to_owned()))
            .collect(),
        Util::Enchanter => ["minus10", "minus1", "plus1", "plus10"]
            .iter()
            .map(|k| ((*k).to_owned(), "minecraft:lapis_lazuli".to_owned()))
            .collect(),
        Util::XpTank => ["give1", "give10", "give_all", "take1", "take10", "take_all"]
            .iter()
            .map(|k| ((*k).to_owned(), "minecraft:experience_bottle".to_owned()))
            .collect(),
        Util::Vein => rustcraft_rules::utility::vein::PICKS
            .iter()
            .map(|o| ((*o).to_owned(), (*o).to_owned()))
            .collect(),
        Util::Apiary => [("honey", "minecraft:honey_bottle"), ("combs", "minecraft:honeycomb")]
            .iter()
            .map(|(k, i)| ((*k).to_owned(), (*i).to_owned()))
            .collect(),
        Util::Projector => crate::game::rustcraft_projector::TOGGLES
            .iter()
            .map(|k| ((*k).to_owned(), "minecraft:light_blue_stained_glass".to_owned()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Its window: what it does, its energy, its toggles and its slots.
pub(super) fn sheet(game: &Game<'_>, dimension: &str, level: &LevelNet, pos: Pos, u: Util, s: &mut Sheet) {
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
    // Slice 18 (UT7): its work mode, for the sheet's switch.
    if SheetKind::Util(u).shape().mode {
        s.mode = machines::entity(game, pos).map(|e| machines::read_work(&e.nbt).code());
    }
    let spec = u.spec();
    let e = energy(game, pos);
    s.bars.push(Bar {
        label: base::v(&line("rustcraft.util.energy", "Energy", &[])),
        value: base::v(&Text::literal(format!(
            "{} / {} E",
            rustcraft_rules::machines::panel::energy_text(e.floor()),
            rustcraft_rules::machines::panel::energy_text(spec.buffer)
        ))),
        share: (e / spec.buffer).clamp(0.0, 1.0),
        energy: true,
        colour: None,
    });
    sheet_extras(game, dimension, pos, u, s);
    // Slice 18, stream 5: the craft machines' own rows.
    craft_sheet(game, dimension, level, pos, u, s);
    let on = toggles_at(game, pos, spec.toggles);
    // Slice 18: the vein's window offers sand or gravel with the stratum module (guide
    // `mod_stratum`), the ores with the ore choice module (`mod_pick`), nothing without either.
    let names = if u == Util::Vein {
        let mods = base::modules_at(game, pos);
        if modules::count(&mods, Module::Stratum) > 0 {
            use rustcraft_rules::utility::vein::{STRATUM_ROCKS, STRATUM_TABLES};
            STRATUM_ROCKS
                .iter()
                .map(|r| ((*r).to_owned(), (*r).to_owned()))
                .chain(STRATUM_TABLES.iter().map(|t| ((*t).to_owned(), "minecraft:brush".to_owned())))
                .collect()
        } else if modules::count(&mods, Module::Pick) > 0 {
            toggle_names(u)
        } else {
            Vec::new()
        }
    } else {
        toggle_names(u)
    };
    let shown_n = names.len();
    // Slice 18, wave 2: the stratum's two choices, a rock and a find table, one of each on.
    let stratum_on = (u == Util::Vein && modules::count(&base::modules_at(game, pos), Module::Stratum) > 0).then(|| {
        let (rock, table) = rustcraft_rules::utility::vein::stratum_choice(&on);
        (rock, rustcraft_rules::utility::vein::STRATUM_ROCKS.len() + table)
    });
    s.toggles = names
        .into_iter()
        .into_iter()
        .enumerate()
        .map(|(i, (key, icon))| Toggle {
            icon,
            name: base::v(&line(&format!("rustcraft.util.{short}.toggle.{}", key.replace([':', '/'], "_")), &key, &[])),
            on: stratum_on.map_or(false, |(r, t)| i == r || i == t)
                || (stratum_on.is_none() && on.get(i).copied().unwrap_or(false))
                || (matches!(u, Util::Archeo | Util::Enchanter | Util::Vein | Util::Apiary)
                    && stratum_on.is_none()
                    && i == 0
                    && !on.iter().take(shown_n).any(|t| *t)),
            hint: base::v(&line(&format!("rustcraft.util.{short}.hint"), "Click to turn on or off", &[])),
        })
        .collect();
    craft_toggles(game, pos, u, s);
    let shown = shown_at(dimension, pos).unwrap_or(Shown::Idle("idle"));
    s.status = Some(SheetStatus {
        severity: severity_code(severity(&shown)),
        text: base::v(&shown_text(u, &shown)),
    });
    // Slice 18: the living vein's mode, progress and counts (rustcraft_mining).
    if u == Util::Vein {
        let now = state().ticks.get(dimension).copied().unwrap_or(0);
        crate::game::rustcraft_mining::vein_sheet(game, pos, now, paced(game, pos, u).0, s);
    }
}

/// Slice 18 (guide `ui.js`): each craft machine's own parts of its window, after the energy bar.
fn craft_sheet(game: &Game<'_>, dimension: &str, level: &LevelNet, pos: Pos, u: Util, s: &mut Sheet) {
    let _ = dimension;
    match u {
        Util::Repair => {
            // «алмазная кирка · 970 из 1561».
            let tool = items(game, pos)
                .into_iter()
                .take(rustcraft_rules::utility::REPAIR_TOOLS.end)
                .flatten()
                .next();
            let (value, share) = match tool {
                Some(t) => {
                    let max = game.items().get(&t.item).map_or(0, |i| i.max_damage);
                    let left = max.saturating_sub(t.damage);
                    let name = line(&item_key(&t.item), &t.item, &[]);
                    let text = line(
                        "rustcraft.util.repair.durability",
                        "%s · %s of %s",
                        &[name, Text::literal(left.to_string()), Text::literal(max.to_string())],
                    );
                    (text, if max > 0 { f64::from(left) / f64::from(max) } else { 1.0 })
                }
                None => (line("rustcraft.util.repair.nothing", "Nothing to repair", &[]), 0.0),
            };
            s.bars.push(Bar {
                label: base::v(&line("rustcraft.util.repair.bar", "Durability", &[])),
                value: base::v(&value),
                share,
                energy: false,
                colour: None,
            });
        }
        Util::Kitchen => kitchen_sheet(game, pos, s),
        Util::Projector => {
            s.rows = crate::game::rustcraft_projector::projector_rows(game, dimension, level, pos).iter().map(base::v).collect();
        }
        Util::Builder => crate::game::rustcraft_projector::builder_sheet(game, pos, s),
        Util::Brewer => brew_sheet(game, pos, s),
        Util::Enchanter => {
            let level = enchant_level(game, pos);
            let need = xp::enchant_cost(level);
            let row = line(
                "rustcraft.util.enchanter.level",
                "Enchanting level %s · costs %s levels and %s lapis",
                &[
                    Text::literal(level.to_string()),
                    Text::literal(need.to_string()),
                    Text::literal(need.to_string()),
                ],
            );
            if let Some(r) = s.rows.get_mut(1) {
                *r = base::v(&row);
            }
        }
        Util::Archeo => {
            // «Расчищает: песок из пустынного храма».
            let block = items(game, pos).first().cloned().flatten();
            let row = match block {
                Some(b) => {
                    let remembered = found_of(&b);
                    let table = found::brush_table(remembered.as_ref().map(|(f, _)| f), found::default_table(&b.item)).to_owned();
                    let key = format!("rustcraft.util.archeo.toggle.{}", table.replace([':', '/'], "_"));
                    let place = line(&key, &table, &[]);
                    line(
                        "rustcraft.util.archeo.brushes",
                        "Brushes: %s from %s",
                        &[line(&item_key(&b.item).replace("item.", "block."), &b.item, &[]), place],
                    )
                }
                None => line("rustcraft.util.archeo_empty", "No suspicious block", &[]),
            };
            if let Some(r) = s.rows.get_mut(1) {
                *r = base::v(&row);
            }
        }
        Util::XpTank => {
            // «Хранит» as a bar; the row says whether an enchanting machine stands beside it.
            let points = xp_of(game, pos);
            s.bars.push(Bar {
                label: base::v(&line("rustcraft.util.xp_tank.holds", "Holds", &[])),
                value: base::v(&line(
                    "rustcraft.util.xp_tank.points",
                    "%s points of %s (%s levels from zero)",
                    &[
                        Text::literal(points.to_string()),
                        Text::literal(xp::CAPACITY.to_string()),
                        Text::literal(xp::levels_from(0, points).to_string()),
                    ],
                )),
                share: f64::from(points) / f64::from(xp::CAPACITY),
                energy: false,
                colour: Some(0xFF7C_E05A),
            });
            let beside = neighbour_of(game, pos, Util::Enchanter.spec().block).is_some();
            if let Some(r) = s.rows.get_mut(1) {
                *r = base::v(&if beside {
                    line("rustcraft.util.xp_tank.beside", "Beside it: an enchanting machine", &[])
                } else {
                    line("rustcraft.util.xp_tank.alone", "No enchanting machine beside it", &[])
                });
            }
        }
        Util::Feeder => {
            if let Some(r) = s.rows.first_mut() {
                *r = base::v(&farm::feeder_row(game, pos));
            }
        }
        Util::Apiary => {
            if let Some(r) = s.rows.first_mut() {
                *r = base::v(&farm::apiary_row(game, pos));
            }
        }
        Util::Shearer => {
            if let Some(r) = s.rows.first_mut() {
                *r = base::v(&farm::shearer_row(game, pos));
            }
            let (held, room) = farm::milk_of(pos);
            let b = rustcraft_rules::utility::kitchen::BUCKET_MB;
            s.bars.push(Bar {
                label: base::v(&line("rustcraft.util.kitchen.milk", "Milk in the tank", &[])),
                value: base::v(&Text::literal(format!("{} / {}", held / b, (held + room) / b))),
                share: if held + room > 0 { held as f64 / (held + room) as f64 } else { 0.0 },
                energy: false,
                colour: Some(0xFFF2_F2F2),
            });
        }
        Util::Fishery => {
            let now = state().ticks.get(dimension).copied().unwrap_or(0);
            s.rows = farm::fishery_rows(game, dimension, pos, now).iter().map(base::v).collect();
        }
        // Wave 2 (ui.js `portal_dock`): «Пара · док в Незере · связан», «Тот мир · выгружен» and
        // its fluid buffer; its 27 slots are one buffer of what goes and what came.
        Util::PortalDock => {
            s.rows = vec![
                base::v(&builder::row(game, pos, u)),
                base::v(&builder::dock_world_row(game, pos)),
            ];
        }
        _ => {}
    }
}

/// The kitchen's window: the page's dish and what it takes, the milk in the tank it touches.
fn kitchen_sheet(game: &Game<'_>, pos: Pos, s: &mut Sheet) {
    use rustcraft_rules::utility::kitchen::{self, BUCKET_MB, MILK};
    let Some(entity) = game.native_mod_block_entity(pos) else { return };
    let pages = kitchen_pages(&entity.nbt);
    let usable = kitchen::pages(capacity_of(&entity));
    let next = read_i64(&entity.nbt, NEXT_PAGE_KEY).max(0) as usize % usable.max(1);
    drop(entity);
    let row = match pages[next].and_then(|d| KITCHEN_RECIPES.get(d)) {
        Some(dish) => {
            let need = food_recipe(game, dish).map(|(_, n, _)| n).unwrap_or_default();
            let parts: Vec<String> = need.iter().map(|(i, n)| format!("{} ×{n}", i.rsplit(':').next().unwrap_or(i))).collect();
            line(
                "rustcraft.util.kitchen.page",
                "Page %s: %s (%s)",
                &[
                    Text::literal((next + 1).to_string()),
                    line(&item_key(dish), dish, &[]),
                    Text::literal(parts.join(", ")),
                ],
            )
        }
        None => line("rustcraft.util.kitchen.no_dish", "Click a page to choose its dish", &[]),
    };
    let milk = server_milk(pos);
    let milk_line = line(
        "rustcraft.util.kitchen.milk_line",
        "Milk in the tank: %s of %s buckets",
        &[
            Text::literal((milk.0 / BUCKET_MB).to_string()),
            Text::literal(((milk.0 + milk.1) / BUCKET_MB).to_string()),
        ],
    );
    s.rows = vec![base::v(&row), base::v(&milk_line)];
    let _ = MILK;
}

/// The milk (held, room) of the tank a kitchen or shearer touches, as its last step read it.
fn server_milk(pos: Pos) -> (u64, u64) {
    state().milk.get(&pos).copied().unwrap_or((0, 0))
}

/// Slice 18: the kitchen's page toggles show each page's dish, a page without a capacity module
/// for it barred.
fn craft_toggles(game: &Game<'_>, pos: Pos, u: Util, s: &mut Sheet) {
    use rustcraft_rules::utility::kitchen;
    if u == Util::Brewer {
        brew_toggles(game, pos, s);
        return;
    }
    if u != Util::Kitchen {
        return;
    }
    let Some(entity) = game.native_mod_block_entity(pos) else { return };
    let pages = kitchen_pages(&entity.nbt);
    let usable = kitchen::pages(capacity_of(&entity));
    drop(entity);
    for (i, t) in s.toggles.iter_mut().enumerate() {
        let dish = pages.get(i).copied().flatten().and_then(|d| KITCHEN_RECIPES.get(d));
        t.on = dish.is_some() && i < usable;
        t.icon = match (i < usable, dish) {
            (false, _) => "minecraft:iron_bars".to_owned(),
            (true, Some(d)) => (*d).to_owned(),
            (true, None) => "minecraft:paper".to_owned(),
        };
        t.name = base::v(&match dish {
            Some(d) => line(&item_key(d), d, &[]),
            None => line("rustcraft.util.kitchen.empty_page", "Empty page", &[]),
        });
        t.hint = base::v(&if i < usable {
            line("rustcraft.util.kitchen.page_hint", "Click: the next dish", &[])
        } else {
            line("rustcraft.util.kitchen.locked", "One more page for each capacity module", &[])
        });
    }
}

/// The vat's page buttons show their effect's ingredient, the kind buttons the chosen page's kind.
fn brew_toggles(game: &Game<'_>, pos: Pos, s: &mut Sheet) {
    use rustcraft_rules::utility::brew::{self, EFFECTS, Kind, PAGES_MAX};
    let Some(entity) = game.native_mod_block_entity(pos) else { return };
    let pages = brew_pages(&entity.nbt);
    let usable = brew::pages(capacity_of(&entity));
    let sel = (read_i64(&entity.nbt, BREW_SEL_KEY).max(0) as usize).min(usable - 1);
    drop(entity);
    for (i, t) in s.toggles.iter_mut().enumerate() {
        if i < PAGES_MAX {
            let effect = pages[i].effect.and_then(|e| EFFECTS.get(e));
            t.on = i == sel && i < usable;
            t.icon = match (i < usable, effect) {
                (false, _) => "minecraft:iron_bars".to_owned(),
                (true, Some((_, ingredient))) => (*ingredient).to_owned(),
                (true, None) => "minecraft:glass_bottle".to_owned(),
            };
            t.name = base::v(&match effect {
                Some((name, _)) => line(&format!("rustcraft.util.brewer.effect.{name}"), name, &[]),
                None => line("rustcraft.util.kitchen.empty_page", "Empty page", &[]),
            });
        } else if let Some(k) = Kind::ALL.get(i - PAGES_MAX) {
            t.on = pages[sel].kind == *k;
            t.name = base::v(&line(&format!("rustcraft.util.brewer.kind.{}", k.name()), k.name(), &[]));
        }
    }
}

/// The translation key of an item's name.
pub(super) fn item_key(item: &str) -> String {
    format!("item.{}", item.replace(':', "."))
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
        Util::Kitchen => kitchen_press(game, pos, entity, i),
        Util::Brewer => brew_press(entity, i),
        // A choice: this one on, the rest off.
        Util::Enchanter => {
            let now = read_i64(&entity.nbt, ENCHANT_LEVEL_KEY);
            let now = if now > 0 { now as u32 } else { xp::LEVEL_MAX };
            let by = xp::LEVEL_STEPS[i];
            write_i64(&mut entity.nbt, ENCHANT_LEVEL_KEY, i64::from(xp::step_level(now, by)));
        }
        Util::Vein if modules::count(&modules::read(&base::read_module_items(&entity.nbt)), Module::Stratum) > 0 => {
            let t = rustcraft_rules::utility::vein::stratum_press(&toggles(&entity.nbt), i);
            write_toggles(&mut entity.nbt, &t);
        }
        Util::Archeo | Util::Vein | Util::Apiary => {
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

// ---- Slice 18, stream 3: the trash's, the disintegrator's and the charger's windows (`ui.js`) ----

/// What the trash at a place burnt lately, for «сгорело за минуту».
static BURNT: LazyLock<Mutex<HashMap<(String, Pos), std::collections::VecDeque<(std::time::Instant, u32)>>>> =
    LazyLock::new(Mutex::default);
const MINUTE: std::time::Duration = std::time::Duration::from_secs(60);

/// The trash at `pos` destroyed `n` items or buckets.
pub(super) fn note_burnt(dimension: &str, pos: Pos, n: u32) {
    if n == 0 {
        return;
    }
    let now = std::time::Instant::now();
    let mut b = BURNT.lock().unwrap_or_else(PoisonError::into_inner);
    let q = b.entry((dimension.to_owned(), pos)).or_default();
    q.push_back((now, n));
    while q.front().is_some_and(|(t, _)| now.duration_since(*t) > MINUTE) {
        q.pop_front();
    }
}

/// Slice 18, wave 2: what fluid the trash at a place destroyed lately, mB, for its window's fluid
/// bar and «… вёдер» of the minute.
static FLUID_BURNT: LazyLock<Mutex<HashMap<(String, Pos), std::collections::VecDeque<(std::time::Instant, u64, String)>>>> =
    LazyLock::new(Mutex::default);

/// The trash at `pos` destroyed `mb` of `fluid` (rustcraft_fluids).
pub(super) fn note_fluid_burnt(dimension: &str, pos: Pos, fluid: &str, mb: u64) {
    if mb == 0 {
        return;
    }
    let now = std::time::Instant::now();
    let mut b = FLUID_BURNT.lock().unwrap_or_else(PoisonError::into_inner);
    let q = b.entry((dimension.to_owned(), pos)).or_default();
    q.push_back((now, mb, fluid.to_owned()));
    while q.front().is_some_and(|(t, _, _)| now.duration_since(*t) > MINUTE) {
        q.pop_front();
    }
}

/// (mB in the last minute, mB a second over the last two seconds, the fluid last destroyed).
fn fluid_burnt(dimension: &str, pos: Pos) -> (u64, f64, Option<String>) {
    let now = std::time::Instant::now();
    let b = FLUID_BURNT.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(q) = b.get(&(dimension.to_owned(), pos)) else {
        return (0, 0.0, None);
    };
    let minute = q.iter().filter(|(t, _, _)| now.duration_since(*t) <= MINUTE).map(|(_, n, _)| n).sum();
    let recent: u64 = q
        .iter()
        .filter(|(t, _, _)| now.duration_since(*t).as_secs_f64() <= 2.0)
        .map(|(_, n, _)| n)
        .sum();
    let last = q.back().filter(|(t, _, _)| now.duration_since(*t).as_secs_f64() <= 2.0).map(|(_, _, f)| f.clone());
    (minute, recent as f64 / 2.0, last)
}

/// Items the trash at a place burnt in the last second.
fn burnt_last_second(dimension: &str, pos: Pos) -> u32 {
    let now = std::time::Instant::now();
    BURNT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&(dimension.to_owned(), pos))
        .map_or(0, |q| q.iter().filter(|(t, _)| now.duration_since(*t).as_secs_f64() <= 1.0).map(|(_, n)| n).sum())
}

fn burnt_last_minute(dimension: &str, pos: Pos) -> u32 {
    let now = std::time::Instant::now();
    BURNT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&(dimension.to_owned(), pos))
        .map_or(0, |q| q.iter().filter(|(t, _)| now.duration_since(*t) <= MINUTE).map(|(_, n)| n).sum())
}

//// The filters its joints name (allow list or not, the items) and their lowest priority, as the
/// guide's window lists them.
fn trash_joints(game: &Game<'_>, pos: Pos) -> (Vec<(bool, Vec<String>)>, Option<i32>) {
    let mut filters: Vec<(bool, Vec<String>)> = Vec::new();
    let mut priority = None;
    for face in Face::ALL {
        let o = face.offset();
        let housing = [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]];
        let Some(e) = machines::entity(game, housing).filter(|e| e.id == crate::game::rustcraft_content::HOUSING) else {
            continue;
        };
        let joints = crate::game::rustcraft_window::read_joints(&e.nbt);
        for slot in 0..4 {
            if let Some(s) = joints.get(face.opposite(), slot) {
                priority = Some(priority.map_or(s.priority, |p: i32| p.min(s.priority)));
                if tiered::trash_filter_set(s) {
                    let items: Vec<String> = s.filter.items.iter().cloned().collect();
                    if !filters.iter().any(|(a, i)| *a == s.filter.allow_only && *i == items) {
                        filters.push((s.filter.allow_only, items));
                    }
                }
            }
        }
    }
    (filters, priority)
}

/// Slice 18, wave 2: an item's or block's name in the reader's language.
fn name_of(game: &Game<'_>, id: &str) -> Text {
    let kind = if game.items().get(id).is_some_and(|i| i.block.is_some()) { "block" } else { "item" };
    let fallback = id.rsplit(':').next().unwrap_or(id).replace('_', " ");
    Text::translate_with_fallback(&format!("{kind}.{}", id.replace(':', ".")), &fallback, &[])
}

/// Names joined with «, ».
fn names_of(game: &Game<'_>, ids: &[String]) -> Text {
    let mut out = Text::literal("");
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            out.push(Text::literal(", "));
        }
        out.push(name_of(game, id));
    }
    out
}

/// A fluid's bar colour: water blue, lava orange, a solution green.
fn fluid_colour(fluid: &str) -> u32 {
    match fluid {
        "minecraft:water" => 0xFF3F_76E4,
        "minecraft:lava" => 0xFFE0_661A,
        _ => 0xFF59_D38C,
    }
}

/// The share of its step a machine stepping every `every` ticks has done, while it works.
fn step_share(dimension: &str, pos: Pos, every: u64) -> f64 {
    if !shown_at(dimension, pos).is_some_and(|s| s.working()) {
        return 0.0;
    }
    let now = state().ticks.get(dimension).copied().unwrap_or(0);
    (now % every.max(1)) as f64 / every.max(1) as f64
}

/// The rows the guide's windows add (`ui.js` trash, disintegrator) and the charger's charge bar.
fn sheet_extras(game: &Game<'_>, dimension: &str, pos: Pos, u: Util, s: &mut Sheet) {
    match u {
        Util::Trash => trash_sheet(game, dimension, pos, s),
        Util::Disintegrator => disintegrator_sheet(game, dimension, pos, s),
        Util::Charger => charger_sheet(game, pos, s),
        _ => {}
    }
}

/// Slice 18, wave 2 (`ui.js` trash): «Фильтр стыка», «Приоритет», «Сгорело за минуту», the
/// throughput; the fluid bar over the energy bar; «Сгорает сейчас» and the arrow of its batch.
fn trash_sheet(game: &Game<'_>, dimension: &str, pos: Pos, s: &mut Sheet) {
    let lit = |s: String| Text::literal(s);
    let t = crate::game::rustcraft_server::tuning();
    let tier = crate::game::rustcraft_tiers::tier_at(game, pos);
    let mode = machines::entity(game, pos).map_or(Default::default(), |e| machines::read_work(&e.nbt));
    let (rate, _) = tiered::trash_rate(tier, &base::modules_at(game, pos), mode, t);
    let (filters, priority) = trash_joints(game, pos);
    let filter = match filters.first() {
        None => line("rustcraft.util.trash.filter_none", "Joint filter: none (set it with the wrench)", &[]),
        Some((allow, _)) => {
            let all: Vec<String> = filters.iter().flat_map(|(_, i)| i.iter().cloned()).collect();
            let key = if *allow { "rustcraft.util.trash.filter_allow" } else { "rustcraft.util.trash.filter_block" };
            let names = if all.is_empty() {
                line("rustcraft.util.trash.filter_all", "everything", &[])
            } else {
                names_of(game, &all)
            };
            line(key, "Joint filter: allow list: %s", &[names])
        }
    };
    let priority = match priority {
        None => lit("—".to_owned()),
        Some(p) if p <= -rustcraft_rules::net::settings::PRIORITY_LIMIT => {
            line("rustcraft.util.trash.priority_lowest", "the lowest", &[])
        }
        Some(p) => lit(p.to_string()),
    };
    let (fluid_minute, fluid_rate, fluid) = fluid_burnt(dimension, pos);
    let items_minute = burnt_last_minute(dimension, pos);
    let buckets = (fluid_minute as f64 / 1000.0).round() as u64;
    let buckets_a_second = fluid_rate / 1000.0;
    let now = f64::from(burnt_last_second(dimension, pos)) + buckets_a_second;
    s.rows = vec![
        base::v(&filter),
        base::v(&line("rustcraft.util.trash.priority", "Priority: %s", &[priority])),
        // In-game check w3: the guide's key-value rows (the lang puts a tab between them), the
        // buckets only when some burnt, so the value is not cut.
        base::v(&line(
            "rustcraft.util.trash.burnt_row",
            "Burnt in a minute\t%s",
            &[burnt_value(u64::from(items_minute), buckets)],
        )),
        base::v(&line(
            "rustcraft.util.trash.throughput_row",
            "Throughput\t%s of %s",
            &[lit(format!("{now:.0}")), lit(format!("{rate:.0}"))],
        )),
    ];
    s.hints = vec![
        vec![base::v(&line(
            "rustcraft.util.trash.filter_hint",
            "Set it on its joint with the wrench; with none it takes nothing",
            &[],
        ))],
        vec![base::v(&line(
            "rustcraft.util.trash.priority_hint",
            "Only what no storage or machine took comes to it",
            &[],
        ))],
        Vec::new(),
        vec![base::v(&line(
            "rustcraft.util.trash.throughput_hint",
            "8 a second on tier 1, 16, 32 and 64 on the higher ones; a speed module doubles it",
            &[],
        ))],
    ];
    let value = match &fluid {
        Some(f) => line(
            "rustcraft.util.trash.fluid",
            "%s · %s buckets a second",
            &[name_of(game, f), lit(format!("{buckets_a_second:.1}").replace('.', ","))],
        ),
        None => line("rustcraft.util.trash.fluid_none", "nothing comes", &[]),
    };
    s.bars.insert(
        0,
        Bar {
            label: base::v(&line("rustcraft.util.trash.fluid_label", "Fluid", &[])),
            value: base::v(&value),
            share: (buckets_a_second / rate.max(1.0)).clamp(0.0, 1.0),
            energy: false,
            colour: Some(fluid.as_deref().map_or(0xFF3F_76E4, fluid_colour)),
        },
    );
    s.work = Some(step_share(dimension, pos, paced(game, pos, Util::Trash).0));
    s.label = Some(base::v(&line("rustcraft.util.trash.burning", "Burning now", &[])));
}

/// «412 предметов · 6 вёдер», each count in its plural form; no buckets when none burnt.
fn burnt_value(items: u64, buckets: u64) -> Text {
    let counted = |key: &str, fallback: &str, n: u64| {
        let form = rustcraft_rules::machines::panel::plural_form(n);
        line(&format!("{key}.{form}"), fallback, &[Text::literal(n.to_string())])
    };
    let mut v = counted("rustcraft.window.items", "%s items", items);
    if buckets > 0 {
        v.push(Text::literal(" · "));
        v.push(counted("rustcraft.window.buckets", "%s buckets", buckets));
    }
    v
}

/// Slice 18, wave 2 (`ui.js` disintegrator): its input, the arrow and «Разборка» (what comes
/// out, in whole items and fractions); the durability bar; the share and «чары теряются», the
/// kept fractions.
fn disintegrator_sheet(game: &Game<'_>, dimension: &str, pos: Pos, s: &mut Sheet) {
    let lit = |s: String| Text::literal(s);
    let entity = machines::entity(game, pos);
    let item = entity.and_then(|e| e.items.first().cloned().flatten()).filter(|s| s.count > 0);
    let yields = modules::count(&base::modules_at(game, pos), Module::Yield);
    let percent = disassembly::share_percent(yields as usize);
    let (value, share) = match &item {
        Some(st) => {
            let left = durability_left(game, st);
            let state = if left >= 1.0 {
                line("rustcraft.util.disintegrator.whole", "whole", &[])
            } else {
                lit(format!("{:.0}%", left * 100.0))
            };
            (line("rustcraft.util.disintegrator.durability", "%s · %s", &[name_of(game, &st.item), state]), left)
        }
        None => (line("rustcraft.util.disintegrator.taking_none", "Taking apart: nothing", &[]), 0.0),
    };
    s.bars.insert(
        0,
        Bar {
            label: base::v(&line("rustcraft.util.repair.bar", "Durability", &[])),
            value: base::v(&value),
            share,
            energy: false,
            colour: Some(0xFF59_D38C),
        },
    );
    s.preview = match &item {
        Some(st) => {
            if state().recipes.is_none() {
                let r = crafting_recipes(game);
                state().recipes = Some(r);
            }
            let is_charged = charged(game, st);
            let held = state();
            let all = held.recipes.as_deref().unwrap_or_default();
            disassembly::recipe_for(&st.item, all, is_charged)
                .map(|r| disassembly::preview(r, percent, share))
                .unwrap_or_default()
                .into_iter()
                .map(|(i, n)| (i, disassembly::amount_text(n)))
                .collect()
        }
        None => Vec::new(),
    };
    let modules_text = match yields {
        0 => line("rustcraft.util.disintegrator.no_yield", "No yield module", &[]),
        n => line("rustcraft.util.disintegrator.yields", "Yield modules: %s", &[lit(n.to_string())]),
    };
    let kept: BTreeMap<String, u32> = entity.map(|e| read_json(&e.nbt, KEPT_KEY)).unwrap_or_default();
    let mut kept_text = Text::literal("");
    for (i, (id, n)) in kept.iter().enumerate() {
        if i > 0 {
            kept_text.push(Text::literal(", "));
        }
        kept_text.push(name_of(game, id));
        kept_text.push(Text::literal(format!(" {}", disassembly::amount_text(*n))));
    }
    if kept.is_empty() {
        kept_text = lit("—".to_owned());
    }
    s.rows = vec![
        base::v(&line(
            "rustcraft.util.disintegrator.share_lost",
            "Share back %s · enchantments are lost",
            &[lit(format!("{percent}%"))],
        )),
        base::v(&line("rustcraft.util.disintegrator.kept", "Kept: %s", &[kept_text])),
    ];
    s.hints = vec![
        vec![
            base::v(&modules_text),
            base::v(&line(
                "rustcraft.util.disintegrator.share_hint",
                "A yield module: 60%, two 70%, never above 75%",
                &[],
            )),
        ],
        vec![base::v(&line(
            "rustcraft.util.disintegrator.kept_hint",
            "Fractions wait inside until they make a whole item",
            &[],
        ))],
    ];
    s.work = Some(step_share(dimension, pos, paced(game, pos, Util::Disintegrator).0));
    s.label = Some(base::v(&line("rustcraft.util.disintegrator.preview", "Taking apart", &[])));
}

/// Slice 18, wave 2 (`ui.js` charger): «Броня», the charge of what charges now, by its name.
fn charger_sheet(game: &Game<'_>, pos: Pos, s: &mut Sheet) {
    s.rows.clear();
    let slots = items(game, pos);
    let held: Vec<(ItemStack, charge::Battery)> = (0..CHARGER_ARMOUR)
        .filter_map(|i| {
            let st = slots.get(i).cloned().flatten()?;
            battery_of(game, &st).map(|b| (st, b))
        })
        .collect();
    let charging = held.iter().find(|(_, b)| !b.full()).or(held.first());
    let (label, share) = match charging {
        Some((st, b)) => (
            line("rustcraft.util.charger.charge", "Charge: %s", &[name_of(game, &st.item)]),
            if b.capacity > 0.0 { (b.charge / b.capacity).clamp(0.0, 1.0) } else { 1.0 },
        ),
        None => (line("rustcraft.util.charger.charge_none", "Charge: nothing in it", &[]), 0.0),
    };
    s.bars.insert(
        0,
        Bar {
            label: base::v(&label),
            value: base::v(&Text::literal(format!("{:.0}%", share * 100.0))),
            share,
            energy: false,
            colour: Some(0xFF69_D2D8),
        },
    );
    s.label = Some(base::v(&line("rustcraft.util.charger.armour", "Armour", &[])));
}

/// Slice 18, wave 2: the charger's cells for gear (`ui.js` «Броня»); 4..8 are its hidden output.
const CHARGER_ARMOUR: usize = 4;

/// How much of a worn item's durability is left, 0 to 1 (1 for an item that does not wear).
fn durability_left(game: &Game<'_>, st: &ItemStack) -> f64 {
    let max = game.items().get(&st.item).map_or(0, |i| i.max_damage);
    if max == 0 {
        return 1.0;
    }
    f64::from(max.saturating_sub(st.damage)) / f64::from(max)
}

// ---- Slice 18, wave 2: the charger's and the plate's glow (guide `charger`, `charge_plate`) ----

/// `native_block_entity_submit` (API 71): while a charger or a charge plate charges (`lit`), its
/// coils glow full bright in a pulse, drawn over the model each frame
/// (rustcraft_rules::machines::charging::pulse); standing, nothing is drawn and kept. True for
/// these two blocks.
pub(super) fn draw_glow(
    game: &Game<'_>,
    block: &str,
    state: horizon_world::registry::BlockStateId,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) -> bool {
    use crate::game::native_block_entity_draw::CubeFace;
    use rustcraft_rules::machines::charging as c;
    let (coils, uv, sprite): (&[([f32; 3], [f32; 3])], [f32; 4], &str) = match block {
        "rustcraft:charger" => (&c::CHARGER_COILS, c::CHARGER_COIL_UV, "rustcraft:block/charger"),
        "rustcraft:charge_plate" => (&c::PLATE_COILS, c::PLATE_COIL_UV, "rustcraft:block/charge_plate"),
        _ => return false,
    };
    if game.native_block_state_property(state, "lit") != Some("true") {
        draw.keep();
        return true;
    }
    let t = (draw.game_time.rem_euclid(1_200_000) as f32 + draw.partial) / 20.0;
    let tint = c::glow_tint(c::pulse(t));
    let light = draw.light;
    draw.light = 0xF0 | (0xF0 << 8);
    let face = Some(CubeFace { sprite, uv: Some(uv) });
    for coil in coils {
        let (from, to) = c::glow_box(*coil);
        game.native_draw_cube(draw, from, to, [face; 6], tint);
    }
    draw.light = light;
    true
}
