//! 0.21 (round 32, spec 39; the owner: «если выбрано два рецепта, ищет для одного ресурсы, берет
//! их, делает первый рецепт, пока не исчезнут ресурсы … приступает ко второму»): the mixer's and
//! the kitchen's recipes, turned on and off by toggles in their windows.
//!
//! - A mixer with the solution and the accelerator on, beside a storage with bone meal for three
//!   buckets and sugar and glowstone for two, makes all three of solution first, then the
//!   accelerator; both end in the storage's cells, its ingredients all taken from the storage.
//! - A kitchen with bread and cookies on, beside a storage with wheat for two loaves and a batch
//!   of cookies, bakes both loaves first, then the cookies; they end in the storage.
//! - With nothing on, each says «Включи рецепт в окне»; with a recipe on and nothing for it,
//!   «Нет ингредиентов» naming it. Their windows have a toggle a recipe.
//! - A mixer of 0.20 whose joint's allow list names bone meal has its solution turned on (RT7).
//! - An order for cookies runs on a kitchen busy baking its own bread: what the kitchen took for
//!   its bread goes back to the storage, the cookies come (RT6).

use super::craft18::status_key;
use super::slice15::{FILL, build, crafter_at, held, holds, machine, put_book, terminal_at};
use super::transport::{chest, housing};
use super::{Bench, Build, Memo, Scenario, at, count_of, plot_area, same, total};
use crate::geom::{Face, Pos};
use crate::machines::sheet::Press;
use crate::net::fluids::BUCKET;
use crate::net::items::{Filter, Mode};
use crate::net::settings::JointSetting;
use crate::tuning::Tuning;

pub const ALL: [Build; 5] = [
    mixer_makes_one_recipe_then_the_next,
    kitchen_cooks_one_dish_then_the_next,
    nothing_on_says_so,
    mixer_turns_on_what_its_joint_named,
    order_runs_on_a_kitchen_busy_with_its_own,
];

const MIXER: &str = "rustcraft:mixer";
const KITCHEN: &str = "rustcraft:kitchen";
const TANK: &str = "rustcraft:tank";
const WATER: &str = "minecraft:water";
const SOLUTION: &str = "rustcraft:solution";
const ACCELERATOR: &str = "rustcraft:accelerator";
const BONE_MEAL: &str = "minecraft:bone_meal";
const SUGAR: &str = "minecraft:sugar";
const GLOWSTONE: &str = "minecraft:glowstone_dust";
const WHEAT: &str = "minecraft:wheat";
const COCOA: &str = "minecraft:cocoa_beans";
const BREAD: &str = "minecraft:bread";
const COOKIE: &str = "minecraft:cookie";
/// The window's order (rustcraft_rules::machines::mixer::ORDER, KITCHEN_RECIPES).
const SOLUTION_TOGGLE: usize = 0;
const ACCELERATOR_TOGGLE: usize = 1;
const BREAD_TOGGLE: usize = 0;
const COOKIE_TOGGLE: usize = 1;
/// Five buckets of 100 ticks, the ingredients' fetches and the product's way to the cells.
const MIX_LIMIT: u64 = 2400;
/// Three dishes of 80 ticks, their fetches and their way to the storage.
const COOK_LIMIT: u64 = 1200;

fn toggle(b: &mut dyn Bench, pos: Pos, i: usize) -> Result<(), String> {
    b.press(pos, Press::Toggle(i).id(), None)
}

/// The status line of the window at `pos`, its text component as JSON (its key and its
/// arguments' keys).
fn status_json(b: &dyn Bench, pos: Pos) -> String {
    b.sheet(pos)
        .and_then(|s| s.status)
        .map(|s| s.text.to_string())
        .unwrap_or_default()
}

fn fluid_in_storage(b: &dyn Bench, o: Pos, fluid: &str) -> Result<u64, String> {
    Ok(b.storage(terminal_at(o)).ok_or("the terminal shows no storage")?.store.fluid(fluid))
}

/// A water tank of two stacked tanks on top of the mixer `m`: it never runs dry here, so the
/// mixer's product goes along the channel to the storage, not into it.
fn water_on_top(b: &mut dyn Bench, m: Pos) -> Result<(), String> {
    for y in [1, 2] {
        b.place(at(m, [0, y, 0]), TANK, &[])?;
        b.set_tank(at(m, [0, y, 0]), WATER, 16 * BUCKET)?;
    }
    Ok(())
}

fn mixer_makes_one_recipe_then_the_next(o: Pos, _: &Tuning) -> Scenario {
    let m = crafter_at(o);
    let parts = [(BONE_MEAL, 3), (SUGAR, 2), (GLOWSTONE, 2)];
    Scenario::new("recipes21_mixer_makes_one_recipe_then_the_next", plot_area(o))
        .then("a storage on a tier 3 line with bone meal, sugar and glowstone", move |b, _| build(b, o, &parts))
        .until("the storage holds them", FILL, move |b, _| holds(b, o, &parts))
        .then("a mixer with energy and water on the line, the solution and the accelerator on", move |b, _| {
            machine(b, m, MIXER, true)?;
            water_on_top(b, m)?;
            toggle(b, m, SOLUTION_TOGGLE)?;
            toggle(b, m, ACCELERATOR_TOGGLE)
        })
        .until("3 buckets of solution and 2 of accelerator in the storage", MIX_LIMIT, move |b, memo: &mut Memo| {
            let accelerator = fluid_in_storage(b, o, ACCELERATOR)?;
            let mixing_it = b.mixer(m).is_some_and(|v| v.product.as_ref().is_some_and(|(f, _)| f == ACCELERATOR));
            if (accelerator > 0 || mixing_it) && memo.get("switched") == 0.0 {
                // The bone meal left when it turned to the accelerator: none, in the storage or in it.
                let left = held(b, o, BONE_MEAL)? + u64::from(count_of(&b.items(m), BONE_MEAL));
                memo.mark("switched", 1.0);
                memo.mark("bone_meal_left", left as f64);
            }
            Ok(fluid_in_storage(b, o, SOLUTION)? >= 3 * BUCKET && accelerator >= 2 * BUCKET)
        })
        .check("all the solution came first, its ingredients from the storage", move |b, memo| {
            same("bone meal left when the accelerator began", memo.get("bone_meal_left"), 0.0)?;
            for item in [BONE_MEAL, SUGAR, GLOWSTONE] {
                same(item, held(b, o, item)?, 0)?;
            }
            same("items left in the mixer", total(&b.items(m)), 0)
        })
        .until("it says it has no ingredients for the solution", 60, move |b, _| {
            Ok(status_key(b, m) == "rustcraft.mixer.no_ingredients_for")
        })
}

fn kitchen_cooks_one_dish_then_the_next(o: Pos, _: &Tuning) -> Scenario {
    let k = crafter_at(o);
    // Bread takes 3 wheat; a batch of 8 cookies 2 wheat and a cocoa bean.
    let parts = [(WHEAT, 8), (COCOA, 1)];
    Scenario::new("recipes21_kitchen_cooks_one_dish_then_the_next", plot_area(o))
        .then("a storage on a tier 3 line with 8 wheat and a cocoa bean", move |b, _| build(b, o, &parts))
        .until("the storage holds them", FILL, move |b, _| holds(b, o, &parts))
        .then("a kitchen with energy on the line", move |b, _| {
            machine(b, k, KITCHEN, true)?;
            b.set_buffer(k, 400.0)
        })
        // Its toggles, a step after it stands, as the window would press them.
        .then("bread and cookies on", move |b, _| {
            toggle(b, k, BREAD_TOGGLE)?;
            toggle(b, k, COOKIE_TOGGLE)
        })
        .until("2 loaves and 8 cookies in the storage", COOK_LIMIT, move |b, memo: &mut Memo| {
            // What its window says it cooks, step by step.
            let status = status_json(b, k);
            if status.contains("rustcraft.util.kitchen.making") {
                if status.contains("minecraft_cookie") {
                    memo.mark("cookies", 1.0);
                } else if status.contains("minecraft_bread") {
                    memo.mark("bread", 1.0);
                    if memo.get("cookies") > 0.0 {
                        memo.mark("bread_after_cookies", 1.0);
                    }
                }
            }
            Ok(held(b, o, BREAD)? == 2 && held(b, o, COOKIE)? == 8)
        })
        .check("it baked the bread first, then the cookies, all from the storage", move |b, memo| {
            same("its window named bread", memo.get("bread"), 1.0)?;
            same("its window named cookies", memo.get("cookies"), 1.0)?;
            same("bread after the cookies", memo.get("bread_after_cookies"), 0.0)?;
            same("wheat", held(b, o, WHEAT)?, 0)?;
            same("cocoa", held(b, o, COCOA)?, 0)
        })
}

fn nothing_on_says_so(o: Pos, _: &Tuning) -> Scenario {
    let (m, k) = (at(o, [2, 1, 2]), at(o, [5, 1, 2]));
    Scenario::new("recipes21_nothing_on_says_so", plot_area(o))
        .then("a mixer and a kitchen with energy, nothing on", move |b, _| {
            machine(b, m, MIXER, true)?;
            machine(b, k, KITCHEN, true)?;
            b.set_buffer(k, 400.0)
        })
        .until("each says to turn a recipe on", 200, move |b, _| {
            Ok(status_key(b, m) == "rustcraft.mixer.off" && status_key(b, k) == "rustcraft.util.kitchen_off")
        })
        .check("their windows have a toggle a recipe, all off", move |b, _| {
            let mixer = b.sheet(m).ok_or("no mixer window")?;
            let kitchen = b.sheet(k).ok_or("no kitchen window")?;
            same("mixer toggles", mixer.toggles.len(), 4)?;
            same("kitchen toggles", kitchen.toggles.len(), crate::utility::KITCHEN_RECIPES.len())?;
            same("on", mixer.toggles.iter().chain(&kitchen.toggles).filter(|t| t.on).count(), 0)
        })
        .then("the solution and bread on, nothing for them", move |b, _| {
            toggle(b, m, SOLUTION_TOGGLE)?;
            toggle(b, k, BREAD_TOGGLE)
        })
        .until("each says it has no ingredients for it", 200, move |b, _| {
            Ok(status_key(b, m) == "rustcraft.mixer.no_ingredients_for"
                && status_key(b, k) == "rustcraft.util.kitchen_missing_dish")
        })
        .check("their windows show them on", move |b, _| {
            same("mixer's solution", b.sheet(m).and_then(|s| s.toggles.first().map(|t| t.on)), Some(true))?;
            same("kitchen's bread", b.sheet(k).and_then(|s| s.toggles.first().map(|t| t.on)), Some(true))
        })
        .then("bread off again", move |b, _| toggle(b, k, BREAD_TOGGLE))
        .until("the kitchen says to turn a recipe on", 200, move |b, _| {
            Ok(status_key(b, k) == "rustcraft.util.kitchen_off")
        })
}

fn mixer_turns_on_what_its_joint_named(o: Pos, _: &Tuning) -> Scenario {
    // RT7: 0.21's intake fed a mixer by its joint's allow list; the world's update turns on the
    // recipes that list covers.
    let m = chest(o, 4);
    Scenario::new("recipes21_mixer_turns_on_what_its_joint_named", plot_area(o))
        .then("a mixer of 0.20 on a storage line, its joint naming bone meal", move |b, _| {
            build(b, o, &[])?;
            b.place(m, MIXER, &[])?;
            let mut setting = JointSetting::of_mode(Mode::Into);
            setting.filter = Filter { allow_only: true, items: [BONE_MEAL.to_owned()].into() };
            b.set_joint(housing(o, 4), Face::ZNeg, 0, setting)
        })
        .until("its window shows the solution on, the accelerator off", 200, move |b, _| {
            let on: Vec<bool> = b.sheet(m).map(|s| s.toggles.iter().map(|t| t.on).collect()).unwrap_or_default();
            Ok(on.len() == 4 && on[SOLUTION_TOGGLE] && !on[ACCELERATOR_TOGGLE])
        })
}

fn order_runs_on_a_kitchen_busy_with_its_own(o: Pos, _: &Tuning) -> Scenario {
    let k = crafter_at(o);
    let parts = [(WHEAT, 30), (COCOA, 1)];
    Scenario::new("recipes21_order_runs_on_a_kitchen_busy_with_its_own", plot_area(o))
        .then("a storage on a tier 3 line with 30 wheat and a cocoa bean", move |b, _| build(b, o, &parts))
        .until("the storage holds them", FILL, move |b, _| holds(b, o, &parts))
        .then("a kitchen with energy, the book", move |b, _| {
            machine(b, k, KITCHEN, true)?;
            b.set_buffer(k, 400.0)?;
            put_book(b, o)
        })
        .then("bread on", move |b, _| toggle(b, k, BREAD_TOGGLE))
        .until("it took wheat for its bread, or baked with it", 400, move |b, _| {
            Ok(count_of(&b.items(k), WHEAT) > 0 || count_of(&b.items(k), BREAD) > 0 || held(b, o, BREAD)? > 0)
        })
        .until("the storage takes an order for 8 cookies", FILL, move |b, _| {
            Ok(b.order(terminal_at(o), COOKIE, 8).is_ok())
        })
        .until("the cookies are in the storage and the order done", COOK_LIMIT, move |b, _| {
            Ok(held(b, o, COOKIE)? == 8 && b.orders(terminal_at(o)).is_empty())
        })
        .until("no wheat lost: 2 in the cookies, the rest loaves (3 each) or wheat", COOK_LIMIT, move |b, _| {
            let loaves = held(b, o, BREAD)? + u64::from(count_of(&b.items(k), BREAD));
            let wheat = held(b, o, WHEAT)? + u64::from(count_of(&b.items(k), WHEAT));
            Ok(wheat + 3 * loaves + 2 == 30)
        })
}
