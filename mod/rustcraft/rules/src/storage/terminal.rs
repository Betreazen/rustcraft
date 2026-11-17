//! The terminal's window (guide `ui.js`, window `terminal`; S4, S5, S8, TM1, O2–O8, O14): where
//! its parts are, what a press on them sends the server, and what a limit or «держать» button
//! does. The window draws the list the server sent (`wire`), filtered by the tab and the search
//! (`view`); taking, putting in, the limits, the recipe book's socket and the orders are menu
//! buttons, answered by the server.
//!
//! A press on a stored item names the item: its number in the item table both sides share and its
//! damage, packed into the button id with what the press asks for, so a list that changed in the
//! meantime never makes the server hand out another item.
//!
//! The window has three modes: taking items (the default), «Лимиты» (S5) and «Заказы» (O3, O8,
//! O14). In the orders mode a click on an item chooses it for the row under the grid (order ×1,
//! ×8, ×64; «держать N»), and the rows under the status list the queue (wave 2, `ui.js`: «каждый
//! заказ — цепочка шагов»), QUEUE_ROWS orders at a time, each with its chain and its cancel button;
//! a click on a row shows the next ones.

use crate::machines::panel::{Rect, inside, slot_frame};
use crate::net::items::ItemKey;
use crate::storage::view::{PER_PAGE, Tab};
use crate::storage::{Limit, Store};

pub const WIDTH: f32 = 214.0;
pub const TITLE: [f32; 2] = [8.0, 6.0];
/// The quick filter's tabs, in `Tab::ALL` order (slice 18: «своя» last).
pub const TABS: [Rect; 5] = [
    [8.0, 16.0, 28.0, 12.0],
    [38.0, 16.0, 36.0, 12.0],
    [76.0, 16.0, 62.0, 12.0],
    [140.0, 16.0, 36.0, 12.0],
    [178.0, 16.0, 28.0, 12.0],
];
pub const SEARCH: Rect = [8.0, 30.0, 64.0, 12.0];
pub const ORDER_MODE: Rect = [74.0, 30.0, 36.0, 12.0];
pub const PREV: Rect = [112.0, 30.0, 12.0, 12.0];
pub const PAGE_TEXT: Rect = [124.0, 30.0, 30.0, 12.0];
pub const NEXT: Rect = [154.0, 30.0, 12.0, 12.0];
pub const LIMIT_MODE: Rect = [170.0, 30.0, 36.0, 12.0];
pub const COLUMNS: usize = 9;
/// Where the first stored item's icon goes; the others follow in rows of COLUMNS.
pub const GRID: [f32; 2] = [26.0, 45.0];
/// Slice 18: the three sockets left of the grid (the icons' corners), in
/// `storage::modules::SOCKET_*` order: the stack module, the recipe book (O1), the filter.
pub const SOCKETS: [[f32; 2]; 3] = [[6.0, 45.0], [6.0, 63.0], [6.0, 81.0]];
/// The recipe book's socket.
pub const BOOK: [f32; 2] = SOCKETS[1];
pub const LIMIT_ICON: [f32; 2] = [8.0, 100.0];
pub const LIMIT_TEXT: [f32; 2] = [24.0, 102.0];
pub const LIMIT_MINUS: Rect = [96.0, 100.0, 13.0, 12.0];
pub const LIMIT_PLUS: Rect = [110.0, 100.0, 13.0, 12.0];
pub const LIMIT_CLEAR: Rect = [124.0, 100.0, 13.0, 12.0];
pub const LIMIT_DESTROY: Rect = [140.0, 100.0, 66.0, 12.0];
/// The orders mode's row under the grid (slice 18: any count): «−», the count, «+», «Заказать»,
/// then «держать N» and its buttons. ORDER_BUTTONS in ORDER_MINUS, ORDER_PLUS, ORDER_GO order.
pub const ORDER_BUTTONS: [Rect; 3] = [
    [24.0, 100.0, 10.0, 12.0],
    [58.0, 100.0, 10.0, 12.0],
    [70.0, 100.0, 32.0, 12.0],
];
pub const ORDER_COUNT: Rect = [35.0, 100.0, 22.0, 12.0];
pub const ORDER_MINUS: usize = 0;
pub const ORDER_PLUS: usize = 1;
pub const ORDER_GO: usize = 2;
pub const KEEP_TEXT: [f32; 2] = [104.0, 102.0];
/// Slice 18: the «Лимиты» mode lists every item as a row (those with a limit first), this many a
/// page, in the grid's place.
pub const LIMIT_ROWS: usize = 5;
pub const LIMIT_ROW_H: f32 = 11.0;
pub const KEEP_MINUS: Rect = [166.0, 100.0, 13.0, 12.0];
pub const KEEP_PLUS: Rect = [180.0, 100.0, 13.0, 12.0];
pub const KEEP_CLEAR: Rect = [194.0, 100.0, 12.0, 12.0];
pub const STATUS: Rect = [8.0, 114.0, 198.0, 12.0];
pub const DETAIL: [f32; 2] = [8.0, 129.0];
/// The orders mode: the rows of the queue (wave 2: a list, QUEUE_ROWS orders at a time), each
/// with its cancel button.
pub const QUEUE_ROWS: usize = 2;
pub const QUEUE_ROW_H: f32 = 10.0;
pub const QUEUE: Rect = [8.0, 127.0, 184.0, QUEUE_ROW_H];
pub const CANCEL: Rect = [194.0, 127.0, 12.0, QUEUE_ROW_H];
pub const INVENTORY: [f32; 2] = [26.0, 158.0];
pub const INVENTORY_LABEL_Y: f32 = INVENTORY[1] - 10.0;
pub const HEIGHT: f32 = INVENTORY[1] + 80.0;
/// The most one order press orders (the press's count field).
pub const MAX_ORDER: u16 = MAX_DAMAGE as u16;

/// Wave 2: the queue's row `k` and its cancel button.
pub fn queue_row(k: usize) -> (Rect, Rect) {
    let dy = QUEUE_ROW_H * k as f32;
    ([QUEUE[0], QUEUE[1] + dy, QUEUE[2], QUEUE[3]], [CANCEL[0], CANCEL[1] + dy, CANCEL[2], CANCEL[3]])
}

/// Wave 2: the orders the queue's rows show, from the page `first` is on: their places in the
/// queue of `len`, QUEUE_ROWS at most.
pub fn queue_page(len: usize, first: usize) -> Vec<usize> {
    if len == 0 {
        return Vec::new();
    }
    let start = (first % len) / QUEUE_ROWS * QUEUE_ROWS;
    (start..len.min(start + QUEUE_ROWS)).collect()
}

/// Slice 18: the order count after «−» or «+» by `delta`, never below 1 nor past MAX_ORDER.
pub fn step_count(count: u16, delta: i32) -> u16 {
    (i32::from(count) + delta).clamp(1, i32::from(MAX_ORDER)) as u16
}

/// The «Лимиты» mode's row `i` (its whole width).
pub fn limit_row(i: usize) -> Rect {
    [GRID[0], GRID[1] + LIMIT_ROW_H * i as f32, 18.0 * COLUMNS as f32, LIMIT_ROW_H - 1.0]
}

/// The row at `at` in the «Лимиты» mode.
pub fn limit_row_at(at: [f32; 2]) -> Option<usize> {
    (0..LIMIT_ROWS).find(|&i| inside(limit_row(i), at))
}

/// Where the item of the page's `i`th place goes.
pub fn cell(i: usize) -> [f32; 2] {
    [
        GRID[0] + 18.0 * (i % COLUMNS) as f32,
        GRID[1] + 18.0 * (i / COLUMNS) as f32,
    ]
}

/// A part of the window a press or the pointer can be on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Tab(Tab),
    Search,
    OrderMode,
    Prev,
    Next,
    LimitMode,
    /// A place of the page, from 0.
    Cell(usize),
    /// Slice 18: a socket, by its place in SOCKETS.
    Socket(usize),
    LimitMinus,
    LimitPlus,
    LimitClear,
    LimitDestroy,
    /// The orders mode: «−», «+» or «Заказать» (ORDER_MINUS, ORDER_PLUS, ORDER_GO).
    Order(usize),
    KeepMinus,
    KeepPlus,
    KeepClear,
    Status,
    /// Wave 2: a row of the queue, and its cancel button, by the row.
    Queue(usize),
    Cancel(usize),
}

/// The part at `at`, in window pixels, in the taking or the limits mode.
pub fn part_at(at: [f32; 2]) -> Option<Part> {
    part_in(at, false)
}

/// The part at `at`; `orders` for the orders mode, whose row under the grid and line under the
/// status have parts of their own.
pub fn part_in(at: [f32; 2], orders: bool) -> Option<Part> {
    if let Some(i) = TABS.iter().position(|&r| inside(r, at)) {
        return Some(Part::Tab(Tab::ALL[i]));
    }
    if let Some(i) = (0..PER_PAGE).find(|&i| inside(slot_frame(cell(i)), at)) {
        return Some(Part::Cell(i));
    }
    if let Some(k) = SOCKETS.iter().position(|&p| inside(slot_frame(p), at)) {
        return Some(Part::Socket(k));
    }
    let shared = [
        (SEARCH, Part::Search),
        (ORDER_MODE, Part::OrderMode),
        (PREV, Part::Prev),
        (NEXT, Part::Next),
        (LIMIT_MODE, Part::LimitMode),
        (STATUS, Part::Status),
    ];
    let limits = [
        (LIMIT_MINUS, Part::LimitMinus),
        (LIMIT_PLUS, Part::LimitPlus),
        (LIMIT_CLEAR, Part::LimitClear),
        (LIMIT_DESTROY, Part::LimitDestroy),
    ];
    let order_parts = [
        (ORDER_BUTTONS[0], Part::Order(0)),
        (ORDER_BUTTONS[1], Part::Order(1)),
        (ORDER_BUTTONS[2], Part::Order(2)),
        (KEEP_MINUS, Part::KeepMinus),
        (KEEP_PLUS, Part::KeepPlus),
        (KEEP_CLEAR, Part::KeepClear),
    ];
    if orders
        && let Some(k) = (0..QUEUE_ROWS).find(|&k| inside(queue_row(k).0, at) || inside(queue_row(k).1, at))
    {
        return Some(if inside(queue_row(k).1, at) { Part::Cancel(k) } else { Part::Queue(k) });
    }
    let own: &[(Rect, Part)] = if orders { &order_parts } else { &limits };
    shared
        .iter()
        .chain(own)
        .find_map(|&(r, p)| inside(r, at).then_some(p))
}

// ---- Presses the server answers ----

/// An item as a press names it: its number in the item table and its damage; or, with VARIANT in
/// the second, the place of a component stack among its item's (`Store::variants_of`).
pub type ItemRef = (u16, u16);

/// Slice 18: the mark of a component stack's place in an ItemRef's second number (and a wire
/// line's damage). A press with it has the top bit of its id set.
pub const VARIANT: u16 = 0x8000;
const VARIANT_BIT: u32 = 1 << 31;

/// The largest damage a press can name. A stack worn further has no place in the storage.
pub const MAX_DAMAGE: u32 = (1 << DAMAGE_BITS) - 1;
const DAMAGE_BITS: u32 = 11;
const ITEM_BITS: u32 = 16;
const ACTION_SHIFT: u32 = ITEM_BITS + DAMAGE_BITS;
/// The item field of a press on an empty place.
const NO_ITEM: u32 = 0xFFFF;
/// The action codes after ACTIONS': an order, a «держать» button, a cancel, the book's socket.
const ORDER: u32 = ACTIONS.len() as u32 + 1;
const KEEP: u32 = ORDER + 1;
const CANCEL_ORDER: u32 = KEEP + 1;
const BOOK_SOCKET: u32 = CANCEL_ORDER + 1;
/// The «держать» buttons, by their code in a press.
const KEEP_CHANGES: [LimitChange; 5] = [
    LimitChange::Add(64),
    LimitChange::Add(-64),
    LimitChange::Add(1),
    LimitChange::Add(-1),
    LimitChange::Clear,
];

/// What a press on a place of the page takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Take {
    /// A stack onto the cursor (left click).
    Stack,
    /// One item onto the cursor (right click).
    One,
    /// A stack into the inventory (shift and click).
    ToInventory,
}

/// What a limit button does to the chosen item's limit (S5), or a «держать» button to its N.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitChange {
    Add(i64),
    Clear,
    /// «Уничтожать излишек» on or off.
    ToggleDestroy,
}

/// A press the server answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// On a place of the page: with an item on the cursor it puts the cursor's stack in (one
    /// item for `One`), else it takes the place's item.
    Place {
        item: Option<ItemRef>,
        take: Take,
    },
    Limit {
        item: ItemRef,
        change: LimitChange,
    },
    /// O3: order `count` of the item (its number in the item table).
    Order {
        item: u16,
        count: u16,
    },
    /// O14: «держать N» of the item changed.
    Keep {
        item: u16,
        change: LimitChange,
    },
    /// O7: the order with this id cancelled.
    Cancel {
        order: u16,
    },
    /// O1, slice 18: socket `k` (`storage::modules::SOCKET_*`): the module on the cursor goes
    /// in, or the module comes out.
    Socket(u8),
}

/// A press that does nothing on the server: a tab, the search, a page.
pub const NO_PRESS: i32 = 0;

const ACTIONS: [Press; 9] = [
    Press::Place {
        item: None,
        take: Take::Stack,
    },
    Press::Place {
        item: None,
        take: Take::One,
    },
    Press::Place {
        item: None,
        take: Take::ToInventory,
    },
    Press::Limit {
        item: (0, 0),
        change: LimitChange::Add(64),
    },
    Press::Limit {
        item: (0, 0),
        change: LimitChange::Add(-64),
    },
    Press::Limit {
        item: (0, 0),
        change: LimitChange::Add(1),
    },
    Press::Limit {
        item: (0, 0),
        change: LimitChange::Add(-1),
    },
    Press::Limit {
        item: (0, 0),
        change: LimitChange::Clear,
    },
    Press::Limit {
        item: (0, 0),
        change: LimitChange::ToggleDestroy,
    },
];

/// Whether an item with this damage can be named by a press.
pub fn addressable(damage: u32) -> bool {
    damage <= MAX_DAMAGE
}

fn pack(action: u32, damage: u32, number: u32) -> Option<i32> {
    i32::try_from(action << ACTION_SHIFT | damage << ITEM_BITS | number).ok()
}

/// The menu button id of `press`; none for an action it has no code for, a damage past
/// MAX_DAMAGE or an order count past it.
pub fn encode(press: Press) -> Option<i32> {
    match press {
        Press::Order { item, count } => {
            let count = u32::from(count);
            (count > 0 && count <= MAX_DAMAGE && u32::from(item) != NO_ITEM)
                .then(|| pack(ORDER, count, u32::from(item)))
                .flatten()
        }
        Press::Keep { item, change } => {
            let code = KEEP_CHANGES.iter().position(|&c| c == change)? as u32;
            (u32::from(item) != NO_ITEM)
                .then(|| pack(KEEP, code, u32::from(item)))
                .flatten()
        }
        Press::Cancel { order } => pack(CANCEL_ORDER, 0, u32::from(order)),
        Press::Socket(k) => (usize::from(k) < SOCKETS.len())
            .then(|| pack(BOOK_SOCKET, 0, u32::from(k)))
            .flatten(),
        Press::Place { .. } | Press::Limit { .. } => encode_item_press(press),
    }
}

/// A press of ACTIONS: a place's or a limit's.
fn encode_item_press(press: Press) -> Option<i32> {
    let bare = match press {
        Press::Place { take, .. } => Press::Place { item: None, take },
        Press::Limit { change, .. } => Press::Limit {
            item: (0, 0),
            change,
        },
        _ => return None,
    };
    let action = ACTIONS.iter().position(|&a| a == bare)? as u32 + 1;
    let item = match press {
        Press::Place { item, .. } => item,
        Press::Limit { item, .. } => Some(item),
        _ => None,
    };
    let (number, damage, variant) = match item {
        Some((n, d))
            if addressable(u32::from(d & !VARIANT)) && u32::from(n) != NO_ITEM =>
        {
            (u32::from(n), u32::from(d & !VARIANT), d & VARIANT != 0)
        }
        Some(_) => return None,
        None => (NO_ITEM, 0, false),
    };
    let id = pack(action, damage, number)?;
    // A component stack's press: the top bit, so the id is below zero.
    Some(if variant {
        (id as u32 | VARIANT_BIT) as i32
    } else {
        id
    })
}

/// The press a menu button id names; none for NO_PRESS or an id it does not know.
pub fn decode(id: i32) -> Option<Press> {
    let raw = id as u32;
    let variant = raw & VARIANT_BIT != 0;
    let id = raw & !VARIANT_BIT;
    if variant {
        return match decode_plain(id)? {
            Press::Place {
                item: Some((n, d)),
                take,
            } => Some(Press::Place {
                item: Some((n, d | VARIANT)),
                take,
            }),
            Press::Limit { item: (n, d), change } => Some(Press::Limit {
                item: (n, d | VARIANT),
                change,
            }),
            _ => None,
        };
    }
    decode_plain(id)
}

fn decode_plain(id: u32) -> Option<Press> {
    let action = id >> ACTION_SHIFT;
    let number = id & ((1 << ITEM_BITS) - 1);
    let damage = (id >> ITEM_BITS) & MAX_DAMAGE;
    match action {
        ORDER => (damage > 0 && number != NO_ITEM).then_some(Press::Order {
            item: number as u16,
            count: damage as u16,
        }),
        KEEP => Some(Press::Keep {
            item: (number != NO_ITEM).then_some(number as u16)?,
            change: *KEEP_CHANGES.get(damage as usize)?,
        }),
        CANCEL_ORDER => Some(Press::Cancel {
            order: number as u16,
        }),
        BOOK_SOCKET => u8::try_from(number)
            .ok()
            .filter(|k| usize::from(*k) < SOCKETS.len())
            .map(Press::Socket),
        _ => {
            let bare = *ACTIONS.get((action as usize).checked_sub(1)?)?;
            let item = (number != NO_ITEM).then_some((number as u16, damage as u16));
            match bare {
                Press::Place { take, .. } => Some(Press::Place { item, take }),
                Press::Limit { change, .. } => Some(Press::Limit {
                    item: item?,
                    change,
                }),
                _ => None,
            }
        }
    }
}

/// Applies a limit button to `key`'s limit (S5). Raising a missing limit starts it from zero;
/// switching «уничтожать излишек» on without a limit keeps what is stored now.
pub fn change_limit(store: &mut Store, key: &ItemKey, change: LimitChange) {
    let now = store.limits.get(key).copied();
    let next = match change {
        LimitChange::Clear => None,
        LimitChange::Add(delta) => {
            let base = now.map_or(0, |l| l.max);
            let max = add(base, delta);
            match now {
                None if delta <= 0 => None,
                _ => Some(Limit {
                    max,
                    destroy_surplus: now.is_some_and(|l| l.destroy_surplus),
                }),
            }
        }
        LimitChange::ToggleDestroy => Some(match now {
            Some(l) => Limit {
                destroy_surplus: !l.destroy_surplus,
                ..l
            },
            None => Limit {
                max: store.count(key),
                destroy_surplus: true,
            },
        }),
    };
    match next {
        Some(l) => store.limits.insert(key.clone(), l),
        None => store.limits.remove(key),
    };
}

fn add(base: u64, delta: i64) -> u64 {
    if delta >= 0 {
        base.saturating_add(delta.unsigned_abs())
    } else {
        base.saturating_sub(delta.unsigned_abs())
    }
}

/// Applies a «держать» button to `key`'s N (O14): zero clears it.
pub fn change_keep(store: &mut Store, key: &ItemKey, change: LimitChange) {
    let now = store.keep.get(key).copied().unwrap_or(0);
    let next = match change {
        LimitChange::Add(delta) => add(now, delta),
        LimitChange::Clear | LimitChange::ToggleDestroy => 0,
    };
    if next == 0 {
        store.keep.remove(key);
    } else {
        store.keep.insert(key.clone(), next);
    }
}

/// O1: a press on the book's socket. With `book` (the item of the recipe book) on the cursor and
/// the socket empty, one of the cursor goes in; with an empty cursor and the book in, the book
/// comes out onto the cursor. The socket after it and the cursor after it; none when nothing
/// changes.
pub fn press_book(
    held: bool,
    cursor: Option<(&str, u32)>,
    book: &str,
) -> Option<(bool, Option<(String, u32)>)> {
    match (held, cursor) {
        (false, Some((item, n))) if item == book && n > 0 => {
            Some((true, (n > 1).then(|| (item.to_owned(), n - 1))))
        }
        (true, None) => Some((false, Some((book.to_owned(), 1)))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_count_can_be_ordered() {
        assert_eq!(step_count(1, -1), 1);
        assert_eq!(step_count(1, 8), 9);
        assert_eq!(step_count(2000, 64), MAX_ORDER);
        let p = Press::Order {
            item: 5,
            count: MAX_ORDER,
        };
        assert_eq!(decode(encode(p).unwrap()), Some(p));
        assert_eq!(limit_row_at([30.0, 46.0]), Some(0));
        assert_eq!(limit_row_at([30.0, 45.0 + 11.0 * 4.0 + 2.0]), Some(4));
        assert_eq!(limit_row_at([30.0, 45.0 + 11.0 * 5.0 + 2.0]), None);
    }

    #[test]
    fn a_component_stacks_press_names_its_place() {
        // Slice 18: a press on the third enchanted book of the list.
        for p in [
            Press::Place {
                item: Some((812, VARIANT | 2)),
                take: Take::One,
            },
            Press::Limit {
                item: (812, VARIANT | 2047),
                change: LimitChange::Clear,
            },
        ] {
            let id = encode(p).unwrap();
            assert!(id < 0, "the top bit marks it");
            assert_eq!(decode(id), Some(p));
        }
        let plain = Press::Place {
            item: Some((812, 2)),
            take: Take::One,
        };
        assert_eq!(decode(encode(plain).unwrap()), Some(plain));
    }

    #[test]
    fn every_press_comes_back_from_its_id() {
        let presses = [
            Press::Place {
                item: Some((812, 0)),
                take: Take::Stack,
            },
            Press::Place {
                item: None,
                take: Take::One,
            },
            Press::Place {
                item: Some((65_534, MAX_DAMAGE as u16)),
                take: Take::ToInventory,
            },
            Press::Limit {
                item: (3, 7),
                change: LimitChange::Add(-64),
            },
            Press::Limit {
                item: (3, 0),
                change: LimitChange::ToggleDestroy,
            },
            Press::Order {
                item: 812,
                count: 64,
            },
            Press::Keep {
                item: 65_534,
                change: LimitChange::Add(-1),
            },
            Press::Keep {
                item: 4,
                change: LimitChange::Clear,
            },
            Press::Cancel { order: 0xFFFF },
            Press::Cancel { order: 1 },
            Press::Socket(0),
            Press::Socket(1),
            Press::Socket(2),
        ];
        for p in presses {
            let id = encode(p).unwrap();
            assert!(id > NO_PRESS);
            assert_eq!(decode(id), Some(p));
        }
        assert_eq!(decode(NO_PRESS), None);
        assert_eq!(decode(-5), None);
        assert_eq!(decode(15 << ACTION_SHIFT), None);
    }

    #[test]
    fn a_damage_past_the_field_is_no_press() {
        let p = Press::Place {
            item: Some((1, (MAX_DAMAGE + 1) as u16)),
            take: Take::Stack,
        };
        assert_eq!(encode(p), None);
        assert_eq!(encode(Press::Order { item: 1, count: 0 }), None);
        let destroy = Press::Keep {
            item: 1,
            change: LimitChange::ToggleDestroy,
        };
        assert_eq!(encode(destroy), None);
    }

    #[test]
    fn limit_buttons_raise_lower_clear_and_switch_destroying() {
        // Rule S5.
        let mut s = Store {
            cells: 1,
            ..Store::default()
        };
        let key: ItemKey = "minecraft:wheat_seeds".into();
        s.items.insert(key.clone(), 40);
        change_limit(&mut s, &key, LimitChange::Add(-64));
        assert_eq!(s.limits.get(&key), None);
        change_limit(&mut s, &key, LimitChange::Add(64));
        change_limit(&mut s, &key, LimitChange::Add(1));
        assert_eq!(s.limits[&key].max, 65);
        change_limit(&mut s, &key, LimitChange::ToggleDestroy);
        assert!(s.limits[&key].destroy_surplus);
        change_limit(&mut s, &key, LimitChange::Add(-100));
        assert_eq!(s.limits[&key].max, 0);
        change_limit(&mut s, &key, LimitChange::Clear);
        assert!(s.limits.is_empty());
        change_limit(&mut s, &key, LimitChange::ToggleDestroy);
        assert_eq!(
            s.limits[&key],
            Limit {
                max: 40,
                destroy_surplus: true
            }
        );
    }

    #[test]
    fn keep_buttons_raise_lower_and_clear_n() {
        // Rule O14.
        let mut s = Store::default();
        let key: ItemKey = "minecraft:book".into();
        change_keep(&mut s, &key, LimitChange::Add(-64));
        assert!(s.keep.is_empty());
        change_keep(&mut s, &key, LimitChange::Add(64));
        change_keep(&mut s, &key, LimitChange::Add(1));
        assert_eq!(s.keep[&key], 65);
        change_keep(&mut s, &key, LimitChange::Add(-100));
        assert!(s.keep.is_empty());
        change_keep(&mut s, &key, LimitChange::Add(2));
        change_keep(&mut s, &key, LimitChange::Clear);
        assert!(s.keep.is_empty());
    }

    #[test]
    fn the_book_goes_into_its_socket_and_back() {
        // Rule O1.
        const B: &str = "rustcraft:module_recipes";
        assert_eq!(press_book(false, Some((B, 1)), B), Some((true, None)));
        assert_eq!(
            press_book(false, Some((B, 3)), B),
            Some((true, Some((B.into(), 2))))
        );
        assert_eq!(
            press_book(true, None, B),
            Some((false, Some((B.into(), 1))))
        );
        assert_eq!(press_book(true, Some((B, 1)), B), None);
        assert_eq!(press_book(false, Some(("minecraft:dirt", 1)), B), None);
        assert_eq!(press_book(false, None, B), None);
    }

    #[test]
    fn the_parts_are_where_the_layout_puts_them() {
        assert_eq!(part_at([10.0, 20.0]), Some(Part::Tab(Tab::All)));
        assert_eq!(part_at([100.0, 20.0]), Some(Part::Tab(Tab::Tools)));
        assert_eq!(part_at([27.0, 46.0]), Some(Part::Cell(0)));
        assert_eq!(
            part_at([26.0 + 18.0 * 8.0 + 2.0, 45.0 + 36.0 + 2.0]),
            Some(Part::Cell(26))
        );
        assert_eq!(part_at([150.0, 104.0]), Some(Part::LimitDestroy));
        assert_eq!(part_at([200.0, 200.0]), None);
        assert_eq!(part_at([8.0, 50.0]), Some(Part::Socket(0)));
        assert_eq!(part_at([8.0, 68.0]), Some(Part::Socket(1)));
        assert_eq!(part_at([8.0, 86.0]), Some(Part::Socket(2)));
        assert_eq!(part_at([190.0, 20.0]), Some(Part::Tab(Tab::Own)));
        assert_eq!(encode(Press::Socket(3)), None);
        assert_eq!(part_at([80.0, 34.0]), Some(Part::OrderMode));
        // The orders mode's row under the grid and line under the status.
        assert_eq!(part_in([30.0, 104.0], true), Some(Part::Order(0)));
        assert_eq!(part_in([70.0, 104.0], true), Some(Part::Order(2)));
        assert_eq!(part_in([200.0, 104.0], true), Some(Part::KeepClear));
        assert_eq!(part_in([100.0, 131.0], true), Some(Part::Queue(0)));
        assert_eq!(part_in([198.0, 131.0], true), Some(Part::Cancel(0)));
        assert_eq!(part_in([100.0, 141.0], true), Some(Part::Queue(1)));
        assert_eq!(part_in([198.0, 141.0], true), Some(Part::Cancel(1)));
        assert_eq!(part_in([150.0, 104.0], true), None);
        assert_eq!(part_in([100.0, 131.0], false), None);
        // The book's socket and the grid do not touch.
        let book = slot_frame(BOOK);
        assert!(book[0] + book[2] <= slot_frame(cell(0))[0]);
        // The window fits a 1280×720 screen at GUI scale 3.
        const { assert!(HEIGHT <= 240.0) };
    }

    #[test]
    fn the_queue_is_a_list_a_page_at_a_time() {
        // Wave 2: two orders a page; a click goes on to the next page and round.
        assert_eq!(queue_page(5, 0), vec![0, 1]);
        assert_eq!(queue_page(5, 3), vec![2, 3]);
        assert_eq!(queue_page(5, 4), vec![4]);
        assert_eq!(queue_page(5, 6), vec![0, 1]);
        assert!(queue_page(0, 3).is_empty());
        // The last row ends above the inventory's label.
        assert!(queue_row(QUEUE_ROWS - 1).0[1] + QUEUE_ROW_H <= INVENTORY_LABEL_Y);
    }
}
