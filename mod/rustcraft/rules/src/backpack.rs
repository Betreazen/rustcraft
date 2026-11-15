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

/// Whether `item` is a backpack.
pub fn is_backpack(item: &str) -> bool {
    ITEMS.contains(&item)
}

/// How many slots the backpack `item` has.
pub fn slots(item: &str, t: &Tuning) -> Option<usize> {
    let i = ITEMS.iter().position(|b| *b == item)?;
    t.backpack_slots.get(i).copied()
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
    fn a_backpack_keeps_anything_but_a_backpack() {
        assert!(keeps("minecraft:diamond_sword"));
        assert!(keeps("minecraft:shulker_box"));
        assert!(!keeps("rustcraft:backpack"));
        assert!(!keeps("rustcraft:backpack_2"));
    }
}
