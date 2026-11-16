//! Slice 18, energy stream: the crystal shard in the recipes, the druse breaking into shards
//! (E5), and growing crystals on a tank of solution (CR2).

use super::{Build, Scenario, at, plot_area, same};
use crate::grow::{self, DRUSE, SEED, SHARD, STAGE};
use crate::tuning::Tuning;

pub const ALL: [Build; 10] = [
    shard_recipes,
    druse_breaks_into_shards,
    seed_grows_on_solution,
    seed_stands_without_solution,
    structure_window_and_look,
    structure_unfed_window_and_look,
    miner_fills_a_load_dock,
    vein_window,
    structure_three_by_three,
    structure_cube,
];

const TANK: &str = "rustcraft:tank";
const CRYSTAL: &str = "rustcraft:crystal";
const PICKAXE: &str = "minecraft:diamond_pickaxe";
const LIMIT: u64 = 200;

fn shards_in(drops: &[(String, u32)]) -> u32 {
    drops
        .iter()
        .filter(|(i, _)| i == SHARD)
        .map(|(_, n)| *n)
        .sum()
}

/// The guide's recipes take the mod's own shard (guide `crystal`, `battery`).
fn shard_recipes(o: crate::geom::Pos, _t: &Tuning) -> Scenario {
    Scenario::new("s18_shard_recipes", plot_area(o)).check(
        "the crystal is made of crystal shards round copper-steel, not of amethyst",
        move |b, _| {
            let s = Some(SHARD);
            let a = Some("rustcraft:coppersteel");
            let made = b.craft(&[s, s, s, s, a, s, s, s, s]);
            same(
                "crystal from shards",
                made.map(|(i, _)| i),
                Some("rustcraft:crystal".to_owned()),
            )?;
            let am = Some("minecraft:amethyst_shard");
            let old = b.craft(&[am, am, am, am, a, am, am, am, am]);
            same("crystal from amethyst", old.map(|(i, _)| i), None)?;
            same("the shard is an item", b.item_known(SHARD), true)
        },
    )
}

/// E5: a druse breaks into two to four shards.
fn druse_breaks_into_shards(o: crate::geom::Pos, _t: &Tuning) -> Scenario {
    let p = at(o, [2, 0, 2]);
    Scenario::new("s18_druse_breaks_into_shards", plot_area(o))
        .then("a druse on the floor", move |b, _| b.place(p, DRUSE, &[]))
        .check("broken with a pickaxe it gives 2–4 shards", move |b, _| {
            for round in 0..4 {
                if round > 0 {
                    b.place(p, DRUSE, &[])?;
                }
                let n = shards_in(&b.break_block(p, Some(PICKAXE))?);
                if !(2..=4).contains(&n) {
                    return Err(format!("{n} shards from a druse"));
                }
            }
            Ok(())
        })
}

/// CR2: a seed on a tank of solution drinks it and grows; grown, it breaks into 2–4 shards.
fn seed_grows_on_solution(o: crate::geom::Pos, t: &Tuning) -> Scenario {
    let tank = at(o, [2, 0, 2]);
    let seed = at(o, [2, 1, 2]);
    let span = 100;
    let drink = grow::drink_per_second(t) * span as f64 / t.ticks_per_second;
    let buckets = t.grow_buckets;
    Scenario::new("s18_seed_grows_on_solution", plot_area(o))
        .then("a tank of solution and a seed on it", move |b, _| {
            b.place(tank, TANK, &[])?;
            b.set_tank(tank, grow::SOLUTION, 8000)?;
            b.place(seed, SEED, &[])
        })
        .until("the seed takes solution from the tank", LIMIT, move |b, m| {
            let s = b.seed(seed).ok_or("no seed")?;
            m.mark("progress", s.progress);
            Ok(s.solution > 0.0 || s.progress > 0.0)
        })
        .wait(span)
        .check("it grows by what it drinks: 4 buckets in 10 minutes", move |b, m| {
            let s = b.seed(seed).ok_or("no seed")?;
            let grown = (s.progress - m.get("progress")) * buckets * 1000.0;
            if (grown - drink).abs() > drink * 0.25 + 1.0 {
                return Err(format!("drank {grown:.1} mB in {span} ticks, wanted {drink:.1}"));
            }
            same("stage", b.property(seed, STAGE), Some("0".to_owned()))
        })
        .then("nearly grown", move |b, _| b.set_seed(seed, 0.999))
        .until("grown, it shows the druse (stage 2)", LIMIT, move |b, _| {
            Ok(b.property(seed, STAGE).as_deref() == Some("2"))
        })
        .check("broken grown, it gives 2–4 shards", move |b, _| {
            let n = shards_in(&b.break_block(seed, Some(PICKAXE))?);
            if (2..=4).contains(&n) {
                Ok(())
            } else {
                Err(format!("{n} shards from a grown druse"))
            }
        })
}

/// Whether the sheet's texts (JSON components) name the lang key `key`.
fn names(sheet: &crate::machines::sheet::Sheet, key: &str) -> bool {
    let rows = sheet.rows.iter().chain(sheet.status.iter().map(|s| &s.text));
    rows.into_iter().any(|v| v.to_string().contains(key))
}

/// A 2×2 of crystals (a tank under one corner, a furnace by another) on solution or not: its window
/// (ui.js `structure`) and how clients are told to draw it (one crystal from the anchor).
fn structure_scenario(name: &'static str, o: crate::geom::Pos, t: &Tuning, fed: bool) -> Scenario {
    use crate::power::{self as rules, Form, Shown};
    let cells = [[2, 0, 2], [3, 0, 2], [2, 0, 3], [3, 0, 3]].map(|d| at(o, d));
    let tank = at(o, [1, 0, 2]);
    let oven = at(o, [4, 0, 2]);
    let total = rules::structure_output(4, Form::Box([1, 2, 2]), fed, t);
    let wanted = if fed {
        Shown::Gives(total)
    } else {
        Shown::NoSolution(total)
    };
    let (limit, severity) = if fed {
        ("rustcraft.structure.limit_size", 0)
    } else {
        ("rustcraft.structure.limit_solution", 1)
    };
    Scenario::new(name, plot_area(o))
        .then("four crystals in a square, a tank and a furnace by them", move |b, _| {
            for c in cells {
                b.place(c, CRYSTAL, &[])?;
            }
            b.place(tank, TANK, &[])?;
            if fed {
                b.set_tank(tank, grow::SOLUTION, 4000)?;
            }
            b.place(oven, "rustcraft:furnace", &[])
        })
        .until("the structure shows the rules' output", LIMIT, move |b, _| {
            Ok(b.power(cells[0]).is_some_and(|v| v.shown == wanted))
        })
        .wait(25)
        .check("its window: crystals, form, output and solution bars, the limit", move |b, _| {
            let s = b.sheet(cells[3]).ok_or("no sheet for a crystal")?;
            same("bars", s.bars.len(), 2)?;
            same("rows", s.rows.len(), 4)?;
            for key in ["rustcraft.structure.crystals", "rustcraft.structure.form", limit] {
                if !names(&s, key) {
                    return Err(format!("the window does not say {key}: {:?}", s.rows));
                }
            }
            same("severity", s.status.map(|x| x.severity), Some(severity))
        })
        .check("clients draw one crystal over the box from the anchor", move |b, _| {
            same("anchor", b.crystal_drawn(cells[0]), Some((Some([2, 1, 2]), false, fed)))?;
            for &c in &cells[1..] {
                same("member", b.crystal_drawn(c), Some((None, true, fed)))?;
            }
            Ok(())
        })
}

fn structure_window_and_look(o: crate::geom::Pos, t: &Tuning) -> Scenario {
    structure_scenario("s18_structure_window_and_look", o, t, true)
}

fn structure_unfed_window_and_look(o: crate::geom::Pos, t: &Tuning) -> Scenario {
    structure_scenario("s18_structure_unfed_window_and_look", o, t, false)
}

/// MN1: with no chest and no housing beside it, the miner puts its ore into a load dock beside
/// it; with a chest as well, the chest gets it and the dock nothing.
fn miner_fills_a_load_dock(o: crate::geom::Pos, _t: &Tuning) -> Scenario {
    let m = at(o, [5, 6, 3]);
    let dock = at(m, [0, 1, 0]);
    let chest = at(m, [0, 0, 1]);
    let ores = [at(m, [2, -6, -1]), at(m, [-2, -6, 1])];
    let raw = |b: &dyn super::Bench, p| super::count_of(&b.items(p), "minecraft:raw_iron");
    Scenario::new("s18_miner_fills_a_load_dock", plot_area(o))
        .then("a miner on two crystals, a load dock on it, an iron ore below", move |b, _| {
            b.place(m, "rustcraft:miner", &[])?;
            b.place(at(m, [1, 0, 0]), CRYSTAL, &[])?;
            b.place(at(m, [-1, 0, 0]), CRYSTAL, &[])?;
            b.place(dock, "rustcraft:dock_load", &[])?;
            for p in ores {
                for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                    b.place(at(p, d), "minecraft:stone", &[])?;
                }
            }
            b.place(ores[0], "minecraft:iron_ore", &[])
        })
        .until("the raw iron went into the load dock", 600, move |b, _| Ok(raw(b, dock) >= 1))
        .until(
            "its window: one ore found, none left, the load dock marked",
            200,
            move |b, _| {
                let s = b.sheet(m).ok_or("no sheet for the miner")?;
                let rows: Vec<String> = s.rows.iter().map(|r| r.to_string()).collect();
                Ok(rows.len() == 2
                    && rows[0].contains("rustcraft.miner.found_left")
                    && !rows[0].contains("rustcraft.miner.counting")
                    && rows[1].contains("\"[\"")
                    && rows[1].contains("rustcraft.miner.outlet.dock"))
            },
        )
        .then("a chest beside it too, another ore, the miner set anew", move |b, m2| {
            m2.mark("dock", f64::from(raw(b, dock)));
            b.place(chest, "minecraft:chest", &[])?;
            b.place(ores[1], "minecraft:iron_ore", &[])?;
            b.place(m, "minecraft:air", &[])?;
            b.place(m, "rustcraft:miner", &[])
        })
        .until("the chest comes first", 1200, move |b, _| Ok(raw(b, chest) >= 1))
        .check("and the dock got no more", move |b, m2| {
            same("dock", f64::from(raw(b, dock)), m2.get("dock"))
        })
}

/// Guide `vein` (ui.js): its window has the work modes, a progress bar beside the energy and the
/// free cells round it; boost is the mode a press sets.
fn vein_window(o: crate::geom::Pos, _t: &Tuning) -> Scenario {
    use crate::machines::{processor::WorkMode, sheet::Press};
    let vein = at(o, [5, 3, 5]);
    Scenario::new("s18_vein_window", plot_area(o))
        .then("a living vein in the open", move |b, _| b.place(vein, "rustcraft:vein", &[]))
        .check("its window: modes, two bars, free cells and grown blocks", move |b, _| {
            let s = b.sheet(vein).ok_or("no sheet for the vein")?;
            same("mode", s.mode, Some(WorkMode::Normal.code()))?;
            same("bars", s.bars.len(), 2)?;
            let rows: Vec<String> = s.rows.iter().map(|r| r.to_string()).collect();
            if !(rows.len() == 2 && rows[0].contains("rustcraft.vein.free") && rows[1].contains("rustcraft.vein.grown")) {
                return Err(format!("rows {rows:?}"));
            }
            Ok(())
        })
        .then("boost pressed", move |b, _| b.press_cursor(vein, Press::Mode(WorkMode::Boost).id(), None).map(|_| ()))
        .check("the window shows boost", move |b, _| {
            same("mode", b.sheet(vein).and_then(|s| s.mode), Some(WorkMode::Boost.code()))
        })
}

/// E7: a fed box of `size` crystals gives the spec's number (3×3: 108, 3×3×3: 561) and drinks
/// 0.1 bucket a minute a crystal.
fn structure_box(name: &'static str, o: crate::geom::Pos, t: &Tuning, height: i32) -> Scenario {
    use crate::power::{self as rules, Form, Shown};
    let mut cells = Vec::new();
    for y in 0..height {
        for x in 0..3 {
            for z in 0..3 {
                cells.push(at(o, [2 + x, y, 2 + z]));
            }
        }
    }
    let n = cells.len();
    let form = if height == 3 { Form::Box([3, 3, 3]) } else { Form::Box([1, 3, 3]) };
    let total = rules::structure_output(n, form, true, t);
    let tank = at(o, [1, 0, 2]);
    let first = cells[0];
    Scenario::new(name, plot_area(o))
        .then("the crystals in a box, a tank of solution and a furnace", move |b, _| {
            for &c in &cells {
                b.place(c, CRYSTAL, &[])?;
            }
            b.place(tank, TANK, &[])?;
            b.set_tank(tank, grow::SOLUTION, 16000)?;
            b.place(at(o, [5, 0, 2]), "rustcraft:furnace", &[])
        })
        .until("it gives the spec's output", LIMIT, move |b, _| {
            Ok(b.power(first).is_some_and(|v| v.blocks == n && v.shown == Shown::Gives(total)))
        })
        .until("one crystal drawn over the whole box", LIMIT, move |b, _| {
            Ok(b.crystal_drawn(first).map(|d| d.0) == Some(Some([3, height, 3])))
        })
}

fn structure_three_by_three(o: crate::geom::Pos, t: &Tuning) -> Scenario {
    structure_box("s18_structure_3x3", o, t, 1)
}

fn structure_cube(o: crate::geom::Pos, t: &Tuning) -> Scenario {
    structure_box("s18_structure_3x3x3", o, t, 3)
}

/// CR2: on a tank of water the seed stands; broken, it gives its one shard back.
fn seed_stands_without_solution(o: crate::geom::Pos, _t: &Tuning) -> Scenario {
    let tank = at(o, [2, 0, 2]);
    let seed = at(o, [2, 1, 2]);
    Scenario::new("s18_seed_stands_without_solution", plot_area(o))
        .then("a tank of water and a seed on it", move |b, _| {
            b.place(tank, TANK, &[])?;
            b.set_tank(tank, "minecraft:water", 8000)?;
            b.place(seed, SEED, &[])
        })
        .wait(100)
        .check("it does not grow and drinks no water", move |b, _| {
            let s = b.seed(seed).ok_or("no seed")?;
            same("progress", s.progress, 0.0)?;
            same("held", s.solution, 0.0)?;
            same("tank", b.tank(tank).map(|g| g.amount), Some(8000))
        })
        .check("broken, it gives its one shard back", move |b, _| {
            same("shards", shards_in(&b.break_block(seed, None)?), 1)
        })
}
