// The storage on the server (spec 4.5, S1–S8, TM1): cells `rustcraft:cell` that touch are one
// storage; a terminal `rustcraft:terminal` that touches a cell shows it. A storage keeps counts,
// not stacks (a container has at most 256 slots, docs/research's risk table): each item, keyed by
// its id and its damage (rustcraft_rules::storage::keys), has a count, and the slots it fills are
// worked out from them (S2: 27 a cell).
//
// Where the counts live: in a level attachment (`AttachmentType` with `Holder::Level`,
// SRC/docs/native-mods.md:4970-5025, saved with the level in its dimension's data folder), keyed
// by a storage id that every cell's block entity also keeps. So the record belongs to no one cell:
// whichever cell is broken, by a player or an explosion, the storage keeps its items; cells that
// join two storages merge their records; a storage split in two keeps its record in the bigger
// part (rustcraft_rules::storage::cells). Only the last cell of a storage, broken by a player,
// drops what the storage held, as a chest drops its items. The record in one cell's block entity
// would have to move whenever that cell goes, and would be lost with it to an explosion.
//
// The storage is a joint of the item nets it touches (any cell or a terminal next to a housing),
// standing at its first cell, the anchor: it takes what it has room for under its limits (S5) and
// gives everything on a joint set «из блока» (rustcraft_transport). While its cells are no solid
// box (S8) it does neither, and loses nothing; the terminal names the missing cells.
//
// The terminal's window (rustcraft_terminal) draws the list the server sends as a mod message
// (`native_net_send_to_player`, SRC/docs/native-mods.md:4316-4329) to every player who has it
// open (`native_player_container_open`/`_close`, native_hooks.rs:8583-8640), again whenever the
// storage changes. Its presses are menu buttons with the cursor stack (`native_menu_button_v2`):
// take a stack, one item or a stack into the inventory, put the cursor's stack in, change a limit.
// A shift-click from the inventory puts the stack in (`native_menu_quick_move`, API 309). A
// shift-clicked recipe book goes into the terminal's socket instead.
//
// Slice 15: the terminal's recipe book socket (its saved compound), the orders and «держать N»
// (rustcraft_orders): an order press needs the book in the pressed terminal; the N of each item is
// saved with the storage's record, as its limits are, and so are the orders: each storage's queue
// and jobs as rustcraft_rules::orders::saved's strings, handed back to rustcraft_orders on load.
// The list also carries the queue of orders and, with the book, the items it can make
// (rustcraft_rules::storage::wire).
//
// Round 31 (spec 29, FL4–FL6): the cells hold fluids too, in millibuckets beside the items in the
// same record (`fluids`), each cell a share of each fluid (`Tuning::storage_cell_buckets`) that
// the stack module multiplies. The storage is a store of the fluid nets its cells or terminals
// touch (rustcraft_fluids reads and writes its fluids each move), and the tanks on those nets
// count as its stock too: rustcraft_fluids hands their fluids over after each move
// (`set_line_fluids`), not saved. The terminal lists each fluid by its number past the item
// table (rustcraft_rules::orders::FLUID_NUMBERS) in whole buckets, its own and its line's; its
// limit and «держать N» go by the fluid's key, in buckets. A fluid is not taken out by hand.
use crate::game::{
    Game,
    rustcraft_content::{self as content, item_number, item_of_number},
    rustcraft_machines::{max_stack, read_int, write_int},
    rustcraft_orders as orders,
    rustcraft_server::{line, tell},
};
use horizon_client::{
    containers::ModBlockEntity,
    items::ItemStack,
    mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Holder, Host},
    text::Text,
};
use horizon_core::geometry::Direction;
use horizon_client::save;
use horizon_world::nbt::{self, Compound, JavaString, Tag};
use rustcraft_rules::{
    anim::{self, Shown},
    geom::Pos,
    net::{fluids::BUCKET, items::ItemKey, stock::NetStore},
    storage::{
        self, Limit, Shape, Store, cells, keys, modules as sockets,
        terminal::{self as term, Press, Take},
        typed,
        wire::{self, Line, Listing, State},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

/// The mod message channel of the terminal's list.
pub const CHANNEL: &str = "rustcraft:terminal";
/// A cell's saved compound keeps its storage's id under this key.
const ID_KEY: &str = "storage";
/// An order pressed in a terminal without the recipe book (O1).
const NO_BOOK: orders::Refusal = ("rustcraft.orders.no_book", "No recipe book in the terminal");
/// How often, in level ticks, the server sends changed lists to the terminals' viewers.
const SEND_EVERY: u64 = 5;
/// 0.21 (spec 40, SA2): how often, in level ticks, the cells and terminals show their storage's
/// state on their sides.
const SIDES_EVERY: u64 = 20;

/// One storage as the last rescan found it.
struct Found {
    cells: BTreeSet<Pos>,
    shape: Shape,
    anchor: Pos,
}

/// One level's storages.
#[derive(Default)]
struct LevelStorage {
    loaded: bool,
    stores: BTreeMap<u32, Store>,
    next_id: u32,
    found: BTreeMap<u32, Found>,
    cell_of: HashMap<Pos, u32>,
    /// Each terminal and the storage it touches.
    terminals: BTreeMap<Pos, Option<u32>>,
    /// A storage's change count: its viewers get its list again when it moves on.
    versions: HashMap<u32, u64>,
    /// Each storage's orders, queue and jobs, as rustcraft_orders last handed them
    /// (rustcraft_rules::orders::saved's strings).
    orders: BTreeMap<u32, Vec<String>>,
    /// Round 31 (FL5): what the tanks on each storage's fluid nets hold, mB by fluid, as
    /// rustcraft_fluids last handed it over. Not saved: the next move counts it again.
    line_fluids: HashMap<u32, BTreeMap<String, u64>>,
    /// Changed since the attachment was last written.
    dirty: bool,
    ticks: u64,
    /// 0.21 (spec 40, SA2): what each cell and terminal was last told to show, so a block entity
    /// is written only when it changes. Emptied by each rescan: a cell put back starts blank.
    sides: HashMap<Pos, Shown>,
}

impl LevelStorage {
    fn changed(&mut self, id: u32) {
        *self.versions.entry(id).or_insert(0) += 1;
        self.dirty = true;
    }

    fn works(&self, id: u32) -> bool {
        self.found.get(&id).is_some_and(|f| f.shape == Shape::Solid)
    }

    /// The storage whose anchor is `anchor`.
    fn at_anchor(&self, anchor: Pos) -> Option<u32> {
        self.found
            .iter()
            .find(|(_, f)| f.anchor == anchor)
            .map(|(id, _)| *id)
    }
}

/// A player with a terminal open, and what they were last sent.
struct Viewer {
    dimension: String,
    terminal: Pos,
    /// The storage, its change count and the orders' change count last sent.
    sent: Option<(Option<u32>, u64, u64)>,
    /// 0.21.1 (OT12): the stamp of the recipe book's items last sent on their own channel.
    orderable: Option<u32>,
    /// 0.21.1 (OT13): the stamp of the component stacks last sent on their own channel.
    variants: Option<u32>,
}

#[derive(Default)]
struct Storages {
    levels: HashMap<String, LevelStorage>,
    viewers: HashMap<u128, Viewer>,
    /// 0.21.2 (spec 49): numbers typed into terminals' windows, for their level's tick: the
    /// player, the terminal and what it sets.
    typed: Vec<(u128, Pos, typed::Set)>,
}

static STORAGES: LazyLock<Mutex<Storages>> = LazyLock::new(Mutex::default);

fn storages() -> MutexGuard<'static, Storages> {
    STORAGES.lock().unwrap_or_else(PoisonError::into_inner)
}

fn with_level<R>(dimension: &str, f: impl FnOnce(&mut LevelStorage) -> R) -> Option<R> {
    storages().levels.get_mut(dimension).map(f)
}

// ---- The saved record ----

/// Every storage of a level, saved with it.
#[derive(Clone, Debug, Default)]
struct Saved {
    next_id: u32,
    stores: BTreeMap<u32, Store>,
    /// Slice 15's orders of each storage that has any.
    orders: BTreeMap<u32, Vec<String>>,
}

static SAVED: AttachmentType<Saved> = AttachmentType {
    namespace: "rustcraft",
    name: "storages",
    default: Saved::default,
    codec: Some(AttachmentCodec {
        encode: encode_saved,
        decode: decode_saved,
    }),
    copy_on_death: false,
    sync: false,
};

fn string(s: &str) -> Tag {
    Tag::String(JavaString::from(s))
}

fn long(n: u64) -> Tag {
    Tag::Long(i64::try_from(n).unwrap_or(i64::MAX))
}

/// `{next, stores: [{id, items: [{k, n}], limits: [{k, max, destroy}], keep: [{k, n}],
/// fluids: [{k, n}], orders: [string]}]}`; `orders` only for a storage that has some, `fluids`
/// (round 31, mB) only for one that holds any.
fn encode_saved(saved: &Saved) -> Result<Compound, AttachmentError> {
    let stores = saved
        .stores
        .iter()
        .map(|(id, store)| {
            let items = store
                .items
                .iter()
                .map(|(k, &n)| Tag::compound([("k", string(k)), ("n", long(n))]))
                .collect();
            let limits = store
                .limits
                .iter()
                .map(|(k, l)| {
                    Tag::compound([
                        ("k", string(k)),
                        ("max", long(l.max)),
                        ("destroy", Tag::Byte(i8::from(l.destroy_surplus))),
                    ])
                })
                .collect();
            let keep = store
                .keep
                .iter()
                .map(|(k, &n)| Tag::compound([("k", string(k)), ("n", long(n))]))
                .collect();
            let mut entry = vec![
                ("id", Tag::Int(*id as i32)),
                ("items", Tag::List(items)),
                ("limits", Tag::List(limits)),
                ("keep", Tag::List(keep)),
            ];
            if !store.fluids.is_empty() {
                let fluids = store
                    .fluids
                    .iter()
                    .map(|(k, &n)| Tag::compound([("k", string(k)), ("n", long(n))]))
                    .collect();
                entry.push(("fluids", Tag::List(fluids)));
            }
            if let Some(orders) = saved.orders.get(id).filter(|o| !o.is_empty()) {
                entry.push((
                    "orders",
                    Tag::List(orders.iter().map(|s| string(s)).collect()),
                ));
            }
            Tag::compound(entry)
        })
        .collect();
    Ok(Compound::from([
        (JavaString::from("next"), Tag::Int(saved.next_id as i32)),
        (JavaString::from("stores"), Tag::List(stores)),
    ]))
}

fn field<'a>(c: &'a Compound, key: &str) -> Option<&'a Tag> {
    c.get(&JavaString::from(key))
}

fn as_u64(t: Option<&Tag>) -> Option<u64> {
    match t? {
        Tag::Long(n) => u64::try_from(*n).ok(),
        Tag::Int(n) => u64::try_from(*n).ok(),
        _ => None,
    }
}

fn as_string(t: Option<&Tag>) -> Option<String> {
    match t? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn compounds<'a>(t: Option<&'a Tag>) -> impl Iterator<Item = &'a Compound> {
    let list = match t {
        Some(Tag::List(l)) => l.as_slice(),
        _ => &[],
    };
    list.iter().filter_map(|t| match t {
        Tag::Compound(c) => Some(c),
        _ => None,
    })
}

/// What `encode_saved` wrote; an entry it cannot read is left out (and said in the log).
fn decode_saved(data: &Compound) -> Result<Saved, AttachmentError> {
    let mut saved = Saved {
        next_id: as_u64(field(data, "next")).unwrap_or(1) as u32,
        stores: BTreeMap::new(),
        orders: BTreeMap::new(),
    };
    for entry in compounds(field(data, "stores")) {
        let Some(id) = as_u64(field(entry, "id")) else {
            crate::game::rustcraft_debug::journal_warn!("rustcraft: a storage record without an id was left out");
            continue;
        };
        let mut store = Store::default();
        for item in compounds(field(entry, "items")) {
            if let (Some(k), Some(n)) = (as_string(field(item, "k")), as_u64(field(item, "n"))) {
                store.restore(&k, n);
            }
        }
        for limit in compounds(field(entry, "limits")) {
            let (Some(k), Some(max)) = (as_string(field(limit, "k")), as_u64(field(limit, "max")))
            else {
                continue;
            };
            let destroy_surplus = matches!(field(limit, "destroy"), Some(Tag::Byte(1)));
            store.limits.insert(
                k,
                Limit {
                    max,
                    destroy_surplus,
                },
            );
        }
        for keep in compounds(field(entry, "keep")) {
            if let (Some(k), Some(n)) = (as_string(field(keep, "k")), as_u64(field(keep, "n")))
                && n > 0
            {
                store.keep.insert(k, n);
            }
        }
        // Round 31's fluids (absent from a record of v0.19 or older).
        for fluid in compounds(field(entry, "fluids")) {
            if let (Some(k), Some(n)) = (as_string(field(fluid, "k")), as_u64(field(fluid, "n")))
                && n > 0
            {
                store.fluids.insert(k, n);
            }
        }
        // Slice 15's orders (absent from a record of v0.15.0 or older).
        if let Some(Tag::List(list)) = field(entry, "orders") {
            let strings: Vec<String> = list.iter().filter_map(|t| as_string(Some(t))).collect();
            if !strings.is_empty() {
                saved.orders.insert(id as u32, strings);
            }
        }
        saved.stores.insert(id as u32, store);
    }
    Ok(saved)
}

/// The level's record, read once when the server first looks at the level's storages; the
/// orders it keeps go back to rustcraft_orders.
fn load(game: &mut Game<'_>, dimension: &str, level: &mut LevelStorage) {
    if level.loaded {
        return;
    }
    let host: &mut dyn Host = game;
    let saved = SAVED.get(host, Holder::Level).unwrap_or_else(|e| {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: storages not read: {e:?}");
        Saved::default()
    });
    level.stores = saved.stores;
    level.next_id = saved.next_id.max(1);
    level.loaded = true;
    // 0.21 (spec 45, CR2): the old channel items a storage kept become their alloy.
    convert_old_channels(level);
    if !saved.orders.is_empty() {
        orders::restore(dimension, &saved.orders);
    }
    level.orders = saved.orders;
}

/// Slice 15: the orders of the level's storages as rustcraft_orders keeps them now; the record
/// is written again when they changed.
pub(super) fn set_order_records(dimension: &str, records: BTreeMap<u32, Vec<String>>) {
    with_level(dimension, |level| {
        if level.loaded && level.orders != records {
            level.orders = records;
            level.dirty = true;
        }
    });
}

/// Writes the level's record when it changed. Only from the level's own tick, so the level in
/// place is the one the record belongs to.
fn save(game: &mut Game<'_>, dimension: &str) {
    let saved = with_level(dimension, |level| {
        std::mem::replace(&mut level.dirty, false).then(|| Saved {
            next_id: level.next_id,
            stores: level.stores.clone(),
            orders: level.orders.clone(),
        })
    })
    .flatten();
    let Some(saved) = saved else {
        return;
    };
    let host: &mut dyn Host = game;
    if let Err(e) = SAVED.set(host, Holder::Level, saved) {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: storages not saved: {e:?}");
    }
}

// ---- Finding the storages ----

/// Whether `block` is a block of the storage, which the item nets reach.
pub(super) fn is_storage_block(block: &str) -> bool {
    block == content::CELL || block == content::TERMINAL
}

/// Reads the level's cells and terminals again: which cells are one storage, records merged
/// where cells joined storages, each cell's id written where it changed, each terminal's storage.
pub(super) fn rescan(
    game: &mut Game<'_>,
    dimension: &str,
    cell_blocks: &BTreeSet<Pos>,
    terminal_blocks: &BTreeSet<Pos>,
) {
    let mut level = storages().levels.remove(dimension).unwrap_or_default();
    load(game, dimension, &mut level);
    let ids: BTreeMap<Pos, Option<u32>> = cell_blocks
        .iter()
        .map(|&p| {
            let id = crate::game::rustcraft_machines::entity(game, p)
                .and_then(|e| read_int(&e.nbt, ID_KEY))
                .and_then(|id| u32::try_from(id).ok());
            (p, id)
        })
        .collect();
    let groups = cells::group(&ids, &mut level.next_id);
    let mut found = BTreeMap::new();
    let mut cell_of = HashMap::new();
    for g in groups {
        let mut changed = !level.stores.contains_key(&g.id);
        let mut store = level.stores.remove(&g.id).unwrap_or_default();
        for merged in &g.merged {
            if let Some(other) = level.stores.remove(merged) {
                store.absorb(other);
                changed = true;
            }
        }
        store.cells = g.cells.len() as u32;
        let shape = storage::shape(&g.cells);
        let old = level.found.get(&g.id);
        changed |= old.is_none_or(|o| o.cells != g.cells || o.shape != shape);
        level.stores.insert(g.id, store);
        for &c in &g.cells {
            cell_of.insert(c, g.id);
            if ids.get(&c).copied().flatten() != Some(g.id) {
                write_id(game, c, g.id);
            }
        }
        if changed {
            level.changed(g.id);
        }
        found.insert(
            g.id,
            Found {
                anchor: g.anchor(),
                cells: g.cells,
                shape,
            },
        );
    }
    level.terminals = terminal_blocks
        .iter()
        .map(|&t| (t, cells::terminal_storage(t, &cell_of)))
        .collect();
    level.found = found;
    level.cell_of = cell_of;
    level.sides.clear();
    // Wave 2: and the cells sticking out of the box (a protrusion), drawn over the cell.
    let holes: Vec<(Pos, Vec<Pos>, Vec<Pos>)> = level
        .terminals
        .iter()
        .map(|(&t, id)| {
            let shape = id.and_then(|id| level.found.get(&id)).map(|f| &f.shape);
            let take = |h: &BTreeSet<Pos>| h.iter().copied().take(HOLES_SHOWN).collect();
            match shape {
                Some(Shape::Holes(h)) => (t, take(h), Vec::new()),
                Some(Shape::Extra(e)) => (t, Vec::new(), take(e)),
                _ => (t, Vec::new(), Vec::new()),
            }
        })
        .collect();
    let ids: Vec<u32> = level.found.keys().copied().collect();
    storages().levels.insert(dimension.to_owned(), level);
    // S3: each storage's stack from its terminals' stack modules.
    for id in ids {
        refresh_stack_module(game, dimension, id, None);
    }
    // S8: each terminal shows its storage's missing cells in the world.
    for (t, h, e) in holes {
        write_holes(game, t, HOLES_KEY, &h);
        write_holes(game, t, EXTRAS_KEY, &e);
    }
    save(game, dimension);
}

/// S6 (slice 18): after the level's topology is made, each terminal that touches no cell shows
/// the storage on an item net a housing beside it is on (rustcraft_rules::storage::cells::
/// terminal_on_net).
pub(super) fn link_far_terminals(
    game: &Game<'_>,
    dimension: &str,
    topology: &rustcraft_rules::net::topology::Topology,
) {
    let linked = with_level(dimension, |level| {
        let mut blocks: BTreeMap<u32, Vec<Pos>> = BTreeMap::new();
        for (&c, &id) in &level.cell_of {
            blocks.entry(id).or_default().push(c);
        }
        for (&t, id) in &level.terminals {
            if let Some(id) = id {
                blocks.entry(*id).or_default().push(t);
            }
        }
        let far: Vec<Pos> = level
            .terminals
            .iter()
            .filter(|(_, id)| id.is_none())
            .map(|(t, _)| *t)
            .collect();
        let mut linked = Vec::new();
        for t in far {
            let found = cells::terminal_on_net(t, &blocks, topology);
            if let Some(id) = found {
                level.terminals.insert(t, found);
                linked.push(id);
            }
        }
        linked
    })
    .unwrap_or_default();
    // S3: a far terminal's stack module counts too.
    for id in linked {
        refresh_stack_module(game, dimension, id, None);
    }
}

// ---- Slice 18 (S8): the missing cells drawn in the world from the terminal ----

/// A terminal's compound keeps its storage's missing cells, as offsets from it, under this key
/// (three ints a cell); clients draw red glass there.
const HOLES_KEY: &str = "holes";
/// Wave 2: and the cells that stick out of the box, under this key.
const EXTRAS_KEY: &str = "extras";
/// The most missing cells a terminal draws.
const HOLES_SHOWN: usize = 64;

/// The missing (or sticking-out) cells into the terminal at `t` under `key`, and the clients
/// told when they changed.
fn write_holes(game: &mut Game<'_>, t: Pos, key: &str, holes: &[Pos]) {
    let flat: Vec<i32> = holes
        .iter()
        .flat_map(|h| [h[0] - t[0], h[1] - t[1], h[2] - t[2]])
        .collect();
    let changed = game
        .native_with_mod_block_entity(t, |e| {
            let key = JavaString::from(key);
            let old = match e.nbt.get(&key) {
                Some(Tag::IntArray(v)) => v.clone(),
                _ => Vec::new(),
            };
            if old == flat {
                return false;
            }
            if flat.is_empty() {
                e.nbt.remove(&key);
            } else {
                e.nbt.insert(key, Tag::IntArray(flat.clone()));
            }
            true
        })
        .unwrap_or(false);
    if changed {
        game.native_block_entity_sync(t);
    }
}

fn holes_of(entity: &ModBlockEntity) -> Vec<[i32; 3]> {
    marks_of(entity, HOLES_KEY)
}

fn marks_of(entity: &ModBlockEntity, key: &str) -> Vec<[i32; 3]> {
    match entity.nbt.get(&JavaString::from(key)) {
        Some(Tag::IntArray(v)) => v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
        _ => Vec::new(),
    }
}

/// `native_block_entity_update_tag`: clients get a terminal's missing cells.
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id != content::TERMINAL {
        return;
    }
    for key in [HOLES_KEY, EXTRAS_KEY] {
        if let Some(v) = entity.nbt.get(&JavaString::from(key)) {
            tag.get_or_insert_with(Compound::new)
                .insert(JavaString::from(key), v.clone());
        }
    }
}

/// On a client: red glass in each missing cell of the terminal's storage, a little smaller than
/// a block, where the world has air.
pub(super) fn draw_holes(
    game: &Game<'_>,
    pos: Pos,
    entity: &ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    const SCALE: f32 = 0.9;
    for h in holes_of(entity) {
        let at = [pos[0] + h[0], pos[1] + h[1], pos[2] + h[2]];
        if !matches!(game.native_block(at).as_str(), "minecraft:air" | "minecraft:cave_air") {
            continue;
        }
        draw.push();
        let k = (1.0 - SCALE) / 2.0;
        draw.translate([h[0] as f32 + k, h[1] as f32 + k, h[2] as f32 + k]);
        draw.scale([SCALE; 3]);
        game.native_draw_block(draw, "minecraft:red_stained_glass", &[]);
        draw.pop();
    }
    // Wave 2: red glass a little bigger than a block over each cell that sticks out.
    const OVER: f32 = 1.04;
    for h in marks_of(entity, EXTRAS_KEY) {
        draw.push();
        let k = (1.0 - OVER) / 2.0;
        draw.translate([h[0] as f32 + k, h[1] as f32 + k, h[2] as f32 + k]);
        draw.scale([OVER; 3]);
        game.native_draw_block(draw, "minecraft:red_stained_glass", &[]);
        draw.pop();
    }
}

/// The terminal's culling box takes in its missing cells.
pub(super) fn hole_bounds(entity: &ModBlockEntity, bounds: &mut [f32; 6]) {
    for h in holes_of(entity).into_iter().chain(marks_of(entity, EXTRAS_KEY)) {
        for k in 0..3 {
            bounds[k] = bounds[k].min(h[k] as f32);
            bounds[k + 3] = bounds[k + 3].max(h[k] as f32 + 1.0);
        }
    }
}

fn write_id(game: &mut Game<'_>, cell: Pos, id: u32) {
    let written = game.native_with_mod_block_entity(cell, |entity| {
        write_int(&mut entity.nbt, ID_KEY, id as i32);
    });
    if written.is_none() {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: the cell at {cell:?} has no block entity");
    }
}

/// The block that stands for the storage `pos` belongs to on an item net: a cell's storage, or
/// the storage a terminal touches.
pub(super) fn anchor_of(dimension: &str, pos: Pos) -> Option<Pos> {
    with_level(dimension, |level| {
        let id = level
            .cell_of
            .get(&pos)
            .copied()
            .or_else(|| level.terminals.get(&pos).copied().flatten())?;
        level.found.get(&id).map(|f| f.anchor)
    })
    .flatten()
}

/// The storage at `anchor` as the net plans on it this move.
pub(super) fn net_store(dimension: &str, anchor: Pos) -> Option<NetStore> {
    with_level(dimension, |level| {
        let id = level.at_anchor(anchor)?;
        Some(NetStore {
            store: level.stores.get(&id)?.clone(),
            works: level.works(id),
        })
    })
    .flatten()
}

/// Slice 15: every storage of the level that works as one: its id, its anchor and its
/// terminals.
pub(super) fn level_storages(dimension: &str) -> Vec<(u32, Pos, Vec<Pos>)> {
    with_level(dimension, |level| {
        level
            .found
            .iter()
            .map(|(&id, f)| {
                let terminals = level
                    .terminals
                    .iter()
                    .filter(|(_, s)| **s == Some(id))
                    .map(|(&t, _)| t)
                    .collect();
                (id, f.anchor, terminals)
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Slice 15: what the storage `id` holds and its «держать N» counts; none while it does not work
/// (S8).
pub(super) fn stock(
    dimension: &str,
    id: u32,
) -> Option<(BTreeMap<ItemKey, u64>, BTreeMap<ItemKey, u64>)> {
    with_level(dimension, |level| {
        let store = level.stores.get(&id).filter(|_| level.works(id))?;
        Some((store.items.clone(), store.keep.clone()))
    })
    .flatten()
}

// ---- Round 31: the fluids of the storage (FL4–FL6) ----

/// The storage standing at `anchor` on a fluid net: its id and its fluid part
/// (`Store::fluid_part`); none while it does not work (S8).
pub(super) fn fluid_store(dimension: &str, anchor: Pos) -> Option<(u32, Store)> {
    with_level(dimension, |level| {
        let id = level.at_anchor(anchor).filter(|&id| level.works(id))?;
        Some((id, level.stores.get(&id)?.fluid_part()))
    })
    .flatten()
}

/// What a fluid move did to the storage `id`: the change from `before` to `after` (both fluid
/// parts) laid onto its record, so nothing else that changed it meanwhile is lost.
pub(super) fn apply_fluids(dimension: &str, id: u32, before: &Store, after: &Store) {
    if before.fluids == after.fluids {
        return;
    }
    with_level(dimension, |level| {
        let Some(store) = level.stores.get_mut(&id) else {
            return;
        };
        let names: BTreeSet<&String> = before.fluids.keys().chain(after.fluids.keys()).collect();
        for fluid in names {
            let (was, now) = (before.fluid(fluid), after.fluid(fluid));
            if now > was {
                *store.fluids.entry(fluid.clone()).or_insert(0) += now - was;
            } else if was > now {
                store.take_fluid(fluid, was - now);
            }
        }
        level.changed(id);
    });
}

/// The mB of each fluid the cells of the storage `id` hold (none while it does not work).
pub(super) fn own_fluids(dimension: &str, id: u32) -> BTreeMap<String, u64> {
    with_level(dimension, |level| {
        level
            .works(id)
            .then(|| level.stores.get(&id).map(|s| s.fluids.clone()))
            .flatten()
    })
    .flatten()
    .unwrap_or_default()
}

/// Takes up to `amount` mB of `fluid` out of the cells of the storage `id`: how much.
pub(super) fn take_fluid(dimension: &str, id: u32, fluid: &str, amount: u64) -> u64 {
    with_level(dimension, |level| {
        if !level.works(id) {
            return 0;
        }
        let took = level.stores.get_mut(&id).map_or(0, |s| s.take_fluid(fluid, amount));
        if took > 0 {
            level.changed(id);
        }
        took
    })
    .unwrap_or(0)
}

/// Puts up to `amount` mB of `fluid` into the cells of the storage `id`: how much it took.
pub(super) fn put_fluid(dimension: &str, id: u32, fluid: &str, amount: u64, cell_mb: u64) -> u64 {
    with_level(dimension, |level| {
        if !level.works(id) {
            return 0;
        }
        let took = level
            .stores
            .get_mut(&id)
            .map_or(0, |s| s.put_fluid(fluid, amount, cell_mb));
        if took > 0 {
            level.changed(id);
        }
        took
    })
    .unwrap_or(0)
}

/// The self-test: the storage the cell or terminal at `pos` belongs to holds `amount` mB of
/// `fluid` in its cells, whatever its room.
pub(super) fn set_fluid_at(dimension: &str, pos: Pos, fluid: &str, amount: u64) -> bool {
    let Some(id) = storage_id(dimension, pos) else {
        return false;
    };
    with_level(dimension, |level| {
        let Some(store) = level.stores.get_mut(&id) else {
            return false;
        };
        if amount == 0 {
            store.fluids.remove(fluid);
        } else {
            store.fluids.insert(fluid.to_owned(), amount);
        }
        level.changed(id);
        true
    })
    .unwrap_or(false)
}

/// FL5: what the tanks on the fluid nets of the storage at `anchor` hold now; its terminals'
/// viewers get the list again when it changed.
pub(super) fn set_line_fluids(dimension: &str, anchor: Pos, fluids: BTreeMap<String, u64>) {
    with_level(dimension, |level| {
        let Some(id) = level.at_anchor(anchor) else {
            return;
        };
        if level.line_fluids.get(&id) != Some(&fluids) {
            level.line_fluids.insert(id, fluids);
            *level.versions.entry(id).or_insert(0) += 1;
        }
    });
}

/// FL5: the line's tanks of every storage not at one of `anchors` (no longer on a fluid net)
/// are forgotten.
pub(super) fn clear_line_fluids_but(dimension: &str, anchors: &BTreeSet<Pos>) {
    with_level(dimension, |level| {
        let gone: Vec<u32> = level
            .line_fluids
            .keys()
            .copied()
            .filter(|id| level.found.get(id).is_none_or(|f| !anchors.contains(&f.anchor)))
            .collect();
        for id in gone {
            level.line_fluids.remove(&id);
            *level.versions.entry(id).or_insert(0) += 1;
        }
    });
}

// ---- Items in and out ----

/// The stack a key and a count make: a component key's saved stack with every component back.
fn stack_of(key: &str, count: u32) -> ItemStack {
    if let Some(stack) = keys::payload(key).and_then(|p| saved_stack(p, count)) {
        return stack;
    }
    let (item, damage) = keys::parse(key);
    let mut stack = ItemStack::new(item, count);
    stack.damage = damage;
    stack
}

/// Slice 18: the stack a component key's payload saved, `count` of it.
fn saved_stack(payload: &str, count: u32) -> Option<ItemStack> {
    let bytes = keys::unhex(payload)?;
    let (tag, _) = nbt::read_network(&bytes, nbt::Limits::saved()).ok()?;
    let mut stack = save::item_from_nbt(&tag)?;
    stack.count = count;
    Some(stack)
}

/// The key a stack goes under on the nets and in the storage: its id and damage, or, with other
/// components or worn further than a press names, its id and its whole saved form (one item).
pub(super) fn net_key(stack: &ItemStack) -> ItemKey {
    // An all-default component box is no component (`ItemStack::tidy`).
    if stack.extra.as_ref().is_none_or(|d| d.is_empty()) && term::addressable(stack.damage) {
        return keys::key(&stack.item, stack.damage);
    }
    let mut one = stack.clone();
    one.count = 1;
    let payload = nbt::write_network(&save::item_to_nbt(&one), nbt::Limits::saved())
        .map(|b| keys::hex(&b))
        .unwrap_or_default();
    keys::component_key(&stack.item, &payload)
}

/// The key a stack goes under; none for a stack the storage cannot keep (its saved form too long).
fn key_of(stack: &ItemStack) -> Option<ItemKey> {
    if stack.count == 0 {
        return None;
    }
    Some(net_key(stack)).filter(|k| keys::storable(k))
}

/// The storage behind `anchor`: what it takes of `key` now.
fn room_at(game: &Game<'_>, dimension: &str, anchor: Pos, key: &str) -> u64 {
    let stack = |k: &str| max_stack(game, keys::item_of(k));
    with_level(dimension, |level| {
        let id = level.at_anchor(anchor)?;
        level
            .works(id)
            .then(|| level.stores.get(&id).map(|s| s.room_for(key, &stack)))
            .flatten()
    })
    .flatten()
    .unwrap_or(0)
}

/// Slice 17: what the storage `id` takes of `key` now (none while it does not work), so an
/// order's products set off only as far as they will go in when they arrive.
pub(super) fn room_in(game: &Game<'_>, dimension: &str, id: u32, key: &str) -> u64 {
    let stack = |k: &str| max_stack(game, keys::item_of(k));
    with_level(dimension, |level| {
        level
            .works(id)
            .then(|| level.stores.get(&id).map(|s| s.room_for(key, &stack)))
            .flatten()
    })
    .flatten()
    .unwrap_or(0)
}

/// Puts `count` of `key` into the storage `id`; how many it took (destroyed surplus counts).
pub(super) fn put_into(game: &Game<'_>, dimension: &str, id: u32, key: &str, count: u64) -> u64 {
    let stack = |k: &str| max_stack(game, keys::item_of(k));
    with_level(dimension, |level| {
        if !level.works(id) {
            return 0;
        }
        let took = level
            .stores
            .get_mut(&id)
            .map_or(0, |s| s.put(key, count, &stack));
        if took > 0 {
            level.changed(id);
        }
        took
    })
    .unwrap_or(0)
}

pub(super) fn take_from(dimension: &str, id: u32, key: &str, count: u64) -> u64 {
    with_level(dimension, |level| {
        let took = level.stores.get_mut(&id).map_or(0, |s| s.take(key, count));
        if took > 0 {
            level.changed(id);
        }
        took
    })
    .unwrap_or(0)
}

pub(super) fn restore_into(dimension: &str, id: u32, key: &str, count: u64) {
    with_level(dimension, |level| {
        if let Some(s) = level.stores.get_mut(&id) {
            s.restore(key, count);
            level.changed(id);
        }
    });
}

fn id_at(dimension: &str, anchor: Pos) -> Option<u32> {
    with_level(dimension, |level| level.at_anchor(anchor)).flatten()
}

/// The net's move of `count` of `key` from the container at `from` into the storage at `anchor`:
/// the stacks of that key come out of the container's slots through `side` as far as the storage
/// has room, so nothing comes out that cannot go in. How many moved.
pub(super) fn into_storage(
    game: &mut Game<'_>,
    dimension: &str,
    from: Pos,
    side: Direction,
    anchor: Pos,
    key: &str,
    count: u32,
) -> u32 {
    let Some(id) = id_at(dimension, anchor) else {
        return 0;
    };
    let mut left = u64::from(count).min(room_at(game, dimension, anchor, key));
    let Some(slots) = game.native_container_slots(dimension, from, side) else {
        return 0;
    };
    let mut moved = 0;
    for slot in slots {
        if left == 0 {
            break;
        }
        let matches = slot
            .stack
            .as_ref()
            .is_some_and(|s| slot.can_take && key_of(s).as_deref() == Some(key));
        if !matches {
            continue;
        }
        let want = u32::try_from(left).unwrap_or(u32::MAX);
        let Some(got) = game.native_extract_item(dimension, from, side, slot.slot, want) else {
            continue;
        };
        let took = put_into(game, dimension, id, key, u64::from(got.count));
        if took < u64::from(got.count) {
            // Cannot happen (the room was counted first); kept, never lost.
            restore_into(dimension, id, key, u64::from(got.count) - took);
        }
        left -= u64::from(got.count).min(left);
        moved += got.count;
    }
    if moved > 0 {
        // rustcraft_debug: items into the storage from a block on the net, summed a second.
        let route = format!(
            "{}({}) -> storage#{id} net",
            game.native_block(from),
            rustcraft_rules::debug::pos_text(from)
        );
        crate::game::rustcraft_debug::flow(dimension, &route, key, u64::from(moved));
    }
    moved
}

/// The net's move of `count` of `key` out of the storage at `anchor` into the block at `to`,
/// through its side `side` (`native_insert_item`, which asks a machine's slot rules); what the
/// block does not take stays in the storage.
pub(super) fn out_of_storage(
    game: &mut Game<'_>,
    dimension: &str,
    anchor: Pos,
    to: Pos,
    side: Direction,
    key: &str,
    count: u32,
) -> u32 {
    let Some(id) = id_at(dimension, anchor) else {
        return 0;
    };
    let took = take_from(dimension, id, key, u64::from(count));
    if took == 0 {
        return 0;
    }
    let took = took as u32;
    let rest = game
        .native_insert_item(to, side, stack_of(key, took))
        .map_or(0, |r| r.count);
    restore_into(dimension, id, key, u64::from(rest));
    if took > rest {
        // rustcraft_debug: the net's own routing out of the storage (no order), summed a second.
        let route = format!(
            "storage#{id} -> {}({}) net",
            game.native_block(to),
            rustcraft_rules::debug::pos_text(to)
        );
        crate::game::rustcraft_debug::flow(dimension, &route, key, u64::from(took - rest));
    }
    took - rest
}

/// The net's move between two storages.
pub(super) fn between_storages(
    game: &Game<'_>,
    dimension: &str,
    from: Pos,
    to: Pos,
    key: &str,
    count: u32,
) -> u32 {
    let (Some(a), Some(b)) = (id_at(dimension, from), id_at(dimension, to)) else {
        return 0;
    };
    let room = room_at(game, dimension, to, key).min(u64::from(count));
    let took = take_from(dimension, a, key, room);
    let put = put_into(game, dimension, b, key, took);
    restore_into(dimension, a, key, took - put);
    if put > 0 {
        let route = format!("storage#{a} -> storage#{b} net");
        crate::game::rustcraft_debug::flow(dimension, &route, key, put);
    }
    put as u32
}

/// Ticks between two looks for old channel items in the level's storages: what a chest on a
/// joint sends in becomes alloy within a second.
const OLD_CHANNELS_EVERY: u64 = 20;

/// 0.21 (spec 45, CR2): every storage of the level with its old channel items turned into their
/// alloy (rustcraft_rules::net::old_channels::convert_store).
fn convert_old_channels(level: &mut LevelStorage) {
    let changed: Vec<u32> = level
        .stores
        .iter_mut()
        .filter_map(|(id, store)| (rustcraft_rules::net::old_channels::convert_store(store) > 0).then_some(*id))
        .collect();
    for id in changed {
        level.changed(id);
    }
}

// ---- The level's tick: saving and the viewers' lists ----

/// `native_level_tick_post`: every OLD_CHANNELS_EVERY ticks the old channel items turned into
/// alloy, the record written when it changed, and every SEND_EVERY ticks the changed lists sent
/// to the players who have a terminal of this level open.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    let (send, sides) = with_level(dimension, |level| {
        level.ticks += 1;
        if level.loaded && level.ticks.is_multiple_of(OLD_CHANNELS_EVERY) {
            convert_old_channels(level);
        }
        (level.ticks.is_multiple_of(SEND_EVERY), level.ticks.is_multiple_of(SIDES_EVERY))
    })
    .unwrap_or((false, false));
    save(game, dimension);
    apply_typed(game, dimension);
    if sides {
        show_sides(game, dimension);
    }
    if !send {
        return;
    }
    send_orderable_to_viewers(game, dimension);
    let orders_revision = orders::revision(dimension);
    let due: Vec<(u128, Pos, Option<u32>, u64)> = {
        let s = storages();
        let Some(level) = s.levels.get(dimension) else {
            return;
        };
        s.viewers
            .iter()
            .filter(|(_, v)| v.dimension == dimension)
            .filter_map(|(uuid, v)| {
                let id = level.terminals.get(&v.terminal).copied().flatten();
                let version = id
                    .and_then(|id| level.versions.get(&id).copied())
                    .unwrap_or(0);
                (v.sent != Some((id, version, orders_revision)))
                    .then_some((*uuid, v.terminal, id, version))
            })
            .collect()
    };
    for (uuid, terminal, id, version) in due {
        let mut list = listing(game, dimension, terminal, id);
        // OT13: the component stacks apart, before the list that names their stamp.
        let sent = storages().viewers.get(&uuid).and_then(|v| v.variants);
        let now = send_variants(game, uuid, storage::variants::CHANNEL, &mut list, sent);
        if now != sent
            && let Some(v) = storages().viewers.get_mut(&uuid)
        {
            v.variants = now;
        }
        let bytes = wire::encode(&list);
        if let Err(e) = game.native_net_send_to_player(uuid, CHANNEL, &bytes) {
            log::debug!("rustcraft: the terminal's list not sent to {uuid:032x}: {e}");
            continue;
        }
        if let Some(v) = storages().viewers.get_mut(&uuid) {
            v.sent = Some((id, version, orders_revision));
        }
    }
}

/// 0.21.1 (spec 47, OT12): the items the recipe book can make, on their own channel
/// (rustcraft_rules::storage::orderable), to a player whose window was last sent another list of
/// them (`sent`, a stamp): once when a window opens, again when the book is built anew. The new
/// stamp when they went (or nothing was due), `sent` when they did not.
pub(super) fn send_orderable(game: &mut Game<'_>, uuid: u128, sent: Option<u32>) -> Option<u32> {
    let Some((stamp, numbers)) = orders::orderable_list() else {
        return sent;
    };
    if sent == Some(stamp) {
        return sent;
    }
    for bytes in storage::orderable::encode(&numbers) {
        if let Err(e) = game.native_net_send_to_player(uuid, storage::orderable::CHANNEL, &bytes) {
            log::debug!("rustcraft: the recipe book's items not sent to {uuid:032x}: {e}");
            return sent;
        }
    }
    Some(stamp)
}

/// The terminals' viewers of this level get the recipe book's items when theirs are not the
/// book's now; they go before the list in the same tick.
fn send_orderable_to_viewers(game: &mut Game<'_>, dimension: &str) {
    let due: Vec<(u128, Option<u32>)> = storages()
        .viewers
        .iter()
        .filter(|(_, v)| v.dimension == dimension)
        .map(|(uuid, v)| (*uuid, v.orderable))
        .collect();
    for (uuid, sent) in due {
        let now = send_orderable(game, uuid, sent);
        if now != sent
            && let Some(v) = storages().viewers.get_mut(&uuid)
        {
            v.orderable = now;
        }
    }
}

/// The self-test (OT12): the messages a player who opens the terminal at `pos` gets: its list
/// and the recipe book's items (none before the book is built).
pub(super) fn terminal_wire(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
) -> Option<rustcraft_rules::selftest::TerminalWire> {
    let id = with_level(dimension, |level| level.terminals.get(&pos).copied()).flatten()?;
    let mut list = listing(game, dimension, pos, id);
    let variants = match stamp_variants(&mut list) {
        Some(_) => storage::variants::encode(&list.variants),
        None => Vec::new(),
    };
    let orderable = orders::orderable_list()
        .map(|(_, numbers)| storage::orderable::encode(&numbers))
        .unwrap_or_default();
    Some(rustcraft_rules::selftest::TerminalWire {
        list: wire::encode(&list),
        orderable,
        variants,
    })
}

/// 0.21.1 (OT13): the list names the stamp of its component stacks; none without any.
fn stamp_variants(list: &mut Listing) -> Option<u32> {
    list.variants_stamp = (!list.variants.is_empty()).then(|| storage::variants::stamp(&list.variants));
    list.variants_stamp
}

/// 0.21.1 (spec 47, OT13): a window's component stacks on `channel` in parts
/// (rustcraft_rules::storage::variants), when the player was last sent another list of them
/// (`sent`, a stamp); `list` then names their stamp. The stamp the player has now.
pub(super) fn send_variants(
    game: &mut Game<'_>,
    uuid: u128,
    channel: &str,
    list: &mut Listing,
    sent: Option<u32>,
) -> Option<u32> {
    let Some(stamp) = stamp_variants(list) else {
        return sent;
    };
    if sent == Some(stamp) {
        return sent;
    }
    for bytes in storage::variants::encode(&list.variants) {
        if let Err(e) = game.native_net_send_to_player(uuid, channel, &bytes) {
            log::debug!("rustcraft: the storage's component stacks not sent to {uuid:032x}: {e}");
            // The list names no stamp: the window keeps to the stacks it carries.
            list.variants_stamp = None;
            return sent;
        }
    }
    Some(stamp)
}

/// The self-test (OT12): `count` of `stack` straight into the record of the storage the cell or
/// terminal at `pos` belongs to, past its room.
pub(super) fn stock_at(dimension: &str, pos: Pos, stack: &ItemStack, count: u64) -> bool {
    let Some(key) = key_of(stack) else {
        return false;
    };
    let Some(id) = storage_id(dimension, pos) else {
        return false;
    };
    with_level(dimension, |level| {
        let Some(store) = level.stores.get_mut(&id) else {
            return false;
        };
        *store.items.entry(key).or_insert(0) += count;
        level.changed(id);
        true
    })
    .unwrap_or(false)
}

/// 0.21 (spec 40, SA2): each cell shows its storage's fill and, with the terminals, whether it
/// works (rustcraft_rules::anim::storage_shown); clients draw them on the blocks' sides
/// (rustcraft_anim). Only the blocks whose state changed are written.
fn show_sides(game: &mut Game<'_>, dimension: &str) {
    let stack = |k: &str| max_stack(game, keys::item_of(k));
    let due: Vec<(Pos, Shown)> = with_level(dimension, |level| {
        let mut all = Vec::new();
        for (id, f) in &level.found {
            let shown = level.stores.get(id).map_or_else(Shown::default, |s| {
                anim::storage_shown(level.works(*id), s.used_slots(&stack), s.total_slots())
            });
            all.extend(f.cells.iter().map(|&c| (c, shown)));
            let terminal = Shown { level: 0, ..shown };
            all.extend(
                level
                    .terminals
                    .iter()
                    .filter(|(_, s)| **s == Some(*id))
                    .map(|(&t, _)| (t, terminal)),
            );
        }
        // A terminal that touches no cell stands.
        all.extend(
            level
                .terminals
                .iter()
                .filter(|(_, s)| s.is_none())
                .map(|(&t, _)| (t, Shown::default())),
        );
        all.retain(|(p, s)| level.sides.get(p) != Some(s));
        for (p, s) in &all {
            level.sides.insert(*p, *s);
        }
        all
    })
    .unwrap_or_default();
    for (pos, shown) in due {
        crate::game::rustcraft_anim::show(game, pos, shown);
    }
}

/// What the terminal at `terminal` shows of the storage `id`.
fn listing(game: &Game<'_>, dimension: &str, terminal: Pos, id: Option<u32>) -> Listing {
    let stack = |k: &str| max_stack(game, keys::item_of(k));
    let shown = with_level(dimension, |level| {
        let id = id?;
        let store = level.stores.get(&id)?.clone();
        let found = level.found.get(&id)?;
        let (holes, extra) = match &found.shape {
            Shape::Solid => (Vec::new(), false),
            Shape::Holes(h) => (h.iter().copied().collect(), false),
            Shape::Extra(e) => (e.iter().copied().collect(), true),
        };
        Some((store, holes, extra))
    })
    .flatten();
    let Some((store, holes, extra)) = shown else {
        return Listing {
            terminal,
            state: State::NoCells,
            ..Listing::default()
        };
    };
    let mut lines: Vec<(u64, Line)> = Vec::new();
    let mut variants: Vec<wire::Variant> = Vec::new();
    let mut keys: BTreeSet<&ItemKey> = store.items.keys().collect();
    keys.extend(store.limits.keys());
    keys.extend(store.keep.keys());
    for key in keys {
        let (item, damage) = keys::parse(key);
        let Some(number) = item_number(game, item).and_then(|n| u16::try_from(n).ok()) else {
            continue;
        };
        // Slice 18: a component stack goes by its place among its item's, with its saved form.
        let damage = match keys::payload(key) {
            Some(payload) => {
                let Some(index) = store.variant_index(key).filter(|i| u32::from(*i) <= term::MAX_DAMAGE) else {
                    continue;
                };
                variants.push(wire::Variant {
                    item: number,
                    index,
                    payload: payload.to_owned(),
                });
                term::VARIANT | index
            }
            None => {
                let Ok(damage) = u16::try_from(damage) else {
                    continue;
                };
                damage
            }
        };
        let count = store.count(key);
        lines.push((
            count,
            Line {
                item: number,
                damage,
                count,
                limit: store.limits.get(key).copied(),
                keep: store.keep.get(key).copied().unwrap_or(0),
            },
        ));
    }
    // The most numerous first: a message too small for them all keeps those. Round 31: the
    // fluids before every item, so none of them is cut off.
    lines.sort_by(|a, b| b.0.cmp(&a.0));
    let line_fluids = id
        .and_then(|id| with_level(dimension, |level| level.line_fluids.get(&id).cloned()).flatten())
        .unwrap_or_default();
    let mut all: Vec<(u64, Line)> = fluid_lines(&store, &line_fluids)
        .into_iter()
        .map(|l| (l.count, l))
        .collect();
    all.append(&mut lines);
    let lines = all;
    let view = id.map(|id| orders::view(game, dimension, terminal, id));
    Listing {
        terminal,
        state: if holes.is_empty() {
            State::Works
        } else if extra {
            State::Extra
        } else {
            State::Holes
        },
        cells: store.cells,
        used: store.used_slots(&stack),
        total: store.total_slots(),
        hole_count: holes.len() as u32,
        holes: holes.into_iter().take(wire::MAX_HOLES).collect(),
        lines: lines.into_iter().map(|(_, l)| l).collect(),
        book: view.as_ref().is_some_and(|v| v.book),
        smart: view.as_ref().map_or_else(Default::default, |v| v.smart),
        orders: view.as_ref().map_or_else(Vec::new, |v| v.orders.clone()),
        orderable: view.map_or_else(Vec::new, |v| v.orderable),
        // The message says when it cuts the list (OT10).
        orderable_cut: false,
        variants,
        sockets: socket_numbers(game, terminal),
        own: own_items(game, terminal)
            .iter()
            .filter_map(|i| item_number(game, i).and_then(|n| u16::try_from(n).ok()))
            .collect(),
        // Set when the stacks go apart (OT13).
        variants_stamp: None,
    }
}

/// Round 31 (FL5): a line for each fluid the storage knows: its cells' and its line's tanks'
/// whole buckets, its limit and its «держать N»; a fluid the terminal has no number for is left
/// out.
fn fluid_lines(store: &Store, line: &BTreeMap<String, u64>) -> Vec<Line> {
    use rustcraft_rules::orders::{fluid_key, fluid_number, fluid_of};
    let mut fluids: BTreeSet<String> = store.fluids.keys().chain(line.keys()).cloned().collect();
    for key in store.limits.keys().chain(store.keep.keys()) {
        if let Some(f) = fluid_of(key) {
            fluids.insert(f.to_owned());
        }
    }
    fluids
        .into_iter()
        .filter_map(|fluid| {
            let item = fluid_number(&fluid)?;
            let key = fluid_key(&fluid);
            let mb = store.fluid(&fluid) + line.get(&fluid).copied().unwrap_or(0);
            let limit = store.limits.get(&key).copied();
            let keep = store.keep.get(&key).copied().unwrap_or(0);
            (mb >= BUCKET || limit.is_some() || keep > 0).then_some(Line {
                item,
                damage: 0,
                count: mb / BUCKET,
                limit,
                keep,
            })
        })
        .collect()
}

/// Slice 18: the items in the terminal's sockets, by number.
fn socket_numbers(game: &Game<'_>, terminal: Pos) -> Vec<u16> {
    let entity = crate::game::rustcraft_machines::entity(game, terminal);
    (0..sockets::SOCKETS)
        .map(|k| {
            entity
                .as_ref()
                .and_then(|e| socket_stack(&e.nbt, k))
                .and_then(|s| item_number(game, &s.item))
                .and_then(|n| u16::try_from(n).ok())
                .unwrap_or(wire::NO_ITEM)
        })
        .collect()
}

/// `native_player_container_open`: a player who opens a terminal is sent its list next.
pub(super) fn on_container_open(player: u128, dimension: &str, menu: &str, pos: Option<Pos>) {
    let Some(terminal) = pos.filter(|_| menu == content::TERMINAL) else {
        return;
    };
    storages().viewers.insert(
        player,
        Viewer {
            dimension: dimension.to_owned(),
            terminal,
            sent: None,
            orderable: None,
            variants: None,
        },
    );
}

/// `native_player_container_close`.
pub(super) fn on_container_close(player: u128, menu: &str) {
    if menu == content::TERMINAL {
        storages().viewers.remove(&player);
    }
}

// ---- The terminal's presses ----

/// 0.21.2 (spec 49): `native_net_from_client`: a number typed into a terminal's window, for the
/// next tick of the player's level.
pub(super) fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel != typed::CHANNEL {
        return;
    }
    match typed::decode_terminal(data) {
        Some((terminal, set)) => storages().typed.push((player, terminal, set)),
        None => log::debug!("rustcraft: a typed number from {player:032x} that does not read"),
    }
}

/// Single player: the client's typed number straight into the queue, the local player being the
/// server's own.
pub(super) fn queue_typed(player: u128, terminal: Pos, set: typed::Set) {
    storages().typed.push((player, terminal, set));
}

/// The typed numbers of this level's players whose window is open on the terminal they name; the
/// others are dropped.
fn apply_typed(game: &mut Game<'_>, dimension: &str) {
    let due: Vec<(u128, Pos, typed::Set)> = {
        let mut s = storages();
        if s.typed.is_empty() {
            return;
        }
        let all = std::mem::take(&mut s.typed);
        let (here, rest): (Vec<_>, Vec<_>) = all.into_iter().partition(|(player, _, _)| {
            s.viewers.get(player).is_some_and(|v| v.dimension == dimension)
        });
        // A player of another level waits for that level's tick; one with no window is dropped.
        let waiting: Vec<_> = rest.into_iter().filter(|(p, _, _)| s.viewers.contains_key(p)).collect();
        s.typed = waiting;
        here.into_iter()
            .filter(|(player, terminal, _)| s.viewers.get(player).is_some_and(|v| v.terminal == *terminal))
            .collect()
    };
    for (player, terminal, set) in due {
        typed_at_terminal(game, dimension, terminal, set, Some(player));
    }
}

/// 0.21.2 (spec 49): a typed limit or N on the storage of the terminal at `terminal`; `player` is
/// told why one is refused. Whether it was set.
pub(super) fn typed_at_terminal(
    game: &mut Game<'_>,
    dimension: &str,
    terminal: Pos,
    set: typed::Set,
    player: Option<u128>,
) -> bool {
    if game.native_block(terminal) != content::TERMINAL {
        return false;
    }
    let Some(storage) = with_level(dimension, |l| l.terminals.get(&terminal).copied().flatten()).flatten() else {
        return false;
    };
    let book = crate::game::rustcraft_machines::entity(game, terminal).is_some_and(|e| orders::has_book(&e.nbt));
    typed_on(game, dimension, storage, book, set, player)
}

/// A typed limit or N on the storage `storage`, for an item it lists (holds some of, limits,
/// keeps N of, or, with the book, can make); an N only with the book and, above 0, only for an
/// item a recipe makes (OT8).
fn typed_on(
    game: &mut Game<'_>,
    dimension: &str,
    storage: u32,
    book: bool,
    set: typed::Set,
    player: Option<u128>,
) -> bool {
    let Some(key) = key_of_ref(game, dimension, storage, set.item) else {
        return false;
    };
    let held = with_level(dimension, |l| {
        l.stores
            .get(&storage)
            .is_some_and(|s| s.count(&key) > 0 || s.limits.contains_key(&key) || s.keep.contains_key(&key))
    })
    .unwrap_or(false);
    if !held && !(book && orders::can_order(&key)) {
        return false;
    }
    match set.press() {
        Press::Limit { change, .. } => apply_limit(dimension, storage, &key, change),
        Press::Keep { change, .. } => {
            if !book {
                return false;
            }
            if set.value > 0 && !orders::can_order(&key) {
                if let Some(uuid) = player {
                    let (lang, fallback) = orders::NOT_MADE;
                    tell(game, uuid, line(lang, fallback, &[item_text(game, &key)]));
                }
                return false;
            }
            apply_keep(dimension, storage, &key, change);
        }
        _ => return false,
    }
    true
}

/// The tablet's typed limit on the storage `id` (its window has no orders).
pub(super) fn tablet_typed(game: &mut Game<'_>, dimension: &str, id: u32, set: typed::Set, player: u128) -> bool {
    set.setting == typed::Setting::Limit && typed_on(game, dimension, id, false, set, Some(player))
}

/// The storage the terminal at `terminal` shows, and the level it is in, for the acting player.
fn terminal_storage(game: &Game<'_>, terminal: Pos) -> Option<(String, u32)> {
    let player = crate::game::rustcraft_server::acting(game)?;
    let id = with_level(&player.dimension, |level| {
        level.terminals.get(&terminal).copied().flatten()
    })
    .flatten()?;
    Some((player.dimension, id))
}

/// `native_menu_button_v2` on a terminal: a press of its window (rustcraft_rules::storage::
/// terminal::Press). The terminal's block entity is out of the world during the call, so the
/// book's socket is read and written in `entity`.
pub(super) fn on_button(
    game: &mut Game<'_>,
    pos: Pos,
    entity: &mut ModBlockEntity,
    id: i32,
    carried: &mut Option<ItemStack>,
) {
    if entity.id != content::TERMINAL {
        return;
    }
    let Some(press) = term::decode(id) else {
        return;
    };
    if let Press::Socket(k) = press {
        press_socket(entity, usize::from(k), carried);
        match crate::game::rustcraft_server::acting(game) {
            Some(player) => {
                orders::touch(&player.dimension);
                let id = with_level(&player.dimension, |l| l.terminals.get(&pos).copied().flatten());
                if let Some(id) = id.flatten() {
                    let own = socket_stack(&entity.nbt, sockets::SOCKET_STACK)
                        .map_or(0, |s| sockets::stack_tier(&s.item));
                    refresh_stack_module(game, &player.dimension, id, Some((pos, own)));
                }
            }
            None => orders::touch_all(),
        }
        return;
    }
    let book = orders::has_book(&entity.nbt);
    let Some((dimension, storage)) = terminal_storage(game, pos) else {
        return;
    };
    match press {
        Press::Socket(_) => {}
        Press::Order { item, count } => {
            // A fluid (O12) by its number past the item table, in buckets.
            let key = orders::key_of_number(game, item);
            let refused = if book {
                key.as_ref().and_then(|key| {
                    orders::order(&dimension, storage, key, u32::from(count)).err()
                })
            } else {
                Some(NO_BOOK)
            };
            if let (Some((lang, fallback)), Some(player)) = (refused, crate::game::rustcraft_server::acting(game)) {
                // OT8: «Осколок кристалла не делается на машинах: его только добывают».
                let args: Vec<Text> = key.as_deref().map(|k| item_text(game, k)).into_iter().collect();
                tell(game, player.uuid, line(lang, fallback, &args));
            }
        }
        Press::Keep { item, change } => {
            if let Some(key) = key_of_ref(game, &dimension, storage, (item, 0)).filter(|_| book) {
                // OT8: no «держать N» of an item no recipe makes; clearing one still goes.
                let raise = matches!(change, term::LimitChange::Add(n) if n > 0);
                if raise && !orders::can_order(&key) {
                    if let Some(player) = crate::game::rustcraft_server::acting(game) {
                        let (lang, fallback) = orders::NOT_MADE;
                        tell(game, player.uuid, line(lang, fallback, &[item_text(game, &key)]));
                    }
                } else {
                    apply_keep(&dimension, storage, &key, change);
                }
            }
        }
        Press::Cancel { order } => orders::cancel(&dimension, storage, order),
        Press::Place { take, .. } if carried.is_some() => {
            put_carried(game, &dimension, storage, carried, take == Take::One);
        }
        Press::Place {
            item: Some(item),
            take,
        } => take_out(game, &dimension, storage, item, take, carried),
        Press::Place { item: None, .. } => {}
        Press::Limit { item, change } => {
            if let Some(key) = key_of_ref(game, &dimension, storage, item) {
                apply_limit(&dimension, storage, &key, change);
            }
        }
    }
}

/// An item's or a fluid's name for a chat line (OT8).
fn item_text(game: &Game<'_>, key: &str) -> Text {
    if let Some(fluid) = rustcraft_rules::orders::fluid_of(key) {
        let name = fluid.rsplit(':').next().unwrap_or(fluid);
        return Text::translate_with_fallback(&format!("rustcraft.fluid.{name}.name"), fluid, &[]);
    }
    let id = keys::item_of(key);
    let kind = if game.items().get(id).is_some_and(|i| i.block.is_some()) { "block" } else { "item" };
    Text::translate_with_fallback(&format!("{kind}.{}", id.replace(':', ".")), id, &[])
}

// ---- Slice 18: the terminal's three sockets ----

/// A terminal's compound keeps the stack module's id and the filter module's whole saved stack
/// (its list is in it) under these keys; the recipe book is rustcraft_orders' `book` byte.
const STACK_SOCKET_KEY: &str = "stack_module";
const FILTER_SOCKET_KEY: &str = "filter_module";

/// What socket `k` of a terminal's compound holds.
pub(super) fn socket_stack(nbt: &Compound, k: usize) -> Option<ItemStack> {
    match k {
        sockets::SOCKET_STACK => as_string(field(nbt, STACK_SOCKET_KEY))
            .filter(|s| !s.is_empty())
            .map(|id| ItemStack::new(id.as_str(), 1)),
        sockets::SOCKET_BOOK => orders::has_book(nbt).then(|| ItemStack::new(orders::book_item(), 1)),
        sockets::SOCKET_FILTER => field(nbt, FILTER_SOCKET_KEY).and_then(save::item_from_nbt),
        _ => None,
    }
}

fn set_socket(nbt: &mut Compound, k: usize, stack: Option<ItemStack>) {
    match k {
        sockets::SOCKET_STACK => {
            let id = stack.map_or_else(String::new, |s| s.item.to_string());
            nbt.insert(JavaString::from(STACK_SOCKET_KEY), string(&id));
        }
        sockets::SOCKET_BOOK => orders::set_book(nbt, stack.is_some()),
        sockets::SOCKET_FILTER => match stack {
            Some(s) => {
                nbt.insert(JavaString::from(FILTER_SOCKET_KEY), save::item_to_nbt(&s));
            }
            None => {
                nbt.remove(&JavaString::from(FILTER_SOCKET_KEY));
            }
        },
        _ => {}
    }
}

/// A press on socket `k`, as a slot of one item takes it (storage::modules::socket_move).
fn press_socket(entity: &mut ModBlockEntity, k: usize, carried: &mut Option<ItemStack>) {
    let held = socket_stack(&entity.nbt, k);
    let cursor = carried.as_ref().map(|c| (&*c.item, c.count));
    let Some(m) = sockets::socket_move(k, held.as_ref().map(|h| &*h.item), cursor) else {
        return;
    };
    match m {
        sockets::SocketMove::In => {
            let Some(c) = carried.as_mut() else { return };
            let mut one = c.clone();
            one.count = 1;
            c.count -= 1;
            if c.count == 0 {
                *carried = None;
            }
            set_socket(&mut entity.nbt, k, Some(one));
        }
        sockets::SocketMove::Out => {
            set_socket(&mut entity.nbt, k, None);
            *carried = held;
        }
        sockets::SocketMove::Swap => {
            let into = carried.take();
            set_socket(&mut entity.nbt, k, into);
            *carried = held;
        }
    }
}

/// S3: the stack of the storage `id` from the best stack module of its terminals; `fresh` is a
/// terminal whose block entity is out of the world now, with the tier it holds.
fn refresh_stack_module(game: &Game<'_>, dimension: &str, id: u32, fresh: Option<(Pos, u8)>) {
    let terminals: Vec<Pos> = with_level(dimension, |level| {
        level
            .terminals
            .iter()
            .filter(|(_, s)| **s == Some(id))
            .map(|(t, _)| *t)
            .collect()
    })
    .unwrap_or_default();
    let tier = terminals
        .iter()
        .map(|&t| match fresh {
            Some((p, tier)) if p == t => tier,
            _ => crate::game::rustcraft_machines::entity(game, t)
                .and_then(|e| socket_stack(&e.nbt, sockets::SOCKET_STACK))
                .map_or(0, |s| sockets::stack_tier(&s.item)),
        })
        .max()
        .unwrap_or(0);
    with_level(dimension, |level| {
        if let Some(store) = level.stores.get_mut(&id)
            && store.stack_module != tier
        {
            store.stack_module = tier;
            level.changed(id);
        }
    });
}

/// The items the filter module in the terminal at `terminal` marks (the «своя» tab).
fn own_items(game: &Game<'_>, terminal: Pos) -> Vec<String> {
    crate::game::rustcraft_machines::entity(game, terminal)
        .and_then(|e| socket_stack(&e.nbt, sockets::SOCKET_FILTER))
        .map(|s| crate::game::rustcraft_filter::marked(&s))
        .unwrap_or_default()
}

/// A terminal broken in survival gives back its stack and filter modules (the book is
/// rustcraft_server's), into the inventory of `player` or onto the ground.
pub(super) fn give_back_sockets(game: &mut Game<'_>, dimension: &str, pos: Pos, player: u128) {
    let Some(nbt) = crate::game::rustcraft_machines::entity(game, pos).map(|e| e.nbt.clone()) else {
        return;
    };
    for k in [sockets::SOCKET_STACK, sockets::SOCKET_FILTER] {
        let Some(stack) = socket_stack(&nbt, k) else { continue };
        if let Some(rest) = game.native_give_stack(player, stack) {
            let nbt = Compound::from([(JavaString::from("Item"), save::item_to_nbt(&rest))]);
            let at = pos.map(|c| f64::from(c) + 0.5);
            if game.native_summon(dimension, "minecraft:item", at, nbt).is_none() {
                crate::game::rustcraft_debug::journal_warn!("rustcraft: the terminal's {} at {pos:?} was lost", rest.item);
            }
        }
    }
}

/// O14: a «держать» button on the storage `storage`: the N of `key` changed.
fn apply_keep(dimension: &str, storage: u32, key: &ItemKey, change: term::LimitChange) {
    with_level(dimension, |level| {
        if let Some(store) = level.stores.get_mut(&storage) {
            term::change_keep(store, key, change);
            level.changed(storage);
        }
    });
}

/// The self-test's «держать» N: the storage of the cell or terminal at `pos`; false when it
/// belongs to none.
pub(super) fn set_keep_at(dimension: &str, pos: Pos, key: &ItemKey, n: u64) -> bool {
    let Some(id) = storage_id(dimension, pos) else {
        return false;
    };
    apply_keep(dimension, id, key, term::LimitChange::Clear);
    let n = i64::try_from(n).unwrap_or(i64::MAX);
    apply_keep(dimension, id, key, term::LimitChange::Add(n));
    true
}

/// The storage the cell or terminal at `pos` belongs to (rustcraft_orders' self-test view).
pub(super) fn storage_at(dimension: &str, pos: Pos) -> Option<u32> {
    storage_id(dimension, pos)
}

/// A limit button on the storage `storage` (S5): the limit of `key` changed.
fn apply_limit(dimension: &str, storage: u32, key: &ItemKey, change: term::LimitChange) {
    with_level(dimension, |level| {
        if let Some(store) = level.stores.get_mut(&storage) {
            term::change_limit(store, key, change);
            level.changed(storage);
        }
    });
}

/// The self-test's limit button: the storage of the cell or terminal at `pos`; false when it
/// belongs to none.
pub(super) fn change_limit_at(
    dimension: &str,
    pos: Pos,
    key: &ItemKey,
    change: term::LimitChange,
) -> bool {
    let Some(id) = storage_id(dimension, pos) else {
        return false;
    };
    apply_limit(dimension, id, key, change);
    true
}

/// The storage the cell or terminal at `pos` belongs to.
fn storage_id(dimension: &str, pos: Pos) -> Option<u32> {
    with_level(dimension, |level| {
        level
            .cell_of
            .get(&pos)
            .copied()
            .or_else(|| level.terminals.get(&pos).copied().flatten())
    })
    .flatten()
}

/// The storage the cell or terminal at `pos` belongs to: its cells, their shape and its record.
pub(super) fn view(dimension: &str, pos: Pos) -> Option<(usize, Shape, Store)> {
    let id = storage_id(dimension, pos)?;
    with_level(dimension, |level| {
        let found = level.found.get(&id)?;
        Some((
            found.cells.len(),
            found.shape.clone(),
            level.stores.get(&id)?.clone(),
        ))
    })
    .flatten()
}

/// rustcraft_debug: the storage `id`: its anchor, cells, whether it works (S8) and its record.
pub(super) fn debug_info(dimension: &str, id: u32) -> Option<(Pos, usize, bool, Store)> {
    with_level(dimension, |level| {
        let found = level.found.get(&id)?;
        Some((
            found.anchor,
            found.cells.len(),
            level.works(id),
            level.stores.get(&id)?.clone(),
        ))
    })
    .flatten()
}

/// The level's storage record through `through` (the binary NBT a save writes and back) and its
/// codec: Err names what came back different.
pub(super) fn round_trip(
    dimension: &str,
    through: impl Fn(&Compound) -> Result<Compound, String>,
) -> Result<(), String> {
    through_record(dimension, through).map(|_| ())
}

/// The level's record as a save writes it, through `through` and back: what came back, or Err
/// naming what came back different.
fn through_record(
    dimension: &str,
    through: impl Fn(&Compound) -> Result<Compound, String>,
) -> Result<Saved, String> {
    let saved = with_level(dimension, |level| Saved {
        next_id: level.next_id,
        stores: level.stores.clone(),
        orders: level.orders.clone(),
    })
    .ok_or("no storage on this level")?;
    let encoded = encode_saved(&saved).map_err(|e| format!("{e:?}"))?;
    let back = decode_saved(&through(&encoded)?).map_err(|e| format!("{e:?}"))?;
    // Orders of a storage the record no longer has are not saved.
    let orders_kept: BTreeMap<u32, Vec<String>> = saved
        .orders
        .iter()
        .filter(|&(id, o)| saved.stores.contains_key(id) && !o.is_empty())
        .map(|(id, o)| (*id, o.clone()))
        .collect();
    if back.orders != orders_kept {
        return Err(format!(
            "orders {orders_kept:?} came back as {:?}",
            back.orders
        ));
    }
    // A store's cells are counted again by each rescan, not saved.
    type Kept = (u32, BTreeMap<ItemKey, u64>, BTreeMap<ItemKey, Limit>, BTreeMap<String, u64>);
    let kept = |s: &Saved| -> Vec<Kept> {
        s.stores
            .iter()
            .map(|(id, st)| (*id, st.items.clone(), st.limits.clone(), st.fluids.clone()))
            .collect()
    };
    if back.next_id != saved.next_id || kept(&back) != kept(&saved) {
        return Err(format!(
            "storages {:?} came back as {:?}",
            kept(&saved),
            kept(&back)
        ));
    }
    Ok(back)
}

/// The self-test's restart for the orders: the level's record through `through` and back, and
/// the level's orders and jobs read again from what came back, as a server start reads them.
pub(super) fn restart_orders(
    dimension: &str,
    through: impl Fn(&Compound) -> Result<Compound, String>,
) -> Result<(), String> {
    let back = through_record(dimension, through)?;
    orders::restore(dimension, &back.orders);
    with_level(dimension, |level| level.orders = back.orders);
    Ok(())
}

/// The key a press names; a component stack's by its place in the storage `storage`.
fn key_of_ref(
    game: &Game<'_>,
    dimension: &str,
    storage: u32,
    (number, damage): term::ItemRef,
) -> Option<ItemKey> {
    // Round 31 (FL5): a fluid's line, for its limit and «держать N».
    if let Some(fluid) = rustcraft_rules::orders::fluid_of_number(number) {
        return Some(rustcraft_rules::orders::fluid_key(fluid));
    }
    let item = item_of_number(game, u32::from(number))?;
    if damage & term::VARIANT != 0 {
        let index = damage & !term::VARIANT;
        return with_level(dimension, |level| {
            level.stores.get(&storage)?.variant(item, index)
        })
        .flatten();
    }
    Some(keys::key(item, u32::from(damage)))
}

/// The cursor's stack, or one item of it, into the storage, as far as it takes it.
fn put_carried(
    game: &Game<'_>,
    dimension: &str,
    storage: u32,
    carried: &mut Option<ItemStack>,
    one: bool,
) {
    let Some(stack) = carried.as_mut() else {
        return;
    };
    let Some(key) = key_of(stack) else {
        return;
    };
    let count = if one { 1 } else { stack.count };
    let took = put_into(game, dimension, storage, &key, u64::from(count)) as u32;
    stack.count -= took.min(stack.count);
    if stack.count == 0 {
        *carried = None;
    }
}

/// A stack, or one item, of a stored item onto the empty cursor or into the player's inventory.
fn take_out(
    game: &mut Game<'_>,
    dimension: &str,
    storage: u32,
    item: term::ItemRef,
    take: Take,
    carried: &mut Option<ItemStack>,
) {
    // Round 31: a fluid is not taken out by hand.
    let Some(key) = key_of_ref(game, dimension, storage, item)
        .filter(|k| rustcraft_rules::orders::fluid_of(k).is_none())
    else {
        return;
    };
    let stack_size = max_stack(game, keys::item_of(&key));
    let want = if take == Take::One { 1 } else { stack_size };
    let took = take_from(dimension, storage, &key, u64::from(want)) as u32;
    if took == 0 {
        return;
    }
    let stack = stack_of(&key, took);
    let player = game.native_acting_id();
    if take != Take::ToInventory {
        *carried = Some(stack);
        crate::game::rustcraft_debug::flow_to_player(dimension, storage, &key, player, u64::from(took));
        return;
    }
    let rest = game.native_give_stack(player, stack).map_or(0, |r| r.count);
    crate::game::rustcraft_debug::flow_to_player(dimension, storage, &key, player, u64::from(took.saturating_sub(rest)));
    restore_into(dimension, storage, &key, u64::from(rest));
}

/// `native_menu_quick_move` on a terminal: a stack of the inventory shift-clicked goes into the
/// storage as far as it takes it; the rest stays in its slot, never in the hidden slot.
pub(super) fn on_quick_move(
    game: &mut Game<'_>,
    pos: Pos,
    entity: &mut ModBlockEntity,
    container_slots: usize,
    slot: usize,
    stack: &mut ItemStack,
    handled: &mut bool,
) {
    if *handled || entity.id != content::TERMINAL {
        return;
    }
    *handled = true;
    if slot < container_slots {
        return;
    }
    // Slice 15, 18: a module goes into its empty socket, one of the stack.
    let free = (0..sockets::SOCKETS).find(|&k| {
        stack.count > 0 && sockets::socket_takes(k, &stack.item) && socket_stack(&entity.nbt, k).is_none()
    });
    if let Some(k) = free {
        let mut one = stack.clone();
        one.count = 1;
        set_socket(&mut entity.nbt, k, Some(one));
        stack.count -= 1;
        if let Some(player) = crate::game::rustcraft_server::acting(game) {
            orders::touch(&player.dimension);
            let id = with_level(&player.dimension, |l| l.terminals.get(&pos).copied().flatten());
            if k == sockets::SOCKET_STACK
                && let Some(id) = id.flatten()
            {
                let tier = sockets::stack_tier(&stack.item);
                refresh_stack_module(game, &player.dimension, id, Some((pos, tier)));
            }
        }
        return;
    }
    let Some((dimension, storage)) = terminal_storage(game, pos) else {
        return;
    };
    let Some(key) = key_of(stack) else {
        return;
    };
    let took = put_into(game, &dimension, storage, &key, u64::from(stack.count)) as u32;
    stack.count -= took.min(stack.count);
}

// ---- A cell broken ----

/// `native_destroy_block` on a cell: the last cell of a storage drops what the storage held, as a
/// chest does; any other cell leaves it to the storage's other cells.
pub(super) fn on_cell_broken(game: &mut Game<'_>, dimension: &str, pos: Pos) {
    let id = with_level(dimension, |level| {
        let id = *level.cell_of.get(&pos)?;
        let others = level.found.get(&id)?.cells.iter().any(|&c| c != pos);
        Some((id, others))
    })
    .flatten();
    let Some((id, others)) = id else {
        return;
    };
    let live = others
        && with_level(dimension, |level| {
            level.found.get(&id).map(|f| f.cells.clone())
        })
        .flatten()
        .is_some_and(|cells| {
            cells
                .iter()
                .any(|&c| c != pos && game.native_block(c) == content::CELL)
        });
    if live {
        return;
    }
    let held = with_level(dimension, |level| {
        level.found.remove(&id);
        level.cell_of.remove(&pos);
        let store = level.stores.remove(&id);
        level.dirty = true;
        store
    })
    .flatten();
    let Some(store) = held else {
        return;
    };
    let mut stacks = 0;
    for (key, mut count) in store.items {
        let size = u64::from(max_stack(game, keys::item_of(&key)));
        while count > 0 {
            let n = count.min(size);
            count -= n;
            drop_stack(game, dimension, pos, &key, n as u32);
            stacks += 1;
        }
    }
    if stacks > 0 {
        log::info!("rustcraft: the last cell at {pos:?} dropped {stacks} stacks");
    }
}

/// An item entity of `count` of `key` at the middle of `pos` (`/summon`'s compound, the damage as
/// the stack's `minecraft:damage` component).
pub(super) fn drop_stack(game: &mut Game<'_>, dimension: &str, pos: Pos, key: &str, count: u32) {
    let (item, damage) = keys::parse(key);
    let mut stack = vec![("id", string(item)), ("count", Tag::Int(count as i32))];
    if damage > 0 {
        let components = Compound::from([(
            JavaString::from("minecraft:damage"),
            Tag::Int(damage as i32),
        )]);
        stack.push(("components", Tag::Compound(components)));
    }
    let nbt = Compound::from([(JavaString::from("Item"), Tag::compound(stack))]);
    let at = pos.map(|c| f64::from(c) + 0.5);
    if game
        .native_summon(dimension, "minecraft:item", at, nbt)
        .is_none()
    {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: {count} {key} from the cell at {pos:?} were lost");
    }
}

// ---- What a look shows ----

/// The diagnosis of a cell or a terminal: «Склад: 4 ячейки, занято 6 из 108 слотов», «Нарушена
/// форма склада: нет ячейки 10 64 -3», «Терминал не касается ячеек склада».
pub(super) fn diagnosis(game: &Game<'_>, dimension: &str, pos: Pos, block: &str) -> Option<Text> {
    let stack = |k: &str| max_stack(game, keys::item_of(k));
    with_level(dimension, |level| {
        let id = if block == content::CELL {
            level.cell_of.get(&pos).copied()
        } else {
            level.terminals.get(&pos).copied().flatten()
        };
        let Some(id) = id else {
            return Some(line(
                "rustcraft.storage.no_cells",
                "The terminal touches no storage cell",
                &[],
            ));
        };
        let found = level.found.get(&id)?;
        let store = level.stores.get(&id)?;
        Some(match &found.shape {
            // Slice 18: «Склад полон».
            Shape::Solid if store.total_slots() > 0 && store.used_slots(&stack) >= store.total_slots() => line(
                "rustcraft.storage.full_at",
                "The storage is full: %s of %s slots used",
                &[
                    Text::literal(store.used_slots(&stack).to_string()),
                    Text::literal(store.total_slots().to_string()),
                ],
            ),
            Shape::Solid => line(
                "rustcraft.storage.works",
                "Storage: %s cells, %s of %s slots used",
                &[
                    Text::literal(found.cells.len().to_string()),
                    Text::literal(store.used_slots(&stack).to_string()),
                    Text::literal(store.total_slots().to_string()),
                ],
            ),
            Shape::Holes(holes) => holes_text(holes),
            Shape::Extra(cells) => extra_text(cells),
        })
    })
    .flatten()
}

/// Wave 2: «Нарушена форма склада: лишняя ячейка 10 64 -3 (и ещё 2)».
pub(super) fn extra_text(cells: &BTreeSet<Pos>) -> Text {
    let first = cells.first().copied().unwrap_or_default();
    let at = Text::literal(format!("{} {} {}", first[0], first[1], first[2]));
    let mut text = line(
        "rustcraft.storage.extra",
        "The storage is no solid box: a cell sticks out at %s",
        &[at],
    );
    if cells.len() > 1 {
        text.push(line(
            "rustcraft.storage.holes_more",
            " (and %s more)",
            &[Text::literal((cells.len() - 1).to_string())],
        ));
    }
    text
}

/// «Нарушена форма склада: нет ячейки 10 64 -3 (и ещё 2)».
pub(super) fn holes_text(holes: &BTreeSet<Pos>) -> Text {
    let first = holes.first().copied().unwrap_or_default();
    let at = Text::literal(format!("{} {} {}", first[0], first[1], first[2]));
    let mut text = line(
        "rustcraft.storage.holes",
        "The storage is no solid box: no cell at %s",
        &[at],
    );
    if holes.len() > 1 {
        text.push(line(
            "rustcraft.storage.holes_more",
            " (and %s more)",
            &[Text::literal((holes.len() - 1).to_string())],
        ));
    }
    text
}

// ---- Slice 14: the storage tablet (rustcraft_tablet) ----

/// The level's storages the tablet may open, each with the blocks it is joined to the magistral
/// by: its cells and the terminals that touch it.
pub(super) fn tablet_candidates(dimension: &str) -> Vec<storage::tablet::Candidate> {
    with_level(dimension, |level| {
        level
            .found
            .iter()
            .map(|(id, f)| {
                let mut blocks: Vec<Pos> = f.cells.iter().copied().collect();
                blocks.extend(
                    level
                        .terminals
                        .iter()
                        .filter(|(_, t)| **t == Some(*id))
                        .map(|(p, _)| *p),
                );
                storage::tablet::Candidate { id: *id, blocks }
            })
            .collect()
    })
    .unwrap_or_default()
}

/// What the storage `id` holds, as the tablet lists it: most numerous first.
pub(super) fn tablet_items(dimension: &str, id: u32) -> Vec<(ItemKey, u64)> {
    with_level(dimension, |level| {
        level
            .stores
            .get(&id)
            .map(|s| storage::tablet::entries(&s.items))
    })
    .flatten()
    .unwrap_or_default()
}

// ---- Slice 18: the projector's and the building machine's storage ----

/// The storage on the item net a housing beside the block at `pos` is on (S6's rule for
/// terminals): the projector's «В руку», the building machine's blocks.
pub(super) fn storage_by(dimension: &str, pos: Pos, topology: &rustcraft_rules::net::topology::Topology) -> Option<u32> {
    with_level(dimension, |level| {
        if let Some(id) = level.cell_of.get(&pos).copied().or_else(|| level.terminals.get(&pos).copied().flatten()) {
            return Some(id);
        }
        let mut blocks: BTreeMap<u32, Vec<Pos>> = BTreeMap::new();
        for (&c, &id) in &level.cell_of {
            blocks.entry(id).or_default().push(c);
        }
        for (&t, id) in &level.terminals {
            if let Some(id) = id {
                blocks.entry(*id).or_default().push(t);
            }
        }
        cells::terminal_on_net(pos, &blocks, topology).filter(|id| level.works(*id))
    })
    .flatten()
}

/// How many of `key` the storage `id` holds (none while it does not work).
pub(super) fn stored_count(dimension: &str, id: u32, key: &str) -> u64 {
    with_level(dimension, |level| {
        level.works(id).then(|| level.stores.get(&id).map_or(0, |s| s.count(key)))
    })
    .flatten()
    .unwrap_or(0)
}

/// 0.21 (spec 38, FR1): every key of `item` the storage `id` holds, with its count (none while it
/// does not work): a fishing station picks the least worn rod among them.
pub(super) fn stored_keys_of(dimension: &str, id: u32, item: &str) -> Vec<(ItemKey, u64)> {
    with_level(dimension, |level| {
        level.works(id).then(|| {
            level.stores.get(&id).map_or_else(Vec::new, |s| {
                s.items
                    .iter()
                    .filter(|(k, n)| **n > 0 && keys::item_of(k) == item)
                    .map(|(k, n)| (k.clone(), *n))
                    .collect()
            })
        })
    })
    .flatten()
    .unwrap_or_default()
}

/// Up to `count` of `key` from the storage `id` into the inventory of `player`; what does not
/// fit stays in the storage. How many went.
pub(super) fn give_from(game: &mut Game<'_>, dimension: &str, id: u32, key: &str, count: u32, player: u128) -> u32 {
    let took = take_from(dimension, id, key, u64::from(count)) as u32;
    if took == 0 {
        return 0;
    }
    let rest = game
        .native_give_stack(player, stack_of(key, took))
        .map_or(0, |r| r.count.min(took));
    restore_into(dimension, id, key, u64::from(rest));
    crate::game::rustcraft_debug::flow_to_player(dimension, id, key, player, u64::from(took - rest));
    took - rest
}

// ---- Slice 18: the tablet's drawn window ----

/// The storage `id`'s change count: its list is sent again when it moves on.
pub(super) fn version(dimension: &str, id: u32) -> u64 {
    with_level(dimension, |level| level.versions.get(&id).copied().unwrap_or(0)).unwrap_or(0)
}

/// What the tablet's window shows of the storage `id`: the list a terminal of it shows (its
/// first terminal's sockets and filter count; the cells' anchor when it has none).
pub(super) fn tablet_listing(game: &Game<'_>, dimension: &str, id: u32) -> Listing {
    let at = with_level(dimension, |level| {
        level
            .terminals
            .iter()
            .find(|(_, s)| **s == Some(id))
            .map(|(t, _)| *t)
            .or_else(|| level.found.get(&id).map(|f| f.anchor))
    })
    .flatten()
    .unwrap_or_default();
    listing(game, dimension, at, Some(id))
}

/// A press on a stored item in the tablet's window: a stack (or one) into the inventory of
/// `player`; what does not fit stays in the storage. How many went.
pub(super) fn tablet_take(
    game: &mut Game<'_>,
    dimension: &str,
    id: u32,
    item: term::ItemRef,
    one: bool,
    player: u128,
) -> u32 {
    let Some(key) = key_of_ref(game, dimension, id, item) else {
        return 0;
    };
    let want = if one { 1 } else { max_stack(game, keys::item_of(&key)) };
    let took = take_from(dimension, id, &key, u64::from(want)) as u32;
    if took == 0 {
        return 0;
    }
    let rest = game
        .native_give_stack(player, stack_of(&key, took))
        .map_or(0, |r| r.count.min(took));
    restore_into(dimension, id, &key, u64::from(rest));
    crate::game::rustcraft_debug::flow_to_player(dimension, id, &key, player, u64::from(took - rest));
    took - rest
}

/// A limit button in the tablet's window (S5).
pub(super) fn tablet_limit(game: &Game<'_>, dimension: &str, id: u32, item: term::ItemRef, change: term::LimitChange) {
    if let Some(key) = key_of_ref(game, dimension, id, item) {
        apply_limit(dimension, id, &key, change);
    }
}

/// The tablet's deposit: the stack in inventory slot `slot` of `player` into the storage `id`,
/// as far as it takes it; the rest goes back to the player. How many went in.
pub(super) fn tablet_deposit(game: &mut Game<'_>, dimension: &str, id: u32, player: u128, slot: usize) -> u32 {
    let Some(stack) = game.native_player_stacks(player).and_then(|s| s.get(slot).cloned().flatten()) else {
        return 0;
    };
    let Some(key) = key_of(&stack) else {
        return 0;
    };
    let room = room_in(game, dimension, id, &key).min(u64::from(stack.count)) as u32;
    if room == 0 {
        return 0;
    }
    let rest = (room < stack.count).then(|| {
        let mut r = stack.clone();
        r.count -= room;
        r
    });
    let taken = game.native_set_inventory_slot(
        player,
        crate::game::native_inventories::NativeInventoryKind::Player,
        slot,
        Some(&stack),
        rest,
    );
    if taken.is_err() {
        return 0;
    }
    let took = put_into(game, dimension, id, &key, u64::from(room)) as u32;
    if took < room {
        // Never lost: what the storage did not take after all goes back.
        let mut back = stack.clone();
        back.count = room - took;
        if let Some(r) = game.native_give_stack(player, back) {
            restore_into(dimension, id, &key, u64::from(r.count));
        }
    }
    took
}

/// The self-test's view: every stack the storage the cell or terminal at `pos` belongs to
/// holds, made from its key (a component key's stack with its components).
pub(super) fn stored_stacks_at(dimension: &str, pos: Pos) -> Vec<ItemStack> {
    let Some(id) = storage_id(dimension, pos) else {
        return Vec::new();
    };
    with_level(dimension, |level| {
        level.stores.get(&id).map(|s| {
            s.items
                .iter()
                .map(|(k, n)| stack_of(k, u32::try_from(*n).unwrap_or(u32::MAX)))
                .collect()
        })
    })
    .flatten()
    .unwrap_or_default()
}

/// The stack `count` of a stored key make.
pub(super) fn stored_stack(key: &str, count: u32) -> ItemStack {
    stack_of(key, count)
}

/// The tablet's click: a stack of `key` from the storage `id` into the inventory of `player`
/// (`native_give_stack`, from any server hook); what does not fit stays in the storage. How
/// many went.
pub(super) fn take_to_player(
    game: &mut Game<'_>,
    dimension: &str,
    id: u32,
    key: &str,
    player: u128,
) -> u32 {
    let want = max_stack(game, keys::item_of(key));
    let took = take_from(dimension, id, key, u64::from(want)) as u32;
    if took == 0 {
        return 0;
    }
    let rest = game
        .native_give_stack(player, stack_of(key, took))
        .map_or(0, |r| r.count.min(took));
    restore_into(dimension, id, key, u64::from(rest));
    crate::game::rustcraft_debug::flow_to_player(dimension, id, key, player, u64::from(took - rest));
    took - rest
}
