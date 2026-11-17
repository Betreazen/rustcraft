//! The node's tiers as items (guide `node`, `swap`): one item a tier places the same block with
//! its `tier`, a broken node gives back its own tier's item, and a click with a node of a higher
//! tier on a placed one takes its place (U2), the old one coming back to the hand.
//!
//! The tier items and the U2 rule come from `rustcraft_rules::tiers` (`tiers::NODE`,
//! `tiers::upgrade`). The node is not in `tiers::FAMILIES`: its placement and replacement are its
//! own (`rustcraft_pipe18`), and a node of tiers 2–4 is made from the node below in the centre of
//! the grid, not by `tools/tiers.py`'s alloy swap (owner 2026-11-17).

use crate::tiers::{self, NODE};

/// The node's items, tier 1 first (`tiers::NODE`'s items).
pub const ITEMS: [&str; 4] = [
    "rustcraft:node",
    "rustcraft:node_2",
    "rustcraft:node_3",
    "rustcraft:node_4",
];

/// The node tier `item` places, if it is a node item.
pub fn tier_of_item(item: &str) -> Option<u8> {
    NODE.tier_of(item)
}

/// The item of a node of `tier` (clamped to 1–4).
pub fn item_of_tier(tier: u8) -> &'static str {
    let tier = tier.clamp(NODE.first, NODE.last);
    ITEMS
        .into_iter()
        .find(|i| NODE.tier_of(i) == Some(tier))
        .unwrap_or(ITEMS[0])
}

/// U2: a node of tier `held` clicked on a placed node of tier `placed` takes its place only when
/// it is of a higher tier; the answer is the tier that comes back to the hand.
pub fn swap(placed: u8, held: u8) -> Option<u8> {
    tiers::upgrade(placed, held).then_some(placed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_items_come_from_the_tier_family() {
        for (i, item) in ITEMS.iter().enumerate() {
            assert_eq!(NODE.item(i as u8 + 1).as_deref(), Some(*item));
        }
        assert!(tiers::family_of_block(NODE.block).is_none());
    }

    #[test]
    fn each_node_item_places_its_own_tier() {
        assert_eq!(tier_of_item("rustcraft:node"), Some(1));
        assert_eq!(tier_of_item("rustcraft:node_4"), Some(4));
        assert_eq!(tier_of_item("rustcraft:housing"), None);
        for tier in 1..=4 {
            assert_eq!(tier_of_item(item_of_tier(tier)), Some(tier));
        }
    }

    #[test]
    fn a_tier_out_of_range_names_the_nearest_item() {
        assert_eq!(item_of_tier(0), "rustcraft:node");
        assert_eq!(item_of_tier(9), "rustcraft:node_4");
    }

    #[test]
    fn only_a_higher_tier_takes_the_place_and_the_old_comes_back() {
        assert_eq!(swap(1, 3), Some(1));
        assert_eq!(swap(2, 2), None);
        assert_eq!(swap(3, 2), None);
    }
}
