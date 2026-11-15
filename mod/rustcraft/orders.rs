// Orders from the storage terminal on the server (spec section 19, O1–O14; the rules in
// rustcraft_rules::orders). With the recipe book in a terminal (its socket, rustcraft_terminal and
// rustcraft_storage::on_button) a player orders an item and a count; the storage's queue
// (rustcraft_rules::orders::queue) plans it once a second on what the storage holds and hands each
// step whose items are set aside to a free machine of its kind on the same line: the autocrafter,
// the electric furnace, the crusher or the alloy smelter (O4, O5, O9, O10). The line takes orders
// only when a node of tier 3 or higher stands on it (M15) and the storage is on one of its item
// nets (M11): «on the line» is every block behind a joint of an item net the storage's anchor is a
// joint of (rustcraft_transport::Transport::around).
//
// A job's items go straight from the storage into the machine's input slots, the way a machine
// writes its own slots (rustcraft_machines::write_slots), and its product comes straight back
// into the storage from the machine's output slots once a second, so an order runs on the default
// joints (the storage takes, M16) and never waits on a net's settings. An autocrafter's job is a
// temporary page (rustcraft_crafter::TempPage) in the level's memory, crafted before its own
// pages; it disappears when its crafts are done. A furnace's, crusher's or smelter's job is done
// when its inputs hold none of the job's items. Then the next round plans the rest of the order:
// a step made of another step's products starts once they are back in the storage (O11 «иначе
// он возвращается на склад»).
//
// What is kept where: the «держать N» counts are saved with the storage (rustcraft_storage's
// record); the queues, the jobs and the temporary pages live in memory only, so a restart of the
// server forgets the orders (what a job had put into a machine stays there). The recipe book is
// built from the game's crafting and smelting recipes (`Recipes::crafting`, `Recipes::cooking`,
// SRC/crates/horizon-client/src/crafting.rs:1337-1345, special recipes left out) and the
// machines' recipes (rustcraft_machines::load_recipes), again after every rescan.
use crate::game::{
    Game,
    rustcraft_content::{self as content, Machine, item_number},
    rustcraft_crafter::{self as crafter, Pattern, TempPage},
    rustcraft_machines::{self as machines, max_stack, slot_of, write_slots},
    rustcraft_server::LevelNet,
    rustcraft_storage as storage,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    machines::slots::{self, Slot},
    net::items::ItemKey,
    orders::{
        KINDS, PlanRecipe, Step,
        queue::{FreeMachine, Job, Order, Queue, SMART_TIER, Wait, runs_that_fit},
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

/// The recipe book (O1): every recipe the orders know, and the items they make as the item
/// table's numbers.
pub(super) struct Book {
    recipes: Vec<PlanRecipe>,
    orderable: Vec<u16>,
}

/// A job on a machine, and the storage it works for.
#[derive(Clone, Debug)]
struct Running {
    storage: u32,
    job: Job,
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

/// `native_level_unload`: the level's jobs and temporary pages are gone with its machines; its
/// queues stay and plan again when it ticks.
pub(super) fn on_level_unload(dimension: &str) {
    orders().jobs.remove(dimension);
}

fn alternatives(i: &horizon_client::crafting::Ingredient) -> Vec<ItemKey> {
    if i.order.is_empty() {
        let mut items: Vec<ItemKey> = i.items.iter().cloned().collect();
        items.sort();
        items
    } else {
        i.order.clone()
    }
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
    let outputs: BTreeSet<&str> = book.iter().map(|r| r.output.0.as_str()).collect();
    let orderable = outputs
        .into_iter()
        .filter_map(|item| item_number(game, item).and_then(|n| u16::try_from(n).ok()))
        .collect();
    Book {
        recipes: book,
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

/// O3: an order for `count` of `item` at the storage `storage`; a refusal when its line takes no
/// orders or its queue is full.
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

fn number(game: &Game<'_>, key: &str) -> Option<u16> {
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
    };
    Some(OrderLine {
        id: o.id,
        item: number(game, &o.item)?,
        count: o.count,
        done: o.done,
        keep: o.keep,
        wait,
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
        _ => None,
    }
}

/// The slots an order's items go into, and those its products come out of.
fn slots_of(machine: Machine) -> (Range<usize>, Range<usize>) {
    match machine {
        Machine::Crafter => (content::CRAFTER_BUFFER, content::CRAFTER_OUTPUTS),
        Machine::Alloy => (content::ALLOY_INPUTS, content::ALLOY_OUTPUTS),
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

/// The stack sizes of a step's inputs.
fn stacks(game: &Game<'_>, inputs: &[(ItemKey, u32)]) -> BTreeMap<ItemKey, u32> {
    inputs
        .iter()
        .map(|(i, _)| (i.clone(), max_stack(game, keys::item_of(i))))
        .collect()
}

/// A job's items out of the storage into the machine's inputs; false, and nothing moved, when the
/// storage no longer holds them or the inputs do not take them.
fn deliver(
    game: &mut Game<'_>,
    dimension: &str,
    storage_id: u32,
    machine: Machine,
    job: &Job,
) -> bool {
    let wants: Vec<(ItemKey, u64)> = job
        .step
        .inputs
        .iter()
        .map(|(i, n)| (i.clone(), u64::from(*n) * u64::from(job.runs)))
        .collect();
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
            return false;
        }
    }
    let sizes = stacks(game, &job.step.inputs);
    let (inputs, _) = slots_of(machine);
    let placed = game.native_with_mod_block_entity(job.machine, |entity| {
        let before: Vec<Slot> = inputs.clone().map(|i| slot_of(entity, i)).collect();
        let mut after = before.clone();
        for (item, want) in &wants {
            let n = u32::try_from(*want).unwrap_or(u32::MAX);
            let size = sizes.get(item).copied().unwrap_or(machines::FALLBACK_STACK);
            if !slots::place(&mut after, item, n, size) {
                return false;
            }
        }
        write_slots(entity, inputs.clone(), &before, &after, &[]);
        true
    });
    if placed != Some(true) {
        give_back(&taken);
        return false;
    }
    true
}

/// The plain stacks of `item` in the machine's output slots go into the storage, as far as it
/// takes them.
fn pull(
    game: &mut Game<'_>,
    dimension: &str,
    storage_id: u32,
    pos: Pos,
    machine: Machine,
    item: &str,
) {
    let (_, outputs) = slots_of(machine);
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return;
    };
    // Only stacks without components: the storage keeps none.
    let plain: u32 = outputs
        .clone()
        .filter_map(|i| entity.items.get(i).cloned().flatten())
        .filter(|s| &*s.item == item && s.extra.is_none() && s.damage == 0)
        .map(|s| s.count)
        .sum();
    if plain == 0 {
        return;
    }
    let took = storage::put_into(game, dimension, storage_id, item, u64::from(plain));
    let took = u32::try_from(took).unwrap_or(plain).min(plain);
    if took == 0 {
        return;
    }
    let written = game.native_with_mod_block_entity(pos, |e| {
        let before: Vec<Slot> = outputs.clone().map(|i| slot_of(e, i)).collect();
        let mut after = before.clone();
        slots::take_from(&mut after, item, took);
        write_slots(e, outputs.clone(), &before, &after, &[]);
    });
    if written.is_none() {
        // The machine went meanwhile: what the storage took is not there.
        storage::take_from(dimension, storage_id, item, u64::from(took));
    }
}

/// O7: what a cancelled job did not use yet goes back from the machine's inputs to the storage:
/// at most `runs` runs of its items.
fn take_back(
    game: &mut Game<'_>,
    dimension: &str,
    storage_id: u32,
    machine: Machine,
    job: &Job,
    runs: u32,
) {
    let (inputs, _) = slots_of(machine);
    let back = game.native_with_mod_block_entity(job.machine, |e| {
        let before: Vec<Slot> = inputs.clone().map(|i| slot_of(e, i)).collect();
        let mut after = before.clone();
        let mut back: Vec<(ItemKey, u32)> = Vec::new();
        for (item, n) in &job.step.inputs {
            let want = n.saturating_mul(runs).min(slots::count_in(&after, item));
            let left = slots::take_from(&mut after, item, want);
            back.push((item.clone(), want - left));
        }
        write_slots(e, inputs.clone(), &before, &after, &[]);
        back
    });
    for (item, n) in back.unwrap_or_default() {
        storage::restore_into(dimension, storage_id, &item, u64::from(n));
    }
}

/// Whether a machine has none of the job's items left in its inputs.
fn inputs_spent(game: &Game<'_>, machine: Machine, job: &Job) -> bool {
    let (inputs, _) = slots_of(machine);
    read_slots(game, job.machine, inputs).is_none_or(|slots| {
        job.step
            .inputs
            .iter()
            .all(|(item, _)| slots::count_in(&slots, item) == 0)
    })
}

// ---- The tick ----

/// Once a second from the level's tick: the jobs looked at, the cancelled ones taken back, and a
/// round of every storage's queue.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &mut LevelNet) {
    let (mut state, mut jobs) = {
        let mut o = orders();
        (
            o.levels.remove(dimension).unwrap_or_default(),
            o.jobs.remove(dimension).unwrap_or_default(),
        )
    };
    let before = state.clone();
    let job_count = jobs.len();
    let had_book = orders().book.is_some();
    follow_jobs(game, dimension, level, &mut state, &mut jobs);
    for (id, anchor, terminals) in storage::level_storages(dimension) {
        let started = storage_round(
            game, dimension, level, &mut state, &jobs, id, anchor, &terminals,
        );
        jobs.extend(started);
    }
    // Storages that are gone take their orders with them.
    let live: BTreeSet<u32> = storage::level_storages(dimension)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    state
        .queues
        .retain(|id, q| live.contains(id) && !q.orders.is_empty());
    state.smart.retain(|id, _| live.contains(id));
    // A book built anew gives the windows its items (O2).
    let book_built = !had_book && orders().book.is_some();
    if state != before || jobs.len() != job_count || book_built {
        state.revision = before.revision + 1;
    }
    let mut o = orders();
    // A press meanwhile counted on too.
    if let Some(meanwhile) = o.levels.remove(dimension) {
        state.revision = state.revision.max(meanwhile.revision + 1);
        for (id, q) in meanwhile.queues {
            merge_presses(state.queues.entry(id).or_default(), q);
        }
    }
    o.levels.insert(dimension.to_owned(), state);
    o.jobs.insert(dimension.to_owned(), jobs);
}

/// Presses cannot reach the queue while the tick has it, but keep anything a hook added.
fn merge_presses(queue: &mut Queue, meanwhile: Queue) {
    for o in meanwhile.orders {
        if !queue.orders.iter().any(|q| q.id == o.id) {
            queue.orders.push(o);
        }
    }
}

/// Each job: its machine's products pulled into the storage; a done job counted to its order; a
/// job whose order was cancelled taken back; a job whose machine went dropped.
fn follow_jobs(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelNet,
    state: &mut LevelOrders,
    jobs: &mut Vec<Running>,
) {
    let mut kept = Vec::with_capacity(jobs.len());
    for r in std::mem::take(jobs) {
        let pos = r.job.machine;
        let machine = level
            .machines
            .get(&pos)
            .copied()
            .filter(|m| kind_of(*m) == Some(r.job.step.machine.as_str()));
        let open = state
            .queues
            .get(&r.storage)
            .is_some_and(|q| q.orders.iter().any(|o| o.id == r.job.order));
        let Some(machine) = machine else {
            level.temp_pages.remove(&pos);
            continue;
        };
        let done = match machine {
            Machine::Crafter => level.temp_pages.get(&pos).map(|t| t.owed == 0),
            _ => Some(inputs_spent(game, machine, &r.job)),
        };
        pull(
            game,
            dimension,
            r.storage,
            pos,
            machine,
            &r.job.step.output.0,
        );
        if !open {
            let left = match machine {
                Machine::Crafter => level.temp_pages.get(&pos).map_or(0, |t| t.owed),
                _ => r.job.runs,
            };
            take_back(game, dimension, r.storage, machine, &r.job, left);
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

/// One storage's round: its line, its «держать» orders and the jobs to start, started. The jobs
/// that went to machines.
#[allow(clippy::too_many_arguments)]
fn storage_round(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelNet,
    state: &mut LevelOrders,
    jobs: &[Running],
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
    let running = |order: u16| jobs.iter().any(|r| r.storage == id && r.job.order == order);
    let queue = state.queues.entry(id).or_default();
    if book_in {
        queue.keep(&keeps, &stock, &running);
    }
    let busy: BTreeSet<Pos> = jobs.iter().map(|r| r.job.machine).collect();
    let mut kinds: Vec<&str> = Vec::new();
    let mut free: Vec<FreeMachine> = Vec::new();
    for &(pos, machine) in &machines_on_line {
        let Some(kind) = kind_of(machine) else {
            continue;
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
        let idle = match machine {
            Machine::Crafter => !level.temp_pages.contains_key(&pos),
            _ => read_slots(game, pos, slots_of(machine).0)
                .is_some_and(|s| s.iter().all(Option::is_none)),
        };
        if idle && !busy.contains(&pos) {
            free.push(FreeMachine {
                pos,
                kind: kind.to_owned(),
            });
        }
    }
    let recipes = &mut level.recipes;
    let machine_at: BTreeMap<Pos, Machine> = machines_on_line.iter().copied().collect();
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
        let sizes = stacks(game, &step.inputs);
        let size = |item: &str| sizes.get(item).copied().unwrap_or(machines::FALLBACK_STACK);
        runs_that_fit(&slots, &step.inputs, want, &size)
    };
    let new_jobs = queue.round(&stock, &book.recipes, &kinds, &free, &running, &mut fits);
    let mut started = Vec::new();
    for job in new_jobs {
        let Some(&machine) = machine_at.get(&job.machine) else {
            continue;
        };
        if !deliver(game, dimension, id, machine, &job) {
            continue;
        }
        if machine == Machine::Crafter {
            level.temp_pages.insert(
                job.machine,
                TempPage {
                    pattern: job.step.pattern(&book.recipes),
                    owed: job.runs,
                },
            );
        }
        started.push(Running { storage: id, job });
    }
    // An order whose jobs all failed to start waits again.
    for o in &mut queue.orders {
        let has_job = started.iter().any(|r| r.job.order == o.id) || running(o.id);
        if o.wait == Wait::Running && !has_job {
            o.wait = Wait::New;
        }
    }
    queue.remove_done();
    started
}
