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
/// names; a tool or armour on its way to the storage (a repaired rod) rides on.
pub fn enchanter_net_takes(filters: &[Filter], item: &str) -> bool {
    item == "minecraft:book" || named(filters, item)
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
        assert!(enchanter_net_takes(&[], "minecraft:book"));
        assert!(!enchanter_net_takes(&[open], "minecraft:fishing_rod"), "a repaired rod rides on");
        let rods = Filter { allow_only: true, items: ["minecraft:fishing_rod".to_owned()].into() };
        assert!(enchanter_net_takes(&[rods], "minecraft:fishing_rod"));
    }
}
