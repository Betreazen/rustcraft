//! Slice 18, stream 5: the craft machines and the storage as the handbook says them.
//!
//! - The storage keeps stacks with components (an enchanted book, a named block) whole.
//! - The stack module in the terminal's socket raises the stack of the whole storage (S3); the
//!   filter module goes into its own socket and comes back out.
//! - An extra terminal anywhere on the storage's magistral shows the storage (S6).
//! - A full storage says «Склад полон».
//! - A storage with a missing cell has its terminal keep the cell for clients to draw (S8).
//! - An order for bread runs through a kitchen on the storage's line (guide `kitchen`).
//! - The repair stand's window switches its work mode; the feeder's has none (guide MODES).
//! - The repair stand mends a fishing rod with string (guide `repair`).
//! - The kitchen bakes a cake with milk from the tank it touches and leaves no bucket.
//! - The fishing station keeps a rod's last point and says the rod is worn; a block over the
//!   water closes it.
//! - The butcher stands without energy; with it, its kill's experience goes into the condenser
//!   beside it (K6).

use super::transport::{chest, fill, item_line, joint};
use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::net::items::Mode;
use crate::storage::modules::{FILTER, SOCKET_FILTER, SOCKET_STACK, STACK_MODULES};
use crate::storage::terminal::{self, Press};
use crate::tuning::Tuning;

pub const ALL: [Build; 12] = [
    butcher_energy_and_kill_xp,
    fishery_keeps_the_rods_last_point,
    kitchen_cake_with_milk_from_a_tank,
    repair_mends_a_rod_with_string,
    util_work_mode_switch,
    order_bread_through_the_kitchen,
    storage_terminal_keeps_its_holes,
    storage_full_says_so,
    storage_far_terminal,
    storage_keeps_component_stacks,
    storage_stack_module_raises_the_stack,
    storage_filter_socket,
];

const CELL: &str = "rustcraft:cell";
const TERMINAL: &str = "rustcraft:terminal";
/// How long a net has to bring what it can.
const SETTLE: u64 = 100;

/// A chest «из блока» at housing 1, an item net of tier 2 and a storage of two cells with a
/// terminal touching housings 3 and 4.
fn storage_on_a_net(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    item_line(b, o, 2, 4, &[1])?;
    joint(b, o, 1, Mode::From, 0)?;
    b.place(at(o, [3, 0, 1]), CELL, &[])?;
    b.place(at(o, [4, 0, 1]), CELL, &[])?;
    b.place(at(o, [5, 0, 1]), TERMINAL, &[])
}

const BOOK: &str = r#"{id:"minecraft:enchanted_book",count:1,components:{"minecraft:stored_enchantments":{"minecraft:sharpness":3}}}"#;
const NAMED: &str = r#"{id:"minecraft:stone",count:5,components:{"minecraft:custom_name":"Plan"}}"#;

fn storage_keeps_component_stacks(o: Pos, _: &Tuning) -> Scenario {
    let a = chest(o, 1);
    let t = at(o, [5, 0, 1]);
    Scenario::new("storage18_keeps_component_stacks", plot_area(o))
        .then("a storage on a net", move |b, _| storage_on_a_net(b, o))
        .then("an enchanted book and named stone in the source", move |b, _| {
            same("book in", b.insert_snbt(a, BOOK)?, 1)?;
            same("stone in", b.insert_snbt(a, NAMED)?, 5)
        })
        .until("the storage holds both", SETTLE, move |b, _| {
            Ok(b.stored_stacks(t).len() == 2)
        })
        .check("each comes out with its components", move |b, _| {
            let mut got = b.stored_stacks(t);
            got.sort();
            same(
                "stacks",
                got,
                vec![
                    "minecraft:enchanted_book x1 minecraft:sharpness 3".to_owned(),
                    "minecraft:stone x5 \"Plan\"".to_owned(),
                ],
            )
        })
        .check("its record comes back the same from a save", move |b, _| {
            b.round_trip(t)
        })
}

/// The press id of terminal socket `k`.
fn socket(k: usize) -> Result<i32, String> {
    terminal::encode(Press::Socket(k as u8)).ok_or_else(|| format!("socket {k} has no press"))
}

fn storage_stack_module_raises_the_stack(o: Pos, _: &Tuning) -> Scenario {
    let t = at(o, [5, 0, 1]);
    let module = STACK_MODULES[0].0;
    Scenario::new("storage18_stack_module_raises_the_stack", plot_area(o))
        .then("a storage of two cells", move |b, _| storage_on_a_net(b, o))
        .then("the stack module into the terminal's socket", move |b, _| {
            let after = b.press_cursor(t, socket(SOCKET_STACK)?, Some((module, 1)))?;
            same("cursor after the module went in", after, None)
        })
        .wait(5)
        .check("a slot holds 256, the storage 54 of them", move |b, _| {
            let v = b.storage(t).ok_or("no storage")?;
            same("stack tier", v.store.stack_module, 1)?;
            same("room", v.store.room_for("minecraft:cobblestone", &|_| 64), 54 * 256)
        })
        .then("the module back out", move |b, _| {
            let after = b.press_cursor(t, socket(SOCKET_STACK)?, None)?;
            same("cursor", after, Some((module.to_owned(), 1)))
        })
        .wait(5)
        .check("the stack is 64 again", move |b, _| {
            let v = b.storage(t).ok_or("no storage")?;
            same("stack tier", v.store.stack_module, 0)
        })
}

fn storage_filter_socket(o: Pos, _: &Tuning) -> Scenario {
    let t = at(o, [5, 0, 1]);
    Scenario::new("storage18_filter_socket", plot_area(o))
        .then("a storage", move |b, _| storage_on_a_net(b, o))
        .then("a stone goes nowhere, the filter module goes in", move |b, _| {
            let refused = b.press_cursor(t, socket(SOCKET_FILTER)?, Some(("minecraft:stone", 1)))?;
            same("stone stays on the cursor", refused, Some(("minecraft:stone".to_owned(), 1)))?;
            let after = b.press_cursor(t, socket(SOCKET_FILTER)?, Some((FILTER, 1)))?;
            same("cursor after the module went in", after, None)
        })
        .check("it comes back out", move |b, _| {
            let after = b.press_cursor(t, socket(SOCKET_FILTER)?, None)?;
            same("cursor", after, Some((FILTER.to_owned(), 1)))
        })
}

fn storage_far_terminal(o: Pos, _: &Tuning) -> Scenario {
    let far = at(o, [1, 0, 1]);
    Scenario::new("storage18_far_terminal_on_the_net", plot_area(o))
        .then("a storage, and a terminal by housing 1 away from its cells", move |b, _| {
            storage_on_a_net(b, o)?;
            b.place(far, TERMINAL, &[])
        })
        .until("the far terminal shows the storage", 40, move |b, _| {
            Ok(b.storage(far).is_some_and(|v| v.cells == 2))
        })
}

fn storage_full_says_so(o: Pos, _: &Tuning) -> Scenario {
    let a = chest(o, 1);
    let cell = at(o, [3, 0, 1]);
    Scenario::new("storage18_full_says_so", plot_area(o))
        .then("a storage of one cell on a net", move |b, _| {
            item_line(b, o, 2, 4, &[1])?;
            joint(b, o, 1, Mode::From, 0)?;
            b.place(cell, CELL, &[])?;
            b.place(at(o, [3, 0, 2]), TERMINAL, &[])
        })
        .then("27 swords (one a slot) in the source", move |b, _| {
            fill(b, a, "minecraft:iron_sword", 27)
        })
        .until("the storage says it is full", 200, move |b, _| {
            Ok(b.storage_diagnosis(cell).as_deref() == Some("rustcraft.storage.full_at"))
        })
}

fn storage_terminal_keeps_its_holes(o: Pos, _: &Tuning) -> Scenario {
    let t = at(o, [5, 0, 1]);
    let gone = at(o, [3, 0, 2]);
    Scenario::new("storage18_terminal_keeps_its_holes", plot_area(o))
        .then("a storage of four cells in a 2x2 box", move |b, _| {
            storage_on_a_net(b, o)?;
            b.place(at(o, [4, 0, 2]), CELL, &[])?;
            b.place(gone, CELL, &[])
        })
        .wait(20)
        .check("no hole yet", move |b, _| same("holes", b.int_array(t, "holes"), Vec::new()))
        .then("a corner cell taken away", move |b, _| b.place(gone, "minecraft:air", &[]))
        .until("the terminal keeps the hole, as an offset from it", 60, move |b, _| {
            Ok(b.int_array(t, "holes") == vec![gone[0] - t[0], gone[1] - t[1], gone[2] - t[2]])
        })
}

fn order_bread_through_the_kitchen(o: Pos, _: &Tuning) -> Scenario {
    use super::slice15::{build, crafter_at, held, put_book, terminal_at};
    const WHEAT: &str = "minecraft:wheat";
    const BREAD: &str = "minecraft:bread";
    Scenario::new("orders18_bread_through_the_kitchen", plot_area(o))
        .then("a storage on a tier 3 line with wheat", move |b, _| build(b, o, &[(WHEAT, 3)]))
        .until("the storage holds the wheat", 200, move |b, _| Ok(held(b, o, WHEAT)? == 3))
        .then("a kitchen on the line with energy, and the book", move |b, _| {
            b.place(crafter_at(o), "rustcraft:kitchen", &[])?;
            b.set_buffer(crafter_at(o), 400.0)?;
            put_book(b, o)
        })
        .until("the storage takes an order for bread", 200, move |b, _| {
            Ok(b.order(terminal_at(o), BREAD, 1).is_ok())
        })
        .until("the bread is back in the storage", 1200, move |b, _| Ok(held(b, o, BREAD)? == 1))
        .check("the wheat went into it and the order is done", move |b, _| {
            same("wheat", held(b, o, WHEAT)?, 0)?;
            same("orders", b.orders(terminal_at(o)).len(), 0)
        })
}

fn util_work_mode_switch(o: Pos, _: &Tuning) -> Scenario {
    use crate::machines::processor::WorkMode;
    use crate::machines::sheet::Press as SheetPress;
    let p = at(o, [3, 1, 0]);
    let f = at(o, [5, 1, 0]);
    Scenario::new("util18_work_mode_switch", plot_area(o))
        .then("a repair stand and a feeder", move |b, _| {
            b.place(at(o, [3, 0, 0]), "minecraft:stone", &[])?;
            b.place(at(o, [5, 0, 0]), "minecraft:stone", &[])?;
            b.place(p, "rustcraft:repair", &[])?;
            b.place(f, "rustcraft:feeder", &[])
        })
        .check("the stand works in the normal mode, the feeder has no switch", move |b, _| {
            same("stand", b.sheet(p).and_then(|s| s.mode), Some(WorkMode::Normal.code()))?;
            same("feeder", b.sheet(f).and_then(|s| s.mode), None)
        })
        .then("boost pressed", move |b, _| b.press(p, SheetPress::Mode(WorkMode::Boost).id(), None))
        .check("the stand boosts", move |b, _| {
            same("stand", b.sheet(p).and_then(|s| s.mode), Some(WorkMode::Boost.code()))
        })
}

fn repair_mends_a_rod_with_string(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    const ROD: &str = r#"{id:"minecraft:fishing_rod",count:1,components:{"minecraft:damage":30}}"#;
    Scenario::new("repair18_mends_a_rod_with_string", plot_area(o))
        .then("a repair stand with energy, a worn rod and two string", move |b, _| {
            b.place(at(o, [3, 0, 0]), "minecraft:stone", &[])?;
            b.place(p, "rustcraft:repair", &[])?;
            b.set_buffer(p, 400.0)?;
            same("rod in", b.insert_snbt(p, ROD)?, 1)?;
            same("string in", b.insert(p, "minecraft:string", 2)?, 2)
        })
        .until("the mended rod is in an output cell", 600, move |b, _| {
            Ok(b.items(p).iter().skip(11).flatten().any(|s| s.0 == "minecraft:fishing_rod"))
        })
        .check("both string went into it", move |b, _| {
            same("string", super::count_of(&b.items(p), "minecraft:string"), 0)
        })
}

fn kitchen_cake_with_milk_from_a_tank(o: Pos, _: &Tuning) -> Scenario {
    use crate::machines::sheet::Press as SheetPress;
    let p = at(o, [3, 1, 0]);
    let tank = at(o, [4, 1, 0]);
    Scenario::new("kitchen18_cake_with_milk_from_a_tank", plot_area(o))
        .then("a kitchen with cake's dry parts beside a tank of milk", move |b, _| {
            for x in 2..=5 {
                b.place(at(o, [x, 0, 0]), "minecraft:stone", &[])?;
            }
            b.place(p, "rustcraft:kitchen", &[])?;
            b.set_buffer(p, 400.0)?;
            b.place(tank, "rustcraft:tank", &[])?;
            b.set_tank(tank, "rustcraft:milk", 3000)?;
            b.put(p, 0, "minecraft:sugar", 2)?;
            b.put(p, 1, "minecraft:egg", 1)?;
            b.put(p, 2, "minecraft:wheat", 3)
        })
        .then("the first page set to cake", move |b, _| {
            for _ in 0..4 {
                b.press_cursor(p, SheetPress::Toggle(0).id(), None)?;
            }
            Ok(())
        })
        .until("a cake in its output", 400, move |b, _| {
            Ok(b.items(p).iter().skip(9).flatten().any(|s| s.0 == "minecraft:cake"))
        })
        .check("the milk came from the tank and no bucket is left", move |b, _| {
            same("milk", b.tank(tank).map(|g| g.amount), Some(0))?;
            same("buckets", super::count_of(&b.items(p), "minecraft:bucket"), 0)
        })
}

/// The translation key of the status line of the machine window at `pos`.
fn status_key(b: &dyn Bench, pos: Pos) -> String {
    b.sheet(pos)
        .and_then(|s| s.status)
        .and_then(|s| s.text.get("translate").and_then(|t| t.as_str()).map(str::to_owned))
        .unwrap_or_default()
}

/// A stone basin of water 5 × 5, 2 deep, round x 4, z 0 from `o`, its surface at y 3.
fn basin(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in 1..=7 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
            for y in 1..=2 {
                let inside = (2..=6).contains(&x) && (-2..=2).contains(&z);
                b.place(at(o, [x, y, z]), if inside { "minecraft:water" } else { "minecraft:stone" }, &[])?;
            }
        }
    }
    Ok(())
}

fn fishery_keeps_the_rods_last_point(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [4, 3, 0]);
    const ROD: &str = r#"{id:"minecraft:fishing_rod",count:1,components:{"minecraft:damage":63}}"#;
    Scenario::new("fishery18_keeps_the_rods_last_point", plot_area(o))
        .then("a station on open water with a rod one catch from breaking", move |b, _| {
            basin(b, o)?;
            b.place(p, "rustcraft:fishery", &[])?;
            b.set_buffer(p, 200.0)?;
            same("rod in", b.insert_snbt(p, ROD)?, 1)
        })
        .until("it says the rod is worn", 100, move |b, _| {
            Ok(status_key(b, p) == "rustcraft.util.fish_rod_worn")
        })
        .check("the rod is still there", move |b, _| {
            same("rod", super::count_of(&b.items(p), "minecraft:fishing_rod"), 1)
        })
        .then("a block over the water", move |b, _| b.place(at(o, [2, 3, 2]), "minecraft:stone", &[]))
        .until("it says there is not enough open water", 100, move |b, _| {
            Ok(status_key(b, p) == "rustcraft.util.fish_no_water")
        })
}

fn butcher_energy_and_kill_xp(o: Pos, _: &Tuning) -> Scenario {
    use crate::butcher::{self, Shown as ButcherShown};
    let p = at(o, [3, 1, 0]);
    let tank = at(o, [4, 1, 0]);
    Scenario::new("butcher18_energy_and_kill_xp", plot_area(o))
        .then("a butcher with a sword and no energy, a condenser beside it, three cows", move |b, _| {
            for x in -1..=8 {
                for z in -3..=3 {
                    b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
                }
            }
            b.place(p, butcher::BLOCK, &[])?;
            b.set_owner(p, 0x5e1f_7e57_0000_0000_0000_0000_0000_0b18, 1)?;
            b.put(p, butcher::SWORD_SLOT, "minecraft:iron_sword", 1)?;
            b.place(tank, "rustcraft:xp_tank", &[])?;
            for i in 0..3 {
                let at = [f64::from(o[0]) + 1.5 + f64::from(i), f64::from(o[1] + 1), f64::from(o[2]) + 2.5];
                b.spawn_mob("minecraft:cow", at, None)?;
            }
            Ok(())
        })
        .until("it says it has no energy", 400, move |b, _| Ok(b.butcher(p) == Some(ButcherShown::NoEnergy)))
        .then("energy into its buffer", move |b, _| b.set_buffer(p, butcher::BUFFER))
        .until("the kill's experience is in the condenser", 600, move |b, _| Ok(b.long(tank, "util_xp") > 0))
}
