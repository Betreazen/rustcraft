//! ST4, ST5 (spec 24): a suspicious block the living vein grew remembers it. The vein notes each
//! cell it grows one in (`Grown`, a file of the world, bounded); a player's breaking or placing in
//! that cell forgets it. The grab takes such a block whole into its hold (ST4), where it keeps the
//! mark in its name (`hold_name`), and the item it becomes carries custom data `rustcraft:grown`.
//! The archaeology table gives its find but tells Path-of-craft-tree nothing (no `brush`), as the
//! vein's ore gives no miner's experience (X6): digger's experience is for blocks found in the
//! world only.

use crate::geom::Pos;
use crate::reports::Report;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The key in an item's custom data that marks a grown block (a byte 1).
pub const MARK: &str = "rustcraft:grown";
/// What a grown block's name ends with in a worker's hold, which keeps plain names only.
pub const HOLD_MARK: &str = "+grown";
/// Grown cells kept a dimension; the oldest is forgotten past this.
pub const LIMIT: usize = 1024;

/// The blocks the vein's stratum grows (ST1) and the table brushes.
pub fn is_suspicious(block: &str) -> bool {
    matches!(block, "minecraft:suspicious_sand" | "minecraft:suspicious_gravel")
}

/// Whether `below` holds a falling block up (`FallingBlock.isFree` false): the vein grows a
/// suspicious block only there, so it stays in the cell it is remembered by.
pub fn holds_up(below: &str) -> bool {
    !matches!(
        below,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "minecraft:water" | "minecraft:lava"
    ) && !below.ends_with("fire")
}

/// The hold's name for `item`, marked when it was grown.
pub fn hold_name(item: &str, grown: bool) -> String {
    if grown { format!("{item}{HOLD_MARK}") } else { item.to_owned() }
}

/// The item a hold's name stands for, and whether it was grown.
pub fn from_hold(name: &str) -> (&str, bool) {
    let (item, grown, _) = from_hold_table(name);
    (item, grown)
}

/// Slice 18, wave 2 (guide `mod_stratum`): the custom data key of the find table the vein's
/// window chose for a grown block; it goes with the block to the archaeology table.
pub const TABLE_KEY: &str = "rustcraft:grown_table";
/// What comes between a grown block's hold name and its find table.
pub const TABLE_MARK: char = '@';

/// A grown block's hold name with its find table: `item+grown@table`.
pub fn hold_name_with(item: &str, table: Option<&str>) -> String {
    match table.filter(|t| !t.is_empty()) {
        Some(t) => format!("{item}{HOLD_MARK}{TABLE_MARK}{t}"),
        None => hold_name(item, true),
    }
}

/// The item a hold's name stands for, whether it was grown and the find table it carries.
pub fn from_hold_table(name: &str) -> (&str, bool, Option<&str>) {
    if let Some(at) = name.find(HOLD_MARK) {
        let rest = &name[at + HOLD_MARK.len()..];
        if rest.is_empty() {
            return (&name[..at], true, None);
        }
        if let Some(table) = rest.strip_prefix(TABLE_MARK) {
            return (&name[..at], true, Some(table));
        }
    }
    (name, false, None)
}

/// The cells the vein grew a suspicious block in, by dimension, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grown {
    pub cells: BTreeMap<String, Vec<Pos>>,
}

impl Grown {
    /// The vein grew a suspicious block at `pos`.
    pub fn grew(&mut self, dimension: &str, pos: Pos) {
        let list = self.cells.entry(dimension.to_owned()).or_default();
        if list.contains(&pos) {
            return;
        }
        if list.len() >= LIMIT {
            list.remove(0);
        }
        list.push(pos);
    }

    pub fn contains(&self, dimension: &str, pos: Pos) -> bool {
        self.cells.get(dimension).is_some_and(|l| l.contains(&pos))
    }

    /// The block at `pos` changed or was taken: whether it was a grown one.
    pub fn forget(&mut self, dimension: &str, pos: Pos) -> bool {
        let Some(list) = self.cells.get_mut(dimension) else {
            return false;
        };
        let before = list.len();
        list.retain(|p| *p != pos);
        let was = list.len() != before;
        if list.is_empty() {
            self.cells.remove(dimension);
        }
        was
    }
}

/// ST4: whether the grab takes `block` at a cell whole: a suspicious block the vein grew there.
pub fn grab_takes(block: &str, grown: bool) -> bool {
    grown && is_suspicious(block)
}

/// What the archaeology table tells Path-of-craft-tree for one brushed block of `owner`'s table:
/// `brush` for a block found in the world, nothing for one the vein grew (ST5).
pub fn brush_report(owner: Option<u128>, grown: bool) -> Option<Report> {
    let owner = owner.filter(|_| !grown)?;
    Some(Report::Counted { kind: "brush", owner, count: 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_name_keeps_the_find_table() {
        let name = hold_name_with("minecraft:suspicious_sand", Some("minecraft:archaeology/desert_well"));
        assert_eq!(
            from_hold_table(&name),
            ("minecraft:suspicious_sand", true, Some("minecraft:archaeology/desert_well"))
        );
        assert_eq!(from_hold(&name), ("minecraft:suspicious_sand", true));
        assert_eq!(from_hold_table("minecraft:suspicious_sand+grown"), ("minecraft:suspicious_sand", true, None));
        assert_eq!(from_hold_table("minecraft:sand"), ("minecraft:sand", false, None));
        assert_eq!(hold_name_with("minecraft:suspicious_sand", None), "minecraft:suspicious_sand+grown");
    }

    const OVER: &str = "minecraft:overworld";

    #[test]
    fn a_grown_block_brushes_without_a_report() {
        // ST5.
        assert_eq!(brush_report(Some(7), true), None);
        assert_eq!(
            brush_report(Some(7), false).map(|r| r.command()),
            Some("pct-machine brush 00000000000000000000000000000007 1".to_owned())
        );
        assert_eq!(brush_report(None, false), None, "a table with no owner");
    }

    #[test]
    fn the_hold_keeps_the_mark_in_the_name() {
        let name = hold_name("minecraft:suspicious_sand", true);
        assert_eq!(name, "minecraft:suspicious_sand+grown");
        assert_eq!(from_hold(&name), ("minecraft:suspicious_sand", true));
        assert_eq!(from_hold("minecraft:suspicious_sand"), ("minecraft:suspicious_sand", false));
        assert_eq!(hold_name("minecraft:iron_ore", false), "minecraft:iron_ore");
    }

    #[test]
    fn grown_cells_are_remembered_forgotten_and_bounded() {
        let mut g = Grown::default();
        g.grew(OVER, [1, 2, 3]);
        g.grew(OVER, [1, 2, 3]);
        assert!(g.contains(OVER, [1, 2, 3]));
        assert!(!g.contains("minecraft:the_nether", [1, 2, 3]));
        assert_eq!(g.cells[OVER].len(), 1);
        assert!(g.forget(OVER, [1, 2, 3]));
        assert!(!g.forget(OVER, [1, 2, 3]));
        assert!(g.cells.is_empty());
        for i in 0..(LIMIT as i32 + 5) {
            g.grew(OVER, [i, 0, 0]);
        }
        assert_eq!(g.cells[OVER].len(), LIMIT);
        assert!(!g.contains(OVER, [0, 0, 0]), "the oldest went first");
        assert!(g.contains(OVER, [LIMIT as i32 + 4, 0, 0]));
        let back: Grown = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(back, g);
    }

    #[test]
    fn a_suspicious_block_grows_only_where_it_does_not_fall() {
        assert!(holds_up("minecraft:stone"));
        assert!(holds_up("minecraft:sand"));
        for free in ["minecraft:air", "minecraft:cave_air", "minecraft:water", "minecraft:soul_fire"] {
            assert!(!holds_up(free), "{free}");
        }
    }

    #[test]
    fn the_grab_takes_only_a_grown_suspicious_block() {
        // ST4.
        assert!(grab_takes("minecraft:suspicious_sand", true));
        assert!(grab_takes("minecraft:suspicious_gravel", true));
        assert!(!grab_takes("minecraft:suspicious_sand", false), "a desert temple's stays");
        assert!(!grab_takes("minecraft:sand", true), "brushed in the world already");
    }
}
