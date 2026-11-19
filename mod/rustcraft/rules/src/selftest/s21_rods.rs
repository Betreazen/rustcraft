//! 0.21 (spec 38): fishing stations take their rods from the storage and order new ones.
//!
//! The owner: «рыболовные станции сами должны отправлять заказы на новые удочки… научи
//! рыболовные станции забирать их из склада». Each scenario has a storage on a tier 3 line, the
//! line running on over a pond with two stations side by side beside it.
//!
//! - Two stations without rods each take a rod from the storage and fish (FR1).
//! - A station's worn rod goes into the storage for a whole one, and it fishes (FR1, FR2).
//! - With sticks and string but no rods in the storage, an autocrafter and the recipe book on the
//!   line, the storage orders 2 rods once for both stations (FR3, FR4), and both fish with them.

use super::craft18::status_key;
use super::magistral::place_housing;
use super::slice15::{CRAFTER, FILL, build, crafter_at, held, holds, machine, put_book, terminal_at};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::storage::keys;
use crate::tuning::Tuning;

pub const ALL: [Build; 3] = [
    stations_take_rods_from_the_storage,
    a_worn_rod_is_swapped_for_a_whole_one,
    the_storage_orders_rods_for_its_stations,
];

const ROD: &str = "minecraft:fishing_rod";
const STICK: &str = "minecraft:stick";
const STRING: &str = "minecraft:string";
const WORKING: &str = "rustcraft.util.working";
/// A rod of 64 points one point from breaking.
const WORN: &str = r#"{id:"minecraft:fishing_rod",count:1,components:{"minecraft:damage":63}}"#;

/// The line's origin: three blocks up, so the pond fits under it.
fn line_at(o: Pos) -> Pos {
    at(o, [0, 3, 0])
}

/// The two stations, beside housings 10 and 11 of the line.
fn stations(o: Pos) -> [Pos; 2] {
    let l = line_at(o);
    [at(l, [10, 0, 1]), at(l, [11, 0, 1])]
}

/// The storage's line (slice 15's: a tier 3 node, a chest of `parts`, four cells and a terminal)
/// running on to housing 11, and a pond 8 by 7, 2 deep, under its end.
fn line_and_pond(b: &mut dyn Bench, o: Pos, parts: &[(&str, u32)]) -> Result<(), String> {
    let l = line_at(o);
    build(b, l, parts)?;
    for x in 6..=11 {
        place_housing(b, at(l, [x, 0, 0]), 2)?;
    }
    for x in 6..=15 {
        for z in -3..=5 {
            b.place(at(l, [x, -3, z]), "minecraft:stone", &[])?;
            for y in -2..=-1 {
                let inside = (7..=14).contains(&x) && (-2..=4).contains(&z);
                b.place(at(l, [x, y, z]), if inside { "minecraft:water" } else { "minecraft:stone" }, &[])?;
            }
        }
    }
    Ok(())
}

/// The two stations with energy and nothing in their rod cells.
fn place_stations(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for p in stations(o) {
        b.place(p, "rustcraft:fishery", &[])?;
        b.set_buffer(p, 200.0)?;
    }
    Ok(())
}

fn all_fish(b: &dyn Bench, o: Pos) -> bool {
    stations(o).iter().all(|p| status_key(b, *p) == WORKING)
}

/// The worn rods the storage holds: rods under a key with their damage.
fn worn_in_storage(b: &dyn Bench, o: Pos) -> Result<u64, String> {
    let v = b.storage(terminal_at(line_at(o))).ok_or("the terminal shows no storage")?;
    Ok(v.store.items.iter().filter(|(k, _)| keys::item_of(k) == ROD && k.as_str() != ROD).map(|(_, n)| *n).sum())
}

fn stations_take_rods_from_the_storage(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("rods21_stations_take_rods_from_the_storage", plot_area(o))
        .then("a storage on a tier 3 line, a chest of 3 rods, a pond at the line's end", move |b, _| {
            line_and_pond(b, o, &[(ROD, 3)])
        })
        .until("the storage holds the rods", 2 * FILL, move |b, _| holds(b, line_at(o), &[(ROD, 3)]))
        .then("two stations over the pond beside the line, no rods", move |b, _| place_stations(b, o))
        .until("both take a rod from the storage and fish", 200, move |b, _| Ok(all_fish(b, o)))
        .check("each holds one rod, the storage the third", move |b, _| {
            for p in stations(o) {
                same(&format!("rods in the station at {p:?}"), count_of(&b.items(p), ROD), 1)?;
            }
            same("rods in the storage", held(b, line_at(o), ROD)?, 1)
        })
}

fn a_worn_rod_is_swapped_for_a_whole_one(o: Pos, _: &Tuning) -> Scenario {
    let station = stations(o)[0];
    Scenario::new("rods21_a_worn_rod_is_swapped_for_a_whole_one", plot_area(o))
        .then("a storage on a tier 3 line, a chest of 2 rods, a pond at the line's end", move |b, _| {
            line_and_pond(b, o, &[(ROD, 2)])
        })
        .until("the storage holds the rods", 2 * FILL, move |b, _| holds(b, line_at(o), &[(ROD, 2)]))
        .then("a station over the pond with a rod one catch from breaking", move |b, _| {
            b.place(station, "rustcraft:fishery", &[])?;
            b.set_buffer(station, 200.0)?;
            same("worn rod in", b.insert_snbt(station, WORN)?, 1)
        })
        .until("it swaps the rod and fishes", 200, move |b, _| Ok(status_key(b, station) == WORKING))
        .check("the worn rod is in the storage beside the whole one left", move |b, _| {
            same("rods in the station", count_of(&b.items(station), ROD), 1)?;
            same("whole rods in the storage", held(b, line_at(o), ROD)?, 1)?;
            same("worn rods in the storage", worn_in_storage(b, o)?, 1)
        })
}

fn the_storage_orders_rods_for_its_stations(o: Pos, _: &Tuning) -> Scenario {
    let parts: [(&str, u32); 2] = [(STICK, 6), (STRING, 4)];
    let rod_orders = move |b: &dyn Bench| -> Vec<u32> {
        b.orders(terminal_at(line_at(o))).into_iter().filter(|r| r.item == ROD).map(|r| r.count).collect()
    };
    Scenario::new("rods21_the_storage_orders_rods_for_its_stations", plot_area(o))
        .then("a storage on a tier 3 line, a chest of 6 sticks and 4 string, a pond at the line's end", move |b, _| {
            line_and_pond(b, o, &parts)
        })
        .until("the storage holds them, no rods", 2 * FILL, move |b, _| holds(b, line_at(o), &parts))
        .then("an autocrafter with energy on the line, the book, two stations without rods", move |b, _| {
            machine(b, crafter_at(line_at(o)), CRAFTER, true)?;
            b.refresh();
            put_book(b, line_at(o))?;
            place_stations(b, o)
        })
        .until("the storage orders 2 rods once, and the stations say so", 400, move |b, _| {
            let orders = rod_orders(b);
            if orders.len() > 1 {
                return Err(format!("more than one rod order: {orders:?}"));
            }
            Ok(orders == [2]
                && stations(o)
                    .iter()
                    .all(|p| status_key(b, *p) == "rustcraft.util.fish_rod_ordered"))
        })
        .until("both stations get a crafted rod and fish", 3000, move |b, _| {
            if rod_orders(b).len() > 1 {
                return Err("a second rod order".into());
            }
            Ok(all_fish(b, o))
        })
        .until("the order is done", 200, move |b, _| Ok(rod_orders(b).is_empty()))
        .check("the sticks and the string went into the rods, none left over", move |b, _| {
            same("sticks", held(b, line_at(o), STICK)?, 0)?;
            same("string", held(b, line_at(o), STRING)?, 0)?;
            same("rods in the storage", held(b, line_at(o), ROD)?, 0)
        })
}
