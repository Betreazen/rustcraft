//! The storage: a box of cells and one or more terminals (S1). Every cell adds a page of slots
//! (S2); the stack module in the terminal raises the stack size of the whole storage (S3). For each
//! item the player can set a limit: the storage stops taking the item at it, or destroys the
//! surplus if that is switched on (S5). A storage whose cells are not a solid box shows which cell
//! is wrong and loses nothing (S8).
//!
//! Round 31 (spec 29, FL4, FL5): the cells hold fluids too, as amounts beside the items: each cell
//! a share of each fluid that the stack module multiplies, none of it in the item slots. A limit
//! on a fluid's key (`orders::fluid_key`) counts buckets.

use std::collections::{BTreeMap, BTreeSet};

use crate::geom::Pos;
use crate::net::fluids::BUCKET;
use crate::net::items::ItemKey;
use crate::orders::{fluid_key, fluid_of};

pub mod cells;
pub mod filter;
pub mod keys;
pub mod modules;
// Round 32 (spec 33): the terminal's «Сеть» page.
pub mod network;
// 0.21 (round 32): the surplus over the limits.
pub mod surplus;
pub mod tablet;
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
    /// O14: «держать N»: the items the storage orders whenever it holds fewer than N.
    pub keep: BTreeMap<ItemKey, u64>,
    /// Round 31 (FL4): the fluids its cells hold, in millibuckets, by fluid.
    pub fluids: BTreeMap<String, u64>,
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

    /// Takes in everything `other` holds and the limits and «держать» counts it has that this one
    /// has not: two storages joined into one by a new cell (S1).
    pub fn absorb(&mut self, other: Store) {
        for (item, n) in other.items {
            self.restore(&item, n);
        }
        for (item, limit) in other.limits {
            self.limits.entry(item).or_insert(limit);
        }
        for (item, n) in other.keep {
            self.keep.entry(item).or_insert(n);
        }
        for (fluid, mb) in other.fluids {
            *self.fluids.entry(fluid).or_insert(0) += mb;
        }
    }

    /// FL4: what the storage holds of one fluid at most, mB: `cell_mb` a cell, times what its
    /// stack module multiplies a stack by (×4 / ×16 / ×64).
    pub fn fluid_capacity(&self, cell_mb: u64) -> u64 {
        let module = STACK_BY_MODULE[usize::from(self.stack_module).min(STACK_BY_MODULE.len() - 1)];
        u64::from(self.cells) * cell_mb * u64::from(module) / 64
    }

    /// The mB of `fluid` the cells hold.
    pub fn fluid(&self, fluid: &str) -> u64 {
        self.fluids.get(fluid).copied().unwrap_or(0)
    }

    /// FL5: how much of `fluid` (mB) the storage takes now: room in its cells under the fluid's
    /// limit, in buckets; everything while its surplus is destroyed (S5).
    pub fn fluid_room(&self, fluid: &str, cell_mb: u64) -> u64 {
        let held = self.fluid(fluid);
        let room = self.fluid_capacity(cell_mb).saturating_sub(held);
        match self.limits.get(&fluid_key(fluid)) {
            Some(l) if l.destroy_surplus => u64::MAX,
            Some(l) => room.min(l.max.saturating_mul(BUCKET).saturating_sub(held)),
            None => room,
        }
    }

    /// Puts up to `amount` mB of `fluid` into the cells; how much it took (destroyed surplus
    /// counts as taken).
    pub fn put_fluid(&mut self, fluid: &str, amount: u64, cell_mb: u64) -> u64 {
        let held = self.fluid(fluid);
        let limit = self.limits.get(&fluid_key(fluid)).copied();
        let mut keep = amount.min(self.fluid_capacity(cell_mb).saturating_sub(held));
        if let Some(l) = limit {
            keep = keep.min(l.max.saturating_mul(BUCKET).saturating_sub(held));
        }
        if keep > 0 {
            *self.fluids.entry(fluid.to_owned()).or_insert(0) += keep;
        }
        if limit.is_some_and(|l| l.destroy_surplus) {
            amount
        } else {
            keep
        }
    }

    /// Takes up to `amount` mB of `fluid` out of the cells; how much it took.
    pub fn take_fluid(&mut self, fluid: &str, amount: u64) -> u64 {
        let Some(held) = self.fluids.get_mut(fluid) else {
            return 0;
        };
        let taken = (*held).min(amount);
        *held -= taken;
        if *held == 0 {
            self.fluids.remove(fluid);
        }
        taken
    }

    /// FL4: what the fluid nets need of the storage, without its items: its cells, its stack
    /// module, its fluids and the fluids' limits.
    pub fn fluid_part(&self) -> Store {
        Store {
            cells: self.cells,
            stack_module: self.stack_module,
            fluids: self.fluids.clone(),
            limits: self
                .limits
                .iter()
                .filter(|(k, _)| fluid_of(k).is_some())
                .map(|(k, l)| (k.clone(), *l))
                .collect(),
            ..Store::default()
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

    /// How many of `item` the storage holds; of a fluid's key (FL5), whole buckets.
    pub fn count(&self, item: &str) -> u64 {
        if let Some(fluid) = fluid_of(item) {
            return self.fluid(fluid) / BUCKET;
        }
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

    /// Slice 18: the component keys of `item` the storage names (held, limited or kept), in key
    /// order. A component line's index in the terminal's list is its place here.
    pub fn variants_of(&self, item: &str) -> Vec<&ItemKey> {
        let mut out: BTreeSet<&ItemKey> = BTreeSet::new();
        for k in self.items.keys().chain(self.limits.keys()).chain(self.keep.keys()) {
            if keys::payload(k).is_some() && keys::item_of(k) == item {
                out.insert(k);
            }
        }
        out.into_iter().collect()
    }

    /// The component key of `item` at `index` of `variants_of`.
    pub fn variant(&self, item: &str, index: u16) -> Option<ItemKey> {
        self.variants_of(item).get(usize::from(index)).map(|k| (*k).clone())
    }

    /// The index of the component key `key` among its item's `variants_of`.
    pub fn variant_index(&self, key: &str) -> Option<u16> {
        let item = keys::item_of(key);
        self.variants_of(item)
            .iter()
            .position(|k| k.as_str() == key)
            .and_then(|i| u16::try_from(i).ok())
    }
}

/// S8: the cells form a solid box. Otherwise the cells to add (holes inside the box) or to remove
/// (cells outside the biggest box) are named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    Solid,
    Holes(BTreeSet<Pos>),
    /// Wave 2: cells outside the biggest solid box of them (a protrusion), when fewer are to be
    /// taken away than to be added.
    Extra(BTreeSet<Pos>),
}

/// Wave 2: the most boxes the search for the biggest solid box tries; a bigger bounding box is
/// judged by its holes alone.
const BOX_SEARCH: usize = 2_000_000;

/// The biggest box (lowest and highest corner) every cell of which is in `cells`, inside the
/// bounding box `lo`..=`hi`; none past [`BOX_SEARCH`].
fn biggest_box(cells: &BTreeSet<Pos>, lo: Pos, hi: Pos) -> Option<(Pos, Pos)> {
    let d: [usize; 3] = std::array::from_fn(|a| (hi[a] - lo[a] + 1) as usize);
    let pairs = |n: usize| n * (n + 1) / 2;
    if pairs(d[0]).saturating_mul(pairs(d[1])).saturating_mul(pairs(d[2])) > BOX_SEARCH {
        return None;
    }
    // Prefix sums: p[x][y][z] = cells in lo..(lo + (x, y, z)), exclusive.
    let idx = |x: usize, y: usize, z: usize| (x * (d[1] + 1) + y) * (d[2] + 1) + z;
    let mut p = vec![0i32; (d[0] + 1) * (d[1] + 1) * (d[2] + 1)];
    for x in 1..=d[0] {
        for y in 1..=d[1] {
            for z in 1..=d[2] {
                let c = i32::from(cells.contains(&[lo[0] + x as i32 - 1, lo[1] + y as i32 - 1, lo[2] + z as i32 - 1]));
                p[idx(x, y, z)] = c + p[idx(x - 1, y, z)] + p[idx(x, y - 1, z)] + p[idx(x, y, z - 1)]
                    - p[idx(x - 1, y - 1, z)]
                    - p[idx(x - 1, y, z - 1)]
                    - p[idx(x, y - 1, z - 1)]
                    + p[idx(x - 1, y - 1, z - 1)];
            }
        }
    }
    let count = |a: [usize; 3], b: [usize; 3]| {
        p[idx(b[0], b[1], b[2])] - p[idx(a[0], b[1], b[2])] - p[idx(b[0], a[1], b[2])] - p[idx(b[0], b[1], a[2])]
            + p[idx(a[0], a[1], b[2])]
            + p[idx(a[0], b[1], a[2])]
            + p[idx(b[0], a[1], a[2])]
            - p[idx(a[0], a[1], a[2])]
    };
    let mut best: Option<(i32, [usize; 3], [usize; 3])> = None;
    for x0 in 0..d[0] {
        for x1 in x0 + 1..=d[0] {
            for y0 in 0..d[1] {
                for y1 in y0 + 1..=d[1] {
                    for z0 in 0..d[2] {
                        for z1 in z0 + 1..=d[2] {
                            let vol = ((x1 - x0) * (y1 - y0) * (z1 - z0)) as i32;
                            if best.is_some_and(|(v, _, _)| v >= vol) {
                                continue;
                            }
                            if count([x0, y0, z0], [x1, y1, z1]) == vol {
                                best = Some((vol, [x0, y0, z0], [x1, y1, z1]));
                            }
                        }
                    }
                }
            }
        }
    }
    let (_, a, b) = best?;
    let at = |v: [usize; 3], k: i32| -> Pos { std::array::from_fn(|i| lo[i] + v[i] as i32 + k) };
    Some((at(a, 0), at(b, -1)))
}

/// Checks that `cells` fill their bounding box. Wave 2: when the cells outside the biggest
/// solid box among them are fewer than the holes of the bounding box, those cells are named
/// instead (a protrusion: «подсветит лишнюю ячейку»).
pub fn shape(cells: &BTreeSet<Pos>) -> Shape {
    match holes_shape(cells) {
        Shape::Holes(holes) => {
            let (lo, hi) = bounds(cells);
            let extra: Option<BTreeSet<Pos>> = biggest_box(cells, lo, hi).map(|(a, b)| {
                cells
                    .iter()
                    .filter(|c| (0..3).any(|k| c[k] < a[k] || c[k] > b[k]))
                    .copied()
                    .collect()
            });
            match extra {
                Some(e) if !e.is_empty() && e.len() < holes.len() => Shape::Extra(e),
                _ => Shape::Holes(holes),
            }
        }
        s => s,
    }
}

fn bounds(cells: &BTreeSet<Pos>) -> (Pos, Pos) {
    let first = cells.first().copied().unwrap_or_default();
    let (mut lo, mut hi) = (first, first);
    for c in cells {
        for a in 0..3 {
            lo[a] = lo[a].min(c[a]);
            hi[a] = hi[a].max(c[a]);
        }
    }
    (lo, hi)
}

/// The bounding box's holes.
fn holes_shape(cells: &BTreeSet<Pos>) -> Shape {
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
    fn each_component_stack_of_an_item_has_its_place() {
        // Slice 18: the terminal names a component stack by its item and its place.
        let mut s = store(1);
        let a = keys::component_key("minecraft:potion", "0a");
        let b = keys::component_key("minecraft:potion", "0b");
        s.put(&b, 1, &vanilla);
        s.put(&a, 1, &vanilla);
        s.put("minecraft:potion", 1, &vanilla);
        assert_eq!(s.variants_of("minecraft:potion"), vec![&a, &b]);
        assert_eq!(s.variant_index(&b), Some(1));
        assert_eq!(s.variant("minecraft:potion", 0), Some(a));
        assert_eq!(s.variant("minecraft:potion", 2), None);
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
    fn a_cell_sticking_out_is_the_extra_one() {
        // Wave 2 (guide `cell`: «подсветит лишнюю или недостающую ячейку»): a 2 × 2 with one
        // cell on top is a box and one cell too many, not a 2 × 2 × 2 short of three.
        let mut cells: BTreeSet<Pos> = [[0, 0, 0], [1, 0, 0], [0, 0, 1], [1, 0, 1]].into();
        cells.insert([1, 1, 1]);
        assert_eq!(shape(&cells), Shape::Extra(BTreeSet::from([[1, 1, 1]])));
        // A column of three on a 2 × 2: two too many.
        let mut tall: BTreeSet<Pos> = [[0, 0, 0], [1, 0, 0], [0, 0, 1], [1, 0, 1]].into();
        tall.extend([[0, 1, 0], [0, 2, 0]]);
        assert_eq!(shape(&tall), Shape::Extra(BTreeSet::from([[0, 1, 0], [0, 2, 0]])));
    }

    #[test]
    fn a_hole_is_still_a_hole() {
        // One missing of a 2 × 2 × 2: a hole (one to add) beats four to remove.
        let mut cells = BTreeSet::new();
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    cells.insert([x, y, z]);
                }
            }
        }
        cells.remove(&[1, 1, 1]);
        assert_eq!(shape(&cells), Shape::Holes(BTreeSet::from([[1, 1, 1]])));
        // A tie (an L of three): the missing one is named.
        let l: BTreeSet<Pos> = [[0, 0, 0], [1, 0, 0], [0, 0, 1]].into();
        assert_eq!(shape(&l), Shape::Holes(BTreeSet::from([[1, 0, 1]])));
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

    // ---- Round 31: fluids in the cells (FL4, FL5) ----

    const WATER: &str = "minecraft:water";
    const CELL_MB: u64 = 16 * BUCKET;

    #[test]
    fn each_cell_holds_a_share_of_each_fluid_apart_from_the_slots() {
        // Rule FL4: 16 buckets of each fluid a cell, none of it in the item slots.
        let mut s = store(2);
        assert_eq!(s.put_fluid(WATER, 40 * BUCKET, CELL_MB), 32 * BUCKET);
        assert_eq!(s.put_fluid("rustcraft:solution", 5 * BUCKET, CELL_MB), 5 * BUCKET);
        assert_eq!(s.fluid_room(WATER, CELL_MB), 0);
        assert_eq!(s.used_slots(&vanilla), 0);
        assert_eq!(s.count(&fluid_key(WATER)), 32);
        assert_eq!(s.take_fluid(WATER, 40 * BUCKET), 32 * BUCKET);
        assert!(!s.fluids.contains_key(WATER));
    }

    #[test]
    fn the_stack_module_raises_the_fluid_share_too() {
        let s = Store {
            stack_module: 1,
            ..store(1)
        };
        assert_eq!(s.fluid_capacity(CELL_MB), 64 * BUCKET);
    }

    #[test]
    fn a_fluid_limit_counts_buckets_and_may_destroy_the_surplus() {
        // Rule FL5 with S5.
        let mut s = store(4);
        s.limits.insert(
            fluid_key(WATER),
            Limit {
                max: 3,
                destroy_surplus: false,
            },
        );
        assert_eq!(s.fluid_room(WATER, CELL_MB), 3 * BUCKET);
        assert_eq!(s.put_fluid(WATER, 5 * BUCKET, CELL_MB), 3 * BUCKET);
        s.limits.get_mut(&fluid_key(WATER)).unwrap().destroy_surplus = true;
        assert_eq!(s.fluid_room(WATER, CELL_MB), u64::MAX);
        assert_eq!(s.put_fluid(WATER, 5 * BUCKET, CELL_MB), 5 * BUCKET);
        assert_eq!(s.fluid(WATER), 3 * BUCKET);
    }

    #[test]
    fn the_fluid_part_keeps_the_fluids_and_their_limits_only() {
        let mut s = store(2);
        s.put("minecraft:dirt", 5, &vanilla);
        s.put_fluid(WATER, BUCKET, CELL_MB);
        let limit = Limit {
            max: 9,
            destroy_surplus: false,
        };
        s.limits.insert("minecraft:dirt".into(), limit);
        s.limits.insert(fluid_key(WATER), limit);
        let f = s.fluid_part();
        assert!(f.items.is_empty());
        assert_eq!(f.fluid(WATER), BUCKET);
        assert_eq!(f.limits.len(), 1);
        assert_eq!(f.fluid_capacity(CELL_MB), s.fluid_capacity(CELL_MB));
    }

    #[test]
    fn joined_storages_pour_their_fluids_together() {
        let mut a = store(1);
        a.put_fluid(WATER, 2 * BUCKET, CELL_MB);
        let mut b = store(1);
        b.put_fluid(WATER, 3 * BUCKET, CELL_MB);
        a.absorb(b);
        assert_eq!(a.fluid(WATER), 5 * BUCKET);
    }
}
