//! 0.21 (spec 43, ED1–ED4; owner 2026-11-19: «пока из них не тянут энергию, они и не потребляют
//! катализатор»): a crystal structure drinks its solution only for the energy taken from it.
//!
//! - A 2×2 of crystals fed from a tank, nothing beside it: it shows «Простой» and keeps its
//!   solution to the millibucket for 5 seconds; its window says the solution is not drunk.
//! - An empty battery on one crystal takes that crystal's quarter of the output: the structure
//!   drinks a quarter of its full rate, and its window says the rate now and how long it lasts.
//! - The battery full: it takes nothing, and the structure stops drinking again.

use super::{Build, Scenario, at, near, plot_area, same};
use crate::geom::Pos;
use crate::power::{self as rules, Form, PowerView, Shown};
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [structure_drinks_only_when_drawn];

const CRYSTAL: &str = "rustcraft:crystal";
const TANK: &str = "rustcraft:tank";
const BATTERY: &str = "rustcraft:battery";
const LIMIT: u64 = 200;
/// Ticks the solution is watched over: 5 seconds, 8,3 mB at a 2×2's full rate.
const SPAN: u64 = 100;

fn view(b: &dyn super::Bench, pos: Pos) -> Result<PowerView, String> {
    b.power(pos).ok_or_else(|| format!("nothing known of the crystal at {pos:?}"))
}

/// Whether the structure's window (its solution bar or its status) names `key`.
fn window_says(b: &dyn super::Bench, pos: Pos, key: &str) -> bool {
    b.sheet(pos).is_some_and(|s| {
        s.bars.iter().map(|x| &x.value).chain(s.status.iter().map(|x| &x.text)).any(|v| v.to_string().contains(key))
    })
}

fn structure_drinks_only_when_drawn(o: Pos, t: &Tuning) -> Scenario {
    let cells = [[2, 0, 2], [3, 0, 2], [2, 0, 3], [3, 0, 3]].map(|d| at(o, d));
    let tank = at(o, [1, 0, 2]);
    let battery = at(cells[3], [0, 1, 0]);
    let total = rules::structure_output(4, Form::Box([1, 2, 2]), true, t);
    let seconds = SPAN as f64 / t.ticks_per_second;
    let full_capacity = rules::battery(1, &[], t).capacity;
    let full_rate = rules::solution_per_second(4, t);
    let still = move |b: &mut dyn super::Bench, m: &mut super::Memo| -> Result<(), String> {
        let v = view(b, cells[0])?;
        near("solution drunk with nobody taking, mB", m.get("solution") - v.fluid, 0.0, 1e-6)?;
        same("idle", v.idle, true)
    };
    Scenario::new("catalyst21_structure_drinks_only_when_drawn", plot_area(o))
        .then("a 2×2 of crystals and a tank of solution, nothing to feed", move |b, _| {
            for c in cells {
                b.place(c, CRYSTAL, &[])?;
            }
            b.place(tank, TANK, &[])?;
            b.set_tank(tank, rules::SOLUTION, 1000)
        })
        .until("the tank poured into the structure, which stands idle", LIMIT, move |b, m| {
            let v = view(b, cells[0])?;
            m.mark("solution", v.fluid);
            let poured = b.tank(tank).is_none_or(|g| g.amount == 0);
            Ok(poured && v.shown == Shown::Gives(total) && v.fluid > 0.0 && v.idle)
        })
        .wait(SPAN)
        .check("ED2, ED3: nobody takes its energy, so it drinks nothing", still)
        .check("its look and window say it is idle and the solution is not drunk", move |b, _| {
            same("status idle", window_says(b, cells[0], "rustcraft.power.structure_idle"), true)?;
            same("bar idle", window_says(b, cells[0], "rustcraft.structure.solution_idle"), true)
        })
        .then("an empty battery on one crystal", move |b, _| b.place(battery, BATTERY, &[]))
        .until("the battery takes that crystal's share", LIMIT, move |b, m| {
            let v = view(b, cells[0])?;
            m.mark("solution", v.fluid);
            m.mark("drawn", v.drawn);
            Ok(v.drawn > 0.0 && !v.idle)
        })
        .wait(SPAN)
        .check("ED2: it drinks for the share taken, a quarter of its full rate", move |b, m| {
            let v = view(b, cells[0])?;
            near("drawn, E/s", m.get("drawn"), total / 4.0, 0.01)?;
            let wanted = full_rate * rules::drawn_share(m.get("drawn"), total) * seconds;
            near("solution drunk, mB", m.get("solution") - v.fluid, wanted, 0.3)?;
            same("less than at full draw", m.get("solution") - v.fluid < full_rate * seconds / 2.0, true)
        })
        .check("ED4: its window says the rate now and how long the solution lasts", move |b, _| {
            same("bar now", window_says(b, cells[0], "rustcraft.structure.solution_now"), true)
        })
        .then("the battery full", move |b, _| {
            // The battery keeps its charge in its entity's `energy`, as a station does.
            b.set_station_energy(battery, full_capacity)
        })
        .until("the full battery takes nothing and the structure idles", LIMIT, move |b, m| {
            let v = view(b, cells[0])?;
            m.mark("solution", v.fluid);
            Ok(v.idle && v.drawn == 0.0)
        })
        .wait(SPAN)
        .check("ED1: full batteries draw nothing, so it drinks nothing", still)
}
