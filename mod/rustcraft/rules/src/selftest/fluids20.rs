//! Round 31 (spec 29, FL1–FL7): the smart fluid line and the storage's fluids.
//!
//! - Water and lava ride one tier-1 channel past each other, each from a tank on its default
//!   joint to the consumer that wants it: the mixer at the far end, the lava generator at the near
//!   one (FL1, FL3).
//! - A mixer fed water by the line gives its solution to the storage's cells (FL3, FL4).
//! - An order for solution takes its water from the storage's cells and the solution comes back
//!   to them (FL6).
//! - A crystal structure takes the storage's solution through a channel water shares on its way
//!   to a mixer (FL3).
//! - The kitchen bakes a cake with milk from the storage on its fluid line (FL7).

use super::magistral::line;
use super::slice15::{FILL, build, crafter_at, holds, machine, put_book, terminal_at};
use super::{Bench, Build, Memo, Scenario, at, count_of, near, plot_area, same};
use crate::geom::Pos;
use crate::net::fluids::BUCKET;
use crate::orders::fluid_key;
use crate::power::{self as rules, Shown};
use crate::storage::Store;
use crate::tuning::Tuning;
use std::collections::BTreeSet;

pub const ALL: [Build; 5] = [
    two_fluids_pass_each_other,
    mixer_gives_its_fluid_to_the_storage,
    order_takes_water_from_the_storage,
    structure_fed_through_a_shared_channel,
    kitchen_takes_milk_from_the_storage,
];

const TANK: &str = "rustcraft:tank";
const MIXER: &str = "rustcraft:mixer";
const CRYSTAL: &str = "rustcraft:crystal";
const CELL: &str = "rustcraft:cell";
const LAVA_GENERATOR: &str = "rustcraft:gen_lava";
const WATER: &str = "minecraft:water";
const LAVA: &str = "minecraft:lava";
const SOLUTION: &str = "rustcraft:solution";
const MILK: &str = "rustcraft:milk";
const BONE_MEAL: &str = "minecraft:bone_meal";
const REDSTONE: &str = "minecraft:redstone";
const LIMIT: u64 = 600;

/// A tier-1 node and `len` tier-1 housings: every housing carries its fluid channel (spec 28, R1).
fn fluid_line(b: &mut dyn Bench, o: Pos, len: i32) -> Result<(), String> {
    line(b, o, 1, len, 1)
}

/// The storage the cell or terminal at `pos` belongs to.
fn store(b: &dyn Bench, pos: Pos) -> Result<Store, String> {
    b.storage(pos)
        .map(|v| v.store)
        .ok_or_else(|| format!("no storage at {pos:?}"))
}

fn tank_mb(b: &dyn Bench, pos: Pos) -> u64 {
    b.tank(pos).map_or(0, |g| g.amount)
}

fn mixer_water(b: &dyn Bench, pos: Pos) -> u64 {
    b.mixer(pos).map_or(0, |v| v.water)
}

/// A probe that waits for the storage of the cell or terminal at `pos` to be found, then gives
/// it `mb` of `fluid` in its cells.
fn storage_with(
    pos: Pos,
    fluid: &'static str,
    mb: u64,
) -> impl FnMut(&mut dyn Bench, &mut Memo) -> Result<bool, String> + Send + 'static {
    move |b, _| {
        if b.storage(pos).is_none() {
            return Ok(false);
        }
        b.set_storage_fluid(pos, fluid, mb)?;
        Ok(true)
    }
}

fn two_fluids_pass_each_other(o: Pos, _: &Tuning) -> Scenario {
    let (water, generator) = (at(o, [1, 0, -1]), at(o, [2, 0, -1]));
    let (lava, mixer) = (at(o, [5, 0, -1]), at(o, [6, 0, -1]));
    Scenario::new("fluids20_two_fluids_pass_each_other", plot_area(o))
        .then(
            "a tier-1 fluid line: water and a lava generator at one end, lava and a mixer at the other",
            move |b, _| {
                fluid_line(b, o, 6)?;
                b.place(water, TANK, &[])?;
                b.set_tank(water, WATER, 8 * BUCKET)?;
                b.place(generator, LAVA_GENERATOR, &[])?;
                b.place(lava, TANK, &[])?;
                b.set_tank(lava, LAVA, 8 * BUCKET)?;
                b.place(mixer, MIXER, &[])
            },
        )
        .until(
            "the water reached the mixer and the lava the generator",
            LIMIT,
            move |b, _| {
                let burning = b.power(generator).is_some_and(|v| v.fluid > 0.0);
                Ok(burning && mixer_water(b, mixer) >= BUCKET)
            },
        )
        .check("each came from its tank on its default joint, nothing refused", move |b, _| {
            same("water left the water tank", tank_mb(b, water) < 8 * BUCKET, true)?;
            same("lava left the lava tank", tank_mb(b, lava) < 8 * BUCKET, true)?;
            same("no lane refused (water)", b.no_lane(water), None)?;
            same("no lane refused (lava)", b.no_lane(lava), None)
        })
}

fn mixer_gives_its_fluid_to_the_storage(o: Pos, _: &Tuning) -> Scenario {
    let water = at(o, [1, 0, -1]);
    let m = at(o, [4, 0, -1]);
    let cells = [at(o, [2, 0, 1]), at(o, [3, 0, 1])];
    Scenario::new("fluids20_mixer_gives_its_fluid_to_the_storage", plot_area(o))
        .then(
            "a fluid line from a water tank to a mixer with energy and its parts, a storage on it",
            move |b, _| {
                fluid_line(b, o, 5)?;
                b.place(water, TANK, &[])?;
                b.set_tank(water, WATER, 8 * BUCKET)?;
                machine(b, m, MIXER, true)?;
                b.put(m, 0, BONE_MEAL, 2)?;
                b.put(m, 1, REDSTONE, 2)?;
                // 0.21 (spec 39): its solution recipe on.
                b.press(m, crate::machines::sheet::Press::Toggle(0).id(), None)?;
                for c in cells {
                    b.place(c, CELL, &[])?;
                }
                Ok(())
            },
        )
        .until("a bucket of solution in the storage's cells", LIMIT, move |b, _| {
            Ok(store(b, cells[0])?.fluid(SOLUTION) >= BUCKET)
        })
        .check("no tank holds it: it went into the cells", move |b, _| {
            same(
                "the water tank's fluid",
                b.tank(water).and_then(|g| g.fluid),
                Some(WATER.to_owned()),
            )?;
            same("items in the storage's slots", store(b, cells[0])?.items.len(), 0)
        })
}

/// The slice-15 line (a tier-3 node, tier-2 housings, the storage, the chest of parts): its
/// housings carry the fluid channel beside the item channel (spec 28, R1).
fn smart_fluid_line(b: &mut dyn Bench, o: Pos, parts: &[(&str, u32)]) -> Result<(), String> {
    build(b, o, parts)
}

fn order_takes_water_from_the_storage(o: Pos, _: &Tuning) -> Scenario {
    let parts: Vec<(&str, u32)> = vec![(BONE_MEAL, 2), (REDSTONE, 2)];
    let p2 = parts.clone();
    let m = crafter_at(o);
    let t = terminal_at(o);
    Scenario::new("fluids20_order_takes_water_from_the_storage", plot_area(o))
        .then(
            "a storage on a tier 3 line with item and fluid channels, the parts of solution",
            move |b, _| smart_fluid_line(b, o, &parts),
        )
        .until("the storage holds the parts", FILL, move |b, _| holds(b, o, &p2))
        .until("4 buckets of water in its cells", 40, storage_with(t, WATER, 4 * BUCKET))
        .then("a mixer with energy on the line, the book", move |b, _| {
            machine(b, m, MIXER, true)?;
            put_book(b, o)
        })
        .until(
            "the storage takes an order for a bucket of solution",
            FILL,
            move |b, _| Ok(b.order(t, &fluid_key(SOLUTION), 1).is_ok()),
        )
        .until("a bucket of solution back in the storage's cells", 1200, move |b, _| {
            Ok(store(b, t)?.fluid(SOLUTION) >= BUCKET)
        })
        .wait(40)
        .check("the water came from the cells: one bucket used, the order done", move |b, _| {
            let s = store(b, t)?;
            same("water in the cells and the mixer, mB", s.fluid(WATER) + mixer_water(b, m), 3 * BUCKET)?;
            same("solution in the cells, mB", s.fluid(SOLUTION), BUCKET)?;
            same("bone meal left", s.count(BONE_MEAL), 1)?;
            same("orders", b.orders(t).len(), 0)
        })
}

fn structure_fed_through_a_shared_channel(o: Pos, t: &Tuning) -> Scenario {
    let cells = [at(o, [1, 0, 1]), at(o, [2, 0, 1])];
    let pair = [at(o, [4, 0, 1]), at(o, [5, 0, 1])];
    let (water, m) = (at(o, [1, 0, -1]), at(o, [6, 0, -1]));
    let set: BTreeSet<Pos> = pair.into_iter().collect();
    let fed = rules::structure_output(2, rules::form(&set), true, t);
    Scenario::new("fluids20_structure_fed_through_a_shared_channel", plot_area(o))
        .then(
            "a tier-1 fluid line: a storage and a water tank at one end, a crystal pair and a mixer further on",
            move |b, _| {
                fluid_line(b, o, 6)?;
                for c in cells {
                    b.place(c, CELL, &[])?;
                }
                for c in pair {
                    b.place(c, CRYSTAL, &[])?;
                }
                b.place(water, TANK, &[])?;
                b.set_tank(water, WATER, 8 * BUCKET)?;
                b.place(m, MIXER, &[])
            },
        )
        .until("4 buckets of solution in the storage's cells", 40, storage_with(cells[0], SOLUTION, 4 * BUCKET))
        .until(
            "the solution reached the crystals and the water the mixer, by one channel",
            LIMIT,
            move |b, _| {
                let fed = b
                    .power(pair[0])
                    .is_some_and(|v| v.fluid > 0.0 && matches!(v.shown, Shown::Gives(_)));
                Ok(fed && mixer_water(b, m) > 0)
            },
        )
        .check("the structure gives as one fed, from the storage's solution", move |b, _| {
            let v = b.power(pair[0]).ok_or("no crystal")?;
            near("structure output, E/s", v.output, fed, 0.05)?;
            same("solution left the cells", store(b, cells[0])?.fluid(SOLUTION) < 4 * BUCKET, true)
        })
}

fn kitchen_takes_milk_from_the_storage(o: Pos, _: &Tuning) -> Scenario {
    use crate::machines::sheet::Press;
    let cells = [at(o, [1, 0, 1]), at(o, [2, 0, 1])];
    let p = at(o, [4, 0, -1]);
    Scenario::new("fluids20_kitchen_takes_milk_from_the_storage", plot_area(o))
        .then("a fluid line from a storage to a kitchen with cake's dry parts", move |b, _| {
            fluid_line(b, o, 4)?;
            for c in cells {
                b.place(c, CELL, &[])?;
            }
            b.place(p, "rustcraft:kitchen", &[])?;
            b.set_buffer(p, 400.0)?;
            b.put(p, 0, "minecraft:sugar", 2)?;
            b.put(p, 1, "minecraft:egg", 1)?;
            b.put(p, 2, "minecraft:wheat", 3)
        })
        .until("4 buckets of milk in the storage's cells", 40, storage_with(cells[0], MILK, 4 * BUCKET))
        // 0.21 (spec 39): cake, the fourth dish, turned on.
        .then("cake turned on", move |b, _| {
            b.press_cursor(p, Press::Toggle(3).id(), None).map(|_| ())
        })
        .until("a cake in its output", 400, move |b, _| {
            Ok(b.items(p).iter().skip(9).flatten().any(|s| s.0 == "minecraft:cake"))
        })
        .check("the milk came from the cells and no bucket is left", move |b, _| {
            same("milk left in the cells, mB", store(b, cells[0])?.fluid(MILK), BUCKET)?;
            same("buckets", count_of(&b.items(p), "minecraft:bucket"), 0)
        })
}
