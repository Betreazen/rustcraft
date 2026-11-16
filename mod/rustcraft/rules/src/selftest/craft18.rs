//! Slice 18, stream 5: the craft machines and the storage as the handbook says them.
//!
//! - The storage keeps stacks with components (an enchanted book, a named block) whole.
//! - The stack module in the terminal's socket raises the stack of the whole storage (S3); the
//!   filter module goes into its own socket and comes back out.
//! - An extra terminal anywhere on the storage's magistral shows the storage (S6).
//! - A full storage says «Склад полон».
//! - A storage with a missing cell has its terminal keep the cell for clients to draw (S8).
//! - An order for bread runs through a kitchen on the storage's line (guide `kitchen`).

use super::transport::{chest, fill, item_line, joint};
use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::net::items::Mode;
use crate::storage::modules::{FILTER, SOCKET_FILTER, SOCKET_STACK, STACK_MODULES};
use crate::storage::terminal::{self, Press};
use crate::tuning::Tuning;

pub const ALL: [Build; 7] = [
    order_bread_through_the_kitchen,
    storage_terminal_keeps_its_holes,
    storage_full_says_so,
    storage_far_terminal,
    storage_keeps_component_stacks,
    storage_stack_module_raises_the_stack,
    storage_filter_socket,
];

const CELL: &str = "rustcraft:cell";
const TERMINAL: &str = "rustcraft:terminal";
/// How long a net has to bring what it can.
const SETTLE: u64 = 100;

/// A chest «из блока» at housing 1, an item net of tier 2 and a storage of two cells with a
/// terminal touching housings 3 and 4.
fn storage_on_a_net(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    item_line(b, o, 2, 4, &[1])?;
    joint(b, o, 1, Mode::From, 0)?;
    b.place(at(o, [3, 0, 1]), CELL, &[])?;
    b.place(at(o, [4, 0, 1]), CELL, &[])?;
    b.place(at(o, [5, 0, 1]), TERMINAL, &[])
}

const BOOK: &str = r#"{id:"minecraft:enchanted_book",count:1,components:{"minecraft:stored_enchantments":{"minecraft:sharpness":3}}}"#;
const NAMED: &str = r#"{id:"minecraft:stone",count:5,components:{"minecraft:custom_name":"Plan"}}"#;

fn storage_keeps_component_stacks(o: Pos, _: &Tuning) -> Scenario {
    let a = chest(o, 1);
    let t = at(o, [5, 0, 1]);
    Scenario::new("storage18_keeps_component_stacks", plot_area(o))
        .then("a storage on a net", move |b, _| storage_on_a_net(b, o))
        .then("an enchanted book and named stone in the source", move |b, _| {
            same("book in", b.insert_snbt(a, BOOK)?, 1)?;
            same("stone in", b.insert_snbt(a, NAMED)?, 5)
        })
        .until("the storage holds both", SETTLE, move |b, _| {
            Ok(b.stored_stacks(t).len() == 2)
        })
        .check("each comes out with its components", move |b, _| {
            let mut got = b.stored_stacks(t);
            got.sort();
            same(
                "stacks",
                got,
                vec![
                    "minecraft:enchanted_book x1 minecraft:sharpness 3".to_owned(),
                    "minecraft:stone x5 \"Plan\"".to_owned(),
                ],
            )
        })
        .check("its record comes back the same from a save", move |b, _| {
            b.round_trip(t)
        })
}

/// The press id of terminal socket `k`.
fn socket(k: usize) -> Result<i32, String> {
    terminal::encode(Press::Socket(k as u8)).ok_or_else(|| format!("socket {k} has no press"))
}

fn storage_stack_module_raises_the_stack(o: Pos, _: &Tuning) -> Scenario {
    let t = at(o, [5, 0, 1]);
    let module = STACK_MODULES[0].0;
    Scenario::new("storage18_stack_module_raises_the_stack", plot_area(o))
        .then("a storage of two cells", move |b, _| storage_on_a_net(b, o))
        .then("the stack module into the terminal's socket", move |b, _| {
            let after = b.press_cursor(t, socket(SOCKET_STACK)?, Some((module, 1)))?;
            same("cursor after the module went in", after, None)
        })
        .wait(5)
        .check("a slot holds 256, the storage 54 of them", move |b, _| {
            let v = b.storage(t).ok_or("no storage")?;
            same("stack tier", v.store.stack_module, 1)?;
            same("room", v.store.room_for("minecraft:cobblestone", &|_| 64), 54 * 256)
        })
        .then("the module back out", move |b, _| {
            let after = b.press_cursor(t, socket(SOCKET_STACK)?, None)?;
            same("cursor", after, Some((module.to_owned(), 1)))
        })
        .wait(5)
        .check("the stack is 64 again", move |b, _| {
            let v = b.storage(t).ok_or("no storage")?;
            same("stack tier", v.store.stack_module, 0)
        })
}

fn storage_filter_socket(o: Pos, _: &Tuning) -> Scenario {
    let t = at(o, [5, 0, 1]);
    Scenario::new("storage18_filter_socket", plot_area(o))
        .then("a storage", move |b, _| storage_on_a_net(b, o))
        .then("a stone goes nowhere, the filter module goes in", move |b, _| {
            let refused = b.press_cursor(t, socket(SOCKET_FILTER)?, Some(("minecraft:stone", 1)))?;
            same("stone stays on the cursor", refused, Some(("minecraft:stone".to_owned(), 1)))?;
            let after = b.press_cursor(t, socket(SOCKET_FILTER)?, Some((FILTER, 1)))?;
            same("cursor after the module went in", after, None)
        })
        .check("it comes back out", move |b, _| {
            let after = b.press_cursor(t, socket(SOCKET_FILTER)?, None)?;
            same("cursor", after, Some((FILTER.to_owned(), 1)))
        })
}

fn storage_far_terminal(o: Pos, _: &Tuning) -> Scenario {
    let far = at(o, [1, 0, 1]);
    Scenario::new("storage18_far_terminal_on_the_net", plot_area(o))
        .then("a storage, and a terminal by housing 1 away from its cells", move |b, _| {
            storage_on_a_net(b, o)?;
            b.place(far, TERMINAL, &[])
        })
        .until("the far terminal shows the storage", 40, move |b, _| {
            Ok(b.storage(far).is_some_and(|v| v.cells == 2))
        })
}

fn storage_full_says_so(o: Pos, _: &Tuning) -> Scenario {
    let a = chest(o, 1);
    let cell = at(o, [3, 0, 1]);
    Scenario::new("storage18_full_says_so", plot_area(o))
        .then("a storage of one cell on a net", move |b, _| {
            item_line(b, o, 2, 4, &[1])?;
            joint(b, o, 1, Mode::From, 0)?;
            b.place(cell, CELL, &[])?;
            b.place(at(o, [3, 0, 2]), TERMINAL, &[])
        })
        .then("27 swords (one a slot) in the source", move |b, _| {
            fill(b, a, "minecraft:iron_sword", 27)
        })
        .until("the storage says it is full", 200, move |b, _| {
            Ok(b.storage_diagnosis(cell).as_deref() == Some("rustcraft.storage.full_at"))
        })
}

fn storage_terminal_keeps_its_holes(o: Pos, _: &Tuning) -> Scenario {
    let t = at(o, [5, 0, 1]);
    let gone = at(o, [3, 0, 2]);
    Scenario::new("storage18_terminal_keeps_its_holes", plot_area(o))
        .then("a storage of four cells in a 2x2 box", move |b, _| {
            storage_on_a_net(b, o)?;
            b.place(at(o, [4, 0, 2]), CELL, &[])?;
            b.place(gone, CELL, &[])
        })
        .wait(20)
        .check("no hole yet", move |b, _| same("holes", b.int_array(t, "holes"), Vec::new()))
        .then("a corner cell taken away", move |b, _| b.place(gone, "minecraft:air", &[]))
        .until("the terminal keeps the hole, as an offset from it", 60, move |b, _| {
            Ok(b.int_array(t, "holes") == vec![gone[0] - t[0], gone[1] - t[1], gone[2] - t[2]])
        })
}

fn order_bread_through_the_kitchen(o: Pos, _: &Tuning) -> Scenario {
    use super::slice15::{build, crafter_at, held, put_book, terminal_at};
    const WHEAT: &str = "minecraft:wheat";
    const BREAD: &str = "minecraft:bread";
    Scenario::new("orders18_bread_through_the_kitchen", plot_area(o))
        .then("a storage on a tier 3 line with wheat", move |b, _| build(b, o, &[(WHEAT, 3)]))
        .until("the storage holds the wheat", 200, move |b, _| Ok(held(b, o, WHEAT)? == 3))
        .then("a kitchen on the line with energy, and the book", move |b, _| {
            b.place(crafter_at(o), "rustcraft:kitchen", &[])?;
            b.set_buffer(crafter_at(o), 400.0)?;
            put_book(b, o)
        })
        .until("the storage takes an order for bread", 200, move |b, _| {
            Ok(b.order(terminal_at(o), BREAD, 1).is_ok())
        })
        .until("the bread is back in the storage", 1200, move |b, _| Ok(held(b, o, BREAD)? == 1))
        .check("the wheat went into it and the order is done", move |b, _| {
            same("wheat", held(b, o, WHEAT)?, 0)?;
            same("orders", b.orders(terminal_at(o)).len(), 0)
        })
}
