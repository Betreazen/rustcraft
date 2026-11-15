//! Slice 12: the last blocks of the guide (spec 12.2, 22, 25; rounds 14, 25, 28). Each is a machine
//! with a buffer of energy it fills from the net, as the charge dock and the turret do, a drawn
//! window (sheet) and slots; what it does is in the submodules, the glue only reads the world and
//! moves items.
//!
//! Numbers not in the spec are proposals for the playtest, like every number of the mod.

pub mod charge;
pub mod disassembly;
pub mod farm;
pub mod plan;
pub mod repair;
pub mod transit;
pub mod vein;
pub mod xp;

use std::ops::Range;

/// The slice-12 machines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Util {
    /// UT1–UT7: destroys what its filter takes.
    Trash,
    /// DS1–DS7: takes a crafted item apart into half its ingredients.
    Disintegrator,
    /// Guide `charger`: charges the mod's charged items.
    Charger,
    /// Guide `charge_plate`: charges the charged items of a player standing on it.
    ChargePlate,
    /// Spec 22: repairs tools, weapons and armour with their material.
    Repair,
    /// Spec 22: food recipes, chosen in its window.
    Kitchen,
    /// Spec 22: fishes in open water with a rod.
    Fishery,
    /// Spec 22: feeds the animals round it into love.
    Feeder,
    /// Spec 22: shears sheep and milks cows.
    Shearer,
    /// Spec 22: takes honey and combs from full hives.
    Apiary,
    /// Spec 22: brews potions.
    Brewer,
    /// Spec 22: keeps experience players give it and take back.
    XpTank,
    /// Spec 22: enchants with the experience of a condenser beside it and lapis.
    Enchanter,
    /// Spec 22, 24: brushes suspicious sand and gravel.
    Archeo,
    /// Spec 22: remembers a building and shows it as a ghost.
    Projector,
    /// Spec 22, BU1: builds a projector's building from its slots.
    Builder,
    /// Spec 12.2: sends what it is given to its pair in another world.
    PortalDock,
    /// Spec 4.9 (slice 13): grows ore round it out of stone and energy.
    Vein,
}

/// Which slots the net and hoppers reach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slots {
    pub count: usize,
    pub inputs: Range<usize>,
    pub outputs: Range<usize>,
}

/// The fixed numbers of a slice-12 machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    /// Its block id.
    pub block: &'static str,
    /// The energy it keeps, E.
    pub buffer: f64,
    /// The most it draws a second, E.
    pub draw: f64,
    /// Toggles in its window.
    pub toggles: usize,
    /// Rows of text in its window.
    pub rows: usize,
}

impl Util {
    pub const ALL: [Util; 18] = [
        Util::Trash,
        Util::Disintegrator,
        Util::Charger,
        Util::ChargePlate,
        Util::Repair,
        Util::Kitchen,
        Util::Fishery,
        Util::Feeder,
        Util::Shearer,
        Util::Apiary,
        Util::Brewer,
        Util::XpTank,
        Util::Enchanter,
        Util::Archeo,
        Util::Projector,
        Util::Builder,
        Util::PortalDock,
        Util::Vein,
    ];

    pub fn of(block: &str) -> Option<Util> {
        Util::ALL.into_iter().find(|u| u.spec().block == block)
    }

    pub fn spec(self) -> Spec {
        let s = |block, buffer, draw, toggles, rows| Spec {
            block,
            buffer,
            draw,
            toggles,
            rows,
        };
        match self {
            Util::Trash => s("rustcraft:trash", 200.0, 16.0, 0, 2),
            Util::Disintegrator => s("rustcraft:disintegrator", 400.0, 32.0, 0, 2),
            Util::Charger => s("rustcraft:charger", 1000.0, 128.0, 0, 2),
            Util::ChargePlate => s("rustcraft:charge_plate", 1000.0, 256.0, 0, 2),
            Util::Repair => s("rustcraft:repair", 400.0, 32.0, 0, 2),
            Util::Kitchen => s("rustcraft:kitchen", 400.0, 16.0, KITCHEN_RECIPES.len(), 2),
            Util::Fishery => s("rustcraft:fishery", 200.0, 8.0, 0, 2),
            Util::Feeder => s("rustcraft:feeder", 200.0, 8.0, 0, 2),
            Util::Shearer => s("rustcraft:shearer", 200.0, 8.0, 0, 2),
            Util::Apiary => s("rustcraft:apiary", 200.0, 8.0, 0, 2),
            Util::Brewer => s("rustcraft:brewer", 400.0, 16.0, 0, 2),
            Util::XpTank => s("rustcraft:xp_tank", 100.0, 4.0, 4, 2),
            Util::Enchanter => s("rustcraft:enchanter", 600.0, 32.0, 3, 2),
            Util::Archeo => s("rustcraft:archeo", 200.0, 16.0, ARCHEO_TABLES.len(), 2),
            Util::Projector => s("rustcraft:projector", 400.0, 16.0, 3, 2),
            Util::Builder => s("rustcraft:builder", 400.0, 32.0, 0, 2),
            Util::PortalDock => s("rustcraft:portal_dock", 400.0, 32.0, 0, 2),
            Util::Vein => s("rustcraft:vein", 4000.0, 64.0, 0, 2),
        }
    }

    /// Its slots and those the net reaches.
    pub fn slots(self) -> Slots {
        let s = |count, inputs, outputs| Slots {
            count,
            inputs,
            outputs,
        };
        match self {
            // 0..9 the filter (a player's), 9..18 what comes in to burn.
            Util::Trash => s(18, 9..18, 0..0),
            Util::Disintegrator => s(10, 0..1, 1..10),
            Util::Charger => s(8, 0..4, 4..8),
            Util::ChargePlate => s(1, 0..0, 0..0),
            // 0 the tool, 1..5 material, 5 the repaired tool.
            Util::Repair => s(6, 0..5, 5..6),
            Util::Kitchen => s(13, 0..9, 9..13),
            // 0 the rod, 1..10 the catch.
            Util::Fishery => s(10, 0..1, 1..10),
            Util::Feeder => s(9, 0..9, 0..0),
            // 0 shears, 1 empty buckets, 2..11 wool and milk.
            Util::Shearer => s(11, 0..2, 2..11),
            // 0 glass bottles, 1..10 honey and combs.
            Util::Apiary => s(10, 0..1, 1..10),
            // 0..3 bottles, 3 the ingredient, 4 blaze powder, 5..8 the potions.
            Util::Brewer => s(8, 0..5, 5..8),
            Util::XpTank => s(1, 0..0, 0..0),
            // 0 the item, 1 lapis, 2 the enchanted item.
            Util::Enchanter => s(3, 0..2, 2..3),
            // 0 suspicious blocks, 1 the brush, 2..11 the finds.
            Util::Archeo => s(11, 0..2, 2..11),
            Util::Projector => s(1, 0..0, 0..0),
            Util::Builder => s(18, 0..18, 0..0),
            // 0..18 what it sends, 18..27 what came from its pair.
            Util::PortalDock => s(27, 0..18, 18..27),
            Util::Vein => s(9, 0..9, 0..0),
        }
    }

    /// A machine that works on the animals or the world round it (radius module, K1-style).
    pub fn has_radius(self) -> bool {
        matches!(self, Util::Feeder | Util::Shearer | Util::Apiary)
    }

    /// Whether `item` may go into slot `slot` (a player's and the net's alike).
    pub fn takes(self, slot: usize, item: &str, filter: &[String]) -> bool {
        match (self, slot) {
            (Util::Trash, 0..=8) => true,
            (Util::Trash, _) => filter.iter().any(|f| f == item),
            (Util::Fishery, 0) => item == "minecraft:fishing_rod",
            (Util::Shearer, 0) => item == "minecraft:shears",
            (Util::Shearer, 1) => item == "minecraft:bucket",
            (Util::Apiary, 0) => item == "minecraft:glass_bottle",
            (Util::Brewer, 0..=2) => {
                matches!(item, "minecraft:potion" | "minecraft:splash_potion" | "minecraft:lingering_potion")
            }
            (Util::Brewer, 4) => item == "minecraft:blaze_powder",
            (Util::Enchanter, 1) => item == "minecraft:lapis_lazuli",
            (Util::Archeo, 0) => matches!(item, "minecraft:suspicious_sand" | "minecraft:suspicious_gravel"),
            (Util::Archeo, 1) => item == "minecraft:brush",
            (Util::Repair, 1..5) => repair::is_material(item),
            (Util::Feeder, _) => farm::is_feed(item),
            (Util::Vein, _) => vein::is_rock(item),
            _ => true,
        }
    }
}

/// The kitchen's recipes (spec 22: stews, soups, bread, pies and cakes), by result: each needs a
/// crafting recipe of the game with that result; its toggle in the window turns it on.
pub const KITCHEN_RECIPES: [&str; 9] = [
    "minecraft:bread",
    "minecraft:cake",
    "minecraft:pumpkin_pie",
    "minecraft:cookie",
    "minecraft:mushroom_stew",
    "minecraft:rabbit_stew",
    "minecraft:beetroot_soup",
    "minecraft:golden_carrot",
    "minecraft:golden_apple",
];

/// The archaeology table's loot tables (spec 24, ST2), one a toggle.
pub const ARCHEO_TABLES: [&str; 5] = [
    "minecraft:archaeology/desert_well",
    "minecraft:archaeology/desert_pyramid",
    "minecraft:archaeology/ocean_ruin_warm",
    "minecraft:archaeology/ocean_ruin_cold",
    "minecraft:archaeology/trail_ruins_common",
];

/// Ticks between two actions of a machine that acts in steps (feeding, shearing, a hive's
/// harvest, building a block, burning a batch).
pub fn step_ticks(u: Util) -> u64 {
    match u {
        Util::Trash => 20,
        Util::Disintegrator | Util::Repair | Util::Archeo | Util::Enchanter => 100,
        Util::Kitchen => 200,
        Util::Brewer => 400,
        Util::Feeder | Util::Shearer | Util::Apiary => 100,
        Util::Builder => 20,
        Util::PortalDock => 20,
        Util::Vein => vein::STEP_TICKS,
        Util::Charger | Util::ChargePlate | Util::XpTank | Util::Projector | Util::Fishery => 20,
    }
}

/// E one action costs.
pub fn step_energy(u: Util) -> f64 {
    match u {
        Util::Trash => 1.0,
        Util::Disintegrator => 80.0,
        Util::Repair => 80.0,
        Util::Kitchen => 40.0,
        Util::Brewer => 80.0,
        Util::Archeo => 40.0,
        Util::Enchanter => 200.0,
        Util::Feeder | Util::Shearer | Util::Apiary => 10.0,
        Util::Builder => 4.0,
        Util::PortalDock => 1.0,
        Util::Vein => vein::BLOCK_ENERGY,
        Util::Fishery => 20.0,
        Util::Charger | Util::ChargePlate | Util::XpTank | Util::Projector => 0.0,
    }
}

/// What a slice-12 machine shows in its status line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shown {
    Working,
    NoEnergy,
    /// Nothing to work on (its own words, a translation key's tail).
    Idle(&'static str),
    /// Something keeps it from working (its own words).
    Blocked(&'static str),
}

impl Shown {
    pub fn key(&self) -> &'static str {
        match self {
            Shown::Working => "working",
            Shown::NoEnergy => "no_energy",
            Shown::Idle(k) | Shown::Blocked(k) => k,
        }
    }

    pub fn working(&self) -> bool {
        matches!(self, Shown::Working)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_machine_has_a_block_of_its_own_and_sane_slots() {
        let mut blocks: Vec<_> = Util::ALL.iter().map(|u| u.spec().block).collect();
        blocks.sort_unstable();
        blocks.dedup();
        assert_eq!(blocks.len(), Util::ALL.len());
        for u in Util::ALL {
            let s = u.slots();
            assert!(s.count >= 1 && s.inputs.end <= s.count && s.outputs.end <= s.count, "{u:?}");
            assert_eq!(Util::of(u.spec().block), Some(u));
            assert!(u.spec().buffer > 0.0 && u.spec().draw > 0.0);
        }
    }

    #[test]
    fn the_trash_takes_only_what_its_filter_names() {
        // UT2: no filter, nothing.
        let none: Vec<String> = Vec::new();
        assert!(!Util::Trash.takes(9, "minecraft:dirt", &none));
        let f = vec!["minecraft:dirt".to_owned()];
        assert!(Util::Trash.takes(12, "minecraft:dirt", &f));
        assert!(!Util::Trash.takes(12, "minecraft:stone", &f));
        assert!(Util::Trash.takes(3, "minecraft:stone", &f), "its filter takes anything");
    }

    #[test]
    fn slots_with_a_purpose_refuse_the_rest() {
        assert!(Util::Fishery.takes(0, "minecraft:fishing_rod", &[]));
        assert!(!Util::Fishery.takes(0, "minecraft:stick", &[]));
        assert!(Util::Brewer.takes(4, "minecraft:blaze_powder", &[]));
        assert!(!Util::Brewer.takes(4, "minecraft:coal", &[]));
        assert!(Util::Archeo.takes(0, "minecraft:suspicious_sand", &[]));
        assert!(!Util::Archeo.takes(0, "minecraft:sand", &[]));
    }
}
