// Slice 14, 18: the storage tablet (spec 4.5 S7, guide `tablet`; rustcraft_rules::storage::tablet
// decides which storage and how a press travels).
//
// On the server:
// - A right click with `rustcraft:tablet` asks for its window. On the next tick of the player's
//   level the storage of the magistral the player stands within a node's range of opens, the
//   nearest node's first; none in reach says so. Between bases it does not reach.
// - Slice 18: the window is the mod's own screen on the player's client (API 111), drawn as the
//   terminal's window (rustcraft_terminal: tabs, the search, the list, the limits). The server sends
//   the client a byte on OPEN_CHANNEL to open it and the storage's list on LIST_CHANNEL
//   (`native_net_send_to_player`), again whenever the storage changes; the client's presses come
//   back on PRESS_CHANNEL (`native_net_from_client`) and wait for the level's tick: a stored item's
//   stack or one into the inventory (`native_give_stack`), a limit button, and a stack of the
//   inventory into the storage (`native_set_inventory_slot`, checked against the stack it held).
// - Each second the open window follows the player: out of the node's range it closes.
// - 0.21.1 (OT12): what the recipe book makes goes to the window on its own channel once it opens
//   and again when the book is built anew (rustcraft_storage::send_orderable), before the list.
//
// On the client:
// - OPEN_CHANNEL opens or closes the screen at the next client tick (`native_mod_screen_open` with
//   the id `rustcraft:tablet`, apart from the handbook's `rustcraft`). The screen hooks draw it and
//   take its clicks, keys and typing through rustcraft_terminal in its tablet context; the player's
//   inventory is drawn under the list from the client's own copy, and a click on an item there
//   deposits it. Escape closes it and the server hears so.
use crate::game::{
    Game,
    rustcraft_screens::{slot, tr},
    rustcraft_server::{LevelNet, line, tell},
    rustcraft_storage as storage, rustcraft_terminal as terminal,
};
use horizon_client::{hud::HudBatch, items::ItemStack, mod_api::NativeScreenView, text::Text};
use rustcraft_rules::storage::{
    tablet::{self as rules, LIST_CHANNEL, OPEN_CHANNEL, PRESS_CHANNEL, Press},
    terminal::{self as term, Press as TermPress, Take},
    wire,
};
use std::{
    collections::HashMap,
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU8, Ordering},
    },
};

pub const TABLET: &str = "rustcraft:tablet";
/// The tablet's own screen's id (`native_mod_screen`); the handbook's is `rustcraft`.
pub(super) const SCREEN_ID: &str = "rustcraft:tablet";
/// How often, in level ticks, an open window follows its player.
const FOLLOW_EVERY: u64 = 20;
/// How often, in level ticks, a changed storage's list goes to its tablets.
const SEND_EVERY: u64 = 5;
const LEFT: i32 = 0;
const RIGHT: i32 = 1;

// ---- The server ----

struct Session {
    dimension: String,
    storage: u32,
    /// The storage's change count last sent.
    sent: Option<u64>,
    /// 0.21.1 (OT12): the stamp of the recipe book's items last sent on their own channel.
    orderable: Option<u32>,
}

#[derive(Default)]
struct State {
    sessions: HashMap<u128, Session>,
    /// Windows asked for: the player, their level and where they stood.
    opening: Vec<(u128, String, [f64; 3])>,
    presses: Vec<(u128, Press)>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Round 32 (spec 33): the level and the storage of a player's open tablet window, for its
/// «Сеть» page (rustcraft_terminal_net).
pub(super) fn session_of(uuid: u128) -> Option<(String, u32)> {
    state().sessions.get(&uuid).map(|s| (s.dimension.clone(), s.storage))
}

/// `native_use_item`: a right click with the tablet opens it at the next tick of the level.
pub(super) fn on_use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed || &*stack.item != TABLET {
        return;
    }
    let Some(player) = crate::game::rustcraft_server::acting(game) else {
        return;
    };
    *claimed = true;
    state()
        .opening
        .push((player.uuid, player.dimension.clone(), player.position));
}

/// The server's menu dispatch: the tablet opens no server menu since slice 18.
pub(super) fn on_menu_clicked(_: u128, _: i32, _: i32, _: i32) {}

/// The server's menu dispatch.
pub(super) fn on_menu_closed(_: u128, _: i32) {}

/// `native_net_from_client`: a press of a player's tablet window, for the level's tick.
pub(super) fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel != PRESS_CHANNEL {
        return;
    }
    match rules::decode_press(data) {
        Some(p) => state().presses.push((player, p)),
        None => log::debug!("rustcraft: a tablet press from {player:032x} that does not read"),
    }
}

/// Whether the storage `id` is in the tablet's reach of a player at `at`.
fn in_reach(dimension: &str, level: &LevelNet, at: [f64; 3], id: u32) -> bool {
    let mine: Vec<rules::Candidate> = storage::tablet_candidates(dimension)
        .into_iter()
        .filter(|c| c.id == id)
        .collect();
    rules::pick(at, &mine, &level.topology).is_some()
}

fn send_open(game: &mut Game<'_>, uuid: u128, open: bool) {
    if let Err(e) = game.native_net_send_to_player(uuid, OPEN_CHANNEL, &[u8::from(open)]) {
        log::debug!("rustcraft: the tablet window not sent to {uuid:032x}: {e}");
    }
}

/// 0.21.1 (OT12): the recipe book's items to a player's window when it has not got the book's
/// list now (rustcraft_storage::send_orderable); before the storage's list.
fn send_orderable(game: &mut Game<'_>, uuid: u128) {
    let Some(sent) = state().sessions.get(&uuid).map(|s| s.orderable) else {
        return;
    };
    let now = storage::send_orderable(game, uuid, sent);
    if now != sent
        && let Some(s) = state().sessions.get_mut(&uuid)
    {
        s.orderable = now;
    }
}

/// The storage's list to a player's window when it changed since it was last sent.
fn send_list(game: &mut Game<'_>, uuid: u128, force: bool) {
    let Some((dimension, id, sent)) = state()
        .sessions
        .get(&uuid)
        .map(|s| (s.dimension.clone(), s.storage, s.sent))
    else {
        return;
    };
    let version = storage::version(&dimension, id);
    if !force && sent == Some(version) {
        return;
    }
    let bytes = wire::encode(&storage::tablet_listing(game, &dimension, id));
    if let Err(e) = game.native_net_send_to_player(uuid, LIST_CHANNEL, &bytes) {
        log::debug!("rustcraft: the tablet list not sent to {uuid:032x}: {e}");
        return;
    }
    if let Some(s) = state().sessions.get_mut(&uuid) {
        s.sent = Some(version);
    }
}

fn sessions_in(dimension: &str) -> Vec<u128> {
    state()
        .sessions
        .iter()
        .filter(|(_, x)| x.dimension == dimension)
        .map(|(u, _)| *u)
        .collect()
}

/// Each tick of a level: open the tablets asked for there, answer their presses, send changed
/// lists, and each second let the open windows follow their player.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &LevelNet) {
    let mine = sessions_in(dimension);
    let (opening, presses) = {
        let mut s = state();
        let (here, later): (Vec<_>, Vec<_>) = std::mem::take(&mut s.opening)
            .into_iter()
            .partition(|o| o.1 == dimension);
        s.opening = later;
        let (presses, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.presses)
            .into_iter()
            .partition(|p| mine.contains(&p.0));
        // A press of a player with a window in another level waits for that level's tick; one of
        // a player with no window at all is dropped.
        let open: Vec<u128> = s.sessions.keys().copied().collect();
        s.presses = rest.into_iter().filter(|p| open.contains(&p.0)).collect();
        (here, presses)
    };
    for (uuid, _, at) in opening {
        open(game, dimension, level, uuid, at);
    }
    for (uuid, p) in presses {
        press(game, dimension, uuid, p);
    }
    if level.ticks.is_multiple_of(SEND_EVERY) {
        for uuid in sessions_in(dimension) {
            send_orderable(game, uuid);
            send_list(game, uuid, false);
        }
    }
    if level.ticks.is_multiple_of(FOLLOW_EVERY) {
        follow(game, dimension, level);
    }
    // Slice 18: the filter module's windows.
    crate::game::rustcraft_filter::tick(game);
}

fn open(game: &mut Game<'_>, dimension: &str, level: &LevelNet, uuid: u128, at: [f64; 3]) {
    let candidates = storage::tablet_candidates(dimension);
    let Some(id) = rules::pick(at, &candidates, &level.topology) else {
        let text = line(
            "rustcraft.tablet.far",
            "No storage in reach: stand within the range of a node of the storage's magistral",
            &[],
        );
        tell(game, uuid, text);
        return;
    };
    state().sessions.insert(
        uuid,
        Session {
            dimension: dimension.to_owned(),
            storage: id,
            sent: None,
            orderable: None,
        },
    );
    send_orderable(game, uuid);
    send_list(game, uuid, true);
    send_open(game, uuid, true);
}

fn press(game: &mut Game<'_>, dimension: &str, uuid: u128, p: Press) {
    let Some(id) = state().sessions.get(&uuid).map(|s| s.storage) else {
        return;
    };
    match p {
        Press::Closed => {
            state().sessions.remove(&uuid);
        }
        Press::Deposit(slot) => {
            if storage::tablet_deposit(game, dimension, id, uuid, usize::from(slot)) == 0 {
                let text = line("rustcraft.tablet.no_room", "The storage takes none of it", &[]);
                tell(game, uuid, text);
            }
        }
        Press::Terminal(code) => match term::decode(code) {
            Some(TermPress::Place {
                item: Some(item),
                take,
            }) => {
                if storage::tablet_take(game, dimension, id, item, take == Take::One, uuid) == 0 {
                    let text = line("rustcraft.tablet.full", "No room in your inventory", &[]);
                    tell(game, uuid, text);
                }
            }
            Some(TermPress::Limit { item, change }) => {
                storage::tablet_limit(game, dimension, id, item, change);
            }
            _ => {}
        },
    }
}

/// The open windows of this level follow their player: out of reach, the window closes.
fn follow(game: &mut Game<'_>, dimension: &str, level: &LevelNet) {
    let open: Vec<(u128, u32)> = state()
        .sessions
        .iter()
        .filter(|(_, x)| x.dimension == dimension)
        .map(|(u, x)| (*u, x.storage))
        .collect();
    if open.is_empty() {
        return;
    }
    let players = game.native_server_players();
    for (uuid, id) in open {
        let here = players
            .iter()
            .find(|p| p.uuid == uuid && p.dimension == dimension)
            .is_some_and(|p| in_reach(dimension, level, p.position, id));
        if !here {
            send_open(game, uuid, false);
            state().sessions.remove(&uuid);
        }
    }
}

// ---- The client ----

/// What the server last asked the client: 0 nothing, 1 open, 2 close.
static ASKED: AtomicU8 = AtomicU8::new(0);
/// Whether this client's window is open as far as it knows (to tell the server it closed).
static OPEN: AtomicU8 = AtomicU8::new(0);

/// `native_net_from_server` on a client: the window's list, or a request to open or close it.
pub(super) fn on_net(channel: &str, data: &[u8]) {
    if channel == OPEN_CHANNEL {
        ASKED.store(if data.first() == Some(&1) { 1 } else { 2 }, Ordering::Relaxed);
    } else if channel == LIST_CHANNEL {
        match wire::decode(data) {
            Some(l) => terminal::set_tablet_listing(l),
            None => log::warn!("rustcraft: a tablet list that does not read"),
        }
    }
}

/// Presses made since the client's last tick (a click hook has no `&mut Game` to send with).
static PENDING: Mutex<Vec<Press>> = Mutex::new(Vec::new());

fn queue(p: Press) {
    PENDING.lock().unwrap_or_else(PoisonError::into_inner).push(p);
}

/// A press to the server; in single player (no remote server) straight into the server's queue,
/// the local player being the server's own.
fn send(game: &mut Game<'_>, p: Press) {
    if game.remote.is_none() {
        let me = game.native_acting_id();
        state().presses.push((me, p));
        return;
    }
    if let Err(e) = game.native_net_send_to_server(PRESS_CHANNEL, &rules::encode_press(p)) {
        log::debug!("rustcraft: a tablet press not sent: {e}");
    }
}

/// The client's tick: the window asked for opens or closes; a window the player closed is told
/// to the server.
pub(super) fn client_tick(game: &mut Game<'_>) {
    match ASKED.swap(0, Ordering::Relaxed) {
        1 => {
            terminal::reset_tablet_view();
            if game.native_mod_screen_open(SCREEN_ID) {
                OPEN.store(1, Ordering::Relaxed);
            } else {
                send(game, Press::Closed);
            }
        }
        2 => {
            game.native_mod_screen_close(SCREEN_ID);
            OPEN.store(0, Ordering::Relaxed);
        }
        _ => {}
    }
    let pending = std::mem::take(&mut *PENDING.lock().unwrap_or_else(PoisonError::into_inner));
    for p in pending {
        send(game, p);
    }
    if OPEN.load(Ordering::Relaxed) == 1 && !is_open(game) {
        OPEN.store(0, Ordering::Relaxed);
        send(game, Press::Closed);
    }
}

pub(super) fn is_open(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(SCREEN_ID)
}

/// The window's corner: the terminal's window in the middle of the screen.
fn corner(screen: &NativeScreenView<'_>) -> [f32; 2] {
    [
        screen.rect[0] + ((screen.rect[2] - term::WIDTH) / 2.0).floor(),
        screen.rect[1] + ((screen.rect[3] - term::HEIGHT) / 2.0).floor(),
    ]
}

/// Where inventory slot `i` (0..9 the hotbar) is drawn, as a menu draws it.
fn inventory_slot(i: usize) -> [f32; 2] {
    let at = term::INVENTORY;
    if i < 9 {
        [at[0] + 18.0 * i as f32, at[1] + 58.0]
    } else {
        let k = i - 9;
        [at[0] + 18.0 * (k % 9) as f32, at[1] + 18.0 * (k / 9) as f32]
    }
}

fn inventory_slot_at(at: [f32; 2]) -> Option<usize> {
    (0..36).find(|&i| {
        let p = inventory_slot(i);
        at[0] >= p[0] - 1.0 && at[0] < p[0] + 17.0 && at[1] >= p[1] - 1.0 && at[1] < p[1] + 17.0
    })
}

/// `native_screen_render_post`: the window, its inventory with the client's own items, and the
/// tooltip under the mouse.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !is_open(game) {
        return;
    }
    let o = corner(screen);
    terminal::in_tablet(|| terminal::draw(game, batch, o));
    for i in 0..36 {
        let p = inventory_slot(i);
        slot(game, batch, o, p);
        if let Some(stack) = game.inventory.get(i).cloned().flatten() {
            let count = (stack.count != 1).then(|| stack.count.to_string());
            game.native_gui_draw_item_with(batch, &stack, [o[0] + p[0], o[1] + p[1]], count.as_deref());
        }
    }
    let at = [screen.mouse[0] - o[0], screen.mouse[1] - o[1]];
    let mut lines = Vec::new();
    if let Some(i) = inventory_slot_at(at) {
        if let Some(stack) = game.inventory.get(i).cloned().flatten() {
            lines.push(terminal::item_name(&stack.item));
            lines.extend(terminal::variant_lines(&stack));
            lines.push(tr("rustcraft.tablet.deposit_hint", "Click: into the storage"));
        }
    } else {
        terminal::in_tablet(|| terminal::tooltip(game, at, &mut lines));
    }
    if !lines.is_empty() {
        let texts: Vec<Text> = lines
            .iter()
            .map(|l| Text::literal(l.trim_start_matches("§7")))
            .collect();
        game.native_gui_tooltip(batch, &texts, game.native_cursor());
    }
}

/// `native_screen_mouse_button_pressed_pre`: a click in the window.
pub(super) fn on_mouse(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if *canceled || !is_open(game) {
        return;
    }
    *canceled = true;
    if button != LEFT && button != RIGHT {
        return;
    }
    let o = corner(screen);
    let at = [screen.mouse[0] - o[0], screen.mouse[1] - o[1]];
    if let Some(i) = inventory_slot_at(at) {
        if game.inventory.get(i).is_some_and(Option::is_some) {
            queue(Press::Deposit(i as u8));
        }
        return;
    }
    let id = terminal::in_tablet(|| terminal::on_click(game, at, button == RIGHT));
    if let Some(id) = id.filter(|id| *id != term::NO_PRESS) {
        // In the tablet a stored item always goes into the inventory.
        let id = match term::decode(id) {
            Some(TermPress::Place { item: Some(item), take }) => {
                let take = if take == Take::One { Take::One } else { Take::ToInventory };
                term::encode(TermPress::Place { item: Some(item), take }).unwrap_or(id)
            }
            _ => id,
        };
        queue(Press::Terminal(id));
    }
}

/// `native_screen_key_pressed_pre`: the search takes the keys.
pub(super) fn on_key(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    if is_open(game) {
        terminal::in_tablet(|| terminal::on_key(game, screen, key, canceled));
    }
}

/// `native_screen_char_typed_pre`: typing into the search.
pub(super) fn on_char(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char, canceled: &mut bool) {
    if is_open(game) {
        terminal::in_tablet(|| terminal::on_char(game, screen, c, canceled));
    }
}
