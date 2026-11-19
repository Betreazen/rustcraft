//! 0.21 (spec 36, OD9; owner 2026-11-19: «остатки … возвращали обратно на склад, … особенно вот
//! в плавильне сплавов»): inputs left standing in a machine go back to the storage.
//!
//! - A storage on a line, an alloy smelter with energy beside it. Half a recipe goes into the
//!   smelter by hand (gold and redstone, no coppersteel): it stands idle, no order needs it, and
//!   after 30 seconds both go back to the storage and the smelter's inputs are empty.

use super::magistral::place_housing;
use super::slice15::{FILL, build, held, holds, machine};
use super::transport::chest;
use super::{Build, Scenario, at, plot_area, same, total};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [half_a_recipe_goes_back];

const ALLOY: &str = "rustcraft:alloy";
const GOLD: &str = "minecraft:gold_ingot";
const REDSTONE: &str = "minecraft:redstone";
const PARTS: [(&str, u32); 1] = [("minecraft:cobblestone", 4)];
/// 30 seconds of standing and its ride back, ticks.
const LIMIT: u64 = 1200;

fn half_a_recipe_goes_back(o: Pos, _: &Tuning) -> Scenario {
    let smelter = chest(o, 3);
    Scenario::new("stranded21_half_a_recipe_goes_back", plot_area(o))
        .then("a storage on a line", move |b, _| {
            build(b, o, &PARTS)?;
            place_housing(b, at(o, [6, 0, 0]), 2)
        })
        .until("the storage holds its first items", 2 * FILL, move |b, _| holds(b, o, &PARTS))
        .then("an alloy smelter with energy, 3 gold and 2 redstone put in by hand", move |b, _| {
            machine(b, smelter, ALLOY, true)?;
            b.refresh();
            let inputs = b.inputs(smelter);
            b.put(smelter, inputs[0], GOLD, 3)?;
            b.put(smelter, inputs[1], REDSTONE, 2)
        })
        .until("the gold and the redstone are back in the storage", LIMIT, move |b, _| {
            Ok(held(b, o, GOLD)? == 3 && held(b, o, REDSTONE)? == 2)
        })
        .check("the smelter's inputs hold nothing", move |b, _| {
            let slots = b.items(smelter);
            let inputs: Vec<_> = b.inputs(smelter).into_iter().map(|k| slots.get(k).cloned().flatten()).collect();
            same("items left in the inputs", total(&inputs), 0)
        })
}
