//! 0.18.5, the owner's field test: «работники не замечают доков разгрузки и погрузки» and «доки
//! должны работать с магистралью». A worker gives what it carries to an unload dock it passes,
//! the hold far from full, with the dock beside its rail or under it, placed before or after the
//! worker; tops up at a charge dock it passes; the unload dock hands its load over the magistral
//! into a storage; a storage fills a load dock over the magistral for a cargo worker, which
//! carries it to the unload dock and the chest beside it (guide `dock_unload`, `dock_load`).

use super::transport::{chest, item_line, joint};
use super::{Bench, Build, Memo, Scenario, at, base, count_of, plot_area, same};
use crate::geom::{Face, Pos};
use crate::monorail::shape;
use crate::net::items::Mode;
use crate::tuning::Tuning;
use crate::worker::{Errand, Note, Tool, Worker};

pub const ALL: [Build; 9] = [
    worker_stands_without_its_docks,
    bore_bores_away_from_its_docks,
    bore_on_a_loop_says_so,
    worker_unloads_at_a_dock_it_passes,
    worker_unloads_at_a_dock_under_its_rail,
    worker_unloads_at_a_dock_placed_after_it,
    worker_tops_up_at_a_charge_dock_it_passes,
    unload_dock_fills_the_storage_over_the_magistral,
    storage_loads_a_cargo_worker_over_the_magistral,
];

const MONORAIL: &str = "rustcraft:monorail";
const DOCK_UNLOAD: &str = "rustcraft:dock_unload";
const DOCK_LOAD: &str = "rustcraft:dock_load";
const DOCK_CHARGE: &str = "rustcraft:dock_charge";
const CRYSTAL: &str = "rustcraft:crystal";
const CELL: &str = "rustcraft:cell";
const TERMINAL: &str = "rustcraft:terminal";
const STONE: &str = "minecraft:stone";
const CHEST: &str = "minecraft:chest";
const SHOVEL: &str = "minecraft:wooden_shovel";
const COBBLE: &str = "minecraft:cobblestone";
/// The owner of the scenarios' workers (as in `workers.rs`).
const OWNER: u128 = super::workers::OWNER;
/// The track: floor rails at y 1, z 1, x 0..=LAST.
const LAST: i32 = 10;
/// Generous: a ride along the track and back at the slowest speed, a dock's work, the net's.
const RIDE: u64 = 1200;
/// A few items, far from a full hold.
const CARRIED: u32 = 5;

fn rail_cell(o: Pos, x: i32) -> Pos {
    at(o, [x, 1, 1])
}

/// A stone floor and the track on it.
fn track(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=LAST + 1 {
        for z in -1..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    for x in 0..=LAST {
        b.place(rail_cell(o, x), MONORAIL, &[(shape::MOUNT, shape::mount_value(Face::YNeg))])?;
    }
    Ok(())
}

fn worker_of(b: &dyn Bench, m: &Memo) -> Result<Worker, String> {
    b.worker(m.get("worker") as u64)
        .ok_or_else(|| "the worker is gone".to_owned())
}

/// A worker with `tool` at the track's start.
fn spawn(
    b: &mut dyn Bench,
    m: &mut Memo,
    o: Pos,
    tool: Tool,
    hold: &[(&str, u32)],
    charge: Option<f64>,
) -> Result<(), String> {
    let id = b.spawn_worker(rail_cell(o, 0), OWNER, Some(tool), hold, charge)?;
    m.mark("worker", id as f64);
    Ok(())
}

/// 0.19.1, the owner: «если на монорельсе нет зарядки и разгрузки, работник ставится, но не
/// едет». A saw on a bare track stands and says it lacks an unload dock, then a charge dock; with
/// both it rides.
fn worker_stands_without_its_docks(o: Pos, _: &Tuning) -> Scenario {
    let start = rail_cell(o, 0);
    let still = move |b: &dyn Bench, m: &Memo, note: Note| -> Result<(), String> {
        let w = worker_of(b, m)?;
        same("cell", w.cell, start)?;
        same("note", w.note, note)
    };
    Scenario::new("worker_stands_without_its_docks", plot_area(o))
        .then("a bare track and a saw on it", move |b, m| {
            track(b, o)?;
            spawn(b, m, o, Tool::Saw, &[], None)
        })
        .wait(40)
        .check("it stands: no unload dock", move |b, m| still(b, m, Note::NoUnloadDock))
        .then("an unload dock beside the far end", move |b, _| b.place(at(o, [LAST, 1, 2]), DOCK_UNLOAD, &[]))
        .wait(40)
        .check("it stands: no charge dock", move |b, m| still(b, m, Note::NoChargeDock))
        .then("a charge dock beside the far end", move |b, _| b.place(at(o, [LAST, 1, 0]), DOCK_CHARGE, &[]))
        .until("with both it rides", RIDE, move |b, m| Ok(worker_of(b, m)?.cell != start))
}

/// The bore's line: rails x 0..=3 at y 1, z 1, on a stone floor, stone two high past both ends
/// (x -3..=-1 and 4..=6); its docks beside its west end.
fn bore_line(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -4..=7 {
        for z in -1..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    for x in (-3..=-1).chain(4..=6) {
        for y in 1..=2 {
            b.place(at(o, [x, y, 1]), STONE, &[])?;
        }
    }
    for x in 0..=3 {
        b.place(rail_cell(o, x), MONORAIL, &[(shape::MOUNT, shape::mount_value(Face::YNeg))])?;
    }
    b.place(at(o, [0, 1, 2]), DOCK_UNLOAD, &[])?;
    b.place(at(o, [0, 1, 0]), DOCK_CHARGE, &[])
}

/// 0.19.1, the owner: «бур всегда уходит от зарядки и разгрузчика». Its docks at the west end of
/// its line, it bores east past the east end and never west past the docks.
fn bore_bores_away_from_its_docks(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("worker_bore_bores_away_from_its_docks", plot_area(o))
        .then("a line in rock, its docks at the west end", move |b, m| {
            bore_line(b, o)?;
            let id = b.spawn_worker(rail_cell(o, 0), OWNER, Some(Tool::Bore), &[(MONORAIL, 4), (COBBLE, 8)], None)?;
            m.mark("worker", id as f64);
            Ok(())
        })
        .until("a rail lies east of the line", RIDE, move |b, _| Ok(b.block(rail_cell(o, 4)) == MONORAIL))
        .wait(100)
        .check("the rock west of the docks stands", move |b, _| {
            same("west", b.block(rail_cell(o, -1)), STONE.to_owned())
        })
}

/// A bore on a loop has no end to bore from: it stands and says so.
fn bore_on_a_loop_says_so(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("worker_bore_on_a_loop_says_so", plot_area(o))
        .then("a loop with its docks", move |b, m| {
            for x in -1..=4 {
                for z in -1..=3 {
                    b.place(at(o, [x, 0, z]), STONE, &[])?;
                }
            }
            for (x, z) in [(0, 0), (1, 0), (2, 0), (2, 1), (2, 2), (1, 2), (0, 2), (0, 1)] {
                b.place(at(o, [x, 1, z]), MONORAIL, &[(shape::MOUNT, shape::mount_value(Face::YNeg))])?;
            }
            b.place(at(o, [3, 1, 1]), DOCK_UNLOAD, &[])?;
            b.place(at(o, [1, 1, 1]), DOCK_CHARGE, &[])?;
            let id = b.spawn_worker(at(o, [0, 1, 0]), OWNER, Some(Tool::Bore), &[(MONORAIL, 4), (COBBLE, 8)], None)?;
            m.mark("worker", id as f64);
            Ok(())
        })
        .until("it says its rails have no end", RIDE, |b, m| Ok(worker_of(b, m)?.note == Note::NoEnd))
        .check("and stands", move |b, m| same("cell", worker_of(b, m)?.cell, at(o, [0, 1, 0])))
}

/// A saw worker with a few shovels rides past `dock`; the chest beside the dock gets them all and
/// the worker rides on, empty, at work.
fn unloads_in_passing(
    name: &'static str,
    o: Pos,
    dock: Pos,
    chest_at: Pos,
    dock_after: bool,
) -> Scenario {
    let place_dock = move |b: &mut dyn Bench| -> Result<(), String> {
        b.place(dock, DOCK_UNLOAD, &[])?;
        b.place(chest_at, CHEST, &[])
    };
    let mut s = Scenario::new(name, plot_area(o)).then("the track", move |b, _| {
        track(b, o)?;
        base(b, None, Some(rail_cell(o, LAST)))?;
        if dock_after { Ok(()) } else { place_dock(b) }
    });
    s = s.then("a worker carrying a few shovels", move |b, m| {
        spawn(b, m, o, Tool::Saw, &[(SHOVEL, CARRIED)], None)
    });
    if dock_after {
        s = s
            .wait(20)
            .check("0.19.1: without an unload dock it stands where it was placed", move |b, m| {
                let w = worker_of(b, m)?;
                same("cell", w.cell, rail_cell(o, 0))?;
                same("note", w.note, Note::NoUnloadDock)
            })
            .then("the dock, placed after the worker", move |b, _| place_dock(b));
    }
    s.until("the shovels reach the chest beside the dock", RIDE, move |b, _| {
        Ok(count_of(&b.items(chest_at), SHOVEL) == CARRIED)
    })
    .until("it rides on at work, its hold empty", 200, |b, m| {
        let w = worker_of(b, m)?;
        Ok(w.hold.is_empty() && w.errand == Errand::Work)
    })
}

fn worker_unloads_at_a_dock_it_passes(o: Pos, _: &Tuning) -> Scenario {
    // Beside the rail at the rail's level, a chest beside the dock.
    unloads_in_passing(
        "worker_unloads_at_a_dock_it_passes",
        o,
        at(o, [5, 1, 2]),
        at(o, [5, 1, 3]),
        false,
    )
}

fn worker_unloads_at_a_dock_under_its_rail(o: Pos, _: &Tuning) -> Scenario {
    // The rail stands on the dock; the chest beside the dock in the floor.
    unloads_in_passing(
        "worker_unloads_at_a_dock_under_its_rail",
        o,
        at(o, [5, 0, 1]),
        at(o, [5, 0, 2]),
        false,
    )
}

fn worker_unloads_at_a_dock_placed_after_it(o: Pos, _: &Tuning) -> Scenario {
    unloads_in_passing(
        "worker_unloads_at_a_dock_placed_after_it",
        o,
        at(o, [5, 1, 0]),
        at(o, [5, 1, -1]),
        true,
    )
}

fn worker_tops_up_at_a_charge_dock_it_passes(o: Pos, t: &Tuning) -> Scenario {
    let dock = at(o, [5, 1, 2]);
    let half = 0.5 * t.worker_battery;
    let buffer = t.dock_buffer;
    Scenario::new("worker_tops_up_at_a_charge_dock_it_passes", plot_area(o))
        .then("the track, a charge dock with a crystal and a full buffer", move |b, _| {
                track(b, o)?;
            base(b, Some(rail_cell(o, 0)), None)?;
            b.place(dock, DOCK_CHARGE, &[])?;
            b.place(at(dock, [0, 0, 1]), CRYSTAL, &[])
        })
        .then("a worker at half charge", move |b, m| {
            b.set_buffer(dock, buffer)?;
            spawn(b, m, o, Tool::Saw, &[], Some(half))
        })
        .until("it stops in passing and charges", RIDE, move |b, m| {
            Ok(worker_of(b, m)?.charge >= half + (buffer * 0.5).min(0.3 * half))
        })
        .until("then it rides on at work", RIDE, |b, m| {
            Ok(worker_of(b, m)?.errand == Errand::Work)
        })
}

/// A magistral along z 3 from a node at x 0 (housings x 1..=6, item channel), a storage of four
/// cells and a terminal on its z+ side at housings 3 and 4.
fn magistral_and_storage(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    let line_o = at(o, [0, 1, 3]);
    item_line(b, line_o, 1, 6, &[])?;
    for p in storage_cells(o) {
        b.place(p, CELL, &[])?;
    }
    b.place(terminal(o), TERMINAL, &[])
}

fn storage_cells(o: Pos) -> [Pos; 4] {
    [at(o, [3, 1, 4]), at(o, [4, 1, 4]), at(o, [3, 1, 5]), at(o, [4, 1, 5])]
}

fn terminal(o: Pos) -> Pos {
    at(o, [5, 1, 4])
}

fn stored(b: &dyn Bench, o: Pos, item: &str) -> Result<u64, String> {
    b.storage(terminal(o))
        .map(|v| v.store.count(item))
        .ok_or_else(|| "the terminal shows no storage".into())
}

fn unload_dock_fills_the_storage_over_the_magistral(o: Pos, _: &Tuning) -> Scenario {
    // Guide `dock_unload`: «отдаёт его в магистраль». The dock at x 6 touches the rail and the
    // last housing.
    let dock = at(o, [6, 1, 2]);
    Scenario::new("worker_unload_dock_fills_the_storage_over_the_magistral", plot_area(o))
        .then("the track, the magistral with a storage, the unload dock between", move |b, _| {
                track(b, o)?;
            base(b, None, Some(rail_cell(o, 0)))?;
            magistral_and_storage(b, o)?;
            b.place(dock, DOCK_UNLOAD, &[])
        })
        .then("a worker carrying a few shovels", move |b, m| {
            spawn(b, m, o, Tool::Saw, &[(SHOVEL, CARRIED)], None)
        })
        .until("the storage holds the shovels", RIDE, move |b, _| {
            Ok(stored(b, o, SHOVEL)? == u64::from(CARRIED))
        })
        .check("the worker gave them all", |b, m| {
            same("shovels left", count_of(&worker_of(b, m)?.hold.slots, SHOVEL), 0)
        })
}

fn storage_loads_a_cargo_worker_over_the_magistral(o: Pos, _: &Tuning) -> Scenario {
    // Guide `dock_load`: «Загружает грузовую вагонетку предметами из магистрали». A chest fills the
    // storage and goes; the load dock on the magistral then takes from the storage, the cargo
    // worker carries it to the unload dock at the far end, which gives it to the chest beside it.
    let line_o = at(o, [0, 1, 3]);
    let source = chest(line_o, 1);
    let load = at(o, [6, 1, 2]);
    let unload = at(o, [LAST, 1, 0]);
    let out = at(o, [LAST, 1, -1]);
    Scenario::new("worker_storage_loads_a_cargo_worker_over_the_magistral", plot_area(o))
        .then("the magistral and its storage, a chest «из блока» of cobblestone", move |b, _| {
                track(b, o)?;
            base(b, None, Some(rail_cell(o, 0)))?;
            magistral_and_storage(b, o)?;
            b.place(source, CHEST, &[])?;
            joint(b, line_o, 1, Mode::From, 0)?;
            let n = b.insert(source, COBBLE, 32)?;
            same("cobblestone in the chest", n, 32)
        })
        .until("the storage holds the cobblestone", RIDE, move |b, _| Ok(stored(b, o, COBBLE)? == 32))
        .then("the chest goes; the load dock, the unload dock and its chest come", move |b, _| {
            b.place(source, "minecraft:air", &[])?;
            b.place(load, DOCK_LOAD, &[])?;
            b.place(unload, DOCK_UNLOAD, &[])?;
            b.place(out, CHEST, &[])
        })
        .then("a cargo worker", move |b, m| spawn(b, m, o, Tool::Hold, &[], None))
        .until("the storage's cobblestone reaches the far chest", 2 * RIDE, move |b, _| {
            Ok(count_of(&b.items(out), COBBLE) >= 4)
        })
        .check("it left the storage", move |b, _| {
            let left = stored(b, o, COBBLE)?;
            if left < 32 { Ok(()) } else { Err(format!("the storage still holds {left}")) }
        })
}
