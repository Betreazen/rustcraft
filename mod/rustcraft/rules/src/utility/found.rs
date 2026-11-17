//! Spec 26: suspicious sand and gravel are lifted with the wrench, sneaking, and remember where
//! they were found. The block leaves the world as its item with custom data `rustcraft:found`
//! (`MARK`): the dimension and the cell, and what the block would give, its loot table and seed
//! (or the find a brush stroke already rolled). Placed again, the block gets that loot table
//! back; in the archaeology table it is brushed with it instead of the table the toggle chooses.
//! A lifted block is a block found in the world: the table tells Path-of-craft-tree `brush` for
//! it. A block the living vein grew stays grown when lifted (ST5): its item carries
//! `rustcraft:grown`, not this mark, and gives no `brush`.

use crate::geom::Pos;
use crate::utility::grown;

/// The key in an item's custom data that holds where a lifted block was found (a compound).
pub const MARK: &str = "rustcraft:found";
/// The compound's keys: the dimension (a string), the cell (three ints), the loot table (a
/// string), its seed (a long) and the find already rolled (an item).
pub const DIMENSION: &str = "dimension";
pub const POS: &str = "pos";
pub const LOOT_TABLE: &str = "loot_table";
pub const SEED: &str = "seed";
pub const ITEM: &str = "item";

/// Where a suspicious block was found and what it would give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub dimension: String,
    pub pos: Pos,
    /// The loot table its block entity still had (none for a block a player placed, or one a
    /// brush stroke already rolled).
    pub loot_table: Option<String>,
    pub seed: Option<i64>,
    /// A brush stroke rolled its find already: the item rides in the mark.
    pub rolled: bool,
}

/// What a wrench lift makes of a suspicious block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lift {
    /// The vein grew it: the item is marked grown (ST5).
    Grown,
    /// Found in the world: the item remembers where and what it gives.
    Found(Found),
}

/// Whether the wrench used on `block` lifts it: suspicious sand or gravel, with sneaking (without,
/// the wrench opens joint windows).
pub fn wrench_lifts(block: &str, sneaking: bool) -> bool {
    sneaking && grown::is_suspicious(block)
}

/// The lift of the suspicious block at `pos` of `dimension`: a grown one stays grown, any other
/// remembers its cell and its loot.
pub fn lift(
    dimension: &str,
    pos: Pos,
    grown: bool,
    loot_table: Option<String>,
    seed: Option<i64>,
    rolled: bool,
) -> Lift {
    if grown {
        return Lift::Grown;
    }
    Lift::Found(Found {
        dimension: dimension.to_owned(),
        pos,
        loot_table: loot_table.filter(|t| !t.is_empty()),
        seed,
        rolled,
    })
}

/// The loot table the archaeology table brushes a block with: the one it was found with, else the
/// one its toggle chose.
/// Slice 18 (guide `archeo`: «по таблице того места, где блок лежал»): the table of a block
/// that remembers no place (a creative one): the desert temple's for sand, the trail ruins' for
/// gravel, where most of each is found.
pub fn default_table(block: &str) -> &'static str {
    if block == "minecraft:suspicious_gravel" {
        "minecraft:archaeology/trail_ruins_common"
    } else {
        "minecraft:archaeology/desert_pyramid"
    }
}

pub fn brush_table<'a>(found: Option<&'a Found>, chosen: &'a str) -> &'a str {
    found
        .and_then(|f| f.loot_table.as_deref())
        .unwrap_or(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::Report;

    const OVER: &str = "minecraft:overworld";
    const WELL: &str = "minecraft:archaeology/desert_well";
    const PYRAMID: &str = "minecraft:archaeology/desert_pyramid";

    #[test]
    fn the_wrench_lifts_only_suspicious_blocks_and_only_sneaking() {
        assert!(wrench_lifts("minecraft:suspicious_sand", true));
        assert!(wrench_lifts("minecraft:suspicious_gravel", true));
        assert!(
            !wrench_lifts("minecraft:suspicious_sand", false),
            "opens a joint window"
        );
        assert!(!wrench_lifts("minecraft:sand", true));
        assert!(
            !wrench_lifts("rustcraft:housing", true),
            "takes a channel out"
        );
    }

    #[test]
    fn a_found_block_remembers_where_and_what_it_gives() {
        let l = lift(
            OVER,
            [4, 70, -2],
            false,
            Some(PYRAMID.into()),
            Some(-9),
            false,
        );
        assert_eq!(
            l,
            Lift::Found(Found {
                dimension: OVER.into(),
                pos: [4, 70, -2],
                loot_table: Some(PYRAMID.into()),
                seed: Some(-9),
                rolled: false,
            })
        );
        // A block a player placed has no loot table; an empty one is none.
        let Lift::Found(placed) = lift(OVER, [0, 0, 0], false, Some(String::new()), None, false)
        else {
            panic!("a placed block is a found one");
        };
        assert_eq!(placed.loot_table, None);
        // A stroke that rolled the find already: the find rides along.
        let Lift::Found(rolled) = lift(OVER, [0, 0, 0], false, None, None, true) else {
            panic!("found");
        };
        assert!(rolled.rolled);
    }

    #[test]
    fn a_grown_block_stays_grown_and_gives_no_brush() {
        // ST5 holds through the wrench.
        assert_eq!(
            lift(OVER, [1, 2, 3], true, Some(WELL.into()), None, false),
            Lift::Grown
        );
        assert_eq!(grown::brush_report(Some(5), true), None);
        // A lifted found block is a block found in the world: the table tells `brush`.
        assert_eq!(
            grown::brush_report(Some(5), false),
            Some(Report::Counted {
                kind: "brush",
                owner: 5,
                count: 1
            })
        );
    }

    #[test]
    fn the_table_brushes_with_the_remembered_loot_table() {
        let Lift::Found(f) = lift(OVER, [0, 0, 0], false, Some(PYRAMID.into()), None, false) else {
            panic!("found");
        };
        assert_eq!(brush_table(Some(&f), WELL), PYRAMID);
        let Lift::Found(placed) = lift(OVER, [0, 0, 0], false, None, None, false) else {
            panic!("found");
        };
        assert_eq!(brush_table(Some(&placed), WELL), WELL);
        assert_eq!(brush_table(None, WELL), WELL);
    }
}
