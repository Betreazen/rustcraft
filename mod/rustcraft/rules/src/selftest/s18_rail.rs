//! Slice 18, stream «монорельс и работники»: the worker on every rail (W2, guide `tool_reaper`):
//! a reaper climbs a jungle trunk for its cocoa, hangs from a ceiling rail over kelp, rides up a
//! bamboo on a wall rail, and rides a lift from the upper floor to the unload dock below.

use super::{Bench, Build, Memo, Scenario, at, count_of, plot_area, same};
use crate::geom::{Face, Pos};
use crate::monorail::shape;
use crate::tuning::Tuning;
use crate::worker::{Errand, Tool, Worker};

pub const ALL: [Build; 15] = [
    rails_fork_on_a_climb_and_rise_to_a_wall,
    lift_needs_room_for_the_cabin,

    bore_follows_the_ground_with_its_best_rail,

    load_dock_takes_from_a_chest,

    unloads_into_a_portal_dock,

    map_says_all_mapped,

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

fn map_says_all_mapped(o: Pos, _: &Tuning) -> Scenario {
    // Guide `tool_map`: a cartographer shuttling on a short track charts its band once, then,
    // with nothing new round its way, says «Вокруг пути всё нанесено».
    Scenario::new("worker_map_says_all_mapped", plot_area(o))
        .then("a track and a cartographer", move |b, m| {
            floor(b, o, -1..=7, -2..=2)?;
            for x in 0..=6 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            spawn(b, m, at(o, [0, 1, 0]), Tool::Map, &[])
        })
        .until("it says all round its way is mapped", 1200, |b, m| {
            Ok(worker_of(b, m)?.note == crate::worker::Note::AllMapped)
        })
}

fn unloads_into_a_portal_dock(o: Pos, t: &Tuning) -> Scenario {
    // PD2: a cargo worker with a full hold stops at the portal dock beside its rails and the dock
    // takes the load into the slots it sends to its pair from.
    let n = t.cargo_hold as u32;
    let dock = at(o, [6, 1, 1]);
    Scenario::new("worker_unloads_into_a_portal_dock", plot_area(o))
        .then("a track with a portal dock at its end", move |b, _| {
            floor(b, o, -1..=7, -2..=2)?;
            for x in 0..=6 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            b.place(dock, "rustcraft:portal_dock", &[])
        })
        .then("a cargo worker with a full hold", move |b, m| {
            spawn(b, m, at(o, [0, 1, 0]), Tool::Hold, &[("minecraft:wooden_shovel", n)])
        })
        .until("the dock takes its load", RIDE, move |b, _| {
            Ok(count_of(&b.items(dock), "minecraft:wooden_shovel") >= 18)
        })
}

fn load_dock_takes_from_a_chest(o: Pos, _: &Tuning) -> Scenario {
    // Guide `dock_load`: the load dock takes cargo from the chest beside it and loads the cargo
    // worker standing at it.
    let dock = at(o, [0, 1, 1]);
    let chest = at(o, [0, 1, 2]);
    Scenario::new("worker_load_dock_takes_from_a_chest", plot_area(o))
        .then("a track, a load dock with a chest of cobblestone", move |b, _| {
            floor(b, o, -1..=7, -2..=3)?;
            for x in 0..=6 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            b.place(dock, "rustcraft:dock_load", &[])?;
            b.place(chest, "minecraft:chest", &[])?;
            let n = b.insert(chest, "minecraft:cobblestone", 32)?;
            if n == 32 { Ok(()) } else { Err(format!("{n} went into the chest")) }
        })

        .then("a cargo worker", move |b, m| {
            spawn(b, m, at(o, [3, 1, 0]), Tool::Hold, &[])
        })
        .until("the chest's cobblestone reaches its hold", RIDE, |b, m| {
            // It rides off with what the dock had; the dock takes 8 at a time from the chest.
            Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:cobblestone") >= 1)
        })
}

fn bore_follows_the_ground_with_its_best_rail(o: Pos, _: &Tuning) -> Scenario {
    // Guide `tool_bore`: it lays the rails of its hold, here tier 3; without the height module
    // the way follows the ground a step down where it falls away.
    let low = at(o, [4, 0, 0]);
    Scenario::new("worker_bore_follows_the_ground", plot_area(o))
        .then("rails to where the ground steps down, a bore with tier-3 rails", move |b, m| {
            floor(b, o, -1..=3, -2..=2)?;
            for x in 4..=7 {
                b.place(at(o, [x, -1, 0]), STONE, &[])?;
            }
            for x in 0..=3 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            spawn(
                b,
                m,
                at(o, [0, 1, 0]),
                Tool::Bore,
                &[("rustcraft:monorail_3", 4), ("minecraft:cobblestone", 8)],
            )
        })
        .until("a tier-3 rail lies a step down", 1200, move |b, _| {
            Ok(b.block(low) == MONORAIL && b.property(low, "tier").as_deref() == Some("3"))
        })
}

fn rails_fork_on_a_climb_and_rise_to_a_wall(o: Pos, _: &Tuning) -> Scenario {
    // T2 (guide `monorail`): a climb from x 1 (y 1) up to x 2 (y 2) with a branch to the south
    // of its lower cell, and a floor track x 6..7 that ends under wall rails up a stone wall to
    // the south of x 7.
    let climb = at(o, [1, 1, 0]);
    let foot = at(o, [7, 1, 0]);
    let floor_rails = [
        at(o, [0, 1, 0]),
        climb,
        at(o, [2, 2, 0]),
        at(o, [3, 2, 0]),
        at(o, [1, 1, 1]),
        at(o, [6, 1, 0]),
        foot,
    ];
    let wall_rails = [at(o, [7, 2, 0]), at(o, [7, 3, 0])];
    Scenario::new("monorail_climb_forks_and_floor_rises_to_a_wall", plot_area(o))
        .then("a climb with a branch, a floor track under a wall rail", move |b, _| {
            floor(b, o, -1..=8, -2..=2)?;
            b.place(at(o, [2, 1, 0]), STONE, &[])?;
            b.place(at(o, [3, 1, 0]), STONE, &[])?;
            for y in 1..=3 {
                b.place(at(o, [7, y, 1]), STONE, &[])?;
            }
            for p in floor_rails {
                rail(b, p, Face::YNeg)?;
            }
            for p in wall_rails {
                rail(b, p, Face::ZPos)?;
            }
            Ok(())
        })
        .check("the climb shows its fork, the floor rail its piece up the wall", move |b, _| {
            same(
                "the climb's cell",
                b.property(climb, shape::PROPERTY),
                Some(shape::Shape::ClimbFork(0, 0b01).value()),
            )?;
            same(
                "the floor rail under the wall rail",
                b.property(foot, shape::PROPERTY),
                Some(shape::Shape::Rise(1, 0b0110).value()),
            )?;
            same(
                "the wall rail over it",
                b.property(wall_rails[0], shape::PROPERTY),
                Some(shape::Shape::Halves(0b1010).value()),
            )
        })
}

fn lift_needs_room_for_the_cabin(o: Pos, _: &Tuning) -> Scenario {
    // T8.b over every block the cabin touches: a floor track x 0..2 at y 1, a lift up a stone
    // wall at x 3 (wall rails x 2, y 2..5), a top floor rail x 1 at y 5 on a stone deck. The
    // cabin turns along the wall going up and is one block wide: the lift needs only the block
    // over its top wall rail; a stone there refuses the trip at that block.
    use crate::monorail::TripError;
    let from = at(o, [0, 1, -1]);
    let to = at(o, [1, 5, -1]);
    let over_the_lift = at(o, [2, 6, 0]);
    Scenario::new("monorail_lift_needs_room_for_the_cabin", plot_area(o))
        .then("a track up a lift between two stations", move |b, _| {
            floor(b, o, -1..=4, -2..=2)?;
            for x in 0..=2 {
                rail(b, at(o, [x, 1, 0]), Face::YNeg)?;
            }
            for y in 1..=5 {
                b.place(at(o, [3, y, 0]), STONE, &[])?;
            }
            for y in 2..=5 {
                rail(b, at(o, [2, y, 0]), Face::XPos)?;
            }
            b.place(at(o, [1, 4, 0]), STONE, &[])?;
            rail(b, at(o, [1, 5, 0]), Face::YNeg)?;
            b.place(at(o, [1, 4, -1]), STONE, &[])?;
            b.place(from, "rustcraft:station", &[])?;
            b.place(to, "rustcraft:station", &[])?;
            b.set_station_energy(from, 100.0)
        })
        .check("the trip goes up the lift", move |b, _| {
            b.plan_trip(from, to, false)
                .map(|_| ())
                .map_err(|e| format!("refused: {e:?}"))
        })
        .then("a stone over the lift's top rail", move |b, _| {
            b.place(over_the_lift, STONE, &[])
        })
        .check("the trip is refused at that block", move |b, _| {
            same(
                "answer",
                b.plan_trip(from, to, false).map(|t| t.length),
                Err(TripError::NoRoom { at: over_the_lift }),
            )
        })
}
