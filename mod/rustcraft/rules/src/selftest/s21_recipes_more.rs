//! 0.21.2 (spec 50; the owner: «добавь рецепт кости, его можно получить разобрав костный блок,
//! добавь в целом больше рецептов», then «ой я ошибся, костная мука получается»): more recipes
//! for orders.
//!
//! - An order for bone meal at a storage with only bone blocks and an autocrafter on the line
//!   finishes: two bone blocks make 18 bone meal.
//! - An order for bones, which no recipe made before (OT8 refused it), finishes on a crusher: two
//!   bone blocks give 6 bones (MR1).
//! - The storage feeds an idle crusher its ore, not its cobblestone (MR3): the raw iron turns to
//!   dust, the cobblestone stays in the storage.

use super::slice15::{CRAFTER, DUST_IRON, FILL, RAW_IRON, build, crafter_at, crusher_on, held, holds, machine, put_book, terminal_at};
use super::{Bench, Build, Scenario, plot_area, same};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 3] = [
    bone_meal_from_bone_blocks,
    bones_from_bone_blocks_on_a_crusher,
    storage_keeps_its_cobblestone_from_the_crusher,
];

const BONE_BLOCK: &str = "minecraft:bone_block";
const BONE: &str = "minecraft:bone";
const BONE_MEAL: &str = "minecraft:bone_meal";
const COBBLE: &str = "minecraft:cobblestone";
/// Two runs on a machine, the shipment there and the product back.
const ORDER_LIMIT: u64 = 1600;

/// An order for `count` of `item` at the storage from `o`, taken once the book is in.
fn ordered(b: &mut dyn Bench, o: Pos, item: &str, count: u32) -> Result<bool, String> {
    Ok(b.order(terminal_at(o), item, count).is_ok())
}

fn bone_meal_from_bone_blocks(o: Pos, _: &Tuning) -> Scenario {
    let parts = [(BONE_BLOCK, 3)];
    Scenario::new("recipes_more21_bone_meal_from_bone_blocks", plot_area(o))
        .then("a storage on a tier 3 line with 3 bone blocks", move |b, _| build(b, o, &parts))
        .until("the storage holds them", FILL, move |b, _| holds(b, o, &parts))
        .then("an autocrafter with energy on the line, the book", move |b, _| {
            machine(b, crafter_at(o), CRAFTER, true)?;
            put_book(b, o)
        })
        .until("the storage takes an order for 18 bone meal", FILL, move |b, _| ordered(b, o, BONE_MEAL, 18))
        .until("18 bone meal in the storage", ORDER_LIMIT, move |b, _| Ok(held(b, o, BONE_MEAL)? == 18))
        .wait(20)
        .check("two bone blocks went into it, the order is done", move |b, _| {
            same("bone blocks", held(b, o, BONE_BLOCK)?, 1)?;
            same("orders", b.orders(terminal_at(o)).len(), 0)
        })
}

fn bones_from_bone_blocks_on_a_crusher(o: Pos, _: &Tuning) -> Scenario {
    let parts = [(BONE_BLOCK, 3)];
    Scenario::new("recipes_more21_bones_from_bone_blocks_on_a_crusher", plot_area(o))
        .then("a storage on a tier 3 line with 3 bone blocks", move |b, _| build(b, o, &parts))
        .until("the storage holds them", FILL, move |b, _| holds(b, o, &parts))
        .then("a crusher with energy on housing 2, the book", move |b, _| {
            crusher_on(b, o)?;
            put_book(b, o)
        })
        .wait(100)
        .check("an idle crusher gets no bone block by itself", move |b, _| same("bone blocks", held(b, o, BONE_BLOCK)?, 3))
        .until("the storage takes an order for 6 bones", FILL, move |b, _| ordered(b, o, BONE, 6))
        .until("6 bones in the storage", ORDER_LIMIT, move |b, _| Ok(held(b, o, BONE)? == 6))
        .wait(20)
        .check("two bone blocks went into it, the order is done", move |b, _| {
            same("bone blocks", held(b, o, BONE_BLOCK)?, 1)?;
            same("orders", b.orders(terminal_at(o)).len(), 0)
        })
}

fn storage_keeps_its_cobblestone_from_the_crusher(o: Pos, _: &Tuning) -> Scenario {
    let parts = [(RAW_IRON, 4), (COBBLE, 16)];
    Scenario::new("recipes_more21_storage_keeps_its_cobblestone_from_the_crusher", plot_area(o))
        .then("a storage on a line with 4 raw iron and 16 cobblestone", move |b, _| build(b, o, &parts))
        .until("the storage holds them", FILL, move |b, _| holds(b, o, &parts))
        .then("a crusher with energy on the line, no order", move |b, _| crusher_on(b, o))
        .until("the ore is dust in the storage, two an ore", 3600, move |b, _| {
            Ok(held(b, o, RAW_IRON)? == 0 && held(b, o, DUST_IRON)? == 8)
        })
        .wait(100)
        .check("the cobblestone is all in the storage, no gravel made", move |b, _| {
            same("cobblestone", held(b, o, COBBLE)?, 16)?;
            same("gravel", held(b, o, "minecraft:gravel")?, 0)
        })
}
