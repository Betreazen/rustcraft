//! After 0.16: the rest of the guide.
//!
//! - An order's items ride the item channel (spec 19, O4; `orders::ship`): on a line of 40
//!   housings the parts of a piston reach the far autocrafter no sooner than the channel's
//!   throughput and ride take, and a tier-3 channel brings them sooner than a tier-1 one could.
//! - The wrench, sneaking, lifts suspicious sand found in the world (spec 26): the block goes, its
//!   item remembers the cell and the loot table, and the archaeology table brushes it and tells
//!   Path-of-craft-tree `brush` (ST5: it is a block found in the world).

use super::magistral::line;
use super::slice15::{
    CELL, COBBLE, CRAFTER, FILL, IRON, PISTON, PLANKS, TERMINAL, held, holds, machine, parts_for,
    put_book, terminal_at,
};
use super::transport::{chest, fill, joint};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::net::items::Mode;
use crate::orders::ship::{Route, trip_ticks};
use crate::tuning::Tuning;
use crate::utility::found::Lift;

pub const ALL: [Build; 3] = [
    order_rides_tier_1,
    order_rides_tier_3,
    wrench_lifts_suspicious_sand,
];

/// Housings of the long line; the autocrafter stands beside the last.
const LEN: i32 = 40;
/// The storage's nearest joint is at housing 5 (its terminal): the pipe from there to the last.
const PIPE: u32 = (LEN - 5) as u32;
/// The parts of one piston.
const PARTS: u32 = 3 + 4 + 1 + 1;
/// Room past the trip for the rounds of the queue (once a second) and the arrival's second.
const SLACK: u64 = 120;
const SMART: u8 = crate::orders::queue::SMART_TIER;
const ORDERED: &str = "ordered";
const ARRIVED: &str = "arrived";

fn crafter_far(o: Pos) -> Pos {
    at(o, [LEN, 0, -1])
}

/// The ticks the parts of a piston take on a channel of `tier` from the storage to the far end.
fn trip(tier: u8, t: &Tuning) -> u64 {
    let route = Route::of_tier((0, [0; 3]), PIPE, tier, t);
    trip_ticks(PARTS, &route, t.ticks_per_second)
}

/// A node of tier 3 and LEN housings of `tier`, a chest «из блока» at
/// housing 1 with the parts of a piston, a storage of four cells with its terminal at housings 3
/// to 5, as slice 15 builds it.
fn long_line(b: &mut dyn Bench, o: Pos, tier: u8) -> Result<(), String> {
    line(b, o, SMART, LEN, tier)?;
    b.place(chest(o, 1), "minecraft:chest", &[])?;
    joint(b, o, 1, Mode::From, 0)?;
    for c in [[3, 0, 1], [4, 0, 1], [3, 0, 2], [4, 0, 2]] {
        b.place(at(o, c), CELL, &[])?;
    }
    b.place(terminal_at(o), TERMINAL, &[])?;
    for (item, n) in parts_for(1, IRON) {
        fill(b, chest(o, 1), item, n)?;
    }
    Ok(())
}

fn order_rides_tier_1(o: Pos, t: &Tuning) -> Scenario {
    ride(o, t, 1, "order_rides_a_tier_1_channel")
}

fn order_rides_tier_3(o: Pos, t: &Tuning) -> Scenario {
    ride(o, t, 3, "order_rides_a_tier_3_channel")
}

/// An order for a piston on a long line of `tier`: its parts reach the autocrafter (without
/// energy, so they stay in it) no sooner than the trip takes; on tier 3 sooner than a tier-1 trip.
fn ride(o: Pos, t: &Tuning, tier: u8, name: &'static str) -> Scenario {
    let wanted = trip(tier, t);
    let slow = trip(1, t);
    let parts = parts_for(1, IRON);
    Scenario::new(name, plot_area(o))
        .then(
            "a long line, a storage at its start, a chest of piston parts",
            move |b, _| long_line(b, o, tier),
        )
        .until("the storage holds the parts", FILL * 2, move |b, _| {
            holds(b, o, &parts)
        })
        .then(
            "an autocrafter without energy at the far end, the book",
            move |b, _| {
                machine(b, crafter_far(o), CRAFTER, false)?;
                put_book(b, o)
            },
        )
        .until(
            "the storage takes an order for a piston",
            FILL,
            move |b, memo| {
                let took = b.order(terminal_at(o), PISTON, 1).is_ok();
                if took {
                    memo.mark(ORDERED, memo.elapsed as f64);
                }
                Ok(took)
            },
        )
        .until(
            "the parts reach the autocrafter",
            wanted + SLACK,
            move |b, memo| {
                let items = b.items(crafter_far(o));
                let there = count_of(&items, COBBLE) == 4 && count_of(&items, PLANKS) == 3;
                if there {
                    memo.mark(ARRIVED, memo.elapsed as f64);
                }
                Ok(there)
            },
        )
        .check("they took the channel's time", move |b, memo| {
            let took = (memo.get(ARRIVED) - memo.get(ORDERED)) as u64;
            if took < wanted {
                return Err(format!(
                    "the parts came in {took} ticks, the trip takes {wanted}"
                ));
            }
            if tier > 1 && took >= slow {
                return Err(format!(
                    "{took} ticks on tier {tier}, a tier-1 trip takes {slow}"
                ));
            }
            same("cobblestone left in the storage", held(b, o, COBBLE)?, 0)
        })
}

const SUSPICIOUS: &str = "minecraft:suspicious_sand";
const PYRAMID: &str = "minecraft:archaeology/desert_pyramid";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0003_0e16;
const BRUSH: u64 = 400;

fn wrench_lifts_suspicious_sand(o: Pos, _: &Tuning) -> Scenario {
    let block = at(o, [2, 1, 0]);
    let table = at(o, [5, 1, 0]);
    Scenario::new("wrench_lifts_found_suspicious_sand", plot_area(o))
        .then(
            "suspicious sand with a pyramid's loot on stone, a table with a brush",
            move |b, _| {
                for x in 0..=6 {
                    b.place(at(o, [x, 0, 0]), "minecraft:stone", &[])?;
                }
                b.place(block, SUSPICIOUS, &[])?;
                b.set_brush_loot(block, PYRAMID)?;
                b.place(table, "rustcraft:archeo", &[])?;
                b.set_owner(table, OWNER, 1)?;
                b.set_buffer(table, 1000.0)?;
                b.put(table, 1, "minecraft:brush", 1)
            },
        )
        .then("the wrench lifts it into the table", move |b, _| {
            match b.wrench_lift(block, table, 0)? {
                Lift::Found(f) => {
                    same("found in", f.dimension.as_str(), "minecraft:overworld")?;
                    same("found at", f.pos, block)?;
                    same("its loot table", f.loot_table.as_deref(), Some(PYRAMID))
                }
                Lift::Grown => Err("a block found in the world came out grown".into()),
            }
        })
        .check("the block is gone from the world", move |b, _| {
            same("block", b.block(block).as_str(), "minecraft:air")
        })
        .until("the table gave its sand back", BRUSH, move |b, _| {
            Ok(count_of(&b.items(table), "minecraft:sand") == 1)
        })
        .check("the table told Path-of-craft-tree brush", move |b, _| {
            let head = format!("pct-machine brush {OWNER:032x} ");
            if b.reports().iter().any(|r| r.starts_with(&head)) {
                Ok(())
            } else {
                Err(format!("no brush told: {:?}", b.reports()))
            }
        })
}
