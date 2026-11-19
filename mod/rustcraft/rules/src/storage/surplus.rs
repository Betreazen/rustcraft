//! 0.21 (round 32, spec 32): a storage's limit on an item is how much of it the storage keeps.
//! What it holds over the limit is its surplus (SP1): what the owner lowered the limit under, and
//! what arrives over the net past it. The surplus leaves by itself over the storage's nets
//! (`net::surplus`); here the storage only says how much of each it keeps and holds over.

use crate::net::fluids::BUCKET;
use crate::net::items::ItemKey;
use crate::orders::{fluid_key, fluid_of};
use crate::storage::Store;

impl Store {
    /// How many more of `item` the storage keeps: room by its slots under its limit, whether or
    /// not it destroys the surplus (`room_for` takes everything then).
    pub fn kept_room(&self, item: &str, base_max: &dyn Fn(&str) -> u32) -> u64 {
        let room = self.physical_room(item, base_max);
        match self.limits.get(item) {
            Some(l) => room.min(l.max.saturating_sub(self.count(item))),
            None => room,
        }
    }

    /// SP1: the items the storage holds over their limits, and how many over, in key order.
    pub fn surplus(&self) -> Vec<(ItemKey, u64)> {
        self.limits
            .iter()
            .filter(|(k, _)| fluid_of(k).is_none())
            .filter_map(|(k, l)| {
                let over = self.count(k).saturating_sub(l.max);
                (over > 0).then(|| (k.clone(), over))
            })
            .collect()
    }

    /// SP2: the storage limits `item` and holds all it keeps of it, so what arrives is surplus.
    pub fn at_limit(&self, item: &str) -> bool {
        self.limits.get(item).is_some_and(|l| self.count(item) >= l.max)
    }

    /// SP8: the mB of `fluid` the cells hold over its limit (in buckets, FL5).
    pub fn fluid_surplus(&self, fluid: &str) -> u64 {
        self.limits
            .get(&fluid_key(fluid))
            .map_or(0, |l| self.fluid(fluid).saturating_sub(l.max.saturating_mul(BUCKET)))
    }

    /// SP8: the storage limits `fluid` and holds its limit.
    pub fn fluid_at_limit(&self, fluid: &str) -> bool {
        self.limits
            .get(&fluid_key(fluid))
            .is_some_and(|l| self.fluid(fluid) >= l.max.saturating_mul(BUCKET))
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::{Limit, Store};

    fn vanilla(item: &str) -> u32 {
        if item == "minecraft:iron_pickaxe" { 1 } else { 64 }
    }

    fn limited(cells: u32, item: &str, max: u64, destroy: bool) -> Store {
        let mut s = Store { cells, ..Store::default() };
        s.limits.insert(item.into(), Limit { max, destroy_surplus: destroy });
        s
    }

    const PICK: &str = "minecraft:iron_pickaxe";
    const COBBLE: &str = "minecraft:cobblestone";

    #[test]
    fn what_is_held_over_a_limit_is_surplus() {
        // SP1: the owner lowers the limit under what the storage holds.
        let mut s = limited(2, PICK, 10, false);
        s.restore(PICK, 13);
        s.restore(COBBLE, 500);
        assert_eq!(s.surplus(), vec![(PICK.to_owned(), 3)]);
        s.limits.get_mut(PICK).unwrap().max = 13;
        assert!(s.surplus().is_empty());
        assert!(s.at_limit(PICK));
        assert!(!s.at_limit(COBBLE), "no limit, no surplus");
    }

    #[test]
    fn the_storage_keeps_up_to_its_limit_even_while_it_destroys_the_rest() {
        let mut s = limited(1, COBBLE, 100, true);
        s.restore(COBBLE, 60);
        assert_eq!(s.room_for(COBBLE, &vanilla), u64::MAX, "it takes all and destroys the rest");
        assert_eq!(s.kept_room(COBBLE, &vanilla), 40);
        assert!(!s.at_limit(COBBLE));
        s.restore(COBBLE, 40);
        assert_eq!(s.kept_room(COBBLE, &vanilla), 0);
        assert!(s.at_limit(COBBLE));
        // Without a limit the slots alone.
        let plain = Store { cells: 1, ..Store::default() };
        assert_eq!(plain.kept_room(COBBLE, &vanilla), 27 * 64);
        // A limit over the slots: the slots.
        let roomy = limited(1, PICK, 100, false);
        assert_eq!(roomy.kept_room(PICK, &vanilla), 27);
    }

    #[test]
    fn a_fluid_over_its_limit_in_buckets_is_surplus_too() {
        // SP8 with FL5: the limit counts buckets.
        let water = "minecraft:water";
        let mut s = limited(4, &crate::orders::fluid_key(water), 3, false);
        s.fluids.insert(water.into(), 5 * crate::net::fluids::BUCKET + 200);
        assert_eq!(s.fluid_surplus(water), 2 * crate::net::fluids::BUCKET + 200);
        assert!(s.fluid_at_limit(water));
        assert!(s.surplus().is_empty(), "a fluid's surplus is no item's");
        assert_eq!(s.fluid_surplus("minecraft:lava"), 0);
        assert!(!s.fluid_at_limit("minecraft:lava"));
    }
}
