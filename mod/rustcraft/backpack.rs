// The backpack (spec 4.11 G4, guide `backpack`; rustcraft_rules::backpack has its numbers, the
// screen's layout, the slot rules and the messages).
//
// On the server:
// - Each backpack carries a number in its `minecraft:custom_data` compound (`rustcraft_backpack`),
//   given the first tick it is in a player's inventory (`native_inventory_tick`, where the carried
//   stack may change). What it holds lives on the server under that number, in the world's
//   `data/rustcraft_backpacks.dat` (binary NBT: `next`, `packs` (each a list of slots: the stack
//   saved with count 1, components kept, and its own `count`, which may pass 99; a file of slice 10
//   has the count inside the stack and still reads), `modules` (the stack module in each one's
//   socket) and `worn`). So the item in hand never changes while its window is open, and dropping
//   or moving it loses nothing.
// - Slice 18: the window is the mod's own screen on the player's client (API 111), as the storage
//   tablet's (rustcraft_tablet): a right click or the backpack key asks for it, the next tick sends
//   the client what it shows on VIEW_CHANNEL and a byte on OPEN_CHANNEL to open it
//   (`native_net_send_to_player`), and the client's presses come back on PRESS_CHANNEL
//   (`native_net_from_client`) for the next tick: a slot's stack (up to the item's own stack) or one
//   into the inventory (`native_give_stack`), a stack of the inventory into the backpack
//   (`native_set_inventory_slot`, checked against the stack it held; a stack module goes into the
//   empty socket first), the module back into the inventory. With a stack module a slot holds up to
//   1024 (`rules::cap`); the module stays in while a slot holds more than a plain stack, so taking
//   it out never loses anything. Another backpack does not go in (`rules::keeps`).
// - One window a backpack at a time: a copy of the same number (from creative) says it is open.
// - Slice 18: a sneaking right click puts starsteel's backpack on the back, a slot of the mod's own
//   (the engine has no equipment slot to add, rustcraft_rules::backpack::STAR): the server keeps
//   who wears which in the store's `worn`, the backpack key opens it, a sneaking press of the key
//   takes it off.
//
// On the client:
// - OPEN_CHANNEL opens or closes the screen at the next client tick (`native_mod_screen_open` with
//   the id `rustcraft:backpack`). `native_screen_render_post` draws it in the windows' style
//   (rustcraft_screens): the grid with each slot's item and count, the module socket, the player's
//   inventory from the client's own copy, the tooltip; `native_screen_mouse_button_pressed_pre`
//   takes its clicks. Escape closes it and the server hears so.
use crate::game::{
    Game,
    rustcraft_machines::max_stack,
    rustcraft_screens::{self as screens, tr, tr_with},
    rustcraft_server::{line, tell},
    rustcraft_terminal as terminal,
};
use horizon_client::{hud::HudBatch, items::ItemStack, mod_api::NativeScreenView, text::Text};
use horizon_items::item_data::StackData;
use horizon_world::nbt::{Compound, JavaString, Limits, Tag, read_network, write_network};
use rustcraft_rules::{
    backpack::{self as rules, OPEN_CHANNEL, PRESS_CHANNEL, Part, Press, ShownCell, VIEW_CHANNEL, View},
    machines::style,
    modules::Module,
    tuning::Tuning,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU8, Ordering},
    },
};

const CUSTOM_DATA: &str = "minecraft:custom_data";
const FILE: &str = "data/rustcraft_backpacks.dat";
/// The backpack's own screen's id (`native_mod_screen`).
pub(super) const SCREEN_ID: &str = "rustcraft:backpack";
const LEFT: i32 = 0;
const RIGHT: i32 = 1;

/// What a slot holds, counted apart: two stacks are one kind when their item, damage and
/// components are (the count aside), a stack with no components the same as one with empty ones.
#[derive(Clone, Debug)]
struct Key(ItemStack);

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        let none = StackData::default();
        let (a, b) = (&self.0, &other.0);
        a.item == b.item && a.damage == b.damage && a.data().unwrap_or(&none) == b.data().unwrap_or(&none)
    }
}

impl Key {
    fn of(stack: &ItemStack) -> Self {
        let mut one = stack.clone();
        one.count = 1;
        Key(one)
    }

    fn stack(&self, count: u32) -> ItemStack {
        let mut s = self.0.clone();
        s.count = count;
        s
    }
}

type Cell = rules::Cell<Key>;

/// A backpack's slots and the stack module in its socket.
#[derive(Clone, Debug, Default)]
struct Pack {
    cells: Vec<Cell>,
    module: Option<String>,
}

impl Pack {
    fn module(&self) -> Option<Module> {
        self.module.as_deref().and_then(Module::of)
    }
}

/// An open window: the backpack's number, its slots and its item (its name is the title).
struct Session {
    id: u64,
    slots: usize,
    backpack: String,
}

#[derive(Default)]
struct State {
    /// Loaded from the world's file once its folder is known.
    path: Option<PathBuf>,
    next: u64,
    packs: HashMap<u64, Pack>,
    /// Windows to open at the next tick: the player, the backpack's number, its slots and item.
    opening: Vec<(u128, u64, usize, String)>,
    /// Open windows by player.
    sessions: HashMap<u128, Session>,
    /// Presses of the windows, for the next tick.
    presses: Vec<(u128, Press)>,
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
    if let Some(Tag::Compound(packs)) = c.get(&JavaString::from("packs")) {
        for (key, tag) in packs.iter() {
            let (Ok(id), Tag::List(list)) = (key.to_string_lossy().parse::<u64>(), tag) else {
                continue;
            };
            s.packs.entry(id).or_default().cells = decode_cells(list);
        }
    }
    if let Some(Tag::Compound(modules)) = c.get(&JavaString::from("modules")) {
        for (key, tag) in modules.iter() {
            if let (Ok(id), Tag::String(item)) = (key.to_string_lossy().parse::<u64>(), tag) {
                s.packs.entry(id).or_default().module = Some(item.to_string_lossy());
            }
        }
    }
}

/// A backpack's slots: each entry's stack, and its count apart (`count`) or, in a file of slice
/// 10, inside the stack.
fn decode_cells(list: &[Tag]) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
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
        let count = match e.get(&JavaString::from("count")) {
            Some(Tag::Int(n)) => (*n).max(0) as u32,
            _ => stack.count,
        };
        if count == 0 || slot >= rules::MAX_SLOTS {
            continue;
        }
        if cells.len() <= slot {
            cells.resize(slot + 1, None);
        }
        cells[slot] = Some((Key::of(&stack), count));
    }
    cells
}

fn encode(s: &State) -> Compound {
    let mut packs = Compound::new();
    let mut modules = Compound::new();
    for (id, pack) in &s.packs {
        let list: Vec<Tag> = pack
            .cells
            .iter()
            .enumerate()
            .filter_map(|(i, cell)| {
                let (key, count) = cell.as_ref().filter(|(_, n)| *n > 0)?;
                Some(Tag::compound([
                    ("slot", Tag::Int(i as i32)),
                    ("item", horizon_items::item_nbt::item_to_nbt(&key.0)),
                    ("count", Tag::Int(i32::try_from(*count).unwrap_or(i32::MAX))),
                ]))
            })
            .collect();
        let name = JavaString::from(id.to_string().as_str());
        if !list.is_empty() {
            packs.insert(name.clone(), Tag::List(list));
        }
        if let Some(module) = &pack.module {
            modules.insert(name, Tag::String(JavaString::from(module.as_str())));
        }
    }
    let mut worn = Compound::new();
    for (player, id) in &s.worn {
        worn.insert(JavaString::from(format!("{player:032x}").as_str()), Tag::Long(*id as i64));
    }
    Compound::from([
        (JavaString::from("next"), Tag::Long(s.next as i64)),
        (JavaString::from("packs"), Tag::Compound(packs)),
        (JavaString::from("modules"), Tag::Compound(modules)),
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

fn pack(id: u64) -> Pack {
    state().packs.get(&id).cloned().unwrap_or_default()
}

fn set_pack(id: u64, pack: Pack) {
    let mut s = state();
    s.packs.insert(id, pack);
    save(&s);
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
    state().opening.push((player.uuid, id, slots, stack.item.to_string()));
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

/// The server's menu dispatch: the backpack opens no server menu since slice 18.
pub(super) fn on_menu_clicked(_: &mut Game<'_>, _: u128, _: i32, _: i32) {}

/// `native_menu_input_closed`: the backpack opens no chest window since slice 18.
pub(super) fn on_menu_input_closed(_: u128, _: i32, _: &mut [Option<ItemStack>]) {}

/// Slice 18: the backpack key (], rustcraft_gear_keys): the backpack on the back, else the first
/// backpack in the player's inventory, opens at the next tick.
pub(super) fn open_by_key(game: &mut Game<'_>, uuid: u128) {
    // The lock is let go before the branches: held in an `if let` scrutinee it lived through
    // the block, and `state()` there locked it again and hung the server (in-game check w3).
    let worn = state().worn.get(&uuid).copied();
    if let Some(id) = worn {
        let sneaking = game.native_server_players().into_iter().any(|p| p.uuid == uuid && p.sneaking);
        if sneaking {
            take_off(game, uuid, id);
        } else {
            state().opening.push((uuid, id, rules::STAR_SLOTS, rules::STAR.to_owned()));
        }
        return;
    }
    let t = crate::game::rustcraft_server::tuning();
    let found = game.native_player_stacks(uuid).and_then(|stacks| {
        stacks
            .into_iter()
            .flatten()
            .find_map(|st| Some((id_of(&st)?, rules::slots(&st.item, t)?, st.item.to_string())))
    });
    match found {
        Some((id, slots, item)) => state().opening.push((uuid, id, slots, item)),
        None => tell(game, uuid, line("rustcraft.backpack.none", "No backpack in the inventory", &[])),
    }
}

/// `native_net_from_client`: a press of a player's backpack window, for the next tick.
pub(super) fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel != PRESS_CHANNEL {
        return;
    }
    match rules::decode_press(data) {
        Some(p) => state().presses.push((player, p)),
        None => log::debug!("rustcraft: a backpack press from {player:032x} that does not read"),
    }
}

/// Once a server tick: the windows asked for open, and their presses are answered.
pub(super) fn tick(game: &mut Game<'_>) {
    let (opening, presses) = {
        let mut s = state();
        (std::mem::take(&mut s.opening), std::mem::take(&mut s.presses))
    };
    if opening.is_empty() && presses.is_empty() {
        return;
    }
    load(&mut state(), game);
    for (uuid, id, slots, backpack) in opening {
        open(game, uuid, id, slots, backpack);
    }
    for (uuid, p) in presses {
        press(game, uuid, p);
    }
}

fn open(game: &mut Game<'_>, uuid: u128, id: u64, slots: usize, backpack: String) {
    let busy = state().sessions.iter().any(|(u, x)| x.id == id && *u != uuid);
    if busy {
        tell(game, uuid, line("rustcraft.backpack.busy", "This backpack is open already", &[]));
        return;
    }
    let slots = slots.clamp(1, rules::MAX_SLOTS);
    state().sessions.insert(uuid, Session { id, slots, backpack });
    send_view(game, uuid);
    if let Err(e) = game.native_net_send_to_player(uuid, OPEN_CHANNEL, &[1]) {
        log::debug!("rustcraft: the backpack window not sent to {uuid:032x}: {e}");
    }
}

/// What the window shows of a backpack.
fn view_of(backpack: &str, slots: usize, pack: &Pack) -> View {
    let cells = (0..slots)
        .map(|i| {
            let (key, count) = pack.cells.get(i).cloned().flatten()?;
            Some(ShownCell { item: key.0.item.to_string(), nbt: saved(&key), count })
        })
        .collect();
    View { backpack: backpack.to_owned(), module: pack.module.clone(), cells }
}

/// A stack's saved form for the window when it is more than its item (worn, with components).
fn saved(key: &Key) -> Vec<u8> {
    let none = StackData::default();
    if key.0.damage == 0 && key.0.data().unwrap_or(&none) == &none {
        return Vec::new();
    }
    write_network(&horizon_items::item_nbt::item_to_nbt(&key.0), Limits::saved()).unwrap_or_default()
}

fn send_view(game: &mut Game<'_>, uuid: u128) {
    let view = {
        let s = state();
        let Some(x) = s.sessions.get(&uuid) else { return };
        let none = Pack::default();
        view_of(&x.backpack, x.slots, s.packs.get(&x.id).unwrap_or(&none))
    };
    if let Err(e) = game.native_net_send_to_player(uuid, VIEW_CHANNEL, &rules::encode_view(&view)) {
        log::debug!("rustcraft: the backpack's slots not sent to {uuid:032x}: {e}");
    }
}

fn press(game: &mut Game<'_>, uuid: u128, p: Press) {
    let Some((id, slots)) = state().sessions.get(&uuid).map(|x| (x.id, x.slots)) else {
        return;
    };
    match p {
        Press::Closed => {
            state().sessions.remove(&uuid);
            return;
        }
        Press::Take { slot, one } => take(game, uuid, id, usize::from(slot), one),
        Press::Deposit(inv) => deposit(game, uuid, id, slots, usize::from(inv)),
        Press::Socket => module_out(game, uuid, id),
    }
    send_view(game, uuid);
}

/// A slot's stack (up to the item's own stack) or one into the inventory; what does not fit stays.
fn take(game: &mut Game<'_>, uuid: u128, id: u64, slot: usize, one: bool) {
    let mut pack = pack(id);
    let Some((key, count)) = pack.cells.get(slot).cloned().flatten() else {
        return;
    };
    let want = rules::take_count(count, one, max_stack(game, &key.0.item));
    let left = game
        .native_give_stack(uuid, key.stack(want))
        .map_or(0, |r| r.count.min(want));
    if left == want {
        tell(game, uuid, line("rustcraft.backpack.inventory_full", "No room in your inventory", &[]));
        return;
    }
    rules::remove(&mut pack.cells, slot, want - left);
    set_pack(id, pack);
}

/// The stack in inventory slot `inv` into the backpack: a stack module into the empty socket, any
/// other stack onto its own stacks and into empty slots; the rest stays in the inventory.
fn deposit(game: &mut Game<'_>, uuid: u128, id: u64, slots: usize, inv: usize) {
    let Some(stack) = game
        .native_player_stacks(uuid)
        .and_then(|s| s.get(inv).cloned().flatten())
        .filter(|s| s.count > 0)
    else {
        return;
    };
    if !rules::keeps(&stack.item) {
        tell(game, uuid, line("rustcraft.backpack.nested", "A backpack does not go into a backpack", &[]));
        return;
    }
    let mut pack = pack(id);
    if pack.cells.len() < slots {
        pack.cells.resize(slots, None);
    }
    let put = if rules::fits_socket(&stack.item) && pack.module.is_none() {
        pack.module = Some(stack.item.to_string());
        1
    } else {
        let cap = rules::cap(pack.module(), max_stack(game, &stack.item));
        stack.count - rules::deposit(&mut pack.cells[..slots], &Key::of(&stack), stack.count, cap)
    };
    if put == 0 {
        tell(game, uuid, line("rustcraft.backpack.full", "No room in the backpack for it", &[]));
        return;
    }
    let rest = (put < stack.count).then(|| {
        let mut r = stack.clone();
        r.count -= put;
        r
    });
    let kind = crate::game::native_inventories::NativeInventoryKind::Player;
    if let Err(e) = game.native_set_inventory_slot(uuid, kind, inv, Some(&stack), rest) {
        log::debug!("rustcraft: a backpack deposit of {uuid:032x} refused: {e}");
        return;
    }
    set_pack(id, pack);
}

/// The stack module back into the inventory, unless a slot holds more than a plain stack.
fn module_out(game: &mut Game<'_>, uuid: u128, id: u64) {
    let mut pack = pack(id);
    let Some(module) = pack.module.clone() else { return };
    let counts: Vec<(u32, u32)> = pack
        .cells
        .iter()
        .map(|c| c.as_ref().map_or((0, 1), |(k, n)| (*n, max_stack(game, &k.0.item))))
        .collect();
    if let Some(i) = rules::overfull(counts) {
        let text = line(
            "rustcraft.backpack.module_held",
            "The stack module stays: slot %s holds more than a plain stack. Take the extra out first",
            &[Text::literal((i + 1).to_string())],
        );
        tell(game, uuid, text);
        return;
    }
    if game
        .native_give_stack(uuid, ItemStack::new(&module, 1))
        .is_some_and(|left| left.count > 0)
    {
        tell(game, uuid, line("rustcraft.backpack.inventory_full", "No room in your inventory", &[]));
        return;
    }
    pack.module = None;
    set_pack(id, pack);
}

/// `native_player_logged_out`: an open window is forgotten.
pub(super) fn on_logout(uuid: u128) {
    let mut s = state();
    s.sessions.remove(&uuid);
    s.opening.retain(|o| o.0 != uuid);
    s.presses.retain(|p| p.0 != uuid);
    s.wear.retain(|w| w.0 != uuid);
}

// ---- The self-test (rustcraft_selftest): the store without a player ----

/// The cell's item, damage and count, to compare cells that went through NBT.
fn plain(c: &Cell) -> Option<(String, u32, u32)> {
    c.as_ref().map(|(k, n)| (k.0.item.to_string(), k.0.damage, *n))
}

/// A deposit past 99 with a stack module, the module kept while a slot is past a plain stack, the
/// store saved and read back (counts past 99, the module, a worn tool), a file of slice 10 read,
/// and the window's message with the worn tool's damage.
pub(super) fn self_test_store(game: &Game<'_>) -> Result<(), String> {
    const STONE: &str = "minecraft:cobblestone";
    let stone = Key(ItemStack::new(STONE, 1));
    let mut pick = ItemStack::new("minecraft:iron_pickaxe", 1);
    pick.damage = 5;
    let cap = rules::cap(Some(Module::Stack2), max_stack(game, STONE));
    if cap != 1024 {
        return Err(format!("a slot with the crystallite stack module holds {cap} cobblestone, not 1024"));
    }
    let mut cells: Vec<Cell> = vec![Some((stone.clone(), 1000)), None, Some((Key(pick), 1)), None];
    let left = rules::deposit(&mut cells, &stone, 600, cap);
    let want = vec![
        Some((STONE.to_owned(), 0, 1024)),
        Some((STONE.to_owned(), 0, 576)),
        Some(("minecraft:iron_pickaxe".to_owned(), 5, 1)),
        None,
    ];
    let got: Vec<_> = cells.iter().map(plain).collect();
    if left != 0 || got != want {
        return Err(format!("600 cobblestone into the backpack: {got:?}, {left} left"));
    }
    let counts: Vec<(u32, u32)> = cells
        .iter()
        .map(|c| c.as_ref().map_or((0, 1), |(k, n)| (*n, max_stack(game, &k.0.item))))
        .collect();
    if rules::overfull(counts) != Some(0) {
        return Err("the stack module would leave with 1024 cobblestone in a slot".into());
    }
    let mut s = State::default();
    s.packs.insert(3, Pack { cells, module: Some(Module::Stack2.item().to_owned()) });
    let bytes = write_network(&Tag::Compound(encode(&s)), Limits::saved()).map_err(|e| format!("not saved: {e:?}"))?;
    let Ok((Tag::Compound(c), _)) = read_network(&bytes, Limits::saved()) else {
        return Err("the saved store does not read".into());
    };
    let mut back = State::default();
    decode(&mut back, &c);
    let pack = back.packs.get(&3).ok_or("backpack 3 is not in the store read back")?;
    let got: Vec<_> = (0..4).map(|i| plain(&pack.cells.get(i).cloned().flatten())).collect();
    if got != want || pack.module() != Some(Module::Stack2) {
        return Err(format!("the store read back holds {got:?} and {:?}", pack.module));
    }
    // A file of slice 10: the count inside the stack.
    let old_item = horizon_items::item_nbt::item_to_nbt(&ItemStack::new(STONE, 37));
    let old_list = Tag::List(vec![Tag::compound([("slot", Tag::Int(2)), ("item", old_item)])]);
    let old = Compound::from([(
        JavaString::from("packs"),
        Tag::Compound(Compound::from([(JavaString::from("9"), old_list)])),
    )]);
    let mut olds = State::default();
    decode(&mut olds, &old);
    let got = olds.packs.get(&9).map(|p| plain(&p.cells.get(2).cloned().flatten()));
    if got != Some(Some((STONE.to_owned(), 0, 37))) {
        return Err(format!("a backpack of slice 10 reads as {got:?}"));
    }
    let view = rules::decode_view(&rules::encode_view(&view_of(rules::STAR, 81, pack)))
        .ok_or("the window's message does not read")?;
    let tool = view.cells.get(2).cloned().flatten().ok_or("the window shows no pickaxe")?;
    let shown = shown_stack(&tool);
    if shown.damage != 5 || view.cells.len() != 81 || view.cells[0].as_ref().map(|c| c.count) != Some(1024) {
        return Err(format!("the window shows the pickaxe worn {} and slot 1 {:?}", shown.damage, view.cells[0]));
    }
    Ok(())
}

// ---- The client ----

/// What the client's window shows: the slots' stacks made once from the server's message.
struct Shown {
    backpack: String,
    module: Option<ItemStack>,
    cells: Vec<Option<(ItemStack, u32)>>,
}

impl Shown {
    fn module(&self) -> Option<Module> {
        self.module.as_ref().and_then(|m| Module::of(&m.item))
    }
}

/// What the server last asked the client: 0 nothing, 1 open, 2 close.
static ASKED: AtomicU8 = AtomicU8::new(0);
/// Whether this client's window is open as far as it knows (to tell the server it closed).
static OPEN: AtomicU8 = AtomicU8::new(0);
static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);
/// Presses made since the client's last tick (a click hook has no `&mut Game` to send with).
static PENDING: Mutex<Vec<Press>> = Mutex::new(Vec::new());

fn shown() -> MutexGuard<'static, Option<Shown>> {
    SHOWN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A slot's stack as the server saved it, or its bare item.
fn shown_stack(c: &ShownCell) -> ItemStack {
    let saved = match read_network(&c.nbt, Limits::saved()) {
        Ok((Tag::Compound(tag), _)) if !c.nbt.is_empty() => ItemStack::from_nbt(&tag),
        _ => None,
    };
    let mut stack = saved.unwrap_or_else(|| ItemStack::new(&c.item, 1));
    stack.count = 1;
    stack
}

/// `native_net_from_server` on a client: the window's slots, or a request to open or close it.
pub(super) fn on_net(channel: &str, data: &[u8]) {
    if channel == OPEN_CHANNEL {
        ASKED.store(if data.first() == Some(&1) { 1 } else { 2 }, Ordering::Relaxed);
    } else if channel == VIEW_CHANNEL {
        let Some(v) = rules::decode_view(data) else {
            log::warn!("rustcraft: a backpack window's slots that do not read");
            return;
        };
        *shown() = Some(Shown {
            backpack: v.backpack,
            module: v.module.map(|m| ItemStack::new(&m, 1)),
            cells: v.cells.iter().map(|c| c.as_ref().map(|c| (shown_stack(c), c.count))).collect(),
        });
    }
}

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
        log::debug!("rustcraft: a backpack press not sent: {e}");
    }
}

/// The client's tick: the window asked for opens or closes; a window the player closed is told
/// to the server.
pub(super) fn client_tick(game: &mut Game<'_>) {
    match ASKED.swap(0, Ordering::Relaxed) {
        1 => {
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

/// The window's corner: in the middle of the screen.
fn corner(screen: &NativeScreenView<'_>, size: [f32; 2]) -> [f32; 2] {
    [
        screen.rect[0] + ((screen.rect[2] - size[0]) / 2.0).floor(),
        screen.rect[1] + ((screen.rect[3] - size[1]) / 2.0).floor(),
    ]
}

/// «Рюкзак из искростали · 27 слотов», cut to the window when it is longer.
fn title(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], v: &Shown, width: f32) {
    let name = terminal::item_name(&v.backpack);
    let n = v.cells.len() as u64;
    let rest = screens::plural(n, "rustcraft.backpack.slots", ("%s slot", "%s slots", "%s slots"));
    let whole = format!("{name} · {rest}");
    let room = width - 16.0;
    if game.native_text_width(&whole) <= room {
        screens::title_line(game, batch, o, &name, Some(&rest), width);
    } else {
        screens::title_line(game, batch, o, &screens::fit(game, &whole, room), None, width);
    }
}

fn item_at(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], p: [f32; 2], stack: &ItemStack, count: Option<String>) {
    game.native_gui_draw_item_with(batch, stack, [o[0] + p[0], o[1] + p[1]], count.as_deref());
}

/// `native_screen_render_post`: the window, its slots with their items and counts, the socket, the
/// player's inventory from the client's own items, and the tooltip under the mouse.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !is_open(game) {
        return;
    }
    let guard = shown();
    let Some(v) = guard.as_ref() else { return };
    let l = rules::layout(v.cells.len());
    let o = corner(screen, l.size);
    screens::background(game, batch, o, l.size);
    title(game, batch, o, v, l.size[0]);
    for i in 0..l.slots {
        let p = l.slot(i);
        screens::slot(game, batch, o, p);
        screens::hover(game, batch, o, style::slot_rect(p));
        if let Some((stack, count)) = v.cells.get(i).and_then(Option::as_ref) {
            item_at(game, batch, o, p, stack, rules::count_text(*count));
        }
    }
    screens::socket(game, batch, o, rules::SOCKET, v.module.is_none());
    if let Some(m) = &v.module {
        item_at(game, batch, o, rules::SOCKET, m, None);
    }
    screens::inventory(game, batch, o, l.inventory, l.inventory_label[1]);
    for i in 0..rules::INVENTORY {
        let p = l.inventory_slot(i);
        screens::hover(game, batch, o, style::slot_rect(p));
        if let Some(stack) = game.inventory.get(i).cloned().flatten() {
            let count = (stack.count != 1).then(|| stack.count.to_string());
            item_at(game, batch, o, p, &stack, count);
        }
    }
    let at = [screen.mouse[0] - o[0], screen.mouse[1] - o[1]];
    let lines = l.at(at).map(|part| tooltip(game, v, part)).unwrap_or_default();
    if !lines.is_empty() {
        let texts: Vec<Text> = lines
            .iter()
            .map(|l| Text::literal(l.trim_start_matches("§7")))
            .collect();
        game.native_gui_tooltip(batch, &texts, game.native_cursor());
    }
}

/// The lines over a part of the window.
fn tooltip(game: &Game<'_>, v: &Shown, part: Part) -> Vec<String> {
    let mut lines = Vec::new();
    match part {
        Part::Slot(i) => {
            let Some((stack, count)) = v.cells.get(i).and_then(Option::as_ref) else {
                return lines;
            };
            let cap = rules::cap(v.module(), max_stack(game, &stack.item));
            lines.push(terminal::item_name(&stack.item));
            lines.extend(terminal::variant_lines(stack));
            lines.push(format!(
                "§7{}",
                tr_with("rustcraft.backpack.count", "%s of %s in the slot", &[count.to_string(), cap.to_string()])
            ));
            lines.push(tr(
                "rustcraft.backpack.take_hint",
                "Left click: a stack into the inventory · right click: one",
            ));
        }
        Part::Socket => match &v.module {
            Some(m) => {
                lines.push(terminal::item_name(&m.item));
                let cap = rules::cap(v.module(), 64);
                lines.push(format!(
                    "§7{}",
                    tr_with("rustcraft.backpack.socket_stack", "A slot stacks up to %s", &[cap.to_string()])
                ));
                lines.push(tr("rustcraft.backpack.socket_take", "Click: the module into the inventory"));
            }
            None => {
                lines.push(tr("rustcraft.backpack.socket_empty", "Stack module: stacks up to 1024"));
                lines.push(format!(
                    "§7{}",
                    tr("rustcraft.backpack.socket_put", "Click a stack module in the inventory to put it here")
                ));
            }
        },
        Part::Inventory(i) => {
            let Some(stack) = game.inventory.get(i).cloned().flatten() else {
                return lines;
            };
            lines.push(terminal::item_name(&stack.item));
            lines.extend(terminal::variant_lines(&stack));
            lines.push(if !rules::keeps(&stack.item) {
                tr("rustcraft.backpack.nested", "A backpack does not go into a backpack")
            } else if rules::fits_socket(&stack.item) && v.module.is_none() {
                tr("rustcraft.backpack.socket_hint", "Click: into the stack module socket")
            } else {
                tr("rustcraft.backpack.deposit_hint", "Click: into the backpack")
            });
        }
    }
    lines
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
    let press = {
        let guard = shown();
        let Some(v) = guard.as_ref() else { return };
        let l = rules::layout(v.cells.len());
        let o = corner(screen, l.size);
        match l.at([screen.mouse[0] - o[0], screen.mouse[1] - o[1]]) {
            Some(Part::Slot(i)) if v.cells.get(i).is_some_and(Option::is_some) => {
                Some(Press::Take { slot: i as u8, one: button == RIGHT })
            }
            Some(Part::Socket) if v.module.is_some() => Some(Press::Socket),
            Some(Part::Inventory(i)) if game.inventory.get(i).is_some_and(Option::is_some) => {
                Some(Press::Deposit(i as u8))
            }
            _ => None,
        }
    };
    if let Some(p) = press {
        queue(p);
    }
}

/// The backpack key with a starsteel backpack on the back queues its window for the next tick
/// (the key once hung the server: rustcraft_selftest `s18_backpack_store_holds_past_99`). A made-up
/// player, taken out again; nothing is saved.
pub(super) fn self_test_key(game: &mut Game<'_>) -> Result<(), String> {
    const PLAYER: u128 = 0x7e57_bac4_0000_0000_0000_0000_0000_0018;
    const ID: u64 = 0x7e57_0018;
    state().worn.insert(PLAYER, ID);
    open_by_key(game, PLAYER);
    let mut s = state();
    let queued = s.opening.iter().any(|(u, id, slots, _)| *u == PLAYER && *id == ID && *slots == rules::STAR_SLOTS);
    s.opening.retain(|(u, ..)| *u != PLAYER);
    s.worn.remove(&PLAYER);
    if queued {
        Ok(())
    } else {
        Err("the backpack key did not open the worn starsteel backpack".into())
    }
}
