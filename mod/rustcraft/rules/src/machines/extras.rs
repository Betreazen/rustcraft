//! Slice 18: what a drawn machine window (the furnace, the crusher, the alloy smelter, the pump, the
//! autocrafter) shows besides its own panel: its module sockets (A5, by tier), its hidden queue
//! cells and its result's reserve (rustcraft_rules::machines::queue). They follow the window's own
//! data slots in its menu, at `at` = the window's own count.
//!
//! An item goes as its number in the game's item table plus one (0: empty), as the windows' other
//! item slots do (rustcraft_content::item_number); counts of 256 fit a data slot.

use super::sockets;
use crate::modules::Module;
use crate::tiers::MAX_SOCKETS;

/// The most queue cells a window draws: two a capacity module in four sockets and one for a stack
/// module.
pub const MAX_CELLS: usize = 9;
/// Data slots the extras take: the sockets, the open cells, each cell's item and count, and the
/// reserve.
pub const DATA: usize = sockets::DATA + 1 + 2 * MAX_CELLS + 1;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extras {
    /// Open sockets (3 or 4) and each socket's module.
    pub open: usize,
    pub modules: [Option<Module>; MAX_SOCKETS],
    /// Queue cells open now (a closed cell still holding items is drawn too, from `cells`).
    pub open_cells: usize,
    /// Each cell: the item's number plus one (0 empty) and its count.
    pub cells: [(i32, u32); MAX_CELLS],
    /// What waits behind the result slot.
    pub reserve: u32,
}

pub fn encode(e: &Extras) -> Vec<i32> {
    let mut out = sockets::encode(e.open, &e.modules).to_vec();
    out.push(e.open_cells.min(MAX_CELLS) as i32);
    for (n, c) in e.cells {
        out.push(n.clamp(0, i32::from(i16::MAX)));
        out.push(c.min(i16::MAX as u32) as i32);
    }
    out.push(e.reserve.min(i16::MAX as u32) as i32);
    out
}

/// The extras from a window's data slots `data`, starting at `at`.
pub fn decode(data: &[i32], at: usize) -> Extras {
    let d = data.get(at..).unwrap_or(&[]);
    let (open, modules) = sockets::decode(d);
    let get = |i: usize| d.get(i).copied().unwrap_or(0);
    let base = sockets::DATA;
    Extras {
        open,
        modules,
        open_cells: usize::try_from(get(base)).unwrap_or(0).min(MAX_CELLS),
        cells: std::array::from_fn(|k| {
            (get(base + 1 + 2 * k).max(0), u32::try_from(get(base + 2 + 2 * k)).unwrap_or(0))
        }),
        reserve: u32::try_from(get(base + 1 + 2 * MAX_CELLS)).unwrap_or(0),
    }
}

impl Extras {
    /// How many cells the window draws: the open ones, and past them any that still hold items.
    pub fn shown_cells(&self) -> usize {
        let last_full = self.cells.iter().rposition(|(n, c)| *n > 0 && *c > 0).map_or(0, |i| i + 1);
        self.open_cells.max(last_full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extras_survive_the_data_slots_after_a_panel() {
        let mut cells = [(0, 0); MAX_CELLS];
        cells[0] = (41, 256);
        cells[3] = (7, 3);
        let e = Extras {
            open: 4,
            modules: [Some(Module::Capacity), Some(Module::Stack), None, Some(Module::Yield)],
            open_cells: 3,
            cells,
            reserve: 192,
        };
        let mut data = vec![9, 9, 9];
        data.extend(encode(&e));
        assert_eq!(data.len(), 3 + DATA);
        assert_eq!(decode(&data, 3), e);
        assert_eq!(e.shown_cells(), 4, "a closed cell with items is drawn");
        let empty = decode(&[], 0);
        assert_eq!(empty.open, 3);
        assert_eq!(empty.shown_cells(), 0);
    }
}
