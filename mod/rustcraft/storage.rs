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
    geom::Pos,
    net::{items::ItemKey, stock::NetStore},
    storage::{
        self, Limit, Shape, Store, cells, keys, modules as sockets,
        terminal::{self as term, Press, Take},
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
    /// Changed since the attachment was last written.
    dirty: bool,
    ticks: u64,
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
}

#[derive(Default)]
struct Storages {
    levels: HashMap<String, LevelStorage>,
    viewers: HashMap<u128, Viewer>,
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
/// orders: [string]}]}`; `orders` only for a storage that has some.
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
            log::warn!("rustcraft: a storage record without an id was left out");
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
        log::warn!("rustcraft: storages not read: {e:?}");
        Saved::default()
    });
    level.stores = saved.stores;
    level.next_id = saved.next_id.max(1);
    level.loaded = true;
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
        log::warn!("rustcraft: storages not saved: {e:?}");
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
    let holes: Vec<(Pos, Vec<Pos>)> = level
        .terminals
        .iter()
        .map(|(&t, id)| {
            let h = id
                .and_then(|id| level.found.get(&id))
                .map(|f| match &f.shape {
                    Shape::Solid => Vec::new(),
                    Shape::Holes(h) => h.iter().copied().take(HOLES_SHOWN).collect(),
                })
                .unwrap_or_default();
            (t, h)
        })
        .collect();
    let ids: Vec<u32> = level.found.keys().copied().collect();
    storages().levels.insert(dimension.to_owned(), level);
    // S3: each storage's stack from its terminals' stack modules.
    for id in ids {
        refresh_stack_module(game, dimension, id, None);
    }
    // S8: each terminal shows its storage's missing cells in the world.
    for (t, h) in holes {
        write_holes(game, t, &h);
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
/// The most missing cells a terminal draws.
const HOLES_SHOWN: usize = 64;

/// The missing cells into the terminal at `t`, and the clients told when they changed.
fn write_holes(game: &mut Game<'_>, t: Pos, holes: &[Pos]) {
    let flat: Vec<i32> = holes
        .iter()
        .flat_map(|h| [h[0] - t[0], h[1] - t[1], h[2] - t[2]])
        .collect();
    let changed = game
        .native_with_mod_block_entity(t, |e| {
            let key = JavaString::from(HOLES_KEY);
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
    match entity.nbt.get(&JavaString::from(HOLES_KEY)) {
        Some(Tag::IntArray(v)) => v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
        _ => Vec::new(),
    }
}

/// `native_block_entity_update_tag`: clients get a terminal's missing cells.
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id != content::TERMINAL {
        return;
    }
    if let Some(v) = entity.nbt.get(&JavaString::from(HOLES_KEY)) {
        tag.get_or_insert_with(Compound::new)
            .insert(JavaString::from(HOLES_KEY), v.clone());
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
}

/// The terminal's culling box takes in its missing cells.
pub(super) fn hole_bounds(entity: &ModBlockEntity, bounds: &mut [f32; 6]) {
    for h in holes_of(entity) {
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
        log::warn!("rustcraft: the cell at {cell:?} has no block entity");
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
    if stack.extra.is_none() && term::addressable(stack.damage) {
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
    put as u32
}

// ---- The level's tick: saving and the viewers' lists ----

/// `native_level_tick_post`: the record written when it changed, and every SEND_EVERY ticks the
/// changed lists sent to the players who have a terminal of this level open.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    save(game, dimension);
    let send = with_level(dimension, |level| {
        level.ticks += 1;
        level.ticks.is_multiple_of(SEND_EVERY)
    })
    .unwrap_or(false);
    if !send {
        return;
    }
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
        let bytes = wire::encode(&listing(game, dimension, terminal, id));
        if let Err(e) = game.native_net_send_to_player(uuid, CHANNEL, &bytes) {
            log::debug!("rustcraft: the terminal's list not sent to {uuid:032x}: {e}");
            continue;
        }
        if let Some(v) = storages().viewers.get_mut(&uuid) {
            v.sent = Some((id, version, orders_revision));
        }
    }
}

/// What the terminal at `terminal` shows of the storage `id`.
fn listing(game: &Game<'_>, dimension: &str, terminal: Pos, id: Option<u32>) -> Listing {
    let stack = |k: &str| max_stack(game, keys::item_of(k));
    let shown = with_level(dimension, |level| {
        let id = id?;
        let store = level.stores.get(&id)?.clone();
        let found = level.found.get(&id)?;
        let holes = match &found.shape {
            Shape::Solid => Vec::new(),
            Shape::Holes(h) => h.iter().copied().collect(),
        };
        Some((store, holes))
    })
    .flatten();
    let Some((store, holes)) = shown else {
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
    // The most numerous first: a message too small for them all keeps those.
    lines.sort_by(|a, b| b.0.cmp(&a.0));
    let view = id.map(|id| orders::view(game, dimension, terminal, id));
    Listing {
        terminal,
        state: if holes.is_empty() {
            State::Works
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
        variants,
        sockets: socket_numbers(game, terminal),
        own: own_items(game, terminal)
            .iter()
            .filter_map(|i| item_number(game, i).and_then(|n| u16::try_from(n).ok()))
            .collect(),
    }
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

/// The storage the terminal at `terminal` shows, and the level it is in, for the acting player.
fn terminal_storage(game: &Game<'_>, terminal: Pos) -> Option<(String, u32)> {
    let player = game.native_acting_player()?;
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
        match game.native_acting_player() {
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
            let refused = if book {
                // A fluid (O12) by its number past the item table, in buckets.
                orders::key_of_number(game, item).and_then(|key| {
                    orders::order(&dimension, storage, &key, u32::from(count)).err()
                })
            } else {
                Some(NO_BOOK)
            };
            if let (Some((key, fallback)), Some(player)) = (refused, game.native_acting_player()) {
                tell(game, player.uuid, line(key, fallback, &[]));
            }
        }
        Press::Keep { item, change } => {
            if let Some(key) = key_of_ref(game, &dimension, storage, (item, 0)).filter(|_| book) {
                apply_keep(&dimension, storage, &key, change);
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
    let Some(entity) = crate::game::rustcraft_machines::entity(game, pos) else {
        return;
    };
    for k in [sockets::SOCKET_STACK, sockets::SOCKET_FILTER] {
        let Some(stack) = socket_stack(&entity.nbt, k) else { continue };
        if let Some(rest) = game.native_give_stack(player, stack) {
            let nbt = Compound::from([(JavaString::from("Item"), save::item_to_nbt(&rest))]);
            let at = pos.map(|c| f64::from(c) + 0.5);
            if game.native_summon(dimension, "minecraft:item", at, nbt).is_none() {
                log::warn!("rustcraft: the terminal's {} at {pos:?} was lost", rest.item);
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
    let kept = |s: &Saved| -> Vec<(u32, BTreeMap<ItemKey, u64>, BTreeMap<ItemKey, Limit>)> {
        s.stores
            .iter()
            .map(|(id, st)| (*id, st.items.clone(), st.limits.clone()))
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
    let Some(key) = key_of_ref(game, dimension, storage, item) else {
        return;
    };
    let stack_size = max_stack(game, keys::item_of(&key));
    let want = if take == Take::One { 1 } else { stack_size };
    let took = take_from(dimension, storage, &key, u64::from(want)) as u32;
    if took == 0 {
        return;
    }
    let stack = stack_of(&key, took);
    if take != Take::ToInventory {
        *carried = Some(stack);
        return;
    }
    let player = game.native_acting_id();
    let rest = game.native_give_stack(player, stack).map_or(0, |r| r.count);
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
        if let Some(player) = game.native_acting_player() {
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
        log::warn!("rustcraft: {count} {key} from the cell at {pos:?} were lost");
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
        })
    })
    .flatten()
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
    took - rest
}
