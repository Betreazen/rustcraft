//! Slice 14: the vein's modules and the bore.
//!
//! - With the ore choice module and diamond chosen in its window, the vein grows diamond ore.
//! - With the stratum module and sand in its slots, it grows suspicious sand (ST1).
//! - A bore at the end of its rails bores through the rock ahead and lays the next rail.
//! - W6: a blade and a shield made from the previous tier's keep its charge (and so its number).

use super::{Bench, Build, Scenario, at, plot_area};
use crate::geom::{Face, Pos};
use crate::machines::sheet::Press;
use crate::monorail::shape;
use crate::tuning::Tuning;
use crate::worker::Tool;

pub const ALL: [Build; 4] = [vein_grows_the_chosen_ore, vein_grows_a_stratum, bore_lays_its_way, upgrade_keeps_the_charge];

const STONE: &str = "minecraft:stone";
const VEIN: &str = "rustcraft:vein";
const MONORAIL: &str = "rustcraft:monorail";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a14;

fn floor(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=8 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    Ok(())
}

fn vein(b: &mut dyn Bench, p: Pos, module: &str, rock: &str) -> Result<(), String> {
    b.place(p, VEIN, &[])?;
    b.set_owner(p, OWNER, 1)?;
    b.set_modules(p, &[module])?;
    b.set_buffer(p, 8000.0)?;
    b.put(p, 0, rock, 8)
}

fn grew(b: &dyn Bench, p: Pos, block: &str) -> bool {
    crate::utility::vein::cells().into_iter().any(|c| b.block(at(p, c)) == block)
}

fn vein_grows_the_chosen_ore(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 2, 0]);
    Scenario::new("vein_grows_the_chosen_ore", plot_area(o))
        .then("a vein with the ore choice module, diamond chosen, and cobblestone", move |b, _| {
            floor(b, o)?;
            vein(b, p, "rustcraft:module_pick", "minecraft:cobblestone")?;
            b.press_cursor(p, Press::Toggle(8).id(), None).map(|_| ())
        })
        .until("diamond ore grew beside it", 1400, move |b, _| Ok(grew(b, p, "minecraft:diamond_ore")))
}

fn vein_grows_a_stratum(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 2, 0]);
    Scenario::new("vein_grows_a_stratum", plot_area(o))
        .then("a vein with the stratum module and sand", move |b, _| {
            floor(b, o)?;
            vein(b, p, "rustcraft:module_stratum", "minecraft:sand")
        })
        .until("suspicious sand grew beside it", 2700, move |b, _| Ok(grew(b, p, "minecraft:suspicious_sand")))
}

fn bore_lays_its_way(o: Pos, _: &Tuning) -> Scenario {
    let end = at(o, [4, 1, 0]);
    Scenario::new("worker_bore_lays_its_way", plot_area(o))
        .then("four rails to a wall of stone, a bore with rails and cobblestone in its hold", move |b, m| {
            floor(b, o)?;
            for x in 0..=3 {
                b.place(at(o, [x, 1, 0]), MONORAIL, &[(shape::MOUNT, shape::mount_value(Face::YNeg))])?;
            }
            for x in 4..=6 {
                for y in 1..=2 {
                    b.place(at(o, [x, y, 0]), STONE, &[])?;
                }
            }
            // From the west end its only way is east, so its first dead end is the stone wall
            // (the bore extends any dead end it reaches).
            let id = b.spawn_worker(at(o, [0, 1, 0]), OWNER, Some(Tool::Bore), &[(MONORAIL, 4), ("minecraft:cobblestone", 8)], None)?;
            m.mark("worker", id as f64);
            Ok(())
        })
        .until("a rail lies in the bored rock", 1200, move |b, _| Ok(b.block(end) == MONORAIL))
}

fn upgrade_keeps_the_charge(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("gear_upgrade_keeps_the_charge", plot_area(o)).check(
        "blade_3 from blade_2 and eshield_4 from eshield_3 keep the old piece's charge",
        move |b, _| {
            for (from, alloy, to) in [
                ("rustcraft:blade_2", "rustcraft:crystallite", "rustcraft:blade_3"),
                ("rustcraft:eshield_3", "rustcraft:starsteel", "rustcraft:eshield_4"),
            ] {
                let grid = [None, Some(alloy), None, Some(alloy), Some(from), Some(alloy), None, Some(alloy), None];
                let (made, charge) = b.craft_gear(&grid, 1234.0).ok_or(format!("{from} and four {alloy} make nothing"))?;
                if made != to || charge != Some(1234.0) {
                    return Err(format!("{from} made {made} with charge {charge:?}, not {to} with 1234"));
                }
            }
            Ok(())
        },
    )
}
