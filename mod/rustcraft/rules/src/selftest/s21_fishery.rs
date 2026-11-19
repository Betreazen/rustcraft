//! 0.21 (spec 37, FS4–FS6; owner 2026-11-19: «смягчи правило в моде»): a fishing station needs
//! water 2 deep under it in a pond of 25 cells, not the whole 5 × 5 under it.
//!
//! - A pond 3 by 10, 2 deep, along a stone wall two blocks high: a station at its corner by the
//!   wall and one on its far edge both fish, though neither has the 5 × 5 of water under it.
//! - A pond of 6 cells beside it: its station says there is not enough open water.

use super::craft18::status_key;
use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [fishery_at_the_edge_of_a_pond];

const STATION: &str = "rustcraft:fishery";
const WORKING: &str = "rustcraft.util.working";
const NO_WATER: &str = "rustcraft.util.fish_no_water";

/// Stone from x 1..=12, z -3..=7 at y 0..=2, with water where `water` says at y 1..=2.
fn ground(b: &mut dyn Bench, o: Pos, water: impl Fn(i32, i32) -> bool) -> Result<(), String> {
    for x in 1..=12 {
        for z in -3..=7 {
            b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
            for y in 1..=2 {
                let block = if water(x, z) { "minecraft:water" } else { "minecraft:stone" };
                b.place(at(o, [x, y, z]), block, &[])?;
            }
        }
    }
    Ok(())
}

fn station(b: &mut dyn Bench, p: Pos) -> Result<(), String> {
    b.place(p, STATION, &[])?;
    b.set_buffer(p, 200.0)?;
    same("rod in", b.insert(p, "minecraft:fishing_rod", 1)?, 1)
}

fn fishery_at_the_edge_of_a_pond(o: Pos, _: &Tuning) -> Scenario {
    // The big pond: x 2..=11, z -1..=1 (30 cells); the small one: x 2..=3, z 5..=7 (6 cells).
    let corner = at(o, [2, 3, -1]);
    let edge = at(o, [8, 3, 1]);
    let small = at(o, [2, 3, 6]);
    Scenario::new("fishery21_edge_of_a_pond", plot_area(o))
        .then("a pond 3 by 10 along a wall, a pond of 6, three stations with rods", move |b, _| {
            ground(b, o, |x, z| {
                ((2..=11).contains(&x) && (-1..=1).contains(&z)) || ((2..=3).contains(&x) && (5..=7).contains(&z))
            })?;
            // A wall two blocks high along the big pond's north side, over stone.
            for x in 1..=12 {
                for y in 3..=4 {
                    b.place(at(o, [x, y, -2]), "minecraft:stone", &[])?;
                }
            }
            for p in [corner, edge, small] {
                station(b, p)?;
            }
            Ok(())
        })
        .until("the stations on the big pond fish", 100, move |b, _| {
            Ok(status_key(b, corner) == WORKING && status_key(b, edge) == WORKING)
        })
        .until("the station on the pond of 6 says it lacks water", 100, move |b, _| {
            Ok(status_key(b, small) == NO_WATER)
        })
}
