// Server half of RustCraft. It keeps each level's magistral in memory (housings with their
// slots and joint settings, nodes with their tiers, crystals and machines), rebuilt from the
// block entities whenever something may have changed; puts a channel into a housing when a
// player clicks it with one (M2) and takes the last one out with the wrench and sneaking. Every
// tick it powers and runs the machines (rustcraft_machines) and answers the joint windows
// (rustcraft_window), every few ticks it moves items along the item nets (rustcraft_transport)
// and fluids along the fluid nets (rustcraft_fluids), and once a second it shows each player the
// diagnosis of the housing, machine, storage, tank or joint they look at. The storages
// (rustcraft_storage) are read again with the magistral and saved when they change.
//
// Every helper here reads the level in place: inside a player's hook it is that player's
// (`native_acting_player`, SRC/crates/horizon-client/src/game/native_server.rs:656-661), in
// `native_level_tick_post` it is the level that just ticked (`enter_level` before the hook,
// SRC/crates/horizon-client/src/game/levels.rs:525-560).
use crate::game::{
    Game, rustcraft_base as base,
    rustcraft_content::{self as content, Kind, Machine},
    rustcraft_fluids as fluids, rustcraft_machines as machines, rustcraft_mining as mining,
    rustcraft_monorail as monorail, rustcraft_power as power, rustcraft_selftest as selftest,
    rustcraft_storage as storage, rustcraft_transport as transport, rustcraft_window as window,
    rustcraft_workers as workers,
};
use horizon_client::{
    containers::ModBlockEntity, items::ItemStack, mod_api::NativeServerPlayer, text::Text,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    housing::{self, Diagnosis, SlotState},
    machines::processor::Shown,
    net::{
        joints::Port,
        settings::HousingJoints,
        topology::{self, Channel, ChannelKind, Housing, Layout, Node, Topology},
    },
    ray,
    tuning::Tuning,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

/// How often, in level ticks, the server looks at the magistral and the players (once a second).
const ANALYSE_EVERY: u64 = 20;
/// How often the layout is read again even when no hook said it changed: `/setblock`, an
/// explosion or another mod's edit reach no hook this slice listens to.
const RESCAN_EVERY: u64 = 200;
/// How far a player's look reaches a block, in blocks: a little past survival's 4.5.
const LOOK_REACH: f64 = 5.0;
/// Blocks a look passes through on its way to a housing.
const SEE_THROUGH: [&str; 5] = [
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:void_air",
    "minecraft:water",
    "minecraft:lava",
];
/// The housing's saved compound keeps its four slots under this key, as housing::encode's bytes.
const SLOTS_KEY: &str = "slots";
/// `NativeServerPlayer::game_mode` of creative and spectator.
const CREATIVE: i32 = 1;
const SPECTATOR: i32 = 3;
/// Channels the slice makes are all of the first tier; tiers come with recipes.
const FIRST_TIER: u8 = 1;

/// One level's magistral and machines as the server last read them.
pub struct LevelNet {
    pub layout: Layout,
    pub topology: Topology,
    pub crystals: BTreeSet<Pos>,
    pub machines: BTreeMap<Pos, Machine>,
    /// What each housing keeps of its joints' settings (M7); housings without any are absent.
    pub joints: BTreeMap<Pos, HousingJoints>,
    pub recipes: machines::MachineRecipes,
    pub power: machines::Power,
    pub transport: transport::Transport,
    pub fluids: fluids::Fluids,
    /// Slice 8: the structures of crystals, panels and batteries, and what the sources show.
    pub sources: power::Sources,
    /// Something may have changed since the layout was read: read it again before the next look.
    pub stale: bool,
    pub ticks: u64,
}

impl LevelNet {
    fn new() -> Self {
        LevelNet {
            layout: Layout::default(),
            topology: Topology::default(),
            crystals: BTreeSet::new(),
            machines: BTreeMap::new(),
            joints: BTreeMap::new(),
            recipes: machines::MachineRecipes::default(),
            power: machines::Power::default(),
            transport: transport::Transport::default(),
            fluids: fluids::Fluids::default(),
            sources: power::Sources::default(),
            stale: true,
            ticks: 0,
        }
    }
}

#[derive(Default)]
struct Server {
    levels: HashMap<String, LevelNet>,
    /// Players whose action bar holds a message about their own action, until when.
    quiet_until: HashMap<u128, std::time::Instant>,
}

static SERVER: LazyLock<Mutex<Server>> = LazyLock::new(Mutex::default);
/// The balance numbers. The spec's defaults for now; later a data file (docs/design/03-architecture.md).
static TUNING: LazyLock<Tuning> = LazyLock::new(Tuning::default);

fn server() -> MutexGuard<'static, Server> {
    SERVER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `f` on the level's state, if the server has read that level.
pub(super) fn with_level<R>(dimension: &str, f: impl FnOnce(&mut LevelNet) -> R) -> Option<R> {
    server().levels.get_mut(dimension).map(f)
}

/// A mod's own `native_set_block` reaches no hook: the level is read again on its next second.
pub(super) fn mark_level_stale(dimension: &str) {
    mark_stale(dimension);
}

/// The layout of `dimension` is to be read again before its next look.
fn mark_stale(dimension: &str) {
    server()
        .levels
        .entry(dimension.to_owned())
        .or_insert_with(LevelNet::new)
        .stale = true;
}

pub(super) fn read_slots(nbt: &Compound) -> Housing {
    match nbt.get(&JavaString::from(SLOTS_KEY)) {
        Some(Tag::ByteArray(bytes)) => housing::decode(bytes),
        _ => Housing::default(),
    }
}

pub(super) fn write_slots(nbt: &mut Compound, housing: &Housing) {
    nbt.insert(
        JavaString::from(SLOTS_KEY),
        Tag::ByteArray(housing::encode(housing).to_vec()),
    );
}

/// A translated line: `key` from the content mod's lang files, `fallback` (English) without them.
pub(super) fn line(key: &str, fallback: &str, args: &[Text]) -> Text {
    Text::translate_with_fallback(key, fallback, args)
}

fn number(n: impl ToString) -> Text {
    Text::literal(n.to_string())
}

/// How long a message about the player's own action stays before the looked-at diagnosis comes back.
const MESSAGE_HOLD: std::time::Duration = std::time::Duration::from_secs(2);

/// A message about something the player just did («встал в гнездо», «снят»…). It holds the action
/// bar for MESSAGE_HOLD, so the next diagnosis does not replace it at once (field test 2026-11-12).
pub(super) fn tell(game: &mut Game<'_>, player: u128, text: Text) {
    server()
        .quiet_until
        .insert(player, std::time::Instant::now() + MESSAGE_HOLD);
    show(game, player, text);
}

fn show(game: &mut Game<'_>, player: u128, text: Text) {
    if !game.native_action_bar(player, text) {
        log::warn!("rustcraft: no action bar line for {player:032x}: not on the server");
    }
}

/// Whether a recent message still holds the player's action bar.
fn holding_message(player: u128) -> bool {
    let now = std::time::Instant::now();
    let mut s = server();
    s.quiet_until.retain(|_, until| *until > now);
    s.quiet_until.contains_key(&player)
}

/// Whether `block` is one of RustCraft's blocks the magistral or the machines are made of.
pub(super) fn is_ours(block: &str) -> bool {
    [
        content::HOUSING,
        content::NODE,
        content::CRYSTAL,
        content::CELL,
        content::TERMINAL,
        content::TANK,
        content::MONORAIL,
    ]
    .contains(&block)
        || content::monorail_tier(block).is_some()
        || Machine::of(block).is_some()
}

/// Whether a block at `pos` of `dimension` touches a housing: a chest set or broken there may
/// make or end a joint.
fn touches_magistral(dimension: &str, pos: Pos) -> bool {
    with_level(dimension, |level| {
        Face::ALL
            .into_iter()
            .any(|f| level.layout.housings.contains_key(&step(pos, f)))
    })
    .unwrap_or(false)
}

// ---- Hooks ----

/// `native_level_tick_post`: count the level's ticks; every tick power and run the machines and
/// answer the joint windows, every transport::EVERY move items, and each ANALYSE_EVERY read the magistral again if it may
/// have changed, then show the players their diagnoses. The level's state leaves the shared map
/// while the game is called, so a hook the game runs meanwhile never waits on the lock.
pub(super) fn on_level_tick(game: &mut Game<'_>, dimension: &str) {
    let mut level = server()
        .levels
        .remove(dimension)
        .unwrap_or_else(LevelNet::new);
    level.ticks += 1;
    let second = level.ticks.is_multiple_of(ANALYSE_EVERY);
    if second
        && (std::mem::replace(&mut level.stale, false) || level.ticks.is_multiple_of(RESCAN_EVERY))
    {
        rescan(game, dimension, &mut level);
    }
    machines::tick(game, &mut level, &TUNING);
    window::tick(game, dimension, &mut level);
    if level.ticks.is_multiple_of(transport::EVERY) {
        transport::tick(game, dimension, &mut level, &TUNING);
    }
    if level.ticks.is_multiple_of(fluids::EVERY) {
        fluids::tick(game, &mut level, &TUNING);
    }
    storage::tick(game, dimension);
    monorail::tick(game, dimension, &TUNING);
    workers::tick(game, dimension, &level, &TUNING);
    base::tick(game, dimension, &level, &TUNING);
    {
        let mut s = server();
        // A hook may have marked the level stale while it was out of the map.
        if let Some(meanwhile) = s.levels.remove(dimension) {
            level.stale |= meanwhile.stale;
        }
        s.levels.insert(dimension.to_owned(), level);
    }
    if second {
        show_diagnoses(game, dimension);
    }
    // The self-test's scenarios, after the mod's own work of this tick (rustcraft_selftest).
    selftest::tick(game, dimension);
}

/// The balance numbers the server runs on.
pub(super) fn tuning() -> &'static Tuning {
    &TUNING
}

/// Reads the level's magistral, machines, storages and monorail again now, as the next look
/// would after a player's placing or breaking: the self-test's blocks reach no player's hook.
pub(super) fn rescan_now(game: &mut Game<'_>, dimension: &str) {
    let mut level = server()
        .levels
        .remove(dimension)
        .unwrap_or_else(LevelNet::new);
    rescan(game, dimension, &mut level);
    level.stale = false;
    let mut s = server();
    if let Some(meanwhile) = s.levels.remove(dimension) {
        level.stale |= meanwhile.stale;
    }
    s.levels.insert(dimension.to_owned(), level);
}

/// `native_use_item_on`: a channel clicked on a housing goes into its first free slot (M2) and
/// leaves the hand; the wrench with sneaking takes the last channel out and gives it back.
pub(super) fn on_use_item_on(
    game: &mut Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    claimed: &mut bool,
) {
    workers::on_use_item_on(game, stack, pos, claimed);
    let item: &str = &stack.item;
    let channel = content::channel_kind(item);
    if *claimed || (channel.is_none() && item != content::WRENCH) {
        return;
    }
    if game.native_block(pos) != content::HOUSING {
        return;
    }
    // None outside a player's hook on a server: a client predicting its own click does nothing.
    let Some(player) = game.native_acting_player() else {
        return;
    };
    if let Some(kind) = channel {
        *claimed = true;
        put_channel(game, &player, pos, kind);
    } else if item == content::WRENCH && player.sneaking {
        *claimed = true;
        take_channel(game, &player, pos);
    }
}

/// `native_right_click_block`: the wrench on a housing or a joint's block opens the joint
/// window (rustcraft_transport).
pub(super) fn on_right_click_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    base::on_right_click_block(game, pos, canceled);
    transport::on_right_click_block(game, pos, face, canceled);
}

/// `native_menu_clicked` and `native_menu_closed`: the joint window (rustcraft_window).
pub(super) fn on_menu_clicked(
    game: &mut Game<'_>,
    uuid: u128,
    window_id: i32,
    slot: i32,
    button: i32,
    mode: i32,
) {
    workers::on_menu_clicked(uuid, window_id, slot, mode);
    base::on_menu_clicked(uuid, window_id, slot, mode);
    window::on_menu_clicked(game, uuid, window_id, slot, button, mode);
}

pub(super) fn on_menu_closed(uuid: u128, window_id: i32) {
    workers::on_menu_closed(uuid, window_id);
    base::on_menu_closed(uuid, window_id);
    window::on_menu_closed(uuid, window_id);
}

/// `native_menu_button`: a machine window's mode button (rustcraft_machines).
pub(super) fn on_menu_button(entity: &mut ModBlockEntity, id: i32) {
    machines::on_menu_button(entity, id, &TUNING);
}

/// `native_menu_button_v2`: the same press with the cursor's stack, for the autocrafter's pattern
/// and the terminal's places.
pub(super) fn on_menu_button_v2(
    game: &mut Game<'_>,
    pos: Pos,
    entity: &mut ModBlockEntity,
    id: i32,
    carried: &mut Option<ItemStack>,
) {
    machines::on_menu_button_v2(entity, id, carried, &TUNING);
    storage::on_button(game, pos, entity, id, carried);
    monorail::on_button(game, pos, entity, id);
}

/// `native_container_accepts`: what a machine's input takes from a hopper or the net.
pub(super) fn on_container_accepts(
    entity: &ModBlockEntity,
    slot: usize,
    stack: &ItemStack,
    cancel: &mut bool,
) {
    machines::on_container_accepts(entity, slot, stack, cancel, &TUNING);
}

/// `native_place_block` (native_hooks.rs:8288-8299): one of our blocks placed changes the
/// magistral or the machines, and any block placed beside a housing may be a new joint.
pub(super) fn on_place_block(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    base::on_place_block(game, stack, pos);
    let Some(player) = game.native_acting_player() else {
        mark_all_stale();
        return;
    };
    if is_ours(&stack.item) || touches_magistral(&player.dimension, pos) {
        mark_stale(&player.dimension);
    }
    if content::monorail_tier(&stack.item).is_some() || &*stack.item == content::STATION {
        monorail::touched(&player.dimension, pos);
    }
}

/// `native_destroy_block`: one of our blocks, or a block beside a housing, broken changes the
/// magistral or its joints; a housing broken in survival gives its channels to the player (the
/// engine drops only the block's loot and removes the block entity with what it held), and a
/// vanilla container's slice-2 joint mode goes with it.
pub(super) fn on_destroy_block(game: &mut Game<'_>, pos: [i32; 3]) {
    let block = game.native_block(pos);
    let Some(player) = game.native_acting_player() else {
        return;
    };
    base::on_destroy_block(game, pos, &player.dimension, player.game_mode == CREATIVE);
    if !is_ours(&block) {
        if touches_magistral(&player.dimension, pos) {
            mark_stale(&player.dimension);
            transport::forget(game, pos);
        }
        return;
    }
    mark_stale(&player.dimension);
    if block == content::MONORAIL || block == content::STATION {
        monorail::touched(&player.dimension, pos);
    }
    if block == content::CELL {
        storage::on_cell_broken(game, &player.dimension, pos);
    }
    if block != content::HOUSING || player.game_mode == CREATIVE {
        return;
    }
    let Some(held) = game
        .native_mod_block_entity(pos)
        .map(|entity| read_slots(&entity.nbt))
    else {
        return;
    };
    for channel in held.slots.into_iter().flatten() {
        let item = content::channel_item(channel.kind);
        if game.native_give_items(item, 1) == 0 {
            drop_item(game, &player.dimension, pos, item);
        }
    }
}

/// `native_chunk_load` and `native_chunk_unload`: housings may have come or gone with it.
pub(super) fn on_chunk_changed(dimension: &str) {
    mark_stale(dimension);
}

/// `native_level_unload`: forget the level.
pub(super) fn on_level_unload(dimension: &str) {
    server().levels.remove(dimension);
    monorail::on_level_unload(dimension);
    workers::on_level_unload(dimension);
}

// ---- What the hooks do ----

fn mark_all_stale() {
    for level in server().levels.values_mut() {
        level.stale = true;
    }
}

/// Reads the level's magistral, machines and recipes again and rebuilds its nets, circuits and
/// joints.
fn rescan(game: &mut Game<'_>, dimension: &str, level: &mut LevelNet) {
    let found = scan(game, dimension, level);
    storage::rescan(game, dimension, &found.cells, &found.terminals);
    let stations: Vec<Pos> = level
        .machines
        .iter()
        .filter(|(_, m)| **m == Machine::Station)
        .map(|(p, _)| *p)
        .collect();
    monorail::rescan(game, dimension, &found.rails, &stations);
    level.topology = topology::analyse(&level.layout, &TUNING);
    level.recipes = machines::load_recipes(game);
    machines::teach_slot_rules(game, &level.recipes);
    let ports: BTreeMap<Pos, Port> = level
        .crystals
        .iter()
        .map(|&p| (p, Port::Source))
        .chain(level.machines.iter().map(|(&p, m)| {
            let port = if m.is_source() {
                Port::Source
            } else if *m == Machine::Battery {
                Port::Storage
            } else {
                Port::Machine
            };
            (p, port)
        }))
        .collect();
    level.power.circuits = machines::circuits(
        &level.topology,
        &level.layout,
        &ports,
        &level.joints,
        &TUNING,
    );
    power::rescan(level);
    transport::rebuild(game, dimension, level, &TUNING);
    fluids::rebuild(game, level, found.tanks, &TUNING);
}

/// The blocks of the storages, the tanks and the monorail a scan found.
struct Found {
    cells: BTreeSet<Pos>,
    terminals: BTreeSet<Pos>,
    tanks: BTreeSet<Pos>,
    rails: Vec<Pos>,
}

/// The magistral and the machines of the level in place, from its block entities: each
/// housing's slots and joint settings from its saved compound, each node's tier from its state.
fn scan(game: &Game<'_>, dimension: &str, level: &mut LevelNet) -> Found {
    let mut found = Found {
        cells: BTreeSet::new(),
        terminals: BTreeSet::new(),
        tanks: BTreeSet::new(),
        rails: Vec::new(),
    };
    let mut layout = Layout::default();
    level.crystals.clear();
    level.machines.clear();
    // A mod block's entry is named after its block (native_diagnostics.rs:76-88, 207-218).
    for (pos, kind) in game.native_level_block_entities(dimension) {
        if kind == content::HOUSING {
            let slots = game
                .native_mod_block_entity(pos)
                .map(|entity| read_slots(&entity.nbt))
                .unwrap_or_default();
            layout.housings.insert(pos, slots);
        } else if kind == content::NODE {
            let tier = game
                .native_block_property(pos, content::TIER)
                .and_then(|tier| tier.parse().ok())
                .unwrap_or(FIRST_TIER);
            layout.nodes.insert(pos, Node { tier });
        } else if kind == content::CRYSTAL {
            level.crystals.insert(pos);
        } else if let Some(machine) = Machine::of(&kind) {
            level.machines.insert(pos, machine);
        } else if kind == content::CELL {
            found.cells.insert(pos);
        } else if kind == content::TERMINAL {
            found.terminals.insert(pos);
        } else if kind == content::TANK {
            found.tanks.insert(pos);
        } else if kind == content::MONORAIL {
            found.rails.push(pos);
        }
    }
    level.joints = window::all_joints(game, layout.housings.keys().copied());
    level.layout = layout;
    found
}

fn put_channel(game: &mut Game<'_>, player: &NativeServerPlayer, pos: Pos, kind: ChannelKind) {
    let channel = Channel {
        kind,
        tier: FIRST_TIER,
    };
    // None: no block entity there; Some(None): all four slots taken.
    let placed = game.native_with_mod_block_entity(pos, |entity| {
        let (next, slot) = housing::insert(read_slots(&entity.nbt), channel)?;
        write_slots(&mut entity.nbt, &next);
        Some(slot)
    });
    match placed {
        None => log::warn!("rustcraft: the housing at {pos:?} has no block entity"),
        Some(None) => tell(
            game,
            player.uuid,
            line("rustcraft.housing.full", "All four slots are taken", &[]),
        ),
        Some(Some(slot)) => {
            if player.game_mode != CREATIVE {
                game.native_take_items(content::channel_item(kind), 1);
            }
            mark_stale(&player.dimension);
            let text = line(
                "rustcraft.housing.channel_set",
                "%s went into slot %s",
                &[kind_name(kind), number(slot + 1)],
            );
            tell(game, player.uuid, text);
        }
    }
}

fn take_channel(game: &mut Game<'_>, player: &NativeServerPlayer, pos: Pos) {
    let Some(held) = game
        .native_mod_block_entity(pos)
        .map(|entity| read_slots(&entity.nbt))
    else {
        return;
    };
    let Some((_, _, last)) = housing::remove_last(held) else {
        let text = line("rustcraft.housing.empty", "The housing has no channel", &[]);
        tell(game, player.uuid, text);
        return;
    };
    // Check the room first, so a full inventory leaves the channel where it is.
    let item = content::channel_item(last.kind);
    if game.native_room_for(item) == 0 {
        let text = line("rustcraft.housing.no_room", "No room for the channel", &[]);
        tell(game, player.uuid, text);
        return;
    }
    let taken = game
        .native_with_mod_block_entity(pos, |entity| {
            let (next, slot, channel) = housing::remove_last(read_slots(&entity.nbt))?;
            write_slots(&mut entity.nbt, &next);
            window::forget_slot(&mut entity.nbt, slot);
            Some((slot, channel))
        })
        .flatten();
    let Some((slot, channel)) = taken else {
        return;
    };
    game.native_give_items(content::channel_item(channel.kind), 1);
    mark_stale(&player.dimension);
    let text = line(
        "rustcraft.housing.channel_taken",
        "%s taken from slot %s",
        &[kind_name(channel.kind), number(slot + 1)],
    );
    tell(game, player.uuid, text);
}

/// An item entity of one `item` at the middle of the block at `pos` (`/summon`'s compound).
fn drop_item(game: &mut Game<'_>, dimension: &str, pos: Pos, item: &str) {
    let stack = Tag::compound([
        ("id", Tag::String(JavaString::from(item))),
        ("count", Tag::Int(1)),
    ]);
    let nbt = Compound::from([(JavaString::from("Item"), stack)]);
    let at = pos.map(|c| f64::from(c) + 0.5);
    if game
        .native_summon(dimension, "minecraft:item", at, nbt)
        .is_none()
    {
        log::warn!("rustcraft: {item} from the housing at {pos:?} was lost");
    }
}

/// Each player of the level in place who looks at a housing, a machine or a joint's block gets
/// its diagnosis over the hotbar.
fn show_diagnoses(game: &mut Game<'_>, dimension: &str) {
    let players: Vec<NativeServerPlayer> = game
        .native_server_players()
        .into_iter()
        .filter(|p| p.dimension == dimension && p.game_mode != SPECTATOR)
        .collect();
    for player in players {
        if holding_message(player.uuid) {
            continue;
        }
        let Some((pos, block)) = looked_at(game, &player) else {
            continue;
        };
        let text = {
            let game = &*game;
            with_level(dimension, |level| {
                diagnosis_of(game, dimension, level, pos, &block)
            })
            .flatten()
        };
        if let Some(text) = text {
            show(game, player.uuid, text);
        }
    }
}

/// What the block at `pos` shows: a housing its line and slots, a machine its state (and its
/// joints' directions when it is behind a joint), a cell or a terminal its storage, a tank its
/// fluid, a vanilla block behind a joint its joints' directions and «Приёмник полон».
fn diagnosis_of(
    game: &Game<'_>,
    dimension: &str,
    level: &LevelNet,
    pos: Pos,
    block: &str,
) -> Option<Text> {
    if block == content::HOUSING {
        return housing::diagnose(&level.topology, &level.layout, pos).map(|d| diagnosis_text(&d));
    }
    if storage::is_storage_block(block) {
        return storage::diagnosis(game, dimension, pos, block);
    }
    if block == content::MONORAIL || block == content::STATION {
        return monorail::diagnosis(game, dimension, pos, block, &TUNING);
    }
    if let Some(text) = power::diagnosis(game, level, pos, block, &TUNING) {
        return Some(text);
    }
    if let Some(text) = mining::diagnosis(dimension, pos, block) {
        return Some(text);
    }
    if let Some(text) = base::diagnosis(game, dimension, pos, block, &TUNING) {
        return Some(text);
    }
    let fluid_joint = level.fluids.joint_blocks.get(&pos);
    if block == content::TANK {
        let mut text = fluids::tank_text(game, level, pos, &TUNING);
        if let Some(modes) = fluid_joint {
            text.push(Text::literal(" · "));
            text.push(transport::joint_text(modes));
        }
        return Some(text);
    }
    let joint = level.transport.joint_blocks.get(&pos);
    if Machine::of(block).is_some() {
        let mut text = level.power.status.get(&pos).map_or_else(
            || machine_text(Machine::of(block), Shown::Idle),
            |s| machine_text(Some(s.machine), s.shown),
        );
        if let Some(modes) = joint.or(fluid_joint) {
            text.push(Text::literal(" · "));
            text.push(transport::joint_text(modes));
        }
        if let Some(fluid) = level.fluids.no_lane(pos, level.ticks) {
            text.push(Text::literal(" · "));
            text.push(fluids::no_lane_text(fluid));
        }
        return Some(text);
    }
    let mut text = transport::joint_text(joint?);
    if level.transport.is_full(pos, level.ticks) {
        text.push(Text::literal(" · "));
        text.push(line("rustcraft.joint.receiver_full", "Receiver full", &[]));
    }
    Some(text)
}

/// The block the player looks at and its name, the first their look meets past air and fluids.
fn looked_at(game: &Game<'_>, player: &NativeServerPlayer) -> Option<(Pos, String)> {
    let [x, y, z] = player.position;
    let eyes = [x, y + f64::from(player.eye_height), z];
    let look = ray::look_direction(player.yaw, player.pitch);
    for pos in ray::blocks_along(eyes, look, LOOK_REACH) {
        let block = game.native_block(pos);
        if SEE_THROUGH.contains(&block.as_str()) {
            continue;
        }
        return Some((pos, block));
    }
    None
}

fn kind_name(kind: ChannelKind) -> Text {
    match kind {
        ChannelKind::Item => line("rustcraft.channel.item", "Item channel", &[]),
        ChannelKind::Fluid => line("rustcraft.channel.fluid", "Fluid channel", &[]),
        ChannelKind::Energy => line("rustcraft.channel.energy", "Energy channel", &[]),
    }
}

fn kind_short(kind: ChannelKind) -> Text {
    match kind {
        ChannelKind::Item => line("rustcraft.slot.item", "items", &[]),
        ChannelKind::Fluid => line("rustcraft.slot.fluid", "fluid", &[]),
        ChannelKind::Energy => line("rustcraft.slot.energy", "energy", &[]),
    }
}

/// A machine's diagnosis (A4): «Работает», «Не хватает энергии: 50%», «Нет энергии»,
/// «Готовое некуда отдать», «Нет сырья».
fn machine_text(machine: Option<Machine>, shown: Shown) -> Text {
    let kind = machine.map(Machine::kind);
    match shown {
        Shown::Working if kind == Some(Kind::Generator) => {
            line("rustcraft.generator.working", "Burning", &[])
        }
        Shown::Idle if kind == Some(Kind::Pump) => {
            line("rustcraft.pump.no_source", "No fluid under the pump", &[])
        }
        Shown::OutputFull if kind == Some(Kind::Pump) => {
            line("rustcraft.pump.full", "Nowhere to give the fluid", &[])
        }
        Shown::Working => line("rustcraft.machine.working", "Working", &[]),
        Shown::Slowed { percent } => line(
            "rustcraft.machine.slowed",
            "Not enough energy: %s",
            &[Text::literal(format!("{percent}%"))],
        ),
        Shown::NoEnergy => line("rustcraft.machine.no_energy", "No energy", &[]),
        Shown::OutputFull => line(
            "rustcraft.machine.output_full",
            "No room for the product",
            &[],
        ),
        Shown::Idle if matches!(kind, Some(Kind::Alloy | Kind::Crafter)) => {
            line("rustcraft.machine.no_ingredients", "No ingredients", &[])
        }
        Shown::Idle => line("rustcraft.machine.idle", "Nothing to work on", &[]),
        Shown::NoFuel => line("rustcraft.machine.no_fuel", "No fuel", &[]),
        Shown::OnFuel => line("rustcraft.machine.on_fuel", "Working on fuel", &[]),
        Shown::NoDemand => line(
            "rustcraft.generator.no_demand",
            "Idle: no machine needs energy",
            &[],
        ),
    }
}

fn diagnosis_text(diagnosis: &Diagnosis) -> Text {
    match *diagnosis {
        Diagnosis::NoNode => line("rustcraft.diagnosis.no_node", "No magistral node", &[]),
        Diagnosis::TooFar { distance, limit } => line(
            "rustcraft.diagnosis.too_far",
            "Beyond the node's range: %s of %s",
            &[number(distance), number(limit)],
        ),
        Diagnosis::Works {
            distance,
            limit,
            line_len,
            slots,
        } => {
            let mut text = line(
                "rustcraft.diagnosis.works",
                "Line of %s · node %s of %s ·",
                &[number(line_len), number(distance), number(limit)],
            );
            for (i, slot) in slots.iter().enumerate() {
                text.push(Text::literal(" "));
                text.push(slot_text(i + 1, *slot));
            }
            text
        }
    }
}

/// One slot of a working housing: «1: предметы ×12», the channel and how many housings its net
/// joins (M3).
fn slot_text(number_shown: usize, slot: SlotState) -> Text {
    match slot {
        SlotState::Empty => line("rustcraft.slot.empty", "%s: —", &[number(number_shown)]),
        SlotState::Channel { kind, net_len, .. } => line(
            "rustcraft.slot.channel",
            "%s: %s ×%s",
            &[number(number_shown), kind_short(kind), number(net_len)],
        ),
    }
}
