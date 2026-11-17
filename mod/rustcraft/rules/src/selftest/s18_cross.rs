//! Slice 18, the cross-stream agent: what the streams handed each other after the merge.
//!
//! - The pump has the work modes (owner 2026-11-17): boost pumps a bucket in half the time for
//!   three times the energy, and its window shows the mode.
//! - The placer and the breaker work faster on a higher tier (A8): their windows' pace.

use super::{Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::machines::fluid_windows::pump as pw;
use crate::machines::panel::ENERGY_SCALE;
use crate::machines::processor::WorkMode;
use crate::tuning::Tuning;

pub const ALL: [Build; 2] = [pump_boost_mode, placer_and_breaker_by_tier];

const STONE: &str = "minecraft:stone";
const LAVA: &str = "minecraft:lava";

fn pump_boost_mode(o: Pos, t: &Tuning) -> Scenario {
    let pump = at(o, [0, 1, 0]);
    let boost_ticks = (t.pump_ticks / 2.0).round() as i32;
    let boost_energy = t.pump_energy * 3.0;
    Scenario::new("s18_pump_boost_mode", plot_area(o))
        .then("a pump over lava, on a crystal", move |b, _| {
            for x in -1..=1 {
                for z in -1..=1 {
                    b.place(at(o, [x, -1, z]), STONE, &[])?;
                    if (x, z) != (0, 0) {
                        b.place(at(o, [x, 0, z]), STONE, &[])?;
                    }
                }
            }
            b.place(o, LAVA, &[])?;
            b.place(pump, "rustcraft:pump", &[])?;
            b.place(at(pump, [0, 1, 0]), "rustcraft:crystal", &[])
        })
        .wait(10)
        .check("it starts in the normal mode", move |b, _| {
            same("mode", pw::mode(&b.menu_data(pump)), WorkMode::Normal)
        })
        .then("the boost button is pressed", move |b, _| {
            b.press(pump, i32::from(WorkMode::Boost.code()), None)
        })
        .wait(10)
        .check("it saved boost, and its window says boost, half the ticks and three times the E", move |b, _| {
            same("saved mode", b.long(pump, "work"), i64::from(WorkMode::Boost.code()))?;
            let data = b.menu_data(pump);
            same("window mode", pw::mode(&data), WorkMode::Boost)?;
            same("ticks a bucket", data.get(pw::TICKS_AT).copied(), Some(boost_ticks))?;
            let asked = f64::from(data.get(4).copied().unwrap_or(0)) / ENERGY_SCALE;
            same("E/s asked", (asked * 10.0).round(), (boost_energy * 10.0).round())
        })
}

/// Seconds as the placer's window writes them: «2», «1,3».
fn secs(v: f64) -> String {
    format!("{v:.1}").trim_end_matches(".0").replace('.', ",")
}

fn pace_shown(b: &dyn super::Bench, pos: Pos, secs: &str) -> bool {
    b.sheet(pos).is_some_and(|s| {
        s.rows.iter().any(|r| {
            let r = r.to_string();
            r.contains("rustcraft.placer.pace") && r.contains(&format!("\"{secs}\""))
        })
    })
}

fn placer_and_breaker_by_tier(o: Pos, t: &Tuning) -> Scenario {
    let cells = [
        ("rustcraft:placer", "2", t.placer_ticks, at(o, [1, 1, 0])),
        ("rustcraft:placer", "4", t.placer_ticks, at(o, [3, 1, 0])),
        ("rustcraft:breaker", "2", t.breaker_ticks, at(o, [5, 1, 0])),
        ("rustcraft:breaker", "3", t.breaker_ticks, at(o, [7, 1, 0])),
    ];
    let step = t.tier_speed_step;
    let tps = t.ticks_per_second;
    Scenario::new("s18_placer_and_breaker_by_tier", plot_area(o))
        .then("a placer and a breaker of their first tier and of a higher one", move |b, _| {
            for (block, tier, _, p) in cells {
                b.place(at(p, [0, -1, 0]), STONE, &[])?;
                b.place(p, block, &[("tier", tier), ("facing", "north")])?;
            }
            Ok(())
        })
        .until("each window gives the pace of its tier", 40, move |b, _| {
            Ok(cells.iter().all(|&(_, tier, ticks, p)| {
                let above = f64::from(tier.parse::<u8>().unwrap_or(2) - 2);
                pace_shown(b, p, &secs(ticks / (1.0 + step * above) / tps))
            }))
        })
}
