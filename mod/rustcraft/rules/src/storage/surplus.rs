//! 0.21 (round 32, spec 32): a storage's limit on an item is how much of it the storage keeps.
//! What it holds over the limit is its surplus (SP1): what the owner lowered the limit under, and
//! what arrives over the net past it. The surplus leaves by itself over the storage's nets
//! (`net::surplus`); here the storage only says how much of each it keeps and holds over.

//!
//! 0.21.2 (SP10–SP14), the owner: «даже если выставлены лимиты всё можно положить в склад и
//! переносится с блоков механизмов, но если выставлен лимит, тогда начинается цикл переработки».
//! A limit no longer refuses what is given to the storage (`Source::over_limit`): the player's
//! items, a machine's products, an order's, a dock's, a container the player set «из блока». The
//! storage takes them while its slots hold them; what it then holds over the limit is surplus and
//! leaves by SP1. Only what the storage pulls itself, from a vanilla container on its default joint
//! or from another storage, stops at the limit, so a player's own chest beside a line is never
//! emptied into the trash.

use crate::net::fluids::BUCKET;
use crate::net::items::ItemKey;
use crate::orders::{fluid_key, fluid_of};
use crate::storage::Store;

/// SP10, SP11: where what goes into a storage comes from, as far as its limit cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The player at the terminal or the tablet: a shift-click, the cursor, a deposit button.
    Player,
    /// A machine's products over the net, a fishing station's worn rod.
    Machine,
    /// An order's products, carried back or landing in the storage.
    Order,
    /// A dock of the workers (unload, load) giving to the net.
    Dock,
    /// A container the player set «из блока» with the wrench: «empty it into the net».
    SetContainer,
    /// A vanilla container on its default joint: every chest beside a line, the player's own
    /// among them.
    DefaultContainer,
    /// Another storage giving its stock to the net.
    Storage,
}

impl Source {
    /// SP10: the storage takes from this source past its limit, as far as its slots hold; SP11:
    /// from a container on its default joint and from another storage only up to the limit.
    pub fn over_limit(self) -> bool {
        !matches!(self, Source::DefaultContainer | Source::Storage)
    }

    /// The source a net's joint stands for: a storage, a vanilla container on its default joint
    /// (`Joint::auto`), a machine, or a container the player set.
    pub fn of_joint(store: bool, auto: bool, machine: bool) -> Source {
        if store {
            Source::Storage
        } else if auto {
            Source::DefaultContainer
        } else if machine {
            Source::Machine
        } else {
            Source::SetContainer
        }
    }
}

impl Store {
    /// SP10, SP11: how many of `item` the storage takes now from `source`. Past the limit the
    /// slots alone; everything while it destroys the surplus (S5, SP13).
    pub fn room_from(&self, item: &str, source: Source, base_max: &dyn Fn(&str) -> u32) -> u64 {
        if !source.over_limit() {
            return self.room_for(item, base_max);
        }
        match self.limits.get(item) {
            Some(l) if l.destroy_surplus => u64::MAX,
            _ => self.physical_room(item, base_max),
        }
    }

    /// SP10, SP13: puts up to `count` of `item` from `source` in; how many it took (destroyed
    /// counts as taken). Past the limit it keeps what its slots hold. With «уничтожать» it keeps
    /// past the limit only what a sink of its lines will take (`sent_away`, SP13), and destroys
    /// what it has no room for; with nowhere to send it, it keeps up to the limit and destroys the
    /// rest, as before.
    pub fn put_from(
        &mut self,
        item: &str,
        count: u64,
        source: Source,
        sent_away: bool,
        base_max: &dyn Fn(&str) -> u32,
    ) -> u64 {
        let destroy = self.limits.get(item).is_some_and(|l| l.destroy_surplus);
        if !source.over_limit() || (destroy && !sent_away) {
            return self.put(item, count, base_max);
        }
        let keep = count.min(self.physical_room(item, base_max));
        if keep > 0 {
            *self.items.entry(item.to_owned()).or_insert(0) += keep;
        }
        if destroy { count } else { keep }
    }

    /// SP14: how much of `fluid` (mB) the cells take from a pump, a machine or a tank set «из
    /// блока»: their room, past the limit too; everything while the surplus is destroyed.
    pub fn given_fluid_room(&self, fluid: &str, cell_mb: u64) -> u64 {
        match self.limits.get(&fluid_key(fluid)) {
            Some(l) if l.destroy_surplus => u64::MAX,
            _ => self.fluid_capacity(cell_mb).saturating_sub(self.fluid(fluid)),
        }
    }

    /// SP14: puts up to `amount` mB of `fluid` from such a source in, past the limit as far as the
    /// cells hold; with «уничтожать» what arrives over the limit is destroyed, as before (SP8).
    pub fn put_given_fluid(&mut self, fluid: &str, amount: u64, cell_mb: u64) -> u64 {
        if self.limits.get(&fluid_key(fluid)).is_some_and(|l| l.destroy_surplus) {
            return self.put_fluid(fluid, amount, cell_mb);
        }
        let keep = amount.min(self.given_fluid_room(fluid, cell_mb));
        if keep > 0 {
            *self.fluids.entry(fluid.to_owned()).or_insert(0) += keep;
        }
        keep
    }

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
    use super::Source;
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
    fn a_limit_never_refuses_what_is_given_to_the_storage() {
        // SP10: the player, a machine, an order, a dock, a container set «из блока»: all of it
        // goes in past the limit, as far as the slots hold it.
        for source in [Source::Player, Source::Machine, Source::Order, Source::Dock, Source::SetContainer] {
            assert!(source.over_limit(), "{source:?}");
            let mut s = limited(1, COBBLE, 10, false);
            assert_eq!(s.room_from(COBBLE, source, &vanilla), 27 * 64);
            assert_eq!(s.put_from(COBBLE, 100, source, false, &vanilla), 100);
            assert_eq!(s.count(COBBLE), 100);
            assert_eq!(s.surplus(), vec![(COBBLE.to_owned(), 90)]);
            // The slots full: no more.
            assert_eq!(s.put_from(COBBLE, 27 * 64, source, false, &vanilla), 27 * 64 - 100);
            assert_eq!(s.room_from(COBBLE, source, &vanilla), 0);
        }
    }

    #[test]
    fn a_chest_on_its_default_joint_and_another_storage_stop_at_the_limit() {
        // SP11: what the storage pulls itself stops at the limit, so a player's own chest beside
        // a line is never emptied into the trash.
        for source in [Source::DefaultContainer, Source::Storage] {
            assert!(!source.over_limit(), "{source:?}");
            let mut s = limited(1, COBBLE, 10, false);
            assert_eq!(s.room_from(COBBLE, source, &vanilla), 10);
            assert_eq!(s.put_from(COBBLE, 100, source, true, &vanilla), 10);
            assert_eq!(s.count(COBBLE), 10);
        }
    }

    #[test]
    fn a_joint_names_its_source() {
        assert_eq!(Source::of_joint(true, false, false), Source::Storage);
        assert_eq!(Source::of_joint(false, true, false), Source::DefaultContainer);
        assert_eq!(Source::of_joint(false, false, true), Source::Machine);
        assert_eq!(Source::of_joint(false, false, false), Source::SetContainer);
    }

    #[test]
    fn destroying_keeps_past_the_limit_only_what_a_sink_takes() {
        // SP13: with «уничтожать» and a disintegrator or a trash on its lines the storage holds
        // the surplus for them; with none it keeps to the limit and destroys the rest at once.
        let mut s = limited(1, PICK, 2, true);
        assert_eq!(s.room_from(PICK, Source::Player, &vanilla), u64::MAX);
        assert_eq!(s.put_from(PICK, 5, Source::Player, true, &vanilla), 5);
        assert_eq!(s.count(PICK), 5, "held for the disintegrator");
        let mut s = limited(1, PICK, 2, true);
        assert_eq!(s.put_from(PICK, 5, Source::Player, false, &vanilla), 5);
        assert_eq!(s.count(PICK), 2, "nowhere to send them: destroyed");
        // Past the slots it destroys the rest even with a sink.
        let mut s = limited(1, PICK, 0, true);
        assert_eq!(s.put_from(PICK, 30, Source::Machine, true, &vanilla), 30);
        assert_eq!(s.count(PICK), 27);
    }

    #[test]
    fn a_pumps_fluid_goes_past_the_limit_as_far_as_the_cells_hold() {
        // SP14: 16 buckets a cell here; the limit 3 buckets.
        let water = "minecraft:water";
        let bucket = crate::net::fluids::BUCKET;
        let mut s = limited(1, &crate::orders::fluid_key(water), 3, false);
        assert_eq!(s.given_fluid_room(water, 16 * bucket), 16 * bucket);
        assert_eq!(s.put_given_fluid(water, 20 * bucket, 16 * bucket), 16 * bucket);
        assert_eq!(s.fluid_surplus(water), 13 * bucket);
        // With «уничтожать» what arrives past the limit is destroyed, as before (SP8).
        let mut s = limited(1, &crate::orders::fluid_key(water), 3, true);
        assert_eq!(s.put_given_fluid(water, 5 * bucket, 16 * bucket), 5 * bucket);
        assert_eq!(s.fluid(water), 3 * bucket);
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
