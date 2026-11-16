//! Slice 18, stream 3: the machines as the handbook says.
//!
//! - U2: a higher tier's item on a placed machine replaces it in place; its items stay, the old
//!   tier's item comes back, and a tier that is not higher replaces nothing.

use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [tier_swap_keeps_contents];

const STONE: &str = "minecraft:stone";
const FURNACE: &str = "rustcraft:furnace";
const RAW_IRON: &str = "minecraft:raw_iron";

/// A floor of stone, x -1..=8 and z -3..=3 from `o`.
fn floor(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=8 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    Ok(())
}

fn tier_swap_keeps_contents(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("tier_swap_keeps_contents", plot_area(o))
        .then("a first-tier furnace with raw iron and no energy", move |b, _| {
            floor(b, o)?;
            b.place(p, FURNACE, &[])?;
            b.put(p, 0, RAW_IRON, 5)
        })
        .check("its tier is the first", move |b, _| {
            same("tier", b.property(p, "tier").as_deref(), Some("1"))
        })
        .then("a third-tier furnace replaces it", move |b, _| {
            let back = b.swap_tier(p, "rustcraft:furnace_3")?;
            same("given back", back.as_str(), FURNACE)
        })
        .check("the tier changed and the raw iron stayed", move |b, _| {
            same("tier", b.property(p, "tier").as_deref(), Some("3"))?;
            same("block", b.block(p).as_str(), FURNACE)?;
            same("raw iron", count_of(&b.items(p), RAW_IRON), 5)
        })
        .check("a second tier does not replace a third", move |b, _| {
            match b.swap_tier(p, "rustcraft:furnace_2") {
                Err(_) => same("tier", b.property(p, "tier").as_deref(), Some("3")),
                Ok(back) => Err(format!("a lower tier replaced it and gave back {back}")),
            }
        })
}
