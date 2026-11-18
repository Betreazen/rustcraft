//! Slice 11: the catalysts, the butcher and the guard.
//!
//! - The mixer makes the accelerator out of water, sugar and glowstone dust (K1).
//! - A furnace takes a catalyst from a tank it touches; with the accelerator it smelts half again
//!   as fast as one without (K1) and spends it while it works; it holds one catalyst at a time
//!   (K2).
//! - The butcher strikes adult cows over its number to keep (2) with the sword in its last slot,
//!   picks the drops up into its slots and tells Path-of-craft-tree `slaughter`, never `kill`
//!   (round 26). Without a sword it says so. Its window has a toggle an animal, and a press keeps
//!   one more.
//! - A guard (a worker with the shooter) shoots a hostile mob by its rails; the kill goes to
//!   Path-of-craft-tree as `kill` and to SoulsCraft as `guard` (spec 27.1).

use super::{Bench, Build, Scenario, at, base, count_of, plot_area, same};
use crate::butcher::{self as butcher, ANIMALS, Shown as ButcherShown};
use crate::geom::{Face, Pos};
use crate::machines::catalyst::{ACCELERATOR, ECONOMIZER};
use crate::machines::sheet::Press;
use crate::monorail::shape;
use crate::tuning::Tuning;
use crate::worker::Tool;

pub const ALL: [Build; 8] = [
    mixer_makes_accelerator,
    furnace_takes_a_catalyst,
    accelerator_is_faster,
    one_catalyst_at_a_time,
    butcher_culls_over_the_keep,
    butcher_needs_a_sword,
    butcher_window,
    guard_shoots_a_mob,
];

const MIXER: &str = "rustcraft:mixer";
const FURNACE: &str = "rustcraft:furnace";
const TANK: &str = "rustcraft:tank";
const CRYSTAL: &str = "rustcraft:crystal";
const MONORAIL: &str = "rustcraft:monorail";
const STONE: &str = "minecraft:stone";
const WATER: &str = "minecraft:water";
const RAW_IRON: &str = "minecraft:raw_iron";
const IRON: &str = "minecraft:iron_ingot";
const SWORD: &str = "minecraft:iron_sword";
const COW: &str = "minecraft:cow";
const SPIDER: &str = "minecraft:spider";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a11;
const LIMIT: u64 = 400;

/// A floor of stone, x -1..=8 and z -3..=3 from `o`.
fn floor(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=8 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    Ok(())
}

/// A furnace at `pos` between two crystals (8 E/s, its full draw) with `n` raw iron.
fn furnace(b: &mut dyn Bench, pos: Pos, n: u32) -> Result<(), String> {
    b.place(pos, FURNACE, &[])?;
    b.place(at(pos, [0, 0, -1]), CRYSTAL, &[])?;
    b.place(at(pos, [0, 0, 1]), CRYSTAL, &[])?;
    b.put(pos, 0, RAW_IRON, n)
}

fn output(b: &dyn Bench, pos: Pos) -> u32 {
    count_of(&b.items(pos), IRON)
}

fn catalyst_mb(b: &dyn Bench, pos: Pos, fluid: &str) -> f64 {
    b.catalyst(pos)
        .filter(|c| c.fluid.as_deref() == Some(fluid))
        .map_or(0.0, |c| c.amount)
}

fn tank_mb(b: &dyn Bench, pos: Pos) -> u64 {
    b.tank(pos).map_or(0, |g| g.amount)
}

fn mixer_makes_accelerator(o: Pos, _: &Tuning) -> Scenario {
    let m = at(o, [3, 1, 3]);
    let water = at(o, [2, 1, 3]);
    let product = at(o, [3, 2, 3]);
    Scenario::new("catalyst_mixer_makes_accelerator", plot_area(o))
        .then(
            "a mixer with sugar and glowstone dust, a water tank beside it, a tank on top",
            move |b, _| {
                b.place(m, MIXER, &[])?;
                b.put(m, 0, "minecraft:sugar", 1)?;
                b.put(m, 1, "minecraft:glowstone_dust", 1)?;
                b.place(water, TANK, &[])?;
                b.set_tank(water, WATER, 4000)?;
                b.place(product, TANK, &[])?;
                b.place(at(m, [0, 0, -1]), CRYSTAL, &[])?;
                b.place(at(m, [0, 0, 1]), CRYSTAL, &[])
            },
        )
        .until("a bucket of accelerator in the tank on top", LIMIT, move |b, _| {
            Ok(b.tank(product).is_some_and(|g| {
                g.fluid.as_deref() == Some(ACCELERATOR) && g.amount >= 1000
            }))
        })
}

fn furnace_takes_a_catalyst(o: Pos, _: &Tuning) -> Scenario {
    let f = at(o, [3, 1, 0]);
    let tank = at(o, [2, 1, 0]);
    Scenario::new("catalyst_furnace_takes_and_spends_it", plot_area(o))
        .then(
            "a furnace with raw iron, a tank of accelerator beside it",
            move |b, _| {
                floor(b, o)?;
                furnace(b, f, 16)?;
                b.place(tank, TANK, &[])?;
                b.set_tank(tank, ACCELERATOR, 2000)
            },
        )
        .until("the furnace holds a bucket of accelerator", 60, move |b, _| {
            Ok(catalyst_mb(b, f, ACCELERATOR) >= 999.0)
        })
        .until("it smelted two ingots", LIMIT, move |b, _| Ok(output(b, f) >= 2))
        .check("it spent some of it while it worked", move |b, _| {
            let total = catalyst_mb(b, f, ACCELERATOR) + tank_mb(b, tank) as f64;
            if total < 2000.0 - 1.0 {
                Ok(())
            } else {
                Err(format!("still {total} mB of accelerator after the work"))
            }
        })
}

fn accelerator_is_faster(o: Pos, _: &Tuning) -> Scenario {
    let fast = at(o, [1, 1, 0]);
    let plain = at(o, [6, 1, 0]);
    let tank = at(o, [0, 1, 0]);
    Scenario::new("catalyst_accelerator_smelts_faster", plot_area(o))
        .then(
            "two furnaces with raw iron, one beside a tank of accelerator",
            move |b, _| {
                floor(b, o)?;
                b.place(tank, TANK, &[])?;
                b.set_tank(tank, ACCELERATOR, 4000)?;
                furnace(b, fast, 16)?;
                furnace(b, plain, 16)
            },
        )
        .until("the plain furnace smelted three ingots", 500, move |b, _| {
            Ok(output(b, plain) >= 3)
        })
        .check("the one with the accelerator smelted at least four", move |b, _| {
            let n = output(b, fast);
            if n >= 4 {
                Ok(())
            } else {
                Err(format!("{n} ingots with the accelerator, 3 without"))
            }
        })
}

fn one_catalyst_at_a_time(o: Pos, _: &Tuning) -> Scenario {
    let f = at(o, [3, 1, 0]);
    let west = at(o, [2, 1, 0]);
    let east = at(o, [4, 1, 0]);
    Scenario::new("catalyst_one_at_a_time", plot_area(o))
        .then(
            "a furnace between a tank of accelerator and a tank of economizer",
            move |b, _| {
                floor(b, o)?;
                b.place(f, FURNACE, &[])?;
                b.place(west, TANK, &[])?;
                b.set_tank(west, ACCELERATOR, 3000)?;
                b.place(east, TANK, &[])?;
                b.set_tank(east, ECONOMIZER, 3000)
            },
        )
        .until("the furnace holds a catalyst", 60, move |b, _| {
            Ok(b.catalyst(f).is_some_and(|c| c.active().is_some()))
        })
        .wait(60)
        .check("one tank gave a bucket, the other nothing (K2)", move |b, _| {
            let (w, e) = (tank_mb(b, west), tank_mb(b, east));
            let mut given = [3000 - w, 3000 - e];
            given.sort_unstable();
            same("buckets the two tanks gave, mB", given, [0, 1000])
        })
}

/// A butcher at `pos`, owned by OWNER, with a sword if `sword`.
fn butcher_at(b: &mut dyn Bench, pos: Pos, sword: bool) -> Result<(), String> {
    b.place(pos, butcher::BLOCK, &[])?;
    b.set_owner(pos, OWNER, 1)?;
    // Slice 18: it strikes on its buffer's energy, 4 E/s.
    b.set_buffer(pos, butcher::BUFFER)?;
    if sword {
        b.put(pos, butcher::SWORD_SLOT, SWORD, 1)?;
    }
    Ok(())
}

/// `n` cows on the floor round `pos`, their ids marked `cow0`…
fn cows(b: &mut dyn Bench, m: &mut super::Memo, pos: Pos, n: usize) -> Result<(), String> {
    const KEYS: [&str; 4] = ["cow0", "cow1", "cow2", "cow3"];
    for (i, key) in KEYS.iter().enumerate().take(n) {
        let x = f64::from(pos[0]) - 1.5 + i as f64;
        let id = b.spawn_mob(COW, [x, f64::from(pos[1]), f64::from(pos[2]) + 2.5], None)?;
        m.mark(key, f64::from(id));
    }
    Ok(())
}

fn alive(b: &dyn Bench, m: &super::Memo, n: usize) -> Vec<u32> {
    ["cow0", "cow1", "cow2", "cow3"]
        .iter()
        .take(n)
        .map(|k| m.get(k) as u32)
        .filter(|&id| b.mob(id).is_some())
        .collect()
}

fn butcher_culls_over_the_keep(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("butcher_culls_over_the_keep", plot_area(o))
        .then("a butcher with a sword and four cows by it", move |b, m| {
            floor(b, o)?;
            butcher_at(b, p, true)?;
            cows(b, m, p, 4)
        })
        .until("two cows are left (it keeps 2)", 700, move |b, m| {
            Ok(alive(b, m, 4).len() == 2)
        })
        .wait(240)
        .check("two cows stay: it keeps the herd", move |b, m| {
            same("cows alive", alive(b, m, 4).len(), 2)
        })
        .until("the drops are in its slots", 60, move |b, _| {
            Ok(count_of(&b.items(p), "minecraft:beef") > 0)
        })
        .check("each kill went as slaughter, none as kill", move |b, m| {
            let living = alive(b, m, 4);
            let lines = b.reports();
            for key in ["cow0", "cow1", "cow2", "cow3"] {
                let id = m.get(key) as u32;
                if living.contains(&id) {
                    continue;
                }
                let slaughter = format!("slaughter {OWNER:032x} {id} {COW}");
                if !lines.iter().any(|l| l.contains(&slaughter)) {
                    return Err(format!("no «{slaughter}» in {lines:?}"));
                }
                let kill = format!("kill {OWNER:032x} {id} ");
                if lines.iter().any(|l| l.contains(&kill)) {
                    return Err(format!("a butcher's cow went as «kill»: {kill}"));
                }
            }
            Ok(())
        })
        .check("the sword is still in its slot", move |b, _| {
            same("swords", count_of(&b.items(p), SWORD), 1)
        })
}

fn butcher_needs_a_sword(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("butcher_needs_a_sword", plot_area(o))
        .then("a butcher without a sword and three cows", move |b, m| {
            floor(b, o)?;
            butcher_at(b, p, false)?;
            cows(b, m, p, 3)
        })
        .until("it says «Нет меча»", 300, move |b, _| {
            Ok(b.butcher(p) == Some(ButcherShown::NoSword))
        })
        .check("all three cows live", move |b, m| {
            same("cows alive", alive(b, m, 3).len(), 3)
        })
}

fn butcher_window(o: Pos, t: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    let least = t.butcher_keep_min;
    Scenario::new("butcher_window", plot_area(o))
        .then("a butcher", move |b, _| {
            floor(b, o)?;
            butcher_at(b, p, true)
        })
        .until("its window has a toggle an animal", 60, move |b, _| {
            Ok(b.sheet(p).is_some_and(|s| s.toggles.len() == ANIMALS.len()))
        })
        .then("a press on the cow's toggle", move |b, _| {
            b.press_cursor(p, Press::Toggle(0).id(), None).map(|_| ())
        })
        .until("the cow's toggle says it keeps one more", 60, move |b, _| {
            let Some(s) = b.sheet(p) else {
                return Ok(false);
            };
            // «%s: %s adults · keeps %s»: the number kept is its third argument, which the game
            // sends as a JSON number (Text::argument), not a string.
            let kept = s
                .toggles
                .first()
                .and_then(|t| t.name.get("with")?.get(2)?.as_u64());
            Ok(kept == Some(u64::from(least + 1)))
        })
}

fn guard_shoots_a_mob(o: Pos, _: &Tuning) -> Scenario {
    let spider_at = [
        f64::from(o[0]) + 3.5,
        f64::from(o[1] + 1),
        f64::from(o[2]) + 3.5,
    ];
    Scenario::new("guard_shoots_a_mob", plot_area(o))
        .then("a straight track and a weak spider beside it", move |b, m| {
            floor(b, o)?;
            for x in 0..=6 {
                b.place(
                    at(o, [x, 1, 0]),
                    MONORAIL,
                    &[(shape::MOUNT, shape::mount_value(Face::YNeg))],
                )?;
            }
            base(b, Some(at(o, [0, 1, 0])), Some(at(o, [6, 1, 0])))?;
            let id = b.spawn_mob(SPIDER, spider_at, Some(2.0))?;
            m.mark("mob", f64::from(id));
            Ok(())
        })
        .then("a guard on the track", move |b, m| {
            let id = b.spawn_worker(at(o, [1, 1, 0]), OWNER, Some(Tool::Shooter), &[], None)?;
            m.mark("worker", id as f64);
            Ok(())
        })
        .until("the spider dies", 200, |b, m| Ok(b.mob(m.get("mob") as u32).is_none()))
        .until("its kill goes to Path-of-craft-tree", 40, |b, m| {
            let line = format!("kill {OWNER:032x} {} {SPIDER}", m.get("mob") as u32);
            Ok(b.reports().iter().any(|l| l.contains(&line)))
        })
        .until("and to SoulsCraft as the guard's", 40, |b, m| {
            let line = format!(
                "soulscraft-kill guard {OWNER:032x} {} {SPIDER} ",
                m.get("mob") as u32
            );
            Ok(b.reports().iter().any(|l| l.contains(&line)))
        })
}
