//! Slice 18, energy stream: the crystal shard in the recipes, the druse breaking into shards
//! (E5), and growing crystals on a tank of solution (CR2).

use super::{Build, Scenario, at, plot_area, same};
use crate::grow::{self, DRUSE, SEED, SHARD, STAGE};
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [
    shard_recipes,
    druse_breaks_into_shards,
    seed_grows_on_solution,
    seed_stands_without_solution,
];

const TANK: &str = "rustcraft:tank";
const PICKAXE: &str = "minecraft:diamond_pickaxe";
const LIMIT: u64 = 200;

fn shards_in(drops: &[(String, u32)]) -> u32 {
    drops
        .iter()
        .filter(|(i, _)| i == SHARD)
        .map(|(_, n)| *n)
        .sum()
}

/// The guide's recipes take the mod's own shard (guide `crystal`, `battery`).
fn shard_recipes(o: crate::geom::Pos, _t: &Tuning) -> Scenario {
    Scenario::new("s18_shard_recipes", plot_area(o)).check(
        "the crystal is made of crystal shards round copper-steel, not of amethyst",
        move |b, _| {
            let s = Some(SHARD);
            let a = Some("rustcraft:coppersteel");
            let made = b.craft(&[s, s, s, s, a, s, s, s, s]);
            same(
                "crystal from shards",
                made.map(|(i, _)| i),
                Some("rustcraft:crystal".to_owned()),
            )?;
            let am = Some("minecraft:amethyst_shard");
            let old = b.craft(&[am, am, am, am, a, am, am, am, am]);
            same("crystal from amethyst", old.map(|(i, _)| i), None)?;
            same("the shard is an item", b.item_known(SHARD), true)
        },
    )
}

/// E5: a druse breaks into two to four shards.
fn druse_breaks_into_shards(o: crate::geom::Pos, _t: &Tuning) -> Scenario {
    let p = at(o, [2, 0, 2]);
    Scenario::new("s18_druse_breaks_into_shards", plot_area(o))
        .then("a druse on the floor", move |b, _| b.place(p, DRUSE, &[]))
        .check("broken with a pickaxe it gives 2–4 shards", move |b, _| {
            for round in 0..4 {
                if round > 0 {
                    b.place(p, DRUSE, &[])?;
                }
                let n = shards_in(&b.break_block(p, Some(PICKAXE))?);
                if !(2..=4).contains(&n) {
                    return Err(format!("{n} shards from a druse"));
                }
            }
            Ok(())
        })
}

/// CR2: a seed on a tank of solution drinks it and grows; grown, it breaks into 2–4 shards.
fn seed_grows_on_solution(o: crate::geom::Pos, t: &Tuning) -> Scenario {
    let tank = at(o, [2, 0, 2]);
    let seed = at(o, [2, 1, 2]);
    let span = 100;
    let drink = grow::drink_per_second(t) * span as f64 / t.ticks_per_second;
    let buckets = t.grow_buckets;
    Scenario::new("s18_seed_grows_on_solution", plot_area(o))
        .then("a tank of solution and a seed on it", move |b, _| {
            b.place(tank, TANK, &[])?;
            b.set_tank(tank, grow::SOLUTION, 8000)?;
            b.place(seed, SEED, &[])
        })
        .until("the seed takes solution from the tank", LIMIT, move |b, m| {
            let s = b.seed(seed).ok_or("no seed")?;
            m.mark("progress", s.progress);
            Ok(s.solution > 0.0 || s.progress > 0.0)
        })
        .wait(span)
        .check("it grows by what it drinks: 4 buckets in 10 minutes", move |b, m| {
            let s = b.seed(seed).ok_or("no seed")?;
            let grown = (s.progress - m.get("progress")) * buckets * 1000.0;
            if (grown - drink).abs() > drink * 0.25 + 1.0 {
                return Err(format!("drank {grown:.1} mB in {span} ticks, wanted {drink:.1}"));
            }
            same("stage", b.property(seed, STAGE), Some("0".to_owned()))
        })
        .then("nearly grown", move |b, _| b.set_seed(seed, 0.999))
        .until("grown, it shows the druse (stage 2)", LIMIT, move |b, _| {
            Ok(b.property(seed, STAGE).as_deref() == Some("2"))
        })
        .check("broken grown, it gives 2–4 shards", move |b, _| {
            let n = shards_in(&b.break_block(seed, Some(PICKAXE))?);
            if (2..=4).contains(&n) {
                Ok(())
            } else {
                Err(format!("{n} shards from a grown druse"))
            }
        })
}

/// CR2: on a tank of water the seed stands; broken, it gives its one shard back.
fn seed_stands_without_solution(o: crate::geom::Pos, _t: &Tuning) -> Scenario {
    let tank = at(o, [2, 0, 2]);
    let seed = at(o, [2, 1, 2]);
    Scenario::new("s18_seed_stands_without_solution", plot_area(o))
        .then("a tank of water and a seed on it", move |b, _| {
            b.place(tank, TANK, &[])?;
            b.set_tank(tank, "minecraft:water", 8000)?;
            b.place(seed, SEED, &[])
        })
        .wait(100)
        .check("it does not grow and drinks no water", move |b, _| {
            let s = b.seed(seed).ok_or("no seed")?;
            same("progress", s.progress, 0.0)?;
            same("held", s.solution, 0.0)?;
            same("tank", b.tank(tank).map(|g| g.amount), Some(8000))
        })
        .check("broken, it gives its one shard back", move |b, _| {
            same("shards", shards_in(&b.break_block(seed, None)?), 1)
        })
}
