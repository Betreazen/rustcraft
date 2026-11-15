//! Slice 15: orders from the storage terminal (spec section 19).
//!
//! - With the recipe book in the terminal and a node of tier 3 on the line, an order for 4
//!   pistons goes to the autocrafter and 4 pistons come back to the storage (O3, O4).
//! - An order for a piston from raw iron runs through the electric furnace, then the
//!   autocrafter (O9–O11).
//! - An order short of iron waits and names the iron, and goes on once iron comes (O6).
//! - A cancelled order gives the autocrafter's unused ingredients back to the storage (O7).
//! - «Держать 2» makes the storage order pistons until it holds 2 (O14).

use super::magistral::{channel, line};
use super::transport::{chest, fill, joint};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::net::items::Mode;
use crate::net::topology::ChannelKind;
use crate::orders::queue::Wait;
use crate::storage::terminal::{self, Press};
use crate::tuning::Tuning;

pub const ALL: [Build; 5] = [
    orders_pistons,
    order_runs_through_a_furnace,
    order_waits_for_iron,
    cancel_gives_parts_back,
    keep_orders_the_difference,
];

const CELL: &str = "rustcraft:cell";
const TERMINAL: &str = "rustcraft:terminal";
const CRAFTER: &str = "rustcraft:crafter";
const FURNACE: &str = "rustcraft:furnace";
const CRYSTAL: &str = "rustcraft:crystal";
const BOOK: &str = "rustcraft:module_recipes";
const PISTON: &str = "minecraft:piston";
const PLANKS: &str = "minecraft:oak_planks";
const COBBLE: &str = "minecraft:cobblestone";
const IRON: &str = "minecraft:iron_ingot";
const RAW_IRON: &str = "minecraft:raw_iron";
const REDSTONE: &str = "minecraft:redstone";
/// M15: the node's tier that opens orders.
const SMART: u8 = crate::orders::queue::SMART_TIER;
/// How long the storage may take to take in the chest's items.
const FILL: u64 = 200;

fn terminal_at(o: Pos) -> Pos {
    at(o, [5, 0, 1])
}

fn crafter_at(o: Pos) -> Pos {
    at(o, [3, 0, -1])
}

fn furnace_at(o: Pos) -> Pos {
    at(o, [5, 0, -1])
}

/// A node of tier 3, five housings with an item channel, a chest «из блока» at housing 1, and a
/// storage of four cells with a terminal at housings 3 to 5; the chest holds `parts`.
fn build(b: &mut dyn Bench, o: Pos, parts: &[(&str, u32)]) -> Result<(), String> {
    line(
        b,
        o,
        SMART,
        5,
        [channel(ChannelKind::Item, 2), None, None, None],
    )?;
    b.place(chest(o, 1), "minecraft:chest", &[])?;
    joint(b, o, 1, Mode::From, 0)?;
    for c in [[3, 0, 1], [4, 0, 1], [3, 0, 2], [4, 0, 2]] {
        b.place(at(o, c), CELL, &[])?;
    }
    b.place(terminal_at(o), TERMINAL, &[])?;
    for (item, n) in parts {
        fill(b, chest(o, 1), item, *n)?;
    }
    Ok(())
}

/// A machine on the line, `powered` by two crystals that touch it.
fn machine(b: &mut dyn Bench, pos: Pos, block: &str, powered: bool) -> Result<(), String> {
    b.place(pos, block, &[])?;
    if powered {
        b.place(at(pos, [0, 0, -1]), CRYSTAL, &[])?;
        b.place(at(pos, [0, -1, 0]), CRYSTAL, &[])?;
    }
    Ok(())
}

fn held(b: &dyn Bench, o: Pos, item: &str) -> Result<u64, String> {
    let v = b
        .storage(terminal_at(o))
        .ok_or("the terminal shows no storage")?;
    Ok(v.store.count(item))
}

/// The storage holds every part.
fn holds(b: &dyn Bench, o: Pos, parts: &[(&str, u32)]) -> Result<bool, String> {
    for (item, n) in parts {
        if held(b, o, item)? != u64::from(*n) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The recipe book goes into the terminal's socket from the cursor.
fn put_book(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    let id = terminal::encode(Press::Book).ok_or("the book's press has no id")?;
    let after = b.press_cursor(terminal_at(o), id, Some((BOOK, 1)))?;
    same("cursor after the book went in", after, None)
}

/// What `pistons` pistons take, the iron as `iron` (ingots, or raw iron to smelt).
fn parts_for(pistons: u32, iron: &'static str) -> Vec<(&'static str, u32)> {
    vec![
        (PLANKS, 3 * pistons),
        (COBBLE, 4 * pistons),
        (iron, pistons),
        (REDSTONE, pistons),
    ]
}

fn orders_pistons(o: Pos, _: &Tuning) -> Scenario {
    let parts = parts_for(4, IRON);
    let p2 = parts.clone();
    Scenario::new("orders_make_pistons_on_a_crafter", plot_area(o))
        .then(
            "a storage on a tier 3 line, a chest of piston parts",
            move |b, _| build(b, o, &parts),
        )
        .until("the storage holds the parts", FILL, move |b, _| {
            holds(b, o, &p2)
        })
        .then(
            "an autocrafter on the line, the book in the terminal",
            move |b, _| {
                machine(b, crafter_at(o), CRAFTER, true)?;
                put_book(b, o)
            },
        )
        .until(
            "the storage takes an order for 4 pistons",
            FILL,
            move |b, _| Ok(b.order(terminal_at(o), PISTON, 4).is_ok()),
        )
        .until("4 pistons are back in the storage", 1200, move |b, _| {
            Ok(held(b, o, PISTON)? == 4)
        })
        .wait(40)
        .check("the order is done and its parts are used", move |b, _| {
            same("orders", b.orders(terminal_at(o)).len(), 0)?;
            same("planks in the storage", held(b, o, PLANKS)?, 0)?;
            same(
                "planks in the crafter",
                count_of(&b.items(crafter_at(o)), PLANKS),
                0,
            )
        })
}

fn order_runs_through_a_furnace(o: Pos, _: &Tuning) -> Scenario {
    let parts = parts_for(1, RAW_IRON);
    let p2 = parts.clone();
    Scenario::new("order_runs_through_a_furnace_and_a_crafter", plot_area(o))
        .then(
            "a storage on a tier 3 line, piston parts with raw iron",
            move |b, _| build(b, o, &parts),
        )
        .until("the storage holds the parts", FILL, move |b, _| {
            holds(b, o, &p2)
        })
        .then(
            "an electric furnace and an autocrafter on the line, the book",
            move |b, _| {
                machine(b, crafter_at(o), CRAFTER, true)?;
                machine(b, furnace_at(o), FURNACE, true)?;
                put_book(b, o)
            },
        )
        .until(
            "the storage takes an order for a piston",
            FILL,
            move |b, _| Ok(b.order(terminal_at(o), PISTON, 1).is_ok()),
        )
        .until("a piston is back in the storage", 2400, move |b, _| {
            Ok(held(b, o, PISTON)? == 1)
        })
        .check("the raw iron went through the furnace", move |b, _| {
            same("raw iron", held(b, o, RAW_IRON)?, 0)?;
            same("iron", held(b, o, IRON)?, 0)?;
            same("orders", b.orders(terminal_at(o)).len(), 0)
        })
}

fn order_waits_for_iron(o: Pos, _: &Tuning) -> Scenario {
    let parts: Vec<(&str, u32)> = vec![(PLANKS, 3), (COBBLE, 4), (REDSTONE, 1)];
    let p2 = parts.clone();
    Scenario::new("order_waits_for_a_missing_item", plot_area(o))
        .then("a storage with piston parts but no iron", move |b, _| {
            build(b, o, &parts)
        })
        .until("the storage holds the parts", FILL, move |b, _| {
            holds(b, o, &p2)
        })
        .then("an autocrafter on the line, the book", move |b, _| {
            machine(b, crafter_at(o), CRAFTER, true)?;
            put_book(b, o)
        })
        .until(
            "the storage takes an order for a piston",
            FILL,
            move |b, _| Ok(b.order(terminal_at(o), PISTON, 1).is_ok()),
        )
        .until("the order names the iron it lacks", 100, move |b, _| {
            let orders = b.orders(terminal_at(o));
            Ok(orders
                .first()
                .is_some_and(|q| q.wait == Wait::Items(vec![(IRON.into(), 1)])))
        })
        .then("an iron ingot comes", move |b, _| {
            fill(b, chest(o, 1), IRON, 1)
        })
        .until("the piston is made", 1200, move |b, _| {
            Ok(held(b, o, PISTON)? == 1)
        })
}

fn cancel_gives_parts_back(o: Pos, _: &Tuning) -> Scenario {
    let parts = parts_for(2, IRON);
    let p2 = parts.clone();
    let p3 = parts.clone();
    Scenario::new("cancelled_order_gives_its_parts_back", plot_area(o))
        .then("a storage with parts for 2 pistons", move |b, _| {
            build(b, o, &parts)
        })
        .until("the storage holds the parts", FILL, move |b, _| {
            holds(b, o, &p2)
        })
        .then(
            "an autocrafter without energy on the line, the book",
            move |b, _| {
                machine(b, crafter_at(o), CRAFTER, false)?;
                put_book(b, o)
            },
        )
        .until(
            "the storage takes an order for 2 pistons",
            FILL,
            move |b, _| Ok(b.order(terminal_at(o), PISTON, 2).is_ok()),
        )
        .until("the parts are in the crafter", 100, move |b, _| {
            Ok(count_of(&b.items(crafter_at(o)), COBBLE) == 8)
        })
        .then("cancel the order", move |b, _| {
            let id = b
                .orders(terminal_at(o))
                .first()
                .map(|q| q.id)
                .ok_or("no order to cancel")?;
            b.cancel_order(terminal_at(o), id)
        })
        .until("the parts are back in the storage", 100, move |b, _| {
            holds(b, o, &p3)
        })
        .check("the crafter keeps none of them", move |b, _| {
            same(
                "cobblestone in the crafter",
                count_of(&b.items(crafter_at(o)), COBBLE),
                0,
            )?;
            same("orders", b.orders(terminal_at(o)).len(), 0)
        })
}

fn keep_orders_the_difference(o: Pos, _: &Tuning) -> Scenario {
    let parts = parts_for(2, IRON);
    let p2 = parts.clone();
    Scenario::new("keep_n_orders_the_difference", plot_area(o))
        .then("a storage with parts for 2 pistons", move |b, _| {
            build(b, o, &parts)
        })
        .until("the storage holds the parts", FILL, move |b, _| {
            holds(b, o, &p2)
        })
        .then("an autocrafter, the book, keep 2 pistons", move |b, _| {
            machine(b, crafter_at(o), CRAFTER, true)?;
            put_book(b, o)?;
            b.set_keep(terminal_at(o), PISTON, 2)
        })
        .until("the storage holds 2 pistons", 1600, move |b, _| {
            Ok(held(b, o, PISTON)? == 2)
        })
        .wait(60)
        .check("it orders no more", move |b, _| {
            same("pistons", held(b, o, PISTON)?, 2)?;
            same("orders", b.orders(terminal_at(o)).len(), 0)
        })
}
