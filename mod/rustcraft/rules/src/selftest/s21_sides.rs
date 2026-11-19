//! 0.21 (spec 40): the side walls show whether a block works.
//!
//! - An autocrafter on two crystals with a pattern of a log and two logs tells clients it works
//!   while it crafts (its lamps light on the top and on every side), and that it stands once the
//!   logs are used up.
//! - A storage of four cells in a solid box with a terminal, on an item net: every cell and the
//!   terminal tell clients the storage works (the cells' green lamps, the terminal's scrolling
//!   lines), the cells an empty fill. A stack of cobblestone and one more brought in by the net
//!   take two slots: the cells' bars rise to two of the storage's 108 slots, rounded up. A corner cell taken away (S8: no solid
//!   box): the cells left and the terminal stand, the fill now a share of fewer slots.

use super::transport::{COBBLE, chest, fill, item_line, joint};
use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::anim::{Shown, storage_shown};
use crate::geom::Pos;
use crate::machines::windows::crafter::CELL_BUTTON;
use crate::net::items::Mode;
use crate::storage::SLOTS_PER_CELL;
use crate::tuning::Tuning;

pub const ALL: [Build; 2] = [autocrafter_shows_its_work, storage_shows_its_state];

const CRAFTER: &str = "rustcraft:crafter";
const CRYSTAL: &str = "rustcraft:crystal";
const CELL: &str = "rustcraft:cell";
const TERMINAL: &str = "rustcraft:terminal";
const LOG: &str = "minecraft:oak_log";
const LIMIT: u64 = 400;
/// A stack and one more: two of the storage's slots.
const BROUGHT: u32 = 65;
const SLOTS_USED: u64 = 2;

fn shown(b: &dyn Bench, pos: Pos) -> Result<Shown, String> {
    b.anim(pos).ok_or_else(|| format!("no block entity at {pos:?}"))
}

fn autocrafter_shows_its_work(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("sides21_autocrafter_works_then_stands", plot_area(o))
        .then("an autocrafter on two crystals with a pattern of a log and two logs", move |b, _| {
            b.place(o, CRAFTER, &[])?;
            b.place(at(o, [1, 0, 0]), CRYSTAL, &[])?;
            b.place(at(o, [-1, 0, 0]), CRYSTAL, &[])?;
            b.press(o, CELL_BUTTON + 4, Some((LOG, 1)))?;
            let slot = b.inputs(o).first().copied().ok_or("the autocrafter has no input")?;
            b.put(o, slot, LOG, 2)
        })
        .until("it tells clients it works", LIMIT, move |b, _| Ok(shown(b, o)?.on))
        .until("the logs used up, it tells them it stands", LIMIT, move |b, _| Ok(!shown(b, o)?.on))
}

/// The cells: a 2×1×2 box on the z+ side of housings 3 and 4, as the storage's scenarios have it.
fn cells(o: Pos) -> [Pos; 4] {
    [at(o, [3, 0, 1]), at(o, [4, 0, 1]), at(o, [3, 0, 2]), at(o, [4, 0, 2])]
}

fn terminal(o: Pos) -> Pos {
    at(o, [5, 0, 1])
}

/// Every one of `blocks` shows `want`.
fn all_show(b: &dyn Bench, blocks: &[Pos], want: Shown) -> Result<bool, String> {
    for &p in blocks {
        if shown(b, p)? != want {
            return Ok(false);
        }
    }
    Ok(true)
}

fn storage_shows_its_state(o: Pos, _: &Tuning) -> Scenario {
    let a = chest(o, 1);
    let gone = cells(o)[3];
    let four = u64::from(4 * SLOTS_PER_CELL);
    let three = u64::from(3 * SLOTS_PER_CELL);
    Scenario::new("sides21_storage_shows_its_state", plot_area(o))
        .then("a storage of four cells with a terminal on an item net", move |b, _| {
            item_line(b, o, 2, 4, &[1])?;
            joint(b, o, 1, Mode::From, 0)?;
            for c in cells(o) {
                b.place(c, CELL, &[])?;
            }
            b.place(terminal(o), TERMINAL, &[])
        })
        .until("the cells and the terminal show it works, empty", LIMIT, move |b, _| {
            let empty = storage_shown(true, 0, four);
            Ok(all_show(b, &cells(o), empty)? && shown(b, terminal(o))? == empty)
        })
        .then("65 cobblestone in the net's chest", move |b, _| fill(b, a, COBBLE, BROUGHT))
        .until("the cells' bars show two of 108 slots in use", LIMIT, move |b, _| {
            all_show(b, &cells(o), storage_shown(true, SLOTS_USED, four))
        })
        .check("the terminal shows no fill of its own", move |b, _| {
            same("the terminal", shown(b, terminal(o))?, storage_shown(true, 0, four))
        })
        .then("a corner cell taken away", move |b, _| b.place(gone, "minecraft:air", &[]))
        .until("the cells left and the terminal stand", LIMIT, move |b, _| {
            let left = &cells(o)[..3];
            Ok(all_show(b, left, storage_shown(false, SLOTS_USED, three))?
                && shown(b, terminal(o))? == storage_shown(false, 0, three))
        })
}
