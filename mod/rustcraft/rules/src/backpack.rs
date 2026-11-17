//! The backpack (spec 4.11 G4, guide `backpack`): an item in an ordinary inventory slot that
//! opens a window of its own on a right click or the backpack key. Sparksteel's has 27 slots,
//! crystallite's 54 (`Tuning::backpack_slots`), starsteel's 81 and goes on the back. Each backpack
//! carries a number in its `minecraft:custom_data` component; what it holds lives on the server
//! under that number, so the item in hand never changes while it is open.
//!
//! Slice 18: the window is the mod's own screen (API 111), because a game menu's slot holds at most
//! 99 (SRC/docs/native-mods.md, «Menu slot rules») and a backpack's slot with a stack module holds
//! up to 1024 (guide `mod_stack`): a grid of nine columns, the stack module's socket, and the
//! player's inventory under the grid (beside it for 81 slots, so the window fits 480 × 270 GUI
//! pixels, GUI scale 4 on 1920 × 1080). The server keeps the slots and answers the presses; this
//! file has the layout, the slot rules and the messages both sides send.

use crate::modules::{self, Holder, Module};
use crate::tuning::Tuning;

/// The backpacks by tier: sparksteel's, crystallite's.
pub const ITEMS: [&str; 2] = ["rustcraft:backpack", "rustcraft:backpack_2"];
/// The key of its number in its `minecraft:custom_data` compound.
pub const ID_KEY: &str = "rustcraft_backpack";

/// Slice 18: starsteel's backpack (guide `backpack`): 81 slots, worn in its own slot on the back.
/// The engine has no equipment slot a mod may add for players (SRC crates/horizon-world/src/
/// player_inventory.rs: EquipmentSlot is vanilla's eight), so the back slot is the mod's own: the
/// server keeps which backpack each player wears, and clients draw it on the back.
pub const STAR: &str = "rustcraft:backpack_3";
pub const STAR_SLOTS: usize = 81;

/// Whether `item` is a backpack.
pub fn is_backpack(item: &str) -> bool {
    ITEMS.contains(&item) || item == STAR
}

/// How many slots the backpack `item` has.
pub fn slots(item: &str, t: &Tuning) -> Option<usize> {
    if item == STAR {
        return Some(STAR_SLOTS);
    }
    let i = ITEMS.iter().position(|b| *b == item)?;
    t.backpack_slots.get(i).copied()
}

/// What stays in a backpack: anything but another backpack, which goes back to the player (a
/// backpack in a backpack would hide one store inside another).
pub fn keeps(item: &str) -> bool {
    !is_backpack(item)
}

// ---- Slice 18: the screen's layout ----

/// The most slots a backpack has, and the player's inventory slots (0..9 the hotbar).
pub const MAX_SLOTS: usize = STAR_SLOTS;
pub const INVENTORY: usize = 36;
const COLUMNS: usize = 9;
/// From one slot's item to the next.
const PITCH: f32 = 18.0;
/// The grid's first slot's item, and the stack module's socket's, right of the grid's top row.
pub const GRID: [f32; 2] = [8.0, 18.0];
pub const SOCKET: [f32; 2] = [178.0, 18.0];
/// The window's edge past the last slot's well.
const MARGIN: f32 = 8.0;
/// The most grid rows the inventory goes under; past them it goes beside the grid, under the
/// socket.
const ROWS_UNDER: usize = 6;

/// Where the window's parts are, in GUI pixels from its corner (items' top left corners).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub slots: usize,
    pub size: [f32; 2],
    /// The inventory's label and its first main slot (the hotbar is 58 under it).
    pub inventory_label: [f32; 2],
    pub inventory: [f32; 2],
}

/// A part of the window under the mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// A backpack slot.
    Slot(usize),
    /// The stack module's socket.
    Socket,
    /// An inventory slot (0..9 the hotbar, 9..36 the rest).
    Inventory(usize),
}

/// The window of a backpack of `slots` slots.
pub fn layout(slots: usize) -> Layout {
    let slots = slots.clamp(1, MAX_SLOTS);
    let rows = slots.div_ceil(COLUMNS);
    // The bottom of the grid's last well.
    let grid_bottom = GRID[1] + rows as f32 * PITCH - 1.0;
    let (inventory_label, inventory) = if rows <= ROWS_UNDER {
        ([GRID[0], grid_bottom + 4.0], [GRID[0], grid_bottom + 15.0])
    } else {
        let under = SOCKET[1] + PITCH - 1.0;
        ([SOCKET[0], under + 7.0], [SOCKET[0], under + 18.0])
    };
    // The hotbar's wells end 58 + 17 under the inventory's first item.
    let bottom = grid_bottom.max(inventory[1] + 58.0 + 17.0);
    let right = (SOCKET[0] + 17.0).max(inventory[0] + 8.0 * PITCH + 17.0);
    Layout {
        slots,
        size: [right + MARGIN - 1.0, bottom + MARGIN - 1.0],
        inventory_label,
        inventory,
    }
}

/// Whether `at` is on the well of the slot whose item is at `item`.
fn on(item: [f32; 2], at: [f32; 2]) -> bool {
    at[0] >= item[0] - 1.0 && at[0] < item[0] + 17.0 && at[1] >= item[1] - 1.0 && at[1] < item[1] + 17.0
}

impl Layout {
    /// Backpack slot `i`'s item.
    pub fn slot(&self, i: usize) -> [f32; 2] {
        [
            GRID[0] + (i % COLUMNS) as f32 * PITCH,
            GRID[1] + (i / COLUMNS) as f32 * PITCH,
        ]
    }

    /// Inventory slot `i`'s item, as a menu draws it: the hotbar under the rest.
    pub fn inventory_slot(&self, i: usize) -> [f32; 2] {
        let [x, y] = self.inventory;
        if i < COLUMNS {
            [x + i as f32 * PITCH, y + 58.0]
        } else {
            let k = i - COLUMNS;
            [x + (k % COLUMNS) as f32 * PITCH, y + (k / COLUMNS) as f32 * PITCH]
        }
    }

    /// The part at `at`.
    pub fn at(&self, at: [f32; 2]) -> Option<Part> {
        if on(SOCKET, at) {
            return Some(Part::Socket);
        }
        (0..self.slots)
            .find(|&i| on(self.slot(i), at))
            .map(Part::Slot)
            .or_else(|| (0..INVENTORY).find(|&i| on(self.inventory_slot(i), at)).map(Part::Inventory))
    }
}

// ---- Slice 18: the slots and the stack module ----

/// A backpack slot: what it holds (the stack, counted apart) and how many.
pub type Cell<K> = Option<(K, u32)>;

/// Whether `item` goes into the backpack's socket: a stack module of any tier.
pub fn fits_socket(item: &str) -> bool {
    Module::of(item).is_some_and(|m| Holder::Backpack.takes(m))
}

/// How many of an item the game stacks to `item_max` a slot holds with `module` in the socket.
pub fn cap(module: Option<Module>, item_max: u32) -> u32 {
    modules::backpack_stack(&[module], item_max)
}

/// Puts `count` of `key` into the slots, each up to `cap`: onto the slots holding it first, then
/// into empty ones. What is left over.
pub fn deposit<K: PartialEq + Clone>(cells: &mut [Cell<K>], key: &K, count: u32, cap: u32) -> u32 {
    let mut left = count;
    for cell in cells.iter_mut() {
        if left == 0 {
            return 0;
        }
        if let Some((k, n)) = cell
            && k == key
            && *n < cap
        {
            let put = (cap - *n).min(left);
            *n += put;
            left -= put;
        }
    }
    for cell in cells.iter_mut() {
        if left == 0 || cap == 0 {
            break;
        }
        if cell.is_none() {
            let put = cap.min(left);
            *cell = Some((key.clone(), put));
            left -= put;
        }
    }
    left
}

/// How many a press on a slot holding `count` takes: one (right click) or a stack of the item
/// (`item_max`, the inventory's own stack).
pub fn take_count(count: u32, one: bool, item_max: u32) -> u32 {
    if one { count.min(1) } else { count.min(item_max.max(1)) }
}

/// Takes `n` out of slot `i`; an emptied slot is empty.
pub fn remove<K>(cells: &mut [Cell<K>], i: usize, n: u32) {
    let Some(cell) = cells.get_mut(i) else { return };
    if let Some((_, count)) = cell {
        *count = count.saturating_sub(n);
        if *count == 0 {
            *cell = None;
        }
    }
}

/// Whether the module may leave the socket, `slots` each slot's count and its item's own stack:
/// not while some slot holds more than a slot holds without it (taking it out would lose the
/// rest); then the first such slot.
pub fn overfull(slots: impl IntoIterator<Item = (u32, u32)>) -> Option<usize> {
    slots.into_iter().position(|(count, item_max)| count > cap(None, item_max))
}

/// The count a slot shows: none for one, past 999 in thousands («1k»), as four digits do not fit a
/// slot; the tooltip has the whole count.
pub fn count_text(n: u32) -> Option<String> {
    match n {
        1 => None,
        0..=999 => Some(n.to_string()),
        _ => Some(format!("{}k", n / 1000)),
    }
}

// ---- Slice 18: the messages ----

/// Server to client: one byte, 1 to open the window, 0 to close it.
pub const OPEN_CHANNEL: &str = "rustcraft:backpack_open";
/// Server to client: what the window shows (`encode_view`).
pub const VIEW_CHANNEL: &str = "rustcraft:backpack_view";
/// Client to server: a press of the window (`encode_press`).
pub const PRESS_CHANNEL: &str = "rustcraft:backpack_press";
/// The largest message a mod may send (SRC/docs/native-mods.md, API 40).
pub const MAX_BYTES: usize = 30_000;

/// A press of the backpack's window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// Backpack slot `slot` into the inventory: a stack of the item, or one.
    Take { slot: u8, one: bool },
    /// The stack in this inventory slot into the backpack (a stack module into the empty socket).
    Deposit(u8),
    /// The module in the socket back into the inventory.
    Socket,
    /// The player closed the window.
    Closed,
}

pub fn encode_press(p: Press) -> Vec<u8> {
    match p {
        Press::Take { slot, one } => vec![0, slot, u8::from(one)],
        Press::Deposit(slot) => vec![1, slot],
        Press::Socket => vec![2],
        Press::Closed => vec![3],
    }
}

/// What `encode_press` wrote; none for anything else, a slot past 81 or an inventory slot past 36.
pub fn decode_press(bytes: &[u8]) -> Option<Press> {
    match bytes {
        [0, slot, one @ (0 | 1)] if usize::from(*slot) < MAX_SLOTS => Some(Press::Take { slot: *slot, one: *one == 1 }),
        [1, slot] if usize::from(*slot) < INVENTORY => Some(Press::Deposit(*slot)),
        [2] => Some(Press::Socket),
        [3] => Some(Press::Closed),
        _ => None,
    }
}

/// One slot as the window shows it: the item, its stack as the game saves it (network NBT of one
/// item, components kept; empty when it did not fit the message, then the bare item is drawn),
/// and the count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownCell {
    pub item: String,
    pub nbt: Vec<u8>,
    pub count: u32,
}

/// What the window shows: the backpack's item (its name is the title), its slots, the module in
/// the socket.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub backpack: String,
    pub module: Option<String>,
    pub cells: Vec<Option<ShownCell>>,
}

/// The view as one message, at most MAX_BYTES: slots whose saved stack would not fit go without it.
pub fn encode_view(v: &View) -> Vec<u8> {
    let module = v.module.as_deref().unwrap_or("");
    let cells = &v.cells[..v.cells.len().min(MAX_SLOTS)];
    // Everything but the saved stacks, which go while there is room, in slot order.
    let fixed = 1
        + text_size(&v.backpack)
        + text_size(module)
        + 1
        + cells
            .iter()
            .map(|c| 1 + c.as_ref().map_or(0, |c| text_size(&c.item) + 4 + 2))
            .sum::<usize>();
    let mut room = MAX_BYTES.saturating_sub(fixed);
    let mut out = Vec::with_capacity(fixed.min(MAX_BYTES));
    out.push(VIEW_VERSION);
    put_text(&mut out, &v.backpack);
    put_text(&mut out, module);
    out.push(cells.len() as u8);
    for c in cells {
        let Some(c) = c else {
            out.push(0);
            continue;
        };
        out.push(1);
        put_text(&mut out, &c.item);
        out.extend(c.count.to_le_bytes());
        let nbt: &[u8] = if c.nbt.len() <= room && c.nbt.len() <= usize::from(u16::MAX) {
            room -= c.nbt.len();
            &c.nbt
        } else {
            &[]
        };
        out.extend((nbt.len() as u16).to_le_bytes());
        out.extend(nbt);
    }
    out
}

pub fn decode_view(bytes: &[u8]) -> Option<View> {
    let mut r = Reader(bytes);
    if r.byte()? != VIEW_VERSION {
        return None;
    }
    let backpack = r.text()?;
    let module = Some(r.text()?).filter(|m| !m.is_empty());
    let n = usize::from(r.byte()?);
    if n > MAX_SLOTS {
        return None;
    }
    let mut cells = Vec::with_capacity(n);
    for _ in 0..n {
        cells.push(match r.byte()? {
            0 => None,
            1 => {
                let item = r.text()?;
                let count = u32::from_le_bytes(r.take(4)?.try_into().ok()?);
                let len = usize::from(u16::from_le_bytes(r.take(2)?.try_into().ok()?));
                let nbt = r.take(len)?.to_vec();
                Some(ShownCell { item, nbt, count })
            }
            _ => return None,
        });
    }
    r.0.is_empty().then_some(View { backpack, module, cells })
}

const VIEW_VERSION: u8 = 1;

/// A text's size in the message: its length in two bytes, then its bytes.
fn text_size(s: &str) -> usize {
    2 + s.len().min(usize::from(u16::MAX))
}

fn put_text(out: &mut Vec<u8>, s: &str) {
    let bytes = &s.as_bytes()[..s.len().min(usize::from(u16::MAX))];
    out.extend((bytes.len() as u16).to_le_bytes());
    out.extend(bytes);
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    fn text(&mut self) -> Option<String> {
        let len = usize::from(u16::from_le_bytes(self.take(2)?.try_into().ok()?));
        String::from_utf8(self.take(len)?.to_vec()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backpack_has_its_tiers_slots() {
        let t = Tuning::default();
        assert_eq!(slots("rustcraft:backpack", &t), Some(27));
        assert_eq!(slots("rustcraft:backpack_2", &t), Some(54));
        assert_eq!(slots(STAR, &t), Some(81));
        assert_eq!(slots("minecraft:chest", &t), None);
    }

    #[test]
    fn a_backpack_keeps_anything_but_a_backpack() {
        assert!(keeps("minecraft:diamond_sword"));
        assert!(keeps("minecraft:shulker_box"));
        assert!(!keeps("rustcraft:backpack"));
        assert!(!keeps("rustcraft:backpack_2"));
        assert!(is_backpack(STAR) && !keeps(STAR));
    }

    /// Whether the window fits the GUI of scale 4 on 1920 × 1080.
    fn fits(l: &Layout) -> bool {
        l.size[0] <= 480.0 && l.size[1] <= 270.0
    }

    #[test]
    fn the_inventory_goes_under_the_grid_and_beside_it_for_81() {
        let small = layout(27);
        assert_eq!(small.size, [202.0, 168.0]);
        assert_eq!(small.inventory, [8.0, 86.0]);
        let middle = layout(54);
        assert_eq!(middle.size, [202.0, 222.0]);
        assert_eq!(middle.inventory, [8.0, 140.0]);
        let star = layout(81);
        assert_eq!(star.inventory[0], SOCKET[0], "beside the grid, under the socket");
        assert!(star.inventory[1] > SOCKET[1] + 16.0);
        for l in [small, middle, star] {
            assert!(fits(&l), "{l:?}");
            // The last slot and the hotbar's last slot are inside the window.
            let last = l.slot(l.slots - 1);
            let hot = l.inventory_slot(8);
            assert!(last[0] + 17.0 < l.size[0] && last[1] + 17.0 < l.size[1], "{l:?}");
            assert!(hot[0] + 17.0 < l.size[0] && hot[1] + 17.0 < l.size[1], "{l:?}");
        }
        assert_eq!(star.slot(80), [8.0 + 8.0 * 18.0, 18.0 + 8.0 * 18.0]);
        assert_eq!(small.inventory_slot(0), [8.0, 86.0 + 58.0], "the hotbar under the rest");
        assert_eq!(small.inventory_slot(9), [8.0, 86.0]);
        assert_eq!(small.inventory_slot(35), [8.0 + 8.0 * 18.0, 86.0 + 36.0]);
    }

    #[test]
    fn a_click_finds_its_part() {
        let l = layout(27);
        assert_eq!(l.at([8.0, 18.0]), Some(Part::Slot(0)));
        assert_eq!(l.at([7.0, 17.0]), Some(Part::Slot(0)), "the well's edge");
        assert_eq!(l.at([8.0 + 18.0 * 8.0 + 5.0, 18.0 + 36.0 + 5.0]), Some(Part::Slot(26)));
        assert_eq!(l.at([SOCKET[0] + 3.0, SOCKET[1] + 3.0]), Some(Part::Socket));
        assert_eq!(l.at([10.0, 86.0 + 58.0 + 2.0]), Some(Part::Inventory(0)));
        assert_eq!(l.at([10.0, 88.0]), Some(Part::Inventory(9)));
        assert_eq!(l.at([2.0, 2.0]), None);
        assert_eq!(l.at([190.0, 100.0]), None);
        let star = layout(81);
        assert_eq!(star.at([8.0 + 18.0 * 8.0 + 1.0, 18.0 + 18.0 * 8.0 + 1.0]), Some(Part::Slot(80)));
        let inv = star.inventory_slot(20);
        assert_eq!(star.at([inv[0] + 4.0, inv[1] + 4.0]), Some(Part::Inventory(20)));
    }

    #[test]
    fn the_stack_module_makes_a_slot_hold_up_to_1024() {
        assert!(fits_socket("rustcraft:module_stack"));
        assert!(fits_socket("rustcraft:module_stack_2"));
        assert!(fits_socket("rustcraft:module_stack_3"));
        assert!(!fits_socket("rustcraft:module_speed"));
        assert!(!fits_socket("minecraft:stone"));
        assert_eq!(cap(None, 64), 64);
        assert_eq!(cap(Some(Module::Stack), 64), 256);
        assert_eq!(cap(Some(Module::Stack2), 64), 1024);
        assert_eq!(cap(Some(Module::Stack3), 64), 1024);
        assert_eq!(cap(Some(Module::Stack), 16), 64);
        assert_eq!(cap(Some(Module::Stack3), 1), 1, "a single item stays single");
        assert_eq!(cap(None, 1), 1);
    }

    #[test]
    fn a_deposit_tops_up_its_own_stacks_then_fills_empty_slots() {
        let mut cells: Vec<Cell<&str>> =
            vec![None, Some(("stone", 250)), Some(("dirt", 10)), None, Some(("stone", 100))];
        let left = deposit(&mut cells, &"stone", 500, 256);
        assert_eq!(left, 0);
        assert_eq!(
            cells,
            // 6 and 156 top up the stone slots, 256 fill the first empty slot, 82 the next.
            vec![Some(("stone", 256)), Some(("stone", 256)), Some(("dirt", 10)), Some(("stone", 82)), Some(("stone", 256))]
        );
        // 174 top up the last stone slot; no empty slot is left for the other 126.
        assert_eq!(deposit(&mut cells, &"stone", 300, 256), 126);
        assert_eq!(cells[3], Some(("stone", 256)));
        let mut full: Vec<Cell<&str>> = vec![Some(("dirt", 64)), Some(("stone", 60))];
        assert_eq!(deposit(&mut full, &"stone", 10, 64), 6);
        assert_eq!(full[1], Some(("stone", 64)));
        assert_eq!(deposit(&mut full, &"sand", 3, 64), 3);
        assert_eq!(deposit(&mut full, &"sand", 0, 64), 0);
    }

    #[test]
    fn single_items_take_a_slot_each() {
        let mut cells: Vec<Cell<&str>> = vec![Some(("sword", 1)), None, None];
        assert_eq!(deposit(&mut cells, &"sword", 1, cap(Some(Module::Stack3), 1)), 0);
        assert_eq!(cells, vec![Some(("sword", 1)), Some(("sword", 1)), None]);
    }

    #[test]
    fn a_take_is_a_stack_of_the_item_or_one() {
        assert_eq!(take_count(1000, false, 64), 64);
        assert_eq!(take_count(30, false, 64), 30);
        assert_eq!(take_count(1000, true, 64), 1);
        assert_eq!(take_count(5, false, 1), 1);
        assert_eq!(take_count(0, false, 64), 0);
        let mut cells: Vec<Cell<&str>> = vec![Some(("stone", 1000)), Some(("dirt", 3))];
        remove(&mut cells, 0, 64);
        remove(&mut cells, 1, 3);
        remove(&mut cells, 5, 1);
        assert_eq!(cells, vec![Some(("stone", 936)), None]);
    }

    #[test]
    fn the_module_stays_while_a_slot_holds_more_than_a_plain_stack() {
        assert_eq!(overfull([(64, 64), (1, 1), (16, 16)]), None);
        assert_eq!(overfull([(64, 64), (300, 64), (900, 64)]), Some(1));
        assert_eq!(overfull([(20, 16)]), Some(0));
        assert_eq!(overfull(std::iter::empty()), None);
    }

    #[test]
    fn a_slot_shows_its_count_short() {
        assert_eq!(count_text(1), None);
        assert_eq!(count_text(64), Some("64".to_owned()));
        assert_eq!(count_text(999), Some("999".to_owned()));
        assert_eq!(count_text(1024), Some("1k".to_owned()));
        assert_eq!(count_text(0), Some("0".to_owned()));
    }

    #[test]
    fn every_window_press_comes_back() {
        for p in [
            Press::Take { slot: 80, one: false },
            Press::Take { slot: 0, one: true },
            Press::Deposit(35),
            Press::Socket,
            Press::Closed,
        ] {
            assert_eq!(decode_press(&encode_press(p)), Some(p));
        }
        assert_eq!(decode_press(&[0, 81, 0]), None);
        assert_eq!(decode_press(&[0, 3, 2]), None);
        assert_eq!(decode_press(&[1, 36]), None);
        assert_eq!(decode_press(&[9]), None);
        assert_eq!(decode_press(&[]), None);
    }

    fn cell(item: &str, nbt: usize, count: u32) -> Option<ShownCell> {
        Some(ShownCell { item: item.to_owned(), nbt: vec![7; nbt], count })
    }

    #[test]
    fn the_view_comes_back() {
        let v = View {
            backpack: STAR.to_owned(),
            module: Some("rustcraft:module_stack_2".to_owned()),
            cells: vec![cell("minecraft:stone", 40, 1024), None, cell("minecraft:enchanted_book", 300, 1)],
        };
        assert_eq!(decode_view(&encode_view(&v)), Some(v.clone()));
        let plain = View { module: None, cells: vec![None; 27], ..v };
        assert_eq!(decode_view(&encode_view(&plain)), Some(plain));
        assert_eq!(decode_view(&[]), None);
        assert_eq!(decode_view(&[99, 0]), None);
    }

    #[test]
    fn a_view_too_big_drops_saved_stacks_but_no_slot() {
        let v = View {
            backpack: STAR.to_owned(),
            module: None,
            cells: (0..81).map(|i| cell("minecraft:written_book", 2000, i + 1)).collect(),
        };
        let bytes = encode_view(&v);
        assert!(bytes.len() <= MAX_BYTES, "{}", bytes.len());
        let back = decode_view(&bytes).expect("reads");
        assert_eq!(back.cells.len(), 81);
        assert!(back.cells.iter().flatten().any(|c| c.nbt.is_empty()));
        assert!(back.cells.iter().flatten().any(|c| c.nbt.len() == 2000));
        for (i, c) in back.cells.iter().enumerate() {
            let c = c.as_ref().expect("every slot kept");
            assert_eq!((c.item.as_str(), c.count), ("minecraft:written_book", i as u32 + 1));
        }
    }
}
