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
// O11: a product made for a later step of the same order goes straight to the next step's
// machine (slice 18, spec 19: «промежуточный предмет идёт прямо в следующую машину заказа, если она
// на той же магистрали»). It leaves the machine's output slots into the order's record «held at
// machine A» (rustcraft_rules::orders::held), the order's own stock for its rounds and nobody
// else's, not in the storage's counts; while a machine runs such a job the item nets' own moves
// leave its product be (`kept_outputs`, read by rustcraft_transport). The next step's job takes
// the held items first and they ride from machine A to its machine on the net that joins the
// two (straight in where none does), the rest from the storage as before. Held items the order's
// plan no longer needs, and those of an order cancelled or done, ride to the storage. A product
// that rode back to the storage before slice 18 (a record's `Leg::Products { own: true }`) still
// lands as the order's own (rustcraft_rules::orders::queue::Order::made).
//
// O12: the mixer's recipes are in the book with water as an input and their fluid as the output,
// both in buckets. A mixer step needs a mixer on the line and the water for it in the mixers'
// own tanks or the tanks they touch or share a fluid net with; the product goes on by the fluid
// nets. Water short reads «не хватает: вода ×N». Round 31 (spec 29, FL5, FL6): the storage's
// fluid stock counts too, every fluid in whole buckets (rustcraft_fluids::line_stock): what its
// cells hold and the tanks on its fluid nets. «Держать N» and a kitchen's milk (FL7) plan on it,
// and a mixer step's water comes from the cells first when the step starts
// (rustcraft_fluids::bring_water).
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
        held::{self, Held},
        queue::{FreeMachine, Job, Order, Queue, SMART_TIER, Wait, job_end, runs_that_fit},
        ride,
        saved::{self, Record, SavedJob, SavedPage},
        ship::{self, Leg, Route, Shipment},
    },
    storage::{
        keys,
        surplus::Source,
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
/// Round 31 (FL7): the milk a kitchen takes for a milk bucket.
const MILK: &str = "rustcraft:milk";

/// The recipe book (O1): every recipe the orders know, and the items and fluids they make as the
/// terminal's numbers.
pub(super) struct Book {
    recipes: Vec<PlanRecipe>,
    orderable: Vec<u16>,
    /// 0.21.1 (OT12): what names `orderable` in its own message
    /// (rustcraft_rules::storage::orderable::stamp).
    stamp: u32,
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
    /// Slice 18 (O11): the products the level's orders hold at machines for their next steps.
    held: HashMap<String, Vec<Held>>,
    /// Slice 18: the housings each level's riding items were last written into.
    ridden: HashMap<String, BTreeSet<Pos>>,
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

/// 0.21.1 (OT12): the items the recipe book can make and their stamp, for their message of their
/// own; none until the book is built.
pub(super) fn orderable_list() -> Option<(u32, Vec<u16>)> {
    orders().book.as_ref().map(|b| (b.stamp, b.orderable.clone()))
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
    held: &[Held],
    pages: &BTreeMap<Pos, TempPage>,
) -> BTreeMap<u32, Vec<String>> {
    let ids: BTreeSet<u32> = state
        .queues
        .keys()
        .copied()
        .chain(jobs.iter().map(|r| r.storage))
        .chain(ships.iter().map(|s| s.storage))
        .chain(held.iter().map(|h| h.storage))
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
                held: held.iter().filter(|h| h.storage == id).cloned().collect(),
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
    let mut held = Vec::new();
    for (&id, strings) in saved {
        let Some(record) = saved::decode(strings) else {
            crate::game::rustcraft_debug::journal_warn!("rustcraft: the orders of storage {id} were not read");
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
        held.extend(record.held.into_iter().map(|h| Held { storage: id, ..h }));
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
    o.held.insert(dimension.to_owned(), held);
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
    // 0.19.2: what the crusher grinds (ore and raw metal) goes to the furnace only as dust.
    // 0.21.2 (spec 50): only ore; cobblestone and clay, which it takes apart, still smelt whole.
    let crushed: std::collections::BTreeSet<ItemKey> = recipes
        .crushing
        .iter()
        .filter(|r| rustcraft_rules::machines::crushing::grinds_ore(&r.result.item))
        .flat_map(|r| r.inputs.iter().cloned())
        .collect();
    for c in game
        .recipes()
        .cooking
        .iter()
        .filter(|c| c.kind == machines::SMELTING)
    {
        if c.result.extra.is_some() || c.result.count == 0 {
            continue;
        }
        // 0.19.1: smelting tools and armour into nuggets is recycling, no way to make nuggets:
        // an order never crafts a pickaxe to melt it.
        let worn = |item: &str| game.items().get(item).is_some_and(|i| i.max_damage > 0);
        if !c.ingredient.items.is_empty() && c.ingredient.items.iter().all(|i| worn(i)) {
            continue;
        }
        let Some(inputs) = rustcraft_rules::orders::furnace_inputs(alternatives(&c.ingredient), &crushed) else {
            continue;
        };
        book.push(PlanRecipe {
            machine: KINDS[1].into(),
            inputs: vec![(inputs, 1)],
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
    let orderable: Vec<u16> = outputs
        .into_iter()
        .filter_map(|key| number(game, key))
        .collect();
    Book {
        recipes: book,
        stamp: rustcraft_rules::storage::orderable::stamp(&orderable),
        orderable,
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

/// 0.21 (OT8): the refusal of an order for an item no recipe makes; its `%s` is the item's name.
pub(super) const NOT_MADE: Refusal = (
    "rustcraft.orders.not_made",
    "%s is not made on machines: it is only gathered",
);

/// 0.21 (OT8): an order or a «держать» for `key` can be made: a recipe of the book makes it,
/// not only from itself. True while the book is not built yet: the queue's round drops what it
/// cannot make (OT9).
pub(super) fn can_order(key: &str) -> bool {
    orders()
        .book
        .as_ref()
        .is_none_or(|b| rustcraft_rules::orders::can_make(key, &b.recipes))
}

/// O3: an order for `count` of `item` (a fluid's key in buckets) at the storage `storage`; a
/// refusal when no recipe makes the item (OT8), its line takes no orders or its queue is full.
pub(super) fn order(dimension: &str, storage: u32, item: &str, count: u32) -> Result<(), Refusal> {
    if !can_order(item) {
        return Err(NOT_MADE);
    }
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
        && let Some(gone) = q.cancel(id)
    {
        level.revision += 1;
        crate::game::rustcraft_debug::order_ended(dimension, storage, &gone, "order_cancelled");
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

/// rustcraft_debug: every storage's orders of the level and the jobs on its machines (with the
/// storage each works for).
pub(super) fn debug_orders(dimension: &str) -> (Vec<(u32, Vec<Order>)>, Vec<(u32, Job)>) {
    let o = orders();
    let queues = o
        .levels
        .get(dimension)
        .map(|l| {
            l.queues
                .iter()
                .map(|(id, q)| (*id, q.orders.clone()))
                .collect()
        })
        .unwrap_or_default();
    let jobs = o
        .jobs
        .get(dimension)
        .map(|j| j.iter().map(|r| (r.storage, r.job.clone())).collect())
        .unwrap_or_default();
    (queues, jobs)
}

/// rustcraft_debug: what the level's orders hold at machines (O11) and what rides the item
/// channel for them.
pub(super) fn debug_riding(dimension: &str) -> (Vec<Held>, Vec<Shipment>) {
    let o = orders();
    (
        o.held.get(dimension).cloned().unwrap_or_default(),
        o.ships.get(dimension).cloned().unwrap_or_default(),
    )
}

/// rustcraft_debug: whether the storage at `anchor` takes orders, and every machine behind a
/// joint of an item net the storage is a joint of (not only the kinds orders use).
pub(super) fn debug_line(level: &LevelNet, anchor: Pos) -> (Smart, Vec<(Pos, Machine)>) {
    let (smart, _) = line_state(level, anchor);
    let machines = level
        .transport
        .around(anchor)
        .map(|(blocks, _)| {
            blocks
                .iter()
                .filter_map(|p| level.machines.get(p).map(|m| (*p, *m)))
                .collect()
        })
        .unwrap_or_default();
    (smart, machines)
}

// ---- What the terminal shows ----

/// What a terminal's list says of the orders (rustcraft_rules::storage::wire).
pub(super) struct View {
    pub book: bool,
    pub smart: Smart,
    pub orders: Vec<OrderLine>,
    pub orderable: Vec<u16>,
}

/// The terminal's number of a key: a group's (OT5), a fluid's (O12) or the item's in the item
/// table.
fn number(game: &Game<'_>, key: &str) -> Option<u16> {
    // 0.21 (OT5): a group an order waits for («доски любого дерева»).
    if let Some(n) = rustcraft_rules::orders::groups::group_number(key) {
        return Some(n);
    }
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

/// 0.18.5: the kitchen dishes (KITCHEN_RECIPES indices) some order makes now, in any level: a
/// kitchen takes their ingredients besides its pages' (rustcraft_machines::slot_takes, which has
/// no level at hand).
pub(super) fn ordered_dishes() -> Vec<usize> {
    use rustcraft_rules::utility::KITCHEN_RECIPES;
    let o = orders();
    let mut dishes: Vec<usize> = o
        .jobs
        .values()
        .flatten()
        .filter_map(|r| KITCHEN_RECIPES.iter().position(|d| *d == r.job.step.output.0.as_str()))
        .collect();
    dishes.sort_unstable();
    dishes.dedup();
    dishes
}

/// 0.20 (round 31): what the storages' own ore feed leaves alone (rustcraft_rules::net::items,
/// `Inventories::busy`): the machines an order job runs on, and the anchors of the storages that
/// have orders, whose crushers are the orders' while they wait.
pub(super) fn busy_machines(dimension: &str) -> BTreeSet<Pos> {
    let (mut busy, with_orders): (BTreeSet<Pos>, BTreeSet<u32>) = {
        let o = orders();
        let jobs = o.jobs.get(dimension).map(|j| j.iter().map(|r| r.job.machine).collect()).unwrap_or_default();
        let ids = o
            .levels
            .get(dimension)
            .map(|l| l.queues.iter().filter(|(_, q)| !q.orders.is_empty()).map(|(id, _)| *id).collect())
            .unwrap_or_default();
        (jobs, ids)
    };
    if !with_orders.is_empty() {
        busy.extend(
            storage::level_storages(dimension)
                .into_iter()
                .filter(|(id, _, _)| with_orders.contains(id))
                .map(|(_, anchor, _)| anchor),
        );
    }
    busy
}

/// Slice 18 (O11): the machines of the level whose order job makes a product for a later step of
/// its order, and that product: the item nets' own moves leave it in their output slots, for
/// the order takes it straight to the next step's machine (rustcraft_transport's snapshot).
pub(super) fn kept_outputs(dimension: &str) -> BTreeMap<Pos, ItemKey> {
    let o = orders();
    let (Some(jobs), Some(level)) = (o.jobs.get(dimension), o.levels.get(dimension)) else {
        return BTreeMap::new();
    };
    jobs.iter()
        .filter(|r| {
            level
                .queues
                .get(&r.storage)
                .and_then(|q| q.orders.iter().find(|x| x.id == r.job.order))
                .is_some_and(|x| x.item != r.job.step.output.0)
        })
        .map(|r| (r.job.machine, r.job.step.output.0.clone()))
        .collect()
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

/// What a job took for its items: from the storage, and (O11) from what its order held at
/// machines, each with the machine it waited at.
#[derive(Default)]
struct Taken {
    storage: Vec<(ItemKey, u64)>,
    held: Vec<(Pos, ItemKey, u64)>,
}

impl Taken {
    /// All of it goes back where it was taken from.
    fn undo(&self, dimension: &str, storage_id: u32, order: u16, held: &mut Vec<Held>) {
        for (item, n) in &self.storage {
            storage::restore_into(dimension, storage_id, item, *n);
        }
        for (machine, item, n) in &self.held {
            held::add(held, storage_id, order, *machine, item, *n);
        }
    }

    /// How many of each item came from what the order held.
    fn from_held(&self) -> BTreeMap<ItemKey, u64> {
        let mut out = BTreeMap::new();
        for (_, item, n) in &self.held {
            *out.entry(item.clone()).or_insert(0) += n;
        }
        out
    }
}

fn counts(items: &[(ItemKey, u64)]) -> Vec<(ItemKey, u32)> {
    items
        .iter()
        .map(|(i, n)| (i.clone(), u32::try_from(*n).unwrap_or(u32::MAX)))
        .collect()
}

/// A job's items into the machine's inputs or onto the item channel to it: first what its order
/// holds at machines (O11), riding from each of those machines straight to this one, then the
/// rest out of the storage. A fluid input is not the storage's to give: the fluid nets bring it
/// (O12). What came from the order's held items, by item.
fn deliver(
    game: &mut Game<'_>,
    dimension: &str,
    level: &LevelNet,
    held: &mut Vec<Held>,
    way: &mut Way<'_>,
    machine: Machine,
    job: &Job,
) -> (Sent, BTreeMap<ItemKey, u64>) {
    let wants: Vec<(ItemKey, u64)> = job
        .step
        .items()
        .iter()
        .map(|(i, n)| (i.clone(), u64::from(*n) * u64::from(job.runs)))
        .collect();
    let storage_id = way.storage;
    let mut taken = Taken::default();
    for (item, want) in &wants {
        let mut got = 0;
        for (at, n) in held::take(held, storage_id, job.order, item, *want) {
            taken.held.push((at, item.clone(), n));
            got += n;
        }
        if got == *want {
            continue;
        }
        let from_storage = storage::take_from(dimension, storage_id, item, want - got);
        taken.storage.push((item.clone(), from_storage));
        if from_storage < want - got {
            taken.undo(dimension, storage_id, job.order, held);
            return (Sent::No, BTreeMap::new());
        }
    }
    // Each part straight into the machine, or riding from where it is: the storage's part on the
    // way's route, a held part on the net between its machine and this one.
    let mut direct: Vec<(ItemKey, u64)> = Vec::new();
    let mut rides: Vec<(Option<Pos>, Route, Vec<(ItemKey, u64)>)> = Vec::new();
    let storage_part: Vec<(ItemKey, u64)> =
        taken.storage.iter().filter(|(_, n)| *n > 0).cloned().collect();
    if !storage_part.is_empty() {
        match way.route {
            Some(route) => rides.push((None, route, storage_part)),
            None => direct.extend(storage_part),
        }
    }
    let machines_held: BTreeSet<Pos> = taken.held.iter().map(|(at, _, _)| *at).collect();
    for at in machines_held {
        let part: Vec<(ItemKey, u64)> = taken
            .held
            .iter()
            .filter(|(m, _, _)| *m == at)
            .map(|(_, i, n)| (i.clone(), *n))
            .collect();
        let route = (at != job.machine)
            .then(|| {
                level
                    .transport
                    .route(&level.layout, server::tuning(), at, job.machine)
            })
            .flatten();
        match route {
            Some(route) => rides.push((Some(at), route, part)),
            None => direct.extend(part),
        }
    }
    if !rides.is_empty() && game.native_mod_block_entity(job.machine).is_none() {
        taken.undo(dimension, storage_id, job.order, held);
        return (Sent::No, BTreeMap::new());
    }
    if !direct.is_empty() && !place_inputs(game, machine, job.machine, &direct) {
        taken.undo(dimension, storage_id, job.order, held);
        return (Sent::No, BTreeMap::new());
    }
    // rustcraft_debug: what left the storage for this job, summed a second by item.
    let to = format!(
        "storage#{storage_id} -> {}({}) order#{}",
        machine.block(),
        rustcraft_rules::debug::pos_text(job.machine),
        job.order
    );
    for (item, n) in taken.storage.iter().filter(|(_, n)| *n > 0) {
        crate::game::rustcraft_debug::flow(dimension, &to, item, *n);
    }
    let tps = server::tuning().ticks_per_second;
    let sent = if rides.is_empty() {
        Sent::Placed
    } else {
        Sent::Riding
    };
    for (from, route, part) in rides {
        let mut s = Shipment::new(
            storage_id,
            job.order,
            Leg::Machine {
                machine: job.machine,
            },
            counts(&part),
            &route,
            tps,
        );
        s.from = from;
        way.ships.push(s);
    }
    (sent, taken.from_held())
}

/// O11: the plain stacks of `item` in the machine's output slots, made for a later step of the
/// order `order`, leave them and are held for it at the machine.
fn hold(
    game: &mut Game<'_>,
    held: &mut Vec<Held>,
    storage_id: u32,
    order: u16,
    pos: Pos,
    machine: Machine,
    item: &str,
) {
    let (_, outputs) = slots_of(machine);
    if outputs.is_empty() || fluid_of(item).is_some() {
        return;
    }
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return;
    };
    let plain: u32 = outputs
        .clone()
        .filter_map(|i| entity.items.get(i).cloned().flatten())
        .filter(|s| &*s.item == item && s.extra.is_none() && s.damage == 0)
        .map(|s| s.count)
        .sum();
    if plain == 0 {
        return;
    }
    let written = game.native_with_mod_block_entity(pos, |e| {
        let before: Vec<Slot> = outputs.clone().map(|i| slot_of(e, i)).collect();
        let mut after = before.clone();
        let left = slots::take_from(&mut after, item, plain);
        write_slots(e, outputs.clone(), &before, &after, &[]);
        plain - left
    });
    if let Some(took) = written {
        held::add(held, storage_id, order, pos, item, u64::from(took));
    }
}

/// O11: the products held at machines that their orders no longer need (the order is gone, or
/// its plan of this round wants fewer) go to the storage, riding from their machine where a net
/// joins the two; those of a storage that is gone drop where they wait.
fn release_held(
    game: &mut Game<'_>,
    dimension: &str,
    level: &LevelNet,
    state: &LevelOrders,
    held: &mut Vec<Held>,
    ships: &mut Vec<Shipment>,
    anchors: &BTreeMap<u32, Pos>,
) {
    let storages: BTreeSet<u32> = held.iter().map(|h| h.storage).collect();
    for id in storages {
        let queue = state.queues.get(&id);
        let anchor = anchors.get(&id).copied();
        // A storage that is gone needs nothing.
        let need = |order: u16| -> Option<Option<BTreeMap<ItemKey, u64>>> {
            anchor?;
            queue
                .and_then(|q| q.orders.iter().find(|o| o.id == order))
                .map(|o| o.held_need.clone())
        };
        for h in held::release(held, id, &need) {
            let Some(anchor) = anchor else {
                let mut left = h.count;
                while left > 0 {
                    let part = left.min(u64::from(DROP_STACK));
                    storage::drop_stack(game, dimension, h.machine, &h.item, part as u32);
                    left -= part;
                }
                continue;
            };
            let route = level
                .transport
                .route(&level.layout, server::tuning(), h.machine, anchor);
            match route {
                // Order 0 is no order's: the ride holds no order back.
                Some(route) => ships.push(Shipment::new(
                    id,
                    0,
                    Leg::Back { from: h.machine },
                    vec![(h.item.clone(), u32::try_from(h.count).unwrap_or(u32::MAX))],
                    &route,
                    server::tuning().ticks_per_second,
                )),
                None => storage::restore_into(dimension, id, &h.item, h.count),
            }
        }
    }
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
        let room = storage::room_in(game, dimension, storage_id, item, Source::Order).saturating_sub(on_the_way);
        u32::try_from(room).unwrap_or(u32::MAX).min(plain)
    } else {
        let took = storage::put_into(game, dimension, storage_id, item, u64::from(plain), Source::Order);
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

/// The block entity's key of a machine's hidden input cells: the kitchen's own, the queue cells of
/// the others' modules.
fn hidden_key(machine: Machine) -> &'static str {
    match machine {
        Machine::Util(rustcraft_rules::utility::Util::Kitchen) => crate::game::rustcraft_utility::KITCHEN_CELLS_KEY,
        _ => machines::QUEUE_KEY,
    }
}

/// 0.21 (spec 39): the machines that take their recipes' ingredients from the storage themselves:
/// the mixer and the kitchen.
fn fetches_itself(machine: Machine) -> bool {
    matches!(machine, Machine::Mixer | Machine::Util(rustcraft_rules::utility::Util::Kitchen))
}

/// 0.21 (spec 39, RT6): an order's job starts on a mixer or a kitchen: the plain stacks its inputs
/// and hidden cells hold (what it took for its own recipes, what a hand or a hopper put in) go
/// back to the storage `id`, and the job's items go in alone, so its end is counted as before.
fn evict(game: &mut Game<'_>, dimension: &str, id: u32, machine: Machine, pos: Pos) {
    let (inputs, _) = slots_of(machine);
    let key = hidden_key(machine);
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return;
    };
    let plain = |s: &horizon_client::items::ItemStack| s.count > 0 && s.damage == 0 && s.extra.is_none();
    let back: Vec<usize> = inputs
        .clone()
        .filter(|i| entity.items.get(*i).and_then(Option::as_ref).is_some_and(plain))
        .collect();
    let mut gone: Vec<(ItemKey, u32)> = back
        .iter()
        .filter_map(|i| entity.items.get(*i).cloned().flatten())
        .map(|s| (storage::net_key(&s), s.count))
        .collect();
    let cells: Vec<Slot> = crate::game::rustcraft_utility::read_json(&entity.nbt, key);
    drop(entity);
    gone.extend(cells.iter().flatten().cloned());
    if gone.is_empty() {
        return;
    }
    game.native_with_mod_block_entity(pos, |e| {
        for i in &back {
            if let Some(s) = e.items.get_mut(*i) {
                *s = None;
            }
        }
        if !cells.is_empty() {
            crate::game::rustcraft_utility::write_json(&mut e.nbt, key, &Vec::<Slot>::new());
        }
    });
    let route = format!("({}) -> storage#{id}, an order's job", rustcraft_rules::debug::pos_text(pos));
    for (item, n) in gone {
        storage::restore_into(dimension, id, &item, u64::from(n));
        crate::game::rustcraft_debug::flow(dimension, &route, &item, u64::from(n));
    }
}

/// What a machine still holds of each of the job's items: its inputs and its hidden cells (wave
/// 2: the kitchen's, which a stack or capacity module fills from its slots; 0.21: the queue cells
/// of the other machines' modules too). None when the machine is gone.
fn job_items_left(game: &Game<'_>, machine: Machine, job: &Job) -> Option<BTreeMap<ItemKey, u64>> {
    let (inputs, _) = slots_of(machine);
    let key = hidden_key(machine);
    let hidden: Vec<Slot> = game
        .native_mod_block_entity(job.machine)
        .map(|e| crate::game::rustcraft_utility::read_json(&e.nbt, key))
        .unwrap_or_default();
    let slots = read_slots(game, job.machine, inputs)?;
    Some(
        job.step
            .items()
            .iter()
            .map(|(item, _)| {
                let n = slots::count_in(&slots, item) + slots::count_in(&hidden, item);
                (item.clone(), u64::from(n))
            })
            .collect(),
    )
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
        // rustcraft_debug: a job's items dropped where they came from, their storage gone.
        let to = format!("({}) -> ground, storage#{} gone", rustcraft_rules::debug::pos_text(from), s.storage);
        for (item, n) in &s.items {
            crate::game::rustcraft_debug::flow(dimension, &to, item, u64::from(*n));
        }
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
    // rustcraft_debug: what lands back in the storage from a job.
    let to = format!("({}) -> storage#{} order#{}", rustcraft_rules::debug::pos_text(from), s.storage, s.order);
    for (item, n) in &s.items {
        crate::game::rustcraft_debug::flow(dimension, &to, item, u64::from(*n));
    }
    for (item, n) in &s.items {
        let n = u64::from(*n);
        let took = match own {
            Some(false) => storage::put_into(game, dimension, s.storage, item, n, Source::Order),
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
                    // Slice 18 (O11): a job's items may come in two shipments (from the machine
                    // that made them and from the storage): the second does not give the page
                    // back the crafts the first let it make, only what its buffer can make now.
                    let buffer = read_slots(game, machine, content::CRAFTER_BUFFER).unwrap_or_default();
                    let can = saved::owed_after_load(r.job.runs, &r.job.step.inputs, &buffer);
                    let page = level.temp_pages.entry(machine).or_insert_with(|| TempPage {
                        pattern: r.job.step.pattern(&book.recipes),
                        owed: r.job.runs,
                    });
                    page.owed = page.owed.max(can);
                }
            }
            Leg::Products { from, own } => {
                land(game, dimension, state, &live, &s, from, Some(own));
            }
            Leg::Back { from } => land(game, dimension, state, &live, &s, from, None),
        }
    }
    // 0.21 (spec 36, OD4): a job that ends because its items no longer fit leaves its machine
    // free: what an earlier shipment of it already put in goes back to the storage, and an
    // autocrafter's temporary page goes, instead of keeping the machine from every later order.
    if !failed.is_empty() {
        let anchors: BTreeMap<u32, Pos> = storage::level_storages(dimension)
            .into_iter()
            .map(|(id, anchor, _)| (id, anchor))
            .collect();
        for r in jobs.iter().filter(|r| failed.contains(&(r.storage, r.job.order, r.job.machine))) {
            let Some(machine) = level.machines.get(&r.job.machine).copied() else {
                continue;
            };
            let mut way = Way {
                storage: r.storage,
                order: r.job.order,
                route: route_of(level, anchors.get(&r.storage).copied(), r.job.machine),
                ships: &mut kept,
            };
            take_back(game, dimension, &mut way, machine, &r.job, u32::MAX);
            level.temp_pages.remove(&r.job.machine);
        }
    }
    jobs.retain(|r| !failed.contains(&(r.storage, r.job.order, r.job.machine)));
    *ships = kept;
}

// ---- Riding items seen in the housings ----
//
// Slice 18 (guide `orders`, the handbook's scene of items moving in the housings): once a second
// the server lays each shipment on the housings between its two joints
// (rustcraft_transport::Transport::path) and writes into each housing the items that ride or
// will ride through it, with the faces they come in and leave by and the game ticks they do
// (rustcraft_rules::orders::ride), clears the housings no longer ridden, and sends the changed
// ones to clients (`native_block_entity_sync`, rustcraft_pipe18::update_tag). A client draws
// each item at its place from the frame's game time and partial tick (`draw_riding`,
// `native_draw_item`), and keeps no geometry while items ride. Plain item-net moves are instant
// per tick and have no riding item: only orders' shipments are drawn.

/// Proposal: the size a riding item is drawn at, of a block.
const RIDING_SCALE: f32 = 0.35;

/// The blocks a shipment rides from and to.
fn ends_of(s: &Shipment, anchor: Pos) -> (Pos, Pos) {
    match s.leg {
        Leg::Machine { machine } => (s.from.unwrap_or(anchor), machine),
        Leg::Products { from, .. } | Leg::Back { from } => (from, anchor),
    }
}

fn ride_items(nbt: &Compound) -> Vec<ItemKey> {
    match nbt.get(&JavaString::from(ride::RIDE_ITEMS_KEY)) {
        Some(Tag::List(list)) => list
            .iter()
            .filter_map(|t| match t {
                Tag::String(s) => Some(s.to_string_lossy()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn ride_ints(nbt: &Compound) -> Vec<i32> {
    match nbt.get(&JavaString::from(ride::RIDE_KEY)) {
        Some(Tag::IntArray(v)) => v.clone(),
        _ => Vec::new(),
    }
}

/// Each housing the level's shipments ride gets their items' spans; one no longer ridden is
/// cleared; a housing whose spans changed goes to clients.
fn show_rides(
    game: &mut Game<'_>,
    dimension: &str,
    level: &LevelNet,
    ships: &[Shipment],
    anchors: &BTreeMap<u32, Pos>,
) {
    let now = game.world.game_time;
    let tuning = server::tuning();
    let mut want: BTreeMap<Pos, Vec<(ItemKey, ride::Span)>> = BTreeMap::new();
    for s in ships {
        let Some(&anchor) = anchors.get(&s.storage) else {
            continue;
        };
        let (from, to) = ends_of(s, anchor);
        let Some(path) = level.transport.path(&level.layout, tuning, s.net, from, to) else {
            continue;
        };
        let kinds = s.items.iter().filter(|(_, n)| *n > 0).take(ride::MAX_KINDS);
        for (k, (item, _)) in kinds.enumerate() {
            let lag = ride::LAG_TICKS * k as i32;
            let drawn = keys::item_of(item).to_owned();
            for (housing, span) in ride::spans(&level.layout, &path, from, to, s, now, lag) {
                want.entry(housing).or_default().push((drawn.clone(), span));
            }
        }
    }
    let before = orders().ridden.remove(dimension).unwrap_or_default();
    let touched: BTreeSet<Pos> = before.iter().chain(want.keys()).copied().collect();
    for pos in touched {
        let (ints, items) = ride::encode(want.get(&pos).map_or(&[][..], Vec::as_slice));
        let Some(entity) = game.native_mod_block_entity(pos) else {
            continue;
        };
        if entity.id != content::HOUSING
            || (ride_ints(&entity.nbt) == ints && ride_items(&entity.nbt) == items)
        {
            continue;
        }
        game.native_with_mod_block_entity(pos, |e| {
            if ints.is_empty() {
                e.nbt.remove(&JavaString::from(ride::RIDE_KEY));
                e.nbt.remove(&JavaString::from(ride::RIDE_ITEMS_KEY));
            } else {
                let list = items
                    .iter()
                    .map(|i| Tag::String(JavaString::from(i.as_str())))
                    .collect();
                e.nbt
                    .insert(JavaString::from(ride::RIDE_KEY), Tag::IntArray(ints.clone()));
                e.nbt
                    .insert(JavaString::from(ride::RIDE_ITEMS_KEY), Tag::List(list));
            }
        });
        game.native_block_entity_sync(pos);
    }
    orders()
        .ridden
        .insert(dimension.to_owned(), want.keys().copied().collect());
}

/// On a client: the order items riding the housing (what the server wrote, `show_rides`), each
/// drawn where it is this frame; whether any of them has its ride through the housing still to
/// come or under way, so that its geometry is not kept (spans long over, as a restart may leave
/// them, keep it).
pub(super) fn draw_riding(
    game: &Game<'_>,
    entity: &horizon_client::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) -> bool {
    let riding = ride::decode(&ride_ints(&entity.nbt), &ride_items(&entity.nbt));
    let now = draw.game_time as i32;
    if !riding.iter().any(|(_, s)| now.wrapping_sub(s.t1) <= 0) {
        return false;
    }
    for (k, (item, span)) in riding.iter().enumerate() {
        let Some(at) = ride::place(*span, draw.game_time, draw.partial) else {
            continue;
        };
        draw.push();
        draw.translate(at);
        draw.scale([RIDING_SCALE; 3]);
        game.native_draw_item(
            draw,
            &horizon_client::items::ItemStack::new(item.as_str(), 1),
            horizon_assets::item_model::DisplayContext::Ground,
            None,
            k as i32,
        );
        draw.pop();
    }
    true
}

// ---- The tick ----

/// Once a second from the level's tick: the shipments ride on and those that arrived land, the
/// jobs looked at, the cancelled ones taken back, and a round of every storage's queue; then the
/// storages' records of their orders brought up to date.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &mut LevelNet) {
    let (mut state, mut jobs, mut ships, mut held) = {
        let mut o = orders();
        (
            o.levels.remove(dimension).unwrap_or_default(),
            o.jobs.remove(dimension).unwrap_or_default(),
            o.ships.remove(dimension).unwrap_or_default(),
            o.held.remove(dimension).unwrap_or_default(),
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
        game, dimension, level, &mut state, &mut jobs, &mut ships, &mut held, &anchors,
    );
    for (id, anchor, terminals) in &storages {
        let started = storage_round(
            game, dimension, level, &mut state, &jobs, &mut ships, &mut held, *id, *anchor,
            terminals,
        );
        jobs.extend(started);
        feed_crafters(game, dimension, level, &jobs, *id, *anchor);
    }
    // Slice 18 (O11): held products no order needs go to the storage.
    release_held(game, dimension, level, &state, &mut held, &mut ships, &anchors);
    // 0.21 (owner: «остатки … возвращали обратно на склад»): inputs left standing in a machine.
    return_stranded(game, dimension, level, &jobs, &mut ships, &anchors);
    // Slice 18: the shipments' items in the housings they ride, for clients to draw.
    show_rides(game, dimension, level, &ships, &anchors);
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
        let kept = records(&state, &jobs, &ships, &held, &level.temp_pages);
        o.levels.insert(dimension.to_owned(), state);
        o.jobs.insert(dimension.to_owned(), jobs);
        o.ships.insert(dimension.to_owned(), ships);
        o.held.insert(dimension.to_owned(), held);
        kept
    };
    storage::set_order_records(dimension, kept);
}

// ---- 0.21: what is left standing in a machine ----
//
// The owner (2026-11-19): «если к ним уходит что-то на выполнение заказа, то чтобы остатки, они
// возвращали обратно на склад, потому что у меня много из-за этого теряется ресурсов … особенно
// вот в плавильне сплавов». A job's own leftovers go back when it ends (spec 36, OD2); this takes
// what stays behind any other way (an order cancelled or ended before 0.21, what the 0.20 line
// pushed in, half a recipe nobody brings the rest of): an alloy smelter, a furnace or a crusher
// on a storage's item net that holds inputs, has no job and stands idle on the same inputs for
// `STRANDED_SECONDS` sends them back to that storage, all but what its joints' allow lists name
// (those the line brings on purpose, spec 34).

/// Seconds a machine stands idle on the same inputs before they go back to the storage.
const STRANDED_SECONDS: u32 = 30;

/// Each level's idle machines: a fingerprint of their inputs and the seconds it has stood.
static STRANDED: LazyLock<Mutex<HashMap<String, HashMap<Pos, (u64, u32)>>>> = LazyLock::new(Mutex::default);

fn stranded_state() -> MutexGuard<'static, HashMap<String, HashMap<Pos, (u64, u32)>>> {
    STRANDED.lock().unwrap_or_else(PoisonError::into_inner)
}

fn fingerprint(slots: &[Slot]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    slots.hash(&mut h);
    h.finish()
}

/// Inputs left standing in idle machines with no job go back to the storage on their net.
fn return_stranded(
    game: &mut Game<'_>,
    dimension: &str,
    level: &LevelNet,
    jobs: &[Running],
    ships: &mut Vec<Shipment>,
    anchors: &BTreeMap<u32, Pos>,
) {
    let idle: Vec<(Pos, Machine)> = level
        .power
        .status
        .iter()
        .filter(|(_, s)| matches!(s.shown, Shown::Idle))
        .filter(|(_, s)| matches!(s.machine, Machine::Alloy | Machine::Furnace | Machine::Crusher))
        .map(|(p, s)| (*p, s.machine))
        .collect();
    let mut seen: HashMap<Pos, (u64, u32)> = HashMap::new();
    let before = stranded_state().remove(dimension).unwrap_or_default();
    for (pos, machine) in idle {
        let busy = jobs.iter().any(|r| r.job.machine == pos)
            || ships.iter().any(|s| s.leg == Leg::Machine { machine: pos });
        if busy {
            continue;
        }
        let (inputs, _) = slots_of(machine);
        let Some(slots) = read_slots(game, pos, inputs.clone()) else {
            continue;
        };
        if slots.iter().all(Option::is_none) {
            continue;
        }
        let print = fingerprint(&slots);
        let stood = match before.get(&pos) {
            Some((p, n)) if *p == print => n + 1,
            _ => 1,
        };
        if stood < STRANDED_SECONDS {
            seen.insert(pos, (print, stood));
            continue;
        }
        let Some(id) = storage::storage_by(dimension, pos, &level.topology) else {
            seen.insert(pos, (print, stood));
            continue;
        };
        let filters = crate::game::rustcraft_utility::joint_filters(game, pos);
        let back: Vec<(ItemKey, u32)> = game
            .native_with_mod_block_entity(pos, |e| {
                let before: Vec<Slot> = inputs.clone().map(|i| slot_of(e, i)).collect();
                let mut after = before.clone();
                let mut back = Vec::new();
                for slot in after.iter_mut() {
                    let keep = slot
                        .as_ref()
                        .is_none_or(|(item, _)| rustcraft_rules::machines::intake::named(&filters, item));
                    if !keep && let Some((item, n)) = slot.take() {
                        back.push((item, n));
                    }
                }
                write_slots(e, inputs.clone(), &before, &after, &[]);
                back
            })
            .unwrap_or_default();
        if back.is_empty() {
            seen.insert(pos, (print, stood));
            continue;
        }
        let route = route_of(level, anchors.get(&id).copied(), pos);
        let to = format!("({}) -> storage#{id} left standing", rustcraft_rules::debug::pos_text(pos));
        for (item, n) in &back {
            crate::game::rustcraft_debug::flow(dimension, &to, item, u64::from(*n));
        }
        let mut way = Way { storage: id, order: 0, route, ships };
        if way.route.is_some() {
            way.send(Leg::Back { from: pos }, back);
        } else {
            for (item, n) in back {
                storage::restore_into(dimension, id, &item, u64::from(n));
            }
        }
    }
    stranded_state().insert(dimension.to_owned(), seen);
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
    held: &mut Vec<Held>,
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
        // 0.21 (spec 36, OD2): a machine's job ends once it holds less than a whole run of the
        // job's items, not only when it holds none: what is left (an item the line or a hopper
        // put in, a run's part that never came) goes back to the storage below, so the machine
        // is free for the next order instead of held by the job for good. None: the runs done;
        // `leftover`: something of the job's items is left in the machine.
        let (done, leftover) = match machine {
            Machine::Crafter => (level.temp_pages.get(&pos).map(|t| (t.owed == 0).then_some(r.job.runs)), false),
            _ => match job_items_left(game, machine, &r.job) {
                None => (Some(Some(r.job.runs)), false),
                Some(left) => (
                    Some(job_end(&r.job.step.items(), r.job.runs, &left)),
                    left.values().any(|n| *n > 0),
                ),
            },
        };
        let mut way = Way {
            storage: r.storage,
            order: r.job.order,
            route: route_of(level, anchors.get(&r.storage).copied(), pos),
            ships: &mut *ships,
        };
        let output = &r.job.step.output.0;
        let own = order_item.as_ref().is_some_and(|item| item != output);
        if own {
            // Slice 18 (O11): held at the machine for the order's next step.
            hold(game, held, r.storage, r.job.order, pos, machine, output);
        } else {
            pull(game, dimension, &mut way, pos, machine, output, false);
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
            Some(Some(runs)) => {
                if let Some(q) = state.queues.get_mut(&r.storage) {
                    q.finished(&r.job, runs);
                }
                if leftover {
                    take_back(game, dimension, &mut way, machine, &r.job, u32::MAX);
                }
                level.temp_pages.remove(&pos);
            }
            Some(None) => kept.push(r),
        }
    }
    *jobs = kept;
}

/// O12, round 31 (FL5, FL6): what the storage holds, and every fluid of its stock in whole
/// buckets: its cells', its fluid line's tanks' and what the line's mixers can draw on.
fn stock_with_fluids(
    game: &Game<'_>,
    level: &LevelNet,
    mut stock: BTreeMap<ItemKey, u64>,
    owner: (u32, Pos),
    mixers: &[Pos],
) -> BTreeMap<ItemKey, u64> {
    for (fluid, mb) in fluids::line_stock(game, level, Some(owner), mixers, server::tuning()) {
        if mb >= BUCKET {
            stock.insert(fluid_key(&fluid), mb / BUCKET);
        }
    }
    stock
}

/// One storage's round: its line, its «держать» orders and the jobs to start, started. The jobs
/// that went to machines. An order whose items ride the item channel counts as running.
#[allow(clippy::too_many_arguments)]
/// Slice 18 (handed over from stream 1): «автокрафтер просит у склада недостающий ингредиент».
/// An autocrafter on the storage's line with no order job, whose first page short of something
/// the storage holds all of, gets one craft's worth into its buffer straight from the storage.
fn feed_crafters(game: &mut Game<'_>, dimension: &str, level: &LevelNet, jobs: &[Running], id: u32, anchor: Pos) {
    let (_, machines_on_line) = line_state(level, anchor);
    let tuning = server::tuning();
    for (pos, machine) in machines_on_line {
        if machine != Machine::Crafter || jobs.iter().any(|r| r.job.machine == pos) {
            continue;
        }
        let Some(entity) = crate::game::rustcraft_machines::entity(game, pos) else { continue };
        let (patterns, buffer) = crafter::pages_and_buffer(&entity, tuning);
        drop(entity);
        // The first page with anything in it that lacks something the storage has.
        let short = patterns
            .iter()
            .filter(|p| p.iter().any(Option::is_some))
            .map(|p| rustcraft_rules::orders::page_shortfall(p, &buffer))
            .find(|s| !s.is_empty() && s.iter().all(|(i, n)| storage::stored_count(dimension, id, i) >= u64::from(*n)));
        let Some(short) = short else { continue };
        let mut taken: Vec<(ItemKey, u64)> = Vec::new();
        for (item, n) in &short {
            let got = storage::take_from(dimension, id, item, u64::from(*n));
            taken.push((item.clone(), got));
        }
        if taken.iter().zip(&short).any(|((_, got), (_, n))| *got < u64::from(*n))
            || !place_inputs(game, machine, pos, &taken)
        {
            for (item, got) in taken {
                storage::restore_into(dimension, id, &item, got);
            }
        } else {
            // rustcraft_debug: the storage feeding an autocrafter outside any order.
            let to = format!(
                "storage#{id} -> rustcraft:crafter({}) feed",
                rustcraft_rules::debug::pos_text(pos)
            );
            for (item, got) in &taken {
                crate::game::rustcraft_debug::flow(dimension, &to, item, *got);
            }
        }
    }
}

fn storage_round(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelNet,
    state: &mut LevelOrders,
    jobs: &[Running],
    ships: &mut Vec<Shipment>,
    held: &mut Vec<Held>,
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
    let mixers: Vec<Pos> = machines_on_line
        .iter()
        .filter(|(_, m)| *m == Machine::Mixer)
        .map(|(p, _)| *p)
        .collect();
    // Round 31 (FL5): «держать N» of a fluid sees its buckets too.
    let plan_stock = stock_with_fluids(game, level, stock, (id, anchor), &mixers);
    let queue = state.queues.entry(id).or_default();
    if book_in {
        // OT8: «держать N» of an item no recipe makes orders nothing.
        let keeps: BTreeMap<ItemKey, u64> = keeps
            .into_iter()
            .filter(|(k, _)| rustcraft_rules::orders::can_make(k, &book.recipes))
            .collect();
        queue.keep(&keeps, &plan_stock, &running);
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
            // 0.21 (spec 39, RT6): a mixer or a kitchen holds the ingredients it took for its
            // recipes itself; it is free for an order all the same, and gives them back when the
            // job starts (`evict`).
            _ if fetches_itself(machine) => true,
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
    let machine_at: BTreeMap<Pos, Machine> = machines_on_line.iter().copied().collect();
    // Round 31 (FL7): the milk each kitchen of the line reaches; a step that takes milk from the
    // stock starts on a kitchen only for the runs that milk covers.
    let kitchen_milk: BTreeMap<Pos, u64> = machines_on_line
        .iter()
        .filter(|(_, m)| *m == Machine::Util(rustcraft_rules::utility::Util::Kitchen))
        .map(|&(p, _)| (p, fluids::touching_amount(game, level, p, MILK, server::tuning()).0))
        .collect();
    let new_jobs = {
        let recipes = &mut level.recipes;
        let mut fits = |pos: Pos, step: &Step, want: u32| -> u32 {
            let Some(&machine) = machine_at.get(&pos) else {
                return 0;
            };
            let mut want = want;
            if let Some(&milk) = kitchen_milk.get(&pos) {
                for (key, n) in &step.inputs {
                    if fluid_of(key) == Some(MILK) {
                        let runs = milk / (u64::from(*n).max(1) * BUCKET);
                        want = want.min(u32::try_from(runs).unwrap_or(u32::MAX));
                    }
                }
            }
            if machine == Machine::Crafter {
                let pattern: Pattern = step.pattern(&book.recipes);
                let makes = crafter::makes(game, recipes, &pattern);
                if makes.as_ref().map(|(i, n)| (i.as_str(), *n))
                    != Some((step.output.0.as_str(), step.output.1))
                {
                    return 0;
                }
            }
            let Some(mut slots) = read_slots(game, pos, slots_of(machine).0) else {
                return 0;
            };
            // RT6: what it holds goes back before the job's items come.
            if fetches_itself(machine) {
                slots.iter_mut().for_each(|s| *s = None);
            }
            let items = step.items();
            let sizes = stacks(game, &items);
            let size = |item: &str| sizes.get(item).copied().unwrap_or(machines::FALLBACK_STACK);
            runs_that_fit(&slots, &items, want, &size)
        };
        queue.round_with(
            &plan_stock,
            &held::by_order(held, id),
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
        if fetches_itself(machine) {
            evict(game, dimension, id, machine, job.machine);
        }
        let (sent, from_held) = deliver(game, dimension, level, held, &mut way, machine, &job);
        if sent == Sent::No {
            continue;
        }
        queue.started_with(&job, &from_held);
        // What the job took of the order's held items, its plan needs held no more.
        if let Some(need) = queue
            .orders
            .iter_mut()
            .find(|o| o.id == job.order)
            .and_then(|o| o.held_need.as_mut())
        {
            for (item, n) in &from_held {
                if let Some(x) = need.get_mut(item) {
                    *x = x.saturating_sub(*n);
                }
            }
        }
        // Slice 18 (O12): a mixer step's water comes with the order.
        if machine == Machine::Mixer {
            let water: u64 = job
                .step
                .inputs
                .iter()
                .filter(|(k, _)| fluid_of(k) == Some(WATER))
                .map(|(_, n)| u64::from(*n) * u64::from(job.runs) * BUCKET)
                .sum();
            if water > 0 {
                crate::game::rustcraft_fluids::bring_water(
                    game,
                    level,
                    job.machine,
                    water,
                    Some((id, anchor)),
                    server::tuning(),
                );
            }
        }
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
    for done in queue.remove_done() {
        crate::game::rustcraft_debug::order_ended(dimension, id, &done, "order_finished");
    }
    started
}
