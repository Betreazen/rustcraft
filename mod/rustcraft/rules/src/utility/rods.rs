//! 0.21 (spec 38, FR1–FR6): fishing stations take their rods from the storage and order new ones.
//!
//! The owner: «рыболовные станции сами должны отправлять заказы на новые удочки… чинить их
//! достаточно дорого, проще крафтить новые… научи рыболовные станции забирать их из склада». A
//! station with no rod, or with a worn one (`farm::rod_worn`), takes the least worn whole rod of
//! the storage on its item net; the worn rod goes into that storage. With no whole rod there, the
//! storage orders rods: one order at a time, for the stations waiting on it, gathered for a few
//! seconds first, so fifty stations over one pond make one order, not fifty a second.

use std::collections::BTreeMap;

use super::farm::rod_worn;
use crate::geom::Pos;

/// The rod a station takes and the storage orders.
pub const ROD: &str = "minecraft:fishing_rod";
/// FR4: the most rods one order asks for.
pub const ORDER_CAP: u32 = 16;
/// FR4: ticks a storage gathers its waiting stations before it orders (a station steps every 20,
/// 40 in the economy mode).
pub const GATHER_TICKS: u64 = 60;
/// FR4: a station not heard waiting for this long (broken, or given a rod by hand) counts no more.
pub const STALE_TICKS: u64 = 200;
/// FR5: ticks a storage waits after a refused order before it asks again.
pub const RETRY_TICKS: u64 = 600;

/// FR1: of the rods a storage holds, each as (damage, max damage), the one a station takes: the
/// least worn, never one that is worn itself. None when there is no such rod.
pub fn pick(rods: &[(u32, u32)]) -> Option<usize> {
    rods.iter()
        .enumerate()
        .filter(|(_, (damage, max))| !rod_worn(*damage, *max))
        .min_by_key(|(_, (damage, _))| *damage)
        .map(|(i, _)| i)
}

/// FR4: the size of an order for `waiting` stations.
pub fn order_size(waiting: usize) -> u32 {
    u32::try_from(waiting).unwrap_or(u32::MAX).min(ORDER_CAP)
}

/// What a storage's ask for rods does this step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// A rod order of the storage is open: nothing more is ordered.
    Open,
    /// The stations are being gathered: no order yet.
    Gather,
    /// Order this many rods now.
    Order(u32),
    /// The last order was refused: wait before asking again.
    Retry,
}

/// One storage's ask for rods: the stations waiting on it (each with the tick it was last heard),
/// since when it gathers them, and when it may ask again after a refusal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ask {
    waiting: BTreeMap<Pos, u64>,
    since: Option<u64>,
    retry_at: u64,
}

impl Ask {
    /// The station at `pos` waits for a rod at tick `now`.
    pub fn waits(&mut self, pos: Pos, now: u64) {
        self.waiting.insert(pos, now);
    }

    /// The station at `pos` has a rod again.
    pub fn served(&mut self, pos: Pos) {
        self.waiting.remove(&pos);
    }

    /// Whether the station at `pos` is counted.
    pub fn counts(&self, pos: Pos) -> bool {
        self.waiting.contains_key(&pos)
    }

    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// FR4, FR5: the step at tick `now`, `open` whether a rod order of the storage is open.
    pub fn step(&mut self, open: bool, now: u64) -> Step {
        self.waiting.retain(|_, heard| now.saturating_sub(*heard) < STALE_TICKS);
        if open {
            self.since = None;
            return Step::Open;
        }
        if now < self.retry_at {
            return Step::Retry;
        }
        if self.waiting.is_empty() {
            self.since = None;
            return Step::Gather;
        }
        let since = *self.since.get_or_insert(now);
        if now.saturating_sub(since) < GATHER_TICKS {
            return Step::Gather;
        }
        self.since = None;
        Step::Order(order_size(self.waiting.len()))
    }

    /// FR5: the order was refused at tick `now`.
    pub fn refused(&mut self, now: u64) {
        self.retry_at = now + RETRY_TICKS;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_station_takes_the_least_worn_whole_rod() {
        assert_eq!(pick(&[]), None);
        assert_eq!(pick(&[(63, 64)]), None, "a worn rod is never taken");
        assert_eq!(pick(&[(40, 64), (0, 64), (12, 64)]), Some(1));
        assert_eq!(pick(&[(63, 64), (30, 64)]), Some(1));
        assert_eq!(pick(&[(5, 64), (5, 64)]), Some(0), "the first of equals");
    }

    #[test]
    fn an_order_is_sized_to_the_waiting_stations_and_capped() {
        assert_eq!(order_size(0), 0);
        assert_eq!(order_size(2), 2);
        assert_eq!(order_size(50), ORDER_CAP);
    }

    #[test]
    fn fifty_stations_make_one_order_after_the_gathering() {
        let mut ask = Ask::default();
        // Fifty stations step over one second, each every 20 ticks.
        let mut orders = Vec::new();
        for now in 0..200u64 {
            for k in 0..50i32 {
                if now % 20 == u64::try_from(k % 20).unwrap() {
                    ask.waits([k, 0, 0], now);
                    match ask.step(!orders.is_empty(), now) {
                        Step::Order(n) => orders.push((now, n)),
                        Step::Open | Step::Gather => {}
                        Step::Retry => panic!("nothing was refused"),
                    }
                }
            }
        }
        assert_eq!(orders, vec![(GATHER_TICKS, ORDER_CAP)], "one order, capped, once gathered");
        assert_eq!(ask.waiting(), 50);
    }

    #[test]
    fn two_stations_order_two_and_nothing_while_it_is_open() {
        let mut ask = Ask::default();
        ask.waits([0, 0, 0], 0);
        assert_eq!(ask.step(false, 0), Step::Gather);
        ask.waits([1, 0, 0], 10);
        assert_eq!(ask.step(false, 10), Step::Gather);
        assert_eq!(ask.step(false, GATHER_TICKS), Step::Order(2));
        assert_eq!(ask.step(true, GATHER_TICKS + 20), Step::Open);
        // The rods came: both are served, nobody waits, nothing is ordered.
        ask.served([0, 0, 0]);
        ask.served([1, 0, 0]);
        assert!(!ask.counts([0, 0, 0]));
        for now in (GATHER_TICKS + 40..GATHER_TICKS + 400).step_by(20) {
            assert_eq!(ask.step(false, now), Step::Gather);
        }
    }

    #[test]
    fn a_refused_order_waits_before_asking_again() {
        let mut ask = Ask::default();
        ask.waits([0, 0, 0], 0);
        assert_eq!(ask.step(false, 0), Step::Gather);
        assert_eq!(ask.step(false, GATHER_TICKS), Step::Order(1));
        ask.refused(GATHER_TICKS);
        ask.waits([0, 0, 0], GATHER_TICKS + 20);
        assert_eq!(ask.step(false, GATHER_TICKS + 20), Step::Retry);
        let later = GATHER_TICKS + RETRY_TICKS;
        ask.waits([0, 0, 0], later);
        assert_eq!(ask.step(false, later), Step::Gather, "it gathers again");
        assert_eq!(ask.step(false, later + GATHER_TICKS), Step::Order(1));
    }

    #[test]
    fn a_station_not_heard_for_a_while_counts_no_more() {
        let mut ask = Ask::default();
        ask.waits([0, 0, 0], 0);
        ask.waits([1, 0, 0], 0);
        ask.step(false, 0);
        ask.waits([0, 0, 0], STALE_TICKS);
        assert_eq!(ask.step(false, STALE_TICKS), Step::Order(1), "the broken station is gone");
    }
}
