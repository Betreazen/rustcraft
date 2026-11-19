//! Moving fluids along one fluid channel net. One channel carries several fluids at once, each to
//! where it is wanted and in any direction, every fluid in its own lane with the tier's rate (M13).
//!
//! Round 31 (spec 29, FL1–FL3): the channel carries any number of fluids, and the net is smart.
//! Each block behind a joint has a role (`Endpoint`): a source gives to the consumers that want
//! its fluid first, then to the rest; a store (a tank, the storage) on its default joint gives to
//! consumers only, so fluid never shuttles between stores. A joint the player set «из блока»
//! gives to every receiver, as before. `kinds` caps the fluids at once for a caller that wants a
//! cap (the bundle passes none since round 31); a fluid over the cap shows «В канале нет места
//! для новой жидкости» on its source. Amounts are in millibuckets.

use std::collections::{BTreeMap, BTreeSet};

use crate::geom::Pos;
use crate::net::items::{Joint, Mode};

/// A fluid as the game names it.
pub type FluidKey = String;

/// Millibuckets in a bucket.
pub const BUCKET: u64 = 1000;

/// Round 31 (FL1): no cap on the fluids one channel carries at once.
pub const ANY_KINDS: usize = usize::MAX;

/// How the block behind a joint works on a smart line (round 31, FL2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Endpoint {
    /// Gives and takes by its joint's mode alone: a pump, a machine's catalyst tank, the trash.
    #[default]
    Plain,
    /// Keeps fluid and gives it on: a tank, the storage. On its default joint (`Mode::Both`) it
    /// gives to consumers only (FL3).
    Store,
    /// Takes the fluid it works on, from stores too: a mixer's water, a crystal structure's
    /// solution, a lava generator's lava.
    Consumer,
}

/// The tanks and machines behind the joints of a fluid net.
pub trait FluidInventories {
    /// What the block offers, in mB, by fluid.
    fn offered(&self, block: Pos) -> Vec<(FluidKey, u64)>;
    fn room_for(&self, block: Pos, fluid: &str) -> u64;
    fn take(&mut self, block: Pos, fluid: &str, amount: u64) -> u64;
    fn put(&mut self, block: Pos, fluid: &str, amount: u64) -> u64;
    /// The block's role on a smart line (FL2); plain unless the bundle says otherwise.
    fn endpoint(&self, _block: Pos) -> Endpoint {
        Endpoint::Plain
    }
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

/// One tick of a fluid net that carries `buckets_per_second` in each fluid's lane and at most
/// `kinds` fluids at once.
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
    let receivers = receivers_in_order(joints, state, &*inv);
    let mut places = state.carried.clone();
    // Each lane fills once a tick, however many sources offer its fluid.
    let mut refilled = BTreeSet::new();
    let mut flowed = BTreeSet::new();
    for &s in &sources {
        let src = &joints[s];
        // FL3: a store on its default joint gives to consumers only.
        let consumers_only = src.mode == Mode::Both && inv.endpoint(src.block) == Endpoint::Store;
        for (fluid, amount) in inv.offered(src.block) {
            if amount == 0 || !src.filter.passes(&fluid) {
                continue;
            }
            // A fluid takes a place in the channel only when it moves.
            if !places.contains(&fluid) && places.len() >= kinds {
                result.no_room.push(NoRoomForFluid { joint: s, fluid });
                continue;
            }
            let lane = state.budgets.entry(fluid.clone()).or_insert(0.0);
            if refilled.insert(fluid.clone()) {
                *lane = (*lane + per_second / ticks_per_second).min(lane_cap);
            }
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
                if consumers_only && inv.endpoint(dst.block) != Endpoint::Consumer {
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
                    left -= put;
                    available -= put;
                    moved += put;
                }
            }
            if moved > 0 {
                places.insert(fluid.clone());
                flowed.insert(fluid.clone());
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

/// The receivers in the order they are served this tick: highest priority first; among equal
/// priorities the consumers first (FL3), then the rest from a different one each tick (M8).
fn receivers_in_order(
    joints: &[Joint],
    state: &mut FluidNetState,
    inv: &dyn FluidInventories,
) -> Vec<usize> {
    let mut receivers = first_per_block(joints, Mode::Into);
    if !receivers.is_empty() {
        let shift = state.next_receiver % receivers.len();
        receivers.rotate_left(shift);
        state.next_receiver = state.next_receiver.wrapping_add(1);
    }
    receivers.sort_by_key(|&r| {
        (
            std::cmp::Reverse(joints[r].priority),
            inv.endpoint(joints[r].block) != Endpoint::Consumer,
        )
    });
    receivers
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
        roles: HashMap<Pos, Endpoint>,
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
        fn endpoint(&self, b: Pos) -> Endpoint {
            self.roles.get(&b).copied().unwrap_or_default()
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
            auto: false,
            refiner: false,
            store: false,
            loader: false,
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

    // ---- Round 31: the smart line (FL1–FL3) ----

    const STORE_A: Pos = [3, 0, 0];
    const STORE_B: Pos = [4, 0, 0];
    const GENERATOR: Pos = [5, 0, 0];

    fn smart_line() -> (Vec<Joint>, Tanks) {
        let mut t = Tanks::default();
        t.roles.insert(STORE_A, Endpoint::Store);
        t.roles.insert(STORE_B, Endpoint::Store);
        t.roles.insert(MIXER, Endpoint::Consumer);
        t.roles.insert(GENERATOR, Endpoint::Consumer);
        let joints = vec![
            joint(3, Mode::Both),
            joint(4, Mode::Both),
            joint(2, Mode::Both),
            joint(5, Mode::Into),
        ];
        (joints, t)
    }

    #[test]
    fn a_store_on_its_default_joint_gives_to_consumers_only() {
        // Rule FL3: the tank of water feeds the mixer, never the other tank.
        let (joints, mut t) = smart_line();
        t.holds(STORE_A, "minecraft:water", 16 * BUCKET);
        t.takes(STORE_B, "minecraft:water", 16 * BUCKET);
        t.takes(MIXER, "minecraft:water", 2 * BUCKET);
        let mut s = FluidNetState::default();
        for _ in 0..10 {
            tick(&joints, 100.0, ANY_KINDS, 20.0, &mut s, &mut t);
        }
        assert_eq!(t.got(MIXER, "minecraft:water"), 2 * BUCKET);
        assert_eq!(t.got(STORE_B, "minecraft:water"), 0);
    }

    #[test]
    fn a_source_fills_its_consumers_before_the_stores() {
        // Rule FL3: the mixer's solution goes to the structure first, the rest into a store.
        let (joints, mut t) = smart_line();
        t.holds(MIXER, "rustcraft:solution", 3 * BUCKET);
        t.takes(GENERATOR, "rustcraft:solution", 1000);
        t.takes(STORE_A, "rustcraft:solution", 16 * BUCKET);
        t.takes(STORE_B, "rustcraft:solution", 16 * BUCKET);
        let mut s = FluidNetState::default();
        // Every rotation of the receivers: the consumer is served first each time.
        for _ in 0..4 {
            tick(&joints, 0.5, ANY_KINDS, 1.0, &mut s, &mut t);
        }
        assert_eq!(t.got(GENERATOR, "rustcraft:solution"), 1000);
        assert_eq!(
            t.got(STORE_A, "rustcraft:solution") + t.got(STORE_B, "rustcraft:solution"),
            1000
        );
    }

    #[test]
    fn a_store_set_to_give_fills_the_other_stores() {
        // Rule FL3: «из блока» gives to every receiver, as before round 31.
        let (mut joints, mut t) = smart_line();
        joints[0].mode = Mode::From;
        t.holds(STORE_A, "minecraft:water", 4 * BUCKET);
        t.takes(STORE_B, "minecraft:water", 16 * BUCKET);
        let mut s = FluidNetState::default();
        tick(&joints, 100.0, ANY_KINDS, 20.0, &mut s, &mut t);
        assert_eq!(t.got(STORE_B, "minecraft:water"), 4 * BUCKET);
    }

    #[test]
    fn two_fluids_ride_one_tier_one_channel_past_each_other() {
        // Rule FL1: water east to the mixer, lava west to the generator, one channel of tier 1.
        let (joints, mut t) = smart_line();
        t.holds(STORE_A, "minecraft:lava", 8 * BUCKET);
        t.holds(STORE_B, "minecraft:water", 8 * BUCKET);
        t.takes(GENERATOR, "minecraft:lava", 8 * BUCKET);
        t.takes(MIXER, "minecraft:water", 8 * BUCKET);
        let mut s = FluidNetState::default();
        let mut refused = 0;
        for _ in 0..20 {
            refused += tick(&joints, 1.0, ANY_KINDS, 20.0, &mut s, &mut t).no_room.len();
        }
        assert_eq!(refused, 0);
        // Each in its own lane at the tier's rate: a bucket a second each.
        assert_eq!(t.got(GENERATOR, "minecraft:lava"), BUCKET);
        assert_eq!(t.got(MIXER, "minecraft:water"), BUCKET);
    }

    #[test]
    fn a_lane_fills_once_a_tick_however_many_sources_offer_its_fluid() {
        let mut t = Tanks::default();
        t.holds(WATER_TANK, "minecraft:water", 16 * BUCKET);
        t.holds(LAVA_TANK, "minecraft:water", 16 * BUCKET);
        t.takes(MIXER, "minecraft:water", 16 * BUCKET);
        let joints = [joint(0, Mode::From), joint(1, Mode::From), joint(2, Mode::Into)];
        let mut s = FluidNetState::default();
        for _ in 0..20 {
            tick(&joints, 1.0, ANY_KINDS, 20.0, &mut s, &mut t);
        }
        assert_eq!(t.got(MIXER, "minecraft:water"), BUCKET);
    }
}
