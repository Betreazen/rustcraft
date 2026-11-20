//! 0.20 (round 31): what a machine takes by itself, from the magistral's free flow or a hopper,
//! beyond its slot rule. An order's shipment and a player's hand are not asked: an order writes
//! its items into the machine's inputs (rustcraft_orders), a hand follows the window's slot rules.
//!
//! The owner: «зачем у меня постоянно забирает электропечь брёвна берёзовые и пережаривает их в
//! угли». Before, the furnace took anything it could smelt, the generator anything that burns,
//! the placer any block, the disintegrator any crafted item and the enchanting machine any tool,
//! so the logs, planks, products and repaired tools on their way to the storage ended up burnt,
//! smelted, placed in the world or taken apart.

use std::collections::BTreeSet;

use crate::net::items::Filter;

/// Path-of-craft-tree's `raw_food` list (its `data/tree.json`, `lists.raw_food`): the food a
/// furnace cooks.
pub const RAW_FOOD: [&str; 9] = [
    "minecraft:beef",
    "minecraft:porkchop",
    "minecraft:chicken",
    "minecraft:mutton",
    "minecraft:rabbit",
    "minecraft:cod",
    "minecraft:salmon",
    "minecraft:potato",
    "minecraft:kelp",
];

/// What the generator and the alloy smelter's fuel slot take from the magistral: fuel made to
/// burn, not the logs, planks and wooden things that burn too.
pub const NET_FUELS: [&str; 5] = [
    "minecraft:coal",
    "minecraft:charcoal",
    "minecraft:coal_block",
    "minecraft:blaze_rod",
    "minecraft:dried_kelp_block",
];

/// A crusher's dust (`rustcraft:dust_iron`, …).
pub fn is_dust(item: &str) -> bool {
    item.strip_prefix("rustcraft:dust_").is_some_and(|rest| !rest.is_empty())
}

pub fn is_raw_food(item: &str) -> bool {
    RAW_FOOD.contains(&item)
}

/// The owner's choice (round 31, 2): the electric furnace takes by itself, from the magistral or
/// a hopper, only dust and raw food it can smelt; logs, sand, cobblestone and the rest go in only
/// with an order or by hand. Raw food in `kept_raw` (a kitchen on the same net cooks it raw)
/// stays out of it.
pub fn furnace_takes(item: &str, smeltable: bool, kept_raw: &BTreeSet<String>) -> bool {
    smeltable && (is_dust(item) || (is_raw_food(item) && !kept_raw.contains(item)))
}

/// The generator's and the alloy smelter's fuel from the magistral: coal, charcoal, a coal
/// block, a blaze rod, a dried kelp block. A hopper or a hand still gives them any fuel.
pub fn net_fuel(item: &str, burns: bool) -> bool {
    burns && NET_FUELS.contains(&item)
}

/// The block placer from the magistral: only more of a block it holds now, so an empty placer
/// beside the line takes nothing and places none of what rides to the storage.
pub fn placer_net_takes(held: &[&str], item: &str) -> bool {
    held.contains(&item)
}

/// Whether a joint's filter names `item`: an allow list with it. A joint without a setting lets
/// everything through, but names nothing.
pub fn named(filters: &[Filter], item: &str) -> bool {
    filters.iter().any(|f| f.allow_only && f.items.contains(item))
}

/// The disintegrator from the magistral: only what the allow list of a joint to it names (like
/// the trash, UT2), so the crafted things that ride to the storage stay whole. A hopper still
/// gives it anything it takes apart.
pub fn disintegrator_net_takes(filters: &[Filter], item: &str) -> bool {
    named(filters, item)
}

/// The enchanting machine's item slot from the magistral: books, and what a joint's allow list
/// names; a tool or armour on its way to the storage (a repaired rod) rides on. 0.21 (round 32):
/// only into an empty cell (`cell_empty`): it enchants one item a step, and a stack of books
/// waiting for experience is no longer the storage's.
pub fn enchanter_net_takes(filters: &[Filter], item: &str, cell_empty: bool) -> bool {
    cell_empty && (item == "minecraft:book" || named(filters, item))
}

/// 0.21 (round 32): the enchanting machine's lapis cell from the magistral: lapis only while its
/// item cell holds something to enchant (`has_item`). Before, an idle machine kept a stack of the
/// lapis that rode to the storage.
pub fn enchanter_lapis_net_takes(has_item: bool) -> bool {
    has_item
}

/// 0.21 (round 32; the owner: «теперь все машины знают, что они должны забирать?»): the alloy
/// smelter's and the mixer's ingredient cells from the magistral: only what the allow list of a
/// joint to the machine names, as the disintegrator (IN5). They choose no recipe, so before they
/// took every ingredient of every recipe: the gold, redstone, diamonds and shards, the dust, bone
/// meal and sugar that rode to the storage, and sat on them waiting for the rest of a recipe.
/// A machine that holds anything is no free machine for an order, so the orders stalled. An order
/// brings their ingredients itself; a hopper and a hand still give them any ingredient.
pub fn recipe_net_takes(filters: &[Filter], item: &str) -> bool {
    named(filters, item)
}

/// 0.21.2 (spec 50, MR3): the crusher from the magistral and the storage's own feed: ore it
/// grinds into dust (`ore`, `crushing::grinds_ore`), and what a joint's allow list names. The
/// blocks it takes apart (bone blocks, wool, cobblestone…) ride on to the storage: an order, a
/// hopper or a hand puts them in.
pub fn crusher_net_takes(filters: &[Filter], item: &str, ore: bool) -> bool {
    ore || named(filters, item)
}

/// 0.21 (round 32): the portal dock from the magistral: only what the allow list of a joint to it
/// names. Before, on its default joint it took a share of everything that rode to the storage and
/// sent it to the other world. A cargo worker, a hopper and a hand still give it anything.
pub fn portal_dock_net_takes(filters: &[Filter], item: &str) -> bool {
    named(filters, item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> BTreeSet<String> {
        BTreeSet::new()
    }

    #[test]
    fn the_furnace_takes_dust_and_raw_food_only() {
        // The owner's birch logs went into charcoal.
        for item in ["minecraft:birch_log", "minecraft:sand", "minecraft:cobblestone", "minecraft:raw_iron"] {
            assert!(!furnace_takes(item, true, &none()), "{item}");
        }
        for item in ["rustcraft:dust_iron", "rustcraft:dust_gold", "minecraft:cod", "minecraft:beef", "minecraft:potato"] {
            assert!(furnace_takes(item, true, &none()), "{item}");
        }
        assert!(!furnace_takes("rustcraft:dust_", true, &none()));
        assert!(!furnace_takes("minecraft:cod", false, &none()), "only what it smelts");
    }

    #[test]
    fn raw_food_a_kitchen_cooks_raw_stays_out_of_the_furnace() {
        let kept: BTreeSet<String> = ["minecraft:cod".to_owned()].into();
        assert!(!furnace_takes("minecraft:cod", true, &kept));
        assert!(furnace_takes("minecraft:salmon", true, &kept));
        assert!(furnace_takes("rustcraft:dust_iron", true, &kept));
    }

    #[test]
    fn the_crusher_takes_ore_by_itself_and_blocks_only_by_name() {
        // MR3: the storage's cobblestone and wool must not all turn to gravel and string.
        assert!(crusher_net_takes(&[], "minecraft:raw_iron", true));
        for item in ["minecraft:cobblestone", "minecraft:white_wool", "minecraft:bone_block"] {
            assert!(!crusher_net_takes(&[], item, false), "{item}");
            assert!(!crusher_net_takes(&[Filter::default()], item, false), "{item}: no setting names nothing");
        }
        let bones = Filter { allow_only: true, items: ["minecraft:bone_block".to_owned()].into() };
        assert!(crusher_net_takes(&[bones], "minecraft:bone_block", false));
    }

    #[test]
    fn the_magistral_brings_fuel_made_to_burn() {
        assert!(net_fuel("minecraft:coal", true));
        assert!(net_fuel("minecraft:charcoal", true));
        for item in ["minecraft:oak_log", "minecraft:birch_planks", "minecraft:stick", "minecraft:oak_sapling"] {
            assert!(!net_fuel(item, true), "{item}");
        }
        assert!(!net_fuel("minecraft:coal", false));
    }

    #[test]
    fn an_empty_placer_takes_nothing_from_the_magistral() {
        assert!(!placer_net_takes(&[], "minecraft:cobblestone"));
        assert!(placer_net_takes(&["minecraft:cobblestone"], "minecraft:cobblestone"));
        assert!(!placer_net_takes(&["minecraft:cobblestone"], "minecraft:oak_log"));
    }

    #[test]
    fn the_disintegrator_and_the_enchanter_take_what_a_joint_names() {
        let open = Filter::default();
        let names = Filter { allow_only: true, items: ["minecraft:chest".to_owned()].into() };
        assert!(!disintegrator_net_takes(std::slice::from_ref(&open), "minecraft:chest"), "no setting names nothing");
        assert!(disintegrator_net_takes(&[open.clone(), names.clone()], "minecraft:chest"));
        assert!(!disintegrator_net_takes(std::slice::from_ref(&names), "minecraft:piston"));
        assert!(enchanter_net_takes(&[], "minecraft:book", true));
        assert!(!enchanter_net_takes(&[open], "minecraft:fishing_rod", true), "a repaired rod rides on");
        let rods = Filter { allow_only: true, items: ["minecraft:fishing_rod".to_owned()].into() };
        assert!(enchanter_net_takes(&[rods], "minecraft:fishing_rod", true));
    }

    #[test]
    fn the_enchanter_takes_books_into_an_empty_cell_and_lapis_for_them() {
        // 0.21: a stack of books in its cell waited for experience, and enchanted all at once.
        assert!(!enchanter_net_takes(&[], "minecraft:book", false));
        assert!(enchanter_net_takes(&[], "minecraft:book", true));
        assert!(!enchanter_lapis_net_takes(false), "an idle machine keeps no lapis");
        assert!(enchanter_lapis_net_takes(true));
    }

    #[test]
    fn recipe_machines_take_only_what_a_joint_names() {
        // 0.21: the alloy smelter took the gold and redstone that rode to the storage and waited
        // for coppersteel; the mixer took the dust the furnace was to smelt.
        let open = Filter::default();
        for item in ["minecraft:gold_ingot", "minecraft:redstone", "rustcraft:dust_gold", "minecraft:bone_meal"] {
            assert!(!recipe_net_takes(&[], item), "{item}");
            assert!(!recipe_net_takes(std::slice::from_ref(&open), item), "{item}: no setting names nothing");
        }
        let named_list = Filter {
            allow_only: true,
            items: ["rustcraft:coppersteel".to_owned(), "minecraft:gold_ingot".to_owned()].into(),
        };
        assert!(recipe_net_takes(&[open.clone(), named_list.clone()], "minecraft:gold_ingot"));
        assert!(!recipe_net_takes(std::slice::from_ref(&named_list), "minecraft:redstone"));
        let deny = Filter { allow_only: false, items: ["minecraft:redstone".to_owned()].into() };
        assert!(!recipe_net_takes(&[deny], "minecraft:gold_ingot"), "a deny list names nothing");
    }

    #[test]
    fn the_portal_dock_sends_only_what_a_joint_names() {
        let open = Filter::default();
        assert!(!portal_dock_net_takes(std::slice::from_ref(&open), "minecraft:cobblestone"));
        let cobble = Filter { allow_only: true, items: ["minecraft:cobblestone".to_owned()].into() };
        assert!(portal_dock_net_takes(&[open, cobble], "minecraft:cobblestone"));
    }
}
