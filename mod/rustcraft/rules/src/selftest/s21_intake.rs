//! 0.21 (round 32; the owner: «теперь все машины знают, что они должны забирать?»): the machines
//! take from the magistral only what they work on. Each scenario puts the storage's traffic on a
//! line past a machine: a chest «из блока» beside the line gives it to the storage
//! (`slice15::build`), and the machine on the line keeps none of what it would not use.
//!
//! - The alloy smelter and the mixer take none of the gold, redstone, diamonds, shards, dust,
//!   bone meal, sugar and glowstone that ride to the storage; a joint's allow list feeds the smelter,
//!   the recipes turned on feed the mixer (0.21, spec 39).
//! - A kitchen running an order's bread takes no more wheat than the order brought.
//! - A feeder by two cows takes wheat, never the carrots and seeds that ride to the storage.
//! - An apiary collector set to combs takes no glass bottles; set to honey it does.
//! - A brewing vat with no page takes neither potions nor blaze powder; with a page, powder.
//! - A living vein without the stratum module takes cobblestone, never sand or gravel.
//! - An enchanting machine with nothing to enchant takes no lapis; with a book it does. With two
//!   books it enchants one a step.
//! - A portal dock sends none of the storage's traffic away; a joint's allow list feeds it.

use super::slice15::{FILL, build, crafter_at, held, put_book, terminal_at};
use super::transport::{chest, fill, housing};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same, total};
use crate::geom::{Face, Pos};
use crate::machines::sheet::Press;
use crate::net::items::{Filter, Mode};
use crate::net::settings::JointSetting;
use crate::tuning::Tuning;

pub const ALL: [Build; 9] = [
    alloy_and_mixer_leave_the_storage_traffic,
    kitchen_leaves_an_orders_dish_to_its_order,
    feeder_takes_the_food_of_its_animals,
    apiary_takes_bottles_for_honey_only,
    brewer_without_a_page_takes_nothing,
    vein_takes_the_rock_it_grows_from,
    enchanter_takes_lapis_for_something_to_enchant,
    enchanter_enchants_one_book_a_step,
    portal_dock_sends_only_what_its_joint_names,
];

const ALLOY: &str = "rustcraft:alloy";
const MIXER: &str = "rustcraft:mixer";
const GOLD: &str = "minecraft:gold_ingot";
const REDSTONE: &str = "minecraft:redstone";
const DIAMOND: &str = "minecraft:diamond";
const SHARD: &str = "rustcraft:crystal_shard";
const BONE_MEAL: &str = "minecraft:bone_meal";
const SUGAR: &str = "minecraft:sugar";
const GLOWSTONE: &str = "minecraft:glowstone_dust";
const DUST_COPPER: &str = "rustcraft:dust_copper";
const WHEAT: &str = "minecraft:wheat";
const CARROT: &str = "minecraft:carrot";
const SEEDS: &str = "minecraft:wheat_seeds";
const BOTTLE: &str = "minecraft:glass_bottle";
const POWDER: &str = "minecraft:blaze_powder";
const POTION: &str = "minecraft:potion";
const WATER_BOTTLE: &str = r#"{id:"minecraft:potion",count:1,components:{"minecraft:potion_contents":{potion:"minecraft:water"}}}"#;
const COBBLE: &str = "minecraft:cobblestone";
const SAND: &str = "minecraft:sand";
const GRAVEL: &str = "minecraft:gravel";
const LAPIS: &str = "minecraft:lapis_lazuli";
const BOOK: &str = "minecraft:book";
const STONE: &str = "minecraft:stone";
/// How long the storage may take to take in what the chest gives it, here a few kinds.
const TAKE: u64 = 2 * FILL;

/// The machine spot beside housing `k` of the line `build` makes, on the chest's side (from 3 on,
/// no chest beside it).
fn beside(o: Pos, k: i32) -> Pos {
    chest(o, k)
}

/// The joint of housing `k` toward its machine set «в машину» with an allow list of `items`.
fn feed_joint(b: &mut dyn Bench, o: Pos, k: i32, items: &[&str]) -> Result<(), String> {
    let mut setting = JointSetting::of_mode(Mode::Into);
    setting.filter = Filter { allow_only: true, items: items.iter().map(|i| (*i).to_owned()).collect() };
    b.set_joint(housing(o, k), Face::ZNeg, 0, setting)
}

/// The chest beside housing 1 is empty: the line took all of it.
fn chest_empty(b: &dyn Bench, o: Pos) -> bool {
    total(&b.items(chest(o, 1))) == 0
}

/// None of `items` in the slots of the block at `pos`.
fn keeps_none(b: &dyn Bench, pos: Pos, what: &str, items: &[&str]) -> Result<(), String> {
    let slots = b.items(pos);
    for item in items {
        same(&format!("{item} in the {what}"), count_of(&slots, item), 0)?;
    }
    Ok(())
}

fn alloy_and_mixer_leave_the_storage_traffic(o: Pos, _: &Tuning) -> Scenario {
    // The owner: machines took what rode to the storage and sat on it, so the orders stalled. The
    // smelter and the mixer choose no recipe and took every ingredient of every recipe.
    let (alloy, mixer) = (beside(o, 3), beside(o, 4));
    let parts: [(&str, u32); 8] = [
        (GOLD, 8),
        (REDSTONE, 8),
        (DIAMOND, 2),
        (SHARD, 4),
        (BONE_MEAL, 8),
        (SUGAR, 4),
        (GLOWSTONE, 4),
        (DUST_COPPER, 4),
    ];
    let items: Vec<&str> = parts.iter().map(|(i, _)| *i).collect();
    Scenario::new("intake21_alloy_and_mixer_leave_the_storage_traffic", plot_area(o))
        .then("a storage line with an alloy smelter and a mixer, a chest of their ingredients", move |b, _| {
            build(b, o, &[])?;
            b.place(alloy, ALLOY, &[])?;
            b.place(mixer, MIXER, &[])?;
            for (item, n) in parts {
                fill(b, chest(o, 1), item, n)?;
            }
            Ok(())
        })
        .until("the storage holds all of them", TAKE, move |b, _| {
            for (item, n) in parts {
                if held(b, o, item)? != u64::from(n) {
                    return Ok(false);
                }
            }
            Ok(true)
        })
        .check("the smelter and the mixer took none", move |b, _| {
            same("items in the smelter", total(&b.items(alloy)), 0)?;
            same("items in the mixer", total(&b.items(mixer)), 0)?;
            keeps_none(b, alloy, "smelter", &items)
        })
        // 0.21 (spec 39, RT5): the mixer's recipes on, not a joint's list, choose what it takes.
        .then("the smelter's joint names gold, the mixer's solution is on; more of all in the chest", move |b, _| {
            feed_joint(b, o, 3, &[GOLD])?;
            b.press(mixer, Press::Toggle(0).id(), None)?;
            fill(b, chest(o, 1), GOLD, 4)?;
            fill(b, chest(o, 1), BONE_MEAL, 4)?;
            fill(b, chest(o, 1), REDSTONE, 4)?;
            fill(b, chest(o, 1), SUGAR, 4)
        })
        .until("the smelter takes gold and the mixer bone meal", TAKE, move |b, _| {
            Ok(count_of(&b.items(alloy), GOLD) > 0 && count_of(&b.items(mixer), BONE_MEAL) > 0)
        })
        .until("the redstone and the sugar reach the storage", TAKE, move |b, _| {
            Ok(held(b, o, REDSTONE)? == 12 && held(b, o, SUGAR)? == 8)
        })
        .check("the smelter took no redstone, the mixer nothing of the accelerator", move |b, _| {
            keeps_none(b, alloy, "smelter", &[REDSTONE])?;
            keeps_none(b, mixer, "mixer", &[REDSTONE, SUGAR, GLOWSTONE])
        })
}

fn kitchen_leaves_an_orders_dish_to_its_order(o: Pos, _: &Tuning) -> Scenario {
    // 0.18.5 let every kitchen take the ingredients of a dish any order cooked; the order brings
    // its own, and a kitchen whose inputs hold its items is busy.
    let kitchen = crafter_at(o);
    Scenario::new("intake21_kitchen_leaves_an_orders_dish_to_its_order", plot_area(o))
        .then("a storage on a tier 3 line with 3 wheat", move |b, _| build(b, o, &[(WHEAT, 3)]))
        .until("the storage holds the wheat", FILL, move |b, _| Ok(held(b, o, WHEAT)? == 3))
        .then("a kitchen with no page and no energy, the book", move |b, _| {
            b.place(kitchen, "rustcraft:kitchen", &[])?;
            put_book(b, o)
        })
        .until("the storage takes an order for bread", FILL, move |b, _| {
            Ok(b.order(terminal_at(o), "minecraft:bread", 1).is_ok())
        })
        .until("the order's 3 wheat are in the kitchen", 600, move |b, _| {
            Ok(count_of(&b.items(kitchen), WHEAT) == 3)
        })
        .then("6 more wheat in the chest", move |b, _| fill(b, chest(o, 1), WHEAT, 6))
        .until("they reach the storage", TAKE, move |b, _| Ok(held(b, o, WHEAT)? == 6))
        .check("the kitchen still holds the order's 3", move |b, _| {
            same("wheat in the kitchen", count_of(&b.items(kitchen), WHEAT), 3)
        })
}

fn feeder_takes_the_food_of_its_animals(o: Pos, _: &Tuning) -> Scenario {
    let feeder = beside(o, 3);
    Scenario::new("intake21_feeder_takes_the_food_of_its_animals", plot_area(o))
        .then("a storage line with a feeder and two cows by it", move |b, _| {
            build(b, o, &[])?;
            for x in 1..=5 {
                for z in -4..=-2 {
                    b.place(at(o, [x, -1, z]), STONE, &[])?;
                }
            }
            b.place(feeder, "rustcraft:feeder", &[])?;
            for x in [2.5, 4.5] {
                b.spawn_mob("minecraft:cow", [f64::from(o[0]) + x, f64::from(o[1]), f64::from(o[2]) - 3.5], None)?;
            }
            Ok(())
        })
        // Its step (every 20 s) looks round and remembers what its animals eat.
        .wait(420)
        .then("wheat, carrots and wheat seeds in the chest", move |b, _| {
            fill(b, chest(o, 1), WHEAT, 8)?;
            fill(b, chest(o, 1), CARROT, 4)?;
            fill(b, chest(o, 1), SEEDS, 4)
        })
        .until("the chest is empty", TAKE, move |b, _| Ok(chest_empty(b, o)))
        .check("the carrots and seeds are in the storage, none in the feeder", move |b, _| {
            same("carrots stored", held(b, o, CARROT)?, 4)?;
            same("seeds stored", held(b, o, SEEDS)?, 4)?;
            keeps_none(b, feeder, "feeder", &[CARROT, SEEDS])
        })
        .check("the cows' wheat went into it", move |b, _| {
            let fed = count_of(&b.items(feeder), WHEAT);
            same("wheat in the feeder and the storage", u64::from(fed) + held(b, o, WHEAT)?, 8)?;
            if fed > 0 { Ok(()) } else { Err("the feeder took no wheat".into()) }
        })
}

fn apiary_takes_bottles_for_honey_only(o: Pos, _: &Tuning) -> Scenario {
    let apiary = beside(o, 3);
    Scenario::new("intake21_apiary_takes_bottles_for_honey_only", plot_area(o))
        .then("a storage line with an apiary collector set to combs, a chest of bottles", move |b, _| {
            build(b, o, &[])?;
            b.place(apiary, "rustcraft:apiary", &[])?;
            b.press_cursor(apiary, Press::Toggle(1).id(), None)?;
            fill(b, chest(o, 1), BOTTLE, 4)
        })
        .until("the bottles are in the storage", TAKE, move |b, _| Ok(held(b, o, BOTTLE)? == 4))
        .check("the collector took none", move |b, _| keeps_none(b, apiary, "collector", &[BOTTLE]))
        .then("set to honey, more bottles in the chest", move |b, _| {
            b.press_cursor(apiary, Press::Toggle(0).id(), None)?;
            fill(b, chest(o, 1), BOTTLE, 4)
        })
        .until("the collector takes bottles", TAKE, move |b, _| Ok(count_of(&b.items(apiary), BOTTLE) > 0))
}

fn brewer_without_a_page_takes_nothing(o: Pos, _: &Tuning) -> Scenario {
    let vat = beside(o, 3);
    Scenario::new("intake21_brewer_without_a_page_takes_nothing", plot_area(o))
        .then("a storage line with a vat with no page, a chest of water bottles and blaze powder", move |b, _| {
            build(b, o, &[])?;
            b.place(vat, "rustcraft:brewer", &[])?;
            for _ in 0..2 {
                same("a water bottle in the chest", b.insert_snbt(chest(o, 1), WATER_BOTTLE)?, 1)?;
            }
            fill(b, chest(o, 1), POWDER, 4)
        })
        .until("the chest is empty", TAKE, move |b, _| Ok(chest_empty(b, o)))
        .check("the vat took neither, the powder is in the storage", move |b, _| {
            keeps_none(b, vat, "vat", &[POTION, POWDER])?;
            same("powder stored", held(b, o, POWDER)?, 4)
        })
        .then("its first page set to swiftness, more powder in the chest", move |b, _| {
            b.press_cursor(vat, Press::Toggle(0).id(), None)?;
            fill(b, chest(o, 1), POWDER, 4)
        })
        .until("the vat takes powder", TAKE, move |b, _| Ok(count_of(&b.items(vat), POWDER) > 0))
}

fn vein_takes_the_rock_it_grows_from(o: Pos, _: &Tuning) -> Scenario {
    let vein = beside(o, 3);
    Scenario::new("intake21_vein_takes_the_rock_it_grows_from", plot_area(o))
        .then("a storage line with a vein without modules, a chest of sand, gravel and cobblestone", move |b, _| {
            build(b, o, &[])?;
            b.place(vein, "rustcraft:vein", &[])?;
            fill(b, chest(o, 1), SAND, 4)?;
            fill(b, chest(o, 1), GRAVEL, 4)?;
            fill(b, chest(o, 1), COBBLE, 8)
        })
        .until("the chest is empty", TAKE, move |b, _| Ok(chest_empty(b, o)))
        .check("the sand and gravel are in the storage, the vein took cobblestone", move |b, _| {
            same("sand stored", held(b, o, SAND)?, 4)?;
            same("gravel stored", held(b, o, GRAVEL)?, 4)?;
            keeps_none(b, vein, "vein", &[SAND, GRAVEL])?;
            if count_of(&b.items(vein), COBBLE) > 0 { Ok(()) } else { Err("the vein took no cobblestone".into()) }
        })
}

fn enchanter_takes_lapis_for_something_to_enchant(o: Pos, _: &Tuning) -> Scenario {
    let enchanter = beside(o, 3);
    Scenario::new("intake21_enchanter_takes_lapis_for_something_to_enchant", plot_area(o))
        .then("a storage line with an empty enchanting machine, a chest of lapis", move |b, _| {
            build(b, o, &[])?;
            b.place(enchanter, "rustcraft:enchanter", &[])?;
            fill(b, chest(o, 1), LAPIS, 8)
        })
        .until("the lapis is in the storage", TAKE, move |b, _| Ok(held(b, o, LAPIS)? == 8))
        .check("the machine took none", move |b, _| keeps_none(b, enchanter, "machine", &[LAPIS]))
        .then("a book in it by a hopper, more lapis in the chest", move |b, _| {
            same("the book in", b.insert(enchanter, BOOK, 1)?, 1)?;
            fill(b, chest(o, 1), LAPIS, 4)
        })
        .until("the machine takes lapis", TAKE, move |b, _| Ok(count_of(&b.items(enchanter), LAPIS) > 0))
}

fn enchanter_enchants_one_book_a_step(o: Pos, _: &Tuning) -> Scenario {
    // 0.21: a stack of books went whole into one enchanted stack that never fit its output.
    const ENCHANTED: &str = "minecraft:enchanted_book";
    let p = at(o, [3, 1, 0]);
    let tank = at(o, [4, 1, 0]);
    Scenario::new("intake21_enchanter_enchants_one_book_a_step", plot_area(o))
        .then(
            "an enchanting machine with energy for one step, two books and 3 lapis, a full condenser beside it",
            move |b, _| {
                b.place(at(o, [3, 0, 0]), STONE, &[])?;
                b.place(at(o, [4, 0, 0]), STONE, &[])?;
                b.place(p, "rustcraft:enchanter", &[])?;
                b.set_buffer(p, 600.0)?;
                b.place(tank, "rustcraft:xp_tank", &[])?;
                b.set_long(tank, "util_xp", 3000)?;
                b.put(p, 0, BOOK, 2)?;
                b.put(p, 1, LAPIS, 3)
            },
        )
        .until("an enchanted book is out", 800, move |b, _| Ok(count_of(&b.items(p), ENCHANTED) == 1))
        .check("one book is left, the lapis spent", move |b, _| {
            same("books left", count_of(&b.items(p), BOOK), 1)?;
            same("lapis left", count_of(&b.items(p), LAPIS), 0)
        })
}

fn portal_dock_sends_only_what_its_joint_names(o: Pos, _: &Tuning) -> Scenario {
    let dock = beside(o, 3);
    Scenario::new("intake21_portal_dock_sends_only_what_its_joint_names", plot_area(o))
        .then("a storage line with a portal dock, a chest of cobblestone", move |b, _| {
            build(b, o, &[])?;
            b.place(dock, "rustcraft:portal_dock", &[])?;
            fill(b, chest(o, 1), COBBLE, 8)
        })
        .until("the cobblestone is in the storage", TAKE, move |b, _| Ok(held(b, o, COBBLE)? == 8))
        .check("the dock took none", move |b, _| keeps_none(b, dock, "dock", &[COBBLE]))
        .then("the dock's joint names cobblestone, more in the chest", move |b, _| {
            feed_joint(b, o, 3, &[COBBLE])?;
            fill(b, chest(o, 1), COBBLE, 4)
        })
        .until("the dock takes cobblestone", TAKE, move |b, _| Ok(count_of(&b.items(dock), COBBLE) > 0))
}
