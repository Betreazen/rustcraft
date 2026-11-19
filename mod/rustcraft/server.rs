// Server half of RustCraft. It keeps each level's magistral in memory (housings with their tiers
// and joint settings, nodes with their tiers, crystals and machines), rebuilt from the block
// entities whenever something may have changed. Round 31: the three channels are built into every
// housing; a housing saved by 0.19 with channels set by hand gets them built in when its level is
// read (rustcraft_rules::net::migrate). Every tick it powers and runs the machines
// (rustcraft_machines) and answers the joint windows (rustcraft_window), every few ticks it moves
// items along the item nets (rustcraft_transport) and fluids along the fluid nets
// (rustcraft_fluids), and once a second it shows each player the diagnosis of the housing,
// machine, storage, tank or joint they look at. The storages (rustcraft_storage) are read again
// with the magistral and saved when they change.
//
// Every helper here reads the level in place: inside a player's hook it is that player's
// (`native_acting_player`, SRC/crates/horizon-client/src/game/native_server.rs:656-661), in
// `native_level_tick_post` it is the level that just ticked (`enter_level` before the hook,
// SRC/crates/horizon-client/src/game/levels.rs:525-560).
use crate::game::{
    Game, rustcraft_backpack as backpack, rustcraft_base as base,
    rustcraft_content::{self as content, Kind, Machine},
    rustcraft_fluids as fluids, rustcraft_handbook as handbook, rustcraft_machines as machines,
    rustcraft_mining as mining,
    rustcraft_mixer as mixer, rustcraft_monorail as monorail, rustcraft_orders as orders,
    rustcraft_power as power, rustcraft_selftest as selftest, rustcraft_storage as storage,
    rustcraft_tablet as tablet,
    rustcraft_transport as transport, rustcraft_window as window, rustcraft_workers as workers,
    rustcraft_zipline as zipline,
};
use horizon_client::{
    containers::ModBlockEntity, items::ItemStack, mod_api::NativeServerPlayer, text::Text,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    housing::{self, Diagnosis},
    machines::processor::Shown,
    net::{
        joints::Port,
        migrate::{self, OldSlots},
        settings::HousingJoints,
        topology::{self, Housing, Layout, Node, Topology},
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
/// Round 31: a housing saved by 0.19 kept the channels set into its four slots under this key
/// (rustcraft_rules::net::migrate's bytes); a housing without it has its channels built in.
const OLD_SLOTS_KEY: &str = "slots";
/// `NativeServerPlayer::game_mode` of creative and spectator.
const CREATIVE: i32 = 1;
const SPECTATOR: i32 = 3;
/// A node or a housing whose state names no tier is of the first.
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
    /// Slice 12: whether each machine's moving parts move, as clients were last told.
    pub anim: BTreeMap<Pos, bool>,
    /// Slice 15 (O4): the autocrafters' temporary pages from orders (rustcraft_orders).
    pub temp_pages: BTreeMap<Pos, crate::game::rustcraft_crafter::TempPage>,
    /// s18/pipe: the energy ledger and what each consumer got, as the machines' tick last noted
    /// them once a second, for the node's window (rustcraft_node_window).
    pub pipe18_energy: Option<(rustcraft_rules::net::energy::Ledger, BTreeMap<Pos, f64>)>,
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
            anim: BTreeMap::new(),
            temp_pages: BTreeMap::new(),
            pipe18_energy: None,
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

/// Round 31: the channels a housing saved by 0.19 held in its four slots, if it is one.
pub(super) fn old_slots(nbt: &Compound) -> Option<OldSlots> {
    match nbt.get(&JavaString::from(OLD_SLOTS_KEY)) {
        Some(Tag::ByteArray(bytes)) => Some(migrate::decode(bytes)),
        _ => None,
    }
}

/// 0.21 (spec 45, CR2): an old channel item a player carries becomes its alloy in its slot, two
/// channels an ingot and an odd one a whole ingot (rustcraft_rules::net::old_channels).
pub(super) fn on_inventory_tick(game: &mut Game<'_>, slot: i32) {
    let Some(stack) = game.native_carried_stack(slot) else {
        return;
    };
    if let Some((alloy, n)) = rustcraft_rules::net::old_channels::convert(&stack.item, stack.count) {
        *stack = ItemStack::new(alloy, n);
    }
}

/// Round 31: a housing written as 0.19 saved it (the self-test's old housings).
pub(super) fn write_old_slots(nbt: &mut Compound, slots: &OldSlots) {
    nbt.insert(
        JavaString::from(OLD_SLOTS_KEY),
        Tag::ByteArray(migrate::encode(slots).to_vec()),
    );
}

/// The housing at `pos` as its state says: of the tier its `tier` names, the first without one.
pub(super) fn housing_at(game: &Game<'_>, pos: Pos) -> Housing {
    let tier = game
        .native_block_property(pos, content::TIER)
        .and_then(|tier| tier.parse().ok())
        .unwrap_or(FIRST_TIER);
    Housing::new(tier)
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

// ---- Slice 18, final: the player whose click a hook hears ----
//
// Every handler asks `acting` instead of `native_acting_player`. In play both are the same. The
// self-test's clicks go through the game's own use as `native_use_as_player`'s fake player
// (rustcraft_selftest::click_block), which has no uuid of its own, so `native_acting_player`
// reads None for it (game/native_server.rs:661-671); while the bench's click runs, `acting`
// answers with that fake player instead, so the click meets every handler a player's would.

/// The fake player of the bench's click under way: its uuid (the one `native_give_stack` fills
/// the fake inventory for) and its dimension.
static BENCH_ACTOR: Mutex<Option<(u128, String)>> = Mutex::new(None);

/// The player whose action the hook hears: the acting player, or the bench's fake player.
pub(super) fn acting(game: &Game<'_>) -> Option<NativeServerPlayer> {
    if let Some(p) = game.native_acting_player() {
        return Some(p);
    }
    let (uuid, dimension) = BENCH_ACTOR.lock().unwrap_or_else(PoisonError::into_inner).clone()?;
    Some(NativeServerPlayer {
        uuid,
        name: "rustcraft_selftest".to_owned(),
        permission: 0,
        dimension,
        position: [0.0; 3],
        game_mode: 0,
        health: 20.0,
        yaw: 0.0,
        pitch: 0.0,
        entity_id: -1,
        address: String::new(),
        latency: 0,
        protocol: 0,
        xp_level: 0,
        xp_progress: 0.0,
        on_ground: true,
        flying: false,
        may_fly: false,
        invulnerable: false,
        walk_speed: 0.1,
        fly_speed: 0.05,
        sneaking: game.native_sneaking(),
        eye_height: 1.62,
        selected_slot: 0,
        nickname: None,
        hidden: None,
    })
}

/// Runs `f` (a fake player's use) with `acting` answering for the fake player `uuid`.
pub(super) fn as_bench_actor<R>(uuid: u128, dimension: &str, f: impl FnOnce() -> R) -> R {
    *BENCH_ACTOR.lock().unwrap_or_else(PoisonError::into_inner) = Some((uuid, dimension.to_owned()));
    let out = f();
    *BENCH_ACTOR.lock().unwrap_or_else(PoisonError::into_inner) = None;
    out
}

fn show(game: &mut Game<'_>, player: u128, text: Text) {
    if !game.native_action_bar(player, text) {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: no action bar line for {player:032x}: not on the server");
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
    // 0.18.1: druses for old overworld chunks, a few a tick (rustcraft_crystals).
    crate::game::rustcraft_crystals::retro_tick(game, dimension);
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
    // Slice 15: the orders' jobs and a round of every storage's queue, once a second.
    if second {
        orders::tick(game, dimension, &mut level);
        // 0.20 (round 31): the owners' tree summaries, and the machines' tools to and from the
        // repair stands.
        crate::game::rustcraft_perks::refresh(game);
        crate::game::rustcraft_trips::tick(game, dimension, &level);
        // Wave 2: the plain item flow drawn in the housings (M5).
        crate::game::rustcraft_pipe18::show_flow(game, dimension, &level);
    }
    storage::tick(game, dimension);
    monorail::tick(game, dimension, &TUNING);
    zipline::tick(game, dimension);
    workers::tick(game, dimension, &level, &TUNING);
    base::tick(game, dimension, &level, &TUNING);
    tablet::tick(game, dimension, &level);
    // Round 32 (spec 33): the «Сеть» pages the clients asked for.
    crate::game::rustcraft_terminal_net::tick(game, dimension, &level);
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
        // s18/pipe: the node windows' presses and views.
        crate::game::rustcraft_node_window::tick(game, dimension);
        // The diagnostics journal: status changes of a batch of machines and of the workers.
        crate::game::rustcraft_debug::tick(game, dimension);
        // H2: the first alloy a player carries brings them the engineer's handbook.
        handbook::tick(game, dimension);
    }
    // The self-test's scenarios, after the mod's own work of this tick (rustcraft_selftest).
    backpack::tick(game);
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

/// `native_use_item_on`: the wrench with sneaking takes the housing off as its tier's item
/// (s18/pipe); round 31: a housing of a higher tier clicked on a housing takes its place
/// (rustcraft_pipe18).
pub(super) fn on_use_item_on(
    game: &mut Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    claimed: &mut bool,
) {
    // s18/pipe: a node of a higher tier takes a node's place (U2).
    crate::game::rustcraft_pipe18::on_use_item_on(game, stack, pos, claimed);
    workers::on_use_item_on(game, stack, pos, claimed);
    let item: &str = &stack.item;
    // Spec 26: the wrench with sneaking lifts suspicious sand or gravel (rustcraft_utility).
    if !*claimed
        && item == content::WRENCH
        && let Some(player) = acting(game)
        && rustcraft_rules::utility::found::wrench_lifts(&game.native_block(pos), player.sneaking)
    {
        *claimed = true;
        crate::game::rustcraft_utility::lift_with_wrench(game, player.uuid, &player.dimension, pos);
        return;
    }
    // 0.21 (spec 42, FT4): the sneaking wrench on an empty tank frees it for any fluid.
    if !*claimed
        && item == content::WRENCH
        && game.native_block(pos) == content::TANK
        && let Some(player) = acting(game)
        && player.sneaking
    {
        *claimed = true;
        fluids::wrench_frees_tank(game, player.uuid, &player.dimension, pos);
        return;
    }
    // Slice 12: the facade on a housing or a node; the wrench pairs portal docks.
    if !*claimed
        && let Some(player) = acting(game)
    {
        if item == content::WRENCH && game.native_block(pos) == "rustcraft:portal_dock" {
            *claimed = true;
            crate::game::rustcraft_builder::wrench_dock(game, player.uuid, pos);
            return;
        }
        if crate::game::rustcraft_builder::on_use_item_on(game, player.uuid, player.sneaking, stack, pos) {
            *claimed = true;
            // Wave 2: what an exploded housing would drop of its facade.
            crate::game::rustcraft_pipe18::remember_facade(game, &player.dimension, pos);
            return;
        }
    }
    if *claimed || item != content::WRENCH || game.native_block(pos) != content::HOUSING {
        return;
    }
    // None outside a player's hook on a server: a client predicting its own click does nothing.
    let Some(player) = acting(game) else {
        return;
    };
    if player.sneaking {
        // s18/pipe (guide `facade`): after the facade, the sneaking wrench takes the housing itself.
        *claimed = true;
        crate::game::rustcraft_pipe18::lift_housing(game, &player, pos);
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
    zipline::on_right_click_block(game, pos, canceled);
    // Slice 18, wave 2: a rail clicked with a rail of another tier takes that tier.
    crate::game::rustcraft_monorail::on_right_click_block(game, pos, canceled);
    // s18/pipe: an empty hand opens the node's window.
    crate::game::rustcraft_node_window::on_right_click_block(game, pos, face, canceled);
    base::on_right_click_block(game, pos, canceled);
    transport::on_right_click_block(game, pos, face, canceled);
    crate::game::rustcraft_crystals::on_right_click_block(game, pos, face, canceled);
    // Slice 18, stream 3: after the guards above, a higher tier's item on a lower tier machine
    // replaces it (U2).
    crate::game::rustcraft_tiers::on_right_click_block(game, pos, canceled);
    // Slice 18, stream 3: a bucket on a tank pours or fills.
    fluids::on_right_click_block(game, pos, canceled);
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
    tablet::on_menu_clicked(uuid, window_id, slot, mode);
    crate::game::rustcraft_filter::on_menu_clicked(uuid, window_id, slot, mode);
    window::on_menu_clicked(game, uuid, window_id, slot, button, mode);
    // Slice 18, base and gear: the starsteel backpack's page arrows.
    crate::game::rustcraft_backpack::on_menu_clicked(game, uuid, window_id, slot);
}

pub(super) fn on_menu_closed(uuid: u128, window_id: i32) {
    workers::on_menu_closed(uuid, window_id);
    base::on_menu_closed(uuid, window_id);
    tablet::on_menu_closed(uuid, window_id);
    crate::game::rustcraft_filter::on_menu_closed(uuid, window_id);
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
    base::on_sheet_button(entity, id, carried);
    crate::game::rustcraft_utility::on_button(game, pos, entity, id);
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
    // Slice 18, stream 3: a trash's joints start empty and last (guide `trash`).
    crate::game::rustcraft_utility::on_placed(game, pos);
    // s18/pipe: a machine lifted by the wrench gets its contents back.
    crate::game::rustcraft_pipe18::on_place_block(game, stack, pos);
    // Round 31: a housing of a higher tier placed takes its item's tier.
    crate::game::rustcraft_pipe18::housing_placed(game, stack, pos);
    let Some(player) = acting(game) else {
        mark_all_stale();
        return;
    };
    // rustcraft_debug: the journal notes a RustCraft block placed by a player.
    if stack.item.starts_with("rustcraft:") {
        crate::game::rustcraft_debug::record(
            Some(&player.dimension),
            Some(pos),
            "placed",
            &format!("{} by {}", stack.item, player.name),
        );
    }
    // ST5: a block placed where the vein grew one is no grown block, unless it is a grown one
    // placed again; a lifted suspicious block gets its loot back (spec 26).
    crate::game::rustcraft_utility::forget_grown(game, &player.dimension, pos);
    crate::game::rustcraft_utility::on_place_suspicious(game, &player.dimension, stack, pos);
    // Slice 18: a lifted projector's plan back into it.
    crate::game::rustcraft_projector::on_place(game, stack, pos);
    if is_ours(&stack.item) || touches_magistral(&player.dimension, pos) {
        mark_stale(&player.dimension);
    }
    if content::monorail_tier(&stack.item).is_some() || &*stack.item == content::STATION {
        monorail::touched(&player.dimension, pos);
    }
}

/// `native_destroy_block`: one of our blocks, or a block beside a housing, broken changes the
/// magistral or its joints; a housing broken in survival gives its facade to the player (the
/// engine drops only the block's loot, its own tier's item, and removes the block entity with what
/// it held), and a vanilla container's slice-2 joint mode goes with it.
pub(super) fn on_destroy_block(game: &mut Game<'_>, pos: [i32; 3]) {
    let block = game.native_block(pos);
    let Some(player) = acting(game) else {
        return;
    };
    // rustcraft_debug: the journal notes a RustCraft block broken by a player.
    if block.starts_with("rustcraft:") {
        crate::game::rustcraft_debug::record(
            Some(&player.dimension),
            Some(pos),
            "broken",
            &format!("{block} by {}", player.name),
        );
    }
    base::on_destroy_block(game, pos, &player.dimension, player.game_mode == CREATIVE);
    // Slice 18, stream 3: a drawn-window machine's modules, queue and reserve fall out.
    machines::on_destroyed(game, &player.dimension, pos, player.game_mode == CREATIVE);
    crate::game::rustcraft_utility::forget_grown(game, &player.dimension, pos);
    if block == rustcraft_rules::traversal::ANCHOR {
        zipline::on_anchor_broken(game, &player.dimension, pos);
        return;
    }
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
    // Slice 15: a terminal broken in survival gives back its recipe book.
    if block == content::TERMINAL && player.game_mode != CREATIVE {
        let book = crate::game::rustcraft_machines::entity(game, pos)
            .is_some_and(|e| orders::has_book(&e.nbt));
        if book && game.native_give_items(orders::book_item(), 1) == 0 {
            drop_item(game, &player.dimension, pos, orders::book_item());
        }
        // Slice 18: and its stack and filter modules.
        storage::give_back_sockets(game, &player.dimension, pos, player.uuid);
    }
    if block == content::HOUSING {
        // s18/pipe: its channels are dealt with here, not as a loss at the next rescan.
        crate::game::rustcraft_pipe18::housing_handled(&player.dimension, pos);
    }
    // Wave 2: a node broken in survival gives back its facade, its look and its line-boost modules.
    if block == content::NODE && player.game_mode != CREATIVE {
        for item in crate::game::rustcraft_node_window::node_returns(game, pos) {
            if game.native_give_items(&item, 1) == 0 {
                drop_item(game, &player.dimension, pos, &item);
            }
        }
    }
    if block != content::HOUSING || player.game_mode == CREATIVE {
        return;
    }
    // Round 31: a housing of 0.19 broken before its level was read again still holds the
    // channels set into it. 0.21 (spec 45, CR4): they come back as their alloy.
    let old = crate::game::rustcraft_machines::entity(game, pos).and_then(|entity| old_slots(&entity.nbt));
    let parts = rustcraft_rules::net::old_channels::parts_of_channels(old.into_iter().flatten().flatten());
    for (alloy, n) in parts {
        for _ in 0..n {
            if game.native_give_items(alloy, 1) == 0 {
                drop_item(game, &player.dimension, pos, alloy);
            }
        }
    }
    // s18/pipe: the facade and the block of its look come back too (guide `facade`).
    for item in crate::game::rustcraft_pipe18::facade_items(game, pos) {
        if game.native_give_items(&item, 1) == 0 {
            drop_item(game, &player.dimension, pos, &item);
        }
    }
}

/// `native_chunk_load` and `native_chunk_unload`: housings may have come or gone with it.
pub(super) fn on_chunk_changed(dimension: &str) {
    mark_stale(dimension);
}

/// `native_level_unload`: forget the level; the orders keep its temporary pages for its jobs.
pub(super) fn on_level_unload(dimension: &str) {
    let gone = server().levels.remove(dimension);
    orders::on_level_unload(dimension, gone.map(|l| l.temp_pages).unwrap_or_default());
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
    // s18/pipe: housings gone without a player drop their channels.
    let before = std::mem::take(&mut level.layout.housings);
    let found = scan(game, dimension, level);
    crate::game::rustcraft_pipe18::drop_lost(game, dimension, &before, &level.layout);
    storage::rescan(game, dimension, &found.cells, &found.terminals);
    let stations: Vec<Pos> = level
        .machines
        .iter()
        .filter(|(_, m)| **m == Machine::Station)
        .map(|(p, _)| *p)
        .collect();
    monorail::rescan(game, dimension, &found.rails, &stations);
    zipline::rescan(game, dimension, &found.anchors);
    if !found.old_housings.is_empty() {
        migrate_old(game, dimension, level, &found.old_housings);
        level.joints = window::all_joints(game, level.layout.housings.keys().copied());
    }
    level.topology = topology::analyse(&level.layout, &TUNING);
    // Slice 18 (S6): extra terminals anywhere on the storage's magistral.
    storage::link_far_terminals(game, dimension, &level.topology);
    level.recipes = machines::load_recipes(game);
    orders::forget_book();
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
    // The machines may have changed: every idle one looks at its slots again.
    level.power.idle.clear();
    // Slice 18 (cross): the nodes' line-boost modules for the energy circuits.
    let boosts = crate::game::rustcraft_node_window::node_modules(game, level.layout.nodes.keys().copied());
    level.power.circuits = machines::circuits(
        &level.topology,
        &level.layout,
        &ports,
        &level.joints,
        &boosts,
        &TUNING,
    );
    power::rescan(level);
    // Round 31: the machines', tanks', terminals' and cells' connectors on the sides housings touch.
    let linked: Vec<Pos> = level
        .machines
        .iter()
        .filter(|(_, m)| rustcraft_rules::connectors::has(m.block()))
        .map(|(p, _)| *p)
        .chain(found.cells.iter().chain(&found.terminals).chain(&found.tanks).copied())
        .collect();
    crate::game::rustcraft_connectors::update(game, &level.layout, linked);
    transport::rebuild(game, dimension, level, &TUNING);
    // s18/pipe: the wrench's bottlenecks, told to clients.
    crate::game::rustcraft_pipe18::mark_weak(game, level);
    fluids::rebuild(game, level, found.tanks, &TUNING);
}

/// The blocks of the storages, the tanks and the monorail a scan found.
struct Found {
    cells: BTreeSet<Pos>,
    terminals: BTreeSet<Pos>,
    tanks: BTreeSet<Pos>,
    rails: Vec<Pos>,
    /// The zipline's anchors.
    anchors: Vec<Pos>,
    /// Round 31: the housings saved by 0.19, with the channels set into their slots.
    old_housings: BTreeMap<Pos, OldSlots>,
}

/// The magistral and the machines of the level in place, from its block entities: each
/// housing's tier and each node's from its state, each housing's joint settings from its saved
/// compound, and the housings 0.19 saved with their channels set by hand.
fn scan(game: &Game<'_>, dimension: &str, level: &mut LevelNet) -> Found {
    let mut found = Found {
        cells: BTreeSet::new(),
        terminals: BTreeSet::new(),
        tanks: BTreeSet::new(),
        rails: Vec::new(),
        anchors: Vec::new(),
        old_housings: BTreeMap::new(),
    };
    let mut layout = Layout::default();
    level.crystals.clear();
    level.sources.seeds.clear();
    level.machines.clear();
    // A mod block's entry is named after its block (native_diagnostics.rs:76-88, 207-218).
    for (pos, kind) in game.native_level_block_entities(dimension) {
        if kind == content::HOUSING {
            if let Some(old) =
                crate::game::rustcraft_machines::entity(game, pos).and_then(|e| old_slots(&e.nbt))
            {
                found.old_housings.insert(pos, old);
            }
            layout.housings.insert(pos, housing_at(game, pos));
        } else if kind == content::NODE {
            let tier = game
                .native_block_property(pos, content::TIER)
                .and_then(|tier| tier.parse().ok())
                .unwrap_or(FIRST_TIER);
            layout.nodes.insert(pos, Node { tier });
        } else if kind == content::CRYSTAL {
            level.crystals.insert(pos);
        } else if kind == rustcraft_rules::grow::SEED {
            level.sources.seeds.insert(pos);
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
        } else if kind == rustcraft_rules::traversal::ANCHOR {
            found.anchors.push(pos);
        }
    }
    level.joints = window::all_joints(game, layout.housings.keys().copied());
    level.layout = layout;
    found
}

/// Round 31: the housings 0.19 saved (`old`, with the channels set into their slots) get all three
/// channels built in (rustcraft_rules::net::migrate): each takes the tier of its best channel, its
/// joints' settings move with their kind to the kind's slot, and its old slots go. A housing that
/// does not take its tier stays as it was, for the next read of the level.
fn migrate_old(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelNet,
    old: &BTreeMap<Pos, OldSlots>,
) {
    let plans = migrate::plan(old, &level.layout, &TUNING);
    for (pos, plan) in plans {
        let tier = plan.tier.to_string();
        let has = game.native_block_property(pos, content::TIER);
        if has.as_deref() != Some(tier.as_str())
            && !game.native_set_block_property(pos, content::TIER, &tier)
        {
            crate::game::rustcraft_debug::journal_warn!("rustcraft: the old housing at {pos:?} did not take tier {tier}");
            continue;
        }
        let moved = game.native_with_mod_block_entity(pos, |e| {
            let records = window::read_records(&e.nbt);
            let moved = migrate::remap(&records, &plan);
            window::write_records(&mut e.nbt, &moved);
            e.nbt.remove(&JavaString::from(OLD_SLOTS_KEY));
            (records.len(), moved.len())
        });
        let Some((had, kept)) = moved else {
            crate::game::rustcraft_debug::journal_warn!("rustcraft: the old housing at {pos:?} has no block entity");
            continue;
        };
        game.native_block_entity_sync(pos);
        level.layout.housings.insert(pos, Housing::new(plan.tier));
        crate::game::rustcraft_debug::record(
            Some(dimension),
            Some(pos),
            "housing updated",
            &format!(
                "the channels it held, {:?}, built in at tier {}; joint settings {had} -> {kept}",
                old.get(&pos),
                plan.tier
            ),
        );
    }
}

/// s18/pipe: `drop_item` for the other modules.
pub(super) fn drop_one(game: &mut Game<'_>, dimension: &str, pos: Pos, item: &str) {
    drop_item(game, dimension, pos, item);
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
        crate::game::rustcraft_debug::journal_warn!("rustcraft: {item} from the housing at {pos:?} was lost");
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
pub(super) fn diagnosis_of(
    game: &Game<'_>,
    dimension: &str,
    level: &LevelNet,
    pos: Pos,
    block: &str,
) -> Option<Text> {
    // s18/pipe: «Под фальш-панелью» first on a covered housing or node.
    if let Some(text) = crate::game::rustcraft_pipe18::covered_diagnosis(game, pos, block, || {
        housing::diagnose(&level.topology, &level.layout, pos).map(|d| diagnosis_text(&d))
    }) {
        return Some(text);
    }
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
    if let Some(mut text) = mixer::diagnosis(game, dimension, pos, block, &TUNING) {
        let joint = level.transport.joint_blocks.get(&pos);
        if let Some(modes) = joint.or(level.fluids.joint_blocks.get(&pos)) {
            text.push(Text::literal(" · "));
            text.push(transport::joint_text(modes));
        }
        if let Some(fluid) = level.fluids.no_lane(pos, level.ticks) {
            text.push(Text::literal(" · "));
            text.push(fluids::no_lane_text(fluid));
        }
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
        // Slice 11: the catalyst it holds and the seconds of work it has left (K1).
        if let Some(tank) = machines::catalyst_at(game, pos)
            && let Some(fluid) = tank.fluid.as_deref().filter(|_| tank.active().is_some())
        {
            text.push(Text::literal(" · "));
            text.push(line(
                "rustcraft.machine.catalyst",
                "%s for %s s more",
                &[
                    fluids::fluid_name(fluid),
                    Text::literal(format!("{:.0}", tank.seconds_left(&TUNING).ceil())),
                ],
            ));
        }
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
            tier,
            net_len,
            net_tier,
        } => {
            let mut text = line(
                "rustcraft.diagnosis.works",
                "Line of %s · node %s of %s",
                &[number(line_len), number(distance), number(limit)],
            );
            text.push(Text::literal(" · "));
            text.push(line("rustcraft.diagnosis.tier", "tier %s", &[number(tier)]));
            text.push(Text::literal(" · "));
            // Round 31 (M10): a weaker housing on the net holds this one's channels back.
            text.push(if net_tier < tier {
                line(
                    "rustcraft.diagnosis.net_held",
                    "net of %s held to tier %s",
                    &[number(net_len), number(net_tier)],
                )
            } else {
                line("rustcraft.diagnosis.net", "net of %s", &[number(net_len)])
            });
            text
        }
    }
}
