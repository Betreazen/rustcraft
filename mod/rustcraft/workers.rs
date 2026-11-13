// The worker cart on the server (spec 4.7, W1–W7; slice 7): the chassis placed on a floor rail
// becomes a worker; rustcraft_rules::worker rides it along its rails, sends it to the docks and
// says what its tool may cut; this module breaks the blocks (`native_machine_destroy_block`,
// game/native_machines.rs, with the drops collected into its hold), plants again, serves it at the
// docks, keeps the fleet in a level attachment `rustcraft:workers` (W7: it survives a restart and
// freezes while its chunk is not loaded) and shows it.
//
// How it shows: as the capsule (rustcraft_monorail), a seat mob the server moves each tick — a pig
// with no AI, no gravity and no sound (`native_spawn_mob`, `native_teleport_mob`), tagged
// `rustcraft_worker` so a seat saved with its chunk is found and removed again — and every client
// draws the guide's worker round it (rustcraft_station::draw_worker, `native_mob_submit`). Its name
// tag is the board over it (W5): charge, hold and state.
//
// Hands: the chassis clicked on a floor rail places a worker (`native_use_item_on`); a click on the
// worker opens its window, a server menu like the joint window (rustcraft_window), and the wrench
// with sneaking takes it back with its tool, upgrades and load (`native_interact_entity`, API 29).
use crate::game::{
    Game,
    native_inventories::NativeInventoryKind,
    native_mobs::NativeSpawn,
    rustcraft_base as base,
    rustcraft_content::{self as content, Machine},
    rustcraft_machines as machines, rustcraft_monorail as monorail,
    rustcraft_server::{self as server, LevelNet, line, tell},
    rustcraft_transport as transport,
    rustcraft_window::named,
};
use horizon_client::{
    items::ItemStack,
    mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Holder, Host},
    text::Text,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    modules::{Holder as ModuleHolder, Module},
    monorail::Network,
    reports::Report,
    tuning::Tuning,
    worker::{BlockView, Errand, Fleet, Note, Step, Tool, Track, Worker, plants, trees},
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

/// The workers' seats and tools, server to every client.
pub const NOTES: &str = "rustcraft:workers";
const SEAT_KIND: &str = "minecraft:pig";
const SEAT_TAG: &str = "rustcraft_worker";
const AIR: &str = "minecraft:air";
/// How often clients are told the seats again, the boards change, and lost seats are swept.
const NOTES_EVERY: u64 = 10;
const BOARD_EVERY: u64 = 20;
const SWEEP_EVERY: u64 = 100;
/// The unload dock hands its load to a container beside it this often, this many at a time.
const DOCK_PUSH_EVERY: u64 = 8;
const DOCK_PUSH: u32 = 8;
const ROWS: u8 = 6;
const CELLS: usize = 54;
const MAIN_SLOTS: usize = 27;
const INVENTORY_SLOTS: usize = 36;
/// The window's first row: tool, upgrades, then the gauges.
const TOOL_CELL: usize = 0;
const UPGRADE_CELLS: std::ops::Range<usize> = 1..4;
const CHARGE_CELL: usize = 5;
const HOLD_CELL: usize = 6;
const NOTE_CELL: usize = 7;
const HOLD_FIRST: usize = 9;
/// What the saw's loot is rolled for: an axe, as a player's would be.
const SAW_TOOL: &str = "minecraft:iron_axe";

// ---- State ----

#[derive(Default)]
struct LevelFleet {
    fleet: Fleet,
    loaded: bool,
    dirty: bool,
    /// Worker id → its seat mob.
    seats: HashMap<u64, u32>,
    boards: HashMap<u64, String>,
    ticks: u64,
}

struct Session {
    dimension: String,
    worker: u64,
    window: i32,
}

struct Click {
    uuid: u128,
    window: i32,
    slot: usize,
}

#[derive(Default)]
struct Workers {
    levels: HashMap<String, LevelFleet>,
    sessions: HashMap<u128, Session>,
    opening: Vec<(u128, String, u64)>,
    clicks: Vec<Click>,
    /// Breaks to report once the level ticks (inside a player's hook no command runs).
    placing: Vec<(u128, String, Pos)>,
}

static STATE: LazyLock<Mutex<Workers>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, Workers> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

static SAVED: AttachmentType<String> = AttachmentType {
    namespace: "rustcraft",
    name: "workers",
    default: String::new,
    codec: Some(AttachmentCodec {
        encode: encode_saved,
        decode: decode_saved,
    }),
    copy_on_death: false,
    sync: false,
};

/// `{fleet: "<json>"}`: the fleet as rustcraft_rules::worker::Fleet writes it.
fn encode_saved(saved: &String) -> Result<Compound, AttachmentError> {
    Ok(Compound::from([(
        JavaString::from("fleet"),
        Tag::String(JavaString::from(saved.as_str())),
    )]))
}

fn decode_saved(data: &Compound) -> Result<String, AttachmentError> {
    Ok(match data.get(&JavaString::from("fleet")) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    })
}

fn load(game: &mut Game<'_>, level: &mut LevelFleet) {
    if level.loaded {
        return;
    }
    let host: &mut dyn Host = game;
    let text = SAVED.get(host, Holder::Level).unwrap_or_default();
    level.fleet = if text.is_empty() {
        Fleet::default()
    } else {
        Fleet::decode(&text).unwrap_or_else(|| {
            log::warn!("rustcraft: the workers do not read; starting with none");
            Fleet::default()
        })
    };
    level.loaded = true;
}

fn save(game: &mut Game<'_>, level: &mut LevelFleet) {
    if !std::mem::replace(&mut level.dirty, false) {
        return;
    }
    let host: &mut dyn Host = game;
    if let Err(e) = SAVED.set(host, Holder::Level, level.fleet.encode()) {
        log::warn!("rustcraft: the workers not saved: {e:?}");
    }
}

fn take_level(dimension: &str) -> LevelFleet {
    state().levels.remove(dimension).unwrap_or_default()
}

fn put_level(dimension: &str, level: LevelFleet) {
    state().levels.insert(dimension.to_owned(), level);
}

fn chunk_loaded(game: &Game<'_>, p: Pos) -> bool {
    game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4))
}

// ---- The world as the tools see it ----

struct View<'g, 'a> {
    game: &'g Game<'a>,
}

impl BlockView for View<'_, '_> {
    fn block(&self, p: Pos) -> String {
        self.game.native_block(p)
    }
    fn prop(&self, p: Pos, key: &str) -> Option<String> {
        self.game.native_block_property(p, key)
    }
    fn is_log(&self, p: Pos) -> bool {
        in_tag(
            self.game,
            &self.game.native_block(p),
            "minecraft:logs",
            "_log",
        )
    }
    fn is_leaves(&self, p: Pos) -> bool {
        in_tag(
            self.game,
            &self.game.native_block(p),
            "minecraft:leaves",
            "_leaves",
        )
    }
    fn is_dirt(&self, p: Pos) -> bool {
        let b = self.game.native_block(p);
        in_tag(self.game, &b, "minecraft:dirt", "minecraft:dirt") || b == "minecraft:grass_block"
    }
}

/// Whether `block` is in `tag` by the server's merged tags; before tags load, by its name.
fn in_tag(game: &Game<'_>, block: &str, tag: &str, fallback: &str) -> bool {
    game.native_block_name_in_tag(block, tag)
        .unwrap_or_else(|_| block.ends_with(fallback))
}

// ---- Hooks ----

/// `native_use_item_on`: the chassis on a floor rail becomes a worker of the player (X2).
pub(super) fn on_use_item_on(
    game: &mut Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: Pos,
    claimed: &mut bool,
) {
    if *claimed || &*stack.item != content::CHASSIS {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    *claimed = true;
    let floor = game.native_block(pos) == content::MONORAIL
        && game.native_block_property(pos, "mount").as_deref() == Some("down");
    if !floor {
        tell(
            game,
            player.uuid,
            line(
                "rustcraft.worker.place_on_rail",
                "Place the chassis on a floor monorail",
                &[],
            ),
        );
        return;
    }
    if player.game_mode != 1 {
        game.native_take_items(content::CHASSIS, 1);
    }
    state()
        .placing
        .push((player.uuid, player.dimension.clone(), pos));
}

/// `native_interact_entity`: a click on a worker's seat opens its window; the wrench with
/// sneaking takes the worker back (W3).
pub(super) fn on_interact_entity(
    game: &mut Game<'_>,
    entity: i32,
    stack: Option<&ItemStack>,
    claimed: &mut bool,
) {
    let Ok(id) = u32::try_from(entity) else {
        return;
    };
    let found = state().levels.iter().find_map(|(d, l)| {
        l.seats
            .iter()
            .find(|(_, s)| **s == id)
            .map(|(w, _)| (d.clone(), *w))
    });
    let Some((dimension, worker)) = found else {
        return;
    };
    *claimed = true;
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let wrench = stack.is_some_and(|s| &*s.item == content::WRENCH);
    if wrench && player.sneaking {
        pick_up(game, player.uuid, &dimension, worker);
    } else {
        state().opening.push((player.uuid, dimension, worker));
    }
}

/// Takes the worker away and gives the player its chassis, tool, upgrades and load; what does
/// not fit drops at their feet.
fn pick_up(game: &mut Game<'_>, player: u128, dimension: &str, id: u64) {
    let (taken, seat) = {
        let mut s = state();
        let Some(level) = s.levels.get_mut(dimension) else {
            return;
        };
        let Some(i) = level.fleet.workers.iter().position(|w| w.id == id) else {
            return;
        };
        level.dirty = true;
        (level.fleet.workers.remove(i), level.seats.remove(&id))
    };
    if let Some(seat) = seat {
        game.native_remove_mob(dimension, seat);
    }
    let mut stacks = vec![(content::CHASSIS.to_owned(), 1)];
    stacks.extend(taken.tool.map(|t| (t.item().to_owned(), 1)));
    stacks.extend(
        taken
            .upgrades
            .iter()
            .flatten()
            .map(|m| (m.item().to_owned(), 1)),
    );
    stacks.extend(taken.hold.slots.iter().flatten().cloned());
    let at = taken.feet();
    for (item, n) in stacks {
        if let Some(left) = game.native_give_stack(player, ItemStack::new(&item, n)) {
            drop_stack(game, dimension, at, &left.item, left.count);
        }
    }
}

/// An item entity of `count` `item` at `at`.
fn drop_stack(game: &mut Game<'_>, dimension: &str, at: [f64; 3], item: &str, count: u32) {
    if count == 0 {
        return;
    }
    let stack = Tag::compound([
        ("id", Tag::String(JavaString::from(item))),
        ("count", Tag::Int(count.min(99) as i32)),
    ]);
    let nbt = Compound::from([(JavaString::from("Item"), stack)]);
    if game
        .native_summon(
            dimension,
            "minecraft:item",
            [at[0], at[1] + 0.5, at[2]],
            nbt,
        )
        .is_none()
    {
        log::warn!("rustcraft: {count} {item} from a worker were lost");
    }
}

/// `native_menu_clicked`: a click on a worker's window waits for the level's tick.
pub(super) fn on_menu_clicked(uuid: u128, window: i32, slot: i32, mode: i32) {
    let mut s = state();
    if !s.sessions.get(&uuid).is_some_and(|x| x.window == window) || mode > 1 {
        return;
    }
    if let Ok(slot) = usize::try_from(slot) {
        s.clicks.push(Click { uuid, window, slot });
    }
}

pub(super) fn on_menu_closed(uuid: u128, window: i32) {
    let mut s = state();
    if s.sessions.get(&uuid).is_some_and(|x| x.window == window) {
        s.sessions.remove(&uuid);
    }
}

/// `native_server_stopping` and `native_level_unload`: the seats go, the fleet is saved.
pub(super) fn on_stopping(game: &mut Game<'_>) {
    let levels: Vec<(String, LevelFleet)> = state().levels.drain().collect();
    for (dimension, mut level) in levels {
        for (_, seat) in level.seats.drain() {
            game.native_remove_mob(&dimension, seat);
        }
        level.dirty = true;
        save(game, &mut level);
    }
}

pub(super) fn on_level_unload(dimension: &str) {
    state().levels.remove(dimension);
}

// ---- The tick ----

/// One tick of the level's workers: new ones placed, each rides a tick, its tool works where it
/// came, the docks serve it; seats follow, windows answer, the fleet is saved when it changed.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, net_level: &LevelNet, tuning: &Tuning) {
    let mut level = take_level(dimension);
    load(game, &mut level);
    level.ticks += 1;
    let placing: Vec<(u128, Pos)> = {
        let mut s = state();
        let (here, later): (Vec<_>, Vec<_>) = std::mem::take(&mut s.placing)
            .into_iter()
            .partition(|p| p.1 == dimension);
        s.placing = later;
        here.into_iter().map(|(o, _, p)| (o, p)).collect()
    };
    for (owner, rail) in placing {
        level.fleet.add(owner, rail, tuning);
        level.dirty = true;
    }
    let docks = |kind: Machine| -> BTreeSet<Pos> {
        net_level
            .machines
            .iter()
            .filter(|(_, m)| **m == kind)
            .map(|(p, _)| *p)
            .collect()
    };
    let unload = docks(Machine::DockUnload);
    let charge = docks(Machine::DockCharge);
    if level.ticks.is_multiple_of(DOCK_PUSH_EVERY) {
        push_docks(game, dimension, &unload);
    }
    if !level.fleet.workers.is_empty() {
        let net = monorail::network(dimension);
        let mut workers = std::mem::take(&mut level.fleet.workers);
        for w in &mut workers {
            if !chunk_loaded(game, w.cell) {
                // W7: it waits while its chunk is not loaded; its seat went with the chunk.
                level.seats.remove(&w.id);
                continue;
            }
            let before = (w.cell, w.errand, w.hold.used(), w.note);
            ride(game, dimension, &net, &unload, &charge, w, tuning);
            if before != (w.cell, w.errand, w.hold.used(), w.note) {
                level.dirty = true;
            }
            seat(game, dimension, &mut level, w);
        }
        level.fleet.workers = workers;
    }
    if level.ticks.is_multiple_of(BOARD_EVERY) {
        boards(game, &mut level, tuning);
    }
    if level.ticks.is_multiple_of(NOTES_EVERY) {
        notes(game, &level);
    }
    if level.ticks.is_multiple_of(SWEEP_EVERY) {
        sweep(game, dimension, &level);
    }
    windows(game, dimension, &mut level, tuning);
    if level.ticks.is_multiple_of(BOARD_EVERY) {
        save(game, &mut level);
    }
    put_level(dimension, level);
}

fn ride(
    game: &mut Game<'_>,
    dimension: &str,
    net: &Network,
    unload: &BTreeSet<Pos>,
    charge: &BTreeSet<Pos>,
    w: &mut Worker,
    t: &Tuning,
) {
    let track = Track {
        net,
        unload,
        charge,
    };
    match w.tick(&track, t) {
        Step::Arrived(cell) => work(game, dimension, w, cell, t),
        Step::AtDock(dock) => serve(game, w, dock, t),
        Step::Busy | Step::Moved | Step::Stopped => {}
    }
}

/// The tool looks round the cell it came into.
fn work(game: &mut Game<'_>, dimension: &str, w: &mut Worker, cell: Pos, t: &Tuning) {
    match w.tool {
        Some(Tool::Saw) => saw(game, dimension, w, cell, t),
        Some(Tool::Reaper) => reap(game, dimension, w, cell, t),
        None => {}
    }
}

/// Breaks `pos` with its drops into the hold (what does not fit drops there); whether it broke.
fn take_block(
    game: &mut Game<'_>,
    dimension: &str,
    w: &mut Worker,
    pos: Pos,
    tool: Option<&str>,
) -> bool {
    let tool = tool.map(|i| ItemStack::new(i, 1));
    let Some(drops) = game.native_machine_destroy_block(pos, tool.as_ref(), true) else {
        return false;
    };
    for d in drops {
        if d.count == 0 || &*d.item == AIR {
            continue;
        }
        let max = machines::max_stack(game, &d.item);
        let left = w.hold.add(&d.item, d.count, max);
        if left > 0 {
            drop_stack(
                game,
                dimension,
                pos.map(|c| f64::from(c) + 0.5),
                &d.item,
                left,
            );
        }
    }
    true
}

fn report_break(dimension: &str, w: &Worker, pos: Pos, block: String, tool: Tool) {
    base::report(Report::Break {
        owner: w.owner,
        dimension: dimension.to_owned(),
        pos,
        block,
        tool: tool.report_word(),
    });
}

/// Sets `block` at `pos` with `props`.
fn set_block(
    game: &mut Game<'_>,
    dimension: &str,
    pos: Pos,
    block: &str,
    props: &[(String, String)],
) -> bool {
    let Some(state) = game.native_default_state(block) else {
        return false;
    };
    game.native_set_block(dimension, pos, state, 3);
    for (k, v) in props {
        game.native_set_block_property(pos, k, v);
    }
    game.native_block(pos) == block
}

/// The saw: the first tree with a log in the zone, felled top down, planted again.
fn saw(game: &mut Game<'_>, dimension: &str, w: &mut Worker, cell: Pos, t: &Tuning) {
    let zone = w.zone_cells(cell, t);
    let tree = {
        let view = View { game };
        zone.iter()
            .find_map(|&p| trees::find(&view, p, t.saw_max_logs, t.saw_leaf_reach))
    };
    let Some(tree) = tree else {
        return;
    };
    let mut felled = 0;
    for &log in &tree.logs {
        let block = game.native_block(log);
        if take_block(game, dimension, w, log, Some(SAW_TOOL)) {
            felled += 1;
            report_break(dimension, w, log, block, Tool::Saw);
        }
    }
    for &leaf in &tree.leaves {
        take_block(game, dimension, w, leaf, None);
    }
    let wanted = trees::sapling_for(&tree.wood);
    for &root in &tree.roots {
        let sapling = wanted
            .clone()
            .filter(|s| w.hold.count(s) > 0)
            .or_else(|| w.hold.find(trees::is_sapling));
        let Some(sapling) = sapling else {
            break;
        };
        if set_block(game, dimension, root, &sapling, &[]) {
            w.hold.take(&sapling, 1);
        }
    }
    w.spend(
        t.saw_ticks_per_log * f64::from(felled),
        t.saw_energy_per_log * f64::from(felled),
        t,
    );
}

/// The reaper: every ripe plant of the zone by its group's rule (spec 17).
fn reap(game: &mut Game<'_>, dimension: &str, w: &mut Worker, cell: Pos, t: &Tuning) {
    let (_, height) = w.zone(t);
    let top = cell[1] + height - 1;
    let zone = w.zone_cells(cell, t);
    let plans: Vec<plants::Harvest> = {
        let view = View { game };
        zone.iter()
            .filter_map(|&p| plants::plan(&view, p, top))
            .collect()
    };
    let mut done = 0usize;
    for h in plans {
        for &p in &h.breaks {
            let block = game.native_block(p);
            if take_block(game, dimension, w, p, None) {
                report_break(dimension, w, p, block, Tool::Reaper);
            }
        }
        for pick in &h.picks {
            game.native_set_block_property(pick.pos, &pick.set.0, &pick.set.1);
            let max = machines::max_stack(game, &pick.item);
            let left = w.hold.add(&pick.item, pick.count, max);
            if left > 0 {
                drop_stack(
                    game,
                    dimension,
                    pick.pos.map(|c| f64::from(c) + 0.5),
                    &pick.item,
                    left,
                );
            }
        }
        if let Some(r) = &h.replant {
            if w.hold.count(&r.seed) > 0 && set_block(game, dimension, r.pos, &r.block, &r.props) {
                w.hold.take(&r.seed, 1);
            } else {
                // H3: the cell stays empty.
                w.note = Note::NothingToSow;
            }
        }
        done += h.size();
    }
    if done > 0 {
        w.spend(t.reap_ticks * done as f64, t.reap_energy * done as f64, t);
    }
}

/// A dock serves the worker standing at it: the unload dock takes its load, the charge dock
/// fills its battery.
fn serve(game: &mut Game<'_>, w: &mut Worker, dock: Pos, t: &Tuning) {
    match w.errand {
        Errand::Unload => unload_into(game, w, dock, t),
        Errand::Charge => charge_from(game, w, dock, t),
        Errand::Work => {}
    }
}

fn unload_into(game: &mut Game<'_>, w: &mut Worker, dock: Pos, t: &Tuning) {
    let per_tick = (t.dock_items_per_second / t.ticks_per_second)
        .ceil()
        .max(1.0) as u32;
    let tool = w.tool;
    let keep = |i: &str| tool.is_some_and(|t| t.keeps(i));
    let mut moved = 0;
    while moved < per_tick {
        let Some(slot) = w.hold.next_to_unload(keep) else {
            break;
        };
        let Some((item, count)) = w.hold.slots[slot].clone() else {
            break;
        };
        let n = count.min(per_tick - moved);
        let max = machines::max_stack(game, &item);
        let put = game
            .native_with_mod_block_entity(dock, |e| into_slots(&mut e.items, &item, n, max))
            .unwrap_or(0);
        if put == 0 {
            break;
        }
        w.hold.take(&item, put);
        moved += put;
    }
    if w.unloaded() {
        w.back_to_work();
    } else if moved == 0 {
        w.note = Note::DockFull;
    } else {
        w.note = Note::Unloading;
    }
}

/// Puts up to `count` `item` into the dock's slots; how many went in.
fn into_slots(items: &mut Vec<Option<ItemStack>>, item: &str, count: u32, max: u32) -> u32 {
    if items.len() < content::DOCK_SLOTS {
        items.resize(content::DOCK_SLOTS, None);
    }
    let mut left = count;
    for s in items.iter_mut().flatten() {
        if left == 0 {
            break;
        }
        if &*s.item == item && s.count < max {
            let k = (max - s.count).min(left);
            s.count += k;
            left -= k;
        }
    }
    for s in items.iter_mut() {
        if left == 0 {
            break;
        }
        let empty = s.as_ref().is_none_or(|x| x.count == 0 || &*x.item == AIR);
        if empty {
            let k = max.min(left);
            *s = Some(ItemStack::new(item, k));
            left -= k;
        }
    }
    count - left
}

fn charge_from(game: &mut Game<'_>, w: &mut Worker, dock: Pos, t: &Tuning) {
    let cap = w.capacity(t);
    let buffer = machines::buffer_energy(game, dock);
    let give = (t.dock_charge_rate / t.ticks_per_second)
        .min(buffer)
        .min((cap - w.charge).max(0.0));
    if give > 0.0 {
        machines::set_buffer_energy(game, dock, buffer - give);
        w.charge += give;
        w.note = Note::Charging;
    } else if w.charge < cap {
        w.note = Note::DockEmpty;
    }
    if w.charge >= cap - 1e-6 {
        w.charge = cap;
        w.back_to_work();
    }
}

/// Each unload dock hands its load on to a container beside it (W6: «или соседний сундук»); the
/// magistral takes it like any machine's products.
fn push_docks(game: &mut Game<'_>, dimension: &str, docks: &BTreeSet<Pos>) {
    for &dock in docks {
        for f in Face::ALL {
            let n = step(dock, f);
            let block = game.native_block(n);
            if block == AIR || server::is_ours(&block) {
                continue;
            }
            game.native_move_items(
                dimension,
                dock,
                transport::direction(f),
                n,
                transport::direction(f.opposite()),
                None,
                DOCK_PUSH,
            );
        }
    }
}

// ---- Showing it ----

/// The seat follows the worker; one is spawned for a worker without one.
fn seat(game: &mut Game<'_>, dimension: &str, level: &mut LevelFleet, w: &Worker) {
    let at = w.feet();
    if let Some(&s) = level.seats.get(&w.id) {
        if game.native_teleport_mob(dimension, s, at) {
            return;
        }
        level.seats.remove(&w.id);
    }
    let options = NativeSpawn {
        no_ai: true,
        silent: true,
        invulnerable: true,
        no_gravity: true,
        ..NativeSpawn::default()
    };
    let Some(s) = game.native_spawn_mob(dimension, SEAT_KIND, at, &options) else {
        return;
    };
    if let Some(e) = game.native_entity(s) {
        game.native_entity_add_tag(e, SEAT_TAG);
    }
    level.seats.insert(w.id, s);
    level.boards.remove(&w.id);
}

/// The board over each worker (W5): its tool, charge, hold and state, as its seat's name.
fn boards(game: &mut Game<'_>, level: &mut LevelFleet, t: &Tuning) {
    for w in &level.fleet.workers {
        let Some(&s) = level.seats.get(&w.id) else {
            continue;
        };
        let board = board(w, t);
        let text = board.component().to_string();
        if level.boards.get(&w.id) == Some(&text) {
            continue;
        }
        let Some(e) = game.native_entity(s) else {
            continue;
        };
        let nbt = Compound::from([
            (
                JavaString::from("CustomName"),
                horizon_client::lan::data::component_nbt(&board.component()),
            ),
            (JavaString::from("CustomNameVisible"), Tag::Byte(1)),
        ]);
        if game.native_entity_merge_nbt(e, nbt) {
            level.boards.insert(w.id, text);
        }
    }
}

fn board(w: &Worker, t: &Tuning) -> Text {
    let percent = (100.0 * w.charge / w.capacity(t).max(1.0)).round();
    let mut text = tool_name(w.tool);
    text.push(Text::literal(format!(
        " · {percent}% · {}/{} · ",
        w.hold.used(),
        w.hold.slots.len()
    )));
    text.push(note_text(w.note));
    text
}

fn tool_name(tool: Option<Tool>) -> Text {
    match tool {
        Some(Tool::Saw) => line("rustcraft.worker.saw", "Lumberjack", &[]),
        Some(Tool::Reaper) => line("rustcraft.worker.reaper", "Reaper", &[]),
        None => line("rustcraft.worker.bare", "Chassis", &[]),
    }
}

pub(super) fn note_text(note: Note) -> Text {
    let (key, fallback) = match note {
        Note::Working => ("rustcraft.worker.working", "Working"),
        Note::Unloading => ("rustcraft.worker.unloading", "Unloading"),
        Note::Charging => ("rustcraft.worker.charging", "Charging"),
        Note::ToUnload => ("rustcraft.worker.to_unload", "Going to unload"),
        Note::ToCharge => ("rustcraft.worker.to_charge", "Going to charge"),
        Note::NoTool => ("rustcraft.worker.no_tool", "No tool"),
        Note::NoUnloadDock => ("rustcraft.worker.no_unload", "No unload dock on the way"),
        Note::NoChargeDock => ("rustcraft.worker.no_charge", "No charge dock on the way"),
        Note::Flat => ("rustcraft.worker.flat", "Battery empty"),
        Note::NothingToSow => ("rustcraft.worker.nothing_to_sow", "Nothing to sow"),
        Note::DockFull => (
            "rustcraft.worker.dock_full",
            "Waiting: the unload dock is full",
        ),
        Note::DockEmpty => (
            "rustcraft.worker.dock_empty",
            "Waiting: the charge dock has no energy",
        ),
        Note::NoWay => ("rustcraft.worker.no_way", "No rail to ride"),
    };
    line(key, fallback, &[])
}

/// Every client is told each seat, its heading and its tool (rustcraft_station::on_net).
fn notes(game: &mut Game<'_>, level: &LevelFleet) {
    let mut bytes = Vec::new();
    for w in &level.fleet.workers {
        let Some(&s) = level.seats.get(&w.id) else {
            continue;
        };
        bytes.extend(s.to_le_bytes());
        bytes.extend(w.yaw().unwrap_or(-90.0).to_le_bytes());
        bytes.push(match w.tool {
            None => 0,
            Some(Tool::Saw) => 1,
            Some(Tool::Reaper) => 2,
        });
    }
    if bytes.is_empty() {
        return;
    }
    for p in game.native_server_players() {
        let _ = game.native_net_send_to_player(p.uuid, NOTES, &bytes);
    }
}

/// Seats of no worker (saved with their chunk before a stop, or of a worker taken away) go.
fn sweep(game: &mut Game<'_>, dimension: &str, level: &LevelFleet) {
    let ours: BTreeSet<u32> = level.seats.values().copied().collect();
    let pigs: Vec<u32> = game
        .mobs
        .iter()
        .filter(|m| &*m.kind.id == SEAT_KIND && !ours.contains(&m.id))
        .map(|m| m.id)
        .collect();
    for id in pigs {
        let tagged = game
            .native_entity(id)
            .and_then(|e| game.native_entity_tags(e))
            .is_some_and(|tags| tags.iter().any(|t| t == SEAT_TAG));
        if tagged {
            game.native_remove_mob(dimension, id);
        }
    }
}

// ---- The window ----

fn windows(game: &mut Game<'_>, dimension: &str, level: &mut LevelFleet, t: &Tuning) {
    let (opening, clicks) = {
        let mut s = state();
        let (here, later): (Vec<_>, Vec<_>) = std::mem::take(&mut s.opening)
            .into_iter()
            .partition(|o| o.1 == dimension);
        s.opening = later;
        let mine: Vec<u128> = s
            .sessions
            .iter()
            .filter(|(_, x)| x.dimension == dimension)
            .map(|(u, _)| *u)
            .collect();
        let (clicks, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.clicks)
            .into_iter()
            .partition(|c| mine.contains(&c.uuid));
        s.clicks = rest;
        (here, clicks)
    };
    for (uuid, _, id) in opening {
        let Some(w) = level.fleet.workers.iter().find(|w| w.id == id) else {
            continue;
        };
        let items = cells(game, w, t);
        let title = line(
            "rustcraft.worker.window",
            "Worker chassis · sparksteel",
            &[],
        );
        if let Some(window) = game.native_open_menu(uuid, ROWS, title, items) {
            state().sessions.insert(
                uuid,
                Session {
                    dimension: dimension.to_owned(),
                    worker: id,
                    window,
                },
            );
        }
    }
    for c in clicks {
        let id = {
            let s = state();
            match s.sessions.get(&c.uuid) {
                Some(x) if x.window == c.window => x.worker,
                _ => continue,
            }
        };
        let Some(w) = level.fleet.workers.iter_mut().find(|w| w.id == id) else {
            game.native_close_menu(c.uuid);
            continue;
        };
        if click(game, c.uuid, c.slot, w, t) {
            level.dirty = true;
        }
        let items = cells(game, w, t);
        game.native_set_menu_items(c.uuid, items);
    }
    // Open windows follow their worker each second.
    if level.ticks.is_multiple_of(BOARD_EVERY) {
        let open: Vec<(u128, u64)> = state()
            .sessions
            .iter()
            .filter(|(_, x)| x.dimension == dimension)
            .map(|(u, x)| (*u, x.worker))
            .collect();
        for (uuid, id) in open {
            if let Some(w) = level.fleet.workers.iter().find(|w| w.id == id) {
                let items = cells(game, w, t);
                game.native_set_menu_items(uuid, items);
            } else {
                game.native_close_menu(uuid);
            }
        }
    }
}

fn give(game: &mut Game<'_>, uuid: u128, item: &str, count: u32) -> u32 {
    match game.native_give_stack(uuid, ItemStack::new(item, count)) {
        Some(left) => count - left.count.min(count),
        None => count,
    }
}

/// A click: on the tool, an upgrade or the hold it goes to the player; on an item of the
/// player's inventory below, that item goes in (a tool to the tool slot, an upgrade to a free
/// slot, anything else to the hold). Whether the worker changed.
fn click(game: &mut Game<'_>, uuid: u128, slot: usize, w: &mut Worker, t: &Tuning) -> bool {
    if slot == TOOL_CELL {
        let Some(tool) = w.tool else { return false };
        if give(game, uuid, tool.item(), 1) == 1 {
            w.tool = None;
            return true;
        }
        return false;
    }
    if UPGRADE_CELLS.contains(&slot) {
        let i = slot - UPGRADE_CELLS.start;
        let Some(m) = w.upgrades.get(i).copied().flatten() else {
            return false;
        };
        if give(game, uuid, m.item(), 1) == 1 {
            w.upgrades[i] = None;
            for (item, n) in w.refit(t) {
                give(game, uuid, &item, n);
            }
            return true;
        }
        return false;
    }
    if (HOLD_FIRST..CELLS).contains(&slot) {
        let i = slot - HOLD_FIRST;
        let Some((item, n)) = w.hold.slots.get(i).cloned().flatten() else {
            return false;
        };
        let gave = give(game, uuid, &item, n);
        w.hold.take(&item, gave);
        return gave > 0;
    }
    let Some(n) = slot.checked_sub(CELLS) else {
        return false;
    };
    let inventory = match n {
        0..MAIN_SLOTS => n + 9,
        MAIN_SLOTS..INVENTORY_SLOTS => n - MAIN_SLOTS,
        _ => return false,
    };
    let Some(stack) = game
        .native_player_stacks(uuid)
        .and_then(|s| s.get(inventory).cloned().flatten())
    else {
        return false;
    };
    let item = stack.item.to_string();
    let (keep, back): (u32, Option<String>) = if let Some(tool) = Tool::of(&item) {
        let old = w.tool.replace(tool);
        (1, old.map(|o| o.item().to_owned()))
    } else if let Some(m) = Module::of(&item).filter(|m| ModuleHolder::Worker.takes(*m)) {
        let Some(free) = w.upgrades.iter().position(Option::is_none) else {
            return false;
        };
        w.upgrades[free] = Some(m);
        for (i, n) in w.refit(t) {
            give(game, uuid, &i, n);
        }
        (1, None)
    } else {
        let max = machines::max_stack(game, &item);
        let left = w.hold.add(&item, stack.count, max);
        (stack.count - left, None)
    };
    if keep == 0 {
        return false;
    }
    let rest = (stack.count > keep).then(|| {
        let mut s = stack.clone();
        s.count -= keep;
        s
    });
    let taken = game.native_set_inventory_slot(
        uuid,
        NativeInventoryKind::Player,
        inventory,
        Some(&stack),
        rest,
    );
    if taken.is_err() {
        log::warn!("rustcraft: the worker took an item the player no longer had");
    }
    if let Some(old) = back {
        give(game, uuid, &old, 1);
    }
    if w.errand == Errand::Work && w.tool.is_some() && w.note == Note::NoTool {
        w.note = Note::Working;
    }
    true
}

/// The window's cells (guide `chassis`): tool, three upgrade slots, charge, hold, state, then
/// the hold's slots; slots past the hold's size are grey.
fn cells(game: &Game<'_>, w: &Worker, t: &Tuning) -> Vec<Option<ItemStack>> {
    let _ = game;
    let mut out: Vec<Option<ItemStack>> = vec![None; CELLS];
    out[TOOL_CELL] = Some(match w.tool {
        Some(tool) => named(
            tool.item(),
            1,
            tool_name(Some(tool)),
            &[hint(
                "rustcraft.worker.tool_hint",
                "Click: take the tool back",
            )],
            false,
        ),
        None => named(
            "minecraft:light_gray_stained_glass_pane",
            1,
            line("rustcraft.worker.tool_slot", "Tool", &[]),
            &[hint(
                "rustcraft.worker.tool_put",
                "Click a saw or a reaper in your inventory",
            )],
            false,
        ),
    });
    for (k, cell) in UPGRADE_CELLS.enumerate() {
        out[cell] = Some(match w.upgrades.get(k).copied().flatten() {
            Some(m) => named(
                m.item(),
                1,
                Text::translate(&format!("item.{}", m.item().replace(':', ".")), &[]),
                &[hint("rustcraft.worker.upgrade_hint", "Click: take it back")],
                false,
            ),
            None => named(
                "minecraft:gray_stained_glass_pane",
                1,
                line("rustcraft.worker.upgrade_slot", "Upgrade slot", &[]),
                &[hint(
                    "rustcraft.worker.upgrade_put",
                    "Battery, speed, storage or radius",
                )],
                false,
            ),
        });
    }
    let cap = w.capacity(t);
    let percent = (100.0 * w.charge / cap.max(1.0)).round();
    out[CHARGE_CELL] = Some(named(
        "minecraft:redstone",
        1,
        line(
            "rustcraft.worker.charge",
            "Charge: %s",
            &[Text::literal(format!("{percent}%"))],
        ),
        &[
            Text::literal(format!("{:.0} / {:.0} E", w.charge, cap)),
            hint(
                "rustcraft.worker.charge_hint",
                "Under 20% it goes to the charge dock",
            ),
        ],
        false,
    ));
    out[HOLD_CELL] = Some(named(
        "minecraft:chest",
        1,
        line(
            "rustcraft.worker.hold",
            "Hold: %s of %s slots",
            &[
                Text::literal(w.hold.used().to_string()),
                Text::literal(w.hold.slots.len().to_string()),
            ],
        ),
        &[hint(
            "rustcraft.worker.hold_hint",
            "A full hold goes to the unload dock",
        )],
        false,
    ));
    let bad = !matches!(
        w.note,
        Note::Working | Note::Unloading | Note::Charging | Note::ToUnload | Note::ToCharge
    );
    out[NOTE_CELL] = Some(named(
        if bad {
            "minecraft:red_dye"
        } else {
            "minecraft:lime_dye"
        },
        1,
        note_text(w.note),
        &[],
        false,
    ));
    for i in 0..(CELLS - HOLD_FIRST) {
        out[HOLD_FIRST + i] = match w.hold.slots.get(i) {
            Some(Some((item, n))) => Some(ItemStack::new(item, *n)),
            Some(None) => None,
            None => Some(named(
                "minecraft:black_stained_glass_pane",
                1,
                line(
                    "rustcraft.worker.no_slot",
                    "More slots with a storage module",
                    &[],
                ),
                &[],
                false,
            )),
        };
    }
    out
}

fn hint(key: &str, fallback: &str) -> Text {
    line(key, fallback, &[])
}

// ---- For the self-test ----

/// A worker of `owner` on rail `cell` with `tool` and `hold`, placed at once; its id.
pub(super) fn spawn_for_selftest(
    game: &mut Game<'_>,
    dimension: &str,
    cell: Pos,
    owner: u128,
    tool: Option<Tool>,
    hold: &[(&str, u32)],
    charge: Option<f64>,
    t: &Tuning,
) -> u64 {
    let mut level = take_level(dimension);
    load(game, &mut level);
    let id = level.fleet.add(owner, cell, t);
    if let Some(w) = level.fleet.workers.iter_mut().find(|w| w.id == id) {
        w.tool = tool;
        for (item, n) in hold {
            let max = machines::max_stack(game, item);
            w.hold.add(item, *n, max);
        }
        if let Some(c) = charge {
            w.charge = c;
        }
    }
    level.dirty = true;
    put_level(dimension, level);
    id
}

/// A worker as the server holds it now.
pub(super) fn worker_for_selftest(dimension: &str, id: u64) -> Option<Worker> {
    state()
        .levels
        .get(dimension)?
        .fleet
        .workers
        .iter()
        .find(|w| w.id == id)
        .cloned()
}

/// Takes the worker away, its seat with it.
pub(super) fn remove_for_selftest(game: &mut Game<'_>, dimension: &str, id: u64) {
    let seat = {
        let mut s = state();
        let Some(level) = s.levels.get_mut(dimension) else {
            return;
        };
        level.fleet.workers.retain(|w| w.id != id);
        level.dirty = true;
        level.seats.remove(&id)
    };
    if let Some(seat) = seat {
        game.native_remove_mob(dimension, seat);
    }
}

/// The fleet through its saved form and back.
pub(super) fn round_trip(dimension: &str) -> Result<(), String> {
    let s = state();
    let Some(level) = s.levels.get(dimension) else {
        return Err("no workers in this level".into());
    };
    let text = level.fleet.encode();
    let back = encode_saved(&text)
        .and_then(|c| decode_saved(&c))
        .map_err(|e| format!("{e:?}"))?;
    match Fleet::decode(&back) {
        Some(f) if f == level.fleet => Ok(()),
        other => Err(format!("the fleet came back as {other:?}")),
    }
}

/// What the worker's dock shows (A4).
pub(super) fn dock_text(game: &Game<'_>, dimension: &str, pos: Pos, machine: Machine) -> Text {
    let _ = dimension;
    match machine {
        Machine::DockCharge => {
            let e = machines::buffer_energy(game, pos);
            if e <= 0.0 {
                line("rustcraft.machine.no_energy", "No energy", &[])
            } else {
                line(
                    "rustcraft.dock.charge",
                    "Charge dock · %s E",
                    &[Text::literal(format!("{e:.0}"))],
                )
            }
        }
        _ => {
            let full = game.native_mod_block_entity(pos).is_some_and(|e| {
                e.items.len() >= content::DOCK_SLOTS
                    && e.items
                        .iter()
                        .all(|s| s.as_ref().is_some_and(|x| x.count > 0))
            });
            if full {
                line("rustcraft.joint.receiver_full", "Receiver full", &[])
            } else {
                line(
                    "rustcraft.dock.unload",
                    "Unload dock · waits for a worker",
                    &[],
                )
            }
        }
    }
}

/// The self-test's plot cleared: its workers go, their seats with them.
pub(super) fn clear_area(game: &mut Game<'_>, dimension: &str, min: Pos, max: Pos) {
    let inside = |p: Pos| (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
    let ids: Vec<u64> = state()
        .levels
        .get(dimension)
        .map(|l| {
            l.fleet
                .workers
                .iter()
                .filter(|w| inside(w.cell))
                .map(|w| w.id)
                .collect()
        })
        .unwrap_or_default();
    for id in ids {
        remove_for_selftest(game, dimension, id);
    }
}
