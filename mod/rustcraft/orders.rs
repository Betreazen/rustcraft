// Orders from the storage terminal on the server (spec section 19, O1–O14; the rules in
// rustcraft_rules::orders). With the recipe book in a terminal (its socket, rustcraft_terminal and
// rustcraft_storage::on_button) a player orders an item and a count; the storage's queue
// (rustcraft_rules::orders::queue) plans it once a second on what the storage holds and hands each
// step whose items are set aside to a free machine of its kind on the same line: the autocrafter,
// the electric furnace, the crusher, the alloy smelter or the mixer (O4, O5, O9, O10, O12). The
// line takes orders only when a node of tier 3 or higher stands on it (M15) and the storage is on
// one of its item nets (M11): «on the line» is every block behind a joint of an item net the
// storage's anchor is a joint of (rustcraft_transport::Transport::around).
//
// Slice 17: a job's items ride the item channel (rustcraft_rules::orders::ship). They leave the
// storage as one shipment on the item net that joins the storage and the machine
// (rustcraft_transport::Transport::route), loaded at the net's throughput and riding the pipe at
// its tier's speed, and go into the machine's input slots when they arrive, the way a machine
// writes its own slots (rustcraft_machines::write_slots); its products leave the machine's output
// slots once a second and ride back. An order still runs on the default joints (the storage
// takes, M16) and never waits on a net's settings: the shipment is the order's own move, not
// one the joints' modes, filters and priorities decide (rustcraft_rules::net::items::tick), and
// it takes its share of the net's throughput first. A shipment whose job is gone when it arrives
// turns back to the storage. Where no item net joins the storage and the machine through joints
// that are not switched off, the items go straight from the storage into the machine and back,
// as slice 15 moved them. An autocrafter's job is a temporary page
// (rustcraft_crafter::TempPage), crafted before its own pages; it disappears when its crafts are
// done. A furnace's, crusher's, smelter's or mixer's job is done when its inputs hold none of the
// job's items.
//
// O11: a product made for a later step of the same order comes back into the storage's count as
// the order's own (rustcraft_rules::orders::queue::Order::made), even past the storage's room and
// limits, and the round of the same second hands it to the next machine; it stays in the storage
// only while that machine is busy or missing.
//
// O12: the mixer's recipes are in the book with water as an input and their fluid as the output,
// both in buckets. A mixer step needs a mixer on the line and the water for it in the mixers'
// own tanks or the tanks they touch or share a fluid net with (rustcraft_fluids::reachable); the
// water itself comes by the fluid nets, as it always does, and the product goes on the same way.
// Water short reads «не хватает: вода ×N».
//
// Energy: a step goes to a free machine with energy (a source touching it or on its circuit, and
// it did not say «Нет энергии» last tick) or, for the alloy smelter, fuel first; to one without
// only when no other is free, and then its order waits for energy and says so.
//
// What is kept where: the «держать N» counts and, since this follow-up, the queues, the jobs and
// the temporary pages go into the storage's record (rustcraft_storage, a level attachment, as
// rustcraft_rules::orders::saved's strings), written every second they change. On load the jobs
// take their machines again: a crafter's page owes no more than its buffer still holds the items
// for, a job whose order is gone gives its items back, and a job whose machine is not on the level
// yet (its chunk not loaded) waits ADOPT_SECONDS for it. The recipe book is built from the game's
// crafting and smelting recipes (`Recipes::crafting`, `Recipes::cooking`,
// SRC/crates/horizon-client/src/crafting.rs:1337-1345, special recipes left out) and the
// machines' recipes (rustcraft_machines::load_recipes), again after every rescan.
use crate::game::{
    Game, rustcraft_alloy as alloy,
    rustcraft_content::{self as content, Kind, Machine, item_number},
    rustcraft_crafter::{self as crafter, Pattern, TempPage},
    rustcraft_fluids as fluids,
    rustcraft_machines::{self as machines, max_stack, slot_of, write_slots},
    rustcraft_server::{self as server, LevelNet},
    rustcraft_storage as storage,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    machines::{
        processor::Shown,
        slots::{self, Slot},
    },
    net::{fluids::BUCKET, items::ItemKey},
    orders::{
        KINDS, PlanRecipe, Step, fluid_key, fluid_number, fluid_of,
        queue::{FreeMachine, Job, Order, Queue, SMART_TIER, Wait, runs_that_fit},
        saved::{self, Record, SavedJob, SavedPage},
        ship::{self, Leg, Route, Shipment},
    },
    storage::{
        keys,
        wire::{MAX_WANTS, OrderLine, OrderWait, Smart},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    ops::Range,
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError},
};

/// The terminal's saved compound keeps whether the recipe book is in its socket under this key.
const BOOK_KEY: &str = "book";
/// Proposal: seconds a job read back from the record waits for its machine to be on the level
/// (its chunk loaded) before it ends.
const ADOPT_SECONDS: u32 = 300;
const WATER: &str = "minecraft:water";

/// The recipe book (O1): every recipe the orders know, the items and fluids they make as the
/// terminal's numbers, and the fluids mixer steps take.
pub(super) struct Book {
    recipes: Vec<PlanRecipe>,
    orderable: Vec<u16>,
    fluids_in: Vec<String>,
}

/// A job read back from the record that has not found its machine yet.
#[derive(Clone, Debug)]
struct Restored {
    page: Option<SavedPage>,
    seconds_left: u32,
}

/// A job on a machine, and the storage it works for.
#[derive(Clone, Debug)]
struct Running {
    storage: u32,
    job: Job,
    restored: Option<Restored>,
}

/// One level's orders.
#[derive(Clone, Debug, Default, PartialEq)]
struct LevelOrders {
    queues: BTreeMap<u32, Queue>,
    smart: BTreeMap<u32, Smart>,
    /// Counts on with every change the terminals show.
    revision: u64,
}

#[derive(Default)]
struct Orders {
    levels: HashMap<String, LevelOrders>,
    jobs: HashMap<String, Vec<Running>>,
    /// Slice 17: the items riding the item channel for the level's jobs, in the order they set off.
    ships: HashMap<String, Vec<Shipment>>,
    book: Option<Arc<Book>>,
}

static ORDERS: LazyLock<Mutex<Orders>> = LazyLock::new(Mutex::default);

fn orders() -> MutexGuard<'static, Orders> {
    ORDERS.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- The book in the terminal ----

/// Whether a terminal's saved compound has the recipe book in its socket.
pub(super) fn has_book(nbt: &Compound) -> bool {
    matches!(nbt.get(&JavaString::from(BOOK_KEY)), Some(Tag::Byte(1)))
}

pub(super) fn set_book(nbt: &mut Compound, book: bool) {
    nbt.insert(JavaString::from(BOOK_KEY), Tag::Byte(i8::from(book)));
}

/// The item of the recipe book.
pub(super) fn book_item() -> &'static str {
    rustcraft_rules::modules::Module::Recipes.item()
}

/// The terminals show their window again: the book went in or out.
pub(super) fn touch(dimension: &str) {
    orders()
        .levels
        .entry(dimension.to_owned())
        .or_default()
        .revision += 1;
}

/// Every level's terminals show their window again: a press whose level is not at hand.
pub(super) fn touch_all() {
    for level in orders().levels.values_mut() {
        level.revision += 1;
    }
}

/// What the terminals of the level show of the orders last changed at this count.
pub(super) fn revision(dimension: &str) -> u64 {
    orders().levels.get(dimension).map_or(0, |l| l.revision)
}

/// The recipes changed (a rescan read them again): the book is built anew when next needed.
pub(super) fn forget_book() {
    orders().book = None;
}

/// `native_level_unload`: the level's machines are gone from the server's map, its temporary
/// pages with them (`pages`); its jobs and queues stay, and the jobs take their machines again,
/// pages and all, when the level ticks again.
pub(super) fn on_level_unload(dimension: &str, pages: BTreeMap<Pos, TempPage>) {
    if let Some(jobs) = orders().jobs.get_mut(dimension) {
        for r in jobs {
            r.restored = Some(Restored {
                page: pages.get(&r.job.machine).map(page_of),
                seconds_left: ADOPT_SECONDS,
            });
        }
    }
}

// ---- Kept over a restart ----

fn page_of(t: &TempPage) -> SavedPage {
    SavedPage {
        pattern: t.pattern.to_vec(),
        owed: t.owed,
    }
}

/// The record of each storage of the level that has orders or jobs.
fn records(
    state: &LevelOrders,
    jobs: &[Running],
    ships: &[Shipment],
    pages: &BTreeMap<Pos, TempPage>,
) -> BTreeMap<u32, Vec<String>> {
    let ids: BTreeSet<u32> = state
        .queues
        .keys()
        .copied()
        .chain(jobs.iter().map(|r| r.storage))
        .chain(ships.iter().map(|s| s.storage))
        .collect();
    ids.into_iter()
        .map(|id| {
            let record = Record {
                queue: state.queues.get(&id).cloned().unwrap_or_default(),
                jobs: jobs
                    .iter()
                    .filter(|r| r.storage == id)
                    .map(|r| SavedJob {
                        job: r.job.clone(),
                        // A job not back on its machine yet keeps the page it was read with.
                        page: match &r.restored {
                            Some(rs) => rs.page.clone(),
                            None => pages.get(&r.job.machine).map(page_of),
                        },
                    })
                    .collect(),
                ships: ships.iter().filter(|s| s.storage == id).cloned().collect(),
            };
            (id, saved::encode(&record))
        })
        .filter(|(_, strings)| !strings.is_empty())
        .collect()
}

/// The level's orders as the storage's record keeps them (rustcraft_storage's load, and the
/// self-test's restart): the queues and jobs in memory give way to them.
pub(super) fn restore(dimension: &str, saved: &BTreeMap<u32, Vec<String>>) {
    let mut queues = BTreeMap::new();
    let mut jobs = Vec::new();
    let mut ships = Vec::new();
    for (&id, strings) in saved {
        let Some(record) = saved::decode(strings) else {
            log::warn!("rustcraft: the orders of storage {id} were not read");
            continue;
        };
        if !record.queue.orders.is_empty() {
            queues.insert(id, record.queue);
        }
        // A shipment rides on from where it was: it arrives, or turns back when its job is gone.
        ships.extend(
            record
                .ships
                .into_iter()
                .map(|s| Shipment { storage: id, ..s }),
        );
        jobs.extend(record.jobs.into_iter().map(|j| Running {
            storage: id,
            job: j.job,
            restored: Some(Restored {
                page: j.page,
                seconds_left: ADOPT_SECONDS,
            }),
        }));
    }
    log::info!(
        "rustcraft: {} orders, {} jobs and {} shipments read back on {dimension}",
        queues.values().map(|q| q.orders.len()).sum::<usize>(),
        jobs.len(),
        ships.len()
    );
    let mut o = orders();
    let level = o.levels.entry(dimension.to_owned()).or_default();
    level.queues = queues;
    level.revision += 1;
    o.jobs.insert(dimension.to_owned(), jobs);
    o.ships.insert(dimension.to_owned(), ships);
}

/// A job read back finds its machine: an autocrafter gets its page again, owing no more than its
/// buffer can still make.
fn adopt(game: &Game<'_>, level: &mut LevelNet, machine: Machine, r: &mut Running) {
    let Some(restored) = r.restored.take() else {
        return;
    };
    let (Machine::Crafter, Some(page)) = (machine, restored.page) else {
        return;
    };
    let buffer = read_slots(game, r.job.machine, content::CRAFTER_BUFFER).unwrap_or_default();
    let owed = saved::owed_after_load(page.owed, &r.job.step.inputs, &buffer);
    let pattern: Pattern = std::array::from_fn(|i| page.pattern.get(i).cloned().flatten());
    level
        .temp_pages
        .insert(r.job.machine, TempPage { pattern, owed });
}

// ---- The recipe book ----

fn alternatives(i: &horizon_client::crafting::Ingredient) -> Vec<ItemKey> {
    if i.order.is_empty() {
        let mut items: Vec<ItemKey> = i.items.iter().cloned().collect();
        items.sort();
        items
    } else {
        i.order.clone()
    }
}

/// Buckets of `mb`, at least one, rounded up for what a recipe takes.
fn buckets_in(mb: u64) -> u32 {
    u32::try_from(mb.div_ceil(BUCKET).max(1)).unwrap_or(u32::MAX)
}

/// The recipe book from the game's recipes and the machines'.
fn build_book(game: &Game<'_>, recipes: &machines::MachineRecipes) -> Book {
    use horizon_client::crafting::Recipe;
    let mut book: Vec<PlanRecipe> = Vec::new();
    for (_, recipe) in &game.recipes().crafting {
        let (width, cells, result): (usize, Vec<Option<Vec<ItemKey>>>, _) = match recipe {
            Recipe::Shaped {
                width,
                cells,
                result,
                ..
            } => (
                *width,
                cells.iter().map(|c| c.as_ref().map(alternatives)).collect(),
                result,
            ),
            Recipe::Shapeless {
                ingredients,
                result,
            } => (
                3,
                ingredients.iter().map(|i| Some(alternatives(i))).collect(),
                result,
            ),
            Recipe::Special(_) => continue,
        };
        // A result with components (a named or enchanted item) is no item the storage keeps.
        let Some(item) = result.item.as_deref().filter(|_| result.extra.is_none()) else {
            continue;
        };
        if result.damage > 0 || cells.len() > 9 {
            continue;
        }
        if let Some(r) = PlanRecipe::crafting(width, &cells, (item.to_owned(), result.count)) {
            // Slice 18: a food recipe runs on the kitchen too.
            if let Some(k) = rustcraft_rules::orders::kitchen_copy(&r, &rustcraft_rules::utility::KITCHEN_RECIPES) {
                book.push(k);
            }
            book.push(r);
        }
    }
    for c in game
        .recipes()
        .cooking
        .iter()
        .filter(|c| c.kind == machines::SMELTING)
    {
        if c.result.extra.is_some() || c.result.count == 0 {
            continue;
        }
        book.push(PlanRecipe {
            machine: KINDS[1].into(),
            inputs: vec![(alternatives(&c.ingredient), 1)],
            output: (c.result.item.to_string(), c.result.count),
            cells: Vec::new(),
        });
    }
    for r in &recipes.crushing {
        let mut inputs: Vec<ItemKey> = r.inputs.iter().cloned().collect();
        inputs.sort();
        book.push(PlanRecipe {
            machine: KINDS[2].into(),
            inputs: vec![(inputs, 1)],
            output: (r.result.item.to_string(), r.result.count.max(1)),
            cells: Vec::new(),
        });
    }
    for r in &recipes.alloying {
        book.push(PlanRecipe {
            machine: KINDS[3].into(),
            inputs: r
                .inputs
                .iter()
                .map(|(i, n)| (vec![i.clone()], *n))
                .collect(),
            output: r.output.clone(),
            cells: Vec::new(),
        });
    }
    // O12: items and water in buckets, the fluid out in whole buckets.
    for r in &recipes.mixing {
        let mut inputs: Vec<(Vec<ItemKey>, u32)> = r
            .ingredients
            .iter()
            .map(|(i, n)| (vec![i.clone()], *n))
            .collect();
        if r.water > 0 {
            inputs.push((vec![fluid_key(WATER)], buckets_in(r.water)));
        }
        let out = u32::try_from((r.amount / BUCKET).max(1)).unwrap_or(u32::MAX);
        book.push(PlanRecipe {
            machine: KINDS[4].into(),
            inputs,
            output: (fluid_key(&r.fluid), out),
            cells: Vec::new(),
        });
    }
    let outputs: BTreeSet<&str> = book.iter().map(|r| r.output.0.as_str()).collect();
    let orderable = outputs
        .into_iter()
        .filter_map(|key| number(game, key))
        .collect();
    let fluids_in: BTreeSet<String> = book
        .iter()
        .flat_map(|r| r.inputs.iter().flat_map(|(alts, _)| alts.iter()))
        .filter_map(|k| fluid_of(k).map(str::to_owned))
        .collect();
    Book {
        recipes: book,
        orderable,
        fluids_in: fluids_in.into_iter().collect(),
    }
}

fn book_of(game: &Game<'_>, recipes: &machines::MachineRecipes) -> Arc<Book> {
    if let Some(b) = orders().book.clone() {
        return b;
    }
    let book = Arc::new(build_book(game, recipes));
    log::info!(
        "rustcraft: the recipe book knows {} recipes",
        book.recipes.len()
    );
    orders().book = Some(book.clone());
    book
}

// ---- Presses ----

/// Why an order was not taken, as a lang key and its English.
pub(super) type Refusal = (&'static str, &'static str);

/// O3: an order for `count` of `item` (a fluid's key in buckets) at the storage `storage`; a
/// refusal when its line takes no orders or its queue is full.
pub(super) fn order(dimension: &str, storage: u32, item: &str, count: u32) -> Result<(), Refusal> {
    let mut o = orders();
    let level = o.levels.entry(dimension.to_owned()).or_default();
    match level.smart.get(&storage).copied().unwrap_or(Smart::NoNet) {
        Smart::Works => {}
        Smart::LowTier => {
            return Err((
                "rustcraft.orders.low_tier",
                "Orders need a node of tier 3 on the storage's line",
            ));
        }
        Smart::NoNet => {
            return Err((
                "rustcraft.orders.no_net",
                "The storage is on no item channel",
            ));
        }
    }
    let queue = level.queues.entry(storage).or_default();
    if queue.add(item, count, false).is_none() {
        return Err(("rustcraft.orders.full", "The order queue is full"));
    }
    level.revision += 1;
    Ok(())
}

/// The key a terminal's number names: a fluid's past the item table (O12), else the item's.
pub(super) fn key_of_number(game: &Game<'_>, number: u16) -> Option<ItemKey> {
    if let Some(fluid) = rustcraft_rules::orders::fluid_of_number(number) {
        return Some(fluid_key(fluid));
    }
    content::item_of_number(game, u32::from(number)).map(str::to_owned)
}

/// O7: the order `id` of the storage leaves its queue; what its jobs did not use yet comes back
/// at the next tick.
pub(super) fn cancel(dimension: &str, storage: u32, id: u16) {
    let mut o = orders();
    let Some(level) = o.levels.get_mut(dimension) else {
        return;
    };
    if let Some(q) = level.queues.get_mut(&storage)
        && q.cancel(id).is_some()
    {
        level.revision += 1;
    }
}

/// The self-test's view of a storage's queue.
pub(super) fn queue_of(dimension: &str, storage: u32) -> Vec<Order> {
    orders()
        .levels
        .get(dimension)
        .and_then(|l| l.queues.get(&storage))
        .map(|q| q.orders.clone())
        .unwrap_or_default()
}

// ---- What the terminal shows ----

/// What a terminal's list says of the orders (rustcraft_rules::storage::wire).
pub(super) struct View {
    pub book: bool,
    pub smart: Smart,
    pub orders: Vec<OrderLine>,
    pub orderable: Vec<u16>,
}

/// The terminal's number of a key: a fluid's (O12) or the item's in the item table.
fn number(game: &Game<'_>, key: &str) -> Option<u16> {
    if let Some(fluid) = fluid_of(key) {
        return fluid_number(fluid);
    }
    item_number(game, keys::item_of(key)).and_then(|n| u16::try_from(n).ok())
}

fn kind_code(kind: &str) -> u8 {
    KINDS.iter().position(|k| *k == kind).unwrap_or(0) as u8
}

fn line_of(game: &Game<'_>, o: &Order) -> Option<OrderLine> {
    let wait = match &o.wait {
        Wait::New => OrderWait::New,
        Wait::Running => OrderWait::Running,
        Wait::Items(items) => OrderWait::Items(
            items
                .iter()
                .filter_map(|(k, n)| {
                    Some((number(game, k)?, u32::try_from(*n).unwrap_or(u32::MAX)))
                })
                .take(MAX_WANTS)
                .collect(),
        ),
        Wait::Machines(kinds) => {
            OrderWait::Machines(kinds.iter().map(|k| kind_code(k)).take(MAX_WANTS).collect())
        }
        Wait::Busy(kind) => OrderWait::Busy(kind_code(kind)),
        Wait::Energy(kind) => OrderWait::Energy(kind_code(kind)),
    };
    // Slice 18: its chain of steps, the running ones marked.
    let steps = o
        .chain
        .iter()
        .filter_map(|(kind, item, current)| {
            Some(rustcraft_rules::storage::wire::OrderStep {
                kind: kind_code(kind),
                item: number(game, item)?,
                current: *current,
            })
        })
        .take(rustcraft_rules::storage::wire::MAX_STEPS)
        .collect();
    Some(OrderLine {
        id: o.id,
        item: number(game, &o.item)?,
        count: o.count,
        done: o.done,
        keep: o.keep,
        wait,
        steps,
    })
}

/// What the terminal at `terminal` shows of the orders of the storage `storage`.
pub(super) fn view(game: &Game<'_>, dimension: &str, terminal: Pos, storage: u32) -> View {
    let book = game
        .native_mod_block_entity(terminal)
        .is_some_and(|e| has_book(&e.nbt));
    let (smart, queue, known) = {
        let o = orders();
        let level = o.levels.get(dimension);
        (
            level
                .and_then(|l| l.smart.get(&storage).copied())
                .unwrap_or(Smart::NoNet),
            level
                .and_then(|l| l.queues.get(&storage))
                .map(|q| q.orders.clone())
                .unwrap_or_default(),
            o.book.clone(),
        )
    };
    View {
        book,
        smart,
        orders: queue.iter().filter_map(|o| line_of(game, o)).collect(),
        orderable: known
            .filter(|_| book)
            .map(|b| b.orderable.clone())
            .unwrap_or_default(),
    }
}

// ---- The machines of a line ----

/// The kind a machine runs order steps as.
fn kind_of(machine: Machine) -> Option<&'static str> {
    match machine {
        Machine::Crafter => Some(KINDS[0]),
        Machine::Furnace => Some(KINDS[1]),
        Machine::Crusher => Some(KINDS[2]),
        Machine::Alloy => Some(KINDS[3]),
        Machine::Mixer => Some(KINDS[4]),
        Machine::Util(rustcraft_rules::utility::Util::Kitchen) => Some(KINDS[5]),
        _ => None,
    }
}

/// Slice 18: what the order job on the machine at `pos` makes (the kitchen cooks it whatever its
/// pages say).
pub(super) fn job_output_at(dimension: &str, pos: Pos) -> Option<ItemKey> {
    orders()
        .jobs
        .get(dimension)?
        .iter()
        .find(|r| r.job.machine == pos)
        .map(|r| r.job.step.output.0.clone())
}

/// The slots an order's items go into, and those its products come out of (none for the mixer,
/// whose product is a fluid).
fn slots_of(machine: Machine) -> (Range<usize>, Range<usize>) {
    match machine {
        Machine::Crafter => (content::CRAFTER_BUFFER, content::CRAFTER_OUTPUTS),
        Machine::Alloy => (content::ALLOY_INPUTS, content::ALLOY_OUTPUTS),
        Machine::Mixer => (0..content::MIXER_SLOTS, 0..0),
        // Slice 18: the kitchen's ingredients and dishes.
        Machine::Util(rustcraft_rules::utility::Util::Kitchen) => {
            let s = rustcraft_rules::utility::Util::Kitchen.slots();
            (s.inputs, s.outputs)
        }
        _ => (
            content::MACHINE_INPUT..content::MACHINE_INPUT + 1,
            content::MACHINE_OUTPUT..content::MACHINE_OUTPUT + 1,
        ),
    }
}

fn read_slots(game: &Game<'_>, pos: Pos, range: Range<usize>) -> Option<Vec<Slot>> {
    let entity = game.native_mod_block_entity(pos)?;
    Some(range.map(|i| slot_of(&entity, i)).collect())
}

/// Whether the machine at `pos` can work: an alloy smelter with fuel; else one that did not say
/// «Нет энергии» or «Нет топлива» on its last tick and has a source of energy touching it (E10)
/// or on an energy net it is on (E3).
fn powered(game: &Game<'_>, level: &LevelNet, pos: Pos, machine: Machine) -> bool {
    if machine == Machine::Alloy && alloy::has_fuel(game, pos) {
        return true;
    }
    let stalled = level
        .power
        .status
        .get(&pos)
        .is_some_and(|s| matches!(s.shown, Shown::NoEnergy | Shown::NoFuel));
    if stalled {
        return false;
    }
    let source = |p: &Pos| {
        level.crystals.contains(p)
            || level
                .machines
                .get(p)
                .is_some_and(|m| matches!(m.kind(), Kind::Generator | Kind::Power | Kind::Battery))
    };
    Face::ALL.into_iter().any(|f| source(&step(pos, f)))
        || level
            .power
            .circuits
            .iter()
            .any(|c| c.members.contains(&pos) && c.members.iter().any(|p| source(p)))
}

/// M15: whether the storage at `anchor` is on an item net whose line has a node of SMART_TIER
/// or higher, and the machines of that net that run order steps.
fn line_state(level: &LevelNet, anchor: Pos) -> (Smart, Vec<(Pos, Machine)>) {
    let Some((blocks, housings)) = level.transport.around(anchor) else {
        return (Smart::NoNet, Vec::new());
    };
    let lines: BTreeSet<usize> = housings
        .iter()
        .filter_map(|h| level.topology.line_of(*h))
        .collect();
    let smart_node = lines.iter().any(|&l| {
        level.topology.lines[l].iter().any(|&h| {
            Face::ALL.into_iter().any(|f| {
                level
                    .layout
                    .nodes
                    .get(&step(h, f))
                    .is_some_and(|n| n.tier >= SMART_TIER)
            })
        })
    });
    let machines = blocks
        .iter()
        .filter_map(|p| level.machines.get(p).map(|m| (*p, *m)))
        .filter(|(_, m)| kind_of(*m).is_some())
        .collect();
    let smart = if smart_node {
        Smart::Works
    } else {
        Smart::LowTier
    };
    (smart, machines)
}

// ---- Moving an order's items ----

/// How a job's items move between its storage and its machine: along `route` on the item
/// channel (slice 17), or straight when no item net joins them.
struct Way<'s> {
    storage: u32,
    order: u16,
    route: Option<Route>,
    ships: &'s mut Vec<Shipment>,
}

impl Way<'_> {
    /// `items` set off along the route to `leg`'s end.
    fn send(&mut self, leg: Leg, items: Vec<(ItemKey, u32)>) {
        let Some(route) = &self.route else {
            return;
        };
        let tps = server::tuning().ticks_per_second;
        self.ships.push(Shipment::new(
            self.storage,
            self.order,
            leg,
            items,
            route,
            tps,
        ));
    }
}

/// The way an order's items ride between the storage at `anchor` and the machine at `machine`.
fn route_of(level: &LevelNet, anchor: Option<Pos>, machine: Pos) -> Option<Route> {
    let route = level
        .transport
        .route(&level.layout, server::tuning(), anchor?, machine);
    if route.is_none() {
        log::debug!("rustcraft: no item path to {machine:?}: an order's items move directly");
    }
    route
}

/// The stack sizes of a step's inputs.
fn stacks(game: &Game<'_>, inputs: &[(ItemKey, u32)]) -> BTreeMap<ItemKey, u32> {
    inputs
        .iter()
        .map(|(i, _)| (i.clone(), max_stack(game, keys::item_of(i))))
        .collect()
}

/// `wants` into the inputs of the machine at `pos`; false, and nothing written, when they do not
/// all fit or the machine is gone.
fn place_inputs(game: &mut Game<'_>, machine: Machine, pos: Pos, wants: &[(ItemKey, u64)]) -> bool {
    let items: Vec<(ItemKey, u32)> = wants.iter().map(|(i, _)| (i.clone(), 1)).collect();
    let sizes = stacks(game, &items);
    let (inputs, _) = slots_of(machine);
    let placed = game.native_with_mod_block_entity(pos, |entity| {
        let before: Vec<Slot> = inputs.clone().map(|i| slot_of(entity, i)).collect();
        let mut after = before.clone();
        for (item, want) in wants {
            let n = u32::try_from(*want).unwrap_or(u32::MAX);
            let size = sizes.get(item).copied().unwrap_or(machines::FALLBACK_STACK);
            if !slots::place(&mut after, item, n, size) {
                return false;
            }
        }
        write_slots(entity, inputs.clone(), &before, &after, &[]);
        true
    });
    placed == Some(true)
}

/// What came of handing a job its items.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sent {
    /// The storage no longer holds them, or the machine does not take them: nothing moved.
    No,
    /// Straight into the machine (no item path).
    Placed,
    /// Riding the item channel to the machine.
    Riding,
}

/// A job's items out of the storage, into the machine's inputs or onto the item channel to it.
/// A fluid input is not the storage's to give: the fluid nets bring it (O12).
fn deliver(
    game: &mut Game<'_>,
    dimension: &str,
    way: &mut Way<'_>,
    machine: Machine,
    job: &Job,
) -> Sent {
    let wants: Vec<(ItemKey, u64)> = job
        .step
        .items()
        .iter()
        .map(|(i, n)| (i.clone(), u64::from(*n) * u64::from(job.runs)))
        .collect();
    let storage_id = way.storage;
    let mut taken: Vec<(ItemKey, u64)> = Vec::new();
    let give_back = |taken: &[(ItemKey, u64)]| {
        for (item, n) in taken {
            storage::restore_into(dimension, storage_id, item, *n);
        }
    };
    for (item, want) in &wants {
        let got = storage::take_from(dimension, storage_id, item, *want);
        taken.push((item.clone(), got));
        if got < *want {
            give_back(&taken);
            return Sent::No;
        }
    }
    if way.route.is_some() {
        if game.native_mod_block_entity(job.machine).is_none() {
            give_back(&taken);
            return Sent::No;
        }
        let items = wants
            .iter()
            .map(|(i, n)| (i.clone(), u32::try_from(*n).unwrap_or(u32::MAX)))
            .collect();
        way.send(
            Leg::Machine {
                machine: job.machine,
            },
            items,
        );
        return Sent::Riding;
    }
    if !place_inputs(game, machine, job.machine, &wants) {
        give_back(&taken);
        return Sent::No;
    }
    Sent::Placed
}

/// The plain stacks of `item` in the machine's output slots go to the storage, as far as it
/// takes them; with `own` (O11: a product the order's next step takes) all of them, past the
/// storage's room and limits. Along a route they ride and land when they arrive; how many went
/// into the storage now.
fn pull(
    game: &mut Game<'_>,
    dimension: &str,
    way: &mut Way<'_>,
    pos: Pos,
    machine: Machine,
    item: &str,
    own: bool,
) -> u32 {
    let (_, outputs) = slots_of(machine);
    if outputs.is_empty() || fluid_of(item).is_some() {
        return 0;
    }
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return 0;
    };
    // Only stacks without components: the storage keeps none.
    let plain: u32 = outputs
        .clone()
        .filter_map(|i| entity.items.get(i).cloned().flatten())
        .filter(|s| &*s.item == item && s.extra.is_none() && s.damage == 0)
        .map(|s| s.count)
        .sum();
    if plain == 0 {
        return 0;
    }
    let storage_id = way.storage;
    let riding = way.route.is_some();
    let took = if own {
        if !riding {
            storage::restore_into(dimension, storage_id, item, u64::from(plain));
        }
        plain
    } else if riding {
        // As far as the storage will take them when they arrive, less what rides to it already.
        let on_the_way: u64 = way
            .ships
            .iter()
            .filter(|s| {
                s.storage == storage_id && matches!(s.leg, Leg::Products { own: false, .. })
            })
            .flat_map(|s| s.items.iter())
            .filter(|(i, _)| i == item)
            .map(|(_, n)| u64::from(*n))
            .sum();
        let room = storage::room_in(game, dimension, storage_id, item).saturating_sub(on_the_way);
        u32::try_from(room).unwrap_or(u32::MAX).min(plain)
    } else {
        let took = storage::put_into(game, dimension, storage_id, item, u64::from(plain));
        u32::try_from(took).unwrap_or(plain).min(plain)
    };
    if took == 0 {
        return 0;
    }
    let written = game.native_with_mod_block_entity(pos, |e| {
        let before: Vec<Slot> = outputs.clone().map(|i| slot_of(e, i)).collect();
        let mut after = before.clone();
        slots::take_from(&mut after, item, took);
        write_slots(e, outputs.clone(), &before, &after, &[]);
    });
    if written.is_none() {
        // The machine went meanwhile: what the storage took is not there.
        if !riding {
            storage::take_from(dimension, storage_id, item, u64::from(took));
        }
        return 0;
    }
    if riding {
        way.send(
            Leg::Products { from: pos, own },
            vec![(item.to_owned(), took)],
        );
        return 0;
    }
    took
}

/// O7: what a cancelled job did not use yet goes back from the machine's inputs to the storage:
/// at most `runs` runs of its items, riding the item channel where it joins them.
fn take_back(
    game: &mut Game<'_>,
    dimension: &str,
    way: &mut Way<'_>,
    machine: Machine,
    job: &Job,
    runs: u32,
) {
    let (inputs, _) = slots_of(machine);
    let back = game.native_with_mod_block_entity(job.machine, |e| {
        let before: Vec<Slot> = inputs.clone().map(|i| slot_of(e, i)).collect();
        let mut after = before.clone();
        let mut back: Vec<(ItemKey, u32)> = Vec::new();
        for (item, n) in &job.step.items() {
            let want = n.saturating_mul(runs).min(slots::count_in(&after, item));
            let left = slots::take_from(&mut after, item, want);
            back.push((item.clone(), want - left));
        }
        write_slots(e, inputs.clone(), &before, &after, &[]);
        back
    });
    let back: Vec<(ItemKey, u32)> = back
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .collect();
    if back.is_empty() {
        return;
    }
    if way.route.is_some() {
        way.send(Leg::Back { from: job.machine }, back);
        return;
    }
    for (item, n) in back {
        storage::restore_into(dimension, way.storage, &item, u64::from(n));
    }
}

/// Whether a machine has none of the job's items left in its inputs.
fn inputs_spent(game: &Game<'_>, machine: Machine, job: &Job) -> bool {
    let (inputs, _) = slots_of(machine);
    read_slots(game, job.machine, inputs).is_none_or(|slots| {
        job.step
            .items()
            .iter()
            .all(|(item, _)| slots::count_in(&slots, item) == 0)
    })
}

// ---- Shipments arriving ----

/// The most items one item entity holds when a shipment's storage is gone.
const DROP_STACK: u32 = 64;

/// Items riding to the storage arrive: into it (`own`: O11, the order's own, past its room and
/// limits; `None`: coming back, past them too; `Some(false)`: as far as it takes them, the rest
/// past them, so nothing is lost). A storage that is gone meanwhile leaves them on the ground at
/// the block they came from.
fn land(
    game: &mut Game<'_>,
    dimension: &str,
    state: &mut LevelOrders,
    live: &BTreeSet<u32>,
    s: &Shipment,
    from: Pos,
    own: Option<bool>,
) {
    if !live.contains(&s.storage) {
        for (item, n) in &s.items {
            let mut left = *n;
            while left > 0 {
                let part = left.min(DROP_STACK);
                storage::drop_stack(game, dimension, from, item, part);
                left -= part;
            }
        }
        return;
    }
    for (item, n) in &s.items {
        let n = u64::from(*n);
        let took = match own {
            Some(false) => storage::put_into(game, dimension, s.storage, item, n),
            _ => 0,
        };
        if took < n {
            storage::restore_into(dimension, s.storage, item, n - took);
        }
        if own == Some(true)
            && let Some(q) = state.queues.get_mut(&s.storage)
        {
            q.keep_for(s.order, item, n);
        }
    }
}

/// Slice 17: the shipments that arrived. A job's items go into its machine's inputs (an
/// autocrafter gets its temporary page then); a shipment whose job is gone turns back, one whose
/// machine is not on the level yet while the job read back waits for it waits too, and a job whose
/// items no longer fit its machine ends (they ride back). Products and items coming back land in
/// the storage.
fn arrive(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelNet,
    state: &mut LevelOrders,
    jobs: &mut Vec<Running>,
    ships: &mut Vec<Shipment>,
) {
    if !ships.iter().any(Shipment::arrived) {
        return;
    }
    let live: BTreeSet<u32> = storage::level_storages(dimension)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    let mut kept = Vec::with_capacity(ships.len());
    let mut failed: Vec<(u32, u16, Pos)> = Vec::new();
    for s in std::mem::take(ships) {
        if !s.arrived() {
            kept.push(s);
            continue;
        }
        match s.leg.clone() {
            Leg::Machine { machine } => {
                let Some(r) = jobs
                    .iter()
                    .find(|r| s.bound_for(r.storage, r.job.order, r.job.machine))
                else {
                    kept.push(s.turn_back());
                    continue;
                };
                let on_level = level
                    .machines
                    .get(&machine)
                    .copied()
                    .filter(|m| kind_of(*m) == Some(r.job.step.machine.as_str()));
                let Some(kind) = on_level else {
                    kept.push(if r.restored.is_some() {
                        s
                    } else {
                        s.turn_back()
                    });
                    continue;
                };
                let wants: Vec<(ItemKey, u64)> = s
                    .items
                    .iter()
                    .map(|(i, n)| (i.clone(), u64::from(*n)))
                    .collect();
                if !place_inputs(game, kind, machine, &wants) {
                    failed.push((r.storage, r.job.order, machine));
                    kept.push(s.turn_back());
                    continue;
                }
                if kind == Machine::Crafter {
                    let book = book_of(game, &level.recipes);
                    let page = TempPage {
                        pattern: r.job.step.pattern(&book.recipes),
                        owed: r.job.runs,
                    };
                    level.temp_pages.insert(machine, page);
                }
            }
            Leg::Products { from, own } => {
                land(game, dimension, state, &live, &s, from, Some(own));
            }
            Leg::Back { from } => land(game, dimension, state, &live, &s, from, None),
        }
    }
    jobs.retain(|r| !failed.contains(&(r.storage, r.job.order, r.job.machine)));
    *ships = kept;
}

// ---- The tick ----

/// Once a second from the level's tick: the shipments ride on and those that arrived land, the
/// jobs looked at, the cancelled ones taken back, and a round of every storage's queue; then the
/// storages' records of their orders brought up to date.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &mut LevelNet) {
    let (mut state, mut jobs, mut ships) = {
        let mut o = orders();
        (
            o.levels.remove(dimension).unwrap_or_default(),
            o.jobs.remove(dimension).unwrap_or_default(),
            o.ships.remove(dimension).unwrap_or_default(),
        )
    };
    let before = state.clone();
    let job_count = jobs.len();
    let had_book = orders().book.is_some();
    // Slice 17: a second of riding; the nets' own moves get what the shipments left.
    let tps = server::tuning().ticks_per_second;
    let second = tps.round().max(1.0) as u32;
    let loaded = ship::advance(&mut ships, second);
    level.transport.reserve(loaded, second);
    arrive(game, dimension, level, &mut state, &mut jobs, &mut ships);
    let storages = storage::level_storages(dimension);
    let anchors: BTreeMap<u32, Pos> = storages.iter().map(|(id, a, _)| (*id, *a)).collect();
    follow_jobs(
        game, dimension, level, &mut state, &mut jobs, &mut ships, &anchors,
    );
    for (id, anchor, terminals) in &storages {
        let started = storage_round(
            game, dimension, level, &mut state, &jobs, &mut ships, *id, *anchor, terminals,
        );
        jobs.extend(started);
    }
    // Storages that are gone take their orders with them.
    let live: BTreeSet<u32> = anchors.keys().copied().collect();
    state
        .queues
        .retain(|id, q| live.contains(id) && !q.orders.is_empty());
    state.smart.retain(|id, _| live.contains(id));
    // A book built anew gives the windows its items (O2).
    let book_built = !had_book && orders().book.is_some();
    if state != before || jobs.len() != job_count || book_built {
        state.revision = before.revision + 1;
    }
    let kept = {
        let mut o = orders();
        // A press meanwhile counted on too.
        if let Some(meanwhile) = o.levels.remove(dimension) {
            state.revision = state.revision.max(meanwhile.revision + 1);
            for (id, q) in meanwhile.queues {
                merge_presses(state.queues.entry(id).or_default(), q);
            }
        }
        let kept = records(&state, &jobs, &ships, &level.temp_pages);
        o.levels.insert(dimension.to_owned(), state);
        o.jobs.insert(dimension.to_owned(), jobs);
        o.ships.insert(dimension.to_owned(), ships);
        kept
    };
    storage::set_order_records(dimension, kept);
}

/// Presses cannot reach the queue while the tick has it, but keep anything a hook added.
fn merge_presses(queue: &mut Queue, meanwhile: Queue) {
    for o in meanwhile.orders {
        if !queue.orders.iter().any(|q| q.id == o.id) {
            queue.orders.push(o);
        }
    }
}

/// Each job: its machine's products sent to the storage; a done job counted to its order; a job
/// whose order was cancelled taken back; a job whose machine went dropped. A job read back from
/// the record takes its machine again, or waits for it a while. A job whose items still ride to
/// its machine waits for them.
fn follow_jobs(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelNet,
    state: &mut LevelOrders,
    jobs: &mut Vec<Running>,
    ships: &mut Vec<Shipment>,
    anchors: &BTreeMap<u32, Pos>,
) {
    let mut kept = Vec::with_capacity(jobs.len());
    for mut r in std::mem::take(jobs) {
        let pos = r.job.machine;
        let machine = level
            .machines
            .get(&pos)
            .copied()
            .filter(|m| kind_of(*m) == Some(r.job.step.machine.as_str()));
        let order_item = state
            .queues
            .get(&r.storage)
            .and_then(|q| q.orders.iter().find(|o| o.id == r.job.order))
            .map(|o| o.item.clone());
        let Some(machine) = machine else {
            if let Some(rs) = r.restored.as_mut()
                && rs.seconds_left > 0
            {
                rs.seconds_left -= 1;
                kept.push(r);
            } else {
                level.temp_pages.remove(&pos);
            }
            continue;
        };
        adopt(game, level, machine, &mut r);
        // Its items still ride: nothing is in the machine for it yet. A job whose order went
        // ends; its items turn back when they arrive.
        if ships
            .iter()
            .any(|s| s.bound_for(r.storage, r.job.order, pos))
        {
            if order_item.is_some() {
                kept.push(r);
            }
            continue;
        }
        let done = match machine {
            Machine::Crafter => level.temp_pages.get(&pos).map(|t| t.owed == 0),
            _ => Some(inputs_spent(game, machine, &r.job)),
        };
        let mut way = Way {
            storage: r.storage,
            order: r.job.order,
            route: route_of(level, anchors.get(&r.storage).copied(), pos),
            ships: &mut *ships,
        };
        let output = &r.job.step.output.0;
        let own = order_item.as_ref().is_some_and(|item| item != output);
        let pulled = pull(game, dimension, &mut way, pos, machine, output, own);
        if own && let Some(q) = state.queues.get_mut(&r.storage) {
            q.keep_for(r.job.order, output, u64::from(pulled));
        }
        if order_item.is_none() {
            let left = match machine {
                Machine::Crafter => level.temp_pages.get(&pos).map_or(0, |t| t.owed),
                _ => r.job.runs,
            };
            take_back(game, dimension, &mut way, machine, &r.job, left);
            level.temp_pages.remove(&pos);
            continue;
        }
        match done {
            // A temporary page that is gone without its crafts: the level was read anew.
            None => {}
            Some(true) => {
                if let Some(q) = state.queues.get_mut(&r.storage) {
                    q.finished(&r.job, r.job.runs);
                }
                level.temp_pages.remove(&pos);
            }
            Some(false) => kept.push(r),
        }
    }
    *jobs = kept;
}

/// O12: what the storage holds, and the fluids mixer steps take as the line's mixers can draw
/// on them, in whole buckets.
fn stock_with_fluids(
    game: &Game<'_>,
    level: &LevelNet,
    mut stock: BTreeMap<ItemKey, u64>,
    book: &Book,
    mixers: &[Pos],
) -> BTreeMap<ItemKey, u64> {
    if mixers.is_empty() {
        return stock;
    }
    for fluid in &book.fluids_in {
        let mb = fluids::reachable(game, level, mixers, fluid, server::tuning());
        if mb >= BUCKET {
            stock.insert(fluid_key(fluid), mb / BUCKET);
        }
    }
    stock
}

/// One storage's round: its line, its «держать» orders and the jobs to start, started. The jobs
/// that went to machines. An order whose items ride the item channel counts as running.
#[allow(clippy::too_many_arguments)]
fn storage_round(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelNet,
    state: &mut LevelOrders,
    jobs: &[Running],
    ships: &mut Vec<Shipment>,
    id: u32,
    anchor: Pos,
    terminals: &[Pos],
) -> Vec<Running> {
    let (smart, machines_on_line) = line_state(level, anchor);
    state.smart.insert(id, smart);
    let book_in = terminals.iter().any(|t| {
        game.native_mod_block_entity(*t)
            .is_some_and(|e| has_book(&e.nbt))
    });
    let Some((stock, keeps)) = storage::stock(dimension, id) else {
        return Vec::new();
    };
    let has_queue = state.queues.get(&id).is_some_and(|q| !q.orders.is_empty());
    if !(has_queue || book_in) {
        return Vec::new();
    }
    // The window's search needs the book too (O2), orders or not.
    let book = book_of(game, &level.recipes);
    if smart != Smart::Works || !(has_queue || !keeps.is_empty()) {
        return Vec::new();
    }
    let riding: BTreeSet<u16> = ships
        .iter()
        .filter(|s| s.storage == id)
        .map(|s| s.order)
        .collect();
    let running = |order: u16| {
        riding.contains(&order) || jobs.iter().any(|r| r.storage == id && r.job.order == order)
    };
    let queue = state.queues.entry(id).or_default();
    if book_in {
        queue.keep(&keeps, &stock, &running);
    }
    let busy: BTreeSet<Pos> = jobs.iter().map(|r| r.job.machine).collect();
    let mut kinds: Vec<&str> = Vec::new();
    let mut free: Vec<FreeMachine> = Vec::new();
    let mut with_energy: BTreeMap<Pos, bool> = BTreeMap::new();
    for &(pos, machine) in &machines_on_line {
        let Some(kind) = kind_of(machine) else {
            continue;
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
        let is_powered = powered(game, level, pos, machine);
        with_energy.insert(pos, is_powered);
        let idle = match machine {
            Machine::Crafter => !level.temp_pages.contains_key(&pos),
            _ => read_slots(game, pos, slots_of(machine).0)
                .is_some_and(|s| s.iter().all(Option::is_none)),
        };
        if idle && !busy.contains(&pos) {
            free.push(FreeMachine {
                pos,
                kind: kind.to_owned(),
                powered: is_powered,
            });
        }
    }
    let mixers: Vec<Pos> = machines_on_line
        .iter()
        .filter(|(_, m)| *m == Machine::Mixer)
        .map(|(p, _)| *p)
        .collect();
    let plan_stock = stock_with_fluids(game, level, stock, &book, &mixers);
    let machine_at: BTreeMap<Pos, Machine> = machines_on_line.iter().copied().collect();
    let new_jobs = {
        let recipes = &mut level.recipes;
        let mut fits = |pos: Pos, step: &Step, want: u32| -> u32 {
            let Some(&machine) = machine_at.get(&pos) else {
                return 0;
            };
            if machine == Machine::Crafter {
                let pattern: Pattern = step.pattern(&book.recipes);
                let makes = crafter::makes(game, recipes, &pattern);
                if makes.as_ref().map(|(i, n)| (i.as_str(), *n))
                    != Some((step.output.0.as_str(), step.output.1))
                {
                    return 0;
                }
            }
            let Some(slots) = read_slots(game, pos, slots_of(machine).0) else {
                return 0;
            };
            let items = step.items();
            let sizes = stacks(game, &items);
            let size = |item: &str| sizes.get(item).copied().unwrap_or(machines::FALLBACK_STACK);
            runs_that_fit(&slots, &items, want, &size)
        };
        queue.round(
            &plan_stock,
            &book.recipes,
            &kinds,
            &free,
            &running,
            &mut fits,
        )
    };
    let mut started = Vec::new();
    for job in new_jobs {
        let Some(&machine) = machine_at.get(&job.machine) else {
            continue;
        };
        let mut way = Way {
            storage: id,
            order: job.order,
            route: route_of(level, Some(anchor), job.machine),
            ships: &mut *ships,
        };
        let sent = deliver(game, dimension, &mut way, machine, &job);
        if sent == Sent::No {
            continue;
        }
        queue.started(&job);
        // Along the channel the page comes when the items arrive.
        if machine == Machine::Crafter && sent == Sent::Placed {
            level.temp_pages.insert(
                job.machine,
                TempPage {
                    pattern: job.step.pattern(&book.recipes),
                    owed: job.runs,
                },
            );
        }
        started.push(Running {
            storage: id,
            job,
            restored: None,
        });
    }
    // An order whose jobs all failed to start waits again.
    for o in &mut queue.orders {
        let has_job = started.iter().any(|r| r.job.order == o.id) || running(o.id);
        if o.wait == Wait::Running && !has_job {
            o.wait = Wait::New;
        }
    }
    // An order whose jobs all stand on machines without energy says so. A machine off the line
    // is not this round's to judge.
    let own_jobs: Vec<Job> = jobs
        .iter()
        .chain(started.iter())
        .filter(|r| r.storage == id && r.restored.is_none())
        .map(|r| r.job.clone())
        .collect();
    queue.wait_for_energy(&own_jobs, &|pos| {
        with_energy.get(&pos).copied().unwrap_or(true)
    });
    queue.remove_done();
    started
}
