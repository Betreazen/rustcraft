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
    /// Windows to open at the next tick: the player, the backpack's number and its slots.
    opening: Vec<(u128, u64, usize)>,
    /// Open windows: the player's window id and the backpack's number.
    open: HashMap<u128, (i32, u64)>,
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
    Compound::from([
        (JavaString::from("next"), Tag::Long(s.next as i64)),
        (JavaString::from("packs"), Tag::Compound(packs)),
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
    state().opening.push((player.uuid, id, slots));
}

/// Slice 18: the backpack key (B, rustcraft_gear_keys): the first backpack in the player's
/// inventory opens at the next tick.
pub(super) fn open_by_key(game: &mut Game<'_>, uuid: u128) {
    let t = crate::game::rustcraft_server::tuning();
    let found = game.native_player_stacks(uuid).and_then(|stacks| {
        stacks.into_iter().flatten().find_map(|st| Some((id_of(&st)?, rules::slots(&st.item, t)?)))
    });
    match found {
        Some((id, slots)) => state().opening.push((uuid, id, slots)),
        None => tell(game, uuid, line("rustcraft.backpack.none", "No backpack in the inventory", &[])),
    }
}

/// Slice 18: whether the player wears the starsteel backpack on their back (task 14 fills it in).
pub(super) fn wears_star(_player: u128) -> bool {
    false
}

/// Once a server tick: the windows asked for open.
pub(super) fn tick(game: &mut Game<'_>) {
    let opening = std::mem::take(&mut state().opening);
    for (uuid, id, slots) in opening {
        let items = {
            let mut s = state();
            load(&mut s, game);
            if s.open.values().any(|(_, open)| *open == id) {
                continue;
            }
            let mut items = s.packs.get(&id).cloned().unwrap_or_default();
            items.resize(slots, None);
            items
        };
        let title = line(
            "rustcraft.backpack.title",
            "Backpack · %s slots",
            &[Text::literal(slots.to_string())],
        );
        let rows = rules::rows(slots);
        if let Some(window) = game.native_open_menu_with_input(uuid, rows, title, items, 0..slots) {
            state().open.insert(uuid, (window, id));
        }
    }
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
    let kept: Vec<Option<ItemStack>> = slots
        .iter_mut()
        .map(|slot| {
            if slot.as_ref().is_some_and(|x| rules::keeps(&x.item)) {
                slot.take()
            } else {
                None
            }
        })
        .collect();
    s.packs.insert(id, kept);
    save(&s);
}

/// `native_player_logged_out`: a window not closed is forgotten (its close comes first).
pub(super) fn on_logout(uuid: u128) {
    state().open.remove(&uuid);
}
