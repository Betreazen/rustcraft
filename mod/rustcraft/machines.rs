// Energy and the machines, every server tick. Each tick reads every machine of the level
// (the one-input processors here; the fuel generator, the alloy smelter and the autocrafter in
// rustcraft_generator, rustcraft_alloy and rustcraft_crafter, the pump in rustcraft_fluids), asks the consumers what they need
// and the crystals and generators what they offer, shares it out by contact (E10) and through
// energy channel nets (E3, capped by the channel's tier, E2, and only through joints whose
// direction lets the port work, M7) with rustcraft_rules::net::energy::supply_report, and runs
// each machine on its share; a generator burns for what it gave and its window sums up its
// circuit (E17).
//
// A machine's items live in its block entity's slots (the engine saves them and its menu shows
// them); its progress and settings in its saved compound; what its window shows in the menu's
// data slots (rustcraft_rules::machines::panel and ::windows). The furnace runs vanilla's
// smelting recipes, looked up as the base game's furnace does (`Recipes::cook`,
// SRC/crates/horizon-client/src/crafting.rs:1968-1987, reached through `Game::recipes`,
// SRC/crates/horizon-client/src/game/world_entry.rs:175-180). The crusher and the alloy smelter
// run the content mod's recipes of type `rustcraft:crushing` and `rustcraft:alloying`
// (data/rustcraft/recipe/), which `native_recipes_of_type` serves for a type outside `minecraft`
// (native_hooks.rs:11341-11356). A fuel burns as long as in vanilla's furnace:
// `Providers::burn_time` on the item's `cooking_fuel` component (SRC/crates/horizon-client-render/
// src/providers.rs:108-113, the call the furnace makes in block_entity_ticks.rs:239-243).
use crate::game::{
    Game, rustcraft_alloy as alloy, rustcraft_anim as anim, rustcraft_base as base,
    rustcraft_content::{self as content, Kind, Machine},
    rustcraft_crafter as crafter, rustcraft_fluids as fluids, rustcraft_generator as generator,
    rustcraft_mining as mining, rustcraft_mixer as mixer, rustcraft_monorail as monorail,
    rustcraft_power as power,
    rustcraft_server::LevelNet,
};
use horizon_client::{
    containers::{BlockEntity, ModBlockEntity},
    items::ItemStack,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::machines::{
    extras::{self, Extras},
    queue,
    sheet::{Cursor, press_socket_in},
    sockets,
};
use rustcraft_rules::tiers;
use rustcraft_rules::{
    geom::Pos,
    machines::{
        alloy::AlloyRecipe,
        catalyst::Tank as CatalystTank,
        crafter::CraftRecipe,
        mixer::MixRecipe,
        panel::{self, Panel},
        processor::{Processor, Recipe, Shown, Spec, State, WorkMode},
        slots::{self, Slot},
    },
    modules::{self, Holder, Module},
    net::{
        energy::{Circuit, Consumer, Ledger, Producer, supply_with_storage},
        joints::{self, Port},
        settings::HousingJoints,
        topology::{ChannelKind, Layout, Topology},
    },
    tuning::Tuning,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::{LazyLock, Mutex, PoisonError},
};

/// The recipe type the furnace runs (the type id `Recipes` keeps, crafting.rs:1720-1734).
pub const SMELTING: &str = "minecraft:smelting";
/// The crusher's and the alloy smelter's recipe types, the content mod's files.
const CRUSHING: &str = "rustcraft:crushing";
const ALLOYING: &str = "rustcraft:alloying";
/// Slice 10: the mixer's recipe type (rustcraft_mixer::parse_recipe).
const MIXING: &str = "rustcraft:mixing";
/// A crushing recipe without `time` takes this many ticks in normal mode.
const CRUSH_TICKS: f64 = 100.0;
/// Saved compound keys of a machine.
const PROGRESS_KEY: &str = "progress";
/// Slice 11: a furnace's or a crusher's catalyst tank (K1): its fluid and how much, mB.
const CATALYST_FLUID_KEY: &str = "catalyst_fluid";
const CATALYST_KEY: &str = "catalyst";
pub const WORK_KEY: &str = "work";
/// The stack size of an item the game does not know.
pub const FALLBACK_STACK: u32 = 64;

/// What the machines of one level did on their last tick, for the diagnosis.
#[derive(Default)]
pub struct Power {
    pub circuits: Vec<Circuit>,
    pub status: HashMap<Pos, Status>,
    /// Furnaces and crushers with nothing to do (no input, or no room for the result), and the
    /// tick they look again: an idle machine is read every IDLE_EVERY ticks, not every tick.
    pub idle: HashMap<Pos, u64>,
}

/// How often a furnace or a crusher with nothing to do looks at its slots again, ticks.
const IDLE_EVERY: u64 = 10;

#[derive(Clone, Copy, Debug)]
pub struct Status {
    pub machine: Machine,
    pub shown: Shown,
}

/// One crushing recipe as its file says it.
#[derive(Clone, Debug)]
pub struct CrushRecipe {
    pub inputs: HashSet<String>,
    pub result: ItemStack,
    pub ticks: f64,
}

/// The machines' recipes, read again with each rescan (a data pack may change them on /reload).
#[derive(Clone, Debug, Default)]
pub struct MachineRecipes {
    pub crushing: Vec<CrushRecipe>,
    /// The alloy smelter's, in the order it tries them (rustcraft_rules::machines::alloy::ordered).
    pub alloying: Vec<AlloyRecipe>,
    /// The autocrafter's patterns as the crafting table matched them, with the result's stack.
    pub crafting: HashMap<[Option<String>; 9], Option<(CraftRecipe, ItemStack)>>,
    /// Slice 10: the mixer's, in file order.
    pub mixing: Vec<MixRecipe>,
}

/// The machines' recipes the game holds now. A file without an ingredient or a result the game
/// knows is left out, and said once in the log.
pub(super) fn load_recipes(game: &Game<'_>) -> MachineRecipes {
    let mut crushing = Vec::new();
    for recipe in game.native_recipes_of_type(CRUSHING) {
        let json = &recipe.json;
        let inputs = game
            .native_recipe_ingredient(&json["ingredient"])
            .map(|i| i.items);
        let result = game.native_recipe_result(&json["result"]);
        let (Some(inputs), Some(result)) = (inputs, result) else {
            warn_once(&recipe.id);
            continue;
        };
        let ticks = json["time"]
            .as_f64()
            .filter(|t| *t >= 1.0)
            .unwrap_or(CRUSH_TICKS);
        crushing.push(CrushRecipe {
            inputs,
            result,
            ticks,
        });
    }
    let mut alloying = Vec::new();
    for recipe in game.native_recipes_of_type(ALLOYING) {
        match alloy::parse_recipe(game, &recipe.json) {
            Some(r) => alloying.push(r),
            None => warn_once(&recipe.id),
        }
    }
    let mut mixing = Vec::new();
    for recipe in game.native_recipes_of_type(MIXING) {
        match mixer::parse_recipe(game, &recipe.json) {
            Some(r) => mixing.push(r),
            None => warn_once(&recipe.id),
        }
    }
    MachineRecipes {
        crushing,
        alloying: rustcraft_rules::machines::alloy::ordered(alloying),
        crafting: HashMap::new(),
        mixing,
    }
}

fn warn_once(id: &str) {
    static WARNED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    if WARNED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(id.to_owned())
    {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: recipe {id} left out: no ingredient or result the game knows");
    }
}

/// The energy nets as circuits: each with the sources and machines on it (joints::energy_members).
pub(super) fn circuits(
    topology: &Topology,
    layout: &Layout,
    ports: &BTreeMap<Pos, Port>,
    settings: &BTreeMap<Pos, HousingJoints>,
    boosts: &BTreeMap<Pos, u8>,
    tuning: &Tuning,
) -> Vec<Circuit> {
    topology
        .nets
        .iter()
        .filter(|net| net.kind == ChannelKind::Energy)
        .map(|net| Circuit {
            members: joints::energy_members(net, layout, ports, settings),
            // Slice 18 (cross, guide `mod_line`): the nodes' line-boost modules (`boosts`, their
            // counts) raise what the circuit's channels let through, as the item nets'.
            cap: tuning.energy_of(net.min_tier)
                * rustcraft_rules::net::node_window::net_boost(&net.housings, &topology.reach, boosts),
        })
        .filter(|c| !c.members.is_empty())
        .collect()
}

// ---- The block entities, read and changed in place ----

/// The mod block entity at `pos`, borrowed: what `native_mod_block_entity` gives
/// (SRC/crates/horizon-client/src/game/native_host_bridge.rs:26-31) without copying its slots,
/// compound and data slots, which the machines' tick did for every machine every tick.
pub(super) fn entity<'a>(game: &'a Game<'_>, pos: Pos) -> Option<&'a ModBlockEntity> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(m)) if !m.id.is_empty() => Some(m),
        _ => None,
    }
}

/// Runs `change` on the mod block entity at `pos` as `native_with_mod_block_entity` does
/// (native_hooks.rs:10719-10739): its chunk is saved next time, and comparators read it again
/// when `items_moved`. That call copies the slots to see whether they changed; the caller knows.
pub(super) fn change_entity<R>(
    game: &mut Game<'_>,
    pos: Pos,
    items_moved: bool,
    change: impl FnOnce(&mut ModBlockEntity) -> R,
) -> Option<R> {
    let key = (pos[0], pos[1], pos[2]);
    let out = match game.world.block_entities.get_mut(&key) {
        Some(BlockEntity::Mod(m)) if !m.id.is_empty() => change(m),
        _ => return None,
    };
    game.world.modified.insert((key.0 >> 4, key.2 >> 4));
    if items_moved {
        game.container_changed(key);
    }
    Some(out)
}

// ---- Slots, fuel and the saved compound, for every machine ----

/// What slot `i` of a block entity holds, as the rules see it.
pub(super) fn slot_of(entity: &ModBlockEntity, i: usize) -> Slot {
    entity
        .items
        .get(i)
        .cloned()
        .flatten()
        .filter(|s| s.count > 0 && &*s.item != "minecraft:air")
        .map(|s| (s.item.to_string(), s.count))
}

/// `stack` with the count `held` says, or nothing when `held` is empty; a stack of another item
/// becomes `made` when that is the item, else a plain stack.
pub(super) fn with_count(
    stack: Option<ItemStack>,
    held: &Slot,
    made: Option<&ItemStack>,
) -> Option<ItemStack> {
    let (item, count) = held.as_ref()?;
    let mut stack = stack
        .filter(|s| &*s.item == item.as_str())
        .or_else(|| made.filter(|m| &*m.item == item.as_str()).cloned())
        .unwrap_or_else(|| ItemStack::new(item, 0));
    stack.count = *count;
    Some(stack)
}

/// Writes the slots `indices` of `entity` whose rules view changed from `before` to `after`.
pub(super) fn write_slots(
    entity: &mut ModBlockEntity,
    indices: impl IntoIterator<Item = usize>,
    before: &[Slot],
    after: &[Slot],
    made: &[ItemStack],
) {
    for (k, i) in indices.into_iter().enumerate() {
        if before.get(k) == after.get(k) {
            continue;
        }
        if entity.items.len() <= i {
            entity.items.resize(i + 1, None);
        }
        let held = after.get(k).cloned().flatten();
        let template = held
            .as_ref()
            .and_then(|(item, _)| made.iter().find(|m| &*m.item == item.as_str()));
        entity.items[i] = with_count(entity.items[i].take(), &held, template);
    }
}

/// Ticks a fuel item burns in a furnace, none for an item that is no fuel.
pub(super) fn burn_ticks(game: &Game<'_>, item: &str) -> Option<f64> {
    let ticks = game
        .res
        .providers()
        .burn_time(game.items(), &ItemStack::new(item, 1), false);
    (ticks > 0).then_some(f64::from(ticks))
}

/// What a used-up fuel leaves in its slot: a lava bucket's empty bucket.
pub(super) fn fuel_remainder(game: &Game<'_>, item: &str) -> Option<ItemStack> {
    let info = game.items().get(item)?;
    let rest = info.crafting_remainder.as_deref()?;
    Some(ItemStack::new(rest, 1))
}

/// How many of `item` a slot holds.
pub(super) fn max_stack(game: &Game<'_>, item: &str) -> u32 {
    game.items()
        .get(item)
        .map_or(FALLBACK_STACK, |info| info.max_stack)
}

/// `key` in UTF-16 in a buffer kept per thread, to look it up in a compound without making a key
/// each time (the machines' tick reads a few keys of every machine every tick). As
/// horizon-world's own `with_key` for `Tag::get` (SRC/crates/horizon-world/src/nbt.rs:13-27).
fn with_key<R>(key: &str, f: impl FnOnce(&JavaString) -> R) -> R {
    thread_local! {
        static KEY: std::cell::RefCell<JavaString> = Default::default();
    }
    KEY.with_borrow_mut(|units| {
        units.0.clear();
        units.0.extend(key.encode_utf16());
        f(units)
    })
}

/// The tag under `key`, looked up without making a key.
pub(super) fn tag<'a>(nbt: &'a Compound, key: &str) -> Option<&'a Tag> {
    with_key(key, |k| nbt.get(k))
}

/// Sets `key` to `value`: in place when it is there, so no key is made for each write.
fn set_tag(nbt: &mut Compound, key: &str, value: Tag) {
    let missing = with_key(key, |k| match nbt.get_mut(k) {
        Some(t) => {
            *t = value;
            None
        }
        None => Some(value),
    });
    if let Some(value) = missing {
        nbt.insert(JavaString::from(key), value);
    }
}

pub(super) fn read_f64(nbt: &Compound, key: &str) -> f64 {
    match tag(nbt, key) {
        Some(Tag::Double(bits)) => f64::from_bits(*bits),
        _ => 0.0,
    }
}

pub(super) fn write_f64(nbt: &mut Compound, key: &str, value: f64) {
    set_tag(nbt, key, Tag::Double(value.to_bits()));
}

pub(super) fn read_int(nbt: &Compound, key: &str) -> Option<i32> {
    match tag(nbt, key) {
        Some(Tag::Int(v)) => Some(*v),
        Some(Tag::Byte(v)) => Some(i32::from(*v)),
        _ => None,
    }
}

pub(super) fn write_int(nbt: &mut Compound, key: &str, value: i32) {
    set_tag(nbt, key, Tag::Int(value));
}

// ---- The tick ----

/// A machine as read this tick.
enum Unit {
    Processor(Held),
    Generator(generator::Held),
    Alloy(alloy::Held),
    Crafter(crafter::Held),
    Pump(fluids::PumpHeld),
    Station(monorail::StationHeld),
    /// The turret and the charge dock: a buffer that fills on energy.
    Buffer(BufferHeld),
    /// The anchor: what it asks for the chunks it would hold.
    Anchor(f64),
    /// Slice 8: a solar panel, a windmill or a lava generator, and what it offers.
    Power(power::SourceHeld),
    /// The miner, the placer and the breaker.
    Mining(mining::Held),
    /// Slice 10: the mixer.
    Mixer(mixer::Held),
}

/// A buffer of energy as the machines' tick reads it (the station's, for the turret and the
/// charge dock).
pub struct BufferHeld {
    pub energy: f64,
    pub capacity: f64,
    pub request: f64,
}

/// The buffer's size and the most it fills a second.
pub(super) fn buffer_spec(machine: Machine, tuning: &Tuning) -> (f64, f64) {
    match machine {
        Machine::Turret => (tuning.turret_buffer, tuning.turret_charge),
        Machine::Util(u) => (u.spec().buffer, u.spec().draw),
        // Slice 18 (stream 5): the butcher's buffer, 4 E/s (guide `butcher`).
        Machine::Butcher => (rustcraft_rules::butcher::BUFFER, rustcraft_rules::butcher::DRAW),
        _ => (tuning.dock_buffer, tuning.dock_draw),
    }
}

/// The energy key of a buffer's block entity.
pub(super) const BUFFER_KEY: &str = "energy";

pub(super) fn buffer_energy(game: &Game<'_>, pos: Pos) -> f64 {
    entity(game, pos).map_or(0.0, |e| read_f64(&e.nbt, BUFFER_KEY))
}

pub(super) fn set_buffer_energy(game: &mut Game<'_>, pos: Pos, energy: f64) -> bool {
    change_entity(game, pos, false, |e| {
        write_f64(&mut e.nbt, BUFFER_KEY, energy.max(0.0))
    })
    .is_some()
}

fn read_buffer(game: &Game<'_>, pos: Pos, machine: Machine, tuning: &Tuning) -> Option<BufferHeld> {
    let entity = entity(game, pos)?;
    let (capacity, charge) = buffer_spec(machine, tuning);
    let energy = read_f64(&entity.nbt, BUFFER_KEY).clamp(0.0, capacity);
    let room = (capacity - energy).max(0.0) * tuning.ticks_per_second;
    Some(BufferHeld {
        energy,
        capacity,
        request: charge.min(room),
    })
}

/// One tick of every machine of the level in place.
pub(super) fn tick(game: &mut Game<'_>, level: &mut LevelNet, tuning: &Tuning) {
    if level.machines.is_empty() {
        level.power.status.clear();
        return;
    }
    power::before_tick(game, level, tuning);
    let ticks = level.ticks;
    // The idle machines that rest this tick keep what they showed.
    let resting: HashMap<Pos, Status> = level
        .power
        .idle
        .iter()
        .filter(|&(_, &until)| ticks < until)
        .filter_map(|(p, _)| level.power.status.get(p).map(|s| (*p, *s)))
        .collect();
    let units: Vec<(Pos, Machine, Unit)> = level
        .machines
        .iter()
        .map(|(&pos, &machine)| (pos, machine))
        .filter(|(pos, _)| !resting.contains_key(pos))
        .filter_map(|(pos, machine)| {
            let unit = match machine.kind() {
                Kind::Processor => {
                    Unit::Processor(read(game, pos, machine, &level.recipes, tuning)?)
                }
                Kind::Generator => Unit::Generator(generator::read(game, pos)?),
                Kind::Alloy => Unit::Alloy(alloy::read(game, pos, &level.recipes, tuning)?),
                Kind::Crafter => Unit::Crafter(crafter::read(
                    game,
                    pos,
                    &mut level.recipes,
                    level.temp_pages.get(&pos),
                    tuning,
                )?),
                Kind::Pump => Unit::Pump(fluids::read_pump(game, pos, tuning)?),
                Kind::Station => Unit::Station(monorail::read_station(game, pos, tuning)?),
                Kind::Charger | Kind::Turret | Kind::Util | Kind::Butcher => {
                    Unit::Buffer(read_buffer(game, pos, machine, tuning)?)
                }
                Kind::Anchor => Unit::Anchor(base::anchor_request(game, pos, tuning)),
                Kind::Power => Unit::Power(power::read_source(
                    game,
                    &level.sources,
                    pos,
                    machine,
                    tuning,
                )?),
                Kind::Mining => Unit::Mining(mining::read(game, pos, machine, tuning)?),
                Kind::Mixer => Unit::Mixer(mixer::read(game, pos, &level.recipes, tuning)?),
                Kind::Dock | Kind::Battery | Kind::LoadDock => return None,
            };
            Some((pos, machine, unit))
        })
        .collect();
    let consumers: Vec<Consumer> = units
        .iter()
        .filter_map(|(pos, _, unit)| {
            let request = match unit {
                Unit::Processor(h) => h.job.as_ref().map_or(0.0, |j| {
                    h.processor
                        .request(std::slice::from_ref(&j.recipe), &j.spec)
                }),
                Unit::Alloy(h) => h.request,
                Unit::Crafter(h) => h.request,
                Unit::Pump(h) => h.request,
                Unit::Station(h) => h.request,
                Unit::Buffer(h) => h.request,
                Unit::Anchor(r) => *r,
                Unit::Mining(h) => h.request,
                Unit::Mixer(h) => h.request,
                Unit::Generator(_) | Unit::Power(_) => return None,
            };
            Some(Consumer {
                block: *pos,
                request,
            })
        })
        .collect();
    // Crystals by their structures (E7, E8), the sources of slice 8 by what they offer now.
    let producers: Vec<Producer> = power::crystal_producers(game, &level.sources, tuning)
        .into_iter()
        .chain(units.iter().filter_map(|(pos, _, unit)| match unit {
            Unit::Generator(h) => Some(Producer {
                block: *pos,
                output: generator::offer(game, h, tuning),
            }),
            Unit::Power(h) => Some(Producer {
                block: *pos,
                output: h.output,
            }),
            _ => None,
        }))
        .collect();
    let stores = power::stores(game, &level.sources, tuning);
    let circuits = &level.power.circuits;
    let report = supply_with_storage(
        &producers,
        &consumers,
        &stores,
        circuits,
        1.0 / tuning.ticks_per_second,
    );
    // Slice 18: with what it needs for the windows' circuit balance.
    power::settle(
        game,
        &mut level.sources,
        &report,
        &producers,
        &consumers,
        circuits,
        tuning,
    );
    // s18/pipe: the node's window shows the balance of its line's circuits (rustcraft_node_window).
    if level.ticks.is_multiple_of(20) {
        level.pipe18_energy = Some((Ledger::new(&producers, &consumers), report.got.clone()));
    }
    let requests: HashMap<Pos, f64> = consumers.iter().map(|c| (c.block, c.request)).collect();
    let request_of = |pos: &Pos| requests.get(pos).copied().unwrap_or(0.0);
    let mut status = resting;
    // Made for the first generator, if any.
    let mut ledger: Option<Ledger> = None;
    for (pos, machine, unit) in units {
        let request = request_of(&pos);
        let received = report.got.get(&pos).copied().unwrap_or(0.0);
        let share = if request > 0.0 {
            (received / request).min(1.0)
        } else {
            0.0
        };
        // Slice 12: whether its moving parts move (rustcraft_anim), told to clients on a change.
        let on = match &unit {
            Unit::Generator(_) => report.used.get(&pos).copied().unwrap_or(0.0) > 0.0,
            Unit::Power(h) => h.output > 0.0,
            _ => share > 0.0 && request > 0.0,
        };
        // The slice-12 machines say it from their own work (rustcraft_utility).
        if !matches!(machine, Machine::Util(_)) && level.anim.get(&pos) != Some(&on) {
            anim::show_on(game, pos, on);
            level.anim.insert(pos, on);
        }
        let shown = match unit {
            Unit::Processor(h) => {
                if request > 0.0 {
                    level.power.idle.remove(&pos);
                } else {
                    level.power.idle.insert(pos, ticks + IDLE_EVERY);
                }
                run(game, pos, h, share, request, tuning)
            }
            Unit::Generator(h) => {
                let used = report.used.get(&pos).copied().unwrap_or(0.0);
                let ledger = ledger.get_or_insert_with(|| Ledger::new(&producers, &consumers));
                let members = ledger.group_of(pos, circuits);
                let balance = ledger.balance(&members, &report.got);
                // Slice 18: what the batteries of its circuit hold (rustcraft_power).
                let balance = power::with_storage(&level.sources, &members, balance);
                generator::run(game, pos, h, used, balance, tuning)
            }
            Unit::Alloy(h) => alloy::run(game, pos, h, share, &level.recipes, tuning),
            Unit::Crafter(h) => {
                crafter::run(game, pos, h, share, level.temp_pages.get_mut(&pos), tuning)
            }
            Unit::Pump(h) => {
                let no_lane = level.fluids.no_lane(pos, level.ticks).is_some();
                fluids::run_pump(game, pos, h, share, no_lane, tuning)
            }
            Unit::Station(h) => {
                monorail::run_station(game, pos, h, share, tuning);
                None
            }
            Unit::Buffer(h) => {
                let got = share * h.request / tuning.ticks_per_second;
                if got > 0.0 {
                    set_buffer_energy(game, pos, (h.energy + got).min(h.capacity));
                }
                None
            }
            Unit::Anchor(_) => {
                base::anchor_supplied(pos, share);
                None
            }
            Unit::Power(h) => {
                let used = report.used.get(&pos).copied().unwrap_or(0.0);
                power::run_source(game, &mut level.sources, pos, h, used, tuning);
                None
            }
            Unit::Mining(h) => {
                mining::run(game, pos, machine, h, share, tuning);
                None
            }
            Unit::Mixer(h) => {
                mixer::run(game, pos, h, share, &level.recipes, tuning);
                None
            }
        };
        if let Some(shown) = shown {
            status.insert(pos, Status { machine, shown });
        }
    }
    level.power.status = status;
}

// ---- The one-input machines: the electric furnace and the crusher ----

/// A machine's work this tick: the recipe its input has, how long it takes and what it costs.
struct Job {
    recipe: Recipe,
    /// The result as the recipe makes it, components and all.
    result: ItemStack,
    spec: Spec,
}

/// What a one-input machine holds, read without changing it.
struct Held {
    processor: Processor,
    job: Option<Job>,
    /// E a second it asks in its mode when it has work: what its window shows as «из Y».
    nominal: f64,
    /// The data slots its menu shows now.
    data: Vec<i32>,
    /// Slice 11: its catalyst tank.
    catalyst: CatalystTank,
    /// Slice 18: its modules, tier and hidden cells (rustcraft_rules::machines::queue).
    fitted: Fitted,
    /// The input slot after the intake moved it into the queue, as read and as now.
    intake_before: Slot,
    intake: Slot,
    cells_before: Vec<Slot>,
    cells: Vec<Slot>,
    open_cells: usize,
    /// The queue cell the processor works from this tick; none: the input slot.
    front: Option<usize>,
    /// The result slot and its reserve as read.
    output_before: Slot,
    reserve_before: Slot,
    /// How many of the result one real slot holds.
    output_max: u32,
}

/// Slice 11: the catalyst tank a furnace's or a crusher's saved compound holds.
pub(super) fn read_catalyst(nbt: &Compound) -> CatalystTank {
    let fluid = match tag(nbt, CATALYST_FLUID_KEY) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    };
    let amount = read_f64(nbt, CATALYST_KEY).max(0.0);
    CatalystTank {
        fluid: fluid.filter(|_| amount > 0.0),
        amount,
    }
}

pub(super) fn write_catalyst(nbt: &mut Compound, tank: &CatalystTank) {
    nbt.insert(
        JavaString::from(CATALYST_FLUID_KEY),
        Tag::String(JavaString::from(tank.fluid.as_deref().unwrap_or(""))),
    );
    write_f64(nbt, CATALYST_KEY, tank.amount);
}

/// The machines that take a catalyst (K1): slice 11's electric furnace and crusher, slice 18's
/// alloy smelter, autocrafter and pump (rustcraft_rules::machines::catalyst::TAKERS).
pub(super) fn takes_catalyst(block: &str) -> bool {
    rustcraft_rules::machines::catalyst::TAKERS.contains(&block)
}

/// Slice 18 (cross): the (speed, energy) multipliers the catalyst in `tank` gives a machine
/// `block`; none (1, 1) when it holds none or it does nothing there.
pub(super) fn catalyst_factors(block: &str, tank: &CatalystTank, tuning: &Tuning) -> (f64, f64) {
    tank.active()
        .filter(|c| c.acts_in(block))
        .map_or((1.0, 1.0), |c| c.factors(tuning))
}

/// Slice 18 (cross): whether the enricher in `tank` gives a machine `block` one more product
/// this time, on the roll `roll` (0 to 1).
pub(super) fn catalyst_lucky(block: &str, tank: &CatalystTank, roll: f64, tuning: &Tuning) -> bool {
    tank.active()
        .is_some_and(|c| c.acts_in(block) && roll < c.extra_chance(tuning))
}

/// Slice 11: the catalyst tank of the machine at `pos`, if it takes one.
pub(super) fn catalyst_at(game: &Game<'_>, pos: Pos) -> Option<CatalystTank> {
    let entity = entity(game, pos)?;
    takes_catalyst(&entity.id).then(|| read_catalyst(&entity.nbt))
}

/// Slice 11: writes a machine's catalyst tank back.
pub(super) fn set_catalyst(game: &mut Game<'_>, pos: Pos, tank: &CatalystTank) {
    change_entity(game, pos, false, |e| write_catalyst(&mut e.nbt, tank));
}

/// A number in [0, 1) for the enricher's chance: xorshift over a seed from the clock.
pub(super) fn roll() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(0);
    let mut x = SEED.load(Ordering::Relaxed);
    if x == 0 {
        x = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9_7F4A_7C15, |d| d.as_nanos() as u64)
            | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// The recipe `machine` runs on `input`, if it has one.
fn job_of(
    game: &Game<'_>,
    machine: Machine,
    input: &ItemStack,
    recipes: &MachineRecipes,
    tuning: &Tuning,
    fitted: &Fitted,
) -> Option<Job> {
    let (result, ticks, energy) = match machine {
        Machine::Furnace => {
            let cooking = game.recipes().cook(SMELTING, input)?;
            let ticks = (f64::from(cooking.time) / tuning.furnace_speedup).max(1.0);
            (cooking.result.clone(), ticks, tuning.furnace_energy)
        }
        Machine::Crusher => {
            let r = recipes
                .crushing
                .iter()
                .find(|r| r.inputs.contains(&*input.item))?;
            (r.result.clone(), r.ticks, tuning.crusher_energy)
        }
        _ => return None,
    };
    // A8: a higher tier works faster for the same energy a recipe; the stack module's 256.
    let speed = fitted.speed(machine, tuning);
    let (ticks, energy) = ((ticks / speed).max(1.0), energy * speed);
    let output_capacity = modules::machine_stack(&fitted.modules, max_stack(game, &result.item));
    Some(Job {
        recipe: Recipe {
            input: input.item.to_string(),
            input_count: 1,
            output: result.item.to_string(),
            output_count: result.count,
        },
        spec: Spec {
            ticks,
            energy,
            output_capacity,
        },
        result,
    })
}

/// The energy a machine of `machine` asks in normal mode.
fn base_energy(machine: Machine, tuning: &Tuning) -> f64 {
    match machine {
        Machine::Crusher => tuning.crusher_energy,
        _ => tuning.furnace_energy,
    }
}

/// The machine at `pos` as its block entity holds it, and the recipe its input has. Slice 18:
/// its input slot moves into its queue cells first, and it works from the first cell with a
/// recipe; its result counts with its reserve.
fn read(
    game: &Game<'_>,
    pos: Pos,
    machine: Machine,
    recipes: &MachineRecipes,
    tuning: &Tuning,
) -> Option<Held> {
    let entity = entity(game, pos)?;
    let fitted = fitted(game, pos, entity);
    let slot = |i: usize| entity.items.get(i).and_then(Option::as_ref);
    let catalyst = read_catalyst(&entity.nbt);
    let intake_before = slot_of(entity, content::MACHINE_INPUT);
    let output_before = slot_of(entity, content::MACHINE_OUTPUT);
    let reserve_before: Slot = crate::game::rustcraft_utility::read_json(&entity.nbt, RESERVE_KEY);
    let cells_before: Vec<Slot> =
        crate::game::rustcraft_utility::read_json(&entity.nbt, QUEUE_KEY);
    let open_cells = fitted.open_cells();
    let mut intake = intake_before.clone();
    let mut cells = cells_before.clone();
    if let Some((item, _)) = &intake {
        let size = modules::machine_stack(&fitted.modules, max_stack(game, item));
        queue::intake(&mut intake, &mut cells, open_cells, size);
    }
    queue::spill(&mut intake, &mut cells, open_cells, |i| max_stack(game, i));
    let has_job = |item: &str| {
        job_of(game, machine, &ItemStack::new(item, 1), recipes, tuning, &fitted).is_some()
    };
    let front = queue::front(&cells, has_job);
    let work: Slot = front.map_or_else(|| intake.clone(), |k| cells[k].clone());
    // The real stack (components and all) when the processor works from the input slot.
    let input_stack = match (front, slot(content::MACHINE_INPUT)) {
        (None, Some(s)) if intake.is_some() => Some(s.clone()),
        _ => work.as_ref().map(|(i, n)| ItemStack::new(i, *n)),
    };
    let processor = Processor {
        input: work,
        output: queue::join(&output_before, &reserve_before),
        progress: read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0),
        mode: read_work(&entity.nbt),
        modules: fitted.processor_modules(),
        catalyst: catalyst.active().map(|c| c.factors(tuning)),
        lucky: false,
    };
    let job =
        input_stack.and_then(|input| job_of(game, machine, &input, recipes, tuning, &fitted));
    let output_max = processor
        .output
        .as_ref()
        .map(|(i, _)| i.as_str())
        .or(job.as_ref().map(|j| &*j.result.item))
        .map_or(FALLBACK_STACK, |i| max_stack(game, i));
    let nominal =
        base_energy(machine, tuning) * fitted.speed(machine, tuning) * processor.factors().1;
    Some(Held {
        processor,
        job,
        nominal,
        data: entity.data.clone(),
        catalyst,
        fitted,
        intake_before,
        intake,
        cells_before,
        cells,
        open_cells,
        front,
        output_before,
        reserve_before,
        output_max,
    })
}

/// Runs the machine's processor on `share` of its `request` and writes back what changed: the
/// input's count, the result, the progress and the window's data slots. What it shows; none
/// when the block entity is gone.
fn run(
    game: &mut Game<'_>,
    pos: Pos,
    held: Held,
    share: f64,
    request: f64,
    tuning: &Tuning,
) -> Option<Shown> {
    let Held {
        mut processor,
        job,
        nominal,
        data: shown_data,
        mut catalyst,
        fitted,
        intake_before,
        mut intake,
        cells_before,
        mut cells,
        open_cells,
        front,
        output_before,
        reserve_before,
        output_max,
    } = held;
    let recipes: Vec<Recipe> = job.iter().map(|j| j.recipe.clone()).collect();
    // Without a job any spec does: the processor only resets its progress.
    let spec = job.as_ref().map_or(
        Spec {
            ticks: 1.0,
            energy: 0.0,
            output_capacity: 0,
        },
        |j| j.spec,
    );
    // The enricher: each finished recipe has its chance of one more product.
    processor.lucky = catalyst
        .active()
        .is_some_and(|c| roll() < c.extra_chance(tuning))
        || modules::yield_lucky(&fitted.modules, roll());
    let before = processor.clone();
    let state = processor.tick(&recipes, &spec, share);
    // Slice 18 (cross): what the electric furnace gave goes to Path-of-craft-tree (`smelt`).
    report_smelt(game, pos, &before.output, &processor.output);
    let working = state == State::Working;
    let catalyst_before = catalyst.clone();
    if working {
        catalyst.spend(1.0 / tuning.ticks_per_second, share, tuning);
    }
    let shown = Shown::of(state, share);
    // Slice 18: what the processor worked from goes back to its cell, the result to its slot and
    // reserve.
    match front {
        Some(k) => cells[k] = processor.input.clone(),
        None => intake = processor.input.clone(),
    }
    let (output, reserve) = queue::split(&processor.output, output_max);
    let mut data = panel::encode(&Panel {
        progress: processor.progress,
        got: if working { share * request } else { 0.0 },
        asked: nominal,
        mode: processor.mode,
        shown,
    });
    data.extend(fitted.extras(game, &cells, open_cells, &reserve));
    // An idle machine changes nothing: leave its chunk alone (each call marks it for saving,
    // native_hooks.rs:10719-10739).
    let hidden_moved = cells != cells_before || reserve != reserve_before;
    if processor == before
        && shown_data == data
        && catalyst == catalyst_before
        && intake == intake_before
        && !hidden_moved
        && fitted.tier_saved()
    {
        return Some(shown);
    }
    let result: Vec<ItemStack> = job.map(|j| j.result).into_iter().collect();
    let moved = intake != intake_before || output != output_before;
    change_entity(game, pos, moved, move |entity| {
        write_slots(
            entity,
            [content::MACHINE_INPUT, content::MACHINE_OUTPUT],
            &[intake_before, output_before],
            &[intake, output],
            &result,
        );
        if hidden_moved {
            crate::game::rustcraft_utility::write_json(&mut entity.nbt, QUEUE_KEY, &cells);
            crate::game::rustcraft_utility::write_json(&mut entity.nbt, RESERVE_KEY, &reserve);
        }
        fitted.save_tier(&mut entity.nbt);
        if processor.progress != before.progress {
            write_f64(&mut entity.nbt, PROGRESS_KEY, processor.progress);
        }
        if catalyst != catalyst_before {
            write_catalyst(&mut entity.nbt, &catalyst);
        }
        if entity.data != data {
            entity.data = data;
        }
        shown
    })
}

/// Slice 18 (cross): the `smelt` report of the electric furnace at `pos` for what its output
/// gained (docs/design/path-of-craft-tree-contract.md); the crusher reports nothing.
fn report_smelt(game: &Game<'_>, pos: Pos, before: &Option<(String, u32)>, after: &Option<(String, u32)>) {
    fn as_ref(o: &Option<(String, u32)>) -> Vec<(&str, u32)> {
        o.as_ref().map(|(i, n)| (i.as_str(), *n)).into_iter().collect()
    }
    let gained = rustcraft_rules::reports::gained(as_ref(before), as_ref(after));
    if gained.is_empty() || game.native_block(pos) != content::FURNACE {
        return;
    }
    if let Some(owner) = base::owner_at(game, pos) {
        for (item, count) in gained {
            base::report(rustcraft_rules::reports::Report::Smelt { owner, item, count });
        }
    }
}

/// The work mode (A1) the machine saved; normal when none.
pub(super) fn read_work(nbt: &Compound) -> WorkMode {
    read_int(nbt, WORK_KEY)
        .and_then(|code| u8::try_from(code).ok())
        .and_then(WorkMode::from_code)
        .unwrap_or_default()
}

pub(super) fn write_work(nbt: &mut Compound, mode: WorkMode) {
    nbt.insert(JavaString::from(WORK_KEY), Tag::Byte(mode.code() as i8));
}

// ---- The windows' buttons ----

/// `native_menu_button` (native_hooks.rs:4272-4285, through `mod_menu_button`,
/// native_hooks.rs:12133-12146): a button of a machine's window. A one-input machine's mode
/// button (its id is `WorkMode::code`, rustcraft_screens::on_click) sets its work mode, shown
/// with the next data slots; the alloy smelter's and the autocrafter's go to their modules.
pub(super) fn on_menu_button(entity: &mut ModBlockEntity, id: i32, tuning: &Tuning) {
    let Some(machine) = Machine::of(&entity.id) else {
        return;
    };
    match machine.kind() {
        Kind::Processor => {
            let Some(mode) = u8::try_from(id).ok().and_then(WorkMode::from_code) else {
                return;
            };
            write_work(&mut entity.nbt, mode);
            let mut shown = panel::decode(&entity.data);
            shown.mode = mode;
            replace_front(&mut entity.data, panel::encode(&shown));
        }
        Kind::Alloy => alloy::on_menu_button(entity, id),
        Kind::Crafter => crafter::on_menu_button(entity, id, tuning),
        Kind::Pump => {
            // Owner 2026-11-17: the pump's work modes, as a processor's; its window's mode slot
            // shows the press at once.
            let Some(mode) = u8::try_from(id).ok().and_then(WorkMode::from_code) else {
                return;
            };
            write_work(&mut entity.nbt, mode);
            use rustcraft_rules::machines::fluid_windows::pump as pw;
            if entity.data.len() > pw::MODE_AT {
                entity.data[pw::MODE_AT] = pw::mode_slot(mode);
            }
        }
        _ => {}
    }
}

/// `native_menu_button_v2` (native_hooks.rs:4286-4297): the same press with the player's cursor
/// stack, which a pattern cell of the autocrafter takes as its item.
pub(super) fn on_menu_button_v2(
    entity: &mut ModBlockEntity,
    id: i32,
    carried: &mut Option<ItemStack>,
    tuning: &Tuning,
) {
    // Slice 18: a socket of a drawn window takes or gives a module, as a sheet's does.
    if press_socket(entity, id, carried) {
        return;
    }
    if Machine::of(&entity.id) == Some(Machine::Crafter) {
        crafter::on_cell_button(entity, id, carried, tuning);
    }
}

// ---- What each input takes ----

/// The item lists an input slot is held to, by the net, hoppers and players alike: refreshed
/// with each rescan, read where no level is at hand (`native_container_accepts`, the slot rules).
#[derive(Clone, Debug, Default, PartialEq)]
struct Accepts {
    smeltable: BTreeSet<String>,
    crushable: BTreeSet<String>,
    alloying: BTreeSet<String>,
    fuels: BTreeSet<String>,
    /// Items that place a block: what the block placer takes.
    placeable: BTreeSet<String>,
    /// Slice 10: the mixer's ingredients.
    mixing: BTreeSet<String>,
}

static ACCEPTS: LazyLock<Mutex<Accepts>> = LazyLock::new(Mutex::default);

fn accepts() -> std::sync::MutexGuard<'static, Accepts> {
    ACCEPTS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether slot `slot` of the machine `entity` takes `item` from the net or a hopper: the
/// furnace what it smelts, the crusher what it crushes, a fuel slot fuel, an alloy input an
/// alloy ingredient and a buffer slot an ingredient of the crafter's pages, one kind a slot.
pub(super) fn slot_takes(
    entity: &ModBlockEntity,
    slot: usize,
    item: &str,
    tuning: &Tuning,
) -> bool {
    let Some(machine) = Machine::of(&entity.id) else {
        return true;
    };
    if !machine.inputs().contains(&slot) {
        return false;
    }
    let a = accepts();
    match machine {
        Machine::Furnace => a.smeltable.contains(item),
        Machine::Crusher => a.crushable.contains(item),
        Machine::Generator => a.fuels.contains(item),
        Machine::Placer => a.placeable.contains(item),
        // The load dock takes anything a cargo worker may carry.
        Machine::DockLoad => true,
        // Slice 11: the butcher's sword slot takes a sword (K5).
        Machine::Butcher => rustcraft_rules::butcher::is_sword(item),
        // Slice 12: each slot as its machine says; the trash by its filter slots (UT2).
        Machine::Util(u) => {
            drop(a);
            // Slice 18 (stream 5): a capacity machine's rule reads its module items instead.
            let filter: Vec<String> = if rustcraft_rules::utility::has_capacity(u) {
                crate::game::rustcraft_utility::module_items(entity)
            } else {
                (0..9)
                    .filter_map(|i| entity.items.get(i).cloned().flatten())
                    .filter(|s| s.count > 0)
                    .map(|s| s.item.to_string())
                    .collect()
            };
            // Slice 18, wave 2 (guide `trash`): only what its joints' filters let through, by its
            // copy of them (a hopper above it puts nothing else).
            if u == rustcraft_rules::utility::Util::Trash {
                let filters = crate::game::rustcraft_utility::trash_filters(entity);
                return u.slots().inputs.contains(&slot)
                    && rustcraft_rules::machines::tiered::trash_passes(&filters, item);
            }
            u.takes(slot, item, &filter)
        }
        Machine::Mixer => {
            let inputs: Vec<Slot> = (0..content::MIXER_SLOTS)
                .map(|i| slot_of(entity, i))
                .collect();
            a.mixing.contains(item) && slots::one_kind_takes(&inputs, slot, item)
        }
        Machine::Alloy if slot == content::ALLOY_FUEL => a.fuels.contains(item),
        Machine::Alloy => {
            let inputs: Vec<Slot> = content::ALLOY_INPUTS.map(|i| slot_of(entity, i)).collect();
            a.alloying.contains(item) && slots::one_kind_takes(&inputs, slot, item)
        }
        Machine::Crafter => {
            drop(a);
            crafter::buffer_takes(entity, slot, item, tuning)
        }
        _ => false,
    }
}

/// `native_container_accepts` (native_hooks.rs:3565-3592): a hopper or the net about to put
/// `stack` into slot `slot` of a machine (`mod_container_accepts`, machines.rs:616-638, asked by
/// `move_in_item` for every item `native_move_items` moves).
pub(super) fn on_container_accepts(
    entity: &ModBlockEntity,
    slot: usize,
    stack: &ItemStack,
    cancel: &mut bool,
    tuning: &Tuning,
) {
    if !*cancel
        && Machine::of(&entity.id).is_some()
        && !slot_takes(entity, slot, &stack.item, tuning)
    {
        *cancel = true;
    }
}

/// The name a call of `native_menu_slot_rules` carries when the server teaches the windows'
/// slot rules what a slot takes: TEACH, the menu, `#`, the slot (hook_menu_slot_rules.rs, which
/// shares the format).
const TEACH: &str = "rustcraft:teach_slot_rules/";
const TEACH_CLEAR: usize = 0;
const TEACH_ADD: usize = 1;

/// Refreshes what each input takes (for `slot_takes`) and tells the windows' slot rules: what a
/// player may put into each input slot. `native_menu_slot_rules` lives in horizon-client-lib,
/// which neither this module nor the rules crate reach, and is asked from the stack alone
/// (SRC/docs/native-mods.md:4615-4627), so the server sends the lists through the function
/// itself, under a menu name no menu has, and the helper keeps them. Again only when the lists
/// changed. The autocrafter's buffer takes anything from a player's hand (its pages' ingredients
/// change while its window is open); its module slots take only modules.
pub(super) fn teach_slot_rules(game: &Game<'_>, recipes: &MachineRecipes) {
    let smeltable: BTreeSet<String> = game
        .recipes()
        .cooking
        .iter()
        .filter(|c| c.kind == SMELTING)
        .flat_map(|c| c.ingredient.items.iter().cloned())
        .collect();
    let crushable: BTreeSet<String> = recipes
        .crushing
        .iter()
        .flat_map(|r| r.inputs.iter().cloned())
        .collect();
    let alloying: BTreeSet<String> = recipes
        .alloying
        .iter()
        .flat_map(|r| r.inputs.iter().map(|(i, _)| i.clone()))
        .collect();
    let fuels: BTreeSet<String> = game
        .items()
        .items
        .keys()
        .filter(|item| burn_ticks(game, item).is_some())
        .map(|item| item.to_string())
        .collect();
    let placeable: BTreeSet<String> = game
        .items()
        .items
        .iter()
        .filter(|(_, info)| info.block.is_some())
        .map(|(item, _)| item.to_string())
        .filter(|item| item != "minecraft:air")
        .collect();
    let mixing: BTreeSet<String> = rustcraft_rules::machines::mixer::ingredients(&recipes.mixing)
        .into_iter()
        .collect();
    let next = Accepts {
        smeltable,
        crushable,
        alloying,
        fuels,
        placeable,
        mixing,
    };
    {
        let mut a = accepts();
        if *a == next {
            return;
        }
        *a = next.clone();
    }
    let modules = BTreeSet::from([content::MODULE_CAPACITY.to_owned()]);
    let none = BTreeSet::new();
    let mut lists: Vec<(&str, usize, &BTreeSet<String>)> = vec![
        (
            Machine::Furnace.menu(),
            content::MACHINE_INPUT,
            &next.smeltable,
        ),
        (Machine::Furnace.menu(), 1, &none),
        (
            Machine::Crusher.menu(),
            content::MACHINE_INPUT,
            &next.crushable,
        ),
        (Machine::Crusher.menu(), 1, &none),
        (Machine::Generator.menu(), content::GEN_FUEL, &next.fuels),
        (Machine::Alloy.menu(), content::ALLOY_FUEL, &next.fuels),
        // The hidden slot of the windows without slots of their own takes nothing.
        (Machine::Pump.menu(), 0, &none),
        (content::TANK, 0, &none),
        (content::TERMINAL, 0, &none),
        (content::STATION, 0, &none),
    ];
    // Slice 9: the sheet windows' hidden slot takes nothing; the placer's slots only blocks.
    lists.extend(
        Machine::all()
            .filter(|m| {
                m.owned()
                    && m.has_menu()
                    && (m.slots() == 0 || matches!(m, Machine::Util(u) if u.slots().count == 1))
            })
            .map(|m| (m.menu(), 0, &none)),
    );
    lists.extend((0..content::BLOCK_SLOTS).map(|i| (Machine::Placer.menu(), i, &next.placeable)));
    lists.extend(content::ALLOY_INPUTS.map(|i| (Machine::Alloy.menu(), i, &next.alloying)));
    lists.extend((0..content::MIXER_SLOTS).map(|i| (Machine::Mixer.menu(), i, &next.mixing)));
    lists.extend(content::CRAFTER_MODULES.map(|i| (Machine::Crafter.menu(), i, &modules)));
    for (menu, slot, items) in lists {
        let name = format!("{TEACH}{menu}#{slot}");
        let teach = |what: usize, item: &str| {
            horizon_client::menu::native_menu_slot_rules(
                &name,
                what,
                &ItemStack::new(item, 1),
                &mut true,
                &mut 1,
            );
        };
        teach(TEACH_CLEAR, "minecraft:air");
        for item in items {
            teach(TEACH_ADD, item);
        }
    }
    log::info!(
        "rustcraft: inputs take {} smeltable, {} crushable, {} alloy ingredients, {} fuels",
        next.smeltable.len(),
        next.crushable.len(),
        next.alloying.len(),
        next.fuels.len()
    );
}

// ---- Slice 18, stream 3: modules, tiers, the queue and the reserve of the drawn windows ----

/// A machine's hidden queue cells and its result's reserve (rustcraft_rules::machines::queue).
pub(super) const QUEUE_KEY: &str = "queue";
pub(super) const RESERVE_KEY: &str = "reserve";
/// A drawn window's socket `k` is button SOCKET_BUTTON + k, as on the sheets.
pub(super) const SOCKET_BUTTON: i32 = 40;

/// What a machine is fitted with: its modules (the sheets' key, rustcraft_base), its tier from
/// its block's state, and the tier its block entity saved.
pub(super) struct Fitted {
    pub modules: Vec<Option<Module>>,
    pub tier: u8,
    pub saved_tier: Option<i32>,
}

pub(super) fn fitted(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity) -> Fitted {
    Fitted {
        modules: modules::read(&base::read_module_items(&entity.nbt)),
        tier: crate::game::rustcraft_tiers::tier_of(game, pos, &entity.id),
        saved_tier: read_int(&entity.nbt, sockets::TIER_KEY),
    }
}

impl Fitted {
    /// A8: how much faster its tier works than its first.
    pub fn speed(&self, machine: Machine, tuning: &Tuning) -> f64 {
        tiers::family_of_block(machine.block())
            .map_or(1.0, |f| tiers::speed(f, self.tier, tuning.tier_speed_step))
    }

    /// The speed and economy modules as the processor's rules take them.
    pub fn processor_modules(&self) -> Vec<rustcraft_rules::machines::processor::Module> {
        use rustcraft_rules::machines::processor::Module as P;
        self.modules
            .iter()
            .flatten()
            .filter_map(|m| match m {
                Module::Speed => Some(P::Speed),
                Module::Economy => Some(P::Economy),
                _ => None,
            })
            .collect()
    }

    /// Queue cells open: two a capacity module, one with a stack module.
    pub fn open_cells(&self) -> usize {
        queue::open_cells(
            modules::capacity_slots(&self.modules),
            modules::stack_tier(&self.modules) > 0,
        )
        .min(extras::MAX_CELLS)
    }

    pub fn tier_saved(&self) -> bool {
        self.saved_tier == Some(i32::from(self.tier))
    }

    /// Its block entity's copy of the tier, for the buttons that see only the block entity.
    pub fn save_tier(&self, nbt: &mut Compound) {
        if !self.tier_saved() {
            write_int(nbt, sockets::TIER_KEY, i32::from(self.tier));
        }
    }

    /// The window's extras: its sockets, its queue cells and its reserve.
    pub fn extras(
        &self,
        game: &Game<'_>,
        cells: &[Slot],
        open_cells: usize,
        reserve: &Slot,
    ) -> Vec<i32> {
        let mut e = Extras {
            open: tiers::sockets(self.tier),
            open_cells,
            reserve: reserve.as_ref().map_or(0, |(_, n)| *n),
            ..Extras::default()
        };
        for (k, m) in self.modules.iter().take(tiers::MAX_SOCKETS).enumerate() {
            e.modules[k] = *m;
        }
        for (k, c) in cells.iter().take(extras::MAX_CELLS).enumerate() {
            if let Some((item, n)) = c {
                let number = content::item_number(game, item).map_or(0, |n| n as i32 + 1);
                e.cells[k] = (number, *n);
            }
        }
        extras::encode(&e)
    }
}

/// The modules' holder of a drawn window's block (the sheets have their own).
pub(super) fn socket_holder(block: &str) -> Option<Holder> {
    match Machine::of(block)? {
        Machine::Furnace | Machine::Crusher | Machine::Alloy => Some(Holder::Processor),
        Machine::Pump => Some(Holder::Pump),
        Machine::Crafter => Some(Holder::Crafter),
        _ => None,
    }
}

/// A press on socket `id − SOCKET_BUTTON` of a drawn window with `carried` on the cursor; whether
/// it was one.
fn press_socket(entity: &mut ModBlockEntity, id: i32, carried: &mut Option<ItemStack>) -> bool {
    let Some(holder) = socket_holder(&entity.id) else {
        return false;
    };
    let Some(k) = usize::try_from(id - SOCKET_BUTTON)
        .ok()
        .filter(|k| *k < tiers::MAX_SOCKETS)
    else {
        return false;
    };
    let open = sockets::open(&entity.id, read_int(&entity.nbt, sockets::TIER_KEY));
    let mut slots = modules::read(&base::read_module_items(&entity.nbt));
    let cursor = carried.as_ref().map(|c| Cursor {
        item: c.item.to_string(),
        count: c.count,
    });
    let Some(after) = press_socket_in(&mut slots, k, holder, open, cursor.as_ref()) else {
        return true;
    };
    *carried = after.map(|c| {
        let mut stack = carried
            .clone()
            .filter(|x| *x.item == *c.item)
            .unwrap_or_else(|| ItemStack::new(c.item.as_str(), c.count));
        stack.count = c.count;
        stack
    });
    let items: Vec<Option<String>> = slots
        .iter()
        .map(|m| m.map(|m| m.item().to_owned()))
        .collect();
    base::write_module_items(&mut entity.nbt, &items);
    true
}

/// `data`'s first slots replaced by `front`, the rest (the extras) kept.
pub(super) fn replace_front(data: &mut Vec<i32>, front: Vec<i32>) {
    if data.len() < front.len() {
        data.resize(front.len(), 0);
    }
    data[..front.len()].copy_from_slice(&front);
}

/// `native_destroy_block` of a drawn-window machine (the sheets give theirs in rustcraft_base):
/// its modules, its queue cells and its reserve fall out as items, as its slots do.
pub(super) fn on_destroyed(game: &mut Game<'_>, dimension: &str, pos: Pos, creative: bool) {
    let Some(entity) = entity(game, pos) else {
        return;
    };
    if creative {
        return;
    }
    // Slice 18 (last): the miner's hidden cells and the brewer's (sheets: their modules are
    // rustcraft_base's).
    let hidden: Vec<(String, u32)> = [
        crate::game::rustcraft_mining::CELLS_KEY,
        crate::game::rustcraft_utility::BREW_CELLS_KEY,
        crate::game::rustcraft_utility::BREW_POWDER_CELLS_KEY,
    ]
        .into_iter()
        .flat_map(|key| {
            let cells: Vec<Slot> = crate::game::rustcraft_utility::read_json(&entity.nbt, key);
            queue::spilled(&cells, |i| max_stack(game, i))
        })
        .collect();
    for (item, count) in hidden {
        drop_stack(game, dimension, pos, &item, count);
    }
    // Slice 18, wave 2: the fishing station's catch past its window.
    crate::game::rustcraft_farm::drop_reserve(game, dimension, pos);
    let Some(entity) = self::entity(game, pos) else {
        return;
    };
    // Slice 18 (cross): the kitchen's hidden input cells (a sheet: its modules are rustcraft_base's).
    let kitchen: Vec<Slot> = crate::game::rustcraft_utility::read_json(&entity.nbt, crate::game::rustcraft_utility::KITCHEN_CELLS_KEY);
    if !kitchen.is_empty() {
        let lying = queue::spilled(&kitchen, |i| max_stack(game, i));
        for (item, count) in lying {
            drop_stack(game, dimension, pos, &item, count);
        }
        return;
    }
    let Some(entity) = self::entity(game, pos) else {
        return;
    };
    if socket_holder(&entity.id).is_none() {
        return;
    }
    let modules = modules::read(&base::read_module_items(&entity.nbt));
    let cells: Vec<Slot> = crate::game::rustcraft_utility::read_json(&entity.nbt, QUEUE_KEY);
    let reserve: Slot = crate::game::rustcraft_utility::read_json(&entity.nbt, RESERVE_KEY);
    let mut lying: Vec<(String, u32)> = modules
        .iter()
        .flatten()
        .map(|m| (m.item().to_owned(), 1))
        .collect();
    let hidden: Vec<Slot> = cells.into_iter().chain([reserve]).collect();
    lying.extend(queue::spilled(&hidden, |i| max_stack(game, i)));
    for (item, count) in lying {
        drop_stack(game, dimension, pos, &item, count);
    }
}

/// `count` of `item` as an item entity at the middle of the block at `pos`.
fn drop_stack(game: &mut Game<'_>, dimension: &str, pos: Pos, item: &str, count: u32) {
    let stack = Tag::compound([
        ("id", Tag::String(JavaString::from(item))),
        ("count", Tag::Int(count as i32)),
    ]);
    let nbt = Compound::from([(JavaString::from("Item"), stack)]);
    let at = pos.map(|c| f64::from(c) + 0.5);
    if game
        .native_summon(dimension, "minecraft:item", at, nbt)
        .is_none()
    {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: {count} {item} from the machine at {pos:?} were lost");
    }
}
