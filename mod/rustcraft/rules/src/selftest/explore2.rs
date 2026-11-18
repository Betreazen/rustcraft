//! After 0.15: what the living vein grows for archaeology (spec 24, ST4, ST5).
//!
//! - The vein with the stratum module grows suspicious sand on the floor beside a track; a grab
//!   riding past takes it whole into its hold, marked as grown (ST4).
//! - The archaeology table brushes a grown block (as a hold gives it out) without telling
//!   Path-of-craft-tree `brush`, and a block found in the world with it (ST5).
//!
//! The cartographer's structures (EX1) have no scenario: the engine finds a structure from the
//! starts its world generator placed (structures.rs:9), which the bench's plots never hold, and
//! `/place structure` builds the blocks without a start. The rules' unit tests cover them.

use super::{Bench, Build, Scenario, at, base, count_of, plot_area};
use crate::geom::{Face, Pos};
use crate::monorail::shape;
use crate::tuning::Tuning;
use crate::utility::grown;
use crate::worker::Tool;

pub const ALL: [Build; 2] = [grab_takes_a_grown_block, archeo_grown_gives_no_brush];

const STONE: &str = "minecraft:stone";
const MONORAIL: &str = "rustcraft:monorail";
const SUSPICIOUS: &str = "minecraft:suspicious_sand";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0e16;
/// The table whose block is a grown one, and the table whose block was found in the world.
const GROWN_OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0001_0e16;
const FOUND_OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0002_0e16;
/// ST3: a stratum block in 120 s; room for one more step and the stages before it.
const GROW: u64 = 2700;
/// The worker rides the track there and back at 2 blocks a second.
const RIDE: u64 = 600;
const BRUSH: u64 = 400;

fn floor(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=8 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    Ok(())
}

fn machine(b: &mut dyn Bench, pos: Pos, block: &str, owner: u128, buffer: f64) -> Result<(), String> {
    b.place(pos, block, &[])?;
    b.set_owner(pos, owner, 1)?;
    b.set_buffer(pos, buffer)
}

fn grab_takes_a_grown_block(o: Pos, _: &Tuning) -> Scenario {
    let vein = at(o, [3, 1, 2]);
    // The vein's nearest free cells are (2,1,2) and (4,1,2), stone here, then (3,0,2), the floor;
    // the next is this one, in the grab's zone (one cell aside of the track).
    let cell = at(o, [3, 1, 1]);
    Scenario::new("vein_grown_block_rides_the_grab", plot_area(o))
        .then("a track, and a vein with the stratum module and sand beside it", move |b, _| {
            floor(b, o)?;
            for x in 0..=6 {
                b.place(at(o, [x, 1, 0]), MONORAIL, &[(shape::MOUNT, shape::mount_value(Face::YNeg))])?;
            }
            base(b, Some(at(o, [0, 1, 0])), Some(at(o, [6, 1, 0])))?;
            b.place(at(o, [2, 1, 2]), STONE, &[])?;
            b.place(at(o, [4, 1, 2]), STONE, &[])?;
            machine(b, vein, "rustcraft:vein", OWNER, 8000.0)?;
            b.set_modules(vein, &["rustcraft:module_stratum"])?;
            b.put(vein, 0, "minecraft:sand", 8)
        })
        .until("suspicious sand grew beside the track", GROW, move |b, _| Ok(b.block(cell) == SUSPICIOUS))
        .then("a grab on the track", move |b, m| {
            let id = b.spawn_worker(at(o, [0, 1, 0]), OWNER, Some(Tool::Grab), &[], None)?;
            m.mark("worker", id as f64);
            Ok(())
        })
        .until("the grab holds it whole, marked as grown", RIDE, move |b, m| {
            let w = b.worker(m.get("worker") as u64).ok_or("the worker is gone")?;
            // Wave 2: the hold name carries the vein's find table too (grown::hold_name_with).
            Ok(w.hold.slots.iter().flatten().any(|(name, n)| *n > 0 && grown::from_hold(name) == (SUSPICIOUS, true)))
        })
}

fn archeo_grown_gives_no_brush(o: Pos, _: &Tuning) -> Scenario {
    let grown_table = at(o, [2, 1, 0]);
    let found_table = at(o, [6, 1, 0]);
    let brushed = move |b: &dyn Bench, owner: u128| {
        let head = format!("pct-machine brush {owner:032x} ");
        b.reports().iter().any(|r| r.starts_with(&head))
    };
    Scenario::new("util_archeo_grown_gives_no_brush", plot_area(o))
        .then("two tables with a brush: one with grown suspicious sand, one with sand found in the world", move |b, _| {
            floor(b, o)?;
            machine(b, grown_table, "rustcraft:archeo", GROWN_OWNER, 1000.0)?;
            b.put_grown(grown_table, 0, SUSPICIOUS, 1)?;
            b.put(grown_table, 1, "minecraft:brush", 1)?;
            machine(b, found_table, "rustcraft:archeo", FOUND_OWNER, 1000.0)?;
            b.put(found_table, 0, SUSPICIOUS, 1)?;
            b.put(found_table, 1, "minecraft:brush", 1)
        })
        .until("both gave their sand back", BRUSH, move |b, _| {
            Ok(count_of(&b.items(grown_table), "minecraft:sand") == 1
                && count_of(&b.items(found_table), "minecraft:sand") == 1)
        })
        .check("the grown block's owner was told no brush, the found block's was", move |b, _| {
            match (brushed(b, GROWN_OWNER), brushed(b, FOUND_OWNER)) {
                (false, true) => Ok(()),
                (grown, found) => Err(format!("brush told: grown {grown}, found {found}; {:?}", b.reports())),
            }
        })
}
