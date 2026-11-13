//! Moving fluids along one fluid channel net. One channel carries several fluids at once, each to
//! where it is wanted and in any direction; how many kinds at once is set by the channel's tier
//! (M13: 1 / 2 / 4 / 8). A fluid that finds no free place shows «В канале нет места для новой
//! жидкости» on its source. Every fluid in the channel has its own lane with the tier's rate.
//! Amounts are in millibuckets.

use std::collections::{BTreeMap, BTreeSet};

use crate::geom::Pos;
use crate::net::items::{Joint, Mode};

/// A fluid as the game names it.
pub type FluidKey = String;

/// Millibuckets in a bucket.
pub const BUCKET: u64 = 1000;

/// The tanks and machines behind the joints of a fluid net.
pub trait FluidInventories {
    /// What the block offers, in mB, by fluid.
    fn offered(&self, block: Pos) -> Vec<(FluidKey, u64)>;
    fn room_for(&self, block: Pos, fluid: &str) -> u64;
    fn take(&mut self, block: Pos, fluid: &str, amount: u64) -> u64;
    fn put(&mut self, block: Pos, fluid: &str, amount: u64) -> u64;
}

/// What a fluid net remembers between ticks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FluidNetState {
    /// Throughput not yet spent, in mB, for each fluid in the channel.
    pub budgets: BTreeMap<FluidKey, f64>,
    /// The fluids the channel carried last tick: they keep their places.
    pub carried: BTreeSet<FluidKey>,
    pub next_receiver: usize,
}

/// A source joint whose fluid found no free place in the channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoRoomForFluid {
    pub joint: usize,
    pub fluid: FluidKey,
}

/// What one tick of a fluid net did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FluidTick {
    /// (from, to, fluid, mB).
    pub moved: Vec<(Pos, Pos, FluidKey, u64)>,
    pub no_room: Vec<NoRoomForFluid>,
}

/// One tick of a fluid net that carries `buckets_per_second` and at most `kinds` fluids at once.
pub fn tick(
    joints: &[Joint],
    buckets_per_second: f64,
    kinds: usize,
    ticks_per_second: f64,
    state: &mut FluidNetState,
    inv: &mut dyn FluidInventories,
) -> FluidTick {
    let mut result = FluidTick::default();
    let per_second = buckets_per_second * BUCKET as f64;
    let lane_cap = per_second.max(BUCKET as f64);
    let sources = first_per_block(joints, Mode::From);
    let mut receivers = first_per_block(joints, Mode::Into);
    // Highest priority first; equal priorities start at a different receiver each tick.
    if !receivers.is_empty() {
        let shift = state.next_receiver % receivers.len();
        receivers.rotate_left(shift);
        state.next_receiver = state.next_receiver.wrapping_add(1);
    }
    receivers.sort_by_key(|&r| std::cmp::Reverse(joints[r].priority));
    let mut places = state.carried.clone();
    let mut flowed = BTreeSet::new();
    for &s in &sources {
        let src = &joints[s];
        for (fluid, amount) in inv.offered(src.block) {
            if amount == 0 || !src.filter.passes(&fluid) {
                continue;
            }
            if !places.contains(&fluid) {
                if places.len() >= kinds {
                    result.no_room.push(NoRoomForFluid { joint: s, fluid });
                    continue;
                }
                places.insert(fluid.clone());
            }
            // Every fluid has its own lane with the tier's rate.
            let lane = state.budgets.entry(fluid.clone()).or_insert(0.0);
            *lane = (*lane + per_second / ticks_per_second).min(lane_cap);
            let mut available = (*lane + 1e-9).floor() as u64;
            let mut moved = 0u64;
            let mut left = amount;
            for &r in &receivers {
                let dst = &joints[r];
                if left == 0 || available == 0 {
                    break;
                }
                if dst.block == src.block || !dst.filter.passes(&fluid) {
                    continue;
                }
                let want = left.min(available).min(inv.room_for(dst.block, &fluid));
                if want == 0 {
                    continue;
                }
                let taken = inv.take(src.block, &fluid, want);
                let put = inv.put(dst.block, &fluid, taken);
                if put < taken {
                    inv.put(src.block, &fluid, taken - put);
                }
                if put > 0 {
                    result
                        .moved
                        .push((src.block, dst.block, fluid.clone(), put));
                    flowed.insert(fluid.clone());
                    left -= put;
                    available -= put;
                    moved += put;
                }
            }
            if let Some(lane) = state.budgets.get_mut(&fluid) {
                *lane -= moved as f64;
            }
        }
    }
    // A fluid keeps its place, and its lane, only while it flows.
    state.budgets.retain(|f, _| flowed.contains(f));
    state.carried = flowed;
    result
}

/// The first joint on each block that passes this direction: `From` gives, `Into` takes.
fn first_per_block(joints: &[Joint], direction: Mode) -> Vec<usize> {
    let mut blocks = BTreeSet::new();
    (0..joints.len())
        .filter(|&j| {
            let mode = joints[j].mode;
            let ok = match direction {
                Mode::From => matches!(mode, Mode::From | Mode::Both),
                _ => matches!(mode, Mode::Into | Mode::Both),
            };
            ok && blocks.insert(joints[j].block)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Face;
    use crate::net::items::Filter;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Tanks {
        has: HashMap<Pos, Vec<(FluidKey, u64)>>,
        room: HashMap<Pos, HashMap<FluidKey, u64>>,
        got: HashMap<Pos, HashMap<FluidKey, u64>>,
    }

    impl Tanks {
        fn holds(&mut self, b: Pos, f: &str, mb: u64) {
            self.has.entry(b).or_default().push((f.into(), mb));
        }
        fn takes(&mut self, b: Pos, f: &str, mb: u64) {
            self.room.entry(b).or_default().insert(f.into(), mb);
        }
        fn got(&self, b: Pos, f: &str) -> u64 {
            self.got
                .get(&b)
                .and_then(|m| m.get(f))
                .copied()
                .unwrap_or(0)
        }
    }

    impl FluidInventories for Tanks {
        fn offered(&self, b: Pos) -> Vec<(FluidKey, u64)> {
            self.has.get(&b).cloned().unwrap_or_default()
        }
        fn room_for(&self, b: Pos, f: &str) -> u64 {
            self.room
                .get(&b)
                .and_then(|m| m.get(f))
                .copied()
                .unwrap_or(0)
        }
        fn take(&mut self, b: Pos, f: &str, amount: u64) -> u64 {
            let mut left = amount;
            for (k, v) in self.has.entry(b).or_default().iter_mut() {
                if k == f {
                    let t = (*v).min(left);
                    *v -= t;
                    left -= t;
                }
            }
            amount - left
        }
        fn put(&mut self, b: Pos, f: &str, amount: u64) -> u64 {
            let room = self.room.entry(b).or_default().entry(f.into()).or_insert(0);
            let n = (*room).min(amount);
            *room -= n;
            *self.got.entry(b).or_default().entry(f.into()).or_insert(0) += n;
            n
        }
    }

    fn joint(x: i32, mode: Mode) -> Joint {
        Joint {
            housing: [x, 1, 0],
            face: Face::YNeg,
            block: [x, 0, 0],
            mode,
            filter: Filter::default(),
            priority: 0,
        }
    }

    const WATER_TANK: Pos = [0, 0, 0];
    const LAVA_TANK: Pos = [1, 0, 0];
    const MIXER: Pos = [2, 0, 0];

    fn two_fluids() -> (Vec<Joint>, Tanks) {
        let mut t = Tanks::default();
        t.holds(WATER_TANK, "minecraft:water", 16 * BUCKET);
        t.holds(LAVA_TANK, "minecraft:lava", 16 * BUCKET);
        t.takes(MIXER, "minecraft:water", 8 * BUCKET);
        t.takes(MIXER, "minecraft:lava", 8 * BUCKET);
        (
            vec![
                joint(0, Mode::From),
                joint(1, Mode::From),
                joint(2, Mode::Into),
            ],
            t,
        )
    }

    #[test]
    fn a_tier_one_channel_carries_one_fluid_and_names_the_second() {
        // Rule M13: tier 1 carries one fluid.
        let (joints, mut t) = two_fluids();
        let mut s = FluidNetState::default();
        let r = tick(&joints, 100.0, 1, 20.0, &mut s, &mut t);
        assert!(t.got(MIXER, "minecraft:water") > 0);
        assert_eq!(t.got(MIXER, "minecraft:lava"), 0);
        assert_eq!(
            r.no_room,
            vec![NoRoomForFluid {
                joint: 1,
                fluid: "minecraft:lava".into()
            }]
        );
    }

    #[test]
    fn a_tier_two_channel_carries_both() {
        let (joints, mut t) = two_fluids();
        let mut s = FluidNetState::default();
        let r = tick(&joints, 100.0, 2, 20.0, &mut s, &mut t);
        assert!(t.got(MIXER, "minecraft:water") > 0);
        assert!(t.got(MIXER, "minecraft:lava") > 0);
        assert!(r.no_room.is_empty());
    }

    #[test]
    fn the_channel_moves_no_more_than_its_rate() {
        // A tier-1 fluid channel carries 1 bucket a second.
        let mut t = Tanks::default();
        t.holds(WATER_TANK, "minecraft:water", 16 * BUCKET);
        t.takes(MIXER, "minecraft:water", 16 * BUCKET);
        let joints = [joint(0, Mode::From), joint(2, Mode::Into)];
        let mut s = FluidNetState::default();
        for _ in 0..20 {
            tick(&joints, 1.0, 1, 20.0, &mut s, &mut t);
        }
        assert_eq!(t.got(MIXER, "minecraft:water"), BUCKET);
    }

    #[test]
    fn a_fluid_goes_only_where_there_is_room() {
        let mut t = Tanks::default();
        t.holds(WATER_TANK, "minecraft:water", 16 * BUCKET);
        t.takes(MIXER, "minecraft:water", 300);
        let joints = [joint(0, Mode::From), joint(2, Mode::Into)];
        let mut s = FluidNetState::default();
        tick(&joints, 100.0, 1, 20.0, &mut s, &mut t);
        assert_eq!(t.got(MIXER, "minecraft:water"), 300);
        assert_eq!(t.offered(WATER_TANK)[0].1, 16 * BUCKET - 300);
    }
}
