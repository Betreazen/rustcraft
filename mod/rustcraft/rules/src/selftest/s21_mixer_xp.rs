//! 0.21.1 (owner 2026-11-20: «да сделай»): every bucket a mixer makes is its owner's Alchemy in
//! Path-of-craft-tree, the `mix` report of `docs/design/path-of-craft-tree-contract.md`.
//!
//! - A mixer with an owner, bone meal for two batches, a water tank beside it and a tank on top:
//!   it makes two buckets of solution, and Path-of-craft-tree is told `mix` for its owner once a
//!   bucket, two buckets in all, no more.

use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::machines::sheet::Press;
use crate::power as rules;
use crate::reports::{COMMAND, MIX};
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [mixer_reports_its_buckets];

const MIXER: &str = "rustcraft:mixer";
const TANK: &str = "rustcraft:tank";
const CRYSTAL: &str = "rustcraft:crystal";
const WATER: &str = "minecraft:water";
const BONE_MEAL: &str = "minecraft:bone_meal";
const REDSTONE: &str = "minecraft:redstone";
/// The owner the mixer reports for (nobody on the server).
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0b21;
const BATCHES: u32 = 2;
/// Two batches of 100 ticks, with room to spare.
const LIMIT: u64 = 600;

/// The buckets the `mix` reports told for [`OWNER`] so far, sent or not (without
/// Path-of-craft-tree a line is kept as `unsent …`).
fn reported(b: &dyn Bench) -> u32 {
    let head = format!("{COMMAND} {MIX} {OWNER:032x} ");
    b.reports()
        .iter()
        .filter_map(|r| r.trim_start_matches("unsent ").strip_prefix(&head)?.parse::<u32>().ok())
        .sum()
}

fn mixer_reports_its_buckets(o: Pos, _: &Tuning) -> Scenario {
    let m = at(o, [3, 1, 3]);
    let water = at(o, [2, 1, 3]);
    let product = at(o, [3, 2, 3]);
    Scenario::new("mixer21_reports_its_buckets", plot_area(o))
        .then("an owned mixer with two batches, water beside it, a tank on top", move |b, _| {
            b.place(m, MIXER, &[])?;
            b.set_owner(m, OWNER, 1)?;
            b.put(m, 0, BONE_MEAL, BATCHES)?;
            b.put(m, 1, REDSTONE, BATCHES)?;
            b.press(m, Press::Toggle(0).id(), None)?;
            b.place(water, TANK, &[])?;
            b.set_tank(water, WATER, 8000)?;
            b.place(product, TANK, &[])?;
            b.place(at(m, [0, 0, -1]), CRYSTAL, &[])?;
            b.place(at(m, [0, 0, 1]), CRYSTAL, &[])
        })
        .until("both batches mixed", LIMIT, move |b, _| Ok(count_of(&b.items(m), BONE_MEAL) == 0))
        // The reports go out on the next server ticks.
        .wait(20)
        .check("two buckets of solution made", move |b, _| {
            let tank = b.tank(product).filter(|g| g.fluid.as_deref() == Some(rules::SOLUTION));
            let inside = b.mixer(m).and_then(|v| v.product).map_or(0, |p| p.1);
            same("solution made, mB", tank.map_or(0, |g| g.amount) + inside, u64::from(BATCHES) * 1000)
        })
        .check("Path-of-craft-tree told mix once a bucket", move |b, _| {
            same("buckets reported", reported(b), BATCHES)
        })
}
