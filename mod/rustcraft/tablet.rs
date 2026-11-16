// Slice 14: the storage tablet on the server (spec 4.5 S7, guide `tablet`;
// rustcraft_rules::storage::tablet decides which storage and what a click does).
//
// - A right click with `rustcraft:tablet` asks for its window. On the next tick of the player's
//   level the storage of the magistral the player stands within a node's range of opens, the
//   nearest node's first; none in reach says so. Between bases it does not reach.
// - The window is a server chest of six rows (`native_open_menu`, API 10, the joint window's way,
//   game/native_menus.rs:130-147): five rows of what the storage holds, most numerous first, each
//   with its stored count, and a row with the page arrows. A click on an item
//   (`native_menu_clicked`, with no player's state swapped in, so it waits for the level's tick)
//   takes one stack of it into the inventory (`native_give_stack`, API 98, from any server hook);
//   what does not fit stays in the storage.
// - Each second the open window follows the storage, and closes when the player has left the
//   node's range.
use crate::game::{
    Game,
    rustcraft_machines::max_stack,
    rustcraft_server::{LevelNet, line, tell},
    rustcraft_storage as storage,
    rustcraft_window::{named, styled},
};
use horizon_client::{items::ItemStack, text::Text};
use horizon_world::nbt::Tag;
use rustcraft_rules::storage::{
    keys,
    tablet::{self as rules, Click},
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

pub const TABLET: &str = "rustcraft:tablet";
/// `ClickType`'s PICKUP and QUICK_MOVE: a plain click and a shift-click.
const PICKUP: i32 = 0;
const QUICK_MOVE: i32 = 1;
/// How often, in level ticks, an open window follows its storage and the player's place.
const FOLLOW_EVERY: u64 = 20;

struct Session {
    dimension: String,
    storage: u32,
    window: i32,
    page: usize,
}

struct ClickAt {
    uuid: u128,
    window: i32,
    slot: usize,
}

#[derive(Default)]
struct State {
    sessions: HashMap<u128, Session>,
    /// Windows asked for: the player, their level and where they stood.
    opening: Vec<(u128, String, [f64; 3])>,
    clicks: Vec<ClickAt>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_use_item`: a right click with the tablet opens it at the next tick of the level.
pub(super) fn on_use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed || &*stack.item != TABLET {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    *claimed = true;
    state()
        .opening
        .push((player.uuid, player.dimension.clone(), player.position));
}

/// `native_menu_clicked`: a click on the tablet's own slots waits for the level's tick.
pub(super) fn on_menu_clicked(uuid: u128, window: i32, slot: i32, mode: i32) {
    let mut s = state();
    let ours = s.sessions.get(&uuid).is_some_and(|x| x.window == window);
    if !ours || !(mode == PICKUP || mode == QUICK_MOVE) {
        return;
    }
    if let Ok(slot) = usize::try_from(slot)
        && slot < usize::from(rules::ROWS) * 9
    {
        s.clicks.push(ClickAt { uuid, window, slot });
    }
}

/// `native_menu_closed`: the player closed it, or another menu took its place.
pub(super) fn on_menu_closed(uuid: u128, window: i32) {
    let mut s = state();
    if s.sessions.get(&uuid).is_some_and(|x| x.window == window) {
        s.sessions.remove(&uuid);
        s.clicks.retain(|c| c.uuid != uuid);
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

/// What the window shows of the storage `id` on page `page` (the last one when past it).
fn cells(game: &Game<'_>, dimension: &str, id: u32, page: usize) -> Vec<Option<ItemStack>> {
    let list = storage::tablet_items(dimension, id);
    let pages = rules::pages(list.len());
    let page = page.min(pages - 1);
    let mut out: Vec<Option<ItemStack>> = vec![None; usize::from(rules::ROWS) * 9];
    let shown = list.iter().skip(page * rules::PER_PAGE).take(rules::PER_PAGE);
    for (slot, (key, count)) in shown.enumerate() {
        let max = max_stack(game, keys::item_of(key));
        let mut stack = storage::stored_stack(key, rules::shown_count(*count, max));
        let lore = [
            line("rustcraft.tablet.stored", "Stored: %s", &[Text::literal(count.to_string())]),
            line("rustcraft.tablet.take", "Click: take a stack", &[]),
        ];
        let lines = lore.iter().map(|l| styled(l, "gray")).collect();
        stack.data_mut().set_other("minecraft:lore", Some(Tag::List(lines)));
        out[slot] = Some(stack);
    }
    if page > 0 {
        let name = line("rustcraft.tablet.prev", "Previous page", &[]);
        out[rules::PREV_SLOT] = Some(named("minecraft:arrow", 1, name, &[], false));
    }
    if page + 1 < pages {
        let name = line("rustcraft.tablet.next", "Next page", &[]);
        out[rules::NEXT_SLOT] = Some(named("minecraft:arrow", 1, name, &[], false));
    }
    let name = line(
        "rustcraft.tablet.page",
        "Page %s of %s",
        &[Text::literal((page + 1).to_string()), Text::literal(pages.to_string())],
    );
    let kinds = line("rustcraft.tablet.kinds", "Kinds of items: %s", &[Text::literal(list.len().to_string())]);
    out[rules::INFO_SLOT] = Some(named("minecraft:paper", 1, name, &[kinds], false));
    out
}

/// Each tick of a level: open the tablets asked for there, answer their clicks, and each second
/// let the open windows follow their storage.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &LevelNet) {
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
    for (uuid, _, at) in opening {
        open(game, dimension, level, uuid, at);
    }
    for c in clicks {
        click(game, dimension, c);
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
    let items = cells(game, dimension, id, 0);
    let title = line("rustcraft.tablet.title", "Storage tablet", &[]);
    if let Some(window) = game.native_open_menu(uuid, rules::ROWS, title, items) {
        state().sessions.insert(
            uuid,
            Session {
                dimension: dimension.to_owned(),
                storage: id,
                window,
                page: 0,
            },
        );
    }
}

fn click(game: &mut Game<'_>, dimension: &str, c: ClickAt) {
    let Some((id, page)) = state()
        .sessions
        .get(&c.uuid)
        .filter(|x| x.window == c.window)
        .map(|x| (x.storage, x.page))
    else {
        return;
    };
    let list = storage::tablet_items(dimension, id);
    let page = page.min(rules::pages(list.len()) - 1);
    match rules::click(c.slot, page, list.len()) {
        Click::Take(i) => {
            let key = list[i].0.clone();
            if storage::take_to_player(game, dimension, id, &key, c.uuid) == 0 {
                let text = line("rustcraft.tablet.full", "No room in your inventory", &[]);
                tell(game, c.uuid, text);
            }
        }
        Click::Page(p) => {
            if let Some(x) = state().sessions.get_mut(&c.uuid) {
                x.page = p;
            }
        }
        Click::Nothing => return,
    }
    let page = state().sessions.get(&c.uuid).map_or(0, |x| x.page);
    let items = cells(game, dimension, id, page);
    game.native_set_menu_items(c.uuid, items);
}

/// The open windows of this level follow their storage; a player out of reach has it closed.
fn follow(game: &mut Game<'_>, dimension: &str, level: &LevelNet) {
    let open: Vec<(u128, u32, usize)> = state()
        .sessions
        .iter()
        .filter(|(_, x)| x.dimension == dimension)
        .map(|(u, x)| (*u, x.storage, x.page))
        .collect();
    if open.is_empty() {
        return;
    }
    let players = game.native_server_players();
    for (uuid, id, page) in open {
        let here = players
            .iter()
            .find(|p| p.uuid == uuid && p.dimension == dimension)
            .is_some_and(|p| in_reach(dimension, level, p.position, id));
        if here {
            let items = cells(game, dimension, id, page);
            game.native_set_menu_items(uuid, items);
        } else {
            game.native_close_menu(uuid);
            state().sessions.remove(&uuid);
        }
    }
}
