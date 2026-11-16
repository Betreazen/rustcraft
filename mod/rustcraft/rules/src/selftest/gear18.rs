//! Slice 18, base and gear (stream 7): the gear as the handbook says.
//!
//! - The suit capacity module lets a charger fill its chestplate half again (G6).
//! - The charger leaves a zipline's charges alone: they come back from string only.
//! - A turret turns its head to the mob it shoots (the aim clients draw it by).
//! - Armour of the next tier is made from the piece below and the new alloy, keeping the piece's
//!   number: its charge, modules and enchantments.
//! - A zipline rope comes down by itself five minutes after it was tied, both anchors with it.
//! - The anvil puts technical books on a piece: I, then II, then III, and no further.

use super::{Bench, Build, Scenario, at, plot_area};
use crate::gear::{Held, Module};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 6] = [
    charger_fills_the_suit_capacity,
    charger_skips_the_zipline,
    turret_aims_at_its_target,
    armour_upgrade_keeps_the_piece,
    rope_comes_down_in_time,
    anvil_puts_the_books_on,
];

const STONE: &str = "minecraft:stone";
const CHARGER: &str = "rustcraft:charger";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a18;

fn floor(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=8 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    Ok(())
}

fn charger(b: &mut dyn Bench, pos: Pos) -> Result<(), String> {
    b.place(pos, CHARGER, &[])?;
    b.set_owner(pos, OWNER, 1)?;
    b.set_buffer(pos, 1000.0)
}

fn charger_fills_the_suit_capacity(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("gear_charger_fills_the_suit_capacity", plot_area(o))
        .then("a charger with a crystallite chestplate holding the suit capacity module, at its plain battery", move |b, _| {
            floor(b, o)?;
            charger(b, p)?;
            let held = Held { charge: 19_990.0, modules: vec![Module::Capacity.item().to_owned()], ..Held::default() };
            b.put_gear_held(p, 0, "rustcraft:tech_chestplate_3", held)
        })
        .until("it charges past 20 000 E", 300, move |b, _| {
            Ok(b.gear_held(p, 0).or_else(|| b.gear_held(p, 4)).is_some_and(|h| h.charge > 20_050.0))
        })
}

fn charger_skips_the_zipline(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("gear_charger_skips_the_zipline", plot_area(o))
        .then("a charger with a zipline holding 3 charges", move |b, _| {
            floor(b, o)?;
            charger(b, p)?;
            b.put_gear_held(p, 0, "rustcraft:zipline", Held { charge: 3.0, ..Held::default() })
        })
        .wait(60)
        .check("it still holds 3 charges in its slot", move |b, _| match b.gear_held(p, 0) {
            Some(h) if (h.charge - 3.0).abs() < 1e-9 => Ok(()),
            other => Err(format!("the zipline holds {other:?}")),
        })
}

fn turret_aims_at_its_target(o: Pos, _: &Tuning) -> Scenario {
    let turret = at(o, [10, 1, 0]);
    Scenario::new("turret_aims_at_its_target", plot_area(o))
        .then("a floor, a test-kit turret and a spider beside it", move |b, m| {
            for x in 4..18 {
                for z in -3..=3 {
                    b.place(at(o, [x, 0, z]), STONE, &[])?;
                }
            }
            let answer = b.testkit(&format!("turret {} {} {} modules=poison owner={OWNER:032x}", turret[0], turret[1], turret[2]));
            if answer != "ran true" {
                return Err(answer);
            }
            let feet = [f64::from(turret[0]) + 0.5, f64::from(turret[1]), f64::from(turret[2]) + 4.5];
            let id = b.spawn_mob("minecraft:spider", feet, None)?;
            m.mark("mob", f64::from(id));
            Ok(())
        })
        .until("its head points at the spider", 200, move |b, m| {
            let Some(mob) = b.mob(m.get("mob") as u32) else {
                return Ok(false);
            };
            let Some((yaw, _)) = b.turret_aim(turret) else {
                return Ok(false);
            };
            let head = [f64::from(turret[0]) + 0.5, f64::from(turret[1]) + 0.6, f64::from(turret[2]) + 0.5];
            let (want, _) = crate::aim::aim(head, mob.pos);
            let off = (yaw - want + 540.0).rem_euclid(360.0) - 180.0;
            Ok(off.abs() < 30.0)
        })
}

fn armour_upgrade_keeps_the_piece(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("gear_armour_upgrade_keeps_the_piece", plot_area(o)).check(
        "a piece and its alloy make the next tier with the same charge",
        |b, _| {
            for (piece, alloys) in [("helmet", 5), ("chestplate", 7), ("leggings", 7), ("boots", 4)] {
                for (tier, alloy) in [(2, "rustcraft:crystallite"), (3, "rustcraft:starsteel")] {
                    let from = format!("rustcraft:tech_{piece}_{tier}");
                    let to = format!("rustcraft:tech_{piece}_{}", tier + 1);
                    let mut grid: [Option<&str>; 9] = [None; 9];
                    grid[0] = Some(from.as_str());
                    for cell in grid.iter_mut().skip(1).take(alloys) {
                        *cell = Some(alloy);
                    }
                    let (made, charge) = b.craft_gear(&grid, 777.0).ok_or(format!("{from} and {alloys} {alloy} make nothing"))?;
                    if made != to || charge != Some(777.0) {
                        return Err(format!("{from} made {made} with charge {charge:?}, not {to} with 777"));
                    }
                }
            }
            Ok(())
        },
    )
}

fn rope_comes_down_in_time(o: Pos, _: &Tuning) -> Scenario {
    let (a, far) = (at(o, [1, 1, 0]), at(o, [10, 3, 0]));
    Scenario::new("zipline_rope_comes_down_in_time", plot_area(o))
        .then("a floor, a pillar and a rope tied five minutes ago", move |b, _| {
            for x in 0..=12 {
                for z in -2..=2 {
                    b.place(at(o, [x, 0, z]), STONE, &[])?;
                }
            }
            for y in 1..=2 {
                b.place(at(o, [10, y, 0]), STONE, &[])?;
            }
            let id = b.tie_rope(OWNER, a, far)?;
            b.age_rope(id, crate::traversal::ROPE_LIFE_MS)
        })
        .until("the rope is untied and both anchors are gone", 100, move |b, _| {
            let saved = b.ropes().iter().any(|r| r.a == a && r.b == far);
            Ok(!saved && b.block(a) == "minecraft:air" && b.block(far) == "minecraft:air")
        })
}

fn anvil_puts_the_books_on(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("gear_anvil_puts_the_books_on", plot_area(o)).check("Capacity I, II, III on a blade, a fourth book does nothing", |b, _| {
        let got = b.anvil_capacity("rustcraft:blade_3", 1, 4);
        if got == [Some(1), Some(2), Some(3), None] {
            Ok(())
        } else {
            Err(format!("the anvil gave {got:?}"))
        }
    })
}
