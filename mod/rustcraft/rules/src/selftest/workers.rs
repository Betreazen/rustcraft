//! Slice 7: the worker cart. A lumberjack fells a tree beside its rails and plants a sapling
//! again; a reaper picks each group of plants by its rule (spec 17); a full hold rides to the
//! unload dock and goes on into a chest by the magistral (W4.a, W6); a low battery rides to the
//! charge dock (W4.b); with no dock on its rails it stops and says so (W4.c). The saw's and the
//! reaper's breaks are reported to Path-of-craft-tree (X1, the contract's `break`).

use super::magistral::{channel, place_node};
use super::{Bench, Build, Memo, Scenario, at, count_of, plot_area, same};
use crate::geom::{Face, Pos};
use crate::monorail::shape;
use crate::net::topology::ChannelKind;
use crate::tuning::Tuning;
use crate::worker::{Errand, Note, Tool};

pub const ALL: [Build; 11] = [
    saw_fells_a_tree,
    reaper_aged_crops,
    reaper_columns,
    reaper_stem_fruits,
    reaper_berries,
    reaper_on_a_wall,
    reaper_nothing_to_sow,
    unloads_into_a_chest_by_the_magistral,
    charges_at_the_dock,
    stops_without_a_dock,
    fleet_survives_a_save,
];

const MONORAIL: &str = "rustcraft:monorail";
const DOCK_UNLOAD: &str = "rustcraft:dock_unload";
const DOCK_CHARGE: &str = "rustcraft:dock_charge";
const HOUSING: &str = "rustcraft:housing";
const CRYSTAL: &str = "rustcraft:crystal";
const STONE: &str = "minecraft:stone";
const FARMLAND: &str = "minecraft:farmland";
/// The owner the scenarios' workers report for: nobody on the server, so Path-of-craft-tree
/// refuses the report in its log, but it is sent.
pub const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0007;
/// Rails x 0..=TRACK one block over the floor; plants beside x = SPOT.
const TRACK: i32 = 6;
const SPOT: i32 = 3;
/// Generous limits: the worker rides there and back at 2 blocks a second.
const RIDE: u64 = 600;

fn rail(o: Pos, x: i32) -> Pos {
    at(o, [x, 1, 0])
}

/// A floor of stone and a straight track of floor rails on it.
fn track(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=TRACK + 1 {
        for z in -2..=2 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    for x in 0..=TRACK {
        b.place(
            rail(o, x),
            MONORAIL,
            &[(shape::MOUNT, shape::mount_value(Face::YNeg))],
        )?;
    }
    Ok(())
}

fn worker_of(b: &dyn Bench, m: &Memo) -> Result<crate::worker::Worker, String> {
    b.worker(m.get("worker") as u64)
        .ok_or_else(|| "the worker is gone".to_owned())
}

/// A track, then a worker with `tool` and `hold` at its start.
fn with_worker(
    name: &'static str,
    o: Pos,
    build: impl FnMut(&mut dyn Bench, &mut Memo) -> Result<(), String> + Send + 'static,
    tool: Option<Tool>,
    hold: Vec<(&'static str, u32)>,
) -> Scenario {
    Scenario::new(name, plot_area(o))
        .then("the track and what stands beside it", move |b, _| {
            track(b, o)
        })
        .then("the plants", build)
        .then("a worker on the track", move |b, m| {
            let id = b.spawn_worker(rail(o, 0), OWNER, tool, &hold, None)?;
            m.mark("worker", id as f64);
            Ok(())
        })
}

/// Whether some sent report breaks `block` with `tool`, and how many.
fn breaks(b: &dyn Bench, block: &str, tool: &str) -> usize {
    let owner = format!("{OWNER:032x}");
    b.reports()
        .iter()
        .filter(|l| {
            l.contains(&format!("break {owner} ")) && l.ends_with(&format!("{block} {tool}"))
        })
        .count()
}

fn saw_fells_a_tree(o: Pos, _: &Tuning) -> Scenario {
    let root = at(o, [SPOT, 1, 1]);
    with_worker(
        "worker_saw_fells_a_tree",
        o,
        move |b, _| {
            b.place(at(root, [0, -1, 0]), "minecraft:dirt", &[])?;
            for y in 0..4 {
                b.place(at(root, [0, y, 0]), "minecraft:oak_log", &[])?;
            }
            for (x, z) in [(1, 0), (-1, 0), (0, 1)] {
                b.place(
                    at(root, [x, 3, z]),
                    "minecraft:oak_leaves",
                    &[("persistent", "false")],
                )?;
            }
            b.place(
                at(root, [0, 4, 0]),
                "minecraft:oak_leaves",
                &[("persistent", "false")],
            )
        },
        Some(Tool::Saw),
        vec![("minecraft:oak_sapling", 2)],
    )
    .until("the logs are in the hold", RIDE, move |b, m| {
        Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:oak_log") >= 4)
    })
    .check(
        "the tree is gone and a sapling stands on its root",
        move |b, _| {
            for y in 1..4 {
                same(
                    "block over the root",
                    b.block(at(root, [0, y, 0])),
                    "minecraft:air".to_owned(),
                )?;
            }
            same(
                "leaves",
                b.block(at(root, [0, 4, 0])),
                "minecraft:air".to_owned(),
            )?;
            same("root", b.block(root), "minecraft:oak_sapling".to_owned())
        },
    )
    // The leaves may drop saplings of their own (the base game's chance), so only the planted
    // one is certain: at least one of the two stays.
    .check(
        "saplings stay in the hold for the next tree",
        move |b, m| {
            let n = count_of(&worker_of(b, m)?.hold.slots, "minecraft:oak_sapling");
            if n >= 1 {
                Ok(())
            } else {
                Err(format!("saplings {n}, expected at least 1"))
            }
        },
    )
    .until("each log is reported as an axe's break", 40, |b, _| {
        Ok(breaks(b, "minecraft:oak_log", "axe") >= 4)
    })
}

/// A plant group's scenario: `build` puts plants beside x = SPOT, the reaper rides past, `check`
/// looks at what it did.
fn reaper(
    name: &'static str,
    o: Pos,
    build: impl FnMut(&mut dyn Bench, &mut Memo) -> Result<(), String> + Send + 'static,
    hold: Vec<(&'static str, u32)>,
    done: impl FnMut(&mut dyn Bench, &mut Memo) -> Result<bool, String> + Send + 'static,
) -> Scenario {
    with_worker(name, o, build, Some(Tool::Reaper), hold).until("the reaper rode past", RIDE, done)
}

fn reaper_aged_crops(o: Pos, _: &Tuning) -> Scenario {
    let ripe = at(o, [SPOT, 1, 1]);
    let young = at(o, [SPOT + 1, 1, 1]);
    reaper(
        "worker_reaper_aged_crops",
        o,
        move |b, _| {
            b.place(at(ripe, [0, -1, 0]), FARMLAND, &[])?;
            b.place(at(young, [0, -1, 0]), FARMLAND, &[])?;
            b.place(ripe, "minecraft:wheat", &[("age", "7")])?;
            b.place(young, "minecraft:wheat", &[("age", "3")])
        },
        Vec::new(),
        move |b, m| Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:wheat") >= 1),
    )
    .check(
        "the ripe wheat is sown again, the young stays",
        move |b, _| {
            same("ripe cell", b.block(ripe), "minecraft:wheat".to_owned())?;
            same("its age", b.property(ripe, "age"), Some("0".to_owned()))?;
            same("young age", b.property(young, "age"), Some("3".to_owned()))
        },
    )
    .until("the wheat is reported as a hoe's break", 40, |b, _| {
        Ok(breaks(b, "minecraft:wheat", "hoe") >= 1)
    })
}

fn reaper_columns(o: Pos, _: &Tuning) -> Scenario {
    let base = at(o, [SPOT, 1, 1]);
    reaper(
        "worker_reaper_columns",
        o,
        move |b, _| {
            b.place(at(base, [0, -1, 0]), "minecraft:dirt", &[])?;
            for y in 0..3 {
                b.place(at(base, [0, y, 0]), "minecraft:bamboo", &[])?;
            }
            Ok(())
        },
        Vec::new(),
        move |b, m| Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:bamboo") >= 2),
    )
    .check("the column is cut over its base", move |b, _| {
        same("base", b.block(base), "minecraft:bamboo".to_owned())?;
        same(
            "over it",
            b.block(at(base, [0, 1, 0])),
            "minecraft:air".to_owned(),
        )
    })
}

fn reaper_stem_fruits(o: Pos, _: &Tuning) -> Scenario {
    let fruit = at(o, [SPOT, 1, 1]);
    let stem = at(o, [SPOT, 1, 2]);
    reaper(
        "worker_reaper_stem_fruits",
        o,
        move |b, _| {
            // The fruit first: an attached stem facing no fruit turns back into a plain stem.
            b.place(at(stem, [0, -1, 0]), FARMLAND, &[])?;
            b.place(fruit, "minecraft:pumpkin", &[])?;
            b.place(
                stem,
                "minecraft:attached_pumpkin_stem",
                &[("facing", "north")],
            )
        },
        Vec::new(),
        move |b, m| Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:pumpkin") >= 1),
    )
    .check("the fruit went, its stem stays", move |b, _| {
        same("fruit", b.block(fruit), "minecraft:air".to_owned())?;
        // A stem whose fruit is gone turns back into a plain stem, as in the base game.
        let s = b.block(stem);
        if s.ends_with("pumpkin_stem") {
            Ok(())
        } else {
            Err(format!("the stem is {s}"))
        }
    })
}

fn reaper_berries(o: Pos, _: &Tuning) -> Scenario {
    let bush = at(o, [SPOT, 1, 1]);
    reaper(
        "worker_reaper_berries",
        o,
        move |b, _| {
            b.place(at(bush, [0, -1, 0]), "minecraft:grass_block", &[])?;
            b.place(bush, "minecraft:sweet_berry_bush", &[("age", "3")])
        },
        Vec::new(),
        move |b, m| Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:sweet_berries") >= 2),
    )
    .check("the bush stays, picked", move |b, _| {
        same(
            "bush",
            b.block(bush),
            "minecraft:sweet_berry_bush".to_owned(),
        )?;
        same("its age", b.property(bush, "age"), Some("1".to_owned()))
    })
}

fn reaper_on_a_wall(o: Pos, _: &Tuning) -> Scenario {
    let log = at(o, [SPOT, 2, 2]);
    let pod = at(o, [SPOT, 2, 1]);
    reaper(
        "worker_reaper_on_a_wall",
        o,
        move |b, _| {
            b.place(log, "minecraft:jungle_log", &[])?;
            b.place(pod, "minecraft:cocoa", &[("age", "2"), ("facing", "south")])
        },
        Vec::new(),
        move |b, m| Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:cocoa_beans") >= 1),
    )
    .check("the pod is planted again on its trunk", move |b, _| {
        same("pod", b.block(pod), "minecraft:cocoa".to_owned())?;
        same("its age", b.property(pod, "age"), Some("0".to_owned()))
    })
}

fn reaper_nothing_to_sow(o: Pos, _: &Tuning) -> Scenario {
    let flower = at(o, [SPOT, 1, 1]);
    reaper(
        "worker_reaper_nothing_to_sow",
        o,
        move |b, _| {
            b.place(at(flower, [0, -1, 0]), FARMLAND, &[])?;
            b.place(flower, "minecraft:torchflower", &[])
        },
        Vec::new(),
        move |b, m| Ok(count_of(&worker_of(b, m)?.hold.slots, "minecraft:torchflower") >= 1),
    )
    .check(
        "H3: the cell stays empty and the worker says why",
        move |b, m| {
            same("cell", b.block(flower), "minecraft:air".to_owned())?;
            same("note", worker_of(b, m)?.note, Note::NothingToSow)
        },
    )
}

/// A hold of `n` shovels: one a slot, so 27 fill it.
fn shovels(n: u32) -> Vec<(&'static str, u32)> {
    vec![("minecraft:wooden_shovel", n)]
}

fn unloads_into_a_chest_by_the_magistral(o: Pos, t: &Tuning) -> Scenario {
    let n = t.worker_hold as u32;
    let dock = at(o, [TRACK, 1, 1]);
    let housing = at(o, [TRACK, 1, 2]);
    let chest = at(o, [TRACK - 1, 1, 2]);
    let node = at(o, [TRACK + 1, 1, 2]);
    with_worker(
        "worker_unloads_into_a_chest_by_the_magistral",
        o,
        move |b, _| {
            b.place(dock, DOCK_UNLOAD, &[])?;
            place_node(b, node, 1)?;
            b.place(housing, HOUSING, &[])?;
            b.set_channels(housing, [channel(ChannelKind::Item, 1), None, None, None])?;
            b.place(chest, "minecraft:chest", &[])?;
            Ok(())
        },
        Some(Tool::Saw),
        shovels(n),
    )
    .check("a full hold sends it to unload", |b, m| {
        same("errand", worker_of(b, m)?.errand, Errand::Unload)
    })
    .until("every shovel reached the chest", 2400, move |b, _| {
        Ok(count_of(&b.items(chest), "minecraft:wooden_shovel") == n)
    })
    .until("emptied, it goes back to work", 100, |b, m| {
        let w = worker_of(b, m)?;
        Ok(w.hold.is_empty() && w.errand == Errand::Work)
    })
}

fn charges_at_the_dock(o: Pos, t: &Tuning) -> Scenario {
    let dock = at(o, [TRACK, 1, 1]);
    let low = 0.1 * t.worker_battery;
    let buffer = t.dock_buffer;
    Scenario::new("worker_charges_at_the_dock", plot_area(o))
        .then("the track, a charge dock with a crystal", move |b, _| {
            track(b, o)?;
            b.place(dock, DOCK_CHARGE, &[])?;
            b.place(at(dock, [0, 0, 1]), CRYSTAL, &[])
        })
        .then("its buffer full, a worker low on charge", move |b, m| {
            b.set_buffer(dock, buffer)?;
            let id = b.spawn_worker(rail(o, 0), OWNER, Some(Tool::Saw), &[], Some(low))?;
            m.mark("worker", id as f64);
            Ok(())
        })
        .until("it rides to the dock and charges", RIDE, move |b, m| {
            let w = worker_of(b, m)?;
            Ok(w.errand == Errand::Charge && w.charge >= low + buffer * 0.9)
        })
}

fn stops_without_a_dock(o: Pos, t: &Tuning) -> Scenario {
    let n = t.worker_hold as u32;
    with_worker(
        "worker_stops_without_a_dock",
        o,
        |_, _| Ok(()),
        Some(Tool::Saw),
        shovels(n),
    )
    .until("W4.c: it names the dock it lacks", 40, |b, m| {
        Ok(worker_of(b, m)?.note == Note::NoUnloadDock)
    })
}

fn fleet_survives_a_save(o: Pos, _: &Tuning) -> Scenario {
    with_worker(
        "worker_fleet_survives_a_save",
        o,
        |_, _| Ok(()),
        Some(Tool::Reaper),
        vec![("minecraft:wheat_seeds", 5)],
    )
    .wait(5)
    .check("the workers' record reads back the same", |b, _| {
        b.workers_round_trip()
    })
}
