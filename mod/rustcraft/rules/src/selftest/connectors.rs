//! Round 31: a machine shows a connector («присоска») on each side a housing touches
//! (crate::connectors). The server sets the side's state property when a housing comes and clears
//! it when the housing goes; the sides are the world's, whichever way the machine faces, and a
//! node, which joins no machine, shows nothing. Round 32: a housing below sets the block entity's
//! bottom flag instead of a state property (crate::connectors::BOTTOM_KEY), the sides stay.

use super::{Bench, Build, Scenario, at, plot_area};
use crate::connectors::{BLOCKS, FACES};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 2] = [a_crusher_shows_the_housings_beside_it, every_block_with_connectors_has_its_sides];

const CRUSHER: &str = "rustcraft:crusher";
const HOUSING: &str = "rustcraft:housing";
const NODE: &str = "rustcraft:node";
/// Ticks the server may take to read the magistral again and set the sides (once a second).
const SETTLE: u64 = 60;

/// Whether the block at `pos` shows connectors on `want` and on no other side; an error names a
/// side the block has no property for.
fn shows(b: &dyn Bench, pos: Pos, want: &[&str]) -> Result<bool, String> {
    for (side, _) in FACES {
        let wanted = if want.contains(&side) { "true" } else { "false" };
        match b.property(pos, side) {
            None => return Err(format!("{} at {pos:?} has no side {side}", b.block(pos))),
            Some(v) if v != wanted => return Ok(false),
            Some(_) => {}
        }
    }
    Ok(true)
}

fn a_crusher_shows_the_housings_beside_it(o: Pos, _: &Tuning) -> Scenario {
    let crusher = at(o, [2, 0, 2]);
    let east = at(crusher, [1, 0, 0]);
    let above = at(crusher, [0, 1, 0]);
    let south = at(crusher, [0, 0, 1]);
    let below = at(crusher, [0, -1, 0]);
    Scenario::new("connectors_follow_the_housings", plot_area(o))
        .then("a crusher facing west, a housing east of it and a node south of it", move |b, _| {
            b.place(crusher, CRUSHER, &[("facing", "west")])?;
            b.place(east, HOUSING, &[])?;
            b.place(south, NODE, &[])
        })
        .until("the crusher shows a connector east, none for the node", SETTLE, move |b, _| {
            shows(b, crusher, &["east"])
        })
        .then("a housing on top of it too", move |b, _| b.place(above, HOUSING, &[]))
        .until("east and up", SETTLE, move |b, _| shows(b, crusher, &["east", "up"]))
        .then("the east housing broken", move |b, _| b.break_block(east, None).map(|_| ()))
        .until("only up is left", SETTLE, move |b, _| shows(b, crusher, &["up"]))
        .then("a housing under it", move |b, _| b.place(below, HOUSING, &[]))
        .until("its block entity shows the bottom, the sides stay up only", SETTLE, move |b, _| {
            Ok(bottom(b, crusher)? && shows(b, crusher, &["up"])?)
        })
        .then("the housing under it broken", move |b, _| b.break_block(below, None).map(|_| ()))
        .until("the bottom is gone", SETTLE, move |b, _| Ok(!bottom(b, crusher)?))
}

/// Round 32: whether the block entity at `pos` shows the bottom connector; an error when the
/// block has no block entity to hold it.
fn bottom(b: &dyn Bench, pos: Pos) -> Result<bool, String> {
    b.bottom_connector(pos).ok_or_else(|| format!("{} at {pos:?} has no block entity", b.block(pos)))
}

/// Every block of `BLOCKS` declares the five sides in the game: a block left out of the content
/// declarations would name sides in its blockstate that it does not have, and never show them.
/// Round 32: and has a block entity, which holds its bottom connector.
fn every_block_with_connectors_has_its_sides(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("connectors_every_block_has_its_sides", plot_area(o)).then(
        "each block placed, its sides read and taken away",
        move |b, _| {
            let mut lacking = Vec::new();
            for (i, block) in BLOCKS.iter().enumerate() {
                let pos = at(o, [i as i32, 0, 0]);
                b.place(pos, block, &[])?;
                let missing: Vec<&str> = FACES
                    .iter()
                    .map(|(side, _)| *side)
                    .filter(|side| b.property(pos, side).as_deref() != Some("false"))
                    .collect();
                if !missing.is_empty() {
                    lacking.push(format!("{block}: {missing:?}"));
                }
                if b.bottom_connector(pos) != Some(false) {
                    lacking.push(format!("{block}: no block entity for the bottom, or set at first"));
                }
                b.place(pos, "minecraft:air", &[])?;
            }
            if lacking.is_empty() { Ok(()) } else { Err(format!("sides missing or not false at first: {lacking:?}")) }
        },
    )
}
