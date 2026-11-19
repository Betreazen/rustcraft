//! 0.21 (spec 42, FT1–FT5; the owner: «вода, молоко и раствор периодически занимают место в одном
//! и том же баке, друг друга заменяя»): one fluid a tank structure.
//!
//! - A water tank and a milk tank set «из блока» give into a fluid line with two free tank
//!   structures, a pair of tanks and a single one: each structure takes one fluid, all of it.
//! - The water structure drained and the milk one full, more milk comes: the empty water
//!   structure stays kept for water and takes none of it; the milk waits at its source.

use super::magistral::line;
use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::geom::{Face, Pos};
use crate::net::fluids::BUCKET;
use crate::net::items::Mode;
use crate::net::settings::JointSetting;
use crate::net::tanks::TankGroup;
use crate::net::topology::FLUID_SLOT;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [one_fluid_a_structure];

const TANK: &str = "rustcraft:tank";
const WATER: &str = "minecraft:water";
const MILK: &str = "rustcraft:milk";
const LIMIT: u64 = 600;
/// What each source tank gives: less than the single tank holds.
const FLOW: u64 = 8 * BUCKET;

/// A tank of `fluid` beside housing `x` of the line, its joint set «из блока».
fn source(b: &mut dyn Bench, o: Pos, x: i32, fluid: &str) -> Result<(), String> {
    let tank = at(o, [x, 0, -1]);
    b.place(tank, TANK, &[])?;
    b.set_tank(tank, fluid, FLOW)?;
    b.set_joint(at(o, [x, 0, 0]), Face::ZNeg, FLUID_SLOT, JointSetting::of_mode(Mode::From))
}

fn group(b: &dyn Bench, pos: Pos) -> Result<TankGroup, String> {
    b.tank(pos).ok_or_else(|| format!("no tank at {pos:?}"))
}

fn one_fluid_a_structure(o: Pos, t: &Tuning) -> Scenario {
    let (water, milk) = (at(o, [1, 0, -1]), at(o, [3, 0, -1]));
    // Two structures apart: a pair stacked on the line and a single tank.
    let pair = [at(o, [5, 0, -1]), at(o, [5, 1, -1])];
    let single = at(o, [7, 0, -1]);
    let per_block = t.tank_buckets * BUCKET;
    Scenario::new("tanks21_one_fluid_a_structure", plot_area(o))
        .then(
            "a tier-1 fluid line: water and milk tanks that give, a free pair of tanks and a free single one",
            move |b, _| {
                line(b, o, 1, 8, 1)?;
                source(b, o, 1, WATER)?;
                source(b, o, 3, MILK)?;
                for p in pair {
                    b.place(p, TANK, &[])?;
                }
                b.place(single, TANK, &[])
            },
        )
        .until("the water and the milk left their tanks", LIMIT, move |b, _| {
            let gone = |p| b.tank(p).is_some_and(|g| g.amount == 0);
            Ok(gone(water) && gone(milk))
        })
        .check("each structure took one fluid, all of it, and is kept for it", move |b, _| {
            let (p, s) = (group(b, pair[0])?, group(b, single)?);
            same("the pair's blocks", p.blocks.len(), 2)?;
            same("the pair holds what it is kept for", p.fluid.clone(), p.pin.clone())?;
            same("the single tank holds what it is kept for", s.fluid.clone(), s.pin.clone())?;
            let mut kept = [p.pin.clone(), s.pin.clone()];
            kept.sort();
            same("the fluids kept", kept, [Some(WATER.to_owned()), Some(MILK.to_owned())])?;
            same("mB in the pair and the single tank", (p.amount, s.amount), (FLOW, FLOW))
        })
        .then(
            "the water structure drained, the milk one filled, 8 more buckets of milk at the source",
            move |b, _| {
                for at_ in [pair[0], single] {
                    let g = group(b, at_)?;
                    let (fluid, mb) = if g.pin.as_deref() == Some(WATER) { (WATER, 0) } else { (MILK, per_block) };
                    for &block in &g.blocks {
                        b.set_tank(block, fluid, mb)?;
                    }
                }
                b.set_tank(milk, MILK, FLOW)
            },
        )
        .wait(100)
        .check("the empty water structure took no milk; the milk waits at its source", move |b, _| {
            let (p, s) = (group(b, pair[0])?, group(b, single)?);
            let (dry, full) = if p.pin.as_deref() == Some(WATER) { (p, s) } else { (s, p) };
            same("the drained structure", (dry.fluid, dry.amount, dry.pin), (None, 0, Some(WATER.to_owned())))?;
            same("the milk structure", (full.fluid.as_deref(), full.amount), (Some(MILK), full.capacity))?;
            same("milk at the source, mB", group(b, milk)?.amount, FLOW)
        })
}
