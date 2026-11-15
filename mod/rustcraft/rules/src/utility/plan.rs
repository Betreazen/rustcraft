//! The holo-projector and the building machine (spec 22, BU1). The projector remembers the blocks
//! in its area; the building machine places the missing ones from its slots, and gets builder's
//! experience only for cells of the plan, each cell once for good.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The projector's area from its block: 9 × 9 across, 1..=8 up.
pub const HALF: i32 = 4;
pub const HEIGHT: i32 = 8;
/// The most cells a plan keeps.
pub const MAX_CELLS: usize = 648;

/// One block of a plan, relative to the projector.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cell {
    pub at: [i32; 3],
    /// The block's id (`minecraft:stone_bricks`).
    pub block: String,
}

/// A remembered building.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub cells: Vec<Cell>,
}

/// The offsets of the projector's area, in the order it is read and built: bottom up, so a block
/// stands on what is under it.
pub fn area() -> impl Iterator<Item = [i32; 3]> {
    (1..=HEIGHT).flat_map(|y| (-HALF..=HALF).flat_map(move |x| (-HALF..=HALF).map(move |z| [x, y, z])))
}

impl Plan {
    /// The plan of what stands in the area: `block_at(offset)` names the block there, air and
    /// fluids none.
    pub fn read(mut block_at: impl FnMut([i32; 3]) -> Option<String>) -> Plan {
        let cells = area()
            .filter_map(|at| block_at(at).map(|block| Cell { at, block }))
            .take(MAX_CELLS)
            .collect();
        Plan { cells }
    }

    /// The cells whose block is not there now, in building order.
    pub fn missing<'a>(&'a self, mut block_at: impl FnMut([i32; 3]) -> Option<String> + 'a) -> impl Iterator<Item = &'a Cell> + 'a {
        self.cells.iter().filter(move |c| block_at(c.at).as_deref() != Some(c.block.as_str()))
    }
}

/// The cells the building machine has built and been paid for (BU1): offsets from the projector,
/// kept in its block entity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Built(pub BTreeSet<[i32; 3]>);

impl Built {
    /// Whether placing the block of `at` pays builder's experience: only the first time.
    pub fn pays(&mut self, at: [i32; 3]) -> bool {
        self.0.insert(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_keeps_what_stands_and_names_what_is_missing() {
        let plan = Plan::read(|at| (at == [0, 1, 0] || at == [1, 1, 0]).then(|| "minecraft:stone".to_owned()));
        assert_eq!(plan.cells.len(), 2);
        let missing: Vec<_> = plan
            .missing(|at| (at == [0, 1, 0]).then(|| "minecraft:stone".to_owned()))
            .map(|c| c.at)
            .collect();
        assert_eq!(missing, vec![[1, 1, 0]]);
        assert_eq!(area().count(), 9 * 9 * 8);
    }

    #[test]
    fn each_cell_pays_once() {
        // BU1.
        let mut b = Built::default();
        assert!(b.pays([0, 1, 0]));
        assert!(!b.pays([0, 1, 0]));
    }
}
