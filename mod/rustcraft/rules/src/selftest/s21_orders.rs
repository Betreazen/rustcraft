//! 0.21 (round 32, spec 36): orders without a deadlock. The owner's storage in small: copper,
//! iron, gold, redstone, diamonds and shards, no coppersteel; two alloy smelters and two
//! autocrafters on the line; three orders whose chains all go through coppersteel and the
//! smelters at once (a battery, a guard module and crystallite).
//!
//! On 0.20 the line pushed gold into the smelters after their sparksteel runs: «none of the job's
//! items left» never came, the jobs held both smelters for good and every other order waited
//! «нет свободной плавильни» (22 minutes on the owner's server, until he cancelled the harpoon). Here a
//! gold ingot goes into a smelter beside its job's items the same way: the job still ends once
//! less than a run is left (OD2), the stray gold goes back to the storage, and all three orders
//! finish.

use super::magistral::place_housing;
use super::slice15::{CRAFTER, FILL, build, held, holds, machine, put_book, terminal_at};
use super::transport::chest;
use super::{Bench, Build, Scenario, at, count_of, plot_area, same, total};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [concurrent_orders_share_coppersteel_and_the_smelters];

const ALLOY: &str = "rustcraft:alloy";
const COPPER: &str = "minecraft:copper_ingot";
const IRON: &str = "minecraft:iron_ingot";
const GOLD: &str = "minecraft:gold_ingot";
const REDSTONE: &str = "minecraft:redstone";
const DIAMOND: &str = "minecraft:diamond";
const SHARD: &str = "rustcraft:crystal_shard";
const COPPERSTEEL: &str = "rustcraft:coppersteel";
const SPARKSTEEL: &str = "rustcraft:sparksteel";
const CRYSTALLITE: &str = "rustcraft:crystallite";
const BATTERY: &str = "rustcraft:battery";
const GUARD: &str = "rustcraft:module_guard";

/// What the storage starts with. The three orders take copper 5, iron 9, gold 5, redstone 9,
/// diamonds 3 and shards 8 (battery: 4 sparksteel, 4 redstone, a shard; guard module: 4
/// crystallite, 4 iron, a shard; crystallite ×2: a run of 2 sparksteel, a diamond, 2 shards; each
/// 2 sparksteel a run of 2 coppersteel, a gold and a redstone; each 2 coppersteel a copper and an
/// iron).
const PARTS: [(&str, u32); 6] = [
    (COPPER, 12),
    (IRON, 16),
    (GOLD, 12),
    (REDSTONE, 16),
    (DIAMOND, 6),
    (SHARD, 12),
];
/// What the storage holds when the orders are done: the parts less what they took, and the stray
/// gold back.
const AFTER: [(&str, u64); 11] = [
    (COPPER, 7),
    (IRON, 7),
    (GOLD, 8),
    (REDSTONE, 7),
    (DIAMOND, 3),
    (SHARD, 4),
    (COPPERSTEEL, 0),
    (SPARKSTEEL, 0),
    (CRYSTALLITE, 2),
    (BATTERY, 1),
    (GUARD, 1),
];

/// The alloy smelters beside housings 3 and 4, the autocrafters beside 5 and 6.
fn alloys(o: Pos) -> [Pos; 2] {
    [chest(o, 3), chest(o, 4)]
}

fn crafters(o: Pos) -> [Pos; 2] {
    [chest(o, 5), chest(o, 6)]
}

/// The smelter that holds a sparksteel job's coppersteel and redstone, all of it arrived.
fn smelter_running_sparksteel(b: &dyn Bench, o: Pos) -> Option<Pos> {
    alloys(o).into_iter().find(|&a| {
        let slots = b.items(a);
        count_of(&slots, COPPERSTEEL) > 0 && count_of(&slots, REDSTONE) > 0 && count_of(&slots, GOLD) > 0
    })
}

/// A gold ingot into an empty input slot of the smelter at `pos`, as the 0.20 line put it.
fn stray_gold(b: &mut dyn Bench, pos: Pos) -> Result<(), String> {
    let slots = b.items(pos);
    let free = b
        .inputs(pos)
        .into_iter()
        .find(|&k| slots.get(k).is_none_or(Option::is_none))
        .ok_or("the smelter has no empty input slot")?;
    b.put(pos, free, GOLD, 1)
}

fn concurrent_orders_share_coppersteel_and_the_smelters(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("orders21_concurrent_orders_share_coppersteel_and_the_smelters", plot_area(o))
        .then("a storage on a tier 3 line, a chest of copper, iron, gold, redstone, diamonds, shards", move |b, _| {
            build(b, o, &PARTS)?;
            place_housing(b, at(o, [6, 0, 0]), 2)
        })
        .until("the storage holds them, no coppersteel", 2 * FILL, move |b, _| holds(b, o, &PARTS))
        .then("two alloy smelters and two autocrafters with energy on the line, the book", move |b, _| {
            for a in alloys(o) {
                machine(b, a, ALLOY, true)?;
            }
            for c in crafters(o) {
                machine(b, c, CRAFTER, true)?;
            }
            b.refresh();
            put_book(b, o)
        })
        .until("the storage takes a battery, a guard module and 2 crystallite", FILL, move |b, _| {
            let t = terminal_at(o);
            let n = b.orders(t).len();
            let next = [(BATTERY, 1), (GUARD, 1), (CRYSTALLITE, 2)].get(n).copied();
            match next {
                Some((item, count)) => {
                    let _ = b.order(t, item, count);
                    Ok(false)
                }
                None => Ok(true),
            }
        })
        .until("a smelter runs sparksteel: a stray gold goes in beside the job's items", 2400, move |b, _| {
            let Some(a) = smelter_running_sparksteel(b, o) else {
                return Ok(false);
            };
            stray_gold(b, a)?;
            Ok(true)
        })
        .until("all three orders are done", 6000, move |b, _| Ok(b.orders(terminal_at(o)).is_empty()))
        .until("the stray gold is back in the storage", 400, move |b, _| Ok(held(b, o, GOLD)? == 8))
        .check("the storage holds what the orders made and what they left", move |b, _| {
            for (item, n) in AFTER {
                same(item, held(b, o, item)?, n)?;
            }
            Ok(())
        })
        .check("the smelters' and the autocrafters' inputs hold nothing", move |b, _| {
            for m in alloys(o).into_iter().chain(crafters(o)) {
                let slots = b.items(m);
                let inputs: Vec<_> = b.inputs(m).into_iter().map(|k| slots.get(k).cloned().flatten()).collect();
                same(&format!("items left in the inputs at {m:?}"), total(&inputs), 0)?;
            }
            Ok(())
        })
}
