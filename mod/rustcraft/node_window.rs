// Slice 18, stream 1: the node's window (guide `node`, ui.js `node`) and the line-boost module.
//
// A right click on a node with an empty hand (or a line-boost module) opens its window: the
// server sends the view (rustcraft_rules::net::node_window) and the client opens a screen of the
// mod's own (`native_mod_screen_open`, API 111) under the id `rustcraft:node`, so the handbook's
// own screen (id `rustcraft`) never draws over it. While it is open the server sends the view
// again each second; a press (a line, a socket) goes back as a mod message from the client's tick
// (`native_net_send_to_server` takes the game mutably, the mouse hook only lends it). A socket
// press puts a line-boost module from the player's inventory into the node's block entity or
// gives it back; the item nets on the housings the node serves carry that much more
// (rustcraft_transport, `node_window::net_boost`).
use crate::game::{Game, rustcraft_content as content, rustcraft_server as server};
use horizon_world::nbt::{JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    net::node_window::{self as nw, NodeView, Press},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

/// The node's block entity keeps its line-boost modules under this key.
const MODULES_KEY: &str = "line_modules";
/// The mod screen's id.
const SCREEN: &str = "rustcraft:node";
/// How far a viewer may walk from the node before its window stops getting views, blocks.
const VIEW_REACH: f64 = 8.0;

struct Viewer {
    dimension: String,
    pos: Pos,
    axis: usize,
}

#[derive(Default)]
struct NodeServer {
    viewers: HashMap<u128, Viewer>,
    presses: Vec<(u128, Pos, Press)>,
}

static NODES: LazyLock<Mutex<NodeServer>> = LazyLock::new(Mutex::default);

fn nodes() -> MutexGuard<'static, NodeServer> {
    NODES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The line-boost modules of the node at `pos`.
pub(super) fn modules_of(game: &Game<'_>, pos: Pos) -> u8 {
    game.native_mod_block_entity(pos)
        .map_or(0, |e| match e.nbt.get(&JavaString::from(MODULES_KEY)) {
            Some(Tag::Byte(n)) => (*n).clamp(0, nw::SOCKETS as i8) as u8,
            _ => 0,
        })
}

/// Every node's line-boost modules (nodes without any left out), for the nets' throughput.
pub(super) fn node_modules(game: &Game<'_>, nodes: impl Iterator<Item = Pos>) -> BTreeMap<Pos, u8> {
    nodes
        .map(|p| (p, modules_of(game, p)))
        .filter(|(_, n)| *n > 0)
        .collect()
}

pub(super) fn set_modules(game: &mut Game<'_>, pos: Pos, n: u8) -> bool {
    game.native_with_mod_block_entity(pos, |e| {
        e.nbt.insert(JavaString::from(MODULES_KEY), Tag::Byte(n.min(nw::SOCKETS) as i8));
    })
    .is_some()
}

/// `native_right_click_block`: an empty hand on a node opens its window on the clicked face's line.
pub(super) fn on_right_click_block(game: &mut Game<'_>, pos: Pos, face: [i32; 3], canceled: &mut bool) {
    if *canceled || game.native_block(pos) != content::NODE {
        return;
    }
    let held = game.native_held().map(|(item, _, _)| item);
    if held.as_deref().is_some_and(|i| i != nw::LINE_MODULE) {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    if player.sneaking {
        return;
    }
    *canceled = true;
    let axis = match face {
        [x, _, _] if x != 0 => 0,
        [_, _, z] if z != 0 => 1,
        _ => 2,
    };
    nodes().viewers.insert(
        player.uuid,
        Viewer {
            dimension: player.dimension.clone(),
            pos,
            axis,
        },
    );
    send_view(game, player.uuid, true);
}

/// The view of the window `player` has open, sent to them.
fn send_view(game: &mut Game<'_>, player: u128, open: bool) {
    let Some((dimension, pos, axis)) = nodes()
        .viewers
        .get(&player)
        .map(|v| (v.dimension.clone(), v.pos, v.axis))
    else {
        return;
    };
    let Some(mut view) = view_at(game, &dimension, pos, axis) else {
        return;
    };
    view.open = open;
    if let Err(e) = game.native_net_send_to_player(player, nw::CHANNEL, &nw::encode(&view)) {
        log::debug!("rustcraft: a node window not sent to {player:032x}: {e}");
    }
}

/// The window of the node at `pos` of `dimension` on line `axis`, as the server would send it.
pub(super) fn view_at(game: &Game<'_>, dimension: &str, pos: Pos, axis: usize) -> Option<NodeView> {
    let modules = modules_of(game, pos);
    server::with_level(dimension, |level| view_of(game, level, pos, axis, modules)).flatten()
}

/// The window of the node at `pos` on line `axis`, with the energy of the circuits on that line.
fn view_of(
    game: &Game<'_>,
    level: &server::LevelNet,
    pos: Pos,
    axis: usize,
    modules: u8,
) -> Option<NodeView> {
    if game.native_block(pos) != content::NODE {
        return None;
    }
    let tuning = server::tuning();
    let mut view = nw::view(&level.topology, &level.layout, tuning, pos, axis, modules);
    let housings = nw::line_housings(
        &level.topology,
        &level.layout,
        pos,
        &nw::faces(view.hub, view.axis),
    );
    if let Some((ledger, got)) = &level.pipe18_energy {
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
        view.made = balance.produced;
        view.used = balance.requested;
    }
    Some(view)
}

/// Each second of the level: the presses that came in, then a fresh view to every viewer still
/// near their node.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    let presses = std::mem::take(&mut nodes().presses);
    for (player, pos, press) in presses {
        apply_press(game, player, pos, press);
    }
    let players = game.native_server_players();
    let due: Vec<u128> = {
        let mut s = nodes();
        s.viewers.retain(|uuid, v| {
            players.iter().any(|p| {
                let c = v.pos.map(|c| f64::from(c) + 0.5);
                let d: f64 = (0..3).map(|i| (p.position[i] - c[i]).powi(2)).sum();
                p.uuid == *uuid && p.dimension == v.dimension && d.sqrt() <= VIEW_REACH
            })
        });
        s.viewers
            .iter()
            .filter(|(_, v)| v.dimension == dimension)
            .map(|(u, _)| *u)
            .collect()
    };
    for player in due {
        send_view(game, player, false);
    }
}

/// `native_net_from_client`: a press in a node window, taken in at the next second.
pub(super) fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel != nw::PRESS {
        return;
    }
    if let Some((pos, press)) = nw::decode_press(data) {
        let mut s = nodes();
        if s.viewers.get(&player).is_some_and(|v| v.pos == pos) {
            s.presses.push((player, pos, press));
        }
    }
}

/// `native_player_logged_out`.
pub(super) fn on_logout(player: u128) {
    nodes().viewers.remove(&player);
}

fn apply_press(game: &mut Game<'_>, player: u128, pos: Pos, press: Press) {
    match press {
        Press::Axis(axis) => {
            if let Some(v) = nodes().viewers.get_mut(&player) {
                v.axis = axis.min(2);
            }
        }
        Press::Socket(i) => {
            if socket(game, player, pos, i).is_some()
                && let Some(v) = nodes().viewers.get(&player)
            {
                server::mark_level_stale(&v.dimension);
            }
        }
    }
    send_view(game, player, false);
}

/// A socket press of `player` (none: the self-test, who has no inventory): a filled socket gives
/// its module back, an empty one takes one from the inventory. Sockets fill in order. The modules
/// after it.
pub(super) fn socket(game: &mut Game<'_>, player: u128, pos: Pos, i: u8) -> Option<u8> {
    let have = modules_of(game, pos);
    let next = if i < have {
        if player != 0 && moved(game, player, false) == 0 {
            return None;
        }
        have - 1
    } else if have < nw::SOCKETS {
        if player != 0 && moved(game, player, true) == 0 {
            return None;
        }
        have + 1
    } else {
        return None;
    };
    set_modules(game, pos, next).then_some(next)
}

/// One line-boost module taken from (`take`) or given to the player's inventory: how many moved.
fn moved(game: &mut Game<'_>, player: u128, take: bool) -> u32 {
    let mut n = 0;
    game.native_as_player(player, |g| {
        n = if take {
            g.native_take_items(nw::LINE_MODULE, 1)
        } else {
            g.native_give_items(nw::LINE_MODULE, 1)
        };
    });
    n
}

// ---- The client ----

#[derive(Default)]
struct NodeClient {
    view: Option<NodeView>,
    open_due: bool,
    shown: bool,
    presses: Vec<(Pos, Press)>,
}

static CLIENT: LazyLock<Mutex<NodeClient>> = LazyLock::new(Mutex::default);

fn client() -> MutexGuard<'static, NodeClient> {
    CLIENT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_net_from_server`: a node window's view.
pub(super) fn on_net(channel: &str, data: &[u8]) {
    if channel != nw::CHANNEL {
        return;
    }
    let Some(view) = nw::decode(data) else {
        log::warn!("rustcraft: a node window that does not read");
        return;
    };
    let mut c = client();
    c.open_due |= view.open;
    c.view = Some(view);
}

/// `native_tick` on a client: open the window the server sent, send the presses.
pub(super) fn client_tick(game: &mut Game<'_>) {
    let (open, presses) = {
        let mut c = client();
        (std::mem::take(&mut c.open_due), std::mem::take(&mut c.presses))
    };
    if open && game.native_mod_screen_open(SCREEN) {
        client().shown = true;
    }
    if client().shown && game.native_mod_screen() != Some(SCREEN) {
        let mut c = client();
        c.shown = false;
        c.view = None;
    }
    for (pos, press) in presses {
        if let Err(e) = game.native_net_send_to_server(nw::PRESS, &nw::encode_press(pos, press)) {
            log::warn!("rustcraft: a node window press not sent: {e}");
        }
    }
}

fn is_open(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(SCREEN)
}

fn corner(screen: &horizon_client::mod_api::NativeScreenView<'_>) -> [f32; 2] {
    let o = nw::origin([screen.rect[2], screen.rect[3]]);
    [screen.rect[0] + o[0], screen.rect[1] + o[1]]
}

/// `native_screen_mouse_pre`: a click on a line or a socket.
pub(super) fn on_mouse(
    game: &Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if *canceled || !is_open(game) {
        return;
    }
    *canceled = true;
    if button != 0 {
        return;
    }
    let o = corner(screen);
    let at = [screen.mouse[0] - o[0], screen.mouse[1] - o[1]];
    let mut c = client();
    let Some((pos, hub)) = c.view.as_ref().map(|v| (v.pos, v.hub)) else {
        return;
    };
    if let Some(press) = nw::press_at(at, hub) {
        if let (Press::Axis(a), Some(v)) = (press, c.view.as_mut()) {
            v.axis = a;
        }
        c.presses.push((pos, press));
    }
}

/// `native_screen_render_post`: the window, as ui.js draws it.
pub(super) fn draw(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    use crate::game::rustcraft_screens::{self as kit, tr, tr_with};
    use rustcraft_rules::machines::style::Severity;
    if !is_open(game) {
        return;
    }
    let Some(v) = client().view.clone() else {
        return;
    };
    let o = corner(screen);
    kit::background(game, batch, o, [nw::WIDTH, nw::HEIGHT]);
    let name = tr("block.rustcraft.node", "Magistral Node");
    let tier = tr_with("rustcraft.window.tier", "tier %s", &[v.tier.to_string()]);
    kit::title_line(game, batch, o, &name, Some(&tier), nw::WIDTH);
    kit::label(game, batch, &tr("rustcraft.node.window.line", "Line"), o, [8.0, nw::LINE_LABEL_Y]);
    if v.hub {
        let r = [8.0, nw::AXIS_Y, nw::WIDTH - 16.0, 14.0];
        kit::button(game, batch, o, r, &tr("rustcraft.node.window.hub", "all six faces"), true);
    } else {
        let names = [
            tr("rustcraft.node.window.axis_x", "west–east"),
            tr("rustcraft.node.window.axis_z", "north–south"),
            tr("rustcraft.node.window.axis_y", "up–down"),
        ];
        for (i, r) in nw::axis_rects().into_iter().enumerate() {
            kit::button(game, batch, o, r, &names[i], i == v.axis);
        }
    }
    for (i, (k, val)) in rows(&v).iter().enumerate() {
        let y = if i < 3 {
            nw::BALANCE_Y + nw::ROW * i as f32
        } else {
            nw::LINE_ROWS_Y + nw::ROW * (i - 3) as f32
        };
        kit::text(game, batch, k, o, [8.0, y], kit::TEXT);
        kit::text_right(game, batch, val, o, nw::WIDTH - 8.0, y, kit::VALUE);
    }
    let modules = tr("rustcraft.node.window.modules", "Modules");
    kit::label(game, batch, &modules, o, [8.0, nw::MODULES_LABEL_Y]);
    for (i, p) in nw::socket_items().into_iter().enumerate() {
        let filled = (i as u8) < v.modules;
        kit::socket(game, batch, o, p, !filled);
        if filled {
            game.native_item(batch, nw::LINE_MODULE, [o[0] + p[0], o[1] + p[1]], 16.0);
        }
    }
    let (severity, status) = match v.severity() {
        0 => (Severity::Ok, tr("rustcraft.machine.working", "Working")),
        1 => (
            Severity::Warn,
            tr("rustcraft.node.window.short", "Not enough energy: machines slowed"),
        ),
        _ => (
            Severity::Bad,
            tr("rustcraft.node.window.no_line", "No housing on this line"),
        ),
    };
    kit::status_line(game, batch, o, nw::STATUS, severity, &status);
}

/// The window's rows: the balance (production, consumption, enough), then the line (housings,
/// the farthest, the item channel on the section).
fn rows(v: &NodeView) -> Vec<(String, String)> {
    use crate::game::rustcraft_screens::{tr, tr_with};
    let es = |x: f64| tr_with("rustcraft.node.window.eps", "%s E/s", &[format!("{x:.0}")]);
    let enough = if v.used <= v.made {
        tr("rustcraft.node.window.yes", "yes")
    } else {
        tr_with(
            "rustcraft.node.window.slowed",
            "no, machines slowed by %s",
            &[format!("{}%", v.slowed_percent())],
        )
    };
    let items = v.items.map_or_else(
        || "—".to_owned(),
        |(a, b)| {
            tr_with(
                "rustcraft.node.window.items_rate",
                "%s → %s items/s",
                &[format!("{a:.0}"), format!("{b:.0}")],
            )
        },
    );
    let farthest = tr_with(
        "rustcraft.node.window.of_blocks",
        "%s of %s blocks",
        &[v.farthest.to_string(), v.limit.to_string()],
    );
    vec![
        (tr("rustcraft.node.window.made", "Production"), es(v.made)),
        (tr("rustcraft.node.window.used", "Consumption"), es(v.used)),
        (tr("rustcraft.node.window.enough", "Enough"), enough),
        (tr("rustcraft.node.window.housings", "Housings"), v.housings.to_string()),
        (tr("rustcraft.node.window.farthest", "Farthest"), farthest),
        (tr("rustcraft.node.window.items", "Item channel on the section"), items),
    ]
}
