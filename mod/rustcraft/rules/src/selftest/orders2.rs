//! Orders, second part (spec section 19 after slice 15).
//!
//! - An order whose crafter has no energy waits for energy and says so; its queue, its job and
//!   the crafter's temporary page survive a restart, nothing in the crafter is handed out twice,
//!   and the pistons come once the crafter gets energy.
//! - An order for sparksteel on an alloy smelter with neither fuel nor energy waits for energy;
//!   with coal in its fuel slot the sparksteel comes back to the storage.
//! - An order for nutrient solution runs on the mixer with water from a tank it touches; the
//!   solution goes into the tank on top (O9, O12).
//! - Without water on the line the order names the water it lacks (O12, O6).

use super::slice15::{
    COBBLE, CRAFTER, CRYSTAL, FILL, PISTON, PLANKS, REDSTONE, build, crafter_at, held, holds,
    machine, parts_for, put_book, terminal_at,
};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::orders::fluid_key;
use crate::orders::queue::Wait;
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [
    orders_survive_a_restart,
    alloy_order_waits_for_energy,
    mixer_order_makes_solution,
    mixer_order_names_missing_water,
];

const ALLOY: &str = "rustcraft:alloy";
const MIXER: &str = "rustcraft:mixer";
const TANK: &str = "rustcraft:tank";
const COPPERSTEEL: &str = "rustcraft:coppersteel";
const SPARKSTEEL: &str = "rustcraft:sparksteel";
const GOLD: &str = "minecraft:gold_ingot";
const COAL: &str = "minecraft:coal";
const BONE_MEAL: &str = "minecraft:bone_meal";
const WATER: &str = "minecraft:water";
const SOLUTION: &str = "rustcraft:solution";
/// The memo's mark of the order's id before the restart.
const ORDER_ID: &str = "order_id";
/// The alloy smelter's fuel slot (rustcraft_content::ALLOY_FUEL).
const ALLOY_FUEL: usize = 4;

/// The first order of the storage waits for `wait`.
fn first_waits(b: &dyn Bench, o: Pos, wait: &Wait) -> bool {
    b.orders(terminal_at(o))
        .first()
        .is_some_and(|q| q.wait == *wait)
}

/// Two crystals that touch the machine at `pos`, as slice 15's `machine` places them.
fn power(b: &mut dyn Bench, pos: Pos) -> Result<(), String> {
    b.place(at(pos, [0, 0, -1]), CRYSTAL, &[])?;
    b.place(at(pos, [0, -1, 0]), CRYSTAL, &[])?;
    b.refresh();
    Ok(())
}

fn orders_survive_a_restart(o: Pos, _: &Tuning) -> Scenario {
    let parts = parts_for(4, super::slice15::IRON);
    let p2 = parts.clone();
    Scenario::new("orders_survive_a_restart", plot_area(o))
        .then(
            "a storage on a tier 3 line, parts for 4 pistons",
            move |b, _| build(b, o, &parts),
        )
        .until("the storage holds the parts", FILL, move |b, _| {
            holds(b, o, &p2)
        })
        .then(
            "an autocrafter without energy on the line, the book",
            move |b, _| {
                machine(b, crafter_at(o), CRAFTER, false)?;
                put_book(b, o)
            },
        )
        .until(
            "the storage takes an order for 4 pistons",
            FILL,
            move |b, _| Ok(b.order(terminal_at(o), PISTON, 4).is_ok()),
        )
        .until(
            "the parts are in the crafter and the order waits for energy",
            200,
            move |b, memo| {
                let waits = first_waits(b, o, &Wait::Energy("crafter".into()));
                if let Some(q) = b.orders(terminal_at(o)).first() {
                    memo.mark(ORDER_ID, f64::from(q.id));
                }
                Ok(waits && count_of(&b.items(crafter_at(o)), COBBLE) == 16)
            },
        )
        .then("the server restarts", move |b, _| {
            b.restart_orders(terminal_at(o))
        })
        .wait(40)
        .check(
            "the order is back and nothing was handed out twice",
            move |b, memo| {
                let orders = b.orders(terminal_at(o));
                same("orders", orders.len(), 1)?;
                same(
                    "the order's id",
                    f64::from(orders[0].id),
                    memo.get(ORDER_ID),
                )?;
                same("cobblestone in the storage", held(b, o, COBBLE)?, 0)?;
                same(
                    "cobblestone in the crafter",
                    count_of(&b.items(crafter_at(o)), COBBLE),
                    16,
                )
            },
        )
        .then("the crafter gets energy", move |b, _| {
            power(b, crafter_at(o))
        })
        .until("4 pistons are back in the storage", 1200, move |b, _| {
            Ok(held(b, o, PISTON)? == 4)
        })
        .wait(40)
        .check("the order is done and made no more", move |b, _| {
            same("orders", b.orders(terminal_at(o)).len(), 0)?;
            same("pistons", held(b, o, PISTON)?, 4)?;
            same("planks in the storage", held(b, o, PLANKS)?, 0)?;
            same(
                "planks in the crafter",
                count_of(&b.items(crafter_at(o)), PLANKS),
                0,
            )
        })
}

fn alloy_order_waits_for_energy(o: Pos, _: &Tuning) -> Scenario {
    let parts: Vec<(&str, u32)> = vec![(COPPERSTEEL, 2), (GOLD, 1), (REDSTONE, 1)];
    let p2 = parts.clone();
    Scenario::new("alloy_order_waits_for_energy", plot_area(o))
        .then(
            "a storage on a tier 3 line with the parts of sparksteel",
            move |b, _| build(b, o, &parts),
        )
        .until("the storage holds the parts", FILL, move |b, _| {
            holds(b, o, &p2)
        })
        .then(
            "an alloy smelter with neither fuel nor energy, the book",
            move |b, _| {
                machine(b, crafter_at(o), ALLOY, false)?;
                put_book(b, o)
            },
        )
        .until(
            "the storage takes an order for 2 sparksteel",
            FILL,
            move |b, _| Ok(b.order(terminal_at(o), SPARKSTEEL, 2).is_ok()),
        )
        .until("the order waits for energy", 200, move |b, _| {
            Ok(first_waits(b, o, &Wait::Energy("alloy".into())))
        })
        .then("coal in the smelter's fuel slot", move |b, _| {
            b.put(crafter_at(o), ALLOY_FUEL, COAL, 4)
        })
        .until("2 sparksteel are back in the storage", 2400, move |b, _| {
            Ok(held(b, o, SPARKSTEEL)? == 2)
        })
        .check("the parts went into it", move |b, _| {
            same("coppersteel", held(b, o, COPPERSTEEL)?, 0)?;
            same("orders", b.orders(terminal_at(o)).len(), 0)
        })
}

/// A mixer with energy on the line, a tank on top of it and the book in the terminal; with
/// `water`, a tank of 8 buckets of water beside it. Placed once the storage holds the parts, so
/// the chest's joint gives them to the storage alone.
fn mixer_line(b: &mut dyn Bench, o: Pos, water: bool) -> Result<(), String> {
    let m = crafter_at(o);
    machine(b, m, MIXER, true)?;
    b.place(at(m, [0, 1, 0]), TANK, &[])?;
    if water {
        let w = at(m, [-1, 0, 0]);
        b.place(w, TANK, &[])?;
        b.set_tank(w, WATER, 8000)?;
    }
    put_book(b, o)
}

fn mixer_order_makes_solution(o: Pos, _: &Tuning) -> Scenario {
    // 0.20: the solution is bone meal and water; the redstone stays.
    let parts: Vec<(&str, u32)> = vec![(BONE_MEAL, 2), (REDSTONE, 2)];
    let p2 = parts.clone();
    let top = at(crafter_at(o), [0, 1, 0]);
    Scenario::new("mixer_order_makes_solution", plot_area(o))
        .then(
            "a storage on a tier 3 line with the parts of 2 buckets of solution",
            move |b, _| build(b, o, &parts),
        )
        .until("the storage holds the parts", FILL, move |b, _| {
            holds(b, o, &p2)
        })
        .then(
            "a mixer with energy, water beside it, a tank on top, the book",
            move |b, _| mixer_line(b, o, true),
        )
        .until(
            "the storage takes an order for a bucket of solution",
            FILL,
            move |b, _| Ok(b.order(terminal_at(o), &fluid_key(SOLUTION), 1).is_ok()),
        )
        .until(
            "a bucket of solution in the tank on top or the mixer",
            1200,
            move |b, _| {
                let in_tank = b
                    .tank(top)
                    .filter(|g| g.fluid.as_deref() == Some(SOLUTION))
                    .map_or(0, |g| g.amount);
                let in_mixer = b
                    .mixer(crafter_at(o))
                    .and_then(|v| v.product)
                    .map_or(0, |p| p.1);
                Ok(in_tank + in_mixer >= 1000)
            },
        )
        .wait(40)
        .check("one batch of parts went, the order is done", move |b, _| {
            same("bone meal in the storage", held(b, o, BONE_MEAL)?, 1)?;
            same("redstone in the storage", held(b, o, REDSTONE)?, 2)?;
            same("orders", b.orders(terminal_at(o)).len(), 0)
        })
}

fn mixer_order_names_missing_water(o: Pos, _: &Tuning) -> Scenario {
    let parts: Vec<(&str, u32)> = vec![(BONE_MEAL, 1), (REDSTONE, 1)];
    let p2 = parts.clone();
    Scenario::new("mixer_order_names_missing_water", plot_area(o))
        .then(
            "a storage on a tier 3 line with the parts of a bucket of solution",
            move |b, _| build(b, o, &parts),
        )
        .until("the storage holds the parts", FILL, move |b, _| {
            holds(b, o, &p2)
        })
        .then(
            "a mixer with energy and no water on the line, the book",
            move |b, _| mixer_line(b, o, false),
        )
        .until(
            "the storage takes an order for a bucket of solution",
            FILL,
            move |b, _| Ok(b.order(terminal_at(o), &fluid_key(SOLUTION), 1).is_ok()),
        )
        .until("the order names the water it lacks", 200, move |b, _| {
            Ok(first_waits(b, o, &Wait::Items(vec![(fluid_key(WATER), 1)])))
        })
        .check("the parts stay in the storage", move |b, _| {
            same("bone meal", held(b, o, BONE_MEAL)?, 1)?;
            same(
                "bone meal in the mixer",
                count_of(&b.items(crafter_at(o)), BONE_MEAL),
                0,
            )
        })
}
