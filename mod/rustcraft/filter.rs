// Slice 18 (TM1, guide `terminal`): the filter module's window on the server
// (rustcraft_rules::storage::filter decides).
//
// - A right click with the filter module `rustcraft:module_filter` in hand opens its window on the
//   next tick of the player's level: a server chest of three rows (`native_open_menu`, API 10, the
//   tablet's way) with the items it marks.
// - No click moves anything (native_menus.rs). A click on an item of the player's inventory below
//   the chest marks that item; a click on a marked item unmarks it. The list is the module's custom
//   data `rustcraft:filter` (a list of item ids), written back into the player's slot that holds
//   the module (`native_set_inventory_slot`, checked against the stack it held).
// - In a terminal's filter socket the module's list is the «своя» tab (rustcraft_storage).
use crate::game::{
    Game, native_inventories::NativeInventoryKind,
    rustcraft_server::{line, tell},
    rustcraft_utility::CUSTOM_DATA,
    rustcraft_window::named,
};
use horizon_client::{items::ItemStack, text::Text};
use horizon_world::nbt::{JavaString, Tag};
use rustcraft_rules::storage::{
    filter::{self as rules, Click},
    modules::FILTER,
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

/// `ClickType`'s PICKUP and QUICK_MOVE.
const PICKUP: i32 = 0;
const QUICK_MOVE: i32 = 1;
/// The chest's own slots.
const SIZE: usize = rules::ROWS as usize * 9;

struct Session {
    window: i32,
    /// The player's inventory slot that holds the module.
    slot: usize,
}

#[derive(Default)]
struct State {
    sessions: HashMap<u128, Session>,
    /// Windows asked for: the player and the module they used.
    opening: Vec<(u128, ItemStack)>,
    clicks: Vec<(u128, i32, i32)>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The items a filter module marks.
pub(super) fn marked(stack: &ItemStack) -> Vec<String> {
    match stack.custom_data().and_then(|d| d.get(&JavaString::from(rules::DATA_KEY))) {
        Some(Tag::List(list)) => list
            .iter()
            .filter_map(|t| match t {
                Tag::String(s) => Some(s.to_string_lossy()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The module `stack` with `list` marked.
fn with_marked(stack: &ItemStack, list: &[String]) -> ItemStack {
    let mut out = stack.clone();
    let mut data = out.custom_data().cloned().unwrap_or_default();
    let tags = list.iter().map(|i| Tag::String(JavaString::from(i.as_str()))).collect();
    data.insert(JavaString::from(rules::DATA_KEY), Tag::List(tags));
    out.data_mut().set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
    out
}

/// `native_use_item`: a right click with the filter module opens its window next tick.
pub(super) fn on_use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed || &*stack.item != FILTER {
        return;
    }
    let Some(player) = crate::game::rustcraft_server::acting(game) else {
        return;
    };
    *claimed = true;
    state().opening.push((player.uuid, stack.clone()));
}

/// `native_menu_clicked`: a click on the filter's window waits for the level's tick.
pub(super) fn on_menu_clicked(uuid: u128, window: i32, slot: i32, mode: i32) {
    let mut s = state();
    if s.sessions.get(&uuid).is_some_and(|x| x.window == window) && (mode == PICKUP || mode == QUICK_MOVE) {
        s.clicks.push((uuid, window, slot));
    }
}

/// `native_menu_closed`.
pub(super) fn on_menu_closed(uuid: u128, window: i32) {
    let mut s = state();
    if s.sessions.get(&uuid).is_some_and(|x| x.window == window) {
        s.sessions.remove(&uuid);
        s.clicks.retain(|c| c.0 != uuid);
    }
}

/// What the window shows of `list`.
fn shown(list: &[String]) -> Vec<Option<ItemStack>> {
    let mut out: Vec<Option<ItemStack>> = vec![None; SIZE];
    for (i, item) in list.iter().take(SIZE).enumerate() {
        let name = line("rustcraft.filter.marked", "Marked: click to unmark", &[]);
        let mut stack = ItemStack::new(item.as_str(), 1);
        let lore = [crate::game::rustcraft_window::styled(&name, "gray")];
        stack.data_mut().set_other("minecraft:lore", Some(Tag::List(lore.to_vec())));
        out[i] = Some(stack);
    }
    if list.is_empty() {
        let name = line("rustcraft.filter.empty", "Click an item of your inventory to mark it", &[]);
        out[SIZE / 2] = Some(named("minecraft:paper", 1, name, &[], false));
    }
    out
}

/// Each level tick: open the windows asked for, answer their clicks.
pub(super) fn tick(game: &mut Game<'_>) {
    let (opening, clicks) = {
        let mut s = state();
        (std::mem::take(&mut s.opening), std::mem::take(&mut s.clicks))
    };
    for (uuid, used) in opening {
        let Some(stacks) = game.native_player_stacks(uuid) else { continue };
        let Some(slot) = stacks.iter().position(|s| s.as_ref() == Some(&used)) else {
            continue;
        };
        let title = line("rustcraft.filter.title", "Filter module", &[]);
        if let Some(window) = game.native_open_menu(uuid, rules::ROWS, title, shown(&marked(&used))) {
            state().sessions.insert(uuid, Session { window, slot });
        }
    }
    for (uuid, window, clicked) in clicks {
        click(game, uuid, window, clicked);
    }
}

fn click(game: &mut Game<'_>, uuid: u128, window: i32, clicked: i32) {
    let Some(slot) = state().sessions.get(&uuid).filter(|x| x.window == window).map(|x| x.slot) else {
        return;
    };
    let Some(stacks) = game.native_player_stacks(uuid) else { return };
    let Some(module) = stacks.get(slot).cloned().flatten().filter(|s| &*s.item == FILTER) else {
        game.native_close_menu(uuid);
        state().sessions.remove(&uuid);
        return;
    };
    let mut list = marked(&module);
    let (place, item) = match usize::try_from(clicked) {
        Ok(c) if c < SIZE => (c, None),
        _ => {
            let inv = usize::try_from(clicked).ok().and_then(|c| c.checked_sub(SIZE)).and_then(|n| match n {
                0..27 => Some(n + 9),
                27..36 => Some(n - 27),
                _ => None,
            });
            let Some(item) = inv.and_then(|i| stacks.get(i).cloned().flatten()) else {
                return;
            };
            if &*item.item == FILTER {
                return;
            }
            (0, Some(item.item.to_string()))
        }
    };
    match rules::click(&mut list, place, item.as_deref()) {
        Click::Marked | Click::Unmarked => {
            let new = with_marked(&module, &list);
            if game
                .native_set_inventory_slot(uuid, NativeInventoryKind::Player, slot, Some(&module), Some(new))
                .is_err()
            {
                log::warn!("rustcraft: the filter module moved while its window was open");
                return;
            }
            game.native_set_menu_items(uuid, shown(&list));
        }
        Click::Full => {
            let n = Text::literal(rules::MAX_MARKED.to_string());
            tell(game, uuid, line("rustcraft.filter.full", "The filter marks at most %s items", &[n]));
        }
        Click::Nothing => {}
    }
}
