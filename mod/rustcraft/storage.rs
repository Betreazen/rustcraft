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
// A shift-click from the inventory puts the stack in (`native_menu_quick_move`, API 309).
use crate::game::{
    Game,
    rustcraft_content::{self as content, item_number, item_of_number},
    rustcraft_machines::{max_stack, read_int, write_int},
    rustcraft_server::line,
};
use horizon_client::{
    containers::ModBlockEntity,
    items::ItemStack,
    mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Holder, Host},
    text::Text,
};
use horizon_core::geometry::Direction;
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    net::{items::ItemKey, stock::NetStore},
    storage::{
        self, Limit, Shape, Store, cells, keys,
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
    sent: Option<(Option<u32>, u64)>,
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

/// `{next, stores: [{id, items: [{k, n}], limits: [{k, max, destroy}]}]}`.
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
            Tag::compound([
                ("id", Tag::Int(*id as i32)),
                ("items", Tag::List(items)),
                ("limits", Tag::List(limits)),
            ])
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
        saved.stores.insert(id as u32, store);
    }
    Ok(saved)
}

/// The level's record, read once when the server first looks at the level's storages.
fn load(game: &mut Game<'_>, level: &mut LevelStorage) {
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
}

/// Writes the level's record when it changed. Only from the level's own tick, so the level in
/// place is the one the record belongs to.
fn save(game: &mut Game<'_>, dimension: &str) {
    let saved = with_level(dimension, |level| {
        std::mem::replace(&mut level.dirty, false).then(|| Saved {
            next_id: level.next_id,
            stores: level.stores.clone(),
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
    load(game, &mut level);
    let ids: BTreeMap<Pos, Option<u32>> = cell_blocks
        .iter()
        .map(|&p| {
            let id = game
                .native_mod_block_entity(p)
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
    storages().levels.insert(dimension.to_owned(), level);
    save(game, dimension);
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

// ---- Items in and out ----

/// The stack a key and a count make.
fn stack_of(key: &str, count: u32) -> ItemStack {
    let (item, damage) = keys::parse(key);
    let mut stack = ItemStack::new(item, count);
    stack.damage = damage;
    stack
}

/// The key a stack goes under; none for a stack the storage cannot keep: one with components
/// besides its damage, or worn further than a press can name.
fn key_of(stack: &ItemStack) -> Option<ItemKey> {
    (stack.extra.is_none() && term::addressable(stack.damage) && stack.count > 0)
        .then(|| keys::key(&stack.item, stack.damage))
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

/// Puts `count` of `key` into the storage `id`; how many it took (destroyed surplus counts).
fn put_into(game: &Game<'_>, dimension: &str, id: u32, key: &str, count: u64) -> u64 {
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

fn take_from(dimension: &str, id: u32, key: &str, count: u64) -> u64 {
    with_level(dimension, |level| {
        let took = level.stores.get_mut(&id).map_or(0, |s| s.take(key, count));
        if took > 0 {
            level.changed(id);
        }
        took
    })
    .unwrap_or(0)
}

fn restore_into(dimension: &str, id: u32, key: &str, count: u64) {
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
                (v.sent != Some((id, version))).then_some((*uuid, v.terminal, id, version))
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
            v.sent = Some((id, version));
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
    let mut keys: BTreeSet<&ItemKey> = store.items.keys().collect();
    keys.extend(store.limits.keys());
    for key in keys {
        let (item, damage) = keys::parse(key);
        let Some(number) = item_number(game, item).and_then(|n| u16::try_from(n).ok()) else {
            continue;
        };
        let Ok(damage) = u16::try_from(damage) else {
            continue;
        };
        let count = store.count(key);
        lines.push((
            count,
            Line {
                item: number,
                damage,
                count,
                limit: store.limits.get(key).copied(),
            },
        ));
    }
    // The most numerous first: a message too small for them all keeps those.
    lines.sort_by(|a, b| b.0.cmp(&a.0));
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
    }
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
/// terminal::Press).
pub(super) fn on_button(
    game: &mut Game<'_>,
    pos: Pos,
    entity: &ModBlockEntity,
    id: i32,
    carried: &mut Option<ItemStack>,
) {
    if entity.id != content::TERMINAL {
        return;
    }
    let Some(press) = term::decode(id) else {
        return;
    };
    let Some((dimension, storage)) = terminal_storage(game, pos) else {
        return;
    };
    match press {
        Press::Place { take, .. } if carried.is_some() => {
            put_carried(game, &dimension, storage, carried, take == Take::One);
        }
        Press::Place {
            item: Some(item),
            take,
        } => take_out(game, &dimension, storage, item, take, carried),
        Press::Place { item: None, .. } => {}
        Press::Limit { item, change } => {
            if let Some(key) = key_of_ref(game, item) {
                apply_limit(&dimension, storage, &key, change);
            }
        }
    }
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
    let saved = with_level(dimension, |level| Saved {
        next_id: level.next_id,
        stores: level.stores.clone(),
    })
    .ok_or("no storage on this level")?;
    let encoded = encode_saved(&saved).map_err(|e| format!("{e:?}"))?;
    let back = decode_saved(&through(&encoded)?).map_err(|e| format!("{e:?}"))?;
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
    Ok(())
}

/// The key a press names.
fn key_of_ref(game: &Game<'_>, (number, damage): term::ItemRef) -> Option<ItemKey> {
    let item = item_of_number(game, u32::from(number))?;
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
    let Some(key) = key_of_ref(game, item) else {
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
fn drop_stack(game: &mut Game<'_>, dimension: &str, pos: Pos, key: &str, count: u32) {
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
