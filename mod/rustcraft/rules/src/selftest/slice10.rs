//! Slice 10: the mixer and the nutrient solution, the channels of tiers 2 and 3, the other tools
//! of tier 2 (the cargo hold with its load dock, the grab, the scoop) and the hydro turbine.
//!
//! - The mixer makes a bucket of solution out of water, bone meal and redstone: water from a tank
//!   it touches, the solution into the tank on top of it (guide `mixer`). On a fluid channel of
//!   tier 2 it takes water and gives solution by the same channel (M13, guide `ch_fluid`), and the
//!   solution feeds a crystal structure (E8); on tier 1 the second fluid finds no lane.
//! - Its window: energy, water and product bars, the mode switch and the module sockets.
//! - The crafting table makes housings of each tier out of that tier's alloy (round 31).
//! - A cargo worker rides from the load dock to the unload dock with its load (W6); the grab
//!   breaks an ore by the way and picks up an item lying there; the scoop digs earth beside and
//!   under the way and spares the block under the rails.
//! - Turbines in a current give by their place in it (E12); a turbine out of water says so.

use super::magistral::line;
use super::{Bench, Build, Memo, Scenario, at, base, count_of, near, plot_area, same};
use crate::geom::{Face, Pos};
use crate::machines::mixer::MixerView;
use crate::machines::processor::WorkMode;
use crate::machines::sheet::{Press, Sheet};
use crate::modules::Module;
use crate::monorail::shape;
use crate::net::items::Mode;
use crate::net::settings::JointSetting;
use crate::power::{self as rules, Shown, Water};
use crate::tuning::Tuning;
use crate::worker::Tool;
use std::collections::BTreeSet;

pub const ALL: [Build; 12] = [
    mixer_makes_solution,
    mixer_feeds_crystals_on_tier_2,
    mixer_tier_1_channel_has_no_lane,
    mixer_window,
    housing_recipes_by_tier,
    cargo_carries_its_load,
    grab_takes_ore_and_items,
    scoop_digs_beside_the_way,
    turbines_in_a_current,
    turbine_out_of_water,
    load_dock_window,
    turbine_window,
];

const MIXER: &str = "rustcraft:mixer";
const TANK: &str = "rustcraft:tank";
const CRYSTAL: &str = "rustcraft:crystal";
const MONORAIL: &str = "rustcraft:monorail";
const DOCK_LOAD: &str = "rustcraft:dock_load";
const DOCK_UNLOAD: &str = "rustcraft:dock_unload";
const TURBINE: &str = "rustcraft:turbine";
const STONE: &str = "minecraft:stone";
const WATER: &str = "minecraft:water";
const BONE_MEAL: &str = "minecraft:bone_meal";
const REDSTONE: &str = "minecraft:redstone";
const COBBLE: &str = "minecraft:cobblestone";
/// The owner the scenarios' workers report for (nobody on the server).
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a10;
const LIMIT: u64 = 400;
/// A worker rides there and back at 2 blocks a second.
const RIDE: u64 = 800;

fn mixer_view(b: &dyn Bench, pos: Pos) -> Result<MixerView, String> {
    b.mixer(pos)
        .ok_or_else(|| format!("nothing known of the mixer at {pos:?}"))
}

fn sheet(b: &dyn Bench, pos: Pos) -> Result<Sheet, String> {
    b.sheet(pos)
        .ok_or_else(|| format!("no window sheet at {pos:?}"))
}

/// A mixer at `pos` with bone meal and redstone for `batches` batches.
fn mixer(b: &mut dyn Bench, pos: Pos, batches: u32) -> Result<(), String> {
    b.place(pos, MIXER, &[])?;
    b.put(pos, 0, BONE_MEAL, batches)?;
    b.put(pos, 1, REDSTONE, batches)
}

fn mixer_makes_solution(o: Pos, _: &Tuning) -> Scenario {
    let m = at(o, [3, 1, 3]);
    let water = at(o, [2, 1, 3]);
    let product = at(o, [3, 2, 3]);
    Scenario::new("mixer_makes_solution", plot_area(o))
        .then(
            "a mixer between two crystals, a water tank beside it, a tank on top",
            move |b, _| {
                mixer(b, m, 2)?;
                b.place(water, TANK, &[])?;
                b.set_tank(water, WATER, 8000)?;
                b.place(product, TANK, &[])?;
                b.place(at(m, [0, 0, -1]), CRYSTAL, &[])?;
                b.place(at(m, [0, 0, 1]), CRYSTAL, &[])
            },
        )
        .until(
            "a bucket of solution in the tank on top",
            LIMIT,
            move |b, _| {
                Ok(b.tank(product).is_some_and(|g| {
                    g.fluid.as_deref() == Some(rules::SOLUTION) && g.amount >= 1000
                }))
            },
        )
        .check(
            "a batch took one bone meal, one redstone and a bucket of water",
            move |b, _| {
                let items = b.items(m);
                let made = 2 - count_of(&items, BONE_MEAL);
                same("redstone left", count_of(&items, REDSTONE), 2 - made)?;
                let tank = b.tank(water).map_or(0, |g| g.amount);
                let v = mixer_view(b, m)?;
                let product_now =
                    b.tank(product).map_or(0, |g| g.amount) + v.product.as_ref().map_or(0, |p| p.1);
                same("solution made, mB", product_now, u64::from(made) * 1000)?;
                same(
                    "water in the tank and the mixer, mB",
                    tank + v.water,
                    8000 - u64::from(made) * 1000,
                )
            },
        )
}

/// A line of tier-`tier` housings: a water tank («из блока») at housing 1, the mixer at housing 2
/// with a crystal of its own, a crystal pair at housing 4.
fn mixer_on_a_channel(b: &mut dyn Bench, o: Pos, tier: u8) -> Result<(), String> {
    line(b, o, 1, 5, tier)?;
    let water = at(o, [1, 0, 1]);
    b.place(water, TANK, &[])?;
    b.set_tank(water, WATER, 16_000)?;
    // Slice 18: the mixer's tanks hold 16 buckets (`ui.js` mixer), so the water tank holds more
    // than that: the water keeps the tier-1 channel's one lane.
    b.place(at(water, [0, 1, 0]), TANK, &[])?;
    b.set_tank(at(water, [0, 1, 0]), WATER, 16_000)?;
    b.set_joint(
        at(o, [1, 0, 0]),
        Face::ZPos,
        crate::net::topology::FLUID_SLOT,
        JointSetting::of_mode(Mode::From),
    )?;
    let m = at(o, [2, 0, -1]);
    mixer(b, m, 8)?;
    b.place(at(m, [0, 0, -1]), CRYSTAL, &[])?;
    b.place(at(o, [4, 0, 1]), CRYSTAL, &[])?;
    b.place(at(o, [5, 0, 1]), CRYSTAL, &[])
}

fn mixer_feeds_crystals_on_tier_2(o: Pos, t: &Tuning) -> Scenario {
    let pair = [at(o, [4, 0, 1]), at(o, [5, 0, 1])];
    let set: BTreeSet<Pos> = pair.into_iter().collect();
    let fed = rules::structure_output(2, rules::form(&set), true, t);
    Scenario::new("mixer_feeds_crystals_on_tier_2", plot_area(o))
        .then(
            "water, the mixer and a crystal pair on a tier-2 fluid channel",
            move |b, _| mixer_on_a_channel(b, o, 2),
        )
        .until(
            "the solution went along the channel into the crystals",
            LIMIT * 2,
            move |b, _| {
                Ok(b.power(pair[0])
                    .is_some_and(|v| v.fluid > 0.0 && matches!(v.shown, Shown::Gives(_))))
            },
        )
        .check("the structure gives as one fed by solution", move |b, _| {
            let v = b.power(pair[0]).ok_or("no crystal")?;
            near("structure output, E/s", v.output, fed, 0.05)
        })
}

fn mixer_tier_1_channel_has_no_lane(o: Pos, _: &Tuning) -> Scenario {
    let m = at(o, [2, 0, -1]);
    Scenario::new("mixer_tier_1_channel_has_no_lane", plot_area(o))
        .then(
            "water, the mixer and a crystal pair on a tier-1 fluid channel",
            move |b, _| mixer_on_a_channel(b, o, 1),
        )
        .until(
            "the solution finds no lane: the channel carries one fluid",
            LIMIT * 2,
            move |b, _| Ok(b.no_lane(m).as_deref() == Some(rules::SOLUTION)),
        )
}

fn mixer_window(o: Pos, _: &Tuning) -> Scenario {
    let m = at(o, [3, 1, 3]);
    let speed = Module::Speed.item();
    Scenario::new("window_mixer_bars_mode_and_sockets", plot_area(o))
        .then("a mixer with no water", move |b, _| mixer(b, m, 1))
        .until(
            "its sheet: three bars, the mode, three sockets, «Нет воды»",
            60,
            move |b, _| {
                let s = sheet(b, m)?;
                Ok(s.bars.len() == 3
                    && s.mode == Some(WorkMode::Normal.code())
                    && s.modules.len() == 3
                    && s.status.is_some()
                    && mixer_view(b, m)
                        .is_ok_and(|v| v.shown == Some(crate::machines::mixer::Shown::NoWater)))
            },
        )
        .then(
            "«Форсаж» pressed, a speed module put in",
            move |b, _| {
                b.press(m, Press::Mode(WorkMode::Boost).id(), None)?;
                let left = b.press_cursor(m, Press::Socket(0).id(), Some((speed, 1)))?;
                same("cursor after the socket", left, None)
            },
        )
        .until("the sheet shows both", 60, move |b, _| {
            let s = sheet(b, m)?;
            Ok(s.mode == Some(WorkMode::Boost.code())
                && s.modules.first().cloned().flatten().as_deref() == Some(speed))
        })
}

fn housing_recipes_by_tier(o: Pos, _: &Tuning) -> Scenario {
    // Round 31: the channels are built into the housing, whose tier sets their speed; a housing
    // of each tier is made from its alloy, eight at a time.
    Scenario::new("housings_recipes_by_tier", plot_area(o)).check(
        "a crafting table makes eight housings of each tier from its alloy",
        |b, _| {
            let alloys = [
                ("rustcraft:coppersteel", ""),
                ("rustcraft:sparksteel", "_2"),
                ("rustcraft:crystallite", "_3"),
                ("rustcraft:starsteel", "_4"),
            ];
            for (alloy, tier) in alloys {
                let (a, g) = (Some(alloy), Some("minecraft:glass"));
                let grid = [a, g, a, g, None, g, a, g, a];
                same(
                    "the crafting table's result",
                    b.craft(&grid),
                    Some((format!("rustcraft:housing{tier}"), 8)),
                )?;
            }
            Ok(())
        },
    )
}

// ---- The tools ----

const TRACK: i32 = 6;
const SPOT: i32 = 3;

fn rail(o: Pos, x: i32) -> Pos {
    at(o, [x, 1, 0])
}

/// A floor of stone and a straight track of floor rails on it, x 0..=TRACK, a charge dock under its
/// far end (0.19.1: every worker's base; the scenarios add the unload dock their own way).
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
    base(b, None, Some(rail(o, TRACK)))
}

fn worker_hold(b: &dyn Bench, m: &Memo) -> Result<Vec<super::Slot>, String> {
    b.worker(m.get("worker") as u64)
        .map(|w| w.hold.slots)
        .ok_or_else(|| "the worker is gone".to_owned())
}

fn spawn(b: &mut dyn Bench, m: &mut Memo, o: Pos, x: i32, tool: Tool) -> Result<(), String> {
    let id = b.spawn_worker(rail(o, x), OWNER, Some(tool), &[], None)?;
    m.mark("worker", id as f64);
    Ok(())
}

fn cargo_carries_its_load(o: Pos, _: &Tuning) -> Scenario {
    let load = at(o, [0, 1, 1]);
    let unload = at(o, [TRACK, 1, 1]);
    Scenario::new("worker_cargo_carries_its_load", plot_area(o))
        .then(
            "a track, a load dock with cobblestone at one end, an unload dock at the other",
            move |b, _| {
                track(b, o)?;
                b.place(load, DOCK_LOAD, &[])?;
                b.put(load, 0, COBBLE, 40)?;
                b.place(unload, DOCK_UNLOAD, &[])
            },
        )
        .then("a cargo worker in the middle", move |b, m| {
            spawn(b, m, o, SPOT, Tool::Hold)
        })
        .until(
            "the cobblestone came to the unload dock",
            RIDE,
            move |b, _| Ok(count_of(&b.items(unload), COBBLE) == 40),
        )
        .check("the load dock is empty", move |b, _| {
            same(
                "cobblestone left there",
                count_of(&b.items(load), COBBLE),
                0,
            )
        })
}

fn grab_takes_ore_and_items(o: Pos, _: &Tuning) -> Scenario {
    let ore = at(o, [SPOT, 1, 1]);
    let item_at = [
        f64::from(o[0] + SPOT) + 0.5,
        f64::from(o[1] + 1) + 0.1,
        f64::from(o[2] - 1) + 0.5,
    ];
    let around = (
        [item_at[0] - 1.0, item_at[1] - 1.0, item_at[2] - 1.0],
        [item_at[0] + 1.0, item_at[1] + 1.0, item_at[2] + 1.0],
    );
    Scenario::new("worker_grab_takes_ore_and_items", plot_area(o))
        .then(
            "a track, iron ore beside it, cobblestone lying on the other side",
            move |b, _| {
                track(b, o)?;
                base(b, Some(rail(o, 0)), None)?;
                b.place(ore, "minecraft:iron_ore", &[])
            },
        )
        .then("a grab worker on the track", move |b, m| {
            b.drop_item(item_at, COBBLE, 3)?;
            spawn(b, m, o, 0, Tool::Grab)
        })
        .until(
            "the raw iron and the cobblestone are in its hold",
            RIDE,
            move |b, m| {
                let hold = worker_hold(b, m)?;
                Ok(count_of(&hold, "minecraft:raw_iron") >= 1 && count_of(&hold, COBBLE) == 3)
            },
        )
        .check("the ore is gone and nothing lies there", move |b, _| {
            same("ore cell", b.block(ore), "minecraft:air".to_owned())?;
            same("items lying", b.lying(around.0, around.1), Vec::new())
        })
        .until("the ore is reported as a pickaxe's break", 40, |b, _| {
            let owner = format!("{OWNER:032x}");
            Ok(b.reports().iter().any(|l| {
                l.contains(&format!("break {owner} ")) && l.ends_with("minecraft:iron_ore pickaxe")
            }))
        })
}

fn scoop_digs_beside_the_way(o: Pos, _: &Tuning) -> Scenario {
    let dirt = at(o, [SPOT, 0, 1]);
    let sand = at(o, [SPOT, -1, -1]);
    let under = at(o, [SPOT, 0, 0]);
    Scenario::new("worker_scoop_digs_beside_the_way", plot_area(o))
        .then(
            "a track, dirt and sand beside and under it, dirt under the rail",
            move |b, _| {
                track(b, o)?;
                base(b, Some(rail(o, 0)), None)?;
                b.place(dirt, "minecraft:dirt", &[])?;
                // Sand falls: stone holds it up.
                b.place(at(sand, [0, -1, 0]), STONE, &[])?;
                b.place(sand, "minecraft:sand", &[])?;
                b.place(under, "minecraft:dirt", &[])
            },
        )
        .then("a scoop worker on the track", move |b, m| {
            spawn(b, m, o, 0, Tool::Scoop)
        })
        .until(
            "the dirt and the sand are in its hold",
            RIDE,
            move |b, m| {
                let hold = worker_hold(b, m)?;
                Ok(
                    count_of(&hold, "minecraft:dirt") >= 1
                        && count_of(&hold, "minecraft:sand") >= 1,
                )
            },
        )
        .check(
            "the strip is dug and the dirt under the rail stays",
            move |b, _| {
                same("dirt cell", b.block(dirt), "minecraft:air".to_owned())?;
                same("sand cell", b.block(sand), "minecraft:air".to_owned())?;
                same(
                    "under the rail",
                    b.block(under),
                    "minecraft:dirt".to_owned(),
                )
            },
        )
}

// ---- The hydro turbine ----

/// A stone channel along x with a water source at x 0, turbines in its side wall at x 1..=3.
fn current(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=6 {
        for z in -1..=1 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
        b.place(at(o, [x, 1, -1]), STONE, &[])?;
        if !(1..=3).contains(&x) {
            b.place(at(o, [x, 1, 1]), STONE, &[])?;
        }
    }
    b.place(at(o, [-1, 1, 0]), STONE, &[])?;
    b.place(at(o, [6, 1, 0]), STONE, &[])?;
    for x in 1..=3 {
        b.place(at(o, [x, 1, 1]), TURBINE, &[])?;
    }
    b.place(at(o, [0, 1, 0]), WATER, &[])
}

fn turbines_in_a_current(o: Pos, t: &Tuning) -> Scenario {
    let turbines = [at(o, [1, 1, 1]), at(o, [2, 1, 1]), at(o, [3, 1, 1])];
    let total: f64 = (0..3u8)
        .map(|i| rules::turbine_output(Water::Flowing(i + 1), 0, u32::from(i), t))
        .sum();
    let want = rules::turbine_output(Water::Flowing(3), 0, 2, t);
    Scenario::new("power_turbines_in_a_current", plot_area(o))
        .then(
            "a channel of water flowing past three turbines",
            move |b, _| current(b, o),
        )
        .until(
            "the three are one structure giving by their place in the current",
            LIMIT,
            move |b, _| {
                Ok(b.power(turbines[0]).is_some_and(|v| {
                    v.blocks == 3 && matches!(v.shown, Shown::Gives(x) if (x - total).abs() < 0.01)
                }))
            },
        )
        .check("the last one gives the most", move |b, _| {
            let last = b.power(turbines[2]).ok_or("no turbine")?.output;
            near("the third turbine, E/s", last, want, 0.01)
        })
}

fn turbine_out_of_water(o: Pos, _: &Tuning) -> Scenario {
    let dry = at(o, [3, 2, 3]);
    Scenario::new("power_turbine_out_of_water", plot_area(o))
        .then("a turbine in the air", move |b, _| {
            b.place(dry, TURBINE, &[])
        })
        .until("it says it is not in water", LIMIT, move |b, _| {
            Ok(b.power(dry)
                .is_some_and(|v| v.shown == Shown::NotInWater && v.output == 0.0))
        })
}

// ---- The windows ----

fn load_dock_window(o: Pos, _: &Tuning) -> Scenario {
    let dock = at(o, [3, 1, 3]);
    Scenario::new("window_load_dock_shows_its_slots", plot_area(o))
        .then("a load dock with cobblestone", move |b, _| {
            b.place(dock, DOCK_LOAD, &[])?;
            b.put(dock, 0, COBBLE, 10)
        })
        .until("its sheet: its lines and a state", 60, move |b, _| {
            let s = sheet(b, dock)?;
            // Slice 18: «Берёт груз · …» and «Грузит · всё подряд», as the guide's window.
            Ok(s.rows.len() == 2 && s.status.is_some())
        })

}

fn turbine_window(o: Pos, _: &Tuning) -> Scenario {
    let turbine = at(o, [1, 1, 1]);
    Scenario::new("window_turbine_shows_its_water", plot_area(o))
        .then("a turbine in a current", move |b, _| current(b, o))
        .until("its sheet: three lines and a state", LIMIT, move |b, _| {
            let s = sheet(b, turbine)?;
            Ok(s.rows.len() == 3 && s.status.is_some())
        })
}
