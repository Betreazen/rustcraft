//! Slice 18, the last leftovers (plan `s18-last.md`):
//!
//! - The miner's storage doubles with each storage module past its window's 27 slots: the rest
//!   are hidden cells it keeps in its block entity (guide `mod_storage`).
//! - A stack module lets the brewer's reagents and blaze powder hold 256 a slot (guide
//!   `mod_stack`).

use super::{Build, Scenario, at, count_of, plot_area};
use crate::geom::Pos;
use crate::mining::Shown;
use crate::modules::Module;
use crate::tuning::Tuning;

pub const ALL: [Build; 2] = [miner_storage_beyond_the_window, brewer_stack_module];

const STONE: &str = "minecraft:stone";
const COBBLE: &str = "minecraft:cobblestone";
const IRON_ORE: &str = "minecraft:iron_ore";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_00b2;

fn miner_storage_beyond_the_window(o: Pos, _: &Tuning) -> Scenario {
    let m = at(o, [5, 6, 3]);
    let iron = at(m, [2, -6, -1]);
    Scenario::new("s18_miner_storage_beyond_the_window", plot_area(o))
        .then(
            "a miner with two storage modules (36 slots), its window's 27 full, an iron ore below",
            move |b, _| {
                b.place(m, "rustcraft:miner", &[])?;
                b.set_owner(m, OWNER, 1)?;
                b.place(at(m, [1, 0, 0]), "rustcraft:crystal", &[])?;
                b.place(at(m, [-1, 0, 0]), "rustcraft:crystal", &[])?;
                b.set_modules(m, &[Module::Storage.item(), Module::Storage.item()])?;
                for i in 0..crate::mining::WINDOW_SLOTS {
                    b.put(m, i, COBBLE, 64)?;
                }
                for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                    let p = at(iron, d);
                    if b.block(p) == "minecraft:air" {
                        b.place(p, STONE, &[])?;
                    }
                }
                b.place(iron, IRON_ORE, &[])
            },
        )
        .until("it still digs: the ore is stone again", 1200, move |b, _| {
            Ok(b.block(iron) == STONE)
        })
        .check("its raw iron is in a hidden cell, not in the window's slots", move |b, _| {
            let shown = count_of(&b.items(m), "minecraft:raw_iron");
            if shown != 0 {
                return Err(format!("the window's slots hold {shown} raw iron"));
            }
            if b.mining(m) == Some(Shown::Full) {
                return Err("it says «Некуда складывать добытое» with room in its cells".into());
            }
            Ok(())
        })
}

fn brewer_stack_module(o: Pos, _: &Tuning) -> Scenario {
    const POWDER: &str = "minecraft:blaze_powder";
    let p = at(o, [3, 1, 0]);
    // More than its four reagent slots and its powder slot hold without the module (5 × 64).
    const MORE: u32 = 5 * 64 + 64;
    Scenario::new("s18_brewer_stack_module", plot_area(o))
        .then("a brewer with a stack module, no energy", move |b, _| {
            b.place(at(p, [0, -1, 0]), STONE, &[])?;
            b.place(p, "rustcraft:brewer", &[])?;
            b.set_modules(p, &[Module::Stack.item()])
        })
        .until("it took more blaze powder than five slots of 64", 400, move |b, m| {
            let got = b.insert(p, POWDER, 16)?;
            m.mark("taken", m.get("taken") + f64::from(got));
            Ok(m.get("taken") >= f64::from(MORE))
        })
        .check("and keeps it: what its slots show is less than it took", move |b, m| {
            let shown = count_of(&b.items(p), POWDER);
            if f64::from(shown) < m.get("taken") {
                Ok(())
            } else {
                Err(format!("its slots show {shown} of {}", m.get("taken")))
            }
        })
}
