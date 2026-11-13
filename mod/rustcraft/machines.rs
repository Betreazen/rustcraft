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
    Game, rustcraft_alloy as alloy, rustcraft_base as base,
    rustcraft_content::{self as content, Kind, Machine},
    rustcraft_crafter as crafter, rustcraft_fluids as fluids, rustcraft_generator as generator,
    rustcraft_mining as mining, rustcraft_monorail as monorail, rustcraft_power as power,
    rustcraft_server::LevelNet,
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    machines::{
        alloy::AlloyRecipe,
        crafter::CraftRecipe,
        panel::{self, Panel},
        processor::{Processor, Recipe, Shown, Spec, State, WorkMode},
        slots::{self, Slot},
    },
    net::{
        energy::{Circuit, Consumer, GroupBalance, Producer, group_of, supply_with_storage},
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
/// A crushing recipe without `time` takes this many ticks in normal mode.
const CRUSH_TICKS: f64 = 100.0;
/// Saved compound keys of a machine.
const PROGRESS_KEY: &str = "progress";
pub const WORK_KEY: &str = "work";
/// The stack size of an item the game does not know.
pub const FALLBACK_STACK: u32 = 64;

/// What the machines of one level did on their last tick, for the diagnosis.
#[derive(Default)]
pub struct Power {
    pub circuits: Vec<Circuit>,
    pub status: HashMap<Pos, Status>,
}

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
    MachineRecipes {
        crushing,
        alloying: rustcraft_rules::machines::alloy::ordered(alloying),
        crafting: HashMap::new(),
    }
}

fn warn_once(id: &str) {
    static WARNED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    if WARNED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(id.to_owned())
    {
        log::warn!("rustcraft: recipe {id} left out: no ingredient or result the game knows");
    }
}

/// The energy nets as circuits: each with the sources and machines on it (joints::energy_members).
pub(super) fn circuits(
    topology: &Topology,
    layout: &Layout,
    ports: &BTreeMap<Pos, Port>,
    settings: &BTreeMap<Pos, HousingJoints>,
    tuning: &Tuning,
) -> Vec<Circuit> {
    topology
        .nets
        .iter()
        .filter(|net| net.kind == ChannelKind::Energy)
        .map(|net| Circuit {
            members: joints::energy_members(net, layout, ports, settings),
            cap: tuning.energy_of(net.min_tier),
        })
        .filter(|c| !c.members.is_empty())
        .collect()
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

pub(super) fn read_f64(nbt: &Compound, key: &str) -> f64 {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Double(bits)) => f64::from_bits(*bits),
        _ => 0.0,
    }
}

pub(super) fn write_f64(nbt: &mut Compound, key: &str, value: f64) {
    nbt.insert(JavaString::from(key), Tag::Double(value.to_bits()));
}

pub(super) fn read_int(nbt: &Compound, key: &str) -> Option<i32> {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Int(v)) => Some(*v),
        Some(Tag::Byte(v)) => Some(i32::from(*v)),
        _ => None,
    }
}

pub(super) fn write_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(JavaString::from(key), Tag::Int(value));
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
        _ => (tuning.dock_buffer, tuning.dock_draw),
    }
}

/// The energy key of a buffer's block entity.
pub(super) const BUFFER_KEY: &str = "energy";

pub(super) fn buffer_energy(game: &Game<'_>, pos: Pos) -> f64 {
    game.native_mod_block_entity(pos)
        .map_or(0.0, |e| read_f64(&e.nbt, BUFFER_KEY))
}

pub(super) fn set_buffer_energy(game: &mut Game<'_>, pos: Pos, energy: f64) -> bool {
    game.native_with_mod_block_entity(pos, |e| write_f64(&mut e.nbt, BUFFER_KEY, energy.max(0.0)))
        .is_some()
}

fn read_buffer(game: &Game<'_>, pos: Pos, machine: Machine, tuning: &Tuning) -> Option<BufferHeld> {
    let entity = game.native_mod_block_entity(pos)?;
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
    let units: Vec<(Pos, Machine, Unit)> = level
        .machines
        .clone()
        .into_iter()
        .filter_map(|(pos, machine)| {
            let unit = match machine.kind() {
                Kind::Processor => {
                    Unit::Processor(read(game, pos, machine, &level.recipes, tuning)?)
                }
                Kind::Generator => Unit::Generator(generator::read(game, pos)?),
                Kind::Alloy => Unit::Alloy(alloy::read(game, pos, &level.recipes, tuning)?),
                Kind::Crafter => {
                    Unit::Crafter(crafter::read(game, pos, &mut level.recipes, tuning)?)
                }
                Kind::Pump => Unit::Pump(fluids::read_pump(game, pos, tuning)?),
                Kind::Station => Unit::Station(monorail::read_station(game, pos, tuning)?),
                Kind::Charger | Kind::Turret => {
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
                Kind::Dock | Kind::Battery => return None,
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
    power::settle(game, &mut level.sources, &report, tuning);
    let request_of = |pos: &Pos| {
        consumers
            .iter()
            .find(|c| c.block == *pos)
            .map_or(0.0, |c| c.request)
    };
    let mut status = HashMap::new();
    for (pos, machine, unit) in units {
        let request = request_of(&pos);
        let received = report.got.get(&pos).copied().unwrap_or(0.0);
        let share = if request > 0.0 {
            (received / request).min(1.0)
        } else {
            0.0
        };
        let shown = match unit {
            Unit::Processor(h) => run(game, pos, h, share, request),
            Unit::Generator(h) => {
                let used = report.used.get(&pos).copied().unwrap_or(0.0);
                let members = group_of(pos, &consumers, circuits);
                let balance = GroupBalance::of(&members, &producers, &consumers, &report.got);
                generator::run(game, pos, h, used, balance, tuning)
            }
            Unit::Alloy(h) => alloy::run(game, pos, h, share, &level.recipes, tuning),
            Unit::Crafter(h) => crafter::run(game, pos, h, share, tuning),
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
}

/// The recipe `machine` runs on `input`, if it has one.
fn job_of(
    game: &Game<'_>,
    machine: Machine,
    input: &ItemStack,
    recipes: &MachineRecipes,
    tuning: &Tuning,
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
    let output_capacity = max_stack(game, &result.item);
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

/// The machine at `pos` as its block entity holds it, and the recipe its input has.
fn read(
    game: &Game<'_>,
    pos: Pos,
    machine: Machine,
    recipes: &MachineRecipes,
    tuning: &Tuning,
) -> Option<Held> {
    let entity = game.native_mod_block_entity(pos)?;
    let slot = |i: usize| entity.items.get(i).cloned().flatten();
    let input = slot(content::MACHINE_INPUT);
    let output = slot(content::MACHINE_OUTPUT);
    let processor = Processor {
        input: input.as_ref().map(|s| (s.item.to_string(), s.count)),
        output: output.as_ref().map(|s| (s.item.to_string(), s.count)),
        progress: read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0),
        mode: read_work(&entity.nbt),
        modules: Vec::new(),
    };
    let job = input
        .as_ref()
        .and_then(|input| job_of(game, machine, input, recipes, tuning));
    let nominal = base_energy(machine, tuning) * processor.factors().1;
    Some(Held {
        processor,
        job,
        nominal,
        data: entity.data.clone(),
    })
}

/// Runs the machine's processor on `share` of its `request` and writes back what changed: the
/// input's count, the result, the progress and the window's data slots. What it shows; none
/// when the block entity is gone.
fn run(game: &mut Game<'_>, pos: Pos, held: Held, share: f64, request: f64) -> Option<Shown> {
    let Held {
        mut processor,
        job,
        nominal,
        data: shown_data,
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
    let before = processor.clone();
    let state = processor.tick(&recipes, &spec, share);
    let working = state == State::Working;
    let shown = Shown::of(state, share);
    let data = panel::encode(&Panel {
        progress: processor.progress,
        got: if working { share * request } else { 0.0 },
        asked: nominal,
        mode: processor.mode,
        shown,
    });
    // An idle machine changes nothing: leave its chunk alone (each call marks it for saving,
    // native_hooks.rs:10719-10739).
    if processor == before && shown_data == data {
        return Some(shown);
    }
    let result: Vec<ItemStack> = job.map(|j| j.result).into_iter().collect();
    game.native_with_mod_block_entity(pos, move |entity| {
        write_slots(
            entity,
            [content::MACHINE_INPUT, content::MACHINE_OUTPUT],
            &[before.input.clone(), before.output.clone()],
            &[processor.input.clone(), processor.output.clone()],
            &result,
        );
        if processor.progress != before.progress {
            write_f64(&mut entity.nbt, PROGRESS_KEY, processor.progress);
        }
        if entity.data != data {
            entity.data = data;
        }
        shown
    })
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
            entity.data = panel::encode(&shown);
        }
        Kind::Alloy => alloy::on_menu_button(entity, id),
        Kind::Crafter => crafter::on_menu_button(entity, id, tuning),
        _ => {}
    }
}

/// `native_menu_button_v2` (native_hooks.rs:4286-4297): the same press with the player's cursor
/// stack, which a pattern cell of the autocrafter takes as its item.
pub(super) fn on_menu_button_v2(
    entity: &mut ModBlockEntity,
    id: i32,
    carried: &Option<ItemStack>,
    tuning: &Tuning,
) {
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
    let next = Accepts {
        smeltable,
        crushable,
        alloying,
        fuels,
        placeable,
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
    lists.extend(content::ALLOY_INPUTS.map(|i| (Machine::Alloy.menu(), i, &next.alloying)));
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
