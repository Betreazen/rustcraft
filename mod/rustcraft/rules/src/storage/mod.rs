//! The storage: a box of cells and one or more terminals (S1). Every cell adds a page of slots
//! (S2); the stack module in the terminal raises the stack size of the whole storage (S3). For each
//! item the player can set a limit: the storage stops taking the item at it, or destroys the
//! surplus if that is switched on (S5). A storage whose cells are not a solid box shows which cell
//! is wrong and loses nothing (S8).

use std::collections::{BTreeMap, BTreeSet};

use crate::geom::Pos;
use crate::net::items::ItemKey;

pub mod cells;
pub mod keys;
pub mod terminal;
pub mod view;
pub mod wire;

/// Slots each cell adds (S2).
pub const SLOTS_PER_CELL: u32 = 27;

/// The stack size of a 64-stacking item for each tier of the stack module, 0 for none (S3).
pub const STACK_BY_MODULE: [u32; 5] = [64, 256, 1024, 4096, 4096];

/// A player's limit on one item (S5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub max: u64,
    pub destroy_surplus: bool,
}

/// The contents and settings of one storage.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Store {
    pub cells: u32,
    /// Tier of the stack module in the terminal, 0 for none.
    pub stack_module: u8,
    pub items: BTreeMap<ItemKey, u64>,
    pub limits: BTreeMap<ItemKey, Limit>,
}

impl Store {
    /// How many of `item` one slot holds, given how many the base game stacks (`base_max`: 64, 16 or 1).
    pub fn stack_of(&self, base_max: u32) -> u64 {
        if base_max <= 1 {
            return 1;
        }
        let module = STACK_BY_MODULE[usize::from(self.stack_module).min(STACK_BY_MODULE.len() - 1)];
        u64::from(base_max) * u64::from(module) / 64
    }

    /// How many of `item` the storage takes now (for the net, M6).
    pub fn room_for(&self, item: &str, base_max: &dyn Fn(&str) -> u32) -> u64 {
        match self.limits.get(item) {
            // The surplus is destroyed, so the storage always takes the item.
            Some(l) if l.destroy_surplus => u64::MAX,
            Some(l) => self
                .physical_room(item, base_max)
                .min(l.max.saturating_sub(self.count(item))),
            None => self.physical_room(item, base_max),
        }
    }

    /// Puts up to `count` of `item` in; returns how many it took (destroyed surplus counts as taken).
    pub fn put(&mut self, item: &str, count: u64, base_max: &dyn Fn(&str) -> u32) -> u64 {
        let destroy = self.limits.get(item).is_some_and(|l| l.destroy_surplus);
        let mut keep = count.min(self.physical_room(item, base_max));
        if let Some(l) = self.limits.get(item) {
            keep = keep.min(l.max.saturating_sub(self.count(item)));
        }
        if keep > 0 {
            *self.items.entry(item.to_string()).or_insert(0) += keep;
        }
        if destroy { count } else { keep }
    }

    /// Takes up to `count` of `item` out; returns how many it took.
    pub fn take(&mut self, item: &str, count: u64) -> u64 {
        let Some(held) = self.items.get_mut(item) else {
            return 0;
        };
        let taken = (*held).min(count);
        *held -= taken;
        if *held == 0 {
            self.items.remove(item);
        }
        taken
    }

    /// Puts `count` of `item` back whatever the room and the limits: what a player's full
    /// inventory or a refusing block hands back after a take, which must not be lost.
    pub fn restore(&mut self, item: &str, count: u64) {
        if count > 0 {
            *self.items.entry(item.to_string()).or_insert(0) += count;
        }
    }

    /// Takes in everything `other` holds and the limits it has that this one has not: two
    /// storages joined into one by a new cell (S1).
    pub fn absorb(&mut self, other: Store) {
        for (item, n) in other.items {
            self.restore(&item, n);
        }
        for (item, limit) in other.limits {
            self.limits.entry(item).or_insert(limit);
        }
    }

    /// Slots the storage has.
    pub fn total_slots(&self) -> u64 {
        u64::from(self.cells) * u64::from(SLOTS_PER_CELL)
    }

    /// Slots in use.
    pub fn used_slots(&self, base_max: &dyn Fn(&str) -> u32) -> u64 {
        self.items
            .iter()
            .map(|(item, n)| n.div_ceil(self.stack_of(base_max(item))))
            .sum()
    }

    /// How many of `item` the storage holds.
    pub fn count(&self, item: &str) -> u64 {
        self.items.get(item).copied().unwrap_or(0)
    }

    /// Room by slots alone: free slots plus what is left in the item's last part-filled slot.
    fn physical_room(&self, item: &str, base_max: &dyn Fn(&str) -> u32) -> u64 {
        let stack = self.stack_of(base_max(item));
        let total = u64::from(self.cells) * u64::from(SLOTS_PER_CELL);
        let free = total.saturating_sub(self.used_slots(base_max));
        let rest = self.count(item) % stack;
        let partial = if rest == 0 { 0 } else { stack - rest };
        free * stack + partial
    }
}

/// S8: the cells form a solid box. Otherwise the cells to add (holes inside the box) or to remove
/// (cells outside the biggest box) are named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    Solid,
    Holes(BTreeSet<Pos>),
}

/// Checks that `cells` fill their bounding box.
pub fn shape(cells: &BTreeSet<Pos>) -> Shape {
    let Some(first) = cells.first() else {
        return Shape::Solid;
    };
    let (mut lo, mut hi) = (*first, *first);
    for c in cells {
        for a in 0..3 {
            lo[a] = lo[a].min(c[a]);
            hi[a] = hi[a].max(c[a]);
        }
    }
    let mut holes = BTreeSet::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                if !cells.contains(&[x, y, z]) {
                    holes.insert([x, y, z]);
                }
            }
        }
    }
    if holes.is_empty() {
        Shape::Solid
    } else {
        Shape::Holes(holes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vanilla(item: &str) -> u32 {
        match item {
            "minecraft:iron_sword" => 1,
            "minecraft:ender_pearl" => 16,
            _ => 64,
        }
    }

    fn store(cells: u32) -> Store {
        Store {
            cells,
            ..Store::default()
        }
    }

    #[test]
    fn every_cell_adds_a_page_of_twenty_seven_slots() {
        // Rules S1, S2.
        let mut s = store(2);
        assert_eq!(s.room_for("minecraft:cobblestone", &vanilla), 54 * 64);
        assert_eq!(
            s.put("minecraft:cobblestone", 54 * 64 + 10, &vanilla),
            54 * 64
        );
        assert_eq!(s.room_for("minecraft:cobblestone", &vanilla), 0);
        assert_eq!(s.room_for("minecraft:dirt", &vanilla), 0);
    }

    #[test]
    fn the_stack_module_raises_every_stack() {
        // Rule S3: 64 → 256; a 16-stacking item grows by the same factor; a sword stays one a slot.
        let s = Store {
            stack_module: 1,
            ..store(1)
        };
        assert_eq!(s.stack_of(64), 256);
        assert_eq!(s.stack_of(16), 64);
        assert_eq!(s.stack_of(1), 1);
        assert_eq!(s.room_for("minecraft:cobblestone", &vanilla), 27 * 256);
    }

    #[test]
    fn items_share_the_slots() {
        let mut s = store(1);
        s.put("minecraft:iron_sword", 20, &vanilla);
        assert_eq!(s.used_slots(&vanilla), 20);
        assert_eq!(s.room_for("minecraft:cobblestone", &vanilla), 7 * 64);
        // A part-filled slot still takes its item.
        s.put("minecraft:ender_pearl", 3, &vanilla);
        assert_eq!(s.room_for("minecraft:ender_pearl", &vanilla), 6 * 16 + 13);
    }

    #[test]
    fn a_limit_stops_taking_the_item() {
        // Rule S5.
        let mut s = store(10);
        s.limits.insert(
            "minecraft:book".into(),
            Limit {
                max: 256,
                destroy_surplus: false,
            },
        );
        assert_eq!(s.put("minecraft:book", 300, &vanilla), 256);
        assert_eq!(s.room_for("minecraft:book", &vanilla), 0);
    }

    #[test]
    fn a_limit_with_destroy_surplus_takes_and_destroys_the_rest() {
        // Rule S5: seeds over the limit vanish and do not clog the line.
        let mut s = store(10);
        s.limits.insert(
            "minecraft:wheat_seeds".into(),
            Limit {
                max: 64,
                destroy_surplus: true,
            },
        );
        assert_eq!(s.put("minecraft:wheat_seeds", 100, &vanilla), 100);
        assert_eq!(s.items["minecraft:wheat_seeds"], 64);
        assert!(s.room_for("minecraft:wheat_seeds", &vanilla) > 0);
    }

    #[test]
    fn a_solid_box_of_cells_is_a_storage() {
        // Rule S8.
        let mut cells = BTreeSet::new();
        for x in 0..2 {
            for y in 0..2 {
                cells.insert([x, y, 0]);
            }
        }
        assert_eq!(shape(&cells), Shape::Solid);
        cells.remove(&[1, 1, 0]);
        assert_eq!(shape(&cells), Shape::Holes(BTreeSet::from([[1, 1, 0]])));
    }

    #[test]
    fn taking_and_handing_back_lose_nothing() {
        let mut s = store(1);
        s.put("minecraft:cobblestone", 100, &vanilla);
        assert_eq!(s.take("minecraft:cobblestone", 64), 64);
        assert_eq!(s.take("minecraft:cobblestone", 64), 36);
        assert!(s.items.is_empty());
        // A full inventory hands items back past the limit: they are kept.
        s.limits.insert(
            "minecraft:dirt".into(),
            Limit {
                max: 0,
                destroy_surplus: true,
            },
        );
        s.restore("minecraft:dirt", 5);
        assert_eq!(s.count("minecraft:dirt"), 5);
    }

    #[test]
    fn two_storages_joined_hold_what_both_held() {
        // Rule S1: a cell that joins two boxes makes one storage.
        let mut a = store(2);
        a.put("minecraft:cobblestone", 10, &vanilla);
        let mut b = store(1);
        b.put("minecraft:cobblestone", 5, &vanilla);
        b.put("minecraft:book", 3, &vanilla);
        b.limits.insert(
            "minecraft:book".into(),
            Limit {
                max: 256,
                destroy_surplus: false,
            },
        );
        a.absorb(b);
        assert_eq!(a.count("minecraft:cobblestone"), 15);
        assert_eq!(a.count("minecraft:book"), 3);
        assert_eq!(a.limits["minecraft:book"].max, 256);
        assert_eq!(a.total_slots(), 54);
    }
}
