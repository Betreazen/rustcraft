//! Slice 7: the worker cart. A lumberjack fells a tree beside its rails and plants a sapling
//! again; a reaper picks each group of plants by its rule (spec 17); a full hold rides to the
//! unload dock and goes on into a chest by the magistral (W4.a, W6); a low battery rides to the
//! charge dock (W4.b); with no dock on its rails it stops and says so (W4.c). The saw's and the
//! reaper's breaks are reported to Path-of-craft-tree (X1, the contract's `break`).

use super::magistral::place_node;
use super::{Bench, Build, Memo, Scenario, at, base, count_of, plot_area, same};
use crate::geom::{Face, Pos};
use crate::monorail::shape;
use crate::tuning::Tuning;
use crate::worker::{Errand, Note, Tool};

pub const ALL: [Build; 21] = [
    saw_fells_a_tree,
    reaper_tills_grass_and_sows_it,
    saw_plants_saplings_with_room,
    saw_fells_a_bare_trunk_and_leaves_a_house_wall,
    reaper_aged_crops,
    reaper_columns,
    reaper_stem_fruits,
    reaper_berries,
    reaper_on_a_wall,
    reaper_nothing_to_sow,
    reaper_sows_empty_farmland,
    reaper_sows_pumpkins_with_room,
    reaper_filtered_sows_only_its_crop,
    reaper_without_seeds_says_so,
    reaper_says_nothing_to_reap,
    saw_says_no_trees,
    grab_says_no_ore,
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
const GRASS: &str = "minecraft:grass_block";
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

/// A floor of stone and a straight track of floor rails on it, with its base (0.19.1): an unload
/// dock (full) under the first rail, a charge dock under the last.
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
    base(b, Some(rail(o, 0)), Some(rail(o, TRACK)))
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

/// 0.18.5, the owner's field test: a birch trunk right beside the rail, its leaves gone, stands
/// on grass; the saw fells it and plants a sapling. A log wall of a house a block further on, with
/// planks in it, stays.
fn saw_fells_a_bare_trunk_and_leaves_a_house_wall(o: Pos, _: &Tuning) -> Scenario {
    const BIRCH: &str = "minecraft:birch_log";
    let root = at(o, [SPOT, 1, 1]);
    let wall: Vec<Pos> = (SPOT + 2..=SPOT + 3)
        .flat_map(|x| (1..=3).map(move |y| at(o, [x, y, 1])))
        .collect();
    let planks = at(o, [SPOT + 2, 2, 2]);
    let wall_built = wall.clone();
    let wall_left = wall.clone();
    with_worker(
        "worker_saw_fells_a_bare_trunk_and_leaves_a_house_wall",
        o,
        move |b, _| {
            b.place(at(root, [0, -1, 0]), "minecraft:grass_block", &[])?;
            for y in 0..6 {
                b.place(at(root, [0, y, 0]), BIRCH, &[])?;
            }
            for &p in &wall_built {
                b.place(at(p, [0, -1 - (p[1] - root[1]), 0]), "minecraft:dirt", &[])?;
                b.place(p, BIRCH, &[])?;
            }
            b.place(planks, "minecraft:birch_planks", &[])
        },
        Some(Tool::Saw),
        vec![("minecraft:birch_sapling", 2)],
    )
    .until("the trunk's logs are in the hold", RIDE, move |b, m| {
        Ok(count_of(&worker_of(b, m)?.hold.slots, BIRCH) >= 6)
    })
    .wait(60)
    .check("a sapling stands on its root, the wall and its planks stay", move |b, m| {
        same("root", b.block(root), "minecraft:birch_sapling".to_owned())?;
        for y in 1..6 {
            same("over the root", b.block(at(root, [0, y, 0])), "minecraft:air".to_owned())?;
        }
        for &p in &wall_left {
            same("the house wall", b.block(p), BIRCH.to_owned())?;
        }
        same("planks", b.block(planks), "minecraft:birch_planks".to_owned())?;
        same("logs taken", count_of(&worker_of(b, m)?.hold.slots, BIRCH), 6)
    })
    .until("each log is reported as an axe's break", 40, |b, _| {
        Ok(breaks(b, "minecraft:birch_log", "axe") >= 6)
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
                b.place(at(base, [0, y, 0]), "minecraft:bamboo", &[("stage", "1")])?;
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

/// 0.18.5, the owner's field test (a reaper with seeds in its hold rode round its field doing
/// nothing): empty farmland beside the way is sown from the hold, the seed of the crop growing
/// next to it first, else the first seed of the hold; nether wart goes on soul sand.
fn reaper_sows_empty_farmland(o: Pos, _: &Tuning) -> Scenario {
    let row = [at(o, [SPOT, 1, 1]), at(o, [SPOT + 1, 1, 1])];
    let carrots = at(o, [SPOT + 2, 1, 1]);
    let lone = at(o, [1, 1, -1]);
    let wart = at(o, [5, 1, -1]);
    reaper(
        "worker_reaper_sows_empty_farmland",
        o,
        move |b, _| {
            // Wet farmland: dry and bare, a random tick could turn it to dirt before the reaper
            // comes.
            for p in [row[0], row[1], carrots, lone] {
                b.place(at(p, [0, -1, 0]), FARMLAND, &[("moisture", "7")])?;
            }
            b.place(carrots, "minecraft:carrots", &[("age", "2")])?;
            b.place(at(wart, [0, -1, 0]), "minecraft:soul_sand", &[])
        },
        vec![
            ("minecraft:wheat_seeds", 8),
            ("minecraft:carrot", 8),
            ("minecraft:nether_wart", 8),
        ],
        move |b, _| Ok([row[0], row[1], lone, wart].iter().all(|&p| b.block(p) != "minecraft:air")),
    )
    .check("each cell has its crop, sown young", move |b, _| {
        for p in row {
            same("the carrot row", b.block(p), "minecraft:carrots".to_owned())?;
            same("its age", b.property(p, "age"), Some("0".to_owned()))?;
        }
        same("a lone bed: the hold's first seed", b.block(lone), "minecraft:wheat".to_owned())?;
        same("soul sand", b.block(wart), "minecraft:nether_wart".to_owned())?;
        same("the growing carrots", b.property(carrots, "age"), Some("2".to_owned()))
    })
    .check("the seeds came from the hold and it says it works", move |b, m| {
        let w = worker_of(b, m)?;
        same("carrots left", count_of(&w.hold.slots, "minecraft:carrot"), 6)?;
        same("wheat seeds left", count_of(&w.hold.slots, "minecraft:wheat_seeds"), 7)?;
        same("nether wart left", count_of(&w.hold.slots, "minecraft:nether_wart"), 7)?;
        if w.note == Note::NothingToSow {
            return Err("it says «Нечем засеять» with seeds in its hold".into());
        }
        Ok(())
    })
}

/// 0.19.2, the owner: «жнец почему-то не садит тыкву». Pumpkin seeds in the hold: the stems go
/// every other cell of a bare row, and the cells between them stay empty for the fruit.
fn reaper_sows_pumpkins_with_room(o: Pos, _: &Tuning) -> Scenario {
    const STEM: &str = "minecraft:pumpkin_stem";
    let row: Vec<Pos> = (1..=5).map(|x| at(o, [x, 1, 1])).collect();
    let beds = row.clone();
    let watched = row.clone();
    // Every cell is a stem or beside one: nothing is left to sow.
    let settled = move |b: &dyn Bench, row: &[Pos]| {
        let stem = |i: usize| row.get(i).is_some_and(|&p| b.block(p) == STEM);
        (0..row.len()).all(|i| stem(i) || (i > 0 && stem(i - 1)) || stem(i + 1))
    };
    reaper(
        "worker_reaper_sows_pumpkins_with_room",
        o,
        move |b, _| {
            for &p in &beds {
                b.place(at(p, [0, -1, 0]), FARMLAND, &[("moisture", "7")])?;
            }
            Ok(())
        },
        vec![("minecraft:pumpkin_seeds", 8)],
        move |b, _| Ok(settled(&*b, &watched)),
    )
    .check("stems every other cell, the cells between them empty", move |b, m| {
        let stems: Vec<bool> = row.iter().map(|&p| b.block(p) == STEM).collect();
        if stems.windows(2).any(|w| w[0] && w[1]) {
            return Err(format!("two stems side by side: {stems:?}"));
        }
        for (i, &p) in row.iter().enumerate() {
            if !stems[i] {
                same("a fruit's cell", b.block(p), "minecraft:air".to_owned())?;
            }
        }
        let sown = stems.iter().filter(|s| **s).count() as u32;
        if sown < 2 {
            return Err(format!("{sown} stems: {stems:?}"));
        }
        let w = worker_of(b, m)?;
        same("seeds left in the hold", count_of(&w.hold.slots, "minecraft:pumpkin_seeds"), 8 - sown)?;
        if w.note == Note::NothingToSow {
            return Err("it says «Нечем засеять» with pumpkin seeds in its hold".into());
        }
        Ok(())
    })
}

/// H6 with sowing: a reaper filtered to carrots sows carrots only; the farmland with no carrot
/// to sow stays bare beside wheat seeds it does not sow.
fn reaper_filtered_sows_only_its_crop(o: Pos, _: &Tuning) -> Scenario {
    let bed = at(o, [SPOT, 1, 1]);
    let other = at(o, [SPOT, 1, -1]);
    Scenario::new("worker_reaper_filtered_sows_only_its_crop", plot_area(o))
        .then("the track and two bare beds", move |b, _| {
            track(b, o)?;
            b.place(at(bed, [0, -1, 0]), FARMLAND, &[("moisture", "7")])?;
            b.place(at(other, [0, -1, 0]), FARMLAND, &[("moisture", "7")])
        })
        .then("a reaper filtered to carrots, one carrot and wheat seeds", move |b, m| {
            let hold = [("minecraft:wheat_seeds", 8), ("minecraft:carrot", 1)];
            let id = b.spawn_worker(rail(o, 0), OWNER, Some(Tool::Reaper), &hold, None)?;
            m.mark("worker", id as f64);
            b.set_worker_filter(id, &["minecraft:carrot"])
        })
        .until("one bed has carrots", RIDE, move |b, _| {
            Ok(b.block(bed) == "minecraft:carrots" || b.block(other) == "minecraft:carrots")
        })
        .wait(100)
        .check("the other bed stays bare and the wheat seeds stay in the hold", move |b, m| {
            let bare = [bed, other].iter().filter(|&&p| b.block(p) == "minecraft:air").count();
            same("bare beds", bare, 1)?;
            let w = worker_of(b, m)?;
            same("wheat seeds", count_of(&w.hold.slots, "minecraft:wheat_seeds"), 8)?;
            same("carrots", count_of(&w.hold.slots, "minecraft:carrot"), 0)
        })
}

/// With no seed for the bare farmland and nothing ripe it says «Нечем засеять», and the word
/// stays while it rides on (it used to last a tick); seeds put in, it sows on the next pass.
fn reaper_without_seeds_says_so(o: Pos, _: &Tuning) -> Scenario {
    let bed = at(o, [SPOT, 1, 1]);
    with_worker(
        "worker_reaper_without_seeds_says_so",
        o,
        move |b, _| b.place(at(bed, [0, -1, 0]), FARMLAND, &[("moisture", "7")]),
        Some(Tool::Reaper),
        Vec::new(),
    )
    .until("it says it has nothing to sow", RIDE, |b, m| {
        Ok(worker_of(b, m)?.note == Note::NothingToSow)
    })
    .wait(20)
    .check("the word stays while it rides on, the bed stays bare", move |b, m| {
        same("note", worker_of(b, m)?.note, Note::NothingToSow)?;
        same("bed", b.block(bed), "minecraft:air".to_owned())
    })
}

/// 0.18.5: a tool that finds nothing along a whole stretch of its way says so, and the word
/// stays while it rides on: the reaper, the saw and the grab (the scoop always did).
fn says_when_idle(name: &'static str, o: Pos, tool: Tool, note: Note) -> Scenario {
    with_worker(name, o, |_, _| Ok(()), Some(tool), Vec::new())
        .until("it says there is nothing beside its way", RIDE, move |b, m| {
            Ok(worker_of(b, m)?.note == note)
        })
        .wait(20)
        .check("the word stays while it rides on", move |b, m| {
            same("note", worker_of(b, m)?.note, note)
        })
}

fn reaper_says_nothing_to_reap(o: Pos, _: &Tuning) -> Scenario {
    says_when_idle("worker_reaper_says_nothing_to_reap", o, Tool::Reaper, Note::NothingToReap)
}

fn saw_says_no_trees(o: Pos, _: &Tuning) -> Scenario {
    says_when_idle("worker_saw_says_no_trees", o, Tool::Saw, Note::NoTrees)
}

fn grab_says_no_ore(o: Pos, _: &Tuning) -> Scenario {
    says_when_idle("worker_grab_says_no_ore", o, Tool::Grab, Note::NoOre)
}

/// The track on a floor of grass (0.18.5's own work: tilling, planting).
fn grass_track(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=TRACK + 1 {
        for z in -2..=2 {
            b.place(at(o, [x, 0, z]), GRASS, &[])?;
        }
    }
    for x in 0..=TRACK {
        b.place(
            rail(o, x),
            MONORAIL,
            &[(shape::MOUNT, shape::mount_value(Face::YNeg))],
        )?;
    }
    base(b, Some(rail(o, 0)), Some(rail(o, TRACK)))
}

/// 0.18.5, the owner: «работники сами готовят землю». A reaper with wheat seeds rides along a lawn:
/// the grass beside its way becomes farmland sown with wheat in the same pass; the grass under
/// the rail and outside its zone stays.
fn reaper_tills_grass_and_sows_it(o: Pos, _: &Tuning) -> Scenario {
    let beds: Vec<Pos> = (1..TRACK)
        .flat_map(|x| [at(o, [x, 1, 1]), at(o, [x, 1, -1])])
        .collect();
    let checked = beds.clone();
    Scenario::new("worker_reaper_tills_grass_and_sows_it", plot_area(o))
        .then("a track on a lawn", move |b, _| grass_track(b, o))
        .then("a reaper with wheat seeds", move |b, m| {
            let id = b.spawn_worker(rail(o, 0), OWNER, Some(Tool::Reaper), &[("minecraft:wheat_seeds", 64)], None)?;
            m.mark("worker", id as f64);
            Ok(())
        })
        .until("the grass beside its way is sown", RIDE, move |b, _| {
            Ok(beds.iter().all(|&p| b.block(p) == "minecraft:wheat"))
        })
        .check("farmland under the wheat; the grass under the rail and outside the zone stays", move |b, _| {
            for &p in &checked {
                same("under the wheat", b.block(at(p, [0, -1, 0])), FARMLAND.to_owned())?;
            }
            for x in 1..TRACK {
                same("under the rail", b.block(at(o, [x, 0, 0])), GRASS.to_owned())?;
                same("outside the zone", b.block(at(o, [x, 0, 2])), GRASS.to_owned())?;
                same("over the rail", b.block(at(o, [x, 2, 0])), "minecraft:air".to_owned())?;
            }
            Ok(())
        })
}

/// 0.18.5, the owner: «сажает саженцы так, чтобы деревья хорошо росли». A saw with oak saplings
/// rides along a lawn and plants them beside its way, a free cell between any two, none on the
/// rail's cells nor outside its zone.
fn saw_plants_saplings_with_room(o: Pos, _: &Tuning) -> Scenario {
    const SAPLING: &str = "minecraft:oak_sapling";
    Scenario::new("worker_saw_plants_saplings_with_room", plot_area(o))
        .then("a track on a lawn", move |b, _| grass_track(b, o))
        .then("a saw with oak saplings", move |b, m| {
            let id = b.spawn_worker(rail(o, 0), OWNER, Some(Tool::Saw), &[(SAPLING, 32)], None)?;
            m.mark("worker", id as f64);
            Ok(())
        })
        .until("saplings stand on both sides of its way", RIDE, move |b, _| {
            let side = |z: i32| (1..TRACK).filter(|&x| b.block(at(o, [x, 1, z])) == SAPLING).count();
            Ok(side(1) >= 2 && side(-1) >= 2)
        })
        .wait(40)
        .check("a free cell round each, none outside the zone, the hold paid", move |b, m| {
            let mut planted = Vec::new();
            for x in -2..=TRACK + 2 {
                for z in -3..=3 {
                    if b.block(at(o, [x, 1, z])) == SAPLING {
                        planted.push((x, z));
                    }
                }
            }
            for &(x, z) in &planted {
                if z.abs() != 1 {
                    return Err(format!("a sapling at x {x}, z {z}: off the zone's ground"));
                }
                if planted.iter().any(|&(a, c)| (a, c) != (x, z) && (a - x).abs() <= 1 && (c - z).abs() <= 1) {
                    return Err(format!("the sapling at x {x}, z {z} has a neighbour: {planted:?}"));
                }
            }
            let left = count_of(&worker_of(b, m)?.hold.slots, SAPLING);
            same("saplings paid from the hold", left as usize, 32 - planted.len())
        })
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
            // Its own unload dock at the far end: the base's goes.
            b.place(at(o, [0, 0, 0]), STONE, &[])?;
            b.place(dock, DOCK_UNLOAD, &[])?;
            place_node(b, node, 1)?;
            b.place(housing, HOUSING, &[])?;
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
            // Its own charge dock: the base's goes.
            b.place(at(o, [TRACK, 0, 0]), STONE, &[])?;
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
        // The base's unload dock goes.
        move |b, _| b.place(at(o, [0, 0, 0]), STONE, &[]),
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
