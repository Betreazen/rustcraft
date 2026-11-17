//! Slice 18, the last leftovers: orders as the handbook says them (docs/superpowers/plans/
//! s18-last.md, tasks 4 and 5).
//!
//! - O11: an order for pistons from raw iron runs through the electric furnace and then the
//!   autocrafter; the iron ingots go from the furnace straight to the autocrafter, so the
//!   storage never holds one while the order runs, and the order is done.

use super::slice15::{
    CRAFTER, FURNACE, IRON, PISTON, RAW_IRON, build, crafter_at, furnace_at, held, holds,
    machine, parts_for, put_book, terminal_at,
};
use super::{Build, Scenario, plot_area, same};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [o11_product_goes_straight_to_the_next_machine];

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
        .then(
            "an electric furnace and an autocrafter on the line, the book",
            move |b, _| {
                machine(b, crafter_at(o), CRAFTER, true)?;
                machine(b, furnace_at(o), FURNACE, true)?;
                put_book(b, o)
            },
        )
        .until("the storage takes an order for 2 pistons", FILL, move |b, _| {
            Ok(b.order(terminal_at(o), PISTON, 2).is_ok())
        })
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
        .check("the raw iron went through the furnace", move |b, _| {
            same("raw iron", held(b, o, RAW_IRON)?, 0)?;
            same("pistons", held(b, o, PISTON)?, 2)
        })
}
