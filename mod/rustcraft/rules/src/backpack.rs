//! The backpack (spec 4.11 G4, guide `backpack`): an item in an ordinary inventory slot that
//! opens a chest-like window on a right click. Sparksteel's has 27 slots, crystallite's 54
//! (`Tuning::backpack_slots`); starsteel's, which goes into its own armour slot (18.5 G4), is plan
//! slice 4. Each backpack carries a number in its `minecraft:custom_data` component; what it holds
//! lives on the server under that number, so the item in hand never changes while it is open.

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
/// Its window shows a page of five rows; the sixth holds the page arrows.
pub const PAGE: usize = 45;
pub const PREV_SLOT: usize = 45;
pub const NEXT_SLOT: usize = 53;

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

/// How many pages a window of `slots` has: one when it fits a chest window.
pub fn pages(slots: usize) -> usize {
    if slots <= 54 { 1 } else { slots.div_ceil(PAGE) }
}

/// The backpack's slots page `page` of a paged window shows.
pub fn page_range(slots: usize, page: usize) -> std::ops::Range<usize> {
    let start = (page * PAGE).min(slots);
    start..(start + PAGE).min(slots)
}

/// The page a click on window slot `slot` turns to from `page`, if it is an arrow there.
pub fn turn(slots: usize, page: usize, slot: usize) -> Option<usize> {
    let last = pages(slots).saturating_sub(1);
    match slot {
        PREV_SLOT if page > 0 => Some(page - 1),
        NEXT_SLOT if page < last => Some(page + 1),
        _ => None,
    }
}

/// The rows of its window: nine slots a row, one to six rows.
pub fn rows(slots: usize) -> u8 {
    slots.div_ceil(9).clamp(1, 6) as u8
}

/// What stays in a backpack when its window closes: anything but another backpack, which goes
/// back to the player (a backpack in a backpack would hide one store inside another).
pub fn keeps(item: &str) -> bool {
    !is_backpack(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backpack_has_its_tiers_slots_in_whole_rows() {
        let t = Tuning::default();
        assert_eq!(slots("rustcraft:backpack", &t), Some(27));
        assert_eq!(slots("rustcraft:backpack_2", &t), Some(54));
        assert_eq!(slots("minecraft:chest", &t), None);
        assert_eq!(rows(27), 3);
        assert_eq!(rows(54), 6);
        assert_eq!(rows(81), 6);
    }

    #[test]
    fn the_starsteel_backpack_has_81_slots_on_two_pages() {
        let t = Tuning::default();
        assert_eq!(slots(STAR, &t), Some(81));
        assert!(is_backpack(STAR) && !keeps(STAR));
        assert_eq!(pages(81), 2);
        assert_eq!(pages(54), 1);
        assert_eq!(page_range(81, 0), 0..45);
        assert_eq!(page_range(81, 1), 45..81);
        assert_eq!(turn(81, 0, NEXT_SLOT), Some(1));
        assert_eq!(turn(81, 1, NEXT_SLOT), None);
        assert_eq!(turn(81, 1, PREV_SLOT), Some(0));
        assert_eq!(turn(81, 0, PREV_SLOT), None);
        assert_eq!(turn(81, 0, 10), None);
    }

    #[test]
    fn a_backpack_keeps_anything_but_a_backpack() {
        assert!(keeps("minecraft:diamond_sword"));
        assert!(keeps("minecraft:shulker_box"));
        assert!(!keeps("rustcraft:backpack"));
        assert!(!keeps("rustcraft:backpack_2"));
    }
}
