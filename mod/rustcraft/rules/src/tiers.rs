//! Machine tiers and the right-click replacement (spec 12.1 U1–U4, A5, A8; guide `swap`).
//!
//! A tiered block keeps its tier in a state property, [`PROPERTY`], as the node and the monorail
//! do. Its first tier is the block's own item (`rustcraft:furnace`); each higher one has an item
//! of its own that places the same block (`rustcraft:furnace_2`), made by the first tier's recipe
//! with the tier's alloy instead of the first tier's (U1). A right click with a higher tier's item
//! on a placed lower tier changes only the property (U2): the block entity, so its items, fluids,
//! modules and settings, stays (U3), and the old tier's item goes back to the player.
//!
//! Streams that own other tiered blocks add their [`Family`] to [`FAMILIES`]; a tiered thing that
//! is no block (the worker's chassis) uses [`upgrade`] and [`Family::item`] alone.

use std::ops::RangeInclusive;

/// The block state property that holds the tier, `"1"` to `"4"`.
pub const PROPERTY: &str = "tier";
pub const MAX_TIER: u8 = 4;
/// The property's values, lowest first.
pub const VALUES: [&str; 4] = ["1", "2", "3", "4"];
/// The alloy of each tier (guide `alloys`): copper-steel, sparksteel, crystallite, starsteel.
pub const ALLOYS: [&str; 4] = [
    "rustcraft:coppersteel",
    "rustcraft:sparksteel",
    "rustcraft:crystallite",
    "rustcraft:starsteel",
];
/// The most module sockets a machine of any tier has (A5).
pub const MAX_SOCKETS: usize = 4;

/// A block made in several tiers, from the one it opens on (`first`) up to [`MAX_TIER`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Family {
    pub block: &'static str,
    pub first: u8,
}

impl Family {
    pub const fn new(block: &'static str, first: u8) -> Family {
        Family { block, first }
    }

    /// The tiers it is made in.
    pub fn tiers(self) -> RangeInclusive<u8> {
        self.first..=MAX_TIER
    }

    /// The item of `tier`: the block's own on its first tier, `<block>_<tier>` above it; none for
    /// a tier it is not made in.
    pub fn item(self, tier: u8) -> Option<String> {
        if !self.tiers().contains(&tier) {
            None
        } else if tier == self.first {
            Some(self.block.to_owned())
        } else {
            Some(format!("{}_{tier}", self.block))
        }
    }

    /// The tier `item` places, if it is one of this family's.
    pub fn tier_of(self, item: &str) -> Option<u8> {
        self.tiers().find(|t| self.item(*t).as_deref() == Some(item))
    }

    /// The tier a state property's `value` names; the first tier for none or one out of range.
    pub fn tier_from(self, value: Option<&str>) -> u8 {
        value
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|t| self.tiers().contains(t))
            .unwrap_or(self.first)
    }
}

/// Every tiered block. The machines are stream 3's (guide `machine`: each opens on its own tier,
/// `SLOTS_FROM`); other streams add their own lines below theirs.
pub const FAMILIES: &[Family] = &[
    // Stream 3: the machines.
    Family::new("rustcraft:furnace", 1),
    Family::new("rustcraft:crusher", 1),
    Family::new("rustcraft:alloy", 1),
    Family::new("rustcraft:pump", 1),
    Family::new("rustcraft:trash", 1),
    Family::new("rustcraft:crafter", 2),
    Family::new("rustcraft:placer", 2),
    Family::new("rustcraft:breaker", 2),
    Family::new("rustcraft:mixer", 2),
    Family::new("rustcraft:disintegrator", 3),
    Family::new("rustcraft:charger", 3),
];

/// The family of a placed `block`.
pub fn family_of_block(block: &str) -> Option<Family> {
    FAMILIES.iter().copied().find(|f| f.block == block)
}

/// The family and tier an `item` places.
pub fn family_of_item(item: &str) -> Option<(Family, u8)> {
    FAMILIES
        .iter()
        .find_map(|f| f.tier_of(item).map(|t| (*f, t)))
}

/// The alloy of `tier` (1 to 4).
pub fn alloy(tier: u8) -> Option<&'static str> {
    ALLOYS.get(usize::from(tier).checked_sub(1)?).copied()
}

/// U2: a thing of tier `held` replaces one of tier `placed` only when it is higher.
pub fn upgrade(placed: u8, held: u8) -> bool {
    held > placed
}

/// What a right click with `held_item` on a placed `block` of tier `placed_tier` does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Swap {
    /// The block becomes tier `to`; the player gets `give_back`, the old tier's item.
    Up { to: u8, give_back: String },
    /// Same family, but not a higher tier: nothing is replaced.
    NotHigher,
    /// The item is no higher tier of this block.
    Other,
}

pub fn swap(held_item: &str, block: &str, placed_tier: u8) -> Swap {
    let Some(family) = family_of_block(block) else {
        return Swap::Other;
    };
    let Some(held) = family.tier_of(held_item) else {
        return Swap::Other;
    };
    let placed = family.tier_from(Some(&placed_tier.to_string()));
    if !upgrade(placed, held) {
        return Swap::NotHigher;
    }
    match family.item(placed) {
        Some(give_back) => Swap::Up { to: held, give_back },
        None => Swap::Other,
    }
}

/// A5, guide `SLOTS_BY_TIER`: module sockets by tier, 3 / 3 / 4 / 4.
pub fn sockets(tier: u8) -> usize {
    if tier >= 3 { MAX_SOCKETS } else { 3 }
}

/// A8: how much faster a machine of `tier` works than its first tier: `1 + step` a tier above it.
pub fn speed(family: Family, tier: u8, step: f64) -> f64 {
    let above = family.tier_from(Some(&tier.to_string())) - family.first;
    1.0 + step * f64::from(above)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FURNACE: Family = Family::new("rustcraft:furnace", 1);
    const CRAFTER: Family = Family::new("rustcraft:crafter", 2);

    #[test]
    fn family_items_by_tier() {
        assert_eq!(FURNACE.item(1).as_deref(), Some("rustcraft:furnace"));
        assert_eq!(FURNACE.item(2).as_deref(), Some("rustcraft:furnace_2"));
        assert_eq!(FURNACE.item(4).as_deref(), Some("rustcraft:furnace_4"));
        assert_eq!(FURNACE.item(5), None);
        assert_eq!(FURNACE.item(0), None);
        assert_eq!(CRAFTER.item(1), None);
        assert_eq!(CRAFTER.item(2).as_deref(), Some("rustcraft:crafter"));
        assert_eq!(CRAFTER.item(3).as_deref(), Some("rustcraft:crafter_3"));
    }

    #[test]
    fn tier_of_round_trips() {
        for f in FAMILIES {
            for t in f.tiers() {
                let item = f.item(t).unwrap();
                assert_eq!(f.tier_of(&item), Some(t));
                assert_eq!(family_of_item(&item), Some((*f, t)));
            }
        }
        assert_eq!(FURNACE.tier_of("rustcraft:furnace_9"), None);
        assert_eq!(family_of_item("minecraft:stone"), None);
    }

    #[test]
    fn tier_from_clamps() {
        assert_eq!(FURNACE.tier_from(Some("3")), 3);
        assert_eq!(FURNACE.tier_from(None), 1);
        assert_eq!(FURNACE.tier_from(Some("7")), 1);
        assert_eq!(CRAFTER.tier_from(Some("1")), 2);
        assert_eq!(CRAFTER.tier_from(Some("x")), 2);
    }

    #[test]
    fn swap_only_upward() {
        assert_eq!(
            swap("rustcraft:furnace_3", "rustcraft:furnace", 1),
            Swap::Up { to: 3, give_back: "rustcraft:furnace".into() }
        );
        assert_eq!(
            swap("rustcraft:furnace_4", "rustcraft:furnace", 2),
            Swap::Up { to: 4, give_back: "rustcraft:furnace_2".into() }
        );
        assert_eq!(swap("rustcraft:furnace_2", "rustcraft:furnace", 2), Swap::NotHigher);
        assert_eq!(swap("rustcraft:furnace", "rustcraft:furnace", 3), Swap::NotHigher);
        assert_eq!(swap("rustcraft:crusher_2", "rustcraft:furnace", 1), Swap::Other);
        assert_eq!(swap("rustcraft:furnace_2", "minecraft:stone", 1), Swap::Other);
        // A crafter saved with tier 1 (an old world) counts as its first tier, 2.
        assert_eq!(
            swap("rustcraft:crafter_3", "rustcraft:crafter", 1),
            Swap::Up { to: 3, give_back: "rustcraft:crafter".into() }
        );
    }

    #[test]
    fn upgrade_is_strictly_higher() {
        assert!(upgrade(1, 2));
        assert!(!upgrade(2, 2));
        assert!(!upgrade(3, 2));
    }

    #[test]
    fn sockets_three_three_four_four() {
        assert_eq!([1, 2, 3, 4].map(sockets), [3, 3, 4, 4]);
    }

    #[test]
    fn speed_grows_from_first_tier() {
        assert_eq!(speed(FURNACE, 1, 0.5), 1.0);
        assert_eq!(speed(FURNACE, 3, 0.5), 2.0);
        assert_eq!(speed(CRAFTER, 2, 0.5), 1.0);
        assert_eq!(speed(CRAFTER, 4, 0.5), 2.0);
    }

    #[test]
    fn alloys_by_tier() {
        assert_eq!(alloy(1), Some("rustcraft:coppersteel"));
        assert_eq!(alloy(4), Some("rustcraft:starsteel"));
        assert_eq!(alloy(0), None);
        assert_eq!(alloy(5), None);
    }

    #[test]
    fn every_family_block_is_unique() {
        for (i, a) in FAMILIES.iter().enumerate() {
            assert!((1..=MAX_TIER).contains(&a.first));
            for b in &FAMILIES[i + 1..] {
                assert_ne!(a.block, b.block);
            }
        }
    }
}
