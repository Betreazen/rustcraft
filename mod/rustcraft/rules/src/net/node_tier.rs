//! The node's tiers as items (guide `node`, `swap`): one item a tier places the same block with
//! its `tier`, a broken node gives back its own tier's item, and a click with a node of a higher
//! tier on a placed one takes its place (U2), the old one coming back to the hand.
//!
//! Stream 3 of slice 18 builds the general tier mechanism (`rustcraft_rules::tiers`); until it is
//! merged the node keeps its own functions here, with the same shape, so the bundle switches by
//! changing one import.

/// The node's items, tier 1 first.
pub const ITEMS: [&str; 4] = [
    "rustcraft:node",
    "rustcraft:node_2",
    "rustcraft:node_3",
    "rustcraft:node_4",
];

/// The node tier `item` places, if it is a node item.
pub fn tier_of_item(item: &str) -> Option<u8> {
    ITEMS.iter().position(|i| *i == item).map(|i| i as u8 + 1)
}

/// The item of a node of `tier` (clamped to 1–4).
pub fn item_of_tier(tier: u8) -> &'static str {
    ITEMS[usize::from(tier.clamp(1, 4)) - 1]
}

/// U2: a node of tier `held` clicked on a placed node of tier `placed` takes its place only when
/// it is of a higher tier; the answer is the tier that comes back to the hand.
pub fn swap(placed: u8, held: u8) -> Option<u8> {
    (held > placed).then_some(placed)
}

#[cfg(test)]
mod tests {
    use super::*;

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
