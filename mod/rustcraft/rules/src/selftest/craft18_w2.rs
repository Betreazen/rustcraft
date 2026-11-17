//! Slice 18, wave 2 (w2-craft): the craft machines and the storage after the second audit.
//!
//! - The wrench does not turn the holo-projector: its click is the projector's own (the corners of
//!   its area), so the generic turn leaves it be.
//! - The terminal takes the stack module of crystallite (`module_stack_2`): the storage's stack is
//!   1024.
//! - Two fishing stations on one 5 × 5 of water: the second says the water is another's.
//! - The craft machines of tier 2 go up to tier 4: a feeder swapped to tier 4 has four sockets
//!   and its fourth takes a module; a kitchen of tier 4 cooks twice as fast as one of tier 2.
//! - A projector showing its ghost with no storage on its magistral says «Нет склада на
//!   магистрали».

use super::craft18::{basin, socket, status_key, storage_on_a_net};
use crate::storage::modules::{SOCKET_STACK, STACK_MODULES};
use super::{Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 5] = [
    projector_is_not_turned_by_the_wrench,
    terminal_takes_the_crystallite_stack_module,
    two_fisheries_on_one_patch,
    craft_machines_up_to_tier_four,
    projector_says_no_storage,
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

fn two_fisheries_on_one_patch(o: Pos, _: &Tuning) -> Scenario {
    let first = at(o, [4, 3, 0]);
    // Its 5 × 5 overlaps the first one's, but it stands outside the first one's water.
    let second = at(o, [7, 3, 0]);
    Scenario::new("w2_two_fisheries_on_one_patch", plot_area(o))
        .then("two stations with energy and rods on one basin", move |b, _| {
            basin(b, o)?;
            for p in [first, second] {
                b.place(p, "rustcraft:fishery", &[])?;
                b.set_buffer(p, 200.0)?;
                same("rod in", b.insert(p, "minecraft:fishing_rod", 1)?, 1)?;
            }
            Ok(())
        })
        .until("the second says the water is another station's", 100, move |b, _| {
            Ok(status_key(b, second) == "rustcraft.util.fish_shared")
        })
        .check("the first fishes", move |b, _| {
            same("first", status_key(b, first), "rustcraft.util.working".to_owned())
        })
}

fn craft_machines_up_to_tier_four(o: Pos, _: &Tuning) -> Scenario {
    use crate::machines::sheet::Press;
    let feeder = at(o, [2, 1, 0]);
    let low = at(o, [4, 1, 0]);
    let high = at(o, [6, 1, 0]);
    const ECONOMY: &str = "rustcraft:module_economy";
    Scenario::new("w2_craft_machines_up_to_tier_four", plot_area(o))
        .then("a feeder of tier 2, a kitchen of tier 2 and one of tier 4", move |b, _| {
            for p in [feeder, low, high] {
                b.place(at(p, [0, -1, 0]), "minecraft:stone", &[])?;
            }
            b.place(feeder, "rustcraft:feeder", &[])?;
            b.place(low, "rustcraft:kitchen", &[])?;
            b.place(high, "rustcraft:kitchen", &[("tier", "4")])
        })
        .then("the feeder swapped for tier 4", move |b, _| {
            same("given back", b.swap_tier(feeder, "rustcraft:feeder_4")?, "rustcraft:feeder".to_owned())
        })
        .wait(10)
        .check("four sockets, and the fourth takes a module", move |b, _| {
            same("sockets", b.sheet(feeder).map(|s| s.modules.len()), Some(4))?;
            same("cursor", b.press_cursor(feeder, Press::Socket(3).id(), Some((ECONOMY, 1)))?, None)
        })
        .check("the kitchen of tier 4 steps twice as often", move |b, _| {
            let step = |p| {
                b.sheet(p)
                    .and_then(|s| s.rows.get(1).cloned())
                    .map(|r| format!("{r:?}"))
                    .unwrap_or_default()
            };
            let (l, h) = (step(low), step(high));
            if l.contains("4.0") && h.contains("2.0") {
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
