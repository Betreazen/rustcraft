//! Slice 18, stream 3: the machines as the handbook says.
//!
//! - U2: a higher tier's item on a placed machine replaces it in place; its items stay, the old
//!   tier's item comes back, and a tier that is not higher replaces nothing.
//! - The crusher takes the ore blocks too, two dusts each (guide `crusher`).
//! - A water bucket pours into a tank and an empty bucket takes it back; the solution stays.
//! - Two capacity modules give a furnace two queue cells behind its input; a stack module one
//!   cell of 256 (guide `mod_cap`, `mod_stack`).

use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 5] = [
    tier_swap_keeps_contents,
    crusher_takes_ore_blocks,
    bucket_fills_and_empties_a_tank,
    furnace_capacity_queue,
    furnace_stack_cell,
];

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

fn crusher_takes_ore_blocks(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("crusher_takes_ore_blocks", plot_area(o)).check(
        "every iron, copper and gold ore block crushes into two dusts",
        move |b, _| {
            for (ore, dust) in [
                ("minecraft:iron_ore", "rustcraft:dust_iron"),
                ("minecraft:deepslate_iron_ore", "rustcraft:dust_iron"),
                ("minecraft:copper_ore", "rustcraft:dust_copper"),
                ("minecraft:deepslate_copper_ore", "rustcraft:dust_copper"),
                ("minecraft:gold_ore", "rustcraft:dust_gold"),
                ("minecraft:deepslate_gold_ore", "rustcraft:dust_gold"),
            ] {
                let (got, count, _) = b.crush(ore).ok_or(format!("no crushing recipe for {ore}"))?;
                same(ore, (got.as_str(), count), (dust, 2))?;
            }
            Ok(())
        },
    )
}

fn bucket_fills_and_empties_a_tank(o: Pos, _: &Tuning) -> Scenario {
    let water = at(o, [2, 1, 0]);
    let solution = at(o, [6, 1, 0]);
    Scenario::new("bucket_fills_and_empties_a_tank", plot_area(o))
        .then("an empty tank and a tank of solution", move |b, _| {
            floor(b, o)?;
            b.place(water, "rustcraft:tank", &[])?;
            b.place(solution, "rustcraft:tank", &[])?;
            b.refresh();
            Ok(())
        })
        .until("the tanks are read", 40, move |b, _| Ok(b.tank(water).is_some()))
        .then("a water bucket pours in", move |b, _| {
            let left = b.use_bucket(water, "minecraft:water_bucket")?;
            same("left in the hand", left.as_str(), "minecraft:bucket")
        })
        .check("the tank holds a bucket of water", move |b, _| {
            let t = b.tank(water).ok_or("no tank")?;
            same("water", (t.fluid.as_deref(), t.amount), (Some("minecraft:water"), 1000))
        })
        .then("an empty bucket takes it back", move |b, _| {
            let left = b.use_bucket(water, "minecraft:bucket")?;
            same("left in the hand", left.as_str(), "minecraft:water_bucket")
        })
        .check("the tank is empty again", move |b, _| {
            same("amount", b.tank(water).ok_or("no tank")?.amount, 0)
        })
        .then("the solution tank gets its solution", move |b, _| {
            b.set_tank(solution, "rustcraft:solution", 4000)
        })
        .check("no bucket takes the solution", move |b, _| match b.use_bucket(solution, "minecraft:bucket") {
            Err(_) => Ok(()),
            Ok(left) => Err(format!("a bucket took the solution: {left}")),
        })
}

/// How many raw iron the input slot of the machine at `p` holds.
fn input_count(b: &dyn Bench, p: Pos) -> u32 {
    b.items(p).first().cloned().flatten().filter(|(i, _)| i == RAW_IRON).map_or(0, |(_, n)| n)
}

/// A furnace with no energy and `modules`, then `stacks` stacks of 64 raw iron put into its
/// input one after another, each taken into its queue; the next one stays in the input.
fn queue_scenario(o: Pos, name: &'static str, modules: &'static [&'static str], stacks: u32) -> Scenario {
    let p = at(o, [3, 1, 0]);
    let mut s = Scenario::new(name, plot_area(o)).then("a furnace with its modules", move |b, _| {
        floor(b, o)?;
        b.place(p, FURNACE, &[])?;
        b.set_modules(p, modules)?;
        b.refresh();
        Ok(())
    });
    for _ in 0..stacks {
        s = s
            .then("64 raw iron into its input", move |b, _| b.put(p, 0, RAW_IRON, 64))
            .until("the queue took them", 80, move |b, _| Ok(input_count(b, p) == 0));
    }
    s.then("one stack more", move |b, _| b.put(p, 0, RAW_IRON, 64))
        .wait(40)
        .check("the queue is full: it stays in the input", move |b, _| {
            same("raw iron in the input", input_count(b, p), 64)
        })
}

fn furnace_capacity_queue(o: Pos, _: &Tuning) -> Scenario {
    queue_scenario(
        o,
        "furnace_capacity_queue",
        &["rustcraft:module_capacity", "rustcraft:module_capacity"],
        4,
    )
}

fn furnace_stack_cell(o: Pos, _: &Tuning) -> Scenario {
    queue_scenario(o, "furnace_stack_cell", &["rustcraft:module_stack"], 4)
}
