//! Tanks (guide `tank`, spec 4.3): a tank holds one fluid, 16 buckets a block, and tanks that
//! touch are one tank with their contents shared. The bundle keeps each block's share in its block
//! entity; here the blocks are joined into groups, and a group's fluid is laid out again from the
//! bottom up after every change, as a fluid settles. A tank that holds another fluid than the
//! tanks it touches stays a tank of its own (a group cannot hold two fluids) and shows why. A
//! tank broken loses its share, as a bucket of water poured out would be lost.
//!
//! 0.21 (spec 42, FT1–FT6; the owner: «вода, молоко и раствор периодически занимают место в одном
//! и том же баке, друг друга заменяя»): a tank group is kept for one fluid, its pin. It takes the
//! pin with the first fluid it gets and keeps it when it empties, so no other fluid fills it; a
//! sneaking wrench on the empty tank frees it. A tank that holds a fluid and has no pin yet (a
//! world from before 0.21) is kept for what it holds.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::geom::{Face, Pos, step};
use crate::net::fluids::FluidKey;

/// One tank block as its block entity keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TankBlock {
    pub fluid: Option<FluidKey>,
    pub amount: u64,
    /// 0.21 (FT1): the fluid its group is kept for; none while free.
    pub pin: Option<FluidKey>,
}

/// Tanks that touch, as one tank.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TankGroup {
    /// Its blocks, the lowest first.
    pub blocks: Vec<Pos>,
    pub fluid: Option<FluidKey>,
    pub amount: u64,
    pub capacity: u64,
    /// A tank with another fluid touches this one: they stay apart.
    pub mixed: bool,
    /// 0.21 (FT1): the fluid the tank is kept for, even empty; none while it is free.
    pub pin: Option<FluidKey>,
}

impl TankGroup {
    /// The block that stands for the whole tank on a fluid net: its lowest.
    pub fn anchor(&self) -> Pos {
        self.blocks.first().copied().unwrap_or_default()
    }

    /// 0.21 (FT1): whether the tank is kept for a fluid other than `fluid`.
    pub fn kept_for_other(&self, fluid: &str) -> bool {
        self.pin.as_deref().is_some_and(|p| p != fluid)
    }

    /// How much of `fluid` the tank takes now: none of a fluid other than its own, and none of a
    /// fluid other than the one it is kept for, empty or not (FT1).
    pub fn room_for(&self, fluid: &str) -> u64 {
        if self.kept_for_other(fluid) {
            return 0;
        }
        match &self.fluid {
            Some(f) if f != fluid && self.amount > 0 => 0,
            _ => self.capacity.saturating_sub(self.amount),
        }
    }

    /// Puts up to `amount` of `fluid` in; returns how much went in. A free tank is kept for
    /// `fluid` from then on (FT1).
    pub fn put(&mut self, fluid: &str, amount: u64) -> u64 {
        let n = amount.min(self.room_for(fluid));
        if n > 0 {
            self.fluid = Some(fluid.to_owned());
            self.pin = Some(fluid.to_owned());
            self.amount += n;
        }
        n
    }

    /// 0.21 (FT4): the empty tank freed for any fluid; false when it holds some (what it holds
    /// keeps it) or is free already.
    pub fn free(&mut self) -> bool {
        if self.amount > 0 || self.pin.is_none() {
            return false;
        }
        self.pin = None;
        true
    }

    /// Takes up to `amount` of `fluid` out; returns how much came out.
    pub fn take(&mut self, fluid: &str, amount: u64) -> u64 {
        if self.fluid.as_deref() != Some(fluid) {
            return 0;
        }
        let n = amount.min(self.amount);
        self.amount -= n;
        if self.amount == 0 {
            self.fluid = None;
        }
        n
    }

    /// Each block's share, filled from the bottom up, `per_block` each. What is past the
    /// capacity (a smaller tank size in the numbers) stays in the top block: nothing is lost.
    pub fn shares(&self, per_block: u64) -> Vec<(Pos, TankBlock)> {
        let mut left = self.amount;
        let last = self.blocks.len().saturating_sub(1);
        self.blocks
            .iter()
            .enumerate()
            .map(|(i, &pos)| {
                let n = if i == last { left } else { left.min(per_block) };
                left -= n;
                let fluid = (n > 0).then(|| self.fluid.clone()).flatten();
                (pos, TankBlock { fluid, amount: n, pin: self.pin.clone() })
            })
            .collect()
    }
}

/// The tanks of `tanks` joined into groups of `per_block` a block.
pub fn groups(tanks: &BTreeMap<Pos, TankBlock>, per_block: u64) -> Vec<TankGroup> {
    let mut out = Vec::new();
    for part in components(tanks.keys().copied().collect()) {
        let mut by_fluid: BTreeMap<&str, u64> = BTreeMap::new();
        for pos in &part {
            let t = &tanks[pos];
            if let Some(f) = t.fluid.as_deref().filter(|_| t.amount > 0) {
                *by_fluid.entry(f).or_insert(0) += t.amount;
            }
        }
        // The fluid most of it holds; ties go to the first by name, so the choice is stable.
        let fluid = by_fluid
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(f, _)| (*f).to_owned());
        let foreign: Vec<Pos> = part
            .iter()
            .copied()
            .filter(|p| {
                let t = &tanks[p];
                t.amount > 0 && t.fluid.is_some() && t.fluid != fluid
            })
            .collect();
        let mixed = !foreign.is_empty();
        let main: BTreeSet<Pos> = part
            .iter()
            .copied()
            .filter(|p| !foreign.contains(p))
            .collect();
        // Without the other fluid's tanks the rest may fall apart: each piece is a tank.
        for piece in components(main) {
            let piece: Vec<Pos> = piece.into_iter().collect();
            out.push(group_of(&piece, fluid.clone(), tanks, per_block, mixed));
        }
        for p in foreign {
            out.push(group_of(
                &[p],
                tanks[&p].fluid.clone(),
                tanks,
                per_block,
                true,
            ));
        }
    }
    out
}

fn group_of(
    blocks: &[Pos],
    fluid: Option<FluidKey>,
    tanks: &BTreeMap<Pos, TankBlock>,
    per_block: u64,
    mixed: bool,
) -> TankGroup {
    let mut blocks = blocks.to_vec();
    blocks.sort_by_key(|p| (p[1], p[0], p[2]));
    let amount = blocks
        .iter()
        .map(|p| &tanks[p])
        .filter(|t| t.fluid == fluid)
        .map(|t| t.amount)
        .sum();
    let fluid = fluid.filter(|_| amount > 0);
    let pin = fluid.clone().or_else(|| pin_of(&blocks, tanks));
    TankGroup {
        capacity: per_block * blocks.len() as u64,
        blocks,
        fluid,
        amount,
        mixed,
        pin,
    }
}

/// 0.21 (FT2, FT6): the pin of an empty group: the one most of its blocks keep (ties to the first
/// by name), none when none keeps one. A group that holds a fluid is kept for it instead.
fn pin_of(blocks: &[Pos], tanks: &BTreeMap<Pos, TankBlock>) -> Option<FluidKey> {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for t in blocks.iter().map(|p| &tanks[p]) {
        if let Some(p) = t.pin.as_deref().filter(|p| !p.is_empty()) {
            *count.entry(p).or_insert(0) += 1;
        }
    }
    count
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(p, _)| (*p).to_owned())
}

fn components(mut left: BTreeSet<Pos>) -> Vec<BTreeSet<Pos>> {
    let mut parts = Vec::new();
    while let Some(start) = left.pop_first() {
        let mut part = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(at) = queue.pop_front() {
            for face in Face::ALL {
                let next = step(at, face);
                if left.remove(&next) {
                    part.insert(next);
                    queue.push_back(next);
                }
            }
        }
        parts.push(part);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    const WATER: &str = "minecraft:water";
    const LAVA: &str = "minecraft:lava";
    const MILK: &str = "rustcraft:milk";
    const PER: u64 = 16_000;

    fn tank(fluid: Option<&str>, amount: u64) -> TankBlock {
        // A tank that holds a fluid is kept for it, as `shares` writes it back.
        TankBlock {
            fluid: fluid.map(str::to_owned),
            amount,
            pin: fluid.filter(|_| amount > 0).map(str::to_owned),
        }
    }

    fn kept(pin: &str) -> TankBlock {
        TankBlock { pin: Some(pin.to_owned()), ..TankBlock::default() }
    }

    #[test]
    fn touching_tanks_are_one_tank_with_their_contents_shared() {
        // Spec 4.3: «соседние баки сливаются в один».
        let tanks = BTreeMap::from([
            ([0, 1, 0], tank(Some(WATER), 4_000)),
            ([0, 0, 0], tank(None, 0)),
            ([5, 0, 0], tank(Some(LAVA), 1_000)),
        ]);
        let g = groups(&tanks, PER);
        assert_eq!(g.len(), 2);
        let water = g.iter().find(|g| g.blocks.len() == 2).unwrap();
        assert_eq!(
            (water.fluid.as_deref(), water.amount, water.capacity),
            (Some(WATER), 4_000, 32_000)
        );
        assert_eq!(water.anchor(), [0, 0, 0]);
        assert!(!water.mixed);
    }

    #[test]
    fn the_shared_fluid_settles_from_the_bottom_up() {
        let mut g = TankGroup {
            blocks: vec![[0, 0, 0], [0, 1, 0]],
            capacity: 2 * PER,
            ..TankGroup::default()
        };
        assert_eq!(g.put(WATER, 20_000), 20_000);
        let s = g.shares(PER);
        assert_eq!(s[0].1, tank(Some(WATER), 16_000));
        assert_eq!(s[1].1, tank(Some(WATER), 4_000));
        assert_eq!(g.room_for(LAVA), 0);
        assert_eq!(g.room_for(WATER), 12_000);
        assert_eq!(g.take(WATER, 25_000), 20_000);
        assert_eq!(g.fluid, None);
        // FT1: emptied, it is still kept for water; freed, it takes lava.
        assert_eq!(g.room_for(LAVA), 0);
        assert_eq!(g.room_for(WATER), 32_000);
        assert!(g.free());
        assert_eq!(g.room_for(LAVA), 32_000);
    }

    #[test]
    fn a_tank_with_another_fluid_stays_apart() {
        let tanks = BTreeMap::from([
            ([0, 0, 0], tank(Some(WATER), 8_000)),
            ([1, 0, 0], tank(Some(LAVA), 2_000)),
            ([2, 0, 0], tank(None, 0)),
        ]);
        let g = groups(&tanks, PER);
        // The lava tank parts the water tank from the empty one: three tanks.
        assert_eq!(g.len(), 3);
        assert!(g.iter().all(|g| g.mixed));
        let water = g
            .iter()
            .find(|g| g.fluid.as_deref() == Some(WATER))
            .unwrap();
        assert_eq!((water.blocks.len(), water.amount), (1, 8_000));
        let lava = g.iter().find(|g| g.fluid.as_deref() == Some(LAVA)).unwrap();
        assert_eq!(lava.blocks, vec![[1, 0, 0]]);
    }

    #[test]
    fn nothing_past_the_capacity_is_lost() {
        let g = TankGroup {
            blocks: vec![[0, 0, 0]],
            fluid: Some(WATER.into()),
            amount: 20_000,
            capacity: PER,
            mixed: false,
            pin: Some(WATER.into()),
        };
        assert_eq!(g.shares(PER)[0].1.amount, 20_000);
    }

    // ---- 0.21 (spec 42): one fluid a tank group, kept when it empties ----

    #[test]
    fn an_emptied_tank_keeps_its_fluid_and_refuses_another() {
        // FT1: the tank of water drained to nothing still takes no milk.
        let mut g = groups(&BTreeMap::from([([0, 0, 0], tank(Some(WATER), 3_000))]), PER).remove(0);
        assert_eq!(g.pin.as_deref(), Some(WATER));
        assert_eq!(g.take(WATER, 3_000), 3_000);
        assert_eq!(g.put(MILK, 1_000), 0);
        assert_eq!(g.shares(PER)[0].1, kept(WATER));
        // Read back from its block entity it is the same tank: empty, kept for water.
        let again = groups(&BTreeMap::from([([0, 0, 0], kept(WATER))]), PER).remove(0);
        assert_eq!((again.fluid.clone(), again.amount, again.pin.as_deref()), (None, 0, Some(WATER)));
        assert_eq!(again.room_for(MILK), 0);
    }

    #[test]
    fn a_free_tank_is_kept_for_the_first_fluid_it_gets() {
        let mut g = groups(&BTreeMap::from([([0, 0, 0], tank(None, 0))]), PER).remove(0);
        assert_eq!(g.pin, None);
        assert_eq!(g.put(MILK, 500), 500);
        assert_eq!(g.pin.as_deref(), Some(MILK));
        assert_eq!(g.put(WATER, 500), 0);
    }

    #[test]
    fn a_tank_that_holds_a_fluid_from_an_old_world_is_kept_for_it() {
        // FT6: a 0.20 tank has no pin in its block entity; what it holds becomes its pin.
        let old = TankBlock { fluid: Some(WATER.into()), amount: 2_000, pin: None };
        let g = groups(&BTreeMap::from([([0, 0, 0], old), ([0, 1, 0], TankBlock::default())]), PER);
        assert_eq!(g[0].pin.as_deref(), Some(WATER));
        assert!(g[0].shares(PER).iter().all(|(_, b)| b.pin.as_deref() == Some(WATER)));
        // An empty 0.20 tank starts free.
        let g = groups(&BTreeMap::from([([0, 0, 0], TankBlock::default())]), PER);
        assert_eq!(g[0].pin, None);
    }

    #[test]
    fn joined_empty_tanks_take_the_pin_most_of_them_keep() {
        // FT2: two blocks kept for milk and one for water, all empty, joined: one tank for milk.
        let tanks = BTreeMap::from([
            ([0, 0, 0], kept(MILK)),
            ([1, 0, 0], kept(MILK)),
            ([2, 0, 0], kept(WATER)),
        ]);
        let g = groups(&tanks, PER);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].pin.as_deref(), Some(MILK));
        // A fluid it holds wins over the blocks' pins.
        let tanks = BTreeMap::from([([0, 0, 0], kept(MILK)), ([1, 0, 0], tank(Some(WATER), 100))]);
        assert_eq!(groups(&tanks, PER)[0].pin.as_deref(), Some(WATER));
    }

    #[test]
    fn freeing_works_only_on_an_empty_tank() {
        // FT4: what a tank holds keeps it; an empty one frees, a free one has nothing to free.
        let mut g = groups(&BTreeMap::from([([0, 0, 0], tank(Some(WATER), 1_000))]), PER).remove(0);
        assert!(!g.free());
        g.take(WATER, 1_000);
        assert!(g.free());
        assert_eq!(g.shares(PER)[0].1, TankBlock::default());
        assert!(!g.free());
    }
}
