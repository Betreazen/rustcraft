//! Slice 18, the cross-stream agent: what the streams handed each other after the merge.
//!
//! - The pump has the work modes (owner 2026-11-17): boost pumps a bucket in half the time for
//!   three times the energy, and its window shows the mode.

use super::{Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::machines::fluid_windows::pump as pw;
use crate::machines::panel::ENERGY_SCALE;
use crate::machines::processor::WorkMode;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [pump_boost_mode];

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
