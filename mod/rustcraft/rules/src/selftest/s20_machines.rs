//! 0.20 (round 31): the machines stream.
//!
//! - The electric furnace takes by itself only dust and raw food: from a chest on its line it
//!   smelts the dust and the cod, never the birch logs beside them; a hopper puts in neither logs,
//!   sand nor ore, but cod.
//! - A fishing station's worn rod rides to the repair stand on its magistral, the storage there
//!   gives the stand the string it needs, and the rod comes back whole; meanwhile the station says
//!   its rod is at the repair.
//! - Ore in a storage becomes dust on an idle crusher of its line, the dust back in the storage.
//! - The owner's Path-of-craft-tree summary speeds up a crusher: the smith's branch at its last
//!   level grinds two ores while another crusher grinds one.

use super::craft18::status_key;
use super::magistral::{channel, place_node};
use super::slice15::{CRUSHER, DUST_IRON, RAW_IRON, build, crusher_on, held};
use super::transport::{chest, fill, item_line, joint};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::net::items::Mode;
use crate::net::topology::ChannelKind;
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [
    furnace_takes_dust_and_raw_food_by_itself,
    worn_rod_rides_to_the_stand_and_back,
    storage_ore_becomes_dust_on_an_idle_crusher,
    owners_tree_speeds_up_a_crusher,
];

const FURNACE: &str = "rustcraft:furnace";
const CRYSTAL: &str = "rustcraft:crystal";
const STRING: &str = "minecraft:string";
const ROD: &str = "minecraft:fishing_rod";

fn furnace_takes_dust_and_raw_food_by_itself(o: Pos, _: &Tuning) -> Scenario {
    // The owner: «зачем у меня постоянно забирает электропечь брёвна берёзовые и пережаривает их в
    // угли».
    const LOG: &str = "minecraft:birch_log";
    const COD: &str = "minecraft:cod";
    const COOKED: &str = "minecraft:cooked_cod";
    const INGOT: &str = "minecraft:iron_ingot";
    const CHARCOAL: &str = "minecraft:charcoal";
    let (left, right) = (chest(o, 1), chest(o, 3));
    let furnace = at(o, [2, 0, -1]);
    let lone = at(o, [8, 0, -1]);
    Scenario::new("furnace20_takes_dust_and_raw_food_by_itself", plot_area(o))
        .then("a lone furnace", move |b, _| b.place(lone, FURNACE, &[]))
        .check("a hopper puts in no logs, sand or ore, but cod", move |b, _| {
            same("birch logs", b.insert(lone, LOG, 4)?, 0)?;
            same("sand", b.insert(lone, "minecraft:sand", 4)?, 0)?;
            same("raw iron", b.insert(lone, RAW_IRON, 4)?, 0)?;
            same("cod", b.insert(lone, COD, 2)?, 2)
        })
        .then(
            "a chest of birch logs, dust and cod, a furnace with a crystal and an empty chest on an item line",
            move |b, _| {
                item_line(b, o, 1, 3, &[1, 3])?;
                b.place(furnace, FURNACE, &[])?;
                b.place(at(o, [2, 0, -2]), CRYSTAL, &[])?;
                fill(b, left, LOG, 8)?;
                fill(b, left, DUST_IRON, 2)?;
                fill(b, left, COD, 2)
            },
        )
        .until("two ingots and two cooked cod reach the chests", 3600, move |b, _| {
            let f = b.items(furnace);
            if count_of(&f, LOG) + count_of(&f, CHARCOAL) > 0 {
                return Err("the furnace took the logs".into());
            }
            let both = |item: &str| count_of(&b.items(left), item) + count_of(&b.items(right), item);
            Ok(both(INGOT) == 2 && both(COOKED) == 2)
        })
        .check("the logs stay in the left chest, no charcoal anywhere", move |b, _| {
            same("logs", count_of(&b.items(left), LOG), 8)?;
            same(
                "charcoal",
                count_of(&b.items(left), CHARCOAL) + count_of(&b.items(right), CHARCOAL),
                0,
            )
        })
}

fn worn_rod_rides_to_the_stand_and_back(o: Pos, _: &Tuning) -> Scenario {
    // The owner: «рыболовный стенд, у него портится удочка, у нас там есть ремонтный стенд, чтобы
    // он корректно забирал, отдавал удочку… забирал для её починки необходимые ресурсы… она целая,
    // потом возвращалась». A rod of 64 points one point from breaking: 4 string mend it whole.
    const WORN: &str = r#"{id:"minecraft:fishing_rod",count:1,components:{"minecraft:damage":63}}"#;
    let station = at(o, [0, 0, 0]);
    let stand = at(o, [8, 0, -1]);
    let terminal = at(o, [6, 0, 1]);
    let stored = move |b: &dyn Bench, item: &str| -> Result<u64, String> {
        Ok(b.storage(terminal).ok_or("the terminal shows no storage")?.store.count(item))
    };
    Scenario::new("trip20_worn_rod_rides_to_the_stand_and_back", plot_area(o))
        .then(
            "eight housings with an item channel, a chest of string, a storage and a repair stand with energy",
            move |b, _| {
                for x in 1..=8 {
                    let h = at(o, [x, 0, 0]);
                    b.place(h, "rustcraft:housing", &[])?;
                    b.set_channels(h, [channel(ChannelKind::Item, 2), None, None, None])?;
                }
                place_node(b, at(o, [9, 0, 0]), 1)?;
                b.place(chest(o, 3), "minecraft:chest", &[])?;
                joint(b, o, 3, Mode::From, 0)?;
                for c in [[4, 0, 1], [5, 0, 1], [4, 0, 2], [5, 0, 2]] {
                    b.place(at(o, c), "rustcraft:cell", &[])?;
                }
                b.place(terminal, "rustcraft:terminal", &[])?;
                b.place(stand, "rustcraft:repair", &[])?;
                b.place(at(stand, [0, 0, -1]), CRYSTAL, &[])?;
                b.set_buffer(stand, 400.0)?;
                fill(b, chest(o, 3), STRING, 8)
            },
        )
        .until("the storage holds the string", 200, move |b, _| Ok(stored(b, STRING)? == 8))
        .then("a fishing station with a worn rod at the line's end", move |b, _| {
            b.place(station, "rustcraft:fishery", &[])?;
            same("worn rod in", b.insert_snbt(station, WORN)?, 1)
        })
        .until("the rod is at the stand and the station says so", 400, move |b, _| {
            let at_stand = count_of(&b.items(stand), ROD) == 1;
            Ok(at_stand && status_key(b, station) == "rustcraft.util.fish_rod_at_repair")
        })
        .check("its cell takes no other rod meanwhile", move |b, _| {
            same("a second rod", b.insert(station, ROD, 1)?, 0)
        })
        .until("the mended rod is back in the station", 1600, move |b, _| {
            Ok(count_of(&b.items(station), ROD) == 1 && count_of(&b.items(stand), ROD) == 0)
        })
        .check("the storage gave the stand four string, all spent", move |b, _| {
            same("string in the storage", stored(b, STRING)?, 4)?;
            same("string at the stand", count_of(&b.items(stand), STRING), 0)?;
            same("rods in the storage", stored(b, ROD)?, 0)
        })
}

fn storage_ore_becomes_dust_on_an_idle_crusher(o: Pos, _: &Tuning) -> Scenario {
    // The owner: «если положили руду… из неё однозначно сразу можно делать пыль, если есть дробилки,
    // и они там свободны от заказов».
    Scenario::new("ore20_storage_ore_becomes_dust_on_an_idle_crusher", plot_area(o))
        .then("a storage on a line, a chest of 4 raw iron", move |b, _| build(b, o, &[(RAW_IRON, 4)]))
        .until("the storage holds the ore", 200, move |b, _| Ok(held(b, o, RAW_IRON)? == 4))
        .then("a crusher with energy on the line, no order", move |b, _| crusher_on(b, o))
        .until("the ore is dust in the storage, two an ore", 3600, move |b, _| {
            Ok(held(b, o, RAW_IRON)? == 0 && held(b, o, DUST_IRON)? == 8)
        })
}

fn owners_tree_speeds_up_a_crusher(o: Pos, _: &Tuning) -> Scenario {
    const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0c20;
    const OTHER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0c21;
    let fast = at(o, [2, 1, 0]);
    let plain = at(o, [8, 1, 0]);
    Scenario::new("perk20_owners_tree_speeds_up_a_crusher", plot_area(o))
        .then(
            "two crushers with a crystal each and two raw iron each; one owner is a smith of the last level",
            move |b, _| {
                let smith = crate::perks::fixture(&["smithing"], 75, 68, 68);
                b.set_pct_summary(OWNER, Some(&smith))?;
                for p in [fast, plain] {
                    b.place(p, CRUSHER, &[])?;
                    b.place(at(p, [0, 1, 0]), CRYSTAL, &[])?;
                }
                b.set_owner(fast, OWNER, 1)?;
                b.set_owner(plain, OTHER, 1)?;
                for p in [fast, plain] {
                    let slot = b.inputs(p).first().copied().ok_or("the crusher has no input")?;
                    b.put(p, slot, RAW_IRON, 2)?;
                }
                Ok(())
            },
        )
        .until("the smith's crusher ground both", 2400, move |b, _| {
            Ok(count_of(&b.items(fast), DUST_IRON) == 4)
        })
        .check("the other ground one at most meanwhile (×1.5)", move |b, _| {
            let n = count_of(&b.items(plain), DUST_IRON);
            if n <= 2 { Ok(()) } else { Err(format!("the other crusher made {n} dust as fast")) }
        })
        .until("the other grinds both too", 2400, move |b, _| {
            Ok(count_of(&b.items(plain), DUST_IRON) == 4)
        })
        .then("the summary taken away", move |b, _| b.set_pct_summary(OWNER, None))
}
