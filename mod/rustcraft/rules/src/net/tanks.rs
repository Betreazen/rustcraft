//! Tanks (guide `tank`, spec 4.3): a tank holds one fluid, 16 buckets a block, and tanks that
//! touch are one tank with their contents shared. The bundle keeps each block's share in its block
//! entity; here the blocks are joined into groups, and a group's fluid is laid out again from the
//! bottom up after every change, as a fluid settles. A tank that holds another fluid than the
//! tanks it touches stays a tank of its own (a group cannot hold two fluids) and shows why. A
//! tank broken loses its share, as a bucket of water poured out would be lost.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::geom::{Face, Pos, step};
use crate::net::fluids::FluidKey;

/// One tank block as its block entity keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TankBlock {
    pub fluid: Option<FluidKey>,
    pub amount: u64,
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
}

impl TankGroup {
    /// The block that stands for the whole tank on a fluid net: its lowest.
    pub fn anchor(&self) -> Pos {
        self.blocks.first().copied().unwrap_or_default()
    }

    /// How much of `fluid` the tank takes now: none of a fluid other than its own.
    pub fn room_for(&self, fluid: &str) -> u64 {
        match &self.fluid {
            Some(f) if f != fluid && self.amount > 0 => 0,
            _ => self.capacity.saturating_sub(self.amount),
        }
    }

    /// Puts up to `amount` of `fluid` in; returns how much went in.
    pub fn put(&mut self, fluid: &str, amount: u64) -> u64 {
        let n = amount.min(self.room_for(fluid));
        if n > 0 {
            self.fluid = Some(fluid.to_owned());
            self.amount += n;
        }
        n
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
                (pos, TankBlock { fluid, amount: n })
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
    TankGroup {
        capacity: per_block * blocks.len() as u64,
        blocks,
        fluid: fluid.filter(|_| amount > 0),
        amount,
        mixed,
    }
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
    const PER: u64 = 16_000;

    fn tank(fluid: Option<&str>, amount: u64) -> TankBlock {
        TankBlock {
            fluid: fluid.map(str::to_owned),
            amount,
        }
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
        };
        assert_eq!(g.shares(PER)[0].1.amount, 20_000);
    }
}
