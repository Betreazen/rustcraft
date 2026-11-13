//! Slice 5: the storage. Cells in a solid box are one storage that a terminal shows and an item
//! net fills (S1, S2, M16); a limit holds (S5) or destroys the surplus; a box with a hole names
//! the missing cell and keeps everything it held (S8).

use super::transport::{COBBLE, chest, fill, item_line, joint};
use super::{Bench, Build, Scenario, StorageView, at, count_of, plot_area, same, total};
use crate::geom::Pos;
use crate::net::items::Mode;
use crate::storage::terminal::LimitChange;
use crate::storage::{SLOTS_PER_CELL, Shape};
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [
    counts_cells,
    limit_holds,
    destroys_surplus,
    hole_keeps_items,
];

const CELL: &str = "rustcraft:cell";
const TERMINAL: &str = "rustcraft:terminal";
/// The limit the limit scenarios set, and what the source holds.
const LIMIT: i64 = 10;
const OFFERED: u32 = 30;
/// How long a net has to bring what it can.
const SETTLE: u64 = 100;

/// The cells: a 2×1×2 box on the z+ side of housings 3 and 4.
fn cells(o: Pos) -> [Pos; 4] {
    [
        at(o, [3, 0, 1]),
        at(o, [4, 0, 1]),
        at(o, [3, 0, 2]),
        at(o, [4, 0, 2]),
    ]
}

fn terminal(o: Pos) -> Pos {
    at(o, [5, 0, 1])
}

/// A chest «из блока» at housing 1, an item net of tier 2, and a storage of four cells with a
/// terminal touching housings 3 and 4.
fn build(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    item_line(b, o, 2, 4, &[1])?;
    joint(b, o, 1, Mode::From, 0)?;
    for c in cells(o) {
        b.place(c, CELL, &[])?;
    }
    b.place(terminal(o), TERMINAL, &[])
}

fn view(b: &dyn Bench, o: Pos) -> Result<StorageView, String> {
    b.storage(terminal(o))
        .ok_or_else(|| "the terminal shows no storage".into())
}

fn held(b: &dyn Bench, o: Pos) -> Result<u64, String> {
    Ok(view(b, o)?.store.count(COBBLE))
}

fn counts_cells(o: Pos, _: &Tuning) -> Scenario {
    let a = chest(o, 1);
    Scenario::new("storage_counts_cells_and_takes_from_the_net", plot_area(o))
        .then("build a storage of four cells on a net", move |b, _| {
            build(b, o)
        })
        .then("fill the source", move |b, _| fill(b, a, COBBLE, 40))
        .until("the storage holds it all", SETTLE, move |b, _| {
            Ok(held(b, o)? == 40)
        })
        .check("four cells, one solid box, their slots", move |b, _| {
            let v = view(b, o)?;
            same("cells", v.cells, 4)?;
            same("shape", v.shape, Shape::Solid)?;
            same(
                "slots",
                v.store.total_slots(),
                4 * u64::from(SLOTS_PER_CELL),
            )?;
            same("left in the source", total(&b.items(a)), 0)
        })
        .check("its record comes back the same from a save", move |b, _| {
            b.round_trip(terminal(o))
        })
}

fn limit_holds(o: Pos, _: &Tuning) -> Scenario {
    let a = chest(o, 1);
    Scenario::new("storage_limit_holds", plot_area(o))
        .then("build a storage on a net", move |b, _| build(b, o))
        .then("limit cobblestone", move |b, _| {
            b.change_limit(terminal(o), COBBLE, LimitChange::Add(LIMIT))
        })
        .then("fill the source", move |b, _| fill(b, a, COBBLE, OFFERED))
        .until("the storage reaches its limit", SETTLE, move |b, _| {
            Ok(held(b, o)? == LIMIT as u64)
        })
        .wait(40)
        .check("it took no more and the rest waits", move |b, _| {
            same("stored", held(b, o)?, LIMIT as u64)?;
            same(
                "in the source",
                count_of(&b.items(a), COBBLE),
                OFFERED - LIMIT as u32,
            )
        })
}

fn destroys_surplus(o: Pos, _: &Tuning) -> Scenario {
    let a = chest(o, 1);
    Scenario::new("storage_destroys_the_surplus", plot_area(o))
        .then("build a storage on a net", move |b, _| build(b, o))
        .then("limit cobblestone and destroy the surplus", move |b, _| {
            b.change_limit(terminal(o), COBBLE, LimitChange::Add(LIMIT))?;
            b.change_limit(terminal(o), COBBLE, LimitChange::ToggleDestroy)
        })
        .then("fill the source", move |b, _| fill(b, a, COBBLE, OFFERED))
        .until("the source is empty", SETTLE, move |b, _| {
            Ok(total(&b.items(a)) == 0)
        })
        .check("it kept its limit", move |b, _| {
            same("stored", held(b, o)?, LIMIT as u64)
        })
}

fn hole_keeps_items(o: Pos, _: &Tuning) -> Scenario {
    let a = chest(o, 1);
    let gone = cells(o)[3];
    Scenario::new("storage_with_a_hole_keeps_its_items", plot_area(o))
        .then("build a storage on a net", move |b, _| build(b, o))
        .then("fill the source", move |b, _| fill(b, a, COBBLE, 20))
        .until("the storage holds it", SETTLE, move |b, _| {
            Ok(held(b, o)? == 20)
        })
        .then("take a corner cell away", move |b, _| {
            b.place(gone, "minecraft:air", &[])
        })
        .then("offer more", move |b, _| fill(b, a, COBBLE, 10))
        .wait(40)
        .check(
            "it names the hole, keeps its items and takes nothing",
            move |b, _| {
                let v = view(b, o)?;
                match &v.shape {
                    Shape::Holes(holes) if holes.contains(&gone) => {}
                    s => return Err(format!("shape {s:?}, expected a hole at {gone:?}")),
                }
                same("stored", v.store.count(COBBLE), 20)?;
                same("in the source", count_of(&b.items(a), COBBLE), 10)
            },
        )
}
