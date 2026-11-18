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
// draws the guide's worker round it (rustcraft_station::draw_worker, `native_mob_submit`) and, since
// 0.18.5, the board over it (W5: charge, hold and state) from the same note, in its own language
// (rustcraft_station::worker_boards; a translated name on the seat was read as no name at all).
//
// Hands: the chassis clicked on a floor rail places a worker (`native_use_item_on`); a click on the
// worker opens its window, a server menu like the joint window (rustcraft_window), and the wrench
// with sneaking takes it back with its tool, upgrades and load (`native_interact_entity`, API 29).
//
// The hold keeps plain item names. ST4: the grab takes a suspicious block the living vein grew
// whole, under a marked name (rustcraft_rules::utility::grown::hold_name); wherever a hold's name
// becomes a stack again (the dock, the player's hand, the ground) it is rustcraft_utility's
// `hold_stack`, which gives the grown block its custom data `rustcraft:grown` (ST5).
use crate::game::{
    Game,
    native_inventories::NativeInventoryKind,
    native_mobs::NativeSpawn,
    rustcraft_base as base,
    rustcraft_content::{self as content, Machine},
    rustcraft_machines as machines, rustcraft_monorail as monorail,
    rustcraft_server::{self as server, LevelNet, line, tell},
    rustcraft_transport as transport, rustcraft_utility as util,
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
    utility::grown,
    worker::{
        BlockView, Errand, Fleet, Note, Step, Tool, Track, Worker, chassis, digging, plants, trees,
        screen::{self, Press},
    },
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
/// How often clients are told the seats and their boards again, open windows follow, the fleet is
/// saved, and lost seats are swept.
const NOTES_EVERY: u64 = 10;
const BOARD_EVERY: u64 = 20;
const SWEEP_EVERY: u64 = 100;
/// The unload dock hands its load to a container beside it this often, this many at a time.
const DOCK_PUSH_EVERY: u64 = 8;
const DOCK_PUSH: u32 = 8;
/// Wave 2: the session of a chassis window on the mod's own screen (no menu, no menu id).
const SCREEN_WINDOW: i32 = -1;
const CELLS: usize = screen::INVENTORY_FROM;
const MAIN_SLOTS: usize = 27;
const INVENTORY_SLOTS: usize = 36;
/// The window's first row: tool, the sockets (up to 7, U4), then its state with charge and hold.
const TOOL_CELL: usize = 0;
const UPGRADE_CELLS: std::ops::Range<usize> = 1..1 + chassis::MAX_SOCKETS;
const NOTE_CELL: usize = 8;
const HOLD_FIRST: usize = 9;
/// What the saw's loot is rolled for: an axe, as a player's would be.
const SAW_TOOL: &str = "minecraft:iron_axe";
/// What the reaper tills grass, dirt and dirt paths to (0.18.5).
const FARMLAND: &str = "minecraft:farmland";
/// Rail cells in a row with nothing to do before a tool says so: the scoop's «нечего копать»,
/// 0.18.5 the reaper's, the saw's and the grab's.
const IDLE_CELLS: u32 = 16;

// ---- State ----

#[derive(Default)]
struct LevelFleet {
    fleet: Fleet,
    loaded: bool,
    dirty: bool,
    /// Worker id → its seat mob.
    seats: HashMap<u64, u32>,
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
    /// A shift-click (mode 1): an item of the inventory goes into the filter (H6).
    shift: bool,
}

#[derive(Default)]
struct Workers {
    levels: HashMap<String, LevelFleet>,
    sessions: HashMap<u128, Session>,
    opening: Vec<(u128, String, u64)>,
    clicks: Vec<Click>,
    /// Wave 2: presses in the chassis windows (screen::Press), for the level's tick.
    presses: Vec<(u128, Press)>,
    /// Chassis placed, to become workers once the level ticks: owner, dimension, rail, and the
    /// worker a picked-up chassis carries (W3).
    placing: Vec<(u128, String, Pos, Option<String>, u8)>,
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
            crate::game::rustcraft_debug::journal_warn!("rustcraft: the workers do not read; starting with none");
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
        crate::game::rustcraft_debug::journal_warn!("rustcraft: the workers not saved: {e:?}");
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

/// `native_use_item_on`: the chassis on a monorail (any mount, slice 18) becomes a worker of the
/// player (X2).
pub(super) fn on_use_item_on(
    game: &mut Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: Pos,
    claimed: &mut bool,
) {
    if *claimed {
        return;
    }
    let Some(tier) = chassis::tier_of(&stack.item) else {
        return;
    };
    let Some(player) = crate::game::rustcraft_server::acting(game) else {
        return;
    };
    *claimed = true;
    if game.native_block(pos) != content::MONORAIL {
        tell(
            game,
            player.uuid,
            line(
                "rustcraft.worker.place_on_rail",
                "Place the chassis on a monorail",
                &[],
            ),
        );
        return;
    }
    // W8: the server's limit of workers per player.
    let limit = config();
    if !limit.may_add(owned(player.uuid)) {
        tell(
            game,
            player.uuid,
            line(
                "rustcraft.worker.limit",
                "Worker limit reached: %s per player",
                &[Text::literal(limit.workers_per_player.to_string())],
            ),
        );
        return;
    }
    let packed = packed_of(stack);
    // Wave 2 (W3): a packed worker that does not read back stays in the hand with all it carries.
    if packed.as_deref().is_some_and(|k| !Worker::reads_back(k)) {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: a packed worker that does not read: left with the player");
        tell(
            game,
            player.uuid,
            line("rustcraft.worker.unreadable", "This worker does not read: it stays with you", &[]),
        );
        return;
    }
    if player.game_mode != 1 {
        if packed.is_some() {
            take_exact(game, player.uuid, stack);
        } else {
            game.native_take_items(&stack.item, 1);
        }
    }
    state()
        .placing
        .push((player.uuid, player.dimension.clone(), pos, packed, tier));
}

// ---- The server settings file (W8, API 397) ----

static CONFIG: Mutex<Option<rustcraft_rules::worker::config::ServerConfig>> = Mutex::new(None);

/// The server's settings as last read; the defaults before the file is read.
fn config() -> rustcraft_rules::worker::config::ServerConfig {
    CONFIG
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .unwrap_or_default()
}

/// `native_register_configs`: RustCraft's server settings, `config/rustcraft-server.toml`.
pub(super) fn register_configs(configs: &mut Vec<crate::game::NativeConfig>) {
    use rustcraft_rules::worker::config;
    configs.push(crate::game::NativeConfig::new(
        "rustcraft",
        crate::game::NativeConfigKind::Server,
        config::DEFAULT_TEXT,
    ));
}

/// `native_config_loading` and `_reloading`: RustCraft's server settings read again.
pub(super) fn config_read(file: &crate::game::NativeConfigFile) {
    use rustcraft_rules::worker::config;
    if file.mod_id != "rustcraft" || file.kind != crate::game::NativeConfigKind::Server {
        return;
    }
    let (read, bad) = config::parse(&file.text);
    for key in bad {
        crate::game::rustcraft_debug::journal_warn!(
            "rustcraft: {} does not read its `{key}`; it stays as it was by default",
            file.path.display()
        );
    }
    log::info!(
        "rustcraft: server settings from {}: {} workers a player",
        file.path.display(),
        read.workers_per_player
    );
    *CONFIG.lock().unwrap_or_else(PoisonError::into_inner) = Some(read);
}

/// W8: the workers `owner` has on the server, those being placed included.
fn owned(owner: u128) -> usize {
    let s = state();
    s.levels.values().map(|l| l.fleet.owned_by(owner)).sum::<usize>()
        + s.placing.iter().filter(|p| p.0 == owner).count()
}

/// W3: custom data under which a chassis item carries the worker taken away with it.
const PACKED: &str = "rustcraft:worker";

/// The worker a chassis stack carries, packed (rustcraft_rules::worker::Worker::packed).
fn packed_of(stack: &ItemStack) -> Option<String> {
    match stack.custom_data()?.get(&JavaString::from(PACKED))? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

/// The chassis item carrying worker `w` with all it has (W3).
fn worker_stack(w: &Worker) -> ItemStack {
    let mut stack = ItemStack::new(chassis::item(w.tier), 1);
    let mut data = Compound::new();
    data.insert(
        JavaString::from(PACKED),
        Tag::String(JavaString::from(w.packed().as_str())),
    );
    stack
        .data_mut()
        .set_other(util::CUSTOM_DATA, Some(Tag::Compound(data)));
    stack
}

/// Takes the one stack like `stack` (same item and data) from the player's inventory, the hand
/// they hold first.
fn take_exact(game: &mut Game<'_>, uuid: u128, stack: &ItemStack) {
    let Some(stacks) = game.native_player_stacks(uuid) else {
        return;
    };
    let selected = game.selected;
    let slot = std::iter::once(selected)
        .chain(0..stacks.len())
        .find(|&i| stacks.get(i).cloned().flatten().is_some_and(|s| s == *stack));
    let Some(slot) = slot else {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: the placed chassis was not found in the inventory");
        return;
    };
    let rest = (stack.count > 1).then(|| {
        let mut r = stack.clone();
        r.count -= 1;
        r
    });
    if game
        .native_set_inventory_slot(uuid, NativeInventoryKind::Player, slot, Some(stack), rest)
        .is_err()
    {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: the placed chassis could not be taken");
    }
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
    let Some(player) = crate::game::rustcraft_server::acting(game) else {
        return;
    };
    let wrench = stack.is_some_and(|s| &*s.item == content::WRENCH);
    let new_chassis =
        stack.filter(|s| chassis::tier_of(&s.item).is_some() && packed_of(s).is_none());
    if wrench && player.sneaking {
        pick_up(game, player.uuid, &dimension, worker);
    } else if let Some(new) = new_chassis {
        swap_chassis(game, &player, &dimension, worker, new);
    } else {
        state().opening.push((player.uuid, dimension, worker));
    }
}

/// U4: a higher chassis clicked on a worker takes its old chassis's place: tool, upgrades and
/// hold stay, the old chassis goes to the player.
fn swap_chassis(
    game: &mut Game<'_>,
    player: &horizon_client::mod_api::NativeServerPlayer,
    dimension: &str,
    id: u64,
    new: &ItemStack,
) {
    let t = server::tuning();
    let done = {
        let mut s = state();
        let Some(level) = s.levels.get_mut(dimension) else {
            return;
        };
        let Some(w) = level.fleet.workers.iter_mut().find(|w| w.id == id) else {
            return;
        };
        let done = chassis::swap(w, &new.item, t).map(|old| (old, chassis::sockets(w.tier)));
        if done.is_ok() {
            level.dirty = true;
        }
        done
    };
    match done {
        Ok((old, sockets)) => {
            if player.game_mode != 1 {
                game.native_take_items(&new.item, 1);
            }
            if let Some(left) = game.native_give_stack(player.uuid, ItemStack::new(old, 1)) {
                drop_item(game, dimension, player.position, &left);
            }
            tell(
                game,
                player.uuid,
                line(
                    "rustcraft.worker.swapped",
                    "Chassis replaced: %s sockets, tool and load kept",
                    &[Text::literal(sockets.to_string())],
                ),
            );
        }
        Err(_) => tell(
            game,
            player.uuid,
            line(
                "rustcraft.worker.not_higher",
                "That chassis has no more sockets than this one",
                &[],
            ),
        ),
    }
}

/// W3: takes the worker away and gives the player its chassis as one item carrying its tool,
/// upgrades, load and charge; placed again it is the same worker. With no room it drops at their
/// feet.
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
    let stack = worker_stack(&taken);
    if let Some(left) = game.native_give_stack(player, stack) {
        drop_item(game, dimension, taken.feet(), &left);
    }
}

/// An item entity of `stack`, its data kept, at `at`.
fn drop_item(game: &mut Game<'_>, dimension: &str, at: [f64; 3], stack: &ItemStack) {
    let mut fields = vec![
        ("id", Tag::String(JavaString::from(&*stack.item))),
        ("count", Tag::Int(stack.count.min(99) as i32)),
    ];
    if let Some(data) = stack.custom_data() {
        let components = Compound::from([(
            JavaString::from(util::CUSTOM_DATA),
            Tag::Compound(data.clone()),
        )]);
        fields.push(("components", Tag::Compound(components)));
    }
    let nbt = Compound::from([(JavaString::from("Item"), Tag::compound(fields))]);
    if game
        .native_summon(dimension, "minecraft:item", [at[0], at[1] + 0.5, at[2]], nbt)
        .is_none()
    {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: a picked-up worker could not be dropped and was lost");
    }
}

/// An item entity of `count` `item` (a hold's name: a grown block keeps its mark) at `at`.
fn drop_stack(game: &mut Game<'_>, dimension: &str, at: [f64; 3], item: &str, count: u32) {
    if count == 0 {
        return;
    }
    let (id, marked) = grown::from_hold(item);
    let mut fields = vec![
        ("id", Tag::String(JavaString::from(id))),
        ("count", Tag::Int(count.min(99) as i32)),
    ];
    if marked {
        let mark = Compound::from([(JavaString::from(grown::MARK), Tag::Byte(1))]);
        let components =
            Compound::from([(JavaString::from(util::CUSTOM_DATA), Tag::Compound(mark))]);
        fields.push(("components", Tag::Compound(components)));
    }
    let stack = Tag::compound(fields);
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
        crate::game::rustcraft_debug::journal_warn!("rustcraft: {count} {item} from a worker were lost");
    }
}

/// `native_menu_clicked`: a click on a worker's window waits for the level's tick.
pub(super) fn on_menu_clicked(uuid: u128, window: i32, slot: i32, mode: i32) {
    let mut s = state();
    if !s.sessions.get(&uuid).is_some_and(|x| x.window == window) || mode > 1 {
        return;
    }
    if let Ok(slot) = usize::try_from(slot) {
        s.clicks.push(Click {
            uuid,
            window,
            slot,
            shift: mode == 1,
        });
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
    let placing: Vec<(u128, Pos, Option<String>, u8)> = {
        let mut s = state();
        let (here, later): (Vec<_>, Vec<_>) = std::mem::take(&mut s.placing)
            .into_iter()
            .partition(|p| p.1 == dimension);
        s.placing = later;
        here.into_iter().map(|(o, _, p, k, tier)| (o, p, k, tier)).collect()
    };
    for (owner, rail, packed, tier) in placing {
        let back = packed.and_then(|k| level.fleet.put_back(owner, rail, &k, tuning));
        if back.is_none() {
            let id = level.fleet.add(owner, rail, tuning);
            if let Some(w) = level.fleet.workers.iter_mut().find(|w| w.id == id) {
                w.set_tier(tier, tuning);
            }
        }
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
    // PD2 (slice 18): a portal dock takes a cargo worker's load as an unload dock does, and hands
    // what comes from its pair to a container beside it.
    let portal = docks(Machine::Util(rustcraft_rules::utility::Util::PortalDock));
    let mut unload = docks(Machine::DockUnload);
    let charge = docks(Machine::DockCharge);
    let load = docks(Machine::DockLoad);
    if level.ticks.is_multiple_of(DOCK_PUSH_EVERY) {
        push_docks(game, dimension, &unload);
        push_docks(game, dimension, &portal);
        pull_into_load_docks(game, dimension, &load);
    }
    unload.extend(portal.iter().copied());
    if !level.fleet.workers.is_empty() {
        let net = monorail::network(dimension);
        // 0.19.1: every stretch of rail and the docks it has, once a tick for all its workers.
        let bases = rustcraft_rules::worker::base::Bases::of(&net, &unload, &charge, &load);
        let mut workers = std::mem::take(&mut level.fleet.workers);
        for w in &mut workers {
            if !chunk_loaded(game, w.cell) {
                // W7: it waits while its chunk is not loaded; its seat went with the chunk.
                level.seats.remove(&w.id);
                continue;
            }
            let before = (w.cell, w.errand, w.hold.used(), w.note);
            let docks = Docks {
                unload: &unload,
                charge: &charge,
                load: &load,
                bases: &bases,
            };
            ride(game, dimension, &net, &docks, w, tuning);
            if before != (w.cell, w.errand, w.hold.used(), w.note) {
                level.dirty = true;
            }
            seat(game, dimension, &mut level, w);
        }
        level.fleet.workers = workers;
    }
    if level.ticks.is_multiple_of(NOTES_EVERY) {
        notes(game, &level, &monorail::network(dimension), tuning);
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

/// The level's docks of each kind, and the stretches of rail that have them (0.19.1).
struct Docks<'a> {
    unload: &'a BTreeSet<Pos>,
    charge: &'a BTreeSet<Pos>,
    load: &'a BTreeSet<Pos>,
    bases: &'a rustcraft_rules::worker::base::Bases,
}

fn ride(
    game: &mut Game<'_>,
    dimension: &str,
    net: &Network,
    docks: &Docks<'_>,
    w: &mut Worker,
    t: &Tuning,
) {
    let track = Track {
        net,
        unload: docks.unload,
        charge: docks.charge,
        load: docks.load,
        bases: Some(docks.bases),
    };
    // T4: a fast rail takes it past more than a cell a tick; it rides on, each cell worked.
    let mut step = w.tick(&track, t);
    // Slice 18: a guard standing at a dock or stopped still shoots (P2).
    if w.guards_standing(step) {
        let cell = w.cell;
        guard(game, dimension, w, cell, t);
    }
    for _ in 0..RIDE_ON_MAX {

        match step {
            Step::Arrived(cell) => work(game, dimension, net, w, cell, t),
            Step::AtDock(dock) => serve(game, w, dock, t),
            Step::Busy | Step::Moved | Step::Stopped => {}
        }
        if !w.rides_on() {
            break;
        }
        step = w.ride_on(&track, t);
    }
}

/// Cells a worker may pass in one tick: the fastest rail's 32 bl/s are 1.6 a tick.
const RIDE_ON_MAX: usize = 4;

/// The tool looks round the cell it came into.
fn work(
    game: &mut Game<'_>,
    dimension: &str,
    net: &Network,
    w: &mut Worker,
    cell: Pos,
    t: &Tuning,
) {
    let mount = rustcraft_rules::worker::mount(net, cell);
    w.aim = None;
    match w.tool {
        Some(Tool::Saw) => saw(game, dimension, w, cell, mount, t),
        Some(Tool::Reaper) => reap(game, dimension, w, cell, mount, t),
        Some(Tool::Grab) => grab(game, dimension, w, cell, mount, t),
        Some(Tool::Scoop) => scoop(game, dimension, net, w, cell, mount, t),
        Some(Tool::Shooter) => guard(game, dimension, w, cell, t),
        Some(Tool::Map) => crate::game::rustcraft_explore::map(game, dimension, w, cell, t),
        Some(Tool::Bore) => bore(game, dimension, net, w, cell, t),
        Some(Tool::Hold) | None => {}
    }
}

/// Slice 11: the guard (P2, P3): in each rail cell it comes into it shoots the nearest hostile mob
/// in its range, if its battery has a shot, and stands while it shoots. Its kills count as a
/// turret's for its owner (D5, X8) and go to SoulsCraft as `guard` (spec 27.1).
fn guard(game: &mut Game<'_>, dimension: &str, w: &mut Worker, cell: Pos, t: &Tuning) {
    if w.charge < t.guard_shot_energy {
        return;
    }
    let at = [
        f64::from(cell[0]) + 0.5,
        f64::from(cell[1]) + 0.5,
        f64::from(cell[2]) + 0.5,
    ];
    let range = w.guard_range(t);
    w.aim = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.alive() && m.is_enemy())
        .map(|m| [m.position.x, m.position.y + 0.5, m.position.z])
        .filter(|p| (0..3).map(|k| (p[k] - at[k]).powi(2)).sum::<f64>() <= range * range)
        .min_by(|a, b| {
            let d = |p: &[f64; 3]| (0..3).map(|k| (p[k] - at[k]).powi(2)).sum::<f64>();
            d(a).total_cmp(&d(b))
        })
        .map(|p| p.map(|c| c.floor() as i32));
    if base::guard_shot(game, dimension, w.owner, at, range, t.guard_damage).is_some() {
        w.spend(t.guard_shot_ticks, t.guard_shot_energy, t);
    }
}

/// The grab (guide `tool_grab`): every ore of its zone broken into the hold, every suspicious
/// block the vein grew there taken whole (ST4), and the items lying in its zone picked up.
fn grab(game: &mut Game<'_>, dimension: &str, w: &mut Worker, cell: Pos, mount: Face, t: &Tuning) {
    let zone = w.zone_cells(cell, mount, t);
    let ores: Vec<Pos> = zone
        .iter()
        .copied()
        .filter(|&p| digging::is_ore(&game.native_block(p)))
        .collect();
    let mut done = 0usize;
    w.aim = w.aim.or(ores.first().copied());
    for p in ores {
        let block = game.native_block(p);
        if take_block(game, dimension, w, p, Some(digging::GRAB_TOOL)) {
            done += 1;
            report_break(dimension, w, p, block, Tool::Grab);
        }
    }
    done += take_grown(game, dimension, w, &zone);
    let (lo, hi) = w.zone_box(cell, mount, t);
    let (lo, hi) = digging::pickup_box(lo, hi);
    for e in game.native_item_entities(dimension, lo, hi) {
        let max = machines::max_stack(game, &e.stack.item);
        let left = w.hold.add(&e.stack.item, e.stack.count, max);
        if left == e.stack.count {
            continue;
        }
        let mut rest = e.stack.clone();
        rest.count = left;
        game.native_set_item_entity(dimension, e.id, rest);
        done += 1;
    }
    // 0.18.5: a whole stretch with no ore and nothing lying beside the way, it says so.
    w.found_as(done, IDLE_CELLS, Note::NoOre);
    if done > 0 {
        w.spend(t.grab_ticks * done as f64, t.grab_energy * done as f64, t);
    }
}

/// ST4: the suspicious blocks of `zone` the vein grew, lifted whole into the hold under their
/// marked name, no `break` told (the vein's `place` keeps the cell marked in Path-of-craft-tree);
/// how many.
fn take_grown(game: &mut Game<'_>, dimension: &str, w: &mut Worker, zone: &[Pos]) -> usize {
    let Some(air) = game.native_default_state(AIR) else {
        return 0;
    };
    let mut done = 0;
    for &p in zone {
        let block = game.native_block(p);
        let grown_here = grown::is_suspicious(&block) && util::grown_at(game, dimension, p);
        if !grown::grab_takes(&block, grown_here) {
            continue;
        }
        let max = machines::max_stack(game, &block);
        let name = util::grown_hold_name(game, p, &block);
        if w.hold.add(&name, 1, max) > 0 {
            break;
        }
        if !game.native_set_block(dimension, p, air, 3) {
            w.hold.take(&name, 1);
            continue;
        }
        util::forget_grown(game, dimension, p);
        done += 1;
    }
    done
}

/// The scoop (guide `tool_scoop`): earth, sand, gravel and clay beside the way and beyond its
/// rail (under a floor rail, into a wall beside a wall rail), nearest first; the blocks that hold
/// rails stay.
fn scoop(
    game: &mut Game<'_>,
    dimension: &str,
    net: &Network,
    w: &mut Worker,
    cell: Pos,
    mount: Face,
    t: &Tuning,
) {
    let (side, _) = w.zone(t);
    let depth = t.scoop_depth;
    let back = mount.opposite().offset();
    let under_rail = |p: Pos| {
        (1..=depth).any(|k| {
            net.rails
                .contains_key(&rustcraft_rules::worker::moved(p, back, k))
        })
    };
    let cells: Vec<Pos> = digging::scoop_cells(cell, mount, side, depth, under_rail)
        .into_iter()
        .filter(|&p| {
            let block = game.native_block(p);
            digging::is_scooped(&block) && digging::wanted(&block, &w.filter)
        })
        .collect();
    let mut done = 0usize;
    w.aim = cells.first().copied();
    for p in cells {
        let block = game.native_block(p);
        if take_block(game, dimension, w, p, Some(digging::SCOOP_TOOL)) {
            done += 1;
            report_break(dimension, w, p, block, Tool::Scoop);
        }
    }
    // After a whole stretch with nothing to dig, it says so («Рядом с путём нечего копать»).
    w.found(done, IDLE_CELLS);
    if done > 0 {
        w.spend(t.scoop_ticks * done as f64, t.scoop_energy * done as f64, t);
    }
}

/// Slice 14: the bore at the end of its rails: the tunnel ahead bored, lava closed, the gap under
/// the next rail filled, the rail laid; the monorail rescans it and the worker rides on.
fn bore(game: &mut Game<'_>, dimension: &str, net: &Network, w: &mut Worker, cell: Pos, t: &Tuning) {
    use rustcraft_rules::worker::bore;
    let Some(level_ahead) = w.from.and_then(|f| bore::ahead(f, cell)) else { return };
    // 0.19.1 (the owner): only at the end of its rails away from its docks (worker::base).
    let heading = [level_ahead[0] - cell[0], level_ahead[2] - cell[2]];
    if w.bore_to != Some((cell, heading)) {
        return;
    }
    if net.rails.contains_key(&level_ahead) || net.rails.contains_key(&[level_ahead[0], level_ahead[1] - 1, level_ahead[2]]) {
        return;
    }
    // Slice 18: the best rail of its hold, tiers 1 to 4.
    let Some((rail, tier)) = bore::best_rail(|i| w.hold.count(i)) else {
        w.note = Note::NoRails;
        return;
    };
    use rustcraft_rules::modules::Module as M;
    let has = |m: M| w.upgrades.iter().flatten().filter(|x| **x == m).count() as u32;
    // The height module keeps the way level; without it the way follows the ground down a step.
    let open = |p: Pos| {
        let b = game.native_block(p);
        matches!(b.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:water") || b.contains("lava")
    };
    let (ahead, _) = bore::next_cell(level_ahead, open, has(M::Height) > 0);
    // Wave 2 (guide `mod_height`): with a height set in the chassis window, a step up or down
    // towards it.
    let ahead = bore::toward_height(level_ahead, has(M::Height) > 0, w.height).unwrap_or(ahead);
    let heading = [ahead[0] - cell[0], ahead[2] - cell[2]];
    let (side, height) = bore::size(has(M::Tunnel));
    let (lava_module, build_module) = (has(M::Lava) > 0, has(M::Build) > 0);
    let cells = bore::cells(ahead, heading, side, height);
    w.aim = Some(ahead);
    let filler = |w: &Worker| w.hold.find(bore::is_filler);
    // Lava in the tunnel is closed first: with a block of the hold each, or with nothing at all
    // (it turns to air) under the lava module.
    for &p in cells.iter().chain(bore::shell(ahead, heading, side, height).iter()) {
        if !game.native_block(p).contains("lava") {
            continue;
        }
        if lava_module {
            if let Some(state) = game.native_default_state("minecraft:cobblestone") {
                game.native_set_block(dimension, p, state, 3);
            }
            continue;
        }
        let Some(item) = filler(w) else {
            w.note = Note::LavaAhead;
            return;
        };
        if let Some(state) = game.native_default_state(&item) {
            game.native_set_block(dimension, p, state, 3);
            w.hold.take(&item, 1);
        }
    }
    let mut done = 0usize;
    for &p in &cells {
        let block = game.native_block(p);
        if matches!(block.as_str(), "minecraft:air" | "minecraft:cave_air") {
            continue;
        }
        if take_block(game, dimension, w, p, Some(digging::GRAB_TOOL)) {
            done += 1;
            report_break(dimension, w, p, block, Tool::Bore);
        }
    }
    // The build module makes the floor, the walls and the roof good where they are open.
    if build_module {
        for p in bore::shell(ahead, heading, side, height) {
            let b = game.native_block(p);
            if !matches!(b.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:water") {
                continue;
            }
            let Some(item) = filler(w) else { break };
            if let Some(state) = game.native_default_state(&item) {
                game.native_set_block(dimension, p, state, 3);
                w.hold.take(&item, 1);
            }
        }
    }
    let under = [ahead[0], ahead[1] - 1, ahead[2]];
    let ground = game.native_block(under);
    if matches!(ground.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:water") || ground.contains("lava") {
        let Some(item) = filler(w) else {
            w.note = Note::LavaAhead;
            return;
        };
        if let Some(state) = game.native_default_state(&item) {
            game.native_set_block(dimension, under, state, 3);
            w.hold.take(&item, 1);
        }
    }
    if let Some(state) = game.native_default_state(bore::RAIL) {
        game.native_set_block(dimension, ahead, state, 3);
        if tier > 1 {
            game.native_set_block_property(ahead, content::TIER, &tier.to_string());
        }
        w.hold.take(rail, 1);
        crate::game::rustcraft_monorail::touched(dimension, ahead);
        done += 1;
        // A rail laid: «Кончились рельсы» and «Впереди лава» are over.
        if w.note.is_tools() {
            w.note = Note::Working;
        }
    }

    w.spend(t.grab_ticks * done as f64, t.grab_energy * done as f64, t);
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
    util::forget_grown(game, dimension, pos);
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

/// The saw: the first tree with a log in the zone, felled top down, planted again; then, 0.18.5,
/// saplings of its hold planted on the free ground of its zone (`plant_zone`).
fn saw(game: &mut Game<'_>, dimension: &str, w: &mut Worker, cell: Pos, mount: Face, t: &Tuning) {
    let zone = w.zone_cells(cell, mount, t);
    let tree = {
        let view = View { game };
        zone.iter()
            .find_map(|&p| trees::find(&view, p, t.saw_max_logs, t.saw_leaf_reach))
    };
    let (felled, unplanted) = match tree {
        Some(tree) => fell(game, dimension, w, &tree),
        None => (0, false),
    };
    let (planted, lacking) = plant_zone(game, dimension, w, &zone);
    let done = felled as usize + planted;
    // A whole stretch with no tree beside the way, it says so; no sapling for a felled tree's
    // root, or for free ground when it did nothing else here, «Нет саженцев».
    w.found_as(done, IDLE_CELLS, Note::NoTrees);
    if unplanted || (done == 0 && lacking) {
        w.note = Note::NoSaplings;
        w.idle = 0;
    }
    w.spend(
        t.saw_ticks_per_log * done as f64,
        t.saw_energy_per_log * done as f64,
        t,
    );
}

/// Fells `tree` top down, its leaves after, and plants its roots again from the hold: the logs
/// felled, and whether a root stayed bare for want of a sapling.
fn fell(game: &mut Game<'_>, dimension: &str, w: &mut Worker, tree: &trees::Tree) -> (u32, bool) {
    w.aim = tree.logs.last().copied();
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
            return (felled, true);
        };
        if set_block(game, dimension, root, &sapling, &[]) {
            w.hold.take(&sapling, 1);
        }
    }
    (felled, false)
}

/// 0.18.5 (the owner: «работники всё делают сами»): the saw plants the saplings of its hold on the
/// free ground of `zone`, with room for each tree to grow (rustcraft_rules::worker::trees::spot), a
/// dark or pale oak four together inside the zone: how many spots it planted, and whether
/// free ground found no sapling in the hold.
fn plant_zone(game: &mut Game<'_>, dimension: &str, w: &mut Worker, zone: &[Pos]) -> (usize, bool) {
    let mut planted = 0;
    let mut lacking = false;
    // The saplings of its hold it plants, in slot order, each once.
    let mut saplings: Vec<String> = Vec::new();
    for (item, _) in w.hold.slots.iter().flatten() {
        if trees::plants(item) && !saplings.contains(item) {
            saplings.push(item.clone());
        }
    }
    for &p in zone {
        let found = {
            let view = View { game };
            saplings.iter().find_map(|s| {
                let cells = trees::spot(&view, p, s)?;
                let enough = w.hold.count(s) >= trees::per_spot(s);
                (enough && cells.iter().all(|c| zone.contains(c))).then(|| (s.clone(), cells))
            })
        };
        let Some((sapling, cells)) = found else {
            // Free ground and nothing to plant on it.
            lacking |= trees::spot(&View { game }, p, "minecraft:oak_sapling").is_some();
            continue;
        };
        let mut put = 0;
        for c in cells {
            if set_block(game, dimension, c, &sapling, &[]) {
                w.hold.take(&sapling, 1);
                put += 1;
            }
        }
        if put > 0 {
            planted += 1;
            w.aim = w.aim.or(Some(p));
        }
        if w.hold.count(&sapling) == 0 {
            saplings.retain(|s| *s != sapling);
        }
    }
    (planted, lacking)
}

/// The reaper: every ripe plant of the zone by its group's rule (spec 17).
fn reap(game: &mut Game<'_>, dimension: &str, w: &mut Worker, cell: Pos, mount: Face, t: &Tuning) {
    let (lo, hi) = w.zone_box(cell, mount, t);
    // On a wall rail it rides up and down beside its plants: a column is cut only from its own
    // top, never under it, so nothing over the cut falls to the ground (guide «вверх вдоль
    // высокого бамбука»).
    let top = if matches!(mount, Face::YNeg | Face::YPos) {
        hi[1]
    } else {
        i32::MAX
    };
    let zone = w.zone_cells(cell, mount, t);
    let plans: Vec<plants::Harvest> = {
        let view = View { game };
        zone.iter()
            .filter(|&&p| plants::wanted(&view.block(p), &w.filter))
            .filter_map(|&p| plants::plan(&view, p, lo[1], top))
            .collect()
    };
    let mut done = 0usize;
    let mut unsown: Vec<Pos> = Vec::new();
    w.aim = plans.first().and_then(|h| h.breaks.first().or(h.picks.first().map(|p| &p.pos)).copied());
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
                // H3: the cell stays empty, unless another seed of the hold sows it below.
                unsown.push(r.pos);
            }
        }
        done += h.size();
    }
    // 0.18.5: empty farmland of its zone (soul sand for nether wart) is sown from the hold.
    let (sown, lacking) = sow_zone(game, dimension, w, &zone);
    w.aim = w.aim.or(sown.first().copied());
    done += sown.len();
    // Nothing ripe for a whole stretch: it says so; nothing to sow with only when it has nothing
    // else to do here, or when a cell it picked stays empty (H3).
    w.found_as(done, IDLE_CELLS, Note::NothingToReap);
    let replant_failed = unsown.iter().any(|p| !sown.contains(p));
    if replant_failed || (done == 0 && lacking) {
        w.note = Note::NothingToSow;
        w.idle = 0;
    }
    if done > 0 {
        w.spend(t.reap_ticks * done as f64, t.reap_energy * done as f64, t);
    }
}

/// The reaper sows each air cell of `zone` over farmland, or soul sand for nether wart, with a
/// seed of its hold, and tills grass, dirt and dirt paths to farmland to sow them in the same pass
/// (rustcraft_rules::worker::plants::sow), ground near water first: the cells it sowed, and
/// whether some ground found no seed in the hold.
fn sow_zone(game: &mut Game<'_>, dimension: &str, w: &mut Worker, zone: &[Pos]) -> (Vec<Pos>, bool) {
    let mut sown = Vec::new();
    let mut lacking = false;
    // The hold's seeds in slot order, each once.
    let mut seeds: Vec<String> = Vec::new();
    for (item, _) in w.hold.slots.iter().flatten() {
        if plants::is_seed(item) && !seeds.contains(item) {
            seeds.push(item.clone());
        }
    }
    // 0.18.5: ground to till near water first, so the seeds go where crops grow best.
    let mut cells: Vec<(bool, Pos)> = zone
        .iter()
        .map(|&p| {
            let view = View { game };
            let under = step(p, Face::YNeg);
            let dry = plants::tills(&view.block(under)) && !plants::hydrated(&view, under);
            (dry, p)
        })
        .collect();
    cells.sort_by_key(|(dry, _)| *dry);
    for (_, p) in cells {
        let sowing = plants::sow(&View { game }, p, &seeds, &w.filter);
        let (r, till) = match sowing {
            plants::Sowing::Nothing => continue,
            plants::Sowing::NoSeed => {
                lacking = true;
                continue;
            }
            plants::Sowing::Sow(r) => (r, false),
            plants::Sowing::Till(r) => (r, true),
        };
        if till && !set_block(game, dimension, step(r.pos, Face::YNeg), FARMLAND, &[]) {
            continue;
        }
        if set_block(game, dimension, r.pos, &r.block, &r.props) {
            w.hold.take(&r.seed, 1);
            sown.push(r.pos);
            if w.hold.count(&r.seed) == 0 {
                seeds.retain(|s| *s != r.seed);
            }
        }
    }
    (sown, lacking)
}

/// A dock serves the worker standing at it: the unload dock takes its load, the charge dock
/// fills its battery.
fn serve(game: &mut Game<'_>, w: &mut Worker, dock: Pos, t: &Tuning) {
    match w.errand {
        Errand::Unload => unload_into(game, w, dock, t),
        Errand::Charge => charge_from(game, w, dock, t),
        Errand::Load => load_from(game, w, dock, t),
        Errand::Work => {}
    }
}

/// W6: the load dock fills a cargo worker's hold from its slots, as many items a tick as the
/// unload dock takes; a full hold, or a hold with cargo at an empty dock, rides on to unload.
fn load_from(game: &mut Game<'_>, w: &mut Worker, dock: Pos, t: &Tuning) {
    let per_tick = (t.load_items_per_second / t.ticks_per_second)
        .ceil()
        .max(1.0) as u32;
    let slots: Vec<Option<ItemStack>> = game
        .native_mod_block_entity(dock)
        .map(|e| e.items.clone())
        .unwrap_or_default();
    let mut moved = 0;
    let mut taken: Vec<(usize, u32)> = Vec::new();
    for (i, s) in slots.iter().enumerate() {
        if moved >= per_tick || w.hold.is_full() {
            break;
        }
        let Some(s) = s.as_ref().filter(|s| s.count > 0 && &*s.item != AIR) else {
            continue;
        };
        let n = s.count.min(per_tick - moved);
        let max = machines::max_stack(game, &s.item);
        let left = w.hold.add(&util::hold_name(s), n, max);
        if n > left {
            taken.push((i, n - left));
            moved += n - left;
        }
    }
    if !taken.is_empty() {
        game.native_with_mod_block_entity(dock, |e| {
            for (i, n) in &taken {
                if let Some(Some(s)) = e.items.get_mut(*i) {
                    s.count = s.count.saturating_sub(*n);
                    if s.count == 0 {
                        e.items[*i] = None;
                    }
                }
            }
        });
    }
    let empty = game.native_mod_block_entity(dock).is_none_or(|e| {
        e.items
            .iter()
            .all(|x| x.as_ref().is_none_or(|x| x.count == 0))
    });
    w.loaded(empty);
}

fn unload_into(game: &mut Game<'_>, w: &mut Worker, dock: Pos, t: &Tuning) {
    let per_tick = (t.dock_items_per_second / t.ticks_per_second)
        .ceil()
        .max(1.0) as u32;
    let tool = w.tool;
    let keep = |i: &str| tool.is_some_and(|t| t.keeps(i));
    let portal_dock = game.native_block(dock) == PORTAL_DOCK;
    let mut moved = 0;
    while moved < per_tick {
        let Some(slot) = w.hold.next_to_unload(keep) else {
            break;
        };
        let Some((item, count)) = w.hold.slots[slot].clone() else {
            break;
        };
        let n = count.min(per_tick - moved);
        let stack = util::hold_stack(&item, 1);
        let max = machines::max_stack(game, &stack.item);
        let put = if portal_dock {
            // Into the slots a portal dock sends from (rustcraft_builder: 0..18).
            let left = util::put(game, dock, PORTAL_SEND, util::hold_stack(&item, n));
            n - left.map_or(0, |l| l.count.min(n))
        } else {
            game.native_with_mod_block_entity(dock, |e| into_slots(&mut e.items, &stack, n, max))
                .unwrap_or(0)
        };
        if put == 0 {
            break;
        }
        w.hold.take(&item, put);
        moved += put;
    }
    if moved > 0 {
        dock_log().received.push((dock, std::time::Instant::now(), moved));
    }
    if w.unloaded() || (moved == 0 && w.only_passing(t)) {
        // 0.18.5: a worker that stopped here in passing rides on past a full dock.
        w.back_to_work();
    } else if moved == 0 {
        w.note = Note::DockFull;
    } else {
        w.note = Note::Unloading;
    }
}

/// The portal dock (rustcraft_builder) and the slots it sends to its pair from.
const PORTAL_DOCK: &str = "rustcraft:portal_dock";
const PORTAL_SEND: std::ops::Range<usize> = 0..18;

/// Puts up to `count` of `like`'s item into the dock's slots; how many went in. A grown block

/// stacks only with grown ones.
fn into_slots(items: &mut Vec<Option<ItemStack>>, like: &ItemStack, count: u32, max: u32) -> u32 {
    if items.len() < content::DOCK_SLOTS {
        items.resize(content::DOCK_SLOTS, None);
    }
    let marked = util::is_grown(like);
    let mut left = count;
    for s in items.iter_mut().flatten() {
        if left == 0 {
            break;
        }
        if s.item == like.item && util::is_grown(s) == marked && s.count < max {
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
            let mut put = like.clone();
            put.count = k;
            *s = Some(put);
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
        crate::game::rustcraft_base::dock_gave(dock, give);
        w.charge += give;
        let share = w.charge / cap.max(1.0);
        dock_log()
            .charging
            .insert(dock, (w.tool, share, std::time::Instant::now()));
        w.note = Note::Charging;
    } else if w.only_passing(t) {
        // 0.18.5: a worker topping up in passing rides on from a dock with no energy.
        w.back_to_work();
        return;
    } else if w.charge < cap {
        w.note = Note::DockEmpty;
    }
    if w.charge >= cap - 1e-6 {
        w.charge = cap;
        w.back_to_work();
    }
}

/// Slice 18 (guide `dock_load`: «предметами из магистрали или соседнего сундука»): each load dock
/// takes items from a container beside it (a chest, a barrel; never a RustCraft block).
fn pull_into_load_docks(game: &mut Game<'_>, dimension: &str, docks: &BTreeSet<Pos>) {
    for &dock in docks {
        for f in Face::ALL {
            let n = step(dock, f);
            let block = game.native_block(n);
            if block == AIR || server::is_ours(&block) {
                continue;
            }
            game.native_move_items(
                dimension,
                n,
                transport::direction(f.opposite()),
                dock,
                transport::direction(f),
                None,
                DOCK_PUSH,
            );
        }
    }
}

/// What the docks did lately, for their windows' lines (guide `ui.js` docks).
#[derive(Default)]
struct DockLog {
    /// Items an unload dock took from workers: dock, when, how many.
    received: Vec<(Pos, std::time::Instant, u32)>,
    /// The worker a charge dock charges: its tool, its charge 0..1, when last.
    charging: HashMap<Pos, (Option<Tool>, f64, std::time::Instant)>,
}

static DOCK_LOG: LazyLock<Mutex<DockLog>> = LazyLock::new(Mutex::default);

fn dock_log() -> MutexGuard<'static, DockLog> {
    let mut log = DOCK_LOG.lock().unwrap_or_else(PoisonError::into_inner);
    let hour = std::time::Duration::from_secs(3600);
    log.received.retain(|(_, at, _)| at.elapsed() < hour);
    log
}

/// The lines a dock's window shows over its slots (guide `ui.js`: «Отдаёт груз · в магистраль»,
/// «Принято за час · 312 предметов»; «Берёт груз · из сундука рядом», «Грузит · всё подряд»;
/// «Заряжает · Лесоруб · 46%»).
pub(super) fn dock_rows(game: &Game<'_>, pos: Pos, machine: Machine) -> Vec<Text> {
    let beside = |pick: &dyn Fn(&str) -> bool| {
        Face::ALL
            .into_iter()
            .any(|f| pick(&game.native_block(step(pos, f))))
    };
    let chest = beside(&|b: &str| b != AIR && !server::is_ours(b));
    let magistral = beside(&|b: &str| b == content::HOUSING);
    match machine {
        Machine::DockCharge => {
            let now = dock_log().charging.get(&pos).copied();
            match now.filter(|(_, _, at)| at.elapsed().as_secs() < 2) {
                Some((tool, share, _)) => {
                    let mut who = tool_name(tool);
                    who.push(Text::literal(format!(" · {:.0}%", share * 100.0)));
                    vec![line("rustcraft.dock.charging", "Charges · %s", &[who])]
                }
                None => vec![line("rustcraft.dock.charging_none", "Charges · nobody now", &[])],
            }
        }
        Machine::DockLoad => {
            let from = if chest {
                line("rustcraft.dock.takes_chest", "Takes cargo · from the chest beside it", &[])
            } else {
                line("rustcraft.dock.takes_net", "Takes cargo · from the magistral", &[])
            };
            vec![from, line("rustcraft.dock.loads_all", "Loads · everything", &[])]
        }
        _ => {
            let to = if magistral {
                line("rustcraft.dock.gives_net", "Gives cargo · to the magistral", &[])
            } else if chest {
                line("rustcraft.dock.gives_chest", "Gives cargo · to the chest beside it", &[])
            } else {
                line("rustcraft.dock.gives_none", "Gives cargo · nowhere yet", &[])
            };
            let hour: u32 = dock_log()
                .received
                .iter()
                .filter(|(p, _, _)| *p == pos)
                .map(|(_, _, n)| n)
                .sum();
            vec![
                to,
                line(
                    "rustcraft.dock.per_hour",
                    "Taken in an hour · %s items",
                    &[Text::literal(hour.to_string())],
                ),
            ]
        }
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
}

fn tool_name(tool: Option<Tool>) -> Text {
    let (key, fallback) = rustcraft_rules::worker::wire::tool_key(tool);
    line(key, fallback, &[])
}

/// Every client is told each seat, its heading, tool, the mount of its rail, whether its tool
/// works and on what, and its board (rustcraft_rules::worker::wire, rustcraft_station::on_net).
fn notes(game: &mut Game<'_>, level: &LevelFleet, net: &Network, t: &Tuning) {
    use rustcraft_rules::worker::{pose, wire};
    let seats: Vec<wire::Seat> = level
        .fleet
        .workers
        .iter()
        .filter_map(|w| {
            let seat = *level.seats.get(&w.id)?;
            let mount = rustcraft_rules::worker::mount(net, w.cell);
            let yaw = w.yaw().or_else(|| pose::off_the_wall(mount)).unwrap_or(-90.0);
            Some(wire::Seat {
                seat,
                yaw,
                tool: w.tool,
                mount,
                // The cartographer's drum turns while it charts its way (poses.js `map`).
                busy: w.busy > 0.0 || (w.tool == Some(Tool::Map) && w.note == Note::Working),
                aim: w.aim.map(|p| p.map(|c| c as f32 + 0.5)),
                board: wire::Board::of(w, t),
            })
        })
        .collect();
    if seats.is_empty() {
        return;
    }
    let bytes = wire::encode(&seats);
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
    // Wave 2: the window is the mod's own screen on the player's client (screen::View).
    for (uuid, _, id) in opening {
        let Some(w) = level.fleet.workers.iter().find(|w| w.id == id) else {
            continue;
        };
        send_view(game, uuid, w, t, true);
        state().sessions.insert(
            uuid,
            Session {
                dimension: dimension.to_owned(),
                worker: id,
                window: SCREEN_WINDOW,
            },
        );
    }
    let presses: Vec<(u128, Press)> = {
        let mut s = state();
        let mine: Vec<u128> = s.sessions.iter().filter(|(_, x)| x.dimension == dimension).map(|(u, _)| *u).collect();
        let (here, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.presses).into_iter().partition(|(u, _)| mine.contains(u));
        s.presses = rest;
        here
    };
    let mut clicks = clicks;
    for (uuid, press) in presses {
        let id = match state().sessions.get(&uuid) {
            Some(x) => x.worker,
            None => continue,
        };
        match press {
            Press::Closed => {
                state().sessions.remove(&uuid);
            }
            Press::Cell { cell, right } => clicks.push(Click { uuid, window: SCREEN_WINDOW, slot: cell, shift: right }),
            Press::Height { by } => {
                if let Some(w) = level.fleet.workers.iter_mut().find(|w| w.id == id) {
                    w.height = screen::adjust_height(w.height, w.cell[1], by);
                    level.dirty = true;
                    send_view(game, uuid, w, t, false);
                }
            }
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
            close_view(game, c.uuid);
            continue;
        };
        if click(game, c.uuid, c.slot, c.shift, w, t) {
            level.dirty = true;
        }
        send_view(game, c.uuid, w, t, false);
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
                send_view(game, uuid, w, t, false);
            } else {
                close_view(game, uuid);
            }
        }
    }
}

/// Wave 2: what the chassis window shows, to the player who has it open (rustcraft_station draws
/// it); `open` opens the screen.
fn send_view(game: &mut Game<'_>, uuid: u128, w: &Worker, t: &Tuning, open: bool) {
    let mut v = screen::View::of(w, t);
    v.open = open;
    if game.remote.is_none() && uuid == game.native_acting_id() {
        crate::game::rustcraft_station::chassis_view(v);
        return;
    }
    if let Err(e) = game.native_net_send_to_player(uuid, screen::VIEW_CHANNEL, &v.encode()) {
        log::debug!("rustcraft: a chassis window not sent: {e}");
    }
}

/// The chassis window closes: its worker is gone.
fn close_view(game: &mut Game<'_>, uuid: u128) {
    state().sessions.remove(&uuid);
    let v = screen::View { close: true, ..screen::View::default() };
    if game.remote.is_none() && uuid == game.native_acting_id() {
        crate::game::rustcraft_station::chassis_view(v);
        return;
    }
    let _ = game.native_net_send_to_player(uuid, screen::VIEW_CHANNEL, &v.encode());
}

/// `native_net_from_client` (through rustcraft_monorail): a press in a player's chassis window.
pub(super) fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel != screen::PRESS_CHANNEL {
        return;
    }
    match screen::decode_press(data) {
        Some(p) => press(player, p),
        None => crate::game::rustcraft_debug::journal_warn!("rustcraft: a chassis window press that does not read"),
    }
}

/// A press in `player`'s chassis window, for their level's next tick (the single player's
/// window gives it straight from rustcraft_station).
pub(super) fn press(player: u128, p: Press) {
    state().presses.push((player, p));
}

/// Gives `count` of `item` (a hold's name) to the player; how many they took.


fn give(game: &mut Game<'_>, uuid: u128, item: &str, count: u32) -> u32 {
    match game.native_give_stack(uuid, util::hold_stack(item, count)) {
        Some(left) => count - left.count.min(count),
        None => count,
    }
}

/// A click: on the tool, an upgrade or the hold it goes to the player; on an item of the
/// player's inventory below, that item goes in (a tool to the tool slot, an upgrade to a free
/// slot, anything else to the hold). Whether the worker changed.
fn click(
    game: &mut Game<'_>,
    uuid: u128,
    slot: usize,
    shift: bool,
    w: &mut Worker,
    t: &Tuning,
) -> bool {
    // H6: a click on the state cell clears the filter.
    if slot == NOTE_CELL {
        return !std::mem::take(&mut w.filter).is_empty();
    }
    if slot == TOOL_CELL {
        let Some(tool) = w.tool else { return false };
        if give(game, uuid, tool.item(), 1) == 1 {
            w.tool = None;
            for (item, n) in w.refit(t) {
                give(game, uuid, &item, n);
            }
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
        // Slice 18: a hold past the window's slots moves its hidden stacks up into them.
        w.hold.settle(CELLS - HOLD_FIRST);
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
    // H6: a shift-click names the item in the filter; the item stays with the player.
    if shift {
        return w.filter_add(&item);
    }
    let (keep, back): (u32, Option<String>) = if let Some(tool) = Tool::of(&item) {
        let old = w.tool.replace(tool);
        // A cargo hold changes the hold's size (guide `tool_hold`).
        for (i, n) in w.refit(t) {
            give(game, uuid, &i, n);
        }
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
        let left = w.hold.add(&util::hold_name(&stack), stack.count, max);
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
        crate::game::rustcraft_debug::journal_warn!("rustcraft: the worker took an item the player no longer had");
    }
    if let Some(old) = back {
        give(game, uuid, &old, 1);
    }
    // A new tool, or seeds, saplings or rails for it: the old word goes until the tool works.
    if w.errand == Errand::Work && w.tool.is_some() && (w.note == Note::NoTool || w.note.is_tools()) {
        w.note = Note::Working;
        w.idle = 0;
    }
    true
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
        w.refit(t);
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

/// W3 for the self-test: worker `id` taken away as its one chassis item, read back from that
/// item and set on rail `cell`; its new id.
pub(super) fn as_item_for_selftest(
    game: &mut Game<'_>,
    dimension: &str,
    id: u64,
    cell: Pos,
    t: &Tuning,
) -> Result<u64, String> {
    let w = worker_for_selftest(dimension, id).ok_or("no such worker")?;
    remove_for_selftest(game, dimension, id);
    let stack = worker_stack(&w);
    if chassis::tier_of(&stack.item) != Some(w.tier) {
        return Err(format!("the item is {}", stack.item));
    }
    let packed = packed_of(&stack).ok_or("the chassis item carries no worker")?;
    let mut level = take_level(dimension);
    let back = level.fleet.put_back(w.owner, cell, &packed, t);
    level.dirty = true;
    put_level(dimension, level);
    back.ok_or_else(|| "the packed worker does not read".to_owned())
}

/// U4 for the self-test: chassis `item` set in worker `id`'s place; the old chassis's item.
pub(super) fn swap_for_selftest(dimension: &str, id: u64, item: &str) -> Result<String, String> {
    let mut s = state();
    let level = s.levels.get_mut(dimension).ok_or("no workers in this level")?;
    let w = level
        .fleet
        .workers
        .iter_mut()
        .find(|w| w.id == id)
        .ok_or("no such worker")?;
    let old = chassis::swap(w, item, server::tuning()).map_err(|e| format!("{e:?}"))?;
    level.dirty = true;
    Ok(old.to_owned())
}

/// W8 for the self-test: whether `owner` may place one more worker under a limit of `limit`.
pub(super) fn may_place_for_selftest(owner: u128, limit: u32) -> bool {
    rustcraft_rules::worker::config::ServerConfig {
        workers_per_player: limit,
    }
    .may_add(owned(owner))
}

/// H6 for the self-test: worker `id`'s filter set to `items`.
pub(super) fn filter_for_selftest(dimension: &str, id: u64, items: &[&str]) -> Result<(), String> {
    let mut s = state();
    let level = s.levels.get_mut(dimension).ok_or("no workers in this level")?;
    let w = level
        .fleet
        .workers
        .iter_mut()
        .find(|w| w.id == id)
        .ok_or("no such worker")?;
    w.filter = items.iter().map(|i| (*i).to_owned()).collect();
    level.dirty = true;
    Ok(())
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

/// rustcraft_debug: each level's fleet as the server holds it now, the levels whose fleet is not
/// read yet left out (so a restart does not look like every worker placed at once).
pub(super) fn fleets() -> Vec<(String, Vec<Worker>)> {
    state()
        .levels
        .iter()
        .filter(|(_, l)| l.loaded)
        .map(|(d, l)| (d.clone(), l.fleet.workers.clone()))
        .collect()
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
    // The tools' aim is the client's pose only and is not saved.
    let mut live = level.fleet.clone();
    for w in &mut live.workers {
        w.aim = None;
        w.passed = None;
    }
    match Fleet::decode(&back) {
        Some(f) if f == live => Ok(()),

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
        Machine::DockLoad => {
            let n: u32 = game
                .native_mod_block_entity(pos)
                .map_or(0, |e| e.items.iter().flatten().map(|s| s.count).sum());
            if n == 0 {
                line(
                    "rustcraft.dock.load_empty",
                    "Load dock · nothing to load",
                    &[],
                )
            } else {
                line(
                    "rustcraft.dock.load",
                    "Load dock · %s items wait for a cargo worker",
                    &[Text::literal(n.to_string())],
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

// ---- Wave 2: the chassis window for the self-test ----

/// The self-test's player of chassis windows: no one's, so its views go nowhere.
const SELFTEST_VIEWER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_c4a5;

/// A press in worker `id`'s window, as a player who has it open would make it.
pub(super) fn press_for_selftest(dimension: &str, id: u64, p: Press) -> Result<(), String> {
    let mut s = state();
    let level = s.levels.get(dimension).ok_or("no workers in this level")?;
    if !level.fleet.workers.iter().any(|w| w.id == id) {
        return Err("no such worker".into());
    }
    s.sessions.insert(SELFTEST_VIEWER, Session { dimension: dimension.to_owned(), worker: id, window: SCREEN_WINDOW });
    s.presses.push((SELFTEST_VIEWER, p));
    Ok(())
}

/// Worker `id`'s sockets hold `modules` (item ids), the rest empty.
pub(super) fn modules_for_selftest(dimension: &str, id: u64, modules: &[&str]) -> Result<(), String> {
    let mut s = state();
    let level = s.levels.get_mut(dimension).ok_or("no workers in this level")?;
    let w = level.fleet.workers.iter_mut().find(|w| w.id == id).ok_or("no such worker")?;
    for (i, slot) in w.upgrades.iter_mut().enumerate() {
        *slot = modules.get(i).and_then(|m| Module::of(m));
    }
    level.dirty = true;
    Ok(())
}
