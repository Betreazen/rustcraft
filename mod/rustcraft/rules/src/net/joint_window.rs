//! The joint window (M7, the guide's `wrench` window): one page per face of a housing that is a
//! joint, a row per slot of the housing with its channel, direction, filter and priority. The
//! game shows it as a chest of six rows (a server menu, which every client can show): the top row
//! picks the face, rows 1–4 are the four slots, the bottom row holds the hint and copy/paste.
//! No slot of it holds an item the player can take: a filter cell shows an item, and an item is
//! added by clicking an empty cell, then the item in the player's own inventory below.
//!
//! This module only says what each cell shows and what a click on it does; the bundle turns
//! cells into item stacks with names and applies the actions to the housing's settings.

use crate::geom::Face;
use crate::net::items::{ItemKey, Mode};
use crate::net::settings::{Edit, FILTER_LIMIT, JointSetting};
use crate::net::topology::{ChannelKind, SLOTS};

pub const COLUMNS: usize = 9;
pub const ROWS: usize = 6;
pub const CELLS: usize = COLUMNS * ROWS;
/// Columns of a slot's row (the channel is column 0).
const DIRECTION: usize = 1;
const LIST: usize = 2;
const FILTER: usize = 3;
const PRIORITY_DOWN: usize = FILTER + FILTER_LIMIT;
const PRIORITY: usize = PRIORITY_DOWN + 1;
const PRIORITY_UP: usize = PRIORITY + 1;
/// Cells of the bottom row.
const HELP: usize = (ROWS - 1) * COLUMNS;
const COPY: usize = CELLS - 2;
const PASTE: usize = CELLS - 1;

/// Why a row's cell does nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unused {
    /// The slot has no channel.
    Empty,
    /// A filter or a priority on an energy or fluid channel: only items have them.
    NotItems,
    /// The block behind this face has nothing this channel carries: no inventory for an item
    /// channel, no energy for an energy channel, and no fluid block yet for a fluid channel.
    NotHere,
}

/// What one cell of the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    /// A face of the housing in the top row: a joint or not, and the page shown now.
    Tab {
        face: Face,
        joint: bool,
        selected: bool,
    },
    Channel {
        slot: usize,
        kind: Option<ChannelKind>,
    },
    Direction {
        slot: usize,
        mode: Mode,
    },
    List {
        slot: usize,
        allow_only: bool,
    },
    /// A filter cell: its item, or empty; `armed` when the next item clicked goes here.
    Filter {
        slot: usize,
        item: Option<ItemKey>,
        armed: bool,
    },
    PriorityDown {
        slot: usize,
        priority: i32,
    },
    Priority {
        slot: usize,
        priority: i32,
    },
    PriorityUp {
        slot: usize,
        priority: i32,
    },
    Unused {
        slot: usize,
        why: Unused,
    },
    Help,
    Copy,
    /// «Вставить», ready when something was copied.
    Paste {
        ready: bool,
    },
    Blank,
}

/// One row of the page: the channel in the slot and whether the selected face's block works
/// with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: Option<ChannelKind>,
    pub here: bool,
}

/// Everything the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// Which faces are joints, in `Face::ALL` order.
    pub joints: [bool; 6],
    pub selected: Face,
    pub rows: [Row; SLOTS],
    /// The selected face's settings, each resolved (the block's default where none is kept).
    pub settings: [JointSetting; SLOTS],
    /// The slot whose filter takes the next item clicked in the inventory.
    pub armed: Option<usize>,
    /// Something was copied.
    pub clipboard: bool,
}

/// What a click on a cell asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Select(Face),
    Edit {
        slot: usize,
        edit: Edit,
    },
    /// The next item clicked in the inventory goes into this slot's filter; `None` disarms.
    Arm(Option<usize>),
    Copy,
    Paste,
}

/// The first face to show: `preferred` when it is a joint, else the first joint.
pub fn first_page(joints: &[bool; 6], preferred: Option<Face>) -> Option<Face> {
    preferred
        .filter(|f| joints[usize::from(f.index())])
        .or_else(|| {
            Face::ALL
                .into_iter()
                .find(|f| joints[usize::from(f.index())])
        })
}

/// The 54 cells of the page, row by row.
pub fn cells(page: &Page) -> Vec<Cell> {
    let mut cells = vec![Cell::Blank; CELLS];
    for face in Face::ALL {
        let i = usize::from(face.index());
        cells[i] = Cell::Tab {
            face,
            joint: page.joints[i],
            selected: face == page.selected,
        };
    }
    for slot in 0..SLOTS {
        let row = (slot + 1) * COLUMNS;
        for (column, cell) in row_cells(page, slot).into_iter().enumerate() {
            cells[row + column] = cell;
        }
    }
    cells[HELP] = Cell::Help;
    cells[COPY] = Cell::Copy;
    cells[PASTE] = Cell::Paste {
        ready: page.clipboard,
    };
    cells
}

fn row_cells(page: &Page, slot: usize) -> Vec<Cell> {
    let Row { kind, here } = page.rows[slot];
    let unused = |why| Cell::Unused { slot, why };
    let mut row = vec![Cell::Channel { slot, kind }];
    let Some(kind) = kind else {
        row.resize(COLUMNS, unused(Unused::Empty));
        return row;
    };
    if !here {
        row.resize(COLUMNS, unused(Unused::NotHere));
        return row;
    }
    let s = &page.settings[slot];
    row.push(Cell::Direction { slot, mode: s.mode });
    if kind != ChannelKind::Item {
        row.resize(COLUMNS, unused(Unused::NotItems));
        return row;
    }
    row.push(Cell::List {
        slot,
        allow_only: s.filter.allow_only,
    });
    let mut items = s.filter.items.iter();
    let mut armed = page.armed == Some(slot);
    for _ in 0..FILTER_LIMIT {
        let item = items.next().cloned();
        // Only the first empty cell shows the slot as armed.
        let this_armed = armed && item.is_none();
        armed &= !this_armed;
        row.push(Cell::Filter {
            slot,
            item,
            armed: this_armed,
        });
    }
    let priority = s.priority;
    row.push(Cell::PriorityDown { slot, priority });
    row.push(Cell::Priority { slot, priority });
    row.push(Cell::PriorityUp { slot, priority });
    debug_assert!(
        row.len() == COLUMNS
            && matches!(row[DIRECTION], Cell::Direction { .. })
            && matches!(row[LIST], Cell::List { .. })
            && matches!(row[FILTER], Cell::Filter { .. })
            && matches!(row[PRIORITY_DOWN], Cell::PriorityDown { .. })
            && matches!(row[PRIORITY], Cell::Priority { .. })
            && matches!(row[PRIORITY_UP], Cell::PriorityUp { .. })
    );
    row
}

/// What a left (or `right`) click on cell `index` of the page asks for.
pub fn click(page: &Page, index: usize, right: bool) -> Action {
    let cells = cells(page);
    let Some(cell) = cells.get(index) else {
        return Action::None;
    };
    let edit = |slot, edit| Action::Edit { slot, edit };
    match cell {
        Cell::Tab {
            face, joint: true, ..
        } => Action::Select(*face),
        Cell::Direction { slot, .. } if right => edit(*slot, Edit::PreviousMode),
        Cell::Direction { slot, .. } => edit(*slot, Edit::NextMode),
        Cell::List { slot, .. } => edit(*slot, Edit::ToggleList),
        Cell::Filter {
            slot,
            item: Some(item),
            ..
        } => edit(*slot, Edit::RemoveItem(item.clone())),
        Cell::Filter { armed: true, .. } => Action::Arm(None),
        Cell::Filter { slot, .. } => Action::Arm(Some(*slot)),
        Cell::PriorityDown { slot, .. } => edit(*slot, Edit::Priority(-1)),
        Cell::PriorityUp { slot, .. } => edit(*slot, Edit::Priority(1)),
        Cell::Priority { slot, .. } => edit(*slot, Edit::Priority(if right { -1 } else { 1 })),
        Cell::Copy => Action::Copy,
        Cell::Paste { ready: true } => Action::Paste,
        _ => Action::None,
    }
}

/// An item clicked in the player's inventory below the page: into the armed slot's filter.
pub fn pick(page: &Page, item: &str) -> Action {
    match page.armed {
        Some(slot) => Action::Edit {
            slot,
            edit: Edit::AddItem(item.to_owned()),
        },
        None => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Page {
        let mut joints = [false; 6];
        joints[usize::from(Face::YPos.index())] = true;
        joints[usize::from(Face::XPos.index())] = true;
        Page {
            joints,
            selected: Face::YPos,
            rows: [
                Row {
                    kind: Some(ChannelKind::Item),
                    here: true,
                },
                Row {
                    kind: Some(ChannelKind::Energy),
                    here: true,
                },
                Row {
                    kind: None,
                    here: false,
                },
                Row {
                    kind: Some(ChannelKind::Item),
                    here: false,
                },
            ],
            settings: std::array::from_fn(|_| JointSetting::of_mode(Mode::Into)),
            armed: None,
            clipboard: false,
        }
    }

    #[test]
    fn the_top_row_picks_a_face_that_is_a_joint() {
        let p = page();
        let cells = cells(&p);
        assert_eq!(cells.len(), CELLS);
        let up = usize::from(Face::YPos.index());
        assert_eq!(
            cells[up],
            Cell::Tab {
                face: Face::YPos,
                joint: true,
                selected: true
            }
        );
        assert_eq!(
            click(&p, usize::from(Face::XPos.index()), false),
            Action::Select(Face::XPos)
        );
        // A face with no joint does nothing.
        assert_eq!(
            click(&p, usize::from(Face::XNeg.index()), false),
            Action::None
        );
    }

    #[test]
    fn an_item_row_has_direction_list_filter_and_priority() {
        let p = page();
        let cells = cells(&p);
        let row = &cells[COLUMNS..2 * COLUMNS];
        assert_eq!(
            row[0],
            Cell::Channel {
                slot: 0,
                kind: Some(ChannelKind::Item)
            }
        );
        assert!(matches!(
            row[DIRECTION],
            Cell::Direction {
                slot: 0,
                mode: Mode::Into
            }
        ));
        assert!(matches!(
            row[LIST],
            Cell::List {
                allow_only: false,
                ..
            }
        ));
        assert!(matches!(
            row[FILTER],
            Cell::Filter {
                item: None,
                armed: false,
                ..
            }
        ));
        assert!(matches!(row[PRIORITY], Cell::Priority { priority: 0, .. }));
        let at = |column| COLUMNS + column;
        assert_eq!(
            click(&p, at(DIRECTION), false),
            Action::Edit {
                slot: 0,
                edit: Edit::NextMode
            }
        );
        assert_eq!(
            click(&p, at(DIRECTION), true),
            Action::Edit {
                slot: 0,
                edit: Edit::PreviousMode
            }
        );
        assert_eq!(
            click(&p, at(PRIORITY_UP), false),
            Action::Edit {
                slot: 0,
                edit: Edit::Priority(1)
            }
        );
        assert_eq!(
            click(&p, at(PRIORITY_DOWN), false),
            Action::Edit {
                slot: 0,
                edit: Edit::Priority(-1)
            }
        );
        assert_eq!(
            click(&p, at(LIST), false),
            Action::Edit {
                slot: 0,
                edit: Edit::ToggleList
            }
        );
    }

    #[test]
    fn energy_empty_and_unreachable_rows_say_why_they_do_little() {
        let p = page();
        let cells = cells(&p);
        let energy = &cells[2 * COLUMNS..3 * COLUMNS];
        assert!(matches!(energy[DIRECTION], Cell::Direction { slot: 1, .. }));
        assert!(matches!(
            energy[LIST],
            Cell::Unused {
                why: Unused::NotItems,
                ..
            }
        ));
        let empty = &cells[3 * COLUMNS..4 * COLUMNS];
        assert!(matches!(empty[0], Cell::Channel { kind: None, .. }));
        assert!(matches!(
            empty[DIRECTION],
            Cell::Unused {
                why: Unused::Empty,
                ..
            }
        ));
        let far = &cells[4 * COLUMNS..5 * COLUMNS];
        assert!(matches!(
            far[DIRECTION],
            Cell::Unused {
                why: Unused::NotHere,
                ..
            }
        ));
        assert_eq!(click(&p, 3 * COLUMNS + DIRECTION, false), Action::None);
    }

    #[test]
    fn a_filter_item_comes_from_the_inventory_after_arming_a_cell() {
        let mut p = page();
        assert_eq!(pick(&p, "minecraft:raw_iron"), Action::None);
        assert_eq!(click(&p, COLUMNS + FILTER, false), Action::Arm(Some(0)));
        p.armed = Some(0);
        assert!(matches!(
            cells(&p)[COLUMNS + FILTER],
            Cell::Filter { armed: true, .. }
        ));
        // A second click on the armed cell disarms it.
        assert_eq!(click(&p, COLUMNS + FILTER, false), Action::Arm(None));
        assert_eq!(
            pick(&p, "minecraft:raw_iron"),
            Action::Edit {
                slot: 0,
                edit: Edit::AddItem("minecraft:raw_iron".into())
            }
        );
        // A filled cell removes its item.
        p.armed = None;
        p.settings[0] = p.settings[0].apply(&Edit::AddItem("minecraft:raw_iron".into()));
        assert_eq!(
            click(&p, COLUMNS + FILTER, false),
            Action::Edit {
                slot: 0,
                edit: Edit::RemoveItem("minecraft:raw_iron".into())
            }
        );
    }

    #[test]
    fn copy_and_paste_sit_in_the_bottom_row() {
        let mut p = page();
        assert_eq!(click(&p, COPY, false), Action::Copy);
        assert_eq!(click(&p, PASTE, false), Action::None);
        p.clipboard = true;
        assert_eq!(click(&p, PASTE, false), Action::Paste);
        assert_eq!(cells(&p)[HELP], Cell::Help);
    }

    #[test]
    fn the_first_page_is_the_clicked_face_when_it_is_a_joint() {
        let p = page();
        assert_eq!(first_page(&p.joints, Some(Face::XPos)), Some(Face::XPos));
        assert_eq!(first_page(&p.joints, Some(Face::ZNeg)), Some(Face::XPos));
        assert_eq!(first_page(&p.joints, None), Some(Face::XPos));
        assert_eq!(first_page(&[false; 6], None), None);
    }
}
