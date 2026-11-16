//! Slice 18, stream «монорельс и работники»: the worker on every rail (W2, guide `tool_reaper`):
//! a reaper climbs a jungle trunk for its cocoa, hangs from a ceiling rail over kelp, rides up a
//! bamboo on a wall rail, and rides a lift from the upper floor to the unload dock below.

use super::{Bench, Build, Memo, Scenario, at, count_of, plot_area, same};
use crate::geom::{Face, Pos};
use crate::monorail::shape;
use crate::tuning::Tuning;
use crate::worker::{Errand, Tool, Worker};

pub const ALL: [Build; 9] = [
    reaper_filter_pumpkins,
    scoop_filter_sand,

    chassis_swaps_up,
    limit_per_player,


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

fn chassis_swaps_up(o: Pos, _: &Tuning) -> Scenario {
    // U4: a starsteel chassis clicked on a sparksteel worker: 7 sockets, its tool and hold kept,
    // the old chassis back; a lower one afterwards is refused.
    Scenario::new("worker_chassis_swaps_up", plot_area(o))
        .then("a track and a lumberjack with saplings", move |b, m| {
            floor(b, o, -1..=7, -2..=2)?;
            for x in 0..=6 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            spawn(b, m, at(o, [0, 1, 0]), Tool::Saw, &[("minecraft:oak_sapling", 7)])
        })
        .check("a starsteel chassis takes its place", |b, m| {
            let old = b.swap_chassis(m.get("worker") as u64, crate::worker::chassis::ITEMS[2])?;
            same("old chassis back", old.as_str(), crate::worker::chassis::ITEMS[0])?;
            let w = worker_of(b, m)?;
            same("tier", w.tier, 3)?;
            same("sockets", w.upgrades.len(), 7)?;
            same("tool", w.tool, Some(Tool::Saw))?;
            same("saplings", count_of(&w.hold.slots, "minecraft:oak_sapling"), 7)
        })
        .check("a crystallite one after it is refused", |b, m| {
            match b.swap_chassis(m.get("worker") as u64, crate::worker::chassis::ITEMS[1]) {
                Err(_) => same("tier", worker_of(b, m)?.tier, 3),
                Ok(old) => Err(format!("it swapped down and gave back {old}")),
            }
        })
}

/// An owner of its own for the limit's scenario: no other scenario's workers count for it.
const LIMIT_OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0018_0006;

fn limit_per_player(o: Pos, _: &Tuning) -> Scenario {
    // W8: with two workers, a limit of 2 refuses a third, 3 lets it, 0 is no limit.
    Scenario::new("worker_limit_per_player", plot_area(o))
        .then("a track and two workers of one owner", move |b, _| {
            floor(b, o, -1..=7, -2..=2)?;
            for x in 0..=6 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            for x in [0, 6] {
                b.spawn_worker(at(o, [x, 1, 0]), LIMIT_OWNER, Some(Tool::Hold), &[], None)?;
            }
            Ok(())
        })
        .check("the server's limit decides", |b, _| {
            same("limit 2", b.worker_room(LIMIT_OWNER, 2), false)?;
            same("limit 3", b.worker_room(LIMIT_OWNER, 3), true)?;
            same("no limit", b.worker_room(LIMIT_OWNER, 0), true)?;
            same("another owner", b.worker_room(LIMIT_OWNER + 1, 1), true)
        })
}

/// A straight floor track x 0..=6 at y 1 on stone, and a worker of `tool` with `filter` at its
/// start.
fn filtered(
    name: &'static str,
    o: Pos,
    tool: Tool,
    filter: &'static [&'static str],
    build: impl FnMut(&mut dyn Bench, &mut Memo) -> Result<(), String> + Send + 'static,
) -> Scenario {
    Scenario::new(name, plot_area(o))
        .then("a track", move |b, _| {
            floor(b, o, -1..=7, -3..=3)?;
            for x in 0..=6 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            Ok(())
        })
        .then("what stands beside it", build)
        .then("a worker with a filter", move |b, m| {
            spawn(b, m, at(o, [0, 1, 0]), tool, &[])?;
            b.set_worker_filter(m.get("worker") as u64, filter)
        })
}

fn reaper_filter_pumpkins(o: Pos, _: &Tuning) -> Scenario {
    // H6: a reaper filtered to pumpkins takes the pumpkin and leaves the ripe wheat.
    let wheat = at(o, [2, 1, 1]);
    let fruit = at(o, [4, 1, 1]);
    let stem = at(o, [4, 1, 2]);
    filtered("worker_reaper_filter_pumpkins", o, Tool::Reaper, &["minecraft:pumpkin"], move |b, _| {
        b.place(at(wheat, [0, -1, 0]), "minecraft:farmland", &[])?;
        b.place(wheat, "minecraft:wheat", &[("age", "7")])?;
        b.place(at(stem, [0, -1, 0]), "minecraft:farmland", &[])?;
        b.place(fruit, "minecraft:pumpkin", &[])?;
        b.place(stem, "minecraft:attached_pumpkin_stem", &[("facing", "north")])
    })
    .until("it takes the pumpkin", RIDE, |b, m| {
        Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:pumpkin") >= 1)
    })
    .wait(100)
    .check("the ripe wheat stays", move |b, m| {
        same("wheat age", b.property(wheat, "age"), Some("7".to_owned()))?;
        same("wheat taken", count_of(&worker_of(b, m)?.hold.slots, "minecraft:wheat"), 0)
    })
}

fn scoop_filter_sand(o: Pos, _: &Tuning) -> Scenario {
    // The scoop filtered to sand digs the sand beside its way and leaves the dirt.
    let sand = at(o, [2, 0, 1]);
    let dirt = at(o, [4, 0, 1]);
    filtered("worker_scoop_filter_sand", o, Tool::Scoop, &["minecraft:sand"], move |b, _| {
        // Stone under both, so the sand does not fall.
        b.place(at(sand, [0, -1, 0]), STONE, &[])?;
        b.place(at(dirt, [0, -1, 0]), STONE, &[])?;
        b.place(sand, "minecraft:sand", &[])?;

        b.place(dirt, "minecraft:dirt", &[])
    })
    .until("it digs the sand", RIDE, |b, m| {
        Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:sand") >= 1)
    })
    .wait(100)
    .check("the dirt stays", move |b, m| {
        same("dirt", b.block(dirt), "minecraft:dirt".to_owned())?;
        same("dirt taken", count_of(&worker_of(b, m)?.hold.slots, "minecraft:dirt"), 0)
    })
}
