//! Slice 18, wave 2 (w2-craft): the craft machines and the storage after the second audit.
//!
//! - The wrench does not turn the holo-projector: its click is the projector's own (the corners of
//!   its area), so the generic turn leaves it be.
//! - The terminal takes the stack module of crystallite (`module_stack_2`): the storage's stack is
//!   1024.
//! - Two fishing stations on one 5 × 5 of water: the second says the water is another's.
//! - The craft machines of tier 2 go up to tier 4: a feeder swapped to tier 4 has four sockets
//!   and its fourth takes a module; a repair stand of tier 4 works twice as fast as one of tier 2.
//! - A projector showing its ghost with no storage on its magistral says «Нет склада на
//!   магистрали».
//! - The kitchen bakes a cake with milk from a tank it does not touch, through a fluid channel.
//! - A cell sticking out of the storage's box is the one the terminal marks (not two holes).
//! - A portal dock sends a tank's water (behind a joint that gives) to its pair, which unloads it
//!   into the tank it touches.

use super::craft18::{socket, status_key, storage_on_a_net};
use crate::storage::modules::{SOCKET_STACK, STACK_MODULES};
use super::{Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 8] = [
    projector_is_not_turned_by_the_wrench,
    terminal_takes_the_crystallite_stack_module,
    two_fisheries_on_one_patch,
    craft_machines_up_to_tier_four,
    projector_says_no_storage,
    kitchen_milk_through_a_channel,
    terminal_marks_the_extra_cell,
    portal_dock_sends_fluid_to_its_pair,
];

fn projector_is_not_turned_by_the_wrench(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("w2_projector_not_turned_by_the_wrench", plot_area(o))
        .then("a projector", move |b, _| {
            b.place(at(o, [3, 0, 0]), "minecraft:stone", &[])?;
            b.place(p, "rustcraft:projector", &[])
        })
        .check("the wrench's turn leaves it, so its corners can be clicked", move |b, _| {
            same("turned", b.pipe("rotate", p).is_ok(), false)
        })
}

fn terminal_takes_the_crystallite_stack_module(o: Pos, _: &Tuning) -> Scenario {
    let t = at(o, [5, 0, 1]);
    let module = STACK_MODULES[1].0;
    Scenario::new("w2_terminal_takes_the_crystallite_stack_module", plot_area(o))
        .then("a storage of two cells", move |b, _| storage_on_a_net(b, o))
        .then("the crystallite stack module into the terminal's socket", move |b, _| {
            let after = b.press_cursor(t, socket(SOCKET_STACK)?, Some((module, 1)))?;
            same("cursor after the module went in", after, None)
        })
        .wait(5)
        .check("a slot holds 1024", move |b, _| {
            let v = b.storage(t).ok_or("no storage")?;
            same("stack tier", v.store.stack_module, 2)?;
            same("room", v.store.room_for("minecraft:cobblestone", &|_| 64), 54 * 1024)
        })
}

/// 0.21 (owner 2026-11-19: stations close together must not block each other): four stations
/// side by side over one long pond, each with the 5 × 5 of water under it, all fish.
fn two_fisheries_on_one_patch(o: Pos, _: &Tuning) -> Scenario {
    let stations = [at(o, [4, 3, 0]), at(o, [5, 3, 0]), at(o, [6, 3, 0]), at(o, [7, 3, 0])];
    Scenario::new("fishery21_side_by_side_all_fish", plot_area(o))
        .then("a pond 8 by 5, 2 deep, and four stations side by side over it", move |b, _| {
            for x in 1..=10 {
                for z in -3..=3 {
                    b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
                    for y in 1..=2 {
                        let inside = (2..=9).contains(&x) && (-2..=2).contains(&z);
                        b.place(at(o, [x, y, z]), if inside { "minecraft:water" } else { "minecraft:stone" }, &[])?;
                    }
                }
            }
            for p in stations {
                b.place(p, "rustcraft:fishery", &[])?;
                b.set_buffer(p, 200.0)?;
                same("rod in", b.insert(p, "minecraft:fishing_rod", 1)?, 1)?;
            }
            Ok(())
        })
        .until("every station fishes", 100, move |b, _| {
            Ok(stations
                .iter()
                .all(|p| status_key(b, *p) == "rustcraft.util.working"))
        })
}

fn craft_machines_up_to_tier_four(o: Pos, _: &Tuning) -> Scenario {
    use crate::machines::sheet::Press;
    let feeder = at(o, [2, 1, 0]);
    let low = at(o, [4, 1, 0]);
    let high = at(o, [6, 1, 0]);
    const ECONOMY: &str = "rustcraft:module_economy";
    Scenario::new("w2_craft_machines_up_to_tier_four", plot_area(o))
        .then("a feeder of tier 2, a repair stand of tier 2 and one of tier 4", move |b, _| {
            for p in [feeder, low, high] {
                b.place(at(p, [0, -1, 0]), "minecraft:stone", &[])?;
            }
            b.place(feeder, "rustcraft:feeder", &[])?;
            b.place(low, "rustcraft:repair", &[])?;
            b.place(high, "rustcraft:repair", &[("tier", "4")])
        })
        .then("the feeder swapped for tier 4", move |b, _| {
            same("given back", b.swap_tier(feeder, "rustcraft:feeder_4")?, "rustcraft:feeder".to_owned())
        })
        .wait(10)
        .check("four sockets, and the fourth takes a module", move |b, _| {
            same("sockets", b.sheet(feeder).map(|s| s.modules.len()), Some(4))?;
            same("cursor", b.press_cursor(feeder, Press::Socket(3).id(), Some((ECONOMY, 1)))?, None)
        })
        // The stand's window keeps the step row (the kitchen's rows are its dish and its milk).
        .check("the repair stand of tier 4 steps twice as often", move |b, _| {
            let step = |p| {
                b.sheet(p)
                    .and_then(|s| s.rows.into_iter().map(|r| format!("{r:?}")).find(|r| r.contains("rustcraft.util.step")))
                    .unwrap_or_default()
            };
            let (l, h) = (step(low), step(high));
            if l.contains("5.0") && h.contains("2.5") {
                Ok(())
            } else {
                Err(format!("tier 2 {l}, tier 4 {h}"))
            }
        })
}

fn projector_says_no_storage(o: Pos, _: &Tuning) -> Scenario {
    use crate::machines::sheet::Press;
    let p = at(o, [3, 1, 0]);
    Scenario::new("w2_projector_says_no_storage", plot_area(o))
        .then("a projector with energy remembers a stone beside it", move |b, _| {
            b.place(at(o, [3, 0, 0]), "minecraft:stone", &[])?;
            b.place(p, "rustcraft:projector", &[])?;
            b.set_buffer(p, 400.0)?;
            b.place(at(p, [1, 1, 0]), "minecraft:stone", &[])?;
            b.press_cursor(p, Press::Toggle(0).id(), None).map(|_| ())
        })
        .until("its ghost shows and it says there is no storage on its magistral", 60, move |b, _| {
            Ok(status_key(b, p) == "rustcraft.util.projector_no_storage")
        })
}

fn kitchen_milk_through_a_channel(o: Pos, _: &Tuning) -> Scenario {
    use super::magistral::line;
    use crate::geom::Face;
    use crate::machines::sheet::Press;
    use crate::net::items::Mode;
    use crate::net::settings::JointSetting;
    let tank = at(o, [1, 0, -1]);
    let p = at(o, [4, 0, -1]);
    Scenario::new("w2_kitchen_milk_through_a_channel", plot_area(o))
        .then("a fluid line from a tank of milk (its joint gives) to a kitchen", move |b, _| {
            line(b, o, 1, 4, 1)?;
            b.place(tank, "rustcraft:tank", &[])?;
            b.set_tank(tank, "rustcraft:milk", 3000)?;
            b.place(p, "rustcraft:kitchen", &[])?;
            b.set_buffer(p, 400.0)?;
            b.set_joint(at(o, [1, 0, 0]), Face::ZNeg, crate::net::topology::FLUID_SLOT, JointSetting::of_mode(Mode::From))?;
            b.put(p, 0, "minecraft:sugar", 2)?;
            b.put(p, 1, "minecraft:egg", 1)?;
            b.put(p, 2, "minecraft:wheat", 3)
        })
        // 0.21 (spec 39): cake, the fourth dish, turned on.
        .then("cake turned on", move |b, _| {
            b.press_cursor(p, Press::Toggle(3).id(), None).map(|_| ())
        })
        .until("a cake in its output", 400, move |b, _| {
            Ok(b.items(p).iter().skip(9).flatten().any(|s| s.0 == "minecraft:cake"))
        })
        .check("the milk came from the tank down the channel", move |b, _| {
            same("milk", b.tank(tank).map(|g| g.amount), Some(0))
        })
}

fn terminal_marks_the_extra_cell(o: Pos, _: &Tuning) -> Scenario {
    const CELL: &str = "rustcraft:cell";
    let t = at(o, [5, 0, 1]);
    let extra = at(o, [3, 1, 1]);
    Scenario::new("w2_terminal_marks_the_extra_cell", plot_area(o))
        .then("a storage of four cells in a 2x2 and one on top", move |b, _| {
            storage_on_a_net(b, o)?;
            b.place(at(o, [3, 0, 2]), CELL, &[])?;
            b.place(at(o, [4, 0, 2]), CELL, &[])?;
            b.place(extra, CELL, &[])
        })
        .until("the terminal keeps the cell that sticks out, and no hole", 60, move |b, _| {
            Ok(b.int_array(t, "extras") == vec![extra[0] - t[0], extra[1] - t[1], extra[2] - t[2]]
                && b.int_array(t, "holes").is_empty())
        })
}

fn portal_dock_sends_fluid_to_its_pair(o: Pos, _: &Tuning) -> Scenario {
    use super::magistral::line;
    use crate::geom::Face;
    use crate::net::items::Mode;
    use crate::net::settings::JointSetting;
    const DOCK: &str = "rustcraft:portal_dock";
    const TANK: &str = "rustcraft:tank";
    const WATER: &str = "minecraft:water";
    let (a, b2) = (at(o, [3, 1, -1]), at(o, [4, 1, 1]));
    let (from, to) = (at(o, [0, 1, -2]), at(o, [4, 1, 2]));
    Scenario::new("w2_portal_dock_sends_fluid_to_its_pair", plot_area(o))
        .then("a nether portal with a dock on each side", move |b, _| {
            for x in 2..=5 {
                for y in 0..=4 {
                    let edge = x == 2 || x == 5 || y == 0 || y == 4;
                    if edge {
                        b.place(at(o, [x, y, 0]), "minecraft:obsidian", &[])?;
                    }
                }
            }
            for x in 3..=4 {
                for y in 1..=3 {
                    b.place(at(o, [x, y, 0]), "minecraft:nether_portal", &[("axis", "x")])?;
                }
            }
            for d in [a, b2] {
                b.place(d, DOCK, &[])?;
                b.set_buffer(d, 400.0)?;
            }
            b.pair_docks(a, b2)
        })
        .then("a tank of water behind a giving joint on the first dock's fluid line", move |b, _| {
            line(b, at(o, [-1, 1, -1]), 1, 3, 1)?;
            b.place(from, TANK, &[])?;
            b.set_tank(from, WATER, 5000)?;
            b.set_joint(at(o, [0, 1, -1]), Face::ZNeg, crate::net::topology::FLUID_SLOT, JointSetting::of_mode(Mode::From))?;
            b.place(to, TANK, &[])
        })
        .until("the water is in the tank by the other dock", 400, move |b, _| {
            Ok(b.tank(to).is_some_and(|g| g.amount == 5000))
        })
        .check("and gone from the first tank", move |b, _| {
            same("left", b.tank(from).map_or(0, |g| g.amount), 0)
        })
}
