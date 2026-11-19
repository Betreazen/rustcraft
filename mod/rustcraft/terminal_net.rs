// Round 32 (spec 33): the «Сеть» page of the storage terminal and the storage tablet
// (rustcraft_rules::storage::network). The owner: «нужна доп. страница в терминал склада, чтобы
// видеть информацию о сети и какие баки с жидкостями подключены».
//
// The page is pulled, not pushed, so it costs the server nothing while nobody looks at it:
// - The client: while the terminal's window (rustcraft_terminal) draws the page, it notes so
//   here; the client's tick then asks the server for it on ASK_CHANNEL
//   (`native_net_send_to_server`), again each ASK_EVERY_MS while the page stays shown. A window
//   that closes or leaves the page simply stops asking. In single player the ask goes straight
//   into the server's queue, as the tablet's presses do.
// - The server (`native_net_from_client`): each player's last ask waits for the next tick of
//   their level, which builds the page from the level's state (rustcraft_server::LevelNet: the
//   topology, the item nets' joints, the fluid nets' joints and tanks, the energy ledger) and
//   sends it on CHANNEL or, for the tablet, TABLET_CHANNEL (`native_net_send_to_player`). A
//   terminal's page goes only to a player within REACH of that terminal; the tablet's only to a
//   player whose tablet window is open (rustcraft_tablet), for its storage.
//
// What the page lists is what the storage sees: its branch (the item nets its cells and terminals
// touch; since round 31 the fluid and energy nets are the same housings), the machines, chests
// and other storages on that branch's joints, and every tank group behind a joint of the
// branch's fluid nets, with how the joint works it (FL5: only a tank whose joint gives counts as
// the storage's stock).
use crate::game::{
    Game,
    rustcraft_content as content,
    rustcraft_fluids as fluids,
    rustcraft_machines::max_stack,
    rustcraft_node_window as node_window,
    rustcraft_power as power,
    rustcraft_screens::{self as kit, LABEL, TEXT, VALUE, plural, text, text_right, tr, tr_with},
    rustcraft_server::{self as server, LevelNet},
    rustcraft_storage as storage,
    rustcraft_tablet as tablet,
};
use horizon_client::hud::HudBatch;
use rustcraft_rules::{
    geom::{Face, Pos, step},
    machines::{
        catalyst,
        panel::{energy_text, grouped},
        style::{self, colour},
    },
    net::{
        fluids::BUCKET,
        node_window::net_boost,
        topology::Reach,
    },
    storage::{
        Shape, Store, keys,
        network::{
            self as net, Activity, Ask, Energy, EndGroup, JointUse, Magistral, MachineGroup,
            NetPage, Row, Section, StoreFacts, TankLine,
        },
    },
    tuning::Tuning,
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

// ---- The server ----

#[derive(Default)]
struct Asks {
    /// Each player's last ask, for the next tick of their level.
    waiting: HashMap<u128, Ask>,
    /// When each player was last answered.
    answered: HashMap<u128, Instant>,
}

static ASKS: LazyLock<Mutex<Asks>> = LazyLock::new(Mutex::default);

fn asks() -> MutexGuard<'static, Asks> {
    ASKS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_net_from_client`: a player's client asks for the page.
pub(super) fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel != net::ASK_CHANNEL {
        return;
    }
    match net::decode_ask(data) {
        Some(ask) => {
            asks().waiting.insert(player, ask);
        }
        None => log::debug!("rustcraft: a network page ask from {player:032x} that does not read"),
    }
}

/// Each tick of a level (rustcraft_server::on_level_tick, the level out of the shared map): the
/// asks of the players in it answered with a fresh page.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &LevelNet) {
    if asks().waiting.is_empty() {
        return;
    }
    let players = game.native_server_players();
    let due: Vec<(u128, Ask, [f64; 3])> = {
        let mut a = asks();
        // An ask of a player who left waits for nobody.
        a.waiting.retain(|uuid, _| players.iter().any(|p| p.uuid == *uuid));
        a.answered.retain(|uuid, _| players.iter().any(|p| p.uuid == *uuid));
        let here: Vec<(u128, Ask, [f64; 3])> = players
            .iter()
            .filter(|p| p.dimension == dimension)
            .filter_map(|p| Some((p.uuid, *a.waiting.get(&p.uuid)?, p.position)))
            .collect();
        let now = Instant::now();
        let mut due = Vec::new();
        for (uuid, ask, at) in here {
            a.waiting.remove(&uuid);
            let recent = a
                .answered
                .get(&uuid)
                .is_some_and(|t| now.duration_since(*t) < Duration::from_millis(net::ANSWER_EVERY_MS));
            if !recent {
                a.answered.insert(uuid, now);
                due.push((uuid, ask, at));
            }
        }
        due
    };
    for (uuid, ask, at) in due {
        let Some((block, channel)) = target(game, dimension, uuid, ask, at) else {
            continue;
        };
        let bytes = net::encode(&build(game, dimension, level, block));
        if let Err(e) = game.native_net_send_to_player(uuid, channel, &bytes) {
            log::debug!("rustcraft: a network page not sent to {uuid:032x}: {e}");
        }
    }
}

/// The block whose storage the ask is for, and the channel its answer goes on; none for a
/// terminal out of the player's reach, or a tablet with no window open in this level.
fn target(
    game: &Game<'_>,
    dimension: &str,
    uuid: u128,
    ask: Ask,
    at: [f64; 3],
) -> Option<(Pos, &'static str)> {
    match ask {
        Ask::Terminal(pos) => {
            let c = pos.map(|v| f64::from(v) + 0.5);
            let d: f64 = (0..3).map(|i| (at[i] - c[i]).powi(2)).sum::<f64>().sqrt();
            (d <= net::REACH && game.native_block(pos) == content::TERMINAL).then_some((pos, net::CHANNEL))
        }
        Ask::Tablet => {
            let (d, id) = tablet::session_of(uuid).filter(|(d, _)| d == dimension)?;
            let block = storage::tablet_candidates(&d)
                .into_iter()
                .find(|c| c.id == id)?
                .blocks
                .first()
                .copied()?;
            Some((block, net::TABLET_CHANNEL))
        }
    }
}

/// The page of the storage the cell or terminal at `at` belongs to (a terminal that touches no
/// cell: its own magistral and no storage).
pub(super) fn build(game: &Game<'_>, dimension: &str, level: &LevelNet, at: Pos) -> NetPage {
    let tuning = server::tuning();
    let id = storage::storage_at(dimension, at);
    let mut blocks: Vec<Pos> = id
        .and_then(|id| storage::tablet_candidates(dimension).into_iter().find(|c| c.id == id))
        .map(|c| c.blocks)
        .unwrap_or_default();
    if !blocks.contains(&at) {
        blocks.push(at);
    }
    let branch = net::branch(&level.topology, &blocks);
    let magistral = magistral(game, level, &branch, tuning);
    let energy = (magistral.branches > 0)
        .then(|| energy(level, &branch.housings))
        .flatten();
    let facts = id
        .and(storage::view(dimension, at))
        .map(|(_, shape, store)| store_facts(game, &shape, &store, tuning));
    let anchor = storage::anchor_of(dimension, at);
    let (machines, containers, storages) = connected(game, level, anchor);
    let (tanks, ends) = lane(game, level, &branch.housings, anchor, tuning);
    NetPage {
        terminal: at,
        magistral,
        energy,
        storage: facts,
        machines,
        containers,
        storages,
        tank_count: u16::try_from(tanks.len()).unwrap_or(u16::MAX),
        tanks,
        ends,
    }
}

/// The branch's size and what it carries: its slowest net, with the line-boost modules of the
/// nodes that serve it (rustcraft_transport and rustcraft_fluids count it so).
fn magistral(game: &Game<'_>, level: &LevelNet, branch: &net::Branch, tuning: &Tuning) -> Magistral {
    let nets: Vec<_> = branch.nets.iter().map(|&i| &level.topology.nets[i]).collect();
    let nodes: BTreeSet<Pos> = branch
        .housings
        .iter()
        .filter_map(|h| match level.topology.reach.get(h) {
            Some(Reach::Served { node, .. }) => Some(*node),
            _ => None,
        })
        .collect();
    let boosts = node_window::node_modules(game, nodes.into_iter());
    let slowest = |rate: &dyn Fn(u8) -> f64| {
        nets.iter()
            .map(|n| rate(n.min_tier) * net_boost(&n.housings, &level.topology.reach, &boosts))
            .reduce(f64::min)
            .unwrap_or(0.0)
    };
    Magistral {
        branches: u8::try_from(nets.len()).unwrap_or(u8::MAX),
        housings: branch.housings.len() as u32,
        out_of_reach: branch.out_of_reach,
        min_tier: branch.min_tier,
        items: slowest(&|t| tuning.items_of(t)) as f32,
        fluid: slowest(&|t| tuning.fluids_of(t)) as f32,
        energy_cap: if nets.is_empty() {
            0.0
        } else {
            tuning.energy_of(branch.min_tier) as f32
        },
    }
}

/// The energy of the circuits on the branch's housings, as the node's window counts it
/// (rustcraft_node_window): what the blocks beside them and their circuits make and ask for, and
/// the batteries on them; none before the machines' tick noted the level's ledger.
fn energy(level: &LevelNet, housings: &BTreeSet<Pos>) -> Option<Energy> {
    let (ledger, got) = level.pipe18_energy.as_ref()?;
    let around: BTreeSet<Pos> = housings
        .iter()
        .flat_map(|&h| Face::ALL.map(|f| step(h, f)))
        .filter(|b| !level.layout.housings.contains_key(b) && !level.layout.nodes.contains_key(b))
        .collect();
    let mut members = around.clone();
    for c in level.power.circuits.iter().filter(|c| !c.members.is_disjoint(&around)) {
        members.extend(c.members.iter().copied());
    }
    let balance = ledger.balance(&members, got);
    let (made, used) = (balance.produced, balance.requested);
    let stored = power::with_storage(&level.sources, &members, balance);
    Some(Energy {
        made: made as f32,
        used: used as f32,
        stored: stored.stored as f32,
        capacity: stored.capacity as f32,
    })
}

fn store_facts(game: &Game<'_>, shape: &Shape, store: &Store, tuning: &Tuning) -> StoreFacts {
    let stack = |k: &str| max_stack(game, keys::item_of(k));
    StoreFacts {
        cells: store.cells,
        used: store.used_slots(&stack),
        total: store.total_slots(),
        fluid_held: store.fluids.values().sum(),
        fluid_capacity: store.fluid_capacity(tuning.storage_cell_buckets * BUCKET),
        works: *shape == Shape::Solid,
    }
}

/// The machines (by kind, with what they do), the vanilla containers and the other storages on
/// the item nets the storage at `anchor` is a joint of (rustcraft_transport `peers`).
fn connected(game: &Game<'_>, level: &LevelNet, anchor: Option<Pos>) -> (Vec<MachineGroup>, u16, u16) {
    let Some(peers) = anchor.and_then(|a| level.transport.peers(a)) else {
        return (Vec::new(), 0, 0);
    };
    let blocks: BTreeSet<Pos> = peers.into_iter().map(|(p, _, _)| p).collect();
    let mut found = Vec::new();
    let (mut containers, mut storages) = (0u16, 0u16);
    for pos in blocks {
        if let Some(m) = level.machines.get(&pos) {
            let shown = level.power.status.get(&pos).map(|s| s.shown);
            found.push((m.block().to_owned(), Activity::of(shown)));
        } else if storage::is_storage_block(&game.native_block(pos)) {
            storages = storages.saturating_add(1);
        } else {
            containers = containers.saturating_add(1);
        }
    }
    (net::group_machines(found), containers, storages)
}

/// The tanks behind a joint of the fluid nets over `housings`, each group once, and the other
/// fluid ends there by kind: pumps, mixers, the kitchen, the trash… The storage itself and the
/// machines that only take a catalyst (FL3.a: no consumers) are left out.
fn lane(
    game: &Game<'_>,
    level: &LevelNet,
    housings: &BTreeSet<Pos>,
    anchor: Option<Pos>,
    tuning: &Tuning,
) -> (Vec<TankLine>, Vec<EndGroup>) {
    let lane = fluids::lane_of(game, level, housings, tuning);
    let mut tanks: Vec<TankLine> = lane
        .tanks
        .iter()
        .map(|(g, (takes, gives))| TankLine {
            pos: g.anchor(),
            fluid: g.fluid.clone().filter(|_| g.amount > 0),
            amount: g.amount,
            capacity: g.capacity,
            blocks: u16::try_from(g.blocks.len()).unwrap_or(u16::MAX),
            joint: JointUse::of(*takes, *gives),
        })
        .collect();
    tanks.sort_by_key(|t| t.pos);
    let ends = lane
        .ends
        .iter()
        .filter(|(p, _)| Some(*p) != anchor)
        .map(|(p, _)| {
            level
                .machines
                .get(p)
                .map_or_else(|| game.native_block(*p), |m| m.block().to_owned())
        })
        .filter(|b| b == content::PUMP || !catalyst::TAKERS.contains(&b.as_str()));
    (tanks, net::group_ends(ends))
}

// ---- The client ----

#[derive(Default)]
struct Client {
    /// What the window drew last and when: what to ask for.
    shown: Option<(Ask, Instant)>,
    /// The last ask sent and when.
    asked: Option<(Ask, Instant)>,
    /// The last page of a terminal, and of the tablet.
    page: Option<NetPage>,
    tablet_page: Option<NetPage>,
}

static CLIENT: LazyLock<Mutex<Client>> = LazyLock::new(Mutex::default);

fn client() -> MutexGuard<'static, Client> {
    CLIENT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// rustcraft_terminal drew the page now: the client's tick keeps asking for it.
pub(super) fn shown(ask: Ask) {
    client().shown = Some((ask, Instant::now()));
}

/// A tablet's window that opens shows no page of an earlier one.
pub(super) fn forget_tablet() {
    client().tablet_page = None;
}

/// The page to draw: the tablet's, or the terminal's when it is for `terminal`.
pub(super) fn page_for(tablet: bool, terminal: Option<Pos>) -> Option<NetPage> {
    let c = client();
    if tablet {
        return c.tablet_page.clone();
    }
    c.page.as_ref().filter(|p| Some(p.terminal) == terminal).cloned()
}

/// `native_net_from_server` on a client: a page.
pub(super) fn on_net(channel: &str, data: &[u8]) {
    let tablet = match channel {
        net::CHANNEL => false,
        net::TABLET_CHANNEL => true,
        _ => return,
    };
    let Some(page) = net::decode(data) else {
        log::warn!("rustcraft: a network page that does not read");
        return;
    };
    let mut c = client();
    if tablet {
        c.tablet_page = Some(page);
    } else {
        c.page = Some(page);
    }
}

/// The client's tick: while the page is shown, ask for it now and then.
pub(super) fn client_tick(game: &mut Game<'_>) {
    let due = {
        let mut c = client();
        let Some((ask, at)) = c.shown else {
            return;
        };
        if at.elapsed() > Duration::from_millis(net::SHOWN_FOR_MS) {
            c.shown = None;
            c.asked = None;
            return;
        }
        let again = c.asked.is_none_or(|(a, t)| {
            a != ask || t.elapsed() >= Duration::from_millis(net::ASK_EVERY_MS)
        });
        if !again {
            return;
        }
        c.asked = Some((ask, Instant::now()));
        ask
    };
    if game.remote.is_none() {
        let me = game.native_acting_id();
        asks().waiting.insert(me, due);
        return;
    }
    if let Err(e) = game.native_net_send_to_server(net::ASK_CHANNEL, &net::encode_ask(due)) {
        log::debug!("rustcraft: a network page ask not sent: {e}");
    }
}

// ---- Drawing ----

/// How many pages the page's rows fill (one while it has not come yet).
pub(super) fn page_count(page: Option<&NetPage>) -> usize {
    page.map_or(1, |p| net::pages(&net::rows(p)).len())
}

/// One row as the window writes it.
enum Line {
    Header(String),
    /// A line of its own, in a colour.
    Note(String, u32),
    /// A key on the left and a value on the right, each in its colour.
    Pair(String, u32, String, u32),
}

fn pair(key: &str, fallback: &str, value: String) -> Line {
    Line::Pair(tr(key, fallback), LABEL, value, VALUE)
}

/// A block's name in the player's language.
fn block_name(id: &str) -> String {
    crate::game::rustcraft_terminal::item_name(id)
}

/// A fluid's name: `rustcraft.fluid.<name>.name`.
fn fluid_name(fluid: Option<&str>) -> String {
    match fluid {
        None => tr("rustcraft.netpage.empty", "Empty"),
        Some(f) => {
            let short = f.rsplit(':').next().unwrap_or(f);
            tr(&format!("rustcraft.fluid.{short}.name"), f)
        }
    }
}

fn section_name(s: Section) -> String {
    match s {
        Section::Magistral => tr("rustcraft.netpage.magistral", "Magistral"),
        Section::Storage => tr("rustcraft.netpage.storage", "Storage"),
        Section::Connected => tr("rustcraft.netpage.connected", "Connected"),
        Section::Tanks => tr("rustcraft.netpage.tanks", "Tanks with fluids"),
        Section::FluidEnds => tr("rustcraft.netpage.fluid_ends", "Fluids on the line"),
    }
}

/// What a group of machines does: «работает», or «работают: 2 · ждут: 1»; its colour.
fn machine_state(g: &MachineGroup) -> (String, u32) {
    let rgb = if g.no_energy > 0 { colour::BAD_TEXT } else { VALUE };
    if g.count == 1 {
        let s = if g.working == 1 {
            tr("rustcraft.netpage.working", "working")
        } else if g.idle == 1 {
            tr("rustcraft.netpage.idle", "waiting")
        } else if g.no_energy == 1 {
            tr("rustcraft.netpage.no_energy", "no energy")
        } else {
            "—".to_owned()
        };
        return (s, rgb);
    }
    let parts: Vec<String> = [
        (g.working, "rustcraft.netpage.working_n", "working: %s"),
        (g.idle, "rustcraft.netpage.idle_n", "waiting: %s"),
        (g.no_energy, "rustcraft.netpage.no_energy_n", "no energy: %s"),
    ]
    .into_iter()
    .filter(|(n, _, _)| *n > 0)
    .map(|(n, key, fallback)| tr_with(key, fallback, &[n.to_string()]))
    .collect();
    let s = if parts.is_empty() { "—".to_owned() } else { parts.join(" · ") };
    (s, rgb)
}

/// «Электропечь ×2».
fn counted_name(block: &str, count: u16) -> String {
    let name = block_name(block);
    if count > 1 { format!("{name} ×{count}") } else { name }
}

fn coords(p: Pos) -> String {
    format!("{} {} {}", p[0], p[1], p[2])
}

/// «Вода 12,5/16», and what the joint does when the storage does not count it.
fn tank_text(t: &TankLine) -> String {
    let mut s = format!(
        "{} {}/{}",
        fluid_name(t.fluid.as_deref()),
        fluids::bucket_text(t.amount),
        fluids::bucket_text(t.capacity)
    );
    match t.joint {
        JointUse::Off => s.push_str(&tr("rustcraft.netpage.joint_off", " · joint off")),
        JointUse::Takes => s.push_str(&tr("rustcraft.netpage.joint_takes", " · takes only")),
        JointUse::Both | JointUse::Gives => {}
    }
    s
}

fn line(p: &NetPage, row: Row) -> Line {
    let m = &p.magistral;
    let s = p.storage.unwrap_or_default();
    match row {
        Row::Header(section) => Line::Header(section_name(section)),
        Row::NoBranch => Line::Note(
            tr("rustcraft.netpage.no_branch", "The storage touches no working housing"),
            colour::BAD_TEXT,
        ),
        Row::Housings => {
            let mut v = grouped(u64::from(m.housings));
            if m.branches > 1 {
                v = tr_with("rustcraft.netpage.branches", "%s in %s branches", &[v, m.branches.to_string()]);
            }
            pair("rustcraft.netpage.housings", "Housings", v)
        }
        Row::OutOfReach => Line::Pair(
            tr("rustcraft.netpage.out_of_reach", "Past the node's range"),
            LABEL,
            grouped(u64::from(m.out_of_reach)),
            colour::BAD_TEXT,
        ),
        Row::Tier => pair(
            "rustcraft.netpage.tier",
            "Weakest housing",
            tr_with("rustcraft.netpage.tier_value", "tier %s", &[m.min_tier.to_string()]),
        ),
        // The unit in the key («Предметов в секунду: 16»): no plural to agree with the number.
        Row::Items => pair(
            "rustcraft.netpage.items",
            "Items a second",
            energy_text(f64::from(m.items)),
        ),
        Row::Fluid => pair(
            "rustcraft.netpage.fluid",
            "Fluid buckets a second",
            energy_text(f64::from(m.fluid)),
        ),
        Row::Energy => {
            let e = p.energy.unwrap_or_default();
            pair(
                "rustcraft.netpage.energy",
                "Energy, E/s",
                tr_with(
                    "rustcraft.netpage.energy_value",
                    "makes %s · uses %s",
                    &[energy_text(f64::from(e.made)), energy_text(f64::from(e.used))],
                ),
            )
        }
        Row::Stored => {
            let e = p.energy.unwrap_or_default();
            pair(
                "rustcraft.netpage.stored",
                "In store",
                tr_with(
                    "rustcraft.netpage.stored_value",
                    "%s of %s E",
                    &[energy_text(f64::from(e.stored)), energy_text(f64::from(e.capacity))],
                ),
            )
        }
        Row::NoStorage => Line::Note(
            tr("rustcraft.storage.no_cells", "The terminal touches no storage cell"),
            colour::BAD_TEXT,
        ),
        Row::Cells => {
            let mut v = s.cells.to_string();
            if !s.works {
                v = tr_with("rustcraft.netpage.cells_broken", "%s · shape broken", &[v]);
            }
            pair("rustcraft.netpage.cells", "Cells", v)
        }
        Row::Slots => pair(
            "rustcraft.netpage.slots",
            "Slots used",
            tr_with("rustcraft.netpage.of", "%s of %s", &[grouped(s.used), grouped(s.total)]),
        ),
        Row::FluidCells => pair(
            "rustcraft.netpage.fluid_cells",
            "Fluid in cells",
            tr_with(
                "rustcraft.netpage.fluid_cells_value",
                "%s · up to %s of each",
                &[
                    plural(
                        s.fluid_held / BUCKET,
                        "rustcraft.window.buckets",
                        ("%s bucket", "%s buckets", "%s buckets"),
                    ),
                    grouped(s.fluid_capacity / BUCKET),
                ],
            ),
        ),
        Row::Machine(i) => match p.machines.get(i) {
            Some(g) => {
                let (state, rgb) = machine_state(g);
                Line::Pair(counted_name(&g.block, g.count), TEXT, state, rgb)
            }
            None => Line::Note(String::new(), LABEL),
        },
        Row::Containers => pair(
            "rustcraft.netpage.containers",
            "Chests and barrels",
            grouped(u64::from(p.containers)),
        ),
        Row::Storages => pair(
            "rustcraft.netpage.storages",
            "Other storages",
            grouped(u64::from(p.storages)),
        ),
        Row::NothingConnected => Line::Note(
            tr("rustcraft.netpage.nothing", "Nothing on the storage's joints"),
            LABEL,
        ),
        Row::Tank(i) => match p.tanks.get(i) {
            Some(t) => {
                let key_colour = if t.joint.counted() { VALUE } else { LABEL };
                Line::Pair(tank_text(t), key_colour, coords(t.pos), LABEL)
            }
            None => Line::Note(String::new(), LABEL),
        },
        Row::MoreTanks => Line::Note(
            tr_with(
                "rustcraft.netpage.more_tanks",
                "and %s more",
                &[(usize::from(p.tank_count).saturating_sub(p.tanks.len())).to_string()],
            ),
            LABEL,
        ),
        Row::NoTanks => Line::Note(
            tr("rustcraft.netpage.no_tanks", "No tanks on the magistral"),
            LABEL,
        ),
        Row::End(i) => match p.ends.get(i) {
            Some(e) => Line::Pair(block_name(&e.block), TEXT, format!("×{}", e.count), VALUE),
            None => Line::Note(String::new(), LABEL),
        },
    }
}

/// Draws one row in `r`: a header in the label's capitals, a note, or a key and a value, the
/// value given at most RIGHT's three fifths and the key cut to what the value leaves.
fn draw_row(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], r: [f32; 4], l: Line) {
    let y = r[1] + 1.0;
    match l {
        Line::Header(s) => {
            let s = kit::fit(game, &s.to_uppercase(), r[2]);
            text(game, batch, &s, o, [r[0], y], LABEL);
        }
        Line::Note(s, rgb) => {
            let s = kit::fit(game, &s, r[2]);
            text(game, batch, &s, o, [r[0], y], rgb);
        }
        Line::Pair(key, key_rgb, value, value_rgb) => {
            let value = kit::fit(game, &value, r[2] * 0.6);
            let room = r[2] - game.native_text_width(&value) - net::GAP;
            let key = kit::fit(game, &key, room.max(0.0));
            text(game, batch, &key, o, [r[0], y], key_rgb);
            text_right(game, batch, &value, o, r[0] + r[2], y, value_rgb);
        }
    }
}

/// The page's rows of page `index` (rustcraft_terminal's network mode), with the rule of the
/// window style between two key-and-value rows.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], page: Option<&NetPage>, index: usize) {
    let Some(p) = page else {
        let s = tr("rustcraft.terminal.loading", "Waiting for the storage…");
        draw_row(game, batch, o, net::row(0), Line::Note(s, LABEL));
        return;
    };
    let rows = net::rows(p);
    let pages = net::pages(&rows);
    let Some(range) = pages.get(index.min(pages.len() - 1)).cloned() else {
        return;
    };
    let shown = &rows[range];
    for (i, &row) in shown.iter().enumerate() {
        let r = net::row(i);
        if !matches!(row, Row::Header(_)) {
            kit::hover(game, batch, o, r);
        }
        draw_row(game, batch, o, r, line(p, row));
        let next_is_row = shown.get(i + 1).is_some_and(|n| !matches!(n, Row::Header(_)));
        if !matches!(row, Row::Header(_)) && next_is_row {
            let y = r[1] + net::ROW_H - 1.0;
            kit::fills(game, batch, o, &[style::kv_rule(r[0], r[0] + r[2], y)]);
        }
    }
}

/// The tooltip of row `i` of page `index`: a tank's place, contents and joint; a group of
/// machines' states; the tanks' header's meaning.
pub(super) fn row_tooltip(page: Option<&NetPage>, index: usize, i: usize, lines: &mut Vec<String>) {
    let Some(p) = page else {
        return;
    };
    let rows = net::rows(p);
    let pages = net::pages(&rows);
    let Some(range) = pages.get(index.min(pages.len() - 1)).cloned() else {
        return;
    };
    let Some(&row) = rows[range].get(i) else {
        return;
    };
    let hint = |key: &str, fallback: &str| format!("§7{}", tr(key, fallback));
    match row {
        Row::Header(Section::Tanks) | Row::NoTanks => lines.push(hint(
            "rustcraft.netpage.tanks_hint",
            "A tank in this list is connected right: the storage sees it",
        )),
        Row::Tank(k) => {
            let Some(t) = p.tanks.get(k) else {
                return;
            };
            lines.push(tr_with("rustcraft.netpage.tank_at", "Tank at %s", &[coords(t.pos)]));
            lines.push(tr_with(
                "rustcraft.netpage.tank_holds",
                "%s: %s of %s buckets, %s blocks",
                &[
                    fluid_name(t.fluid.as_deref()),
                    fluids::bucket_text(t.amount),
                    fluids::bucket_text(t.capacity),
                    t.blocks.to_string(),
                ],
            ));
            let (key, fallback) = match t.joint {
                JointUse::Both | JointUse::Gives => (
                    "rustcraft.netpage.tank_counted",
                    "Its fluid counts as the storage's stock",
                ),
                JointUse::Takes => (
                    "rustcraft.netpage.tank_takes",
                    "Its joint is set «into the block»: the storage does not count its fluid",
                ),
                JointUse::Off => (
                    "rustcraft.netpage.tank_off",
                    "Its joint is switched off: switch it on with the wrench",
                ),
            };
            lines.push(hint(key, fallback));
        }
        Row::Machine(k) => {
            let Some(g) = p.machines.get(k) else {
                return;
            };
            lines.push(counted_name(&g.block, g.count));
            for (n, key, fallback) in [
                (g.working, "rustcraft.netpage.working_n", "working: %s"),
                (g.idle, "rustcraft.netpage.idle_n", "waiting: %s"),
                (g.no_energy, "rustcraft.netpage.no_energy_n", "no energy: %s"),
            ] {
                lines.push(tr_with(key, fallback, &[n.to_string()]));
            }
        }
        Row::OutOfReach | Row::NoBranch => lines.push(hint(
            "rustcraft.netpage.reach_hint",
            "A housing works within its node's range: put a node closer, or one of a higher tier",
        )),
        Row::Tier | Row::Items | Row::Fluid => lines.push(hint(
            "rustcraft.netpage.tier_hint",
            "The weakest housing sets what the whole branch carries",
        )),
        _ => {}
    }
}
