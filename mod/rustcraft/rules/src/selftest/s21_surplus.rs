//! 0.21 (round 32, spec 32): the storage's surplus and the wrench on the trash.
//!
//! - A storage with limits, a disintegrator and a trash on one line: iron pickaxes over their
//!   limit are taken apart, the ingots and sticks they give past their own limits burn (SP1–SP3).
//! - A limit lowered under what the storage holds pushes the rest into the trash (SP1).
//! - With no disintegrator and no trash nothing changes: the stock over the limit stays, what
//!   arrives over it waits in the chest (S5).
//! - A trash with no filter takes only the surplus: the storage keeps the dirt it does not limit
//!   (SP4), and the trash does not say «Фильтр не задан» once a storage limits something.
//! - Water in the storage's cells over its limit burns in the trash of its fluid line (SP8).
//! - The wrench opens the joint of the housing beside the trash and the disintegrator instead of
//!   turning them; a trash with no housing beside it turns (SP9).

use super::craft18::status_key;
use super::magistral::line;
use super::slice15::{CELL, TERMINAL, terminal_at};
use super::transport::{chest, fill, housing, joint};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same, total};
use crate::geom::{Face, Pos};
use crate::net::fluids::BUCKET;
use crate::net::items::Mode;
use crate::orders::fluid_key;
use crate::storage::terminal::LimitChange;
use crate::tuning::Tuning;

pub const ALL: [Build; 6] = [
    gear_taken_apart_ingots_burnt,
    lowered_limit_pushes_the_stock_out,
    no_sink_changes_nothing,
    trash_without_a_filter_takes_only_surplus,
    water_over_the_limit_burns,
    wrench_opens_the_trash_joint,
];

const DISINTEGRATOR: &str = "rustcraft:disintegrator";
const TRASH: &str = "rustcraft:trash";
const WRENCH: &str = "rustcraft:wrench";
const PICK: &str = "minecraft:iron_pickaxe";
const INGOT: &str = "minecraft:iron_ingot";
const STICK: &str = "minecraft:stick";
const COBBLE: &str = "minecraft:cobblestone";
const DIRT: &str = "minecraft:dirt";
const WATER: &str = "minecraft:water";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0c21;
const FULL: f64 = 1000.0;
const NO_FILTER: &str = "rustcraft.util.trash_no_filter";

/// The disintegrator beside housing 3, the trash beside housing 6 (z−, as the chest at 1).
fn dis_at(o: Pos) -> Pos {
    chest(o, 3)
}

fn trash_at(o: Pos) -> Pos {
    chest(o, 6)
}

/// A machine with an owner and a full buffer.
fn machine(b: &mut dyn Bench, pos: Pos, block: &str) -> Result<(), String> {
    b.place(pos, block, &[])?;
    b.set_owner(pos, OWNER, 1)?;
    b.set_buffer(pos, FULL)
}

/// The trash's joint as placing it beside a housing writes it (an empty allow list at the lowest
/// priority) on the item and fluid channels of housing `k`.
fn trash_joint(b: &mut dyn Bench, o: Pos, k: i32) -> Result<(), String> {
    for slot in [0, 1] {
        b.set_joint(housing(o, k), Face::ZNeg, slot, crate::machines::tiered::trash_joint())?;
    }
    Ok(())
}

/// A node and six housings of tier `tier`, a chest «из блока» at housing 1, a storage of four
/// cells with its terminal at housings 3 to 5, and as asked a disintegrator at housing 3 and a
/// trash at housing 6.
fn build(b: &mut dyn Bench, o: Pos, tier: u8, dis: bool, trash: bool) -> Result<(), String> {
    line(b, o, tier, 6, tier)?;
    b.place(chest(o, 1), "minecraft:chest", &[])?;
    joint(b, o, 1, Mode::From, 0)?;
    for c in [[3, 0, 1], [4, 0, 1], [3, 0, 2], [4, 0, 2]] {
        b.place(at(o, c), CELL, &[])?;
    }
    b.place(terminal_at(o), TERMINAL, &[])?;
    if dis {
        machine(b, dis_at(o), DISINTEGRATOR)?;
    }
    if trash {
        machine(b, trash_at(o), TRASH)?;
        trash_joint(b, o, 6)?;
    }
    Ok(())
}

/// The limit on `item` at the storage of the terminal `t` set to `n` (0 too).
fn limit(b: &mut dyn Bench, t: Pos, item: &str, n: i64) -> Result<(), String> {
    if n == 0 {
        b.change_limit(t, item, LimitChange::Add(1))?;
        return b.change_limit(t, item, LimitChange::Add(-1));
    }
    b.change_limit(t, item, LimitChange::Add(n))
}

fn held(b: &dyn Bench, t: Pos, item: &str) -> Result<u64, String> {
    Ok(b.storage(t).ok_or("the terminal shows no storage")?.store.count(item))
}

fn gear_taken_apart_ingots_burnt(o: Pos, _: &Tuning) -> Scenario {
    // The owner: «если я выставляю лимиты на какие-то предметы, чтобы они в первую очередь
    // автоматически шли в дезинтегратор, если он есть, а потом в утилизатор». Two iron pickaxes
    // give 3 ingots and 2 sticks (DS1): the storage keeps one ingot, the rest burns. Had the
    // pickaxes burnt whole, it would keep none.
    let (c, dis, trash, t) = (chest(o, 1), dis_at(o), trash_at(o), terminal_at(o));
    Scenario::new("surplus21_gear_taken_apart_ingots_burnt", plot_area(o))
        .then("a chest, a storage, a disintegrator and a trash on one line", move |b, _| {
            build(b, o, 2, true, true)
        })
        .until("the storage is found", 200, move |b, _| Ok(b.storage(t).is_some()))
        .then("limits: no iron pickaxes, one iron ingot, no sticks", move |b, _| {
            limit(b, t, PICK, 0)?;
            limit(b, t, INGOT, 1)?;
            limit(b, t, STICK, 0)
        })
        .then("two iron pickaxes into the chest", move |b, _| {
            for _ in 0..2 {
                same("a pickaxe in", b.insert(c, PICK, 1)?, 1)?;
            }
            Ok(())
        })
        .until("both taken apart: one ingot kept, the other ingots and the sticks burnt", 2400, move |b, _| {
            Ok(total(&b.items(c)) == 0
                && total(&b.items(dis)) == 0
                && total(&b.items(trash)) == 0
                && held(b, t, PICK)? == 0
                && held(b, t, INGOT)? == 1
                && held(b, t, STICK)? == 0)
        })
}

fn lowered_limit_pushes_the_stock_out(o: Pos, _: &Tuning) -> Scenario {
    let (c, trash, t) = (chest(o, 1), trash_at(o), terminal_at(o));
    Scenario::new("surplus21_lowered_limit_pushes_the_stock_out", plot_area(o))
        .then("a chest of 20 cobblestone, a storage and a trash with no filter", move |b, _| {
            build(b, o, 2, false, true)?;
            fill(b, c, COBBLE, 20)
        })
        .until("the storage holds the 20", 400, move |b, _| Ok(held(b, t, COBBLE)? == 20))
        .until("with nothing limited the trash says it has no filter", 100, move |b, _| {
            Ok(status_key(b, trash) == NO_FILTER)
        })
        .then("the limit set to 5", move |b, _| limit(b, t, COBBLE, 5))
        .until("the 15 over it burn", 600, move |b, _| {
            Ok(held(b, t, COBBLE)? == 5 && total(&b.items(trash)) == 0)
        })
        .until("the trash no longer says it has no filter", 100, move |b, _| {
            Ok(status_key(b, trash) != NO_FILTER)
        })
}

fn no_sink_changes_nothing(o: Pos, _: &Tuning) -> Scenario {
    let (c, t) = (chest(o, 1), terminal_at(o));
    Scenario::new("surplus21_no_sink_changes_nothing", plot_area(o))
        .then("a chest of 20 cobblestone and a storage, no disintegrator, no trash", move |b, _| {
            build(b, o, 2, false, false)?;
            fill(b, c, COBBLE, 20)
        })
        .until("the storage holds the 20", 400, move |b, _| Ok(held(b, t, COBBLE)? == 20))
        .then("the limit set to 5, 10 more in the chest", move |b, _| {
            limit(b, t, COBBLE, 5)?;
            fill(b, c, COBBLE, 10)
        })
        .wait(100)
        .check("the storage keeps all 20, the 10 wait in the chest", move |b, _| {
            same("stored", held(b, t, COBBLE)?, 20)?;
            same("in the chest", count_of(&b.items(c), COBBLE), 10)
        })
}

fn trash_without_a_filter_takes_only_surplus(o: Pos, _: &Tuning) -> Scenario {
    let (c, trash, t) = (chest(o, 1), trash_at(o), terminal_at(o));
    Scenario::new("surplus21_trash_without_a_filter_takes_only_surplus", plot_area(o))
        .then("a storage and a trash with no filter", move |b, _| build(b, o, 2, false, true))
        .until("the storage is found", 200, move |b, _| Ok(b.storage(t).is_some()))
        .then("a limit of 4 cobblestone; 10 cobblestone and 5 dirt into the chest", move |b, _| {
            limit(b, t, COBBLE, 4)?;
            fill(b, c, COBBLE, 10)?;
            fill(b, c, DIRT, 5)
        })
        .until("the chest is empty, the trash too", 600, move |b, _| {
            Ok(total(&b.items(c)) == 0 && total(&b.items(trash)) == 0)
        })
        .check("the storage keeps 4 cobblestone and all the dirt", move |b, _| {
            same("cobblestone", held(b, t, COBBLE)?, 4)?;
            same("dirt", held(b, t, DIRT)?, 5)
        })
        .check("the trash does not say it has no filter", move |b, _| {
            let key = status_key(b, trash);
            if key == NO_FILTER { Err(format!("it says {key}")) } else { Ok(()) }
        })
}

fn water_over_the_limit_burns(o: Pos, _: &Tuning) -> Scenario {
    // SP8: 5 buckets in the cells, the limit lowered to 2: 3 burn in the trash of the fluid line.
    let (trash, t) = (trash_at(o), terminal_at(o));
    Scenario::new("surplus21_water_over_the_limit_burns", plot_area(o))
        .then("a storage and a trash with no filter on a tier-1 line", move |b, _| {
            build(b, o, 1, false, true)
        })
        .until("the storage is found, 5 buckets of water in its cells", 200, move |b, _| {
            if b.storage(t).is_none() {
                return Ok(false);
            }
            b.set_storage_fluid(t, WATER, 5 * BUCKET)?;
            Ok(true)
        })
        .then("the water's limit set to 2 buckets", move |b, _| {
            b.change_limit(t, &fluid_key(WATER), LimitChange::Add(2))
        })
        .until("the cells hold 2 buckets", 600, move |b, _| {
            Ok(b.storage(t).ok_or("no storage")?.store.fluid(WATER) == 2 * BUCKET)
        })
        .wait(40)
        .check("no more burns", move |b, _| {
            same("water", b.storage(t).ok_or("no storage")?.store.fluid(WATER), 2 * BUCKET)?;
            same("the trash's items", total(&b.items(trash)), 0)
        })
}

fn wrench_opens_the_trash_joint(o: Pos, _: &Tuning) -> Scenario {
    // SP9, the owner: «писало что на стыке настроить нужно, но его меню вообще не открывалось».
    let (trash, dis) = (chest(o, 1), chest(o, 3));
    let lone = chest(o, 8);
    Scenario::new("surplus21_wrench_opens_the_trash_joint", plot_area(o))
        .then("a trash and a disintegrator facing north beside housings, a lone trash", move |b, _| {
            line(b, o, 1, 3, 1)?;
            for (p, block) in [(trash, TRASH), (dis, DISINTEGRATOR), (lone, TRASH)] {
                b.place(p, block, &[("facing", "north")])?;
            }
            Ok(())
        })
        .then("a wrench click on each one's front", move |b, _| {
            for p in [trash, dis, lone] {
                b.click_block(p, Face::ZNeg, WRENCH, false)?;
            }
            Ok(())
        })
        .check("the joint windows of their housings were asked for, on the face toward them", move |b, _| {
            same("the trash's housing", b.joint_asked(housing(o, 1)), Some(Face::ZNeg))?;
            same("the disintegrator's housing", b.joint_asked(housing(o, 3)), Some(Face::ZNeg))
        })
        .check("those two still face north; the lone trash turned east", move |b, _| {
            same("the trash", b.property(trash, "facing").as_deref(), Some("north"))?;
            same("the disintegrator", b.property(dis, "facing").as_deref(), Some("north"))?;
            same("the lone trash", b.property(lone, "facing").as_deref(), Some("east"))
        })
}
