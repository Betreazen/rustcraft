//! Slice 12: the last blocks of the guide (spec 12.2, 22, 25; rounds 14, 25, 28). Each is a machine
//! with a buffer of energy it fills from the net, as the charge dock and the turret do, a drawn
//! window (sheet) and slots; what it does is in the submodules, the glue only reads the world and
//! moves items.
//!
//! Numbers not in the spec are proposals for the playtest, like every number of the mod.

pub mod brew;
pub mod charge;
pub mod disassembly;
pub mod farm;
pub mod found;
pub mod grown;
pub mod kitchen;
pub mod plan;
pub mod repair;
// 0.21 (spec 38): rods from the storage, and orders for them.
pub mod rods;
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
            // 0.21 (spec 39): a toggle a dish, all in one row; its rows (the dish it cooks, the milk in
            // the tank: a line, not a bar, so the window fits 240 pixels).
            Util::Kitchen => s("rustcraft:kitchen", 400.0, 16.0, KITCHEN_RECIPES.len(), 2),
            // Slice 18: its rows: the water; the rod and the wait (18 catch slots leave room for
            // two in 240 pixels).
            Util::Fishery => s("rustcraft:fishery", 200.0, 8.0, 0, 2),
            Util::Feeder => s("rustcraft:feeder", 200.0, 8.0, 0, 2),
            Util::Shearer => s("rustcraft:shearer", 200.0, 8.0, 0, 2),
            // Slice 18 (guide `apiary`): «забирать: мёд / соты», a choice of two.
            Util::Apiary => s("rustcraft:apiary", 200.0, 8.0, 2, 2),
            // Slice 18: four page buttons and five kind buttons for the chosen page.
            Util::Brewer => s("rustcraft:brewer", 400.0, 16.0, brew::PAGES_MAX + brew::Kind::ALL.len(), 2),
            // Slice 18: six buttons (give 1 / 10 / all, take 1 / 10 / all).
            Util::XpTank => s("rustcraft:xp_tank", 100.0, 4.0, xp::PRESSES.len(), 2),
            // Slice 18: the level buttons −10, −1, +1, +10.
            Util::Enchanter => s("rustcraft:enchanter", 600.0, 32.0, xp::LEVEL_STEPS.len(), 2),
            // Slice 18: the table is the block's own place, a line not a choice.
            Util::Archeo => s("rustcraft:archeo", 200.0, 16.0, 0, 2),
            // Slice 18: remember, ghost, forget, turn, mirror, «В руку».
            Util::Projector => s("rustcraft:projector", 400.0, 16.0, 6, 2),
            Util::Builder => s("rustcraft:builder", 400.0, 32.0, 0, 2),
            Util::PortalDock => s("rustcraft:portal_dock", 400.0, 32.0, 0, 2),
            Util::Vein => s("rustcraft:vein", 8000.0, 128.0, vein::PICKS.len(), 2),
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
            // Slice 18 (guide `repair`): 0..3 to repair, 3..5 material and two more for each
            // capacity module (3..11), 11..14 the repaired tools.
            Util::Repair => s(REPAIR_OUT.end, 0..REPAIR_MATERIAL.end, REPAIR_OUT),
            Util::Kitchen => s(13, 0..9, 9..13),
            // 0 the rod, 1..19 the catch (9 slots, 18 with a storage module).
            Util::Fishery => s(1 + 2 * farm::FISH_BUFFER, 0..1, 1..1 + 2 * farm::FISH_BUFFER),
            // Slice 18 (guide `feeder`): 3 food cells.
            Util::Feeder => s(3, 0..3, 0..0),
            // Slice 18 (guide `shearer`): its shears are built in and its milk goes to the tank
            // it touches; 0..9 the wool.
            Util::Shearer => s(9, 0..0, 0..9),
            // 0 glass bottles, 1..10 honey and combs.
            Util::Apiary => s(10, 0..1, 1..10),
            // Slice 18: 0..3 the bottles it brews, 3..7 its ingredients, 7 blaze powder, 8..11 the
            // finished potions.
            Util::Brewer => s(BREW_OUT.end, 0..BREW_FUEL + 1, BREW_OUT),
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

    /// Slice 18 (guide MODES): the machines with the «Экономный / Обычный / Форсаж» switch: the
    /// repair stand, the kitchen, the brewer, the enchanting machine, the archaeology table and
    /// the building machine. The farm machines' pace is the animals' or the water's.
    pub fn has_modes(self) -> bool {
        matches!(
            self,
            Util::Repair | Util::Kitchen | Util::Brewer | Util::Enchanter | Util::Archeo | Util::Builder
        )
    }

    /// Slice 18 (guide SLOTS_FROM): the tier a machine opens at; a block of it saves its tier under
    /// `tier` once stream 3's swap makes higher ones, and this is the tier of one that saved none.
    pub fn opening_tier(self) -> u8 {
        match self {
            Util::Brewer | Util::XpTank | Util::Archeo | Util::Projector | Util::Builder => 3,
            Util::Enchanter | Util::ChargePlate | Util::PortalDock => 4,
            Util::Charger | Util::Disintegrator | Util::Vein => 3,
            _ => 2,
        }
    }

    /// Slice 18 (guide `ui.js`): the bars of its window: energy, and the repair stand's
    /// durability, the shearer's milk, the condenser's experience, the building machine's
    /// progress (the kitchen's milk and the brewer's fuel are lines: their windows have no room
    /// for a bar in 240 pixels).
    pub fn bars(self) -> usize {
        match self {
            Util::Repair | Util::Shearer | Util::XpTank | Util::Builder => 2,
            _ => 1,
        }
    }

    /// Slice 18: the fishing station, the feeder, the shearer and the apiary collector: the water
    /// or the animals set their pace, so a speed module does nothing for them.
    pub fn paced_by_nature(self) -> bool {
        matches!(self, Util::Fishery | Util::Feeder | Util::Shearer | Util::Apiary)
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
            (Util::Apiary, 0) => item == "minecraft:glass_bottle",
            (Util::Brewer, s) if BREW_BOTTLES.contains(&s) => {
                matches!(item, "minecraft:potion" | "minecraft:splash_potion" | "minecraft:lingering_potion")
            }
            (Util::Brewer, BREW_FUEL) => item == "minecraft:blaze_powder",
            (Util::Brewer, s) if BREW_INGREDIENTS.contains(&s) => brew::is_ingredient(item),
            (Util::Enchanter, 1) => item == "minecraft:lapis_lazuli",
            (Util::Archeo, 0) => matches!(item, "minecraft:suspicious_sand" | "minecraft:suspicious_gravel"),
            (Util::Archeo, 1) => item == "minecraft:brush",
            // What is repaired is no material (so a hopper's string goes to the material).
            (Util::Repair, s) if REPAIR_TOOLS.contains(&s) => !repair::is_material(item),
            (Util::Repair, s) if REPAIR_MATERIAL.contains(&s) => {
                s < repair_material_end(capacity_in(filter)) && repair::is_material(item)
            }
            (Util::Feeder, _) => farm::is_feed(item),
            (Util::Vein, _) => vein::is_rock(item),
            _ => true,
        }
    }
}

/// Slice 18: the brewing vat's slots.
pub const BREW_BOTTLES: Range<usize> = 0..3;
pub const BREW_INGREDIENTS: Range<usize> = 3..7;
pub const BREW_FUEL: usize = 7;
pub const BREW_OUT: Range<usize> = 8..11;

/// Slice 18: the repair stand's slots: what it repairs, its material (two cells and two more a
/// capacity module, at most three), what it repaired.
pub const REPAIR_TOOLS: Range<usize> = 0..3;
pub const REPAIR_MATERIAL: Range<usize> = 3..11;
pub const REPAIR_OUT: Range<usize> = 11..14;

/// The end of the material cells a stand with `capacity` capacity modules uses.
pub fn repair_material_end(capacity: u32) -> usize {
    (REPAIR_MATERIAL.start + 2 + 2 * capacity as usize).min(REPAIR_MATERIAL.end)
}

/// The capacity modules among a machine's module items, which its slot rule gets in `filter`
/// (the bundle passes a capacity machine's module items there, the trash's filter otherwise).
pub fn capacity_in(items: &[String]) -> u32 {
    items.iter().filter(|i| i.as_str() == crate::storage::modules::CAPACITY).count() as u32
}

/// Slice 18, wave 2 (guide `mod_cap`: «+2 ячейки очереди»): the hidden queue cells a capacity
/// module gives the kitchen and the brewer behind their ingredient slots, two each, as the
/// furnace's (`machines::queue::intake` moves into them). The other slice-12 machines have none.
pub fn queue_cells(u: Util, slots: &[Option<crate::modules::Module>]) -> usize {
    match u {
        Util::Kitchen | Util::Brewer => crate::modules::capacity_slots(slots),
        _ => 0,
    }
}

/// A machine whose slot rule reads its capacity modules.
pub fn has_capacity(u: Util) -> bool {
    matches!(u, Util::Repair | Util::Kitchen | Util::Brewer)
}

/// The kitchen's recipes (spec 22: stews, soups, bread, pies and cakes), by result: each needs a
/// crafting recipe of the game with that result; its toggle in the window turns it on.
/// Slice 18: the handbook's dishes only (bread, cookies, pumpkin pie, cake, the soups and the
/// rabbit stew); a page chooses one.
pub const KITCHEN_RECIPES: [&str; 7] = [
    "minecraft:bread",
    "minecraft:cookie",
    "minecraft:pumpkin_pie",
    "minecraft:cake",
    "minecraft:mushroom_stew",
    "minecraft:beetroot_soup",
    "minecraft:rabbit_stew",
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
        Util::Disintegrator | Util::Repair => 100,
        // Slice 18 (guide): 10 s a block, 10 s an item.
        Util::Archeo | Util::Enchanter => 200,
        // Slice 18 (guide `kitchen`): 4 s a dish.
        Util::Kitchen => 80,
        Util::Brewer => 400,
        // Slice 18 (guide `feeder`): a pair every 20 s.
        Util::Feeder => 400,
        Util::Shearer | Util::Apiary => 100,
        // Slice 18 (guide `builder`): 2 blocks a second.
        Util::Builder => 10,
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
        // Wave 2 (guide «Расход … Е/с» in the normal mode, over the step's seconds): the stand
        // 8 E/s × 5 s, the kitchen 8 × 4, the vat 8 × 20, the table 8 × 10, the enchanting
        // machine 32 × 10, the feeder 2 × 20, the shearer 4 × 5, the apiary 2 × 5.
        Util::Repair => 40.0,
        Util::Kitchen => 32.0,
        Util::Brewer => 160.0,
        Util::Archeo => 80.0,
        Util::Enchanter => 320.0,
        Util::Feeder => 40.0,
        Util::Shearer => 20.0,
        Util::Apiary => 10.0,
        // 16 E/s in the normal mode.
        Util::Builder => 8.0,
        Util::PortalDock => 1.0,
        Util::Vein => vein::BLOCK_ENERGY,
        // Wave 2: 4 E/s, a step a second while it fishes (the wait and the catch alike).
        Util::Fishery => 4.0,
        // Slice 18: the projector 4 E/s while its ghost shows (a step a second).
        Util::Projector => 4.0,
        Util::Charger | Util::ChargePlate | Util::XpTank => 0.0,
    }
}

/// Slice 18 (guide SLOTS_BY_TIER, `modules`): module sockets of a machine of `tier`: three on the
/// first and second, four on the third and fourth.
pub fn sockets_for_tier(tier: u8) -> usize {
    if tier >= 3 { 4 } else { 3 }
}

/// Wave 2 (guide `machine`: «машина следующей ступени работает быстрее»): ticks between the steps
/// of a tiered craft machine on `tier`, `tiers::speed` times fewer than on its first tier with
/// `step` (`Tuning::tier_speed_step`): the repair stand and the kitchen. A machine the water or
/// the animals pace (the farms) keeps its pace, and the trash, the disintegrator and the charger
/// have their own numbers a tier (`machines::tiered`, `rustcraft_utility::charge_slots`).
pub fn tier_ticks(u: Util, ticks: u64, tier: u8, step: f64) -> u64 {
    if !matches!(u, Util::Repair | Util::Kitchen) {
        return ticks;
    }
    match crate::tiers::family_of_block(u.spec().block) {
        Some(f) => ((ticks as f64 / crate::tiers::speed(f, tier, step)).round() as u64).max(1),
        None => ticks,
    }
}

/// (ticks between steps, energy a step) after the work mode and the modules: speed modules halve
/// the wait and cost ×2.5 each, economy ×0.6 each (A7, A9); the mode's speed divides the wait
/// and its energy factor multiplies the cost (guide MODES). A machine without modes works in
/// `Normal`.
pub fn paced(u: Util, mode: crate::machines::processor::WorkMode, speed: u32, economy: u32) -> (u64, f64) {
    let mode = if u.has_modes() { mode } else { crate::machines::processor::WorkMode::Normal };
    // The farm machines' pace is the water's or the animals' (guide: no speed module for them).
    let speed = if u.paced_by_nature() { 0 } else { speed };
    let (fast, dear) = mode.factors();
    let ticks = ((step_ticks(u) >> speed.min(4)) as f64 / fast).round().max(1.0) as u64;
    let cost = step_energy(u) * 2.5f64.powi(speed as i32) * 0.6f64.powi(economy as i32) * dear;
    (ticks, cost)
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
    fn each_menu_file_has_as_many_slots_as_its_machine() {
        // The game opens a machine's window only when its menu file has the container's slot count:
        // the in-game check found the repair stand, fishery, feeder, shearer and brewer windows
        // not opening after their slots changed in slice 18.
        for u in Util::ALL {
            let name = u.spec().block.trim_start_matches("rustcraft:");
            let path = format!(
                "{}/../../content/data/rustcraft/horizon/menu/{name}.json",
                env!("CARGO_MANIFEST_DIR")
            );
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(json["slots"], u.slots().count, "{path}");
        }
    }

    #[test]
    fn a_capacity_module_gives_the_kitchen_and_brewer_two_queue_cells() {
        use crate::modules::Module;
        let two = [Some(Module::Capacity), Some(Module::Capacity), None];
        assert_eq!(queue_cells(Util::Kitchen, &two), 4);
        assert_eq!(queue_cells(Util::Brewer, &two[..1]), 2);
        assert_eq!(queue_cells(Util::Trash, &two), 0);
    }

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
    fn the_repair_stand_takes_material_in_two_cells_and_two_more_a_capacity_module() {
        let none: Vec<String> = Vec::new();
        assert!(Util::Repair.takes(3, "minecraft:iron_ingot", &none));
        assert!(Util::Repair.takes(4, "minecraft:string", &none));
        assert!(!Util::Repair.takes(5, "minecraft:iron_ingot", &none));
        let one = vec![crate::storage::modules::CAPACITY.to_owned()];
        assert!(Util::Repair.takes(6, "minecraft:iron_ingot", &one));
        assert!(!Util::Repair.takes(7, "minecraft:iron_ingot", &one));
        assert!(!Util::Repair.takes(3, "minecraft:dirt", &none));
        assert_eq!(Util::Repair.slots().count, 14);
    }

    #[test]
    fn craft_machines_have_tiers_three_and_four() {
        // Wave 2: SLOTS_FROM(2) — 3 / 4 / 4 sockets on tiers 2 / 3 / 4.
        for block in [
            "rustcraft:repair",
            "rustcraft:kitchen",
            "rustcraft:fishery",
            "rustcraft:feeder",
            "rustcraft:butcher",
            "rustcraft:shearer",
            "rustcraft:apiary",
        ] {
            let f = crate::tiers::family_of_block(block).unwrap_or_else(|| panic!("{block}"));
            assert_eq!(f.first, 2);
            assert_eq!(f.item(4), Some(format!("{block}_4")));
            assert_eq!(crate::machines::sockets::open(block, Some(4)), 4);
            assert_eq!(crate::machines::sockets::open(block, Some(2)), 3);
        }
    }

    #[test]
    fn tier_speeds_a_craft_machine_but_not_a_farm() {
        assert_eq!(tier_ticks(Util::Kitchen, 80, 2, 0.5), 80);
        assert_eq!(tier_ticks(Util::Kitchen, 80, 4, 0.5), 40);
        assert_eq!(tier_ticks(Util::Repair, 100, 3, 0.5), 67);
        assert_eq!(tier_ticks(Util::Feeder, 400, 4, 0.5), 400, "the animals set its pace");
        assert_eq!(tier_ticks(Util::Brewer, 400, 4, 0.5), 400, "no tiers of its own");
        assert_eq!(tier_ticks(Util::Disintegrator, 60, 4, 0.5), 60, "its own numbers a tier");
    }

    #[test]
    fn energy_a_second_is_the_handbooks() {
        // Wave 2: «Расход … Е/с» of data.js in the normal mode: a step's energy over its seconds.
        let per_second = |u: Util| step_energy(u) / (step_ticks(u) as f64 / 20.0);
        for (u, e) in [
            (Util::Repair, 8.0),
            (Util::Kitchen, 8.0),
            (Util::Brewer, 8.0),
            (Util::Enchanter, 32.0),
            (Util::Archeo, 8.0),
            (Util::Fishery, 4.0),
            (Util::Feeder, 2.0),
            (Util::Shearer, 4.0),
            (Util::Apiary, 2.0),
            (Util::Builder, 16.0),
            (Util::Projector, 4.0),
        ] {
            assert!((per_second(u) - e).abs() < 1e-9, "{u:?}: {} E/s, not {e}", per_second(u));
            assert!(step_energy(u) <= u.spec().buffer, "{u:?}: its buffer holds a step");
        }
    }

    #[test]
    fn every_machine_window_fits_the_screen() {
        // Slice 18: a window must fit 240 GUI pixels (1280 × 720 at GUI scale 3).
        use crate::machines::sheet::{Kind, MAX_HEIGHT, layout};
        for u in Util::ALL {
            let h = layout(Kind::Util(u).shape()).height;
            assert!(h <= MAX_HEIGHT, "{u:?}: {h}");
        }
    }

    #[test]
    fn sockets_grow_with_the_tier() {
        assert_eq!(sockets_for_tier(Util::Repair.opening_tier()), 3);
        assert_eq!(sockets_for_tier(Util::Brewer.opening_tier()), 4);
        assert_eq!(sockets_for_tier(Util::Enchanter.opening_tier()), 4);
        assert_eq!(sockets_for_tier(2), 3);
    }

    #[test]
    fn the_work_mode_paces_the_machines_that_have_one() {
        use crate::machines::processor::WorkMode;
        let (t, e) = paced(Util::Repair, WorkMode::Normal, 0, 0);
        assert_eq!((t, e), (step_ticks(Util::Repair), step_energy(Util::Repair)));
        let (t, e) = paced(Util::Repair, WorkMode::Boost, 0, 0);
        assert_eq!(t, step_ticks(Util::Repair) / 2);
        assert!((e - step_energy(Util::Repair) * 3.0).abs() < 1e-9);
        let (t, e) = paced(Util::Repair, WorkMode::Economy, 0, 0);
        assert_eq!(t, step_ticks(Util::Repair) * 2);
        assert!((e - step_energy(Util::Repair) * 0.35).abs() < 1e-9);
        // No modes: the feeder keeps its pace whatever is saved.
        assert_eq!(paced(Util::Feeder, WorkMode::Boost, 0, 0).0, step_ticks(Util::Feeder));
        assert!(Util::Builder.has_modes() && !Util::Fishery.has_modes());
        assert_eq!(paced(Util::Feeder, WorkMode::Normal, 2, 0).0, step_ticks(Util::Feeder), "no speed");
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
