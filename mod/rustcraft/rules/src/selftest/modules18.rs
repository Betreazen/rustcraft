//! Slice 18, the module stream: module effects as the guide gives them.
//!
//! - The disintegrator's share rises with yield modules, not output modules (DS5, guide `mod_yield`):
//!   two yield modules give 2 of a bucket's 3 ingots (70%), where it gives 1 bare.
//! - Economy slows a slice-12 machine's step by ×0.85 (guide `mod_eco`): the trash's window shows a
//!   step of 1.2 s instead of 1.0 s.
//! - The vein's ore choice appears in its window with the ore choice module only (guide `mod_pick`).
//! - With the stratum module the vein's window offers sand or gravel, and the vein grows from the
//!   chosen one: gravel chosen, sand and gravel given, suspicious gravel grows (guide `mod_stratum`).

use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::modules::{self, Module};
use crate::tuning::Tuning;
use crate::utility::{self, Util};

pub const ALL: [Build; 4] = [disintegrator_yield, economy_slows_a_step, vein_choice_with_the_module, stratum_grows_the_chosen_rock];

const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a18;
const LIMIT: u64 = 400;

fn floor(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=8 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
        }
    }
    Ok(())
}

fn machine(b: &mut dyn Bench, pos: Pos, block: &str) -> Result<(), String> {
    b.place(pos, block, &[])?;
    b.set_owner(pos, OWNER, 1)?;
    b.set_buffer(pos, 1000.0)
}

fn disintegrator_yield(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    let percent = utility::disassembly::share_percent(2);
    // A bucket is 3 iron ingots; whole ingots come out, the rest waits inside.
    let wanted = 3 * percent / 100;
    Scenario::new("module_disintegrator_yield", plot_area(o))
        .then("a disintegrator with two yield modules and a bucket", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:disintegrator")?;
            b.set_modules(p, &[Module::Yield.item(), Module::Yield.item()])?;
            b.put(p, 0, "minecraft:bucket", 1)
        })
        .until("its output holds the ingots of 70%", LIMIT, move |b, _| {
            Ok(count_of(&b.items(p).into_iter().skip(1).collect::<Vec<_>>(), "minecraft:iron_ingot") >= wanted)
        })
        .check("exactly 2 ingots, where a bare one gives 1", move |b, _| {
            same(
                "ingots",
                count_of(&b.items(p).into_iter().skip(1).collect::<Vec<_>>(), "minecraft:iron_ingot"),
                wanted,
            )
        })
}

/// The `A step every %s s` row of a slice-12 window names `seconds`.
fn step_shown(b: &dyn Bench, pos: Pos, seconds: &str) -> bool {
    b.sheet(pos).is_some_and(|s| {
        s.rows
            .iter()
            .any(|r| r.to_string().contains("rustcraft.util.step") && r.to_string().contains(&format!("\"{seconds}\"")))
    })
}

fn economy_slows_a_step(o: Pos, t: &Tuning) -> Scenario {
    let bare = at(o, [2, 1, 0]);
    let eco = at(o, [5, 1, 0]);
    let step = utility::step_ticks(Util::Trash);
    let slow = modules::paced_ticks(step, &[Some(Module::Economy)], t);
    let (bare_s, eco_s) = (format!("{:.1}", step as f64 / 20.0), format!("{:.1}", slow as f64 / 20.0));
    Scenario::new("module_economy_slows_a_step", plot_area(o))
        .then("two trashes, one with an economy module", move |b, _| {
            floor(b, o)?;
            machine(b, bare, "rustcraft:trash")?;
            machine(b, eco, "rustcraft:trash")?;
            b.set_modules(eco, &[Module::Economy.item()])
        })
        .until("the bare one steps every 1.0 s, the economy one every 1.2 s", 60, move |b, _| {
            Ok(step_shown(b, bare, &bare_s) && step_shown(b, eco, &eco_s))
        })
}

fn vein_choice_with_the_module(o: Pos, _: &Tuning) -> Scenario {
    let bare = at(o, [1, 1, 0]);
    let pick = at(o, [6, 1, 0]);
    let choices = crate::utility::vein::PICKS.len();
    Scenario::new("module_vein_choice_with_the_module", plot_area(o))
        .then("two veins, one with the ore choice module", move |b, _| {
            floor(b, o)?;
            machine(b, bare, "rustcraft:vein")?;
            machine(b, pick, "rustcraft:vein")?;
            b.set_modules(pick, &[Module::Pick.item()])
        })
        .until("only the one with the module offers the ores", 60, move |b, _| {
            let toggles = |p| b.sheet(p).map(|s| s.toggles.len());
            Ok(toggles(bare) == Some(0) && toggles(pick) == Some(choices))
        })
}

fn stratum_grows_the_chosen_rock(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 2, 0]);
    let grew = move |b: &dyn Bench, block: &str| crate::utility::vein::cells().into_iter().any(|c| b.block(at(p, c)) == block);
    Scenario::new("module_stratum_grows_the_chosen_rock", plot_area(o))
        .then("a vein with the stratum module, sand and gravel, gravel chosen", move |b, _| {
            floor(b, o)?;
            machine(b, p, "rustcraft:vein")?;
            b.set_buffer(p, 8000.0)?;
            b.set_modules(p, &[Module::Stratum.item()])?;
            b.put(p, 0, "minecraft:sand", 8)?;
            b.put(p, 1, "minecraft:gravel", 8)?;
            b.press_cursor(p, crate::machines::sheet::Press::Toggle(1).id(), None).map(|_| ())
        })
        .check("its window offers sand and gravel", move |b, _| {
            same("choices", b.sheet(p).map(|s| s.toggles.len()), Some(crate::utility::vein::STRATUM_ROCKS.len()))
        })
        .until("suspicious gravel grew beside it", 2700, move |b, _| Ok(grew(b, "minecraft:suspicious_gravel")))
        .check("and no suspicious sand", move |b, _| same("suspicious sand", grew(b, "minecraft:suspicious_sand"), false))
}
