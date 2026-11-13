//! The terminal's window (guide `ui.js`, window `terminal`; S4, S5, S8, TM1): where its parts
//! are, what a press on them sends the server, and what a limit button does. The window draws
//! the list the server sent (`wire`), filtered by the tab and the search (`view`); taking,
//! putting in and the limits are menu buttons, answered by the server.
//!
//! A press on a stored item names the item: its number in the item table both sides share and its
//! damage, packed into the button id with what the press asks for, so a list that changed in the
//! meantime never makes the server hand out another item.

use crate::machines::panel::{Rect, inside, slot_frame};
use crate::net::items::ItemKey;
use crate::storage::view::{PER_PAGE, Tab};
use crate::storage::{Limit, Store};

pub const WIDTH: f32 = 214.0;
pub const TITLE: [f32; 2] = [8.0, 6.0];
/// The quick filter's tabs, in `Tab::ALL` order.
pub const TABS: [Rect; 4] = [
    [8.0, 16.0, 30.0, 12.0],
    [40.0, 16.0, 44.0, 12.0],
    [86.0, 16.0, 74.0, 12.0],
    [162.0, 16.0, 44.0, 12.0],
];
pub const SEARCH: Rect = [8.0, 30.0, 100.0, 12.0];
pub const PREV: Rect = [112.0, 30.0, 12.0, 12.0];
pub const PAGE_TEXT: Rect = [124.0, 30.0, 30.0, 12.0];
pub const NEXT: Rect = [154.0, 30.0, 12.0, 12.0];
pub const LIMIT_MODE: Rect = [170.0, 30.0, 36.0, 12.0];
pub const COLUMNS: usize = 9;
/// Where the first stored item's icon goes; the others follow in rows of COLUMNS.
pub const GRID: [f32; 2] = [26.0, 45.0];
pub const LIMIT_ICON: [f32; 2] = [8.0, 100.0];
pub const LIMIT_TEXT: [f32; 2] = [24.0, 102.0];
pub const LIMIT_MINUS: Rect = [96.0, 100.0, 13.0, 12.0];
pub const LIMIT_PLUS: Rect = [110.0, 100.0, 13.0, 12.0];
pub const LIMIT_CLEAR: Rect = [124.0, 100.0, 13.0, 12.0];
pub const LIMIT_DESTROY: Rect = [140.0, 100.0, 66.0, 12.0];
pub const STATUS: Rect = [8.0, 114.0, 198.0, 12.0];
pub const DETAIL: [f32; 2] = [8.0, 129.0];
pub const INVENTORY: [f32; 2] = [26.0, 152.0];
pub const INVENTORY_LABEL_Y: f32 = INVENTORY[1] - 10.0;
pub const HEIGHT: f32 = INVENTORY[1] + 80.0;

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
    Prev,
    Next,
    LimitMode,
    /// A place of the page, from 0.
    Cell(usize),
    LimitMinus,
    LimitPlus,
    LimitClear,
    LimitDestroy,
    Status,
}

/// The part at `at`, in window pixels.
pub fn part_at(at: [f32; 2]) -> Option<Part> {
    if let Some(i) = TABS.iter().position(|&r| inside(r, at)) {
        return Some(Part::Tab(Tab::ALL[i]));
    }
    if let Some(i) = (0..PER_PAGE).find(|&i| inside(slot_frame(cell(i)), at)) {
        return Some(Part::Cell(i));
    }
    [
        (SEARCH, Part::Search),
        (PREV, Part::Prev),
        (NEXT, Part::Next),
        (LIMIT_MODE, Part::LimitMode),
        (LIMIT_MINUS, Part::LimitMinus),
        (LIMIT_PLUS, Part::LimitPlus),
        (LIMIT_CLEAR, Part::LimitClear),
        (LIMIT_DESTROY, Part::LimitDestroy),
        (STATUS, Part::Status),
    ]
    .into_iter()
    .find_map(|(r, p)| inside(r, at).then_some(p))
}

// ---- Presses the server answers ----

/// An item as a press names it: its number in the item table and its damage.
pub type ItemRef = (u16, u16);

/// The largest damage a press can name. A stack worn further has no place in the storage.
pub const MAX_DAMAGE: u32 = (1 << DAMAGE_BITS) - 1;
const DAMAGE_BITS: u32 = 11;
const ITEM_BITS: u32 = 16;
const ACTION_SHIFT: u32 = ITEM_BITS + DAMAGE_BITS;
/// The item field of a press on an empty place.
const NO_ITEM: u32 = 0xFFFF;

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

/// What a limit button does to the chosen item's limit (S5).
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

/// The press with its item taken out, to look its action up in ACTIONS.
fn bare(p: Press) -> Press {
    match p {
        Press::Place { take, .. } => Press::Place { item: None, take },
        Press::Limit { change, .. } => Press::Limit {
            item: (0, 0),
            change,
        },
    }
}

/// Whether an item with this damage can be named by a press.
pub fn addressable(damage: u32) -> bool {
    damage <= MAX_DAMAGE
}

/// The menu button id of `press`; none for an action it has no code for or a damage past
/// MAX_DAMAGE.
pub fn encode(press: Press) -> Option<i32> {
    let action = ACTIONS.iter().position(|&a| a == bare(press))? as u32 + 1;
    let item = match press {
        Press::Place { item, .. } => item,
        Press::Limit { item, .. } => Some(item),
    };
    let (number, damage) = match item {
        Some((n, d)) if addressable(u32::from(d)) && u32::from(n) != NO_ITEM => {
            (u32::from(n), u32::from(d))
        }
        Some(_) => return None,
        None => (NO_ITEM, 0),
    };
    i32::try_from(action << ACTION_SHIFT | damage << ITEM_BITS | number).ok()
}

/// The press a menu button id names; none for NO_PRESS or an id it does not know.
pub fn decode(id: i32) -> Option<Press> {
    let id = u32::try_from(id).ok()?;
    let action = (id >> ACTION_SHIFT) as usize;
    let bare = *ACTIONS.get(action.checked_sub(1)?)?;
    let number = id & ((1 << ITEM_BITS) - 1);
    let damage = (id >> ITEM_BITS) & MAX_DAMAGE;
    let item = (number != NO_ITEM).then_some((number as u16, damage as u16));
    match bare {
        Press::Place { take, .. } => Some(Press::Place { item, take }),
        Press::Limit { change, .. } => Some(Press::Limit {
            item: item?,
            change,
        }),
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
            let max = if delta >= 0 {
                base.saturating_add(delta.unsigned_abs())
            } else {
                base.saturating_sub(delta.unsigned_abs())
            };
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

#[cfg(test)]
mod tests {
    use super::*;

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
        ];
        for p in presses {
            let id = encode(p).unwrap();
            assert!(id > NO_PRESS);
            assert_eq!(decode(id), Some(p));
        }
        assert_eq!(decode(NO_PRESS), None);
        assert_eq!(decode(-5), None);
    }

    #[test]
    fn a_damage_past_the_field_is_no_press() {
        let p = Press::Place {
            item: Some((1, (MAX_DAMAGE + 1) as u16)),
            take: Take::Stack,
        };
        assert_eq!(encode(p), None);
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
        // The window fits a 1280×720 screen at GUI scale 3.
        const { assert!(HEIGHT <= 240.0) };
    }
}
