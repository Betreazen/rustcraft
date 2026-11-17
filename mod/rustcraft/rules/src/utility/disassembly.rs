//! The disintegrator (spec 25, DS1–DS7): a crafted item back into a share of its recipe's
//! ingredients, half by default. Fractions are kept per item and given out once whole (DS1).

use std::collections::BTreeMap;

/// The ingredients of a crafting recipe as the disintegrator reads it: one representative item
/// each, how many, and how many results the recipe makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub result: String,
    pub result_count: u32,
    pub ingredients: Vec<(String, u32)>,
}

/// Why an item is not taken apart (DS4, «Предмет не разбирается»).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoRecipe,
    /// One craft makes several of it (planks, sticks): taking one apart would give more than
    /// went in.
    ManyResults,
    /// The recipe can be undone at the crafting table (a block of iron and its ingots).
    Reversible,
    /// A charged item of the mod with charge left.
    Charged,
}

/// The share of each ingredient it gives, in percent: 50, 60 or 70 with yield modules, never above
/// 75 (DS5).
pub fn share_percent(yield_modules: usize) -> u32 {
    (50 + 10 * yield_modules as u32).min(75)
}

/// The recipe that makes `item`, or why there is none to undo. `recipes` are the game's crafting
/// recipes; `charged` whether the item still holds charge.
pub fn recipe_for<'a>(item: &str, recipes: &'a [Recipe], charged: bool) -> Result<&'a Recipe, Refusal> {
    if charged {
        return Err(Refusal::Charged);
    }
    let r = recipes.iter().find(|r| r.result == item).ok_or(Refusal::NoRecipe)?;
    if r.result_count > 1 {
        return Err(Refusal::ManyResults);
    }
    let reversible = r.ingredients.iter().any(|(ing, _)| {
        recipes
            .iter()
            .any(|back| back.result == *ing && back.ingredients.iter().all(|(i, _)| i == item))
    });
    if reversible {
        return Err(Refusal::Reversible);
    }
    Ok(r)
}

/// What one item gives: whole items now, the fractions (thousandths) added to `kept`. `durability`
/// is the share of its life left, 0..=1 (DS2), 1 for an item without durability.
pub fn take_apart(
    recipe: &Recipe,
    percent: u32,
    durability: f64,
    kept: &mut BTreeMap<String, u32>,
) -> Vec<(String, u32)> {
    let mut whole: BTreeMap<String, u32> = BTreeMap::new();
    let share = (f64::from(percent) * durability.clamp(0.0, 1.0) * 10.0).round() as u32;
    for (item, count) in &recipe.ingredients {
        let k = kept.entry(item.clone()).or_default();
        *k += count * share;
        let n = *k / 1000;
        *k %= 1000;
        if n > 0 {
            *whole.entry(item.clone()).or_default() += n;
        }
    }
    kept.retain(|_, v| *v > 0);
    whole.into_iter().collect()
}

/// Slice 18, wave 2 (guide `ui.js` disintegrator, «Разборка»): what one item gives, before the
/// kept fractions: each ingredient's share in thousandths (an iron pickaxe at 50%: 1500 iron
/// ingots, 1000 sticks).
pub fn preview(recipe: &Recipe, percent: u32, durability: f64) -> Vec<(String, u32)> {
    let share = (f64::from(percent) * durability.clamp(0.0, 1.0) * 10.0).round() as u32;
    recipe
        .ingredients
        .iter()
        .map(|(item, count)| (item.clone(), count * share))
        .filter(|(_, n)| *n > 0)
        .collect()
}

/// An amount of thousandths as the guide writes it: whole items, then ½, ⅓, ⅔, ¼, ¾ or a tenth
/// («1½», «½», «2», «0,3»).
pub fn amount_text(thousandths: u32) -> String {
    let whole = thousandths / 1000;
    let part = thousandths % 1000;
    let frac = match part {
        0 => "",
        245..=255 => "¼",
        328..=338 => "⅓",
        495..=505 => "½",
        662..=672 => "⅔",
        745..=755 => "¾",
        _ => {
            let tenths = (f64::from(thousandths) / 100.0).round() as u32;
            return if tenths % 10 == 0 {
                (tenths / 10).to_string()
            } else {
                format!("{},{}", tenths / 10, tenths % 10)
            };
        }
    };
    match (whole, frac) {
        (0, "") => "0".to_owned(),
        (0, f) => f.to_owned(),
        (w, f) => format!("{w}{f}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_is_the_share_of_each_ingredient() {
        let pick = r("minecraft:iron_pickaxe", 1, &[("minecraft:iron_ingot", 3), ("minecraft:stick", 2)]);
        assert_eq!(
            preview(&pick, 50, 1.0),
            vec![("minecraft:iron_ingot".into(), 1500), ("minecraft:stick".into(), 1000)]
        );
        assert_eq!(preview(&pick, 60, 0.5)[0].1, 900);
        assert!(preview(&pick, 50, 0.0).is_empty());
    }

    #[test]
    fn amount_text_writes_whole_items_and_fractions() {
        assert_eq!(amount_text(1500), "1½");
        assert_eq!(amount_text(500), "½");
        assert_eq!(amount_text(2000), "2");
        assert_eq!(amount_text(333), "⅓");
        assert_eq!(amount_text(1750), "1¾");
        assert_eq!(amount_text(300), "0,3");
        assert_eq!(amount_text(0), "0");
    }

    fn r(result: &str, n: u32, ings: &[(&str, u32)]) -> Recipe {
        Recipe {
            result: result.into(),
            result_count: n,
            ingredients: ings.iter().map(|(i, c)| ((*i).into(), *c)).collect(),
        }
    }

    #[test]
    fn two_items_of_three_ingots_give_three_ingots() {
        // DS1.
        let pick = r("minecraft:iron_pickaxe", 1, &[("minecraft:iron_ingot", 3), ("minecraft:stick", 2)]);
        let mut kept = BTreeMap::new();
        let first = take_apart(&pick, 50, 1.0, &mut kept);
        assert_eq!(first, vec![("minecraft:iron_ingot".into(), 1), ("minecraft:stick".into(), 1)]);
        let second = take_apart(&pick, 50, 1.0, &mut kept);
        assert_eq!(second, vec![("minecraft:iron_ingot".into(), 2), ("minecraft:stick".into(), 1)]);
        assert!(kept.is_empty());
    }

    #[test]
    fn worn_items_give_less_and_yield_modules_more_but_never_past_75() {
        // DS2, DS5.
        let pick = r("minecraft:iron_pickaxe", 1, &[("minecraft:iron_ingot", 4)]);
        let mut kept = BTreeMap::new();
        assert_eq!(take_apart(&pick, 50, 0.5, &mut kept), vec![("minecraft:iron_ingot".into(), 1)]);
        assert_eq!(share_percent(0), 50);
        assert_eq!(share_percent(2), 70);
        assert_eq!(share_percent(9), 75);
    }

    #[test]
    fn planks_blocks_of_ingots_and_charged_items_are_refused() {
        // DS4.
        let recipes = vec![
            r("minecraft:oak_planks", 4, &[("minecraft:oak_log", 1)]),
            r("minecraft:iron_block", 1, &[("minecraft:iron_ingot", 9)]),
            r("minecraft:iron_ingot", 9, &[("minecraft:iron_block", 1)]),
            r("minecraft:bucket", 1, &[("minecraft:iron_ingot", 3)]),
        ];
        assert_eq!(recipe_for("minecraft:oak_planks", &recipes, false), Err(Refusal::ManyResults));
        assert_eq!(recipe_for("minecraft:iron_block", &recipes, false), Err(Refusal::Reversible));
        assert_eq!(recipe_for("minecraft:dirt", &recipes, false), Err(Refusal::NoRecipe));
        assert_eq!(recipe_for("minecraft:bucket", &recipes, true), Err(Refusal::Charged));
        assert!(recipe_for("minecraft:bucket", &recipes, false).is_ok());
    }
}
