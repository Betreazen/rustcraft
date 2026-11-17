// Slice 10's backpack on the server (spec 4.11 G4, guide `backpack`): a right click with it opens
// a chest window of 27 (sparksteel) or 54 (crystallite) slots that hold items as a chest's do
// (`native_open_menu_with_input`). rustcraft_rules::backpack has its numbers.
//
// - Each backpack carries a number in its `minecraft:custom_data` compound (`rustcraft_backpack`),
//   given the first tick it is in a player's inventory (`native_inventory_tick`, where the carried
//   stack may change). What it holds lives on the server under that number, in the world's
//   `data/rustcraft_backpacks.dat` (binary NBT: `next` and `packs`, each a list of slots with their
//   stacks, components kept). So the item in hand never changes while its window is open, and
//   dropping or moving it loses nothing.
// - When the window closes (`native_menu_input_closed`) its slots are kept; a backpack put inside
//   stays in the window's slots, and the engine gives it back to the player.
// - One window a backpack at a time: a copy of the same number (from creative) waits.
// - Slice 18: starsteel's backpack (81 slots) shows a page of 45 slots at a time, the sixth row
//   holding the page arrows: a click on one closes the window and opens the other page. A
//   sneaking right click puts it on the back, a slot of the mod's own (the engine has no
//   equipment slot to add, rustcraft_rules::backpack::STAR): the server keeps who wears which
//   in the store's `worn`, the backpack key opens it, a sneaking press of the key takes it off.
use crate::game::{
    Game,
    rustcraft_server::{line, tell},
};
use horizon_client::{items::ItemStack, text::Text};
use horizon_world::nbt::{Compound, JavaString, Limits, Tag, read_network, write_network};
use rustcraft_rules::{backpack as rules, tuning::Tuning};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const CUSTOM_DATA: &str = "minecraft:custom_data";
const FILE: &str = "data/rustcraft_backpacks.dat";

#[derive(Default)]
struct State {
    /// Loaded from the world's file once its folder is known.
    path: Option<PathBuf>,
    next: u64,
    packs: HashMap<u64, Vec<Option<ItemStack>>>,
    /// Windows to open at the next tick: the player, the backpack's number, its slots and page.
    opening: Vec<(u128, u64, usize, usize)>,
    /// Open windows: the player's window id and the backpack's number.
    open: HashMap<u128, (i32, u64)>,
    /// Slice 18: the page of an open paged window, and pages to open once the window closed.
    pages: HashMap<u128, usize>,
    turning: Vec<(u128, u64, usize)>,
    /// Slice 18: starsteel backpacks asked to go on the back, and who wears which.
    wear: Vec<(u128, u64)>,
    worn: HashMap<u128, u64>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The backpack's number, if it has one yet.
fn id_of(stack: &ItemStack) -> Option<u64> {
    match stack.custom_data()?.get(&JavaString::from(rules::ID_KEY))? {
        Tag::Long(v) => u64::try_from(*v).ok(),
        _ => None,
    }
}

fn set_id(stack: &mut ItemStack, id: u64) {
    let mut data = stack.custom_data().cloned().unwrap_or_default();
    data.insert(
        JavaString::from(rules::ID_KEY),
        Tag::Long(id.min(i64::MAX as u64) as i64),
    );
    stack
        .data_mut()
        .set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
}

// ---- The store ----

fn load(s: &mut State, game: &Game<'_>) {
    if s.path.is_some() {
        return;
    }
    let Some(world) = game.native_world_dir() else {
        return;
    };
    let path = world.join(FILE);
    if let Ok(bytes) = std::fs::read(&path) {
        match read_network(&bytes, Limits::saved()) {
            Ok((Tag::Compound(c), _)) => decode(s, &c),
            _ => log::warn!(
                "rustcraft: {} does not read; backpacks start empty",
                path.display()
            ),
        }
    }
    s.path = Some(path);
}

fn decode(s: &mut State, c: &Compound) {
    if let Some(Tag::Long(n)) = c.get(&JavaString::from("next")) {
        s.next = (*n).max(0) as u64;
    }
    if let Some(Tag::Compound(worn)) = c.get(&JavaString::from("worn")) {
        for (key, tag) in worn.iter() {
            if let (Ok(player), Tag::Long(id)) = (u128::from_str_radix(&key.to_string_lossy(), 16), tag) {
                s.worn.insert(player, (*id).max(0) as u64);
            }
        }
    }
    let Some(Tag::Compound(packs)) = c.get(&JavaString::from("packs")) else {
        return;
    };
    for (key, tag) in packs.iter() {
        let Ok(id) = key.to_string_lossy().parse::<u64>() else {
            continue;
        };
        let Tag::List(list) = tag else { continue };
        let mut slots: Vec<Option<ItemStack>> = Vec::new();
        for entry in list {
            let Tag::Compound(e) = entry else { continue };
            let slot = match e.get(&JavaString::from("slot")) {
                Some(Tag::Int(i)) => (*i).max(0) as usize,
                _ => continue,
            };
            let Some(Tag::Compound(item)) = e.get(&JavaString::from("item")) else {
                continue;
            };
            let Some(stack) = ItemStack::from_nbt(item) else {
                continue;
            };
            if slots.len() <= slot {
                slots.resize(slot + 1, None);
            }
            slots[slot] = Some(stack);
        }
        s.packs.insert(id, slots);
    }
}

fn encode(s: &State) -> Compound {
    let mut packs = Compound::new();
    for (id, slots) in &s.packs {
        let list: Vec<Tag> = slots
            .iter()
            .enumerate()
            .filter_map(|(i, st)| {
                let st = st.as_ref().filter(|x| x.count > 0)?;
                let item = horizon_items::item_nbt::item_to_nbt(st);
                Some(Tag::compound([
                    ("slot", Tag::Int(i as i32)),
                    ("item", item),
                ]))
            })
            .collect();
        if !list.is_empty() {
            packs.insert(JavaString::from(id.to_string().as_str()), Tag::List(list));
        }
    }
    let mut worn = Compound::new();
    for (player, id) in &s.worn {
        worn.insert(JavaString::from(format!("{player:032x}").as_str()), Tag::Long(*id as i64));
    }
    Compound::from([
        (JavaString::from("next"), Tag::Long(s.next as i64)),
        (JavaString::from("packs"), Tag::Compound(packs)),
        (JavaString::from("worn"), Tag::Compound(worn)),
    ])
}

/// Writes the store to the world's file: a new file first, then moved over the old one.
fn save(s: &State) {
    let Some(path) = s.path.as_ref() else {
        log::warn!("rustcraft: backpacks changed before the world's folder was known");
        return;
    };
    let bytes = match write_network(&Tag::Compound(encode(s)), Limits::saved()) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("rustcraft: backpacks not saved: {e:?}");
            return;
        }
    };
    let fresh = path.with_extension("dat_new");
    let done = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&fresh, &bytes))
        .and_then(|()| std::fs::rename(&fresh, path));
    if let Err(e) = done {
        log::warn!("rustcraft: backpacks not saved to {}: {e}", path.display());
    }
}

// ---- Hooks ----

/// `native_inventory_tick`: a backpack in a player's inventory without a number gets one.
pub(super) fn on_inventory_tick(game: &mut Game<'_>, slot: i32) {
    put_on(game, slot);
    let needs = game
        .native_carried_stack(slot)
        .is_some_and(|s| rules::is_backpack(&s.item) && id_of(s).is_none());
    if !needs {
        return;
    }
    let id = {
        let mut s = state();
        load(&mut s, game);
        s.next += 1;
        let id = s.next;
        save(&s);
        id
    };
    if let Some(stack) = game.native_carried_stack(slot) {
        set_id(stack, id);
    }
}

/// `native_use_item`: a right click with a backpack opens it at the next tick.
pub(super) fn on_use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool, t: &Tuning) {
    if *claimed || !rules::is_backpack(&stack.item) {
        return;
    }
    let Some(slots) = rules::slots(&stack.item, t) else {
        return;
    };
    let Some(player) = game.native_acting_player() else {
        return;
    };
    *claimed = true;
    let Some(id) = id_of(stack) else {
        tell(
            game,
            player.uuid,
            line(
                "rustcraft.backpack.not_ready",
                "The backpack is getting ready, try again",
                &[],
            ),
        );
        return;
    };
    if *stack.item == *rules::STAR && player.sneaking {
        if state().worn.contains_key(&player.uuid) {
            tell(game, player.uuid, line("rustcraft.backpack.wearing", "A backpack is on your back already", &[]));
        } else {
            state().wear.push((player.uuid, id));
        }
        return;
    }
    state().opening.push((player.uuid, id, slots, 0));
}

// ---- Slice 18: starsteel's backpack on the back ----

/// A starsteel backpack asked to go on the back leaves the inventory slot it is carried in.
fn put_on(game: &mut Game<'_>, slot: i32) {
    if state().wear.is_empty() {
        return;
    }
    let Some(id) = game.native_carried_stack(slot).filter(|s| *s.item == *rules::STAR).and_then(|s| id_of(s)) else {
        return;
    };
    let Some(player) = game.native_acting_player() else { return };
    {
        let mut s = state();
        let Some(i) = s.wear.iter().position(|w| *w == (player.uuid, id)) else { return };
        s.wear.remove(i);
        if s.worn.contains_key(&player.uuid) {
            return;
        }
        load(&mut s, game);
        s.worn.insert(player.uuid, id);
        save(&s);
    }
    if let Some(stack) = game.native_carried_stack(slot) {
        stack.count = 0;
    }
    tell(game, player.uuid, line("rustcraft.backpack.worn", "The backpack is on your back", &[]));
}

/// Whether the player wears a starsteel backpack on their back.
pub(super) fn wears_star(player: u128) -> bool {
    state().worn.contains_key(&player)
}

/// A sneaking press of the backpack key: the backpack on the back goes back to the inventory.
fn take_off(game: &mut Game<'_>, uuid: u128, id: u64) {
    let mut stack = ItemStack::new(rules::STAR, 1);
    set_id(&mut stack, id);
    if game.native_give_stack(uuid, stack).is_some_and(|left| left.count > 0) {
        tell(game, uuid, line("rustcraft.backpack.no_room", "No room in the inventory to take the backpack off", &[]));
        return;
    }
    let mut s = state();
    s.worn.remove(&uuid);
    save(&s);
    drop(s);
    tell(game, uuid, line("rustcraft.backpack.taken_off", "The backpack is off your back", &[]));
}

/// `native_menu_clicked`: a page arrow of a paged backpack window turns the page: the window
/// closes (keeping this page) and the other opens at the next tick.
pub(super) fn on_menu_clicked(game: &mut Game<'_>, uuid: u128, window: i32, slot: i32) {
    let turn = {
        let s = state();
        let Some(&(open, id)) = s.open.get(&uuid) else { return };
        let Some(&page) = s.pages.get(&uuid) else { return };
        if open != window {
            return;
        }
        usize::try_from(slot).ok().and_then(|slot| rules::turn(rules::STAR_SLOTS, page, slot)).map(|to| (id, to))
    };
    if let Some((id, to)) = turn {
        state().turning.push((uuid, id, to));
        game.native_close_menu(uuid);
    }
}

/// Slice 18: the backpack key (B, rustcraft_gear_keys): the first backpack in the player's
/// inventory opens at the next tick.
pub(super) fn open_by_key(game: &mut Game<'_>, uuid: u128) {
    if let Some(id) = state().worn.get(&uuid).copied() {
        let sneaking = game.native_server_players().into_iter().any(|p| p.uuid == uuid && p.sneaking);
        if sneaking {
            take_off(game, uuid, id);
        } else {
            state().opening.push((uuid, id, rules::STAR_SLOTS, 0));
        }
        return;
    }
    let t = crate::game::rustcraft_server::tuning();
    let found = game.native_player_stacks(uuid).and_then(|stacks| {
        stacks.into_iter().flatten().find_map(|st| Some((id_of(&st)?, rules::slots(&st.item, t)?)))
    });
    match found {
        Some((id, slots)) => state().opening.push((uuid, id, slots, 0)),
        None => tell(game, uuid, line("rustcraft.backpack.none", "No backpack in the inventory", &[])),
    }
}

/// Once a server tick: the windows asked for open.
pub(super) fn tick(game: &mut Game<'_>) {
    // Pages turned: opened once their window has closed.
    {
        let mut s = state();
        let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut s.turning)
            .into_iter()
            .partition(|(_, id, _)| !s.open.values().any(|(_, open)| open == id));
        s.turning = waiting;
        for (uuid, id, page) in ready {
            s.opening.push((uuid, id, rules::STAR_SLOTS, page));
        }
    }
    let opening = std::mem::take(&mut state().opening);
    for (uuid, id, slots, page) in opening {
        let paged = rules::pages(slots) > 1;
        let items = {
            let mut s = state();
            load(&mut s, game);
            if s.open.values().any(|(_, open)| *open == id) {
                continue;
            }
            let mut items = s.packs.get(&id).cloned().unwrap_or_default();
            items.resize(slots, None);
            if paged {
                page_items(&items, slots, page)
            } else {
                items
            }
        };
        let title = if paged {
            line(
                "rustcraft.backpack.title_page",
                "Backpack · %s slots · page %s of %s",
                &[
                    Text::literal(slots.to_string()),
                    Text::literal((page + 1).to_string()),
                    Text::literal(rules::pages(slots).to_string()),
                ],
            )
        } else {
            line(
                "rustcraft.backpack.title",
                "Backpack · %s slots",
                &[Text::literal(slots.to_string())],
            )
        };
        let (rows, input) = if paged { (6, 0..rules::PAGE) } else { (rules::rows(slots), 0..slots) };
        if let Some(window) = game.native_open_menu_with_input(uuid, rows, title, items, input) {
            let mut s = state();
            s.open.insert(uuid, (window, id));
            if paged {
                s.pages.insert(uuid, page);
            } else {
                s.pages.remove(&uuid);
            }
        }
    }
}

/// A paged window's 54 slots: the page's backpack slots, then the arrows' row.
fn page_items(items: &[Option<ItemStack>], slots: usize, page: usize) -> Vec<Option<ItemStack>> {
    let range = rules::page_range(slots, page);
    let mut out: Vec<Option<ItemStack>> = items.get(range).map(<[_]>::to_vec).unwrap_or_default();
    out.resize(rules::PAGE, None);
    out.resize(54, None);
    if rules::turn(slots, page, rules::PREV_SLOT).is_some() {
        out[rules::PREV_SLOT] = Some(ItemStack::new("minecraft:arrow", 1));
    }
    if rules::turn(slots, page, rules::NEXT_SLOT).is_some() {
        out[rules::NEXT_SLOT] = Some(ItemStack::new("minecraft:spectral_arrow", 1));
    }
    out
}

/// `native_menu_input_closed`: the backpack keeps what its window holds; another backpack stays
/// in the slots and so goes back to the player.
pub(super) fn on_menu_input_closed(uuid: u128, window: i32, slots: &mut [Option<ItemStack>]) {
    let mut s = state();
    let Some(&(open, id)) = s.open.get(&uuid) else {
        return;
    };
    if open != window {
        return;
    }
    s.open.remove(&uuid);
    let page = s.pages.remove(&uuid);
    let mut kept: Vec<Option<ItemStack>> = slots
        .iter_mut()
        .map(|slot| {
            if slot.as_ref().is_some_and(|x| rules::keeps(&x.item)) {
                slot.take()
            } else {
                None
            }
        })
        .collect();
    // Slice 18: a page keeps its own slots of the backpack; the arrows' row is not its.
    if let Some(page) = page {
        kept.truncate(rules::PAGE);
        let range = rules::page_range(rules::STAR_SLOTS, page);
        let pack = s.packs.entry(id).or_default();
        pack.resize(rules::STAR_SLOTS, None);
        for (i, at) in range.enumerate() {
            pack[at] = kept.get(i).cloned().flatten();
        }
        save(&s);
        return;
    }
    s.packs.insert(id, kept);
    save(&s);
}

/// `native_player_logged_out`: a window not closed is forgotten (its close comes first).
pub(super) fn on_logout(uuid: u128) {
    let mut s = state();
    s.open.remove(&uuid);
    s.pages.remove(&uuid);
    s.turning.retain(|t| t.0 != uuid);
    s.wear.retain(|w| w.0 != uuid);
}
