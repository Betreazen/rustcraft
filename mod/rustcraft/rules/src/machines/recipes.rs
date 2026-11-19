//! 0.21 (round 32, spec 39 RT1–RT8; the owner: «нужно сделать какое-то окно дополнительное, где я
//! включаю, отключаю рецепт … если выбрано два рецепта, ищет для одного ресурсы, берет их, делает
//! первый рецепт, пока не исчезнут ресурсы … приступает ко второму»): the mixer's and the kitchen's
//! recipes, each turned on or off by a toggle in the machine's window.
//!
//! - The window lists every recipe of the machine (RT1); the toggles are kept in its block entity
//!   as the list of the recipes turned on, by their product (`KEY`): a mixer's fluid, a kitchen's
//!   dish.
//! - The machine works on the recipe it works on now as long as it has a run of it, then on the
//!   next one turned on in list order that it can make, round to the first ([`pick`], RT2).
//! - It takes the ingredients from the storage on its item net itself, a few runs at a time
//!   ([`fetch`], RT3); the magistral brings it only the ingredients of the recipes turned on
//!   ([`net_takes`], RT5).
//! - An order's job runs whatever the toggles say (RT6).

use std::collections::BTreeSet;

use crate::net::items::Filter;

/// The block entity's key of the recipes turned on: a JSON list of products.
pub const KEY: &str = "recipes_on";
/// The block entity's key of the recipe it works on now: its product.
pub const NOW_KEY: &str = "recipe_now";
/// The most runs it takes from the storage at a time.
pub const FETCH_RUNS: u32 = 4;
/// It takes more from the storage once it holds fewer runs than this: the next runs are there
/// before the last one is done.
pub const REFILL_BELOW: u64 = 2;

/// What the machine works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// No recipe is turned on: «Включи рецепт в окне».
    Off,
    /// None of the recipes turned on has its ingredients: «Нет ингредиентов», naming this one,
    /// the first turned on.
    Missing(usize),
    /// This recipe.
    Run(usize),
}

impl Pick {
    pub fn run(self) -> Option<usize> {
        match self {
            Pick::Run(i) => Some(i),
            _ => None,
        }
    }

    /// The recipe it names: the one it makes, or the first turned on when it lacks ingredients.
    pub fn named(self) -> Option<usize> {
        match self {
            Pick::Run(i) | Pick::Missing(i) => Some(i),
            Pick::Off => None,
        }
    }
}

/// RT2: the recipe to work on, of `on` (a flag a recipe, in the window's order). The one it works
/// on now (`now`) while it is on and `can` make it; else the next one on that it can make, after
/// `now` and round to the first; none on: `Off`; none it can make: `Missing` the first one on.
pub fn pick(on: &[bool], now: Option<usize>, can: impl Fn(usize) -> bool) -> Pick {
    let n = on.len();
    let Some(first) = on.iter().position(|b| *b) else {
        return Pick::Off;
    };
    if let Some(c) = now.filter(|c| *c < n && on[*c])
        && can(c)
    {
        return Pick::Run(c);
    }
    let start = now.map_or(0, |c| c + 1);
    (0..n)
        .map(|k| (start + k) % n)
        .find(|i| on[*i] && can(*i))
        .map_or(Pick::Missing(first), Pick::Run)
}

/// The flags of `products` (the window's order) the saved list `on` turns on.
pub fn flags(products: &[&str], on: &[String]) -> Vec<bool> {
    products.iter().map(|p| on.iter().any(|o| o == p)).collect()
}

/// A press on the toggle of `product`: the saved list with it turned on or off, the rest as it
/// was.
pub fn press(on: &[String], product: &str) -> Vec<String> {
    if on.iter().any(|o| o == product) {
        on.iter().filter(|o| *o != product).cloned().collect()
    } else {
        on.iter().cloned().chain(std::iter::once(product.to_owned())).collect()
    }
}

/// How many whole runs of a recipe needing `need` the counts `have` hold; one that needs nothing
/// has none.
pub fn runs_in(need: &[(String, u32)], have: impl Fn(&str) -> u64) -> u64 {
    need.iter()
        .filter(|(_, n)| *n > 0)
        .map(|(item, n)| have(item) / u64::from(*n))
        .min()
        .unwrap_or(0)
}

/// RT3: what the machine takes from the storage for a recipe needing `need`: nothing while it
/// holds `REFILL_BELOW` runs or more (`have`); else, item by item, what makes up to `most` whole
/// runs out of what it holds, what the storage has (`stored`) and the room its inputs have for
/// each item (`room`). As many runs of every ingredient, so nothing is left over; nothing when
/// the storage and its inputs together do not make a run.
pub fn fetch(
    need: &[(String, u32)],
    have: impl Fn(&str) -> u64,
    stored: impl Fn(&str) -> u64,
    room: impl Fn(&str) -> u64,
    most: u32,
) -> Vec<(String, u32)> {
    if need.is_empty() || runs_in(need, &have) >= REFILL_BELOW {
        return Vec::new();
    }
    let reach = |item: &str| have(item) + stored(item).min(room(item));
    let runs = runs_in(need, reach).min(u64::from(most));
    if runs == 0 || runs <= runs_in(need, &have) {
        return Vec::new();
    }
    need.iter()
        .filter_map(|(item, n)| {
            let want = (u64::from(*n) * runs).saturating_sub(have(item));
            (want > 0).then(|| (item.clone(), u32::try_from(want).unwrap_or(u32::MAX)))
        })
        .collect()
}

/// RT5: whether the magistral brings `item`: an ingredient of a recipe turned on
/// (`ingredients`, a set a recipe in the window's order).
pub fn net_takes(on: &[bool], ingredients: &[BTreeSet<String>], item: &str) -> bool {
    on.iter()
        .zip(ingredients)
        .any(|(on, set)| *on && set.contains(item))
}

/// RT7 (the world's update): a mixer whose joints had an allow list (0.21's intake, IN5) gets the
/// recipes whose every ingredient a list names turned on; one without, none.
pub fn mixer_from_filters(ingredients: &[BTreeSet<String>], filters: &[Filter]) -> Vec<bool> {
    ingredients
        .iter()
        .map(|set| !set.is_empty() && set.iter().all(|i| crate::machines::intake::named(filters, i)))
        .collect()
}

/// RT7: a kitchen's dishes chosen on its pages (`pages`, an index of the dish list a page) are
/// turned on, each once.
pub fn kitchen_from_pages(pages: &[Option<usize>], dishes: usize) -> Vec<bool> {
    (0..dishes).map(|d| pages.contains(&Some(d))).collect()
}

/// The saved list of the products `flags` turns on, in the window's order.
pub fn saved(products: &[&str], flags: &[bool]) -> Vec<String> {
    products
        .iter()
        .zip(flags)
        .filter(|(_, on)| **on)
        .map(|(p, _)| (*p).to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|i| (*i).to_owned()).collect()
    }

    fn need(items: &[(&str, u32)]) -> Vec<(String, u32)> {
        items.iter().map(|(i, n)| ((*i).to_owned(), *n)).collect()
    }

    #[test]
    fn nothing_on_is_off_and_nothing_to_make_names_the_first_on() {
        assert_eq!(pick(&[false, false], None, |_| true), Pick::Off);
        assert_eq!(pick(&[], None, |_| true), Pick::Off);
        assert_eq!(pick(&[false, true, true], None, |_| false), Pick::Missing(1));
        assert_eq!(Pick::Missing(1).named(), Some(1));
        assert_eq!(Pick::Off.named(), None);
        assert_eq!(Pick::Missing(1).run(), None);
    }

    #[test]
    fn it_starts_on_the_first_on_it_can_make() {
        assert_eq!(pick(&[true, true], None, |_| true), Pick::Run(0));
        assert_eq!(pick(&[true, true], None, |i| i == 1), Pick::Run(1));
        assert_eq!(pick(&[false, true, true], None, |_| true), Pick::Run(1));
    }

    #[test]
    fn it_makes_one_until_it_runs_out_then_the_next() {
        // The owner's order: the first recipe while its ingredients last, then the second.
        let on = [true, false, true];
        let mut a = 3; // runs of recipe 0
        let mut c = 2; // runs of recipe 2
        let mut now = None;
        let mut made = Vec::new();
        loop {
            let left = [a, 0, c];
            match pick(&on, now, |i| left[i] > 0) {
                Pick::Run(i) => {
                    made.push(i);
                    now = Some(i);
                    if i == 0 { a -= 1 } else { c -= 1 }
                    // More of recipe 0 comes while it makes recipe 2: it finishes recipe 2 first.
                    if i == 2 && c == 1 {
                        a += 1;
                    }
                }
                other => {
                    assert_eq!(other, Pick::Missing(0));
                    break;
                }
            }
        }
        assert_eq!(made, vec![0, 0, 0, 2, 2, 0]);
    }

    #[test]
    fn after_the_last_it_comes_round_to_the_first() {
        assert_eq!(pick(&[true, true], Some(1), |i| i == 0), Pick::Run(0));
        // A recipe turned off is left at once.
        assert_eq!(pick(&[false, true], Some(0), |_| true), Pick::Run(1));
        // A saved recipe past the list (one gone from the data) starts over.
        assert_eq!(pick(&[true], Some(5), |_| true), Pick::Run(0));
    }

    #[test]
    fn a_press_turns_one_recipe_on_or_off() {
        let on = press(&[], "minecraft:bread");
        assert_eq!(on, vec!["minecraft:bread".to_owned()]);
        let on = press(&on, "minecraft:cake");
        assert_eq!(flags(&["minecraft:bread", "minecraft:cookie", "minecraft:cake"], &on), vec![true, false, true]);
        let on = press(&on, "minecraft:bread");
        assert_eq!(on, vec!["minecraft:cake".to_owned()]);
        assert_eq!(saved(&["a", "b", "c"], &[true, false, true]), vec!["a".to_owned(), "c".to_owned()]);
    }

    #[test]
    fn it_takes_a_few_whole_runs_and_leaves_the_rest() {
        let cake = need(&[("sugar", 2), ("egg", 1)]);
        let none = |_: &str| 0;
        let lots = |_: &str| 1000;
        // Empty: four runs from a full storage.
        assert_eq!(fetch(&cake, none, lots, lots, FETCH_RUNS), need(&[("sugar", 8), ("egg", 4)]));
        // As many runs of each: 5 sugar and 9 eggs make two cakes.
        let stored = |i: &str| if i == "sugar" { 5 } else { 9 };
        assert_eq!(fetch(&cake, none, stored, lots, FETCH_RUNS), need(&[("sugar", 4), ("egg", 2)]));
        // No run in the storage: nothing.
        let no_eggs = |i: &str| if i == "sugar" { 64 } else { 0 };
        assert!(fetch(&cake, none, no_eggs, lots, FETCH_RUNS).is_empty());
        // Two runs held: it waits; one held: it tops up to four.
        let two = |i: &str| if i == "sugar" { 4 } else { 2 };
        assert!(fetch(&cake, two, lots, lots, FETCH_RUNS).is_empty());
        let one = |i: &str| if i == "sugar" { 2 } else { 1 };
        assert_eq!(fetch(&cake, one, lots, lots, FETCH_RUNS), need(&[("sugar", 6), ("egg", 3)]));
        // Its inputs' room bounds it.
        let room = |i: &str| if i == "sugar" { 4 } else { 64 };
        assert_eq!(fetch(&cake, none, lots, room, FETCH_RUNS), need(&[("sugar", 4), ("egg", 2)]));
        assert!(fetch(&[], none, lots, lots, FETCH_RUNS).is_empty());
    }

    #[test]
    fn whole_runs_held() {
        let cake = need(&[("sugar", 2), ("egg", 1)]);
        assert_eq!(runs_in(&cake, |i| if i == "sugar" { 5 } else { 3 }), 2);
        assert_eq!(runs_in(&[], |_| 9), 0);
    }

    #[test]
    fn the_magistral_brings_only_what_the_recipes_on_need() {
        let ingredients = vec![set(&["bone_meal"]), set(&["sugar", "glowstone"])];
        assert!(!net_takes(&[false, false], &ingredients, "bone_meal"));
        assert!(net_takes(&[true, false], &ingredients, "bone_meal"));
        assert!(!net_takes(&[true, false], &ingredients, "sugar"));
        assert!(net_takes(&[false, true], &ingredients, "glowstone"));
        assert!(!net_takes(&[true, true], &ingredients, "minecraft:raw_iron"));
    }

    #[test]
    fn the_world_update_turns_on_what_a_joint_list_or_a_page_chose() {
        let ingredients = vec![set(&["bone_meal"]), set(&["sugar", "glowstone"]), set(&["honey", "dust_copper"])];
        let list = Filter { allow_only: true, items: set(&["bone_meal", "sugar"]) };
        // Glowstone is not named: the accelerator stays off.
        assert_eq!(mixer_from_filters(&ingredients, std::slice::from_ref(&list)), vec![true, false, false]);
        let open = Filter::default();
        assert_eq!(mixer_from_filters(&ingredients, &[open]), vec![false, false, false]);
        assert_eq!(mixer_from_filters(&ingredients, &[]), vec![false, false, false]);
        let both = Filter { allow_only: true, items: set(&["sugar", "glowstone"]) };
        assert_eq!(mixer_from_filters(&ingredients, &[list, both]), vec![true, true, false]);
        assert_eq!(kitchen_from_pages(&[Some(3), None, Some(0), Some(3)], 7), vec![true, false, false, true, false, false, false]);
        assert_eq!(kitchen_from_pages(&[None; 4], 3), vec![false; 3]);
        assert_eq!(kitchen_from_pages(&[Some(9)], 3), vec![false; 3], "a dish past the list");
    }
}
