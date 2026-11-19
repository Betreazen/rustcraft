//! 0.21 (spec 46, LP1–LP7; the owner 2026-11-19: «забойщик не собирает предметы … турели …
//! предметы тоже засасываются и в магистрали со складом туда отправляются»).
//!
//! - A butcher on an item line with a storage, its joint as placed: eggs of two kinds and feathers
//!   dropped in its pen with no kill of its own end in the storage (LP1, LP4); someone's diamond
//!   lying beside them stays where it lies (LP2).
//! - A butcher whose nine slots are full leaves the eggs on the ground and says «Добыче некуда»
//!   (LP3).
//! - A turret on the same kind of line kills an evoker: the evoker's totem ends in the storage
//!   (LP5, LP7); a diamond lying four blocks from where it died stays.

use super::transport::item_line;
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::butcher::{self, Shown as ButcherShown};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 3] = [
    butcher_takes_eggs_to_the_storage,
    butcher_with_full_slots_says_so,
    turret_kill_drops_reach_the_storage,
];

const CELL: &str = "rustcraft:cell";
const TERMINAL: &str = "rustcraft:terminal";
const TURRET: &str = "rustcraft:turret";
const STONE: &str = "minecraft:stone";
const EGG: &str = "minecraft:egg";
const BLUE_EGG: &str = "minecraft:blue_egg";
const FEATHER: &str = "minecraft:feather";
const DIAMOND: &str = "minecraft:diamond";
/// A hostile mob whose table always drops one thing: its totem.
const EVOKER: &str = "minecraft:evoker";
const TOTEM: &str = "minecraft:totem_of_undying";
const OWNER: u128 = super::workers::OWNER;
const LIMIT: u64 = 400;

/// The cells: a 2×1×2 box on the z+ side of housings 3 and 4, as the storage's scenarios have it.
fn cells(o: Pos) -> [Pos; 4] {
    [at(o, [3, 0, 1]), at(o, [4, 0, 1]), at(o, [3, 0, 2]), at(o, [4, 0, 2])]
}

fn terminal(o: Pos) -> Pos {
    at(o, [5, 0, 1])
}

/// The machine's place: on the z− side of housing 1, on the floor.
fn machine(o: Pos) -> Pos {
    at(o, [1, 0, -1])
}

/// An item line of tier 2 from `o` (a node, housings 1–4), a storage of four cells with a
/// terminal on housings 3 and 4, and a stone floor one below the line on its z− side.
fn storage_line(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=11 {
        for z in -3..=-1 {
            b.place(at(o, [x, -1, z]), STONE, &[])?;
        }
    }
    item_line(b, o, 2, 4, &[])?;
    for c in cells(o) {
        b.place(c, CELL, &[])?;
    }
    b.place(terminal(o), TERMINAL, &[])
}

fn stored(b: &dyn Bench, o: Pos, item: &str) -> Result<u64, String> {
    Ok(b
        .storage(terminal(o))
        .ok_or("the terminal shows no storage")?
        .store
        .count(item))
}

/// Where an item dropped at block `p`, `dx` and `dz` off, lands: the middle of the block above
/// the floor.
fn spot(p: Pos, dx: i32, dz: i32) -> [f64; 3] {
    [f64::from(p[0] + dx) + 0.5, f64::from(p[1]) + 0.2, f64::from(p[2] + dz) + 0.5]
}

fn lying_count(b: &dyn Bench, near: [f64; 3], item: &str) -> u32 {
    let lo = [near[0] - 1.5, near[1] - 1.5, near[2] - 1.5];
    let hi = [near[0] + 1.5, near[1] + 1.5, near[2] + 1.5];
    b.lying(lo, hi).iter().filter(|(i, _)| i == item).map(|(_, n)| n).sum()
}

fn butcher_takes_eggs_to_the_storage(o: Pos, _: &Tuning) -> Scenario {
    let p = machine(o);
    let eggs = spot(p, 2, -1);
    let things = spot(p, 0, -2);
    Scenario::new("loot21_butcher_takes_eggs_to_the_storage", plot_area(o))
        .then("a storage line and a butcher on housing 1, its joint as placed", move |b, _| {
            storage_line(b, o)?;
            b.place(p, butcher::BLOCK, &[])?;
            b.set_owner(p, OWNER, 1)
        })
        .then("three eggs, two blue eggs, four feathers and someone's diamond in its pen", move |b, _| {
            b.drop_item(eggs, EGG, 3)?;
            b.drop_item(eggs, BLUE_EGG, 2)?;
            b.drop_item(eggs, FEATHER, 4)?;
            b.drop_item(things, DIAMOND, 1)
        })
        .until("the eggs and the feathers are in the storage", LIMIT, move |b, _| {
            Ok(stored(b, o, EGG)? == 3 && stored(b, o, BLUE_EGG)? == 2 && stored(b, o, FEATHER)? == 4)
        })
        .check("the diamond still lies in its pen, not in its slots", move |b, _| {
            same("diamonds lying", lying_count(b, things, DIAMOND), 1)?;
            same("diamonds in its slots", count_of(&b.items(p), DIAMOND), 0)?;
            same("diamonds stored", stored(b, o, DIAMOND)?, 0)
        })
}

fn butcher_with_full_slots_says_so(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    let eggs = spot(p, 0, -2);
    Scenario::new("loot21_butcher_with_full_slots_says_so", plot_area(o))
        .then("a butcher on a floor, its nine slots full of stone", move |b, _| {
            for x in -1..=8 {
                for z in -3..=3 {
                    b.place(at(o, [x, 0, z]), STONE, &[])?;
                }
            }
            b.place(p, butcher::BLOCK, &[])?;
            b.set_owner(p, OWNER, 1)?;
            for slot in 0..butcher::LOOT_SLOTS {
                b.put(p, slot, STONE, 64)?;
            }
            Ok(())
        })
        .then("three eggs in its pen", move |b, _| b.drop_item(eggs, EGG, 3))
        .until("it says the drops have nowhere to go", 100, move |b, _| {
            Ok(b.butcher(p) == Some(ButcherShown::OutputFull))
        })
        .check("the eggs still lie on the ground", move |b, _| {
            same("eggs lying", lying_count(b, eggs, EGG), 3)
        })
}

fn turret_kill_drops_reach_the_storage(o: Pos, t: &Tuning) -> Scenario {
    let turret = machine(o);
    let full = t.turret_buffer;
    let died = spot(turret, 5, -1);
    let far = spot(turret, 9, -1);
    Scenario::new("loot21_turret_kill_drops_reach_the_storage", plot_area(o))
        .then("a storage line and a turret on housing 1 with a full buffer", move |b, _| {
            storage_line(b, o)?;
            b.place(turret, TURRET, &[])?;
            b.set_owner(turret, OWNER, 1)?;
            b.set_buffer(turret, full)
        })
        .then("someone's diamond four blocks off, then a weak evoker", move |b, m| {
            b.drop_item(far, DIAMOND, 1)?;
            let feet = [died[0], died[1] - 0.2, died[2]];
            let id = b.spawn_mob(EVOKER, feet, Some(2.0))?;
            m.mark("evoker", f64::from(id));
            Ok(())
        })
        .until("the turret kills it", 200, move |b, m| Ok(b.mob(m.get("evoker") as u32).is_none()))
        .until("its totem is in the storage", LIMIT, move |b, _| Ok(stored(b, o, TOTEM)? == 1))
        .check("nothing of it lies where it died, the diamond lies where it was", move |b, _| {
            same("totems lying", lying_count(b, died, TOTEM), 0)?;
            same("totems in the turret's slots", count_of(&b.items(turret), TOTEM), 0)?;
            same("diamonds lying", lying_count(b, far, DIAMOND), 1)?;
            same("diamonds stored", stored(b, o, DIAMOND)?, 0)
        })
}
