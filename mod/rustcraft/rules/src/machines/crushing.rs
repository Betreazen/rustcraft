//! 0.21.2 (spec 50; the owner: «добавь в целом больше рецептов, а то видно что не все есть»): the
//! crusher's two kinds of recipe.
//!
//! - **Ore** (iron, copper and gold ore and raw metal → two dust): its own work. The storage feeds
//!   it ore by itself (0.20), the magistral brings it, the enricher and the yield module may give
//!   one more dust, and an order never smelts what it grinds whole (0.19.2).
//! - **Taking a block apart** (MR1): a bone block into 3 bones, wool into 4 string, glowstone into
//!   4 dust, cobblestone into gravel… — so an order can make the everyday items no crafting
//!   recipe makes. Each gives at most what the game crafts the block from (MR2), so no round of
//!   crafting and crushing makes more than it took; only an order, a joint's allow list, a hopper
//!   or a hand puts such a block in (MR3), and nothing raises its yield (MR4).

use crate::machines::intake::is_dust;

/// MR1–MR4: whether a crushing recipe that makes `result` grinds ore (its result is dust) rather
/// than takes a block apart.
pub fn grinds_ore(result: &str) -> bool {
    is_dust(result)
}

/// MR4: whether the enricher or the yield module may give one more of `result` from the crusher.
pub fn crusher_lucky(result: &str) -> bool {
    grinds_ore(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orders::{PlanRecipe, can_make, furnace_inputs, plan};
    use serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};

    const RECIPES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/data/rustcraft/recipe");

    /// MR2: what the game crafts each block the crusher takes apart from, counted in the item the
    /// crusher gives: a bone block is 9 bone meal, and a bone 3 of them, so 3 bones. None: no
    /// recipe makes the block from its product, so no round goes through both.
    const VANILLA: [(&str, &str, Option<u32>); 13] = [
        ("minecraft:bone_block", "minecraft:bone", Some(3)),
        ("minecraft:white_wool", "minecraft:string", Some(4)),
        ("minecraft:glowstone", "minecraft:glowstone_dust", Some(4)),
        ("minecraft:quartz_block", "minecraft:quartz", Some(4)),
        ("minecraft:amethyst_block", "minecraft:amethyst_shard", Some(4)),
        ("minecraft:honeycomb_block", "minecraft:honeycomb", Some(4)),
        ("minecraft:prismarine", "minecraft:prismarine_shard", Some(4)),
        ("minecraft:prismarine_bricks", "minecraft:prismarine_shard", Some(9)),
        // A sea lantern is 4 shards and 5 crystals: the crystals come back, the shards do not.
        ("minecraft:sea_lantern", "minecraft:prismarine_crystals", Some(5)),
        ("minecraft:nether_wart_block", "minecraft:nether_wart", Some(9)),
        ("minecraft:clay", "minecraft:clay_ball", Some(4)),
        ("minecraft:cobblestone", "minecraft:gravel", None),
        ("minecraft:gravel", "minecraft:sand", None),
    ];

    struct Crushing {
        inputs: Vec<String>,
        result: String,
        count: u32,
    }

    /// The content mod's crushing recipes, as their files say them.
    fn crushing() -> Vec<Crushing> {
        let mut all = Vec::new();
        for entry in std::fs::read_dir(RECIPES).expect("the content mod's recipes") {
            let text = std::fs::read_to_string(entry.expect("a recipe file").path()).expect("a recipe");
            let json: Value = serde_json::from_str(&text).expect("a recipe is JSON");
            if json["type"] != "rustcraft:crushing" {
                continue;
            }
            let inputs = match &json["ingredient"] {
                Value::String(s) => vec![s.clone()],
                Value::Array(a) => a.iter().map(|v| v.as_str().expect("an item id").to_owned()).collect(),
                other => panic!("an ingredient the test does not read: {other}"),
            };
            all.push(Crushing {
                inputs,
                result: json["result"]["id"].as_str().expect("a result").to_owned(),
                count: json["result"]["count"].as_u64().map_or(1, |n| n as u32),
            });
        }
        all
    }

    #[test]
    fn each_item_has_one_crushing_recipe() {
        // The crusher runs the first recipe that takes its input: a second would never run, and an
        // order planned on it would get the first one's product.
        let mut seen = BTreeSet::new();
        for r in crushing() {
            for i in &r.inputs {
                assert!(seen.insert(i.clone()), "{i} has two crushing recipes");
            }
        }
    }

    #[test]
    fn taking_a_block_apart_gives_no_more_than_it_was_crafted_from() {
        // MR2: bone block → 3 bones is exactly what 9 bone meal are worth; a fourth bone would make
        // bone meal → bone block → bones → bone meal a source of bone meal.
        let apart: Vec<Crushing> = crushing().into_iter().filter(|r| !grinds_ore(&r.result)).collect();
        assert!(apart.len() >= VANILLA.len(), "the recipes of spec 50 are in the content mod");
        for r in &apart {
            let (_, out, most) = VANILLA
                .iter()
                .find(|(block, _, _)| r.inputs.iter().any(|i| i == block))
                .unwrap_or_else(|| panic!("{:?}: no line of VANILLA says what it is crafted from", r.inputs));
            assert_eq!(*out, r.result, "{:?}", r.inputs);
            if let Some(most) = most {
                assert!(r.count <= *most, "{:?} gives {} {}, crafted from {most}", r.inputs, r.count, r.result);
            }
        }
        let wool = apart.iter().find(|r| r.result == "minecraft:string").expect("wool → string");
        assert_eq!(wool.inputs.len(), 16, "every colour of wool");
    }

    #[test]
    fn only_ore_is_the_crushers_own_work() {
        // MR3, MR4: the storage's feed, the magistral's flow and the extra product are for ore.
        for r in crushing() {
            let ore = r.inputs.iter().all(|i| i.contains("_ore") || i.starts_with("minecraft:raw_"));
            assert_eq!(grinds_ore(&r.result), ore, "{:?} → {}", r.inputs, r.result);
            assert_eq!(crusher_lucky(&r.result), ore);
        }
    }

    /// The book the game makes of these recipes: the vanilla bone meal and bone block, the
    /// furnace's sand and cobblestone, the crusher's files.
    fn book() -> Vec<PlanRecipe> {
        let r = PlanRecipe::simple;
        let mut book = vec![
            r("crafter", &[("minecraft:bone", 1)], ("minecraft:bone_meal", 3)),
            r("crafter", &[("minecraft:bone_block", 1)], ("minecraft:bone_meal", 9)),
            r("crafter", &[("minecraft:bone_meal", 9)], ("minecraft:bone_block", 1)),
            r("crafter", &[("minecraft:string", 4)], ("minecraft:white_wool", 1)),
            r("furnace", &[("minecraft:sand", 1)], ("minecraft:glass", 1)),
            r("furnace", &[("minecraft:cobblestone", 1)], ("minecraft:stone", 1)),
        ];
        for c in crushing() {
            let mut inputs = c.inputs.clone();
            inputs.sort();
            book.push(PlanRecipe {
                machine: "crusher".into(),
                inputs: vec![(inputs, 1)],
                output: (c.result, c.count),
                cells: Vec::new(),
            });
        }
        book
    }

    fn stock(items: &[(&str, u64)]) -> BTreeMap<String, u64> {
        items.iter().map(|(i, n)| (i.to_string(), *n)).collect()
    }

    const ALL: &[&str] = &["crafter", "furnace", "crusher"];

    #[test]
    fn bone_meal_is_ordered_from_bone_blocks() {
        // The owner (corrected): «костная мука получается» from a bone block. 18 bone meal from
        // two bone blocks on the autocrafter, none from the crusher's bones.
        let p = plan("minecraft:bone_meal", 18, &stock(&[("minecraft:bone_block", 5)]), &book(), ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved, stock(&[("minecraft:bone_block", 2)]));
        assert_eq!(p.steps.len(), 1, "{:?}", p.steps);
        assert_eq!(p.steps[0].machine, "crafter");
        assert_eq!(p.steps[0].times, 2);
    }

    #[test]
    fn bones_string_and_the_rest_can_be_ordered() {
        // OT8 refused them: no recipe made them.
        let book = book();
        for item in [
            "minecraft:bone",
            "minecraft:string",
            "minecraft:glowstone_dust",
            "minecraft:quartz",
            "minecraft:amethyst_shard",
            "minecraft:honeycomb",
            "minecraft:prismarine_shard",
            "minecraft:prismarine_crystals",
            "minecraft:nether_wart",
            "minecraft:clay_ball",
            "minecraft:gravel",
            "minecraft:sand",
        ] {
            assert!(can_make(item, &book), "{item}");
        }
        let p = plan("minecraft:bone", 6, &stock(&[("minecraft:bone_block", 2)]), &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.steps[0].machine, "crusher");
        assert_eq!(p.reserved, stock(&[("minecraft:bone_block", 2)]));
        let p = plan("minecraft:string", 8, &stock(&[("minecraft:red_wool", 1), ("minecraft:white_wool", 1)]), &book, ALL);
        assert!(p.ready(), "any wool gives string: {p:?}");
    }

    #[test]
    fn glass_is_ordered_from_cobblestone() {
        // Cobblestone → gravel → sand on the crusher, then the furnace.
        let p = plan("minecraft:glass", 4, &stock(&[("minecraft:cobblestone", 10)]), &book(), ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved, stock(&[("minecraft:cobblestone", 4)]));
        let machines: Vec<&str> = p.steps.iter().map(|s| s.machine.as_str()).collect();
        assert_eq!(machines, ["crusher", "crusher", "furnace"]);
    }

    #[test]
    fn a_round_through_the_crusher_makes_no_more_than_it_took() {
        // 9 bone meal → a bone block → 3 bones → 9 bone meal: the same 9.
        let book = book();
        let p = plan("minecraft:bone", 3, &stock(&[("minecraft:bone_meal", 64)]), &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved, stock(&[("minecraft:bone_meal", 9)]));
        let p = plan("minecraft:bone_meal", 9, &stock(&[("minecraft:bone", 64)]), &book, ALL);
        assert_eq!(p.reserved, stock(&[("minecraft:bone", 3)]));
        // 4 string → white wool → 4 string.
        let p = plan("minecraft:string", 4, &stock(&[("minecraft:white_wool", 64)]), &book, ALL);
        assert_eq!(p.reserved, stock(&[("minecraft:white_wool", 1)]));
    }

    #[test]
    fn what_the_crusher_takes_apart_still_smelts_whole() {
        // 0.19.2 keeps only ore out of the furnace: stone from cobblestone stays a way.
        let ore: BTreeSet<String> = crushing()
            .into_iter()
            .filter(|r| grinds_ore(&r.result))
            .flat_map(|r| r.inputs)
            .collect();
        assert!(ore.contains("minecraft:raw_iron"));
        assert_eq!(furnace_inputs(vec!["minecraft:cobblestone".into()], &ore), Some(vec!["minecraft:cobblestone".into()]));
        assert_eq!(furnace_inputs(vec!["minecraft:clay".into()], &ore), Some(vec!["minecraft:clay".into()]));
        assert_eq!(furnace_inputs(vec!["minecraft:raw_iron".into()], &ore), None);
    }
}
