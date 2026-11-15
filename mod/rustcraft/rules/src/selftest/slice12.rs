//! Slice 12: the last blocks of the guide and the moving parts.
//!
//! Each machine is given its buffer of energy outright (`set_buffer`), so a scenario waits only for
//! the machine's own step.
//!
//! - The trash burns what its filter names and stands without a filter (UT2).
//! - The disintegrator takes a bucket apart into an iron ingot (DS1).
//! - The repair stand passes a whole tool to its output.
//! - The kitchen bakes bread from wheat once its toggle is on, tells Path-of-craft-tree `craft`
//!   and tells clients it works (its moving parts).
//! - The feeder feeds two cows; the shearer shears a sheep and milks a cow; the apiary collector
//!   empties a full hive into a honey bottle.
//! - The archaeology table brushes suspicious sand into its sand and a find.
//! - The projector remembers a block, and the building machine beside it puts it back once it is
//!   gone, telling Path-of-craft-tree `build`.
//! - The condenser's window has its four buttons; the enchanting machine without a condenser, the
//!   fishing station without water and an unpaired portal dock say why they stand.

use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::machines::sheet::Press;
use crate::tuning::Tuning;

pub const ALL: [Build; 15] = [
    trash_burns_what_its_filter_names,
    trash_without_a_filter_stands,
    disintegrator_takes_a_bucket_apart,
    repair_passes_a_whole_tool,
    kitchen_bakes_bread,
    feeder_feeds_a_pair,
    shearer_shears_a_sheep,
    shearer_milks_a_cow,
    apiary_empties_a_full_hive,
    archeo_brushes_sand,
    builder_builds_the_plan,
    xp_tank_window,
    enchanter_needs_a_condenser,
    fishery_needs_water,
    portal_dock_needs_a_pair,
];

const STONE: &str = "minecraft:stone";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a12;
const LIMIT: u64 = 400;
const FULL: f64 = 1000.0;

/// A floor of stone, x -1..=8 and z -3..=3 from `o`.
fn floor(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=8 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    Ok(())
}

/// A machine at `pos` with an owner and a full buffer.
fn machine(b: &mut dyn Bench, pos: Pos, block: &str) -> Result<(), String> {
    b.place(pos, block, &[])?;
    b.set_owner(pos, OWNER, 1)?;
    b.set_buffer(pos, FULL)
}

/// The translation key of its window's status line.
fn status(b: &dyn Bench, pos: Pos) -> String {
    b.sheet(pos)
        .and_then(|s| s.status)
        .and_then(|s| s.text.get("translate").and_then(|t| t.as_str()).map(str::to_owned))
        .unwrap_or_default()
}

/// How many of `item` its slots from `first` on hold (a fresh block entity's list may be shorter).
fn count_from(b: &dyn Bench, pos: Pos, first: usize, item: &str) -> u32 {
    count_of(&b.items(pos).into_iter().skip(first).collect::<Vec<_>>(), item)
}

fn says(b: &dyn Bench, pos: Pos, key: &str) -> bool {
    status(b, pos) == format!("rustcraft.util.{key}")
}

fn reported(b: &dyn Bench, kind: &str) -> bool {
    b.reports().iter().any(|r| r.starts_with(&format!("pct-machine {kind} ")))
}

fn trash_burns_what_its_filter_names(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_trash_burns_its_filter", plot_area(o))
        .then("a trash with dirt in its filter and dirt to burn", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:trash")?;
            b.put(p, 0, "minecraft:dirt", 1)?;
            b.put(p, 9, "minecraft:dirt", 6)?;
            b.put(p, 10, STONE, 3)
        })
        .until("the dirt is burnt", 120, move |b, _| {
            Ok(b.items(p).get(9).is_none_or(|s| s.as_ref().is_none_or(|s| s.1 == 0)))
        })
        .check("the filter and the stone stay", move |b, _| {
            same("filter", count_of(&b.items(p).into_iter().take(9).collect::<Vec<_>>(), "minecraft:dirt"), 1)?;
            same("stone", count_of(&b.items(p), STONE), 3)
        })
}

fn trash_without_a_filter_stands(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_trash_needs_a_filter", plot_area(o))
        .then("a trash with no filter", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:trash")
        })
        .until("it says it has no filter", 60, move |b, _| Ok(says(b, p, "trash_no_filter")))
}

fn disintegrator_takes_a_bucket_apart(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_disintegrator_bucket", plot_area(o))
        .then("a disintegrator with a bucket", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:disintegrator")?;
            b.put(p, 0, "minecraft:bucket", 1)
        })
        .until("an iron ingot in its output", LIMIT, move |b, _| {
            Ok(count_from(b, p, 1, "minecraft:iron_ingot") >= 1)
        })
}

fn repair_passes_a_whole_tool(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_repair_whole_tool_out", plot_area(o))
        .then("a repair stand with a whole iron pickaxe", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:repair")?;
            b.put(p, 0, "minecraft:iron_pickaxe", 1)
        })
        .until("the pickaxe is in its output slot", LIMIT, move |b, _| {
            Ok(b.items(p).get(5).cloned().flatten().is_some_and(|s| s.0 == "minecraft:iron_pickaxe"))
        })
}

fn kitchen_bakes_bread(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_kitchen_bakes_bread", plot_area(o))
        .then("a kitchen with wheat, bread turned on", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:kitchen")?;
            b.put(p, 0, "minecraft:wheat", 6)?;
            b.press_cursor(p, Press::Toggle(0).id(), None).map(|_| ())
        })
        .until("bread in its output", LIMIT, move |b, _| {
            Ok(count_from(b, p, 9, "minecraft:bread") >= 1)
        })
        .check("Path-of-craft-tree heard craft, clients that it works", move |b, _| {
            if !reported(b, "craft") {
                return Err(format!("no craft report in {:?}", b.reports()));
            }
            same("parts move", b.anim(p).map(|s| s.on), Some(true))
        })
}

fn feeder_feeds_a_pair(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_feeder_feeds_a_pair", plot_area(o))
        .then("a feeder with wheat and two cows by it", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:feeder")?;
            b.put(p, 0, "minecraft:wheat", 4)?;
            for x in [1.5, 5.5] {
                b.spawn_mob("minecraft:cow", [f64::from(o[0]) + x, f64::from(o[1] + 1), f64::from(o[2]) + 2.5], None)?;
            }
            Ok(())
        })
        .until("it fed them two wheat", LIMIT, move |b, _| Ok(count_of(&b.items(p), "minecraft:wheat") == 2))
        .check("Path-of-craft-tree heard breed", move |b, _| {
            if reported(b, "breed") { Ok(()) } else { Err(format!("no breed in {:?}", b.reports())) }
        })
}

fn shearer_shears_a_sheep(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_shearer_shears", plot_area(o))
        .then("a shearer with shears and a sheep by it", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:shearer")?;
            b.put(p, 0, "minecraft:shears", 1)?;
            b.spawn_mob("minecraft:sheep", [f64::from(o[0]) + 3.5, f64::from(o[1] + 1), f64::from(o[2]) + 2.5], None)
                .map(|_| ())
        })
        .until("wool in its output", LIMIT, move |b, _| {
            Ok(b.items(p).iter().skip(2).flatten().any(|s| s.0.ends_with("_wool")))
        })
        .check("Path-of-craft-tree heard shear", move |b, _| {
            if reported(b, "shear") { Ok(()) } else { Err(format!("no shear in {:?}", b.reports())) }
        })
}

fn shearer_milks_a_cow(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_shearer_milks", plot_area(o))
        .then("a shearer with a bucket and a cow by it", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:shearer")?;
            b.put(p, 1, "minecraft:bucket", 1)?;
            b.spawn_mob("minecraft:cow", [f64::from(o[0]) + 3.5, f64::from(o[1] + 1), f64::from(o[2]) + 2.5], None)
                .map(|_| ())
        })
        .until("a milk bucket in its output", LIMIT, move |b, _| {
            Ok(count_from(b, p, 2, "minecraft:milk_bucket") == 1)
        })
}

fn apiary_empties_a_full_hive(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    let hive = at(o, [5, 1, 0]);
    Scenario::new("util_apiary_takes_honey", plot_area(o))
        .then("a collector with a glass bottle, a full hive two blocks away", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:apiary")?;
            b.put(p, 0, "minecraft:glass_bottle", 1)?;
            b.place(hive, "minecraft:beehive", &[("honey_level", "5")])
        })
        .until("a honey bottle in its output", LIMIT, move |b, _| {
            Ok(count_from(b, p, 1, "minecraft:honey_bottle") == 1)
        })
        .check("the hive is empty", move |b, _| same("honey", b.property(hive, "honey_level"), Some("0".to_owned())))
}

fn archeo_brushes_sand(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_archeo_brushes", plot_area(o))
        .then("a table with suspicious sand and a brush", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:archeo")?;
            b.put(p, 0, "minecraft:suspicious_sand", 1)?;
            b.put(p, 1, "minecraft:brush", 1)
        })
        .until("its sand back", LIMIT, move |b, _| Ok(count_from(b, p, 2, "minecraft:sand") == 1))
        .check("Path-of-craft-tree heard brush", move |b, _| {
            if reported(b, "brush") { Ok(()) } else { Err(format!("no brush in {:?}", b.reports())) }
        })
}

fn builder_builds_the_plan(o: Pos, _: &Tuning) -> Scenario {
    let proj = at(o, [3, 1, 0]);
    let build = at(o, [4, 1, 0]);
    let cell = at(proj, [-1, 1, 0]);
    Scenario::new("util_builder_builds_the_plan", plot_area(o))
        .then("a projector that remembers a stone over it, a builder beside it", move |b, _| {
            floor(b, o)?;
            machine(b, proj, "rustcraft:projector")?;
            b.place(cell, STONE, &[])?;
            b.press_cursor(proj, Press::Toggle(0).id(), None)?;
            b.place(cell, "minecraft:air", &[])?;
            machine(b, build, "rustcraft:builder")?;
            b.put(build, 0, STONE, 4)
        })
        .until("the stone stands again", LIMIT, move |b, _| Ok(b.block(cell) == STONE))
        .check("Path-of-craft-tree heard build", move |b, _| {
            if reported(b, "build") { Ok(()) } else { Err(format!("no build in {:?}", b.reports())) }
        })
}

fn xp_tank_window(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_xp_tank_window", plot_area(o))
        .then("a condenser", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:xp_tank")
        })
        .until("its window has four buttons", 60, move |b, _| Ok(b.sheet(p).is_some_and(|s| s.toggles.len() == 4)))
}

fn enchanter_needs_a_condenser(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_enchanter_needs_a_condenser", plot_area(o))
        .then("an enchanting machine with a book and lapis, no condenser", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:enchanter")?;
            b.put(p, 0, "minecraft:book", 1)?;
            b.put(p, 1, "minecraft:lapis_lazuli", 3)
        })
        .until("it says it has no condenser", LIMIT, move |b, _| Ok(says(b, p, "enchant_no_tank")))
}

fn fishery_needs_water(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_fishery_needs_water", plot_area(o))
        .then("a fishing station with a rod on dry stone", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:fishery")?;
            b.put(p, 0, "minecraft:fishing_rod", 1)
        })
        .until("it says it needs water", 60, move |b, _| Ok(says(b, p, "fish_no_water")))
}

fn portal_dock_needs_a_pair(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("util_portal_dock_needs_a_pair", plot_area(o))
        .then("a portal dock", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:portal_dock")
        })
        .until("it says it has no pair", 60, move |b, _| Ok(says(b, p, "dock_unpaired")))
}
