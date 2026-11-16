//! Slice 18, stream «монорельс и работники»: the worker on every rail (W2, guide `tool_reaper`):
//! a reaper climbs a jungle trunk for its cocoa, hangs from a ceiling rail over kelp, rides up a
//! bamboo on a wall rail, and rides a lift from the upper floor to the unload dock below.

use super::{Bench, Build, Memo, Scenario, at, count_of, plot_area, same};
use crate::geom::{Face, Pos};
use crate::monorail::shape;
use crate::tuning::Tuning;
use crate::worker::{Errand, Tool, Worker};

pub const ALL: [Build; 5] = [
    reaper_climbs_a_trunk,
    reaper_hangs_over_kelp,
    reaper_rides_up_bamboo,
    rides_a_lift_to_the_dock_below,
    picked_up_as_one_item,
];

const MONORAIL: &str = "rustcraft:monorail";
const STONE: &str = "minecraft:stone";
const DOCK_UNLOAD: &str = "rustcraft:dock_unload";
/// The owner of the scenarios' workers (as in `workers.rs`).
pub const OWNER: u128 = super::workers::OWNER;
/// Generous: up and down a short stretch at the slowest speed.
const RIDE: u64 = 900;

fn rail(b: &mut dyn Bench, p: Pos, mount: Face) -> Result<(), String> {
    b.place(p, MONORAIL, &[(shape::MOUNT, shape::mount_value(mount))])
}

fn floor(b: &mut dyn Bench, o: Pos, x: std::ops::RangeInclusive<i32>, z: std::ops::RangeInclusive<i32>) -> Result<(), String> {
    for x in x {
        for z in z.clone() {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    Ok(())
}

pub fn worker_of(b: &dyn Bench, m: &Memo) -> Result<Worker, String> {
    b.worker(m.get("worker") as u64)
        .ok_or_else(|| "the worker is gone".to_owned())
}

fn spawn(
    b: &mut dyn Bench,
    m: &mut Memo,
    cell: Pos,
    tool: Tool,
    hold: &[(&str, u32)],
) -> Result<(), String> {
    let id = b.spawn_worker(cell, OWNER, Some(tool), hold, None)?;
    m.mark("worker", id as f64);
    Ok(())
}

fn reaper_climbs_a_trunk(o: Pos, _: &Tuning) -> Scenario {
    // A jungle trunk at x 3, z 0, y 1..6; rails up its south face (z 1, hanging on the trunk);
    // a ripe pod on its east face at y 4, out of reach of any floor rail.
    let trunk = move |y: i32| at(o, [3, y, 0]);
    let pod = at(o, [4, 4, 0]);
    Scenario::new("worker_reaper_climbs_a_trunk", plot_area(o))
        .then("a trunk with rails up its face and a pod on its side", move |b, _| {
            floor(b, o, 0..=6, -2..=2)?;
            for y in 1..=6 {
                b.place(trunk(y), "minecraft:jungle_log", &[])?;
            }
            for y in 1..=5 {
                rail(b, at(o, [3, y, 1]), Face::ZNeg)?;
            }
            b.place(pod, "minecraft:cocoa", &[("age", "2"), ("facing", "west")])
        })
        .then("a reaper at the foot of the trunk", move |b, m| {
            spawn(b, m, at(o, [3, 1, 1]), Tool::Reaper, &[])
        })
        .until("it climbs and picks the pod", RIDE, move |b, m| {
            Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:cocoa_beans") >= 1)
        })
        .check("the pod is planted again on its trunk", move |b, _| {
            same("pod", b.block(pod), "minecraft:cocoa".to_owned())?;
            same("its age", b.property(pod, "age"), Some("0".to_owned()))
        })
}

fn reaper_hangs_over_kelp(o: Pos, _: &Tuning) -> Scenario {
    // Ceiling rails at y 4 under a stone roof at y 5; kelp from the floor at y 1 to y 3 beside
    // them, deeper than any floor rail's zone reaches up.
    let kelp = move |y: i32| at(o, [3, y, 1]);
    Scenario::new("worker_reaper_hangs_over_kelp", plot_area(o))
        .then("a roof with ceiling rails and kelp under it", move |b, _| {
            floor(b, o, 0..=6, -2..=2)?;
            for x in 0..=6 {
                b.place(at(o, [x, 5, 0]), STONE, &[])?;
            }
            for x in 0..=6 {
                rail(b, at(o, [x, 4, 0]), Face::YPos)?;
            }
            b.place(kelp(1), "minecraft:kelp_plant", &[])?;
            b.place(kelp(2), "minecraft:kelp_plant", &[])?;
            b.place(kelp(3), "minecraft:kelp", &[])
        })
        .then("a reaper hanging from the roof", move |b, m| {
            spawn(b, m, at(o, [0, 4, 0]), Tool::Reaper, &[])
        })
        .until("it cuts the kelp under it", RIDE, move |b, m| {
            Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:kelp") >= 1)
        })
        .check("the kelp's base stays", move |b, _| {
            let base = b.block(kelp(1));
            if base.starts_with("minecraft:kelp") {
                Ok(())
            } else {
                Err(format!("the base is {base}"))
            }
        })
}

fn reaper_rides_up_bamboo(o: Pos, _: &Tuning) -> Scenario {
    // A stone wall at z -1, rails up it at x 3, z 0, y 1..7; a bamboo 6 high beside them at z 1.
    let bamboo = move |y: i32| at(o, [3, y, 1]);
    Scenario::new("worker_reaper_rides_up_bamboo", plot_area(o))
        .then("a wall with rails up it and a tall bamboo", move |b, _| {
            floor(b, o, 0..=6, -2..=2)?;
            b.place(at(o, [3, 0, 1]), "minecraft:dirt", &[])?;
            for y in 1..=7 {
                b.place(at(o, [3, y, -1]), STONE, &[])?;
                rail(b, at(o, [3, y, 0]), Face::ZNeg)?;
            }
            for y in 1..=6 {
                b.place(bamboo(y), "minecraft:bamboo", &[])?;
            }
            Ok(())
        })
        .then("a reaper at the foot of the wall", move |b, m| {
            spawn(b, m, at(o, [3, 1, 0]), Tool::Reaper, &[])
        })
        .until("it cuts the bamboo from its top down", RIDE, move |b, m| {
            Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:bamboo") >= 5)
        })
        .check("only the base stays, nothing fell", move |b, _| {
            same("base", b.block(bamboo(1)), "minecraft:bamboo".to_owned())?;
            same("over it", b.block(bamboo(2)), "minecraft:air".to_owned())?;
            let fell = b.lying(
                at(o, [-2, 0, -3]).map(f64::from),
                at(o, [9, 9, 4]).map(f64::from),
            );
            if fell.iter().any(|(i, _)| i == "minecraft:bamboo") {
                return Err(format!("bamboo fell to the ground: {fell:?}"));
            }
            Ok(())
        })
}

fn rides_a_lift_to_the_dock_below(o: Pos, t: &Tuning) -> Scenario {
    // Floor 1: rails x 0..6 at y 1. A lift up a wall at x 7: rails x 6, y 2..5. Floor 2: rails
    // x 0..5 at y 5 on a stone deck at y 4. The unload dock is beside floor 1's far end.
    let n = t.worker_hold as u32;
    let dock = at(o, [0, 1, 1]);
    Scenario::new("worker_rides_a_lift_to_the_dock_below", plot_area(o))
        .then("two floors joined by a lift, a dock below", move |b, _| {
            floor(b, o, -1..=7, -2..=2)?;
            for x in 0..=6 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            for y in 2..=5 {
                b.place(at(o, [7, y, 0]), STONE, &[])?;
                rail(b, at(o, [6, y, 0]), Face::XPos)?;
            }
            for x in 0..=5 {
                b.place(at(o, [x, 4, 0]), STONE, &[])?;
                rail(b, at(o, [x, 5, 0]), Face::YNeg)?;
            }
            b.place(dock, DOCK_UNLOAD, &[])
        })
        .then("a reaper upstairs with a full hold", move |b, m| {
            spawn(b, m, at(o, [0, 5, 0]), Tool::Reaper, &[("minecraft:wooden_shovel", n)])
        })
        .until("it rides down the lift and unloads", 1800, move |b, _| {
            Ok(count_of(&b.items(dock), "minecraft:wooden_shovel") > 0)
        })
        .check("it unloads on its errand", |b, m| {
            same("errand", worker_of(b, m)?.errand, Errand::Unload)
        })
}

fn picked_up_as_one_item(o: Pos, t: &Tuning) -> Scenario {
    // W3: a reaper with a storage module, seeds in its hold and half a battery, taken away as one
    // chassis item and placed again on another rail: the same worker.
    let charge = t.worker_battery / 2.0;
    Scenario::new("worker_picked_up_as_one_item", plot_area(o))
        .then("a track and a reaper with a load", move |b, m| {
            floor(b, o, -1..=7, -2..=2)?;
            for x in 0..=6 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            let id = b.spawn_worker(
                at(o, [0, 1, 0]),
                OWNER,
                Some(Tool::Reaper),
                &[("minecraft:wheat_seeds", 40), ("minecraft:wheat", 12)],
                Some(charge),
            )?;
            m.mark("first", id as f64);
            Ok(())
        })
        .then("taken away as one item and placed at the far end", move |b, m| {
            let id = b.worker_as_item(m.get("first") as u64, at(o, [6, 1, 0]))?;
            m.mark("worker", id as f64);
            Ok(())
        })
        .check("it came back with all it carried", move |b, m| {
            let w = worker_of(b, m)?;
            same("tool", w.tool, Some(Tool::Reaper))?;
            same("seeds", count_of(&w.hold.slots, "minecraft:wheat_seeds"), 40)?;
            same("wheat", count_of(&w.hold.slots, "minecraft:wheat"), 12)?;
            same("cell", w.cell, at(o, [6, 1, 0]))?;
            if (w.charge - charge).abs() > 5.0 {
                return Err(format!("charge {} instead of {charge}", w.charge));
            }
            if b.worker(m.get("first") as u64).is_some() {
                return Err("the first worker is still there".into());
            }
            Ok(())
        })
}
