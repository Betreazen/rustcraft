//! Slice 18, the last leftovers: orders as the handbook says them (docs/superpowers/plans/
//! s18-last.md, tasks 4 and 5).
//!
//! - O11: an order for pistons from raw iron runs through the crusher (0.19.3: ore is never
//!   smelted whole), the electric furnace and then the autocrafter; the iron ingots go from the
//!   furnace straight to the autocrafter, so the storage never holds one while the order runs,
//!   and the order is done.
//! - The parts of an order riding to the autocrafter are written into a housing on their way
//!   (guide `orders`: the items seen moving in the housings), and the housings are cleared once
//!   the order is done.

use super::slice15::{
    CRAFTER, DUST_IRON, FURNACE, IRON, PISTON, RAW_IRON, build, crafter_at, crusher_on, furnace_at,
    held, holds, machine, parts_for, put_book, terminal_at,
};
use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::orders::ride;
use crate::tuning::Tuning;

pub const ALL: [Build; 2] = [
    o11_product_goes_straight_to_the_next_machine,
    order_items_ride_the_housings,
];

/// How long the storage may take to take in the chest's items.
const FILL: u64 = 200;

fn o11_product_goes_straight_to_the_next_machine(o: Pos, _: &Tuning) -> Scenario {
    let parts = parts_for(2, RAW_IRON);
    let p2 = parts.clone();
    Scenario::new("s18_o11_product_goes_straight_to_the_next_machine", plot_area(o))
        .then(
            "a storage on a tier 3 line, parts for 2 pistons with raw iron",
            move |b, _| build(b, o, &parts),
        )
        .until("the storage holds the parts", FILL, move |b, _| holds(b, o, &p2))
        .then("the book", move |b, _| put_book(b, o))
        // 0.20: the order comes before the crusher, so the storage keeps its ore for it and gives
        // the idle crusher none of its own accord (rustcraft_rules::net::items::ORE_BATCH).
        .until("the storage takes an order for 2 pistons", FILL, move |b, _| {
            Ok(b.order(terminal_at(o), PISTON, 2).is_ok())
        })
        .then(
            "a crusher, an electric furnace and an autocrafter on the line",
            move |b, _| {
                machine(b, crafter_at(o), CRAFTER, true)?;
                machine(b, furnace_at(o), FURNACE, true)?;
                crusher_on(b, o)
            },
        )
        .until(
            "2 pistons are back, no iron ingot ever in the storage",
            2400,
            move |b, _| {
                same("iron ingots in the storage while the order runs", held(b, o, IRON)?, 0)?;
                Ok(held(b, o, PISTON)? == 2)
            },
        )
        .until("and the order is done", 60, move |b, _| {
            same("iron ingots in the storage", held(b, o, IRON)?, 0)?;
            Ok(b.orders(terminal_at(o)).is_empty())
        })
        .check("the raw iron went through the crusher: one ore made both ingots", move |b, _| {
            same("raw iron left", held(b, o, RAW_IRON)?, 1)?;
            same("dust left", held(b, o, DUST_IRON)?, 0)?;
            same("pistons", held(b, o, PISTON)?, 2)
        })
}

/// Whether some housing of the line built from `o` carries riding items in its saved compound.
fn some_housing_rides(b: &dyn Bench, o: Pos) -> bool {
    (1..=5).any(|k| !b.int_array(at(o, [k, 0, 0]), ride::RIDE_KEY).is_empty())
}

fn order_items_ride_the_housings(o: Pos, _: &Tuning) -> Scenario {
    let parts = parts_for(4, IRON);
    let p2 = parts.clone();
    Scenario::new("s18_order_items_ride_the_housings", plot_area(o))
        .then(
            "a storage on a tier 3 line, a chest of piston parts",
            move |b, _| build(b, o, &parts),
        )
        .until("the storage holds the parts", FILL, move |b, _| holds(b, o, &p2))
        .then("an autocrafter on the line, the book in the terminal", move |b, _| {
            machine(b, crafter_at(o), CRAFTER, true)?;
            put_book(b, o)
        })
        .until("the storage takes an order for 4 pistons", FILL, move |b, _| {
            Ok(b.order(terminal_at(o), PISTON, 4).is_ok())
        })
        .until("a housing on the way carries the riding parts", 400, move |b, _| {
            Ok(some_housing_rides(b, o))
        })
        .until("4 pistons are back in the storage", 1200, move |b, _| {
            Ok(held(b, o, PISTON)? == 4)
        })
        .until("no housing carries riding items any more", 200, move |b, _| {
            Ok(!some_housing_rides(b, o))
        })
}
