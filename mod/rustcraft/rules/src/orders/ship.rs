//! Slice 17: an order's items ride the item channel (spec 19, O4: «ингредиенты под неё приходят
//! по магистрали, готовое уходит обратно на склад»; O11). Until slice 15 they went from the
//! storage straight into the machine and back.
//!
//! A job's items leave the storage as one shipment on the item net that joins the storage and
//! the machine; the machine's products go back the same way. The net has no move «this item to
//! that machine» cell by cell, so a shipment is a model of the ride, not items in the pipe:
//!
//! - It is loaded onto the net at the net's throughput (M10: the weakest channel's items a
//!   second), the net's shipments one after another in the order they came, and the net's own
//!   moves get what the shipments leave of that throughput (`left_for_net`).
//! - Its last item then rides the pipe between the two joints (`Route::blocks`) at the ride speed
//!   of the net's weakest channel (`Tuning::ride_speed_of`, a proposal); it arrives whole.
//! - It is counted in thousandths of an item and in ticks, so the storage's record keeps it
//!   exactly (`saved`).
//!
//! A shipment whose job is gone when it arrives (the order was cancelled, the machine broken, the
//! machine's inputs filled meanwhile) turns back and rides to the storage (`turn_back`), so items
//! are never lost or made twice. Where no item path joins the storage and the machine (their
//! joints only on nets apart, or switched off) the bundle moves the items directly, as slice 15
//! did.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geom::Pos;
use crate::net::items::ItemKey;
use crate::tuning::Tuning;

/// A shipment's load is counted in thousandths of an item.
pub const MILLI: u64 = 1000;

/// The item net a shipment rides, as the bundle keys its nets: the net's slot and first housing.
pub type NetKey = (usize, Pos);

/// The way a shipment rides between the storage and a machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Route {
    pub net: NetKey,
    /// Blocks of pipe its last item rides: the pipe between the two joints' housings and one
    /// for the housing it goes in by.
    pub blocks: u32,
    /// What the net carries, items a second (M10).
    pub per_second: f64,
    /// How fast an item rides it, blocks a second.
    pub speed: f64,
}

impl Route {
    /// The route of a net with weakest channel `tier`, `pipe` blocks of pipe between the joints.
    pub fn of_tier(net: NetKey, pipe: u32, tier: u8, tuning: &Tuning) -> Route {
        Route {
            net,
            blocks: pipe + 1,
            per_second: tuning.items_of(tier),
            speed: tuning.ride_speed_of(tier),
        }
    }

    /// Ticks the last item rides once it is loaded.
    pub fn ride_ticks(&self, ticks_per_second: f64) -> u32 {
        if self.speed <= 0.0 {
            return u32::MAX;
        }
        let ticks = (f64::from(self.blocks) / self.speed * ticks_per_second).ceil();
        ticks.clamp(0.0, f64::from(u32::MAX)) as u32
    }

    /// Thousandths of an item the net loads a tick, at least one.
    pub fn load_per_tick(&self, ticks_per_second: f64) -> u64 {
        let milli = self.per_second * MILLI as f64 / ticks_per_second.max(1.0);
        (milli.round() as u64).max(1)
    }
}

/// Where a shipment goes and what it does there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum Leg {
    /// A job's items from the storage into the inputs of its machine at `machine`.
    Machine { machine: Pos },
    /// A machine's products back into the storage; `own` (O11) when the order's next step takes
    /// them: they are the order's own there, past the storage's room and limits.
    Products { from: Pos, own: bool },
    /// Items that went out for a job and come back unused (a cancelled order, a shipment turned
    /// back): into the storage past its room and limits, where they came from.
    Back { from: Pos },
}

/// Items of one job riding the item channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shipment {
    /// The storage the job works for.
    pub storage: u32,
    pub order: u16,
    pub leg: Leg,
    pub items: Vec<(ItemKey, u32)>,
    pub net: NetKey,
    /// Thousandths of an item still to load onto the net.
    pub to_load: u64,
    /// Thousandths of an item the net loads a tick.
    pub load_rate: u64,
    /// Ticks the last item still rides once loaded.
    pub ride: u32,
    /// The whole ride, for the way back.
    pub full_ride: u32,
}

impl Shipment {
    /// `items` set off along `route` now.
    pub fn new(
        storage: u32,
        order: u16,
        leg: Leg,
        items: Vec<(ItemKey, u32)>,
        route: &Route,
        ticks_per_second: f64,
    ) -> Shipment {
        let count: u64 = items.iter().map(|(_, n)| u64::from(*n)).sum();
        let ride = route.ride_ticks(ticks_per_second);
        Shipment {
            storage,
            order,
            leg,
            items,
            net: route.net,
            to_load: count * MILLI,
            load_rate: route.load_per_tick(ticks_per_second),
            ride,
            full_ride: ride,
        }
    }

    /// How many items it carries.
    pub fn count(&self) -> u64 {
        self.items.iter().map(|(_, n)| u64::from(*n)).sum()
    }

    /// Whether its last item is at the far end.
    pub fn arrived(&self) -> bool {
        self.to_load == 0 && self.ride == 0
    }

    /// Ticks until it arrives when no shipment before it shares its net.
    pub fn ticks_left(&self) -> u64 {
        self.to_load.div_ceil(self.load_rate.max(1)) + u64::from(self.ride)
    }

    /// Whether it carries items of the job of `order` to the machine at `machine`.
    pub fn bound_for(&self, storage: u32, order: u16, machine: Pos) -> bool {
        self.storage == storage && self.order == order && self.leg == Leg::Machine { machine }
    }

    /// It goes back the way it came, the whole ride again, into the storage.
    pub fn turn_back(self) -> Shipment {
        let from = match self.leg {
            Leg::Machine { machine } => machine,
            Leg::Products { from, .. } | Leg::Back { from } => from,
        };
        Shipment {
            leg: Leg::Back { from },
            to_load: self.count() * MILLI,
            ride: self.full_ride,
            ..self
        }
    }
}

/// `ticks` pass. Each net loads its shipments one after another in the order they stand in
/// `ships`, at its rate; a shipment that is loaded rides on, from the tick its last item went in.
/// What each net loaded meanwhile, in thousandths of an item.
pub fn advance(ships: &mut [Shipment], ticks: u32) -> BTreeMap<NetKey, u64> {
    let mut time_left: BTreeMap<NetKey, u32> = BTreeMap::new();
    let mut loaded: BTreeMap<NetKey, u64> = BTreeMap::new();
    for s in ships.iter_mut() {
        if s.to_load == 0 {
            s.ride = s.ride.saturating_sub(ticks);
            continue;
        }
        let left = time_left.entry(s.net).or_insert(ticks);
        if *left == 0 {
            continue;
        }
        let rate = s.load_rate.max(1);
        let need = s.to_load.div_ceil(rate);
        let used = loaded.entry(s.net).or_insert(0);
        if need <= u64::from(*left) {
            *used += s.to_load;
            s.to_load = 0;
            *left -= need as u32;
            // It rides for the rest of this step.
            s.ride = s.ride.saturating_sub(*left);
        } else {
            let part = rate * u64::from(*left);
            *used += part;
            s.to_load -= part;
            *left = 0;
        }
    }
    loaded
}

/// What an item net of `per_second` items a second has left for its own moves while the
/// shipments loaded `loaded_milli` thousandths of an item on it over `ticks` ticks.
pub fn left_for_net(per_second: f64, loaded_milli: u64, ticks: u32, ticks_per_second: f64) -> f64 {
    if ticks == 0 {
        return per_second;
    }
    let seconds = f64::from(ticks) / ticks_per_second.max(1.0);
    let used = loaded_milli as f64 / MILLI as f64 / seconds;
    (per_second - used).max(0.0)
}

/// Ticks `items` items take from the storage to a machine on a route, alone on its net: what a
/// self-test waits at least.
pub fn trip_ticks(items: u32, route: &Route, ticks_per_second: f64) -> u64 {
    u64::from(items)
        .saturating_mul(MILLI)
        .div_ceil(route.load_per_tick(ticks_per_second))
        + u64::from(route.ride_ticks(ticks_per_second))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TPS: f64 = 20.0;
    const NET: NetKey = (0, [1, 64, 0]);
    const OTHER: NetKey = (1, [1, 64, 0]);
    const IRON: &str = "minecraft:iron_ingot";

    fn route(tier: u8, pipe: u32) -> Route {
        Route::of_tier(NET, pipe, tier, &Tuning::default())
    }

    fn ship(n: u32, route: &Route) -> Shipment {
        Shipment::new(
            3,
            7,
            Leg::Machine {
                machine: [9, 64, 0],
            },
            vec![(IRON.into(), n)],
            route,
            TPS,
        )
    }

    /// Steps of a second until it arrives: the ticks it took.
    fn ride_out(ships: &mut [Shipment], which: usize) -> u32 {
        let mut ticks = 0;
        while !ships[which].arrived() {
            advance(ships, 1);
            ticks += 1;
            assert!(ticks < 100_000, "never arrives");
        }
        ticks
    }

    #[test]
    fn a_route_rides_at_its_tiers_speed_and_loads_at_its_rate() {
        // Tier 1: 4 items a second, 4 blocks a second (proposal).
        let r = route(1, 39);
        assert_eq!(r.blocks, 40);
        assert_eq!(r.ride_ticks(TPS), 200);
        assert_eq!(r.load_per_tick(TPS), 200);
        // Tier 3: 64 items a second, 16 blocks a second.
        let r3 = route(3, 39);
        assert_eq!(r3.ride_ticks(TPS), 50);
        assert_eq!(r3.load_per_tick(TPS), 3200);
        // A housing beside both ends: one block of ride.
        assert_eq!(route(2, 0).ride_ticks(TPS), 3);
    }

    #[test]
    fn a_shipment_loads_then_rides_and_arrives_whole() {
        let r = route(1, 39);
        let mut ships = vec![ship(8, &r)];
        assert_eq!(ships[0].ticks_left(), 40 + 200);
        // Loaded in 40 ticks, then 200 of riding.
        assert_eq!(ride_out(&mut ships, 0), 240);
        assert_eq!(trip_ticks(8, &r, TPS), 240);
        // In steps of a second it takes as long.
        let mut ships = vec![ship(8, &r)];
        let mut seconds = 0;
        while !ships[0].arrived() {
            advance(&mut ships, 20);
            seconds += 1;
        }
        assert_eq!(seconds, 12);
    }

    #[test]
    fn a_tier_one_channel_is_slower_than_tier_three() {
        let slow = trip_ticks(9, &route(1, 37), TPS);
        let fast = trip_ticks(9, &route(3, 37), TPS);
        assert!(slow > fast * 3, "{slow} vs {fast}");
    }

    #[test]
    fn shipments_on_one_net_share_its_throughput_in_turn() {
        let r = route(1, 0);
        // 8 items each: 40 ticks of loading apiece.
        let mut ships = vec![ship(8, &r), ship(8, &r)];
        let loaded = advance(&mut ships, 20);
        // The first takes the whole second's throughput, the second waits.
        assert_eq!(loaded[&NET], 4 * MILLI);
        assert_eq!(ships[0].to_load, 4 * MILLI);
        assert_eq!(ships[1].to_load, 8 * MILLI);
        advance(&mut ships, 20);
        assert_eq!(ships[0].to_load, 0);
        assert_eq!(ships[1].to_load, 8 * MILLI);
        let first = ride_out(&mut ships, 0);
        let second = ride_out(&mut ships, 1) + first;
        // The second arrives two seconds of loading after the first.
        assert_eq!(second - first, 40);
        // A shipment on another net does not wait for them.
        let mut ships = vec![
            ship(8, &r),
            Shipment {
                net: OTHER,
                ..ship(8, &r)
            },
        ];
        advance(&mut ships, 20);
        assert_eq!(ships[1].to_load, 4 * MILLI);
    }

    #[test]
    fn the_net_keeps_what_the_shipments_leave_of_its_throughput() {
        assert_eq!(left_for_net(16.0, 4 * MILLI, 20, TPS), 12.0);
        assert_eq!(left_for_net(4.0, 4 * MILLI, 20, TPS), 0.0);
        assert_eq!(left_for_net(4.0, 0, 20, TPS), 4.0);
        assert_eq!(left_for_net(4.0, 9 * MILLI, 0, TPS), 4.0);
    }

    #[test]
    fn a_shipment_turned_back_rides_the_whole_way_back() {
        let r = route(2, 9);
        let mut ships = vec![ship(5, &r)];
        ride_out(&mut ships, 0);
        let back = ships.remove(0).turn_back();
        assert_eq!(back.leg, Leg::Back { from: [9, 64, 0] });
        assert_eq!(back.items, vec![(IRON.to_string(), 5)]);
        assert_eq!(back.ticks_left(), trip_ticks(5, &r, TPS));
        assert!(!back.bound_for(3, 7, [9, 64, 0]));
        assert!(ship(1, &r).bound_for(3, 7, [9, 64, 0]));
        assert!(!ship(1, &r).bound_for(3, 8, [9, 64, 0]));
    }

    #[test]
    fn an_empty_shipment_only_rides() {
        let r = route(4, 31);
        let mut ships = vec![Shipment::new(
            1,
            1,
            Leg::Back { from: [0; 3] },
            vec![],
            &r,
            TPS,
        )];
        assert_eq!(ride_out(&mut ships, 0), r.ride_ticks(TPS));
    }
}
