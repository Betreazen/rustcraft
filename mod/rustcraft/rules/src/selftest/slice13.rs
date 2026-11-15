//! Slice 13: the gear's batteries.
//!
//! - The charger fills an energy blade's battery from its own buffer.
//! - The disintegrator does not take apart a blade that still holds charge (DS4).
//! - The living vein grows a block of ore beside it out of a stone and its energy.

use super::{Bench, Build, Scenario, at, plot_area};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 3] = [charger_fills_a_blade, disintegrator_keeps_a_charged_blade, vein_grows_ore];

const STONE: &str = "minecraft:stone";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a13;

fn floor(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=8 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    Ok(())
}

fn machine(b: &mut dyn Bench, pos: Pos, block: &str) -> Result<(), String> {
    b.place(pos, block, &[])?;
    b.set_owner(pos, OWNER, 1)?;
    b.set_buffer(pos, 1000.0)
}

fn charger_fills_a_blade(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("gear_charger_fills_a_blade", plot_area(o))
        .then("a charger with an empty sparksteel blade", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:charger")?;
            b.put_gear(p, 0, "rustcraft:blade_2", 0.0)
        })
        .until("the blade holds charge", 200, move |b, _| Ok(b.gear_charge(p, 0).is_some_and(|c| c > 100.0)))
}

fn disintegrator_keeps_a_charged_blade(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("gear_disintegrator_keeps_charged", plot_area(o))
        .then("a disintegrator with a charged blade", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:disintegrator")?;
            b.put_gear(p, 0, "rustcraft:blade_2", 500.0)
        })
        .until("it says the item does not come apart", 300, move |b, _| {
            let key = b
                .sheet(p)
                .and_then(|s| s.status)
                .and_then(|s| s.text.get("translate").and_then(|t| t.as_str()).map(str::to_owned));
            Ok(key.as_deref() == Some("rustcraft.util.dis_refused"))
        })
}

fn vein_grows_ore(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 2, 0]);
    Scenario::new("vein_grows_ore", plot_area(o))
        .then("a living vein with cobblestone and a full buffer", move |b, _| {
            floor(b, o)?;
            b.place(p, "rustcraft:vein", &[])?;
            b.set_owner(p, OWNER, 1)?;
            b.set_buffer(p, 4000.0)?;
            b.put(p, 0, "minecraft:cobblestone", 4)
        })
        .until("an ore grew beside it", 1400, move |b, _| {
            Ok(crate::utility::vein::cells()
                .into_iter()
                .any(|c| b.block(at(p, c)).ends_with("_ore") || b.block(at(p, c)) == "minecraft:ancient_debris"))
        })
}
