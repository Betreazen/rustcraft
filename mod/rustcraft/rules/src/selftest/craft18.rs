//! Slice 18, stream 5: the craft machines and the storage as the handbook says them.
//!
//! - The storage keeps stacks with components (an enchanted book, a named block) whole.
//! - The stack module in the terminal's socket raises the stack of the whole storage (S3); the
//!   filter module goes into its own socket and comes back out.

use super::transport::{chest, item_line, joint};
use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::net::items::Mode;
use crate::storage::modules::{FILTER, SOCKET_FILTER, SOCKET_STACK, STACK_MODULES};
use crate::storage::terminal::{self, Press};
use crate::tuning::Tuning;

pub const ALL: [Build; 3] = [
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
