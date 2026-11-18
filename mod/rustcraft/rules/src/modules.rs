//! The module and upgrade items (spec 18.3 A5–A9, 27 TR1–TR4, guide `modules`, `upgrades`): what
//! each item is, which blocks take it, how many sockets a block has, and the numbers of each
//! module's effect. Two modules of a kind in one block stack in full (A9); a limit holds only where
//! the guide names one.
//!
//! This file is the one place the module sets live (slice 18, stream 4). Every holder's list is the
//! `MODS(...)` row of its entry in `docs/showcase/js/data.js`; the blocks' own code calls the effect
//! functions below (the contract is in `docs/superpowers/plans/s18-module.md`).

use serde::{Deserialize, Serialize};

use crate::tuning::Tuning;
use crate::utility::Util;
use crate::worker::Tool;

/// A module or an upgrade, by what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Module {
    /// Common to machines (A7): work ×2, energy ×2.5.
    Speed,
    /// Common to machines (A7): energy ×0.6, work ×0.85.
    Economy,
    /// Zone ×2: the worker's tool, the turret, the anchor, the farm machines (A7, TR4, B5).
    Radius,
    /// Storage ×2: the miner, the fishery, the worker's hold (guide `mod_storage`).
    Storage,
    /// Turret effect modules (TR1).
    Bleed,
    Poison,
    Slow,
    /// Turret support modules (TR3).
    Heal,
    Guard,
    Power,
    Haste,
    /// Worker upgrades (spec 4.7, guide `upgrades`): a bigger battery, a faster tool.
    Battery,
    WorkSpeed,
    /// Slice 8 (A7): a battery structure holds +50% and gives +50% faster a module.
    StoreCapacity,
    Output,
    /// The miner (A7): scans ×2 a module; its square 11 → 22 → 33.
    Scout,
    Area,
    /// Slice 13: the living vein's rare ores ×2 a module.
    Rare,
    /// Slice 13: the vein grows only the ore its window chooses.
    Pick,
    /// Slice 13 (ST1–ST5): the vein grows suspicious sand or gravel from its sand or gravel.
    Stratum,
    /// Slice 14, the bore's (guide `tool_bore`): a wider tunnel, the way kept on its level, lava
    /// cleared without the hold's blocks, the walls and floor made good.
    Tunnel,
    Height,
    Lava,
    Build,
    /// Slice 15 (O1): the storage terminal's recipe book, which takes orders.
    Recipes,
    /// Slice 18 (guide `mod_cap`): two more ingredient queue slots, a recipe page more.
    Capacity,
    /// Slice 18 (guide `mod_stack`, S3): the stack module of sparksteel, crystallite and starsteel:
    /// a machine's slot holds 256; the storage 256 / 1024 / 4096; the backpack up to 1024.
    Stack,
    Stack2,
    Stack3,
    /// Slice 18 (guide `mod_yield`, DS5): a 15% chance of a free ingredient or an extra product; the
    /// disintegrator's share 50 → 60 → 70%.
    Yield,
    /// Slice 18 (guide `mod_line`): the node's stretch of channels carries +50%.
    Line,
}

/// Every module with its item id.
pub const ITEMS: [(Module, &str); 31] = [
    (Module::Speed, "rustcraft:module_speed"),
    (Module::Economy, "rustcraft:module_economy"),
    (Module::Radius, "rustcraft:module_radius"),
    (Module::Storage, "rustcraft:module_storage"),
    (Module::Bleed, "rustcraft:module_bleed"),
    (Module::Poison, "rustcraft:module_poison"),
    (Module::Slow, "rustcraft:module_slow"),
    (Module::Heal, "rustcraft:module_heal"),
    (Module::Guard, "rustcraft:module_guard"),
    (Module::Power, "rustcraft:module_power"),
    (Module::Haste, "rustcraft:module_haste"),
    (Module::Battery, "rustcraft:upgrade_battery"),
    (Module::WorkSpeed, "rustcraft:upgrade_speed"),
    (Module::StoreCapacity, "rustcraft:module_store_cap"),
    (Module::Output, "rustcraft:module_output"),
    (Module::Scout, "rustcraft:module_scout"),
    (Module::Area, "rustcraft:module_area"),
    (Module::Rare, "rustcraft:module_rare"),
    (Module::Pick, "rustcraft:module_pick"),
    (Module::Stratum, "rustcraft:module_stratum"),
    (Module::Tunnel, "rustcraft:module_tunnel"),
    (Module::Height, "rustcraft:module_height"),
    (Module::Lava, "rustcraft:module_lava"),
    (Module::Build, "rustcraft:module_build"),
    (Module::Recipes, "rustcraft:module_recipes"),
    (Module::Capacity, "rustcraft:module_capacity"),
    (Module::Stack, "rustcraft:module_stack"),
    (Module::Stack2, "rustcraft:module_stack_2"),
    (Module::Stack3, "rustcraft:module_stack_3"),
    (Module::Yield, "rustcraft:module_yield"),
    (Module::Line, "rustcraft:module_line"),
];

impl Module {
    pub fn of(item: &str) -> Option<Module> {
        ITEMS.iter().find(|(_, i)| *i == item).map(|(m, _)| *m)
    }

    pub fn item(self) -> &'static str {
        ITEMS
            .iter()
            .find(|(m, _)| *m == self)
            .map_or("", |(_, i)| i)
    }

    /// TR1: adds an effect to each shot of an attack turret.
    pub fn is_effect(self) -> bool {
        matches!(self, Module::Bleed | Module::Poison | Module::Slow)
    }

    /// TR3: switches a turret to support.
    pub fn is_support(self) -> bool {
        matches!(
            self,
            Module::Heal | Module::Guard | Module::Power | Module::Haste
        )
    }

    /// The tier of a stack module (1 sparksteel, 2 crystallite, 3 starsteel), 0 for any other.
    pub fn stack_tier(self) -> u8 {
        match self {
            Module::Stack => 1,
            Module::Stack2 => 2,
            Module::Stack3 => 3,
            _ => 0,
        }
    }

    pub fn is_stack(self) -> bool {
        self.stack_tier() > 0
    }
}

/// The blocks and carts that take modules, each with its own list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    Turret,
    Anchor,
    /// A worker's chassis, whatever its tool (see [`tool_uses`] for what works with which tool).
    Worker,
    Battery,
    Miner,
    /// The block placer and the block breaker.
    Placer,
    Mixer,
    Butcher,
    /// The slice-12 blocks.
    Util(Util),
    /// Slice 18: the electric furnace, the crusher and the alloy smelter.
    Processor,
    /// Slice 18: the pump.
    Pump,
    /// Slice 18: the autocrafter.
    Crafter,
    /// Slice 18: the magistral node.
    Node,
    /// Slice 18: the storage terminal.
    Terminal,
    /// Slice 18: the backpack.
    Backpack,
}

/// Speed and economy: the common pair of most machines.
const COMMON: &[Module] = &[Module::Speed, Module::Economy];

impl Holder {
    /// Every module this holder's sockets take, in the guide's order.
    pub fn accepts(self) -> &'static [Module] {
        use Module as M;
        match self {
            Holder::Turret => &[
                M::Bleed, M::Poison, M::Slow, M::Heal, M::Guard, M::Power, M::Haste, M::Speed,
                M::Economy, M::Radius,
            ],
            Holder::Anchor => &[M::Radius, M::Economy],
            Holder::Worker => &[
                M::Battery, M::WorkSpeed, M::Storage, M::Radius, M::Tunnel, M::Height, M::Lava,
                M::Build,
            ],
            Holder::Battery => &[M::StoreCapacity, M::Output],
            Holder::Miner => &[
                M::Speed, M::Economy, M::Scout, M::Area, M::Storage, M::Stack, M::Stack2, M::Stack3,
            ],
            Holder::Placer | Holder::Pump => COMMON,
            Holder::Mixer | Holder::Crafter => &[
                M::Speed, M::Economy, M::Capacity, M::Stack, M::Stack2, M::Stack3,
            ],
            Holder::Butcher => &[M::Economy, M::Radius],
            Holder::Processor => &[
                M::Speed, M::Economy, M::Capacity, M::Stack, M::Stack2, M::Stack3, M::Yield,
            ],
            Holder::Node => &[M::Line],
            Holder::Terminal => &[M::Stack, M::Stack2, M::Stack3, M::Recipes],
            Holder::Backpack => &[M::Stack, M::Stack2, M::Stack3],
            Holder::Util(u) => util_accepts(u),
        }
    }

    /// Whether a module of `kind` goes into this holder's slots.
    pub fn takes(self, kind: Module) -> bool {
        self.accepts().contains(&kind)
    }

    /// Whether the block has module sockets at all (a window without them draws none).
    pub fn has_sockets(self) -> bool {
        !self.accepts().is_empty()
    }
}

/// The slice-12 blocks' lists (guide entries `trash` … `builder`, `vein`).
fn util_accepts(u: Util) -> &'static [Module] {
    use Module as M;
    match u {
        Util::Trash | Util::Charger | Util::Enchanter | Util::Archeo | Util::Builder => COMMON,
        Util::Disintegrator => &[M::Speed, M::Economy, M::Yield],
        Util::Repair => &[M::Speed, M::Economy, M::Capacity],
        Util::Kitchen | Util::Brewer => &[
            M::Speed, M::Economy, M::Capacity, M::Stack, M::Stack2, M::Stack3,
        ],
        // The animal and fishing machines: their pace is the animals' and the water's, so no speed.
        Util::Fishery => &[M::Economy, M::Storage],
        Util::Feeder | Util::Shearer | Util::Apiary => &[M::Economy, M::Radius],
        Util::Vein => &[M::Speed, M::Economy, M::Rare, M::Pick, M::Stratum],
        // The guide gives these no modules.
        Util::ChargePlate | Util::XpTank | Util::Projector | Util::PortalDock => &[],
    }
}

/// Whether a chassis module works with `tool` (guide `tool_*` entries): storage and radius for the
/// saw, reaper, grab and scoop; the corridor, height, lava, build and storage modules for the bore;
/// radius for the cartographer. Upgrades work with every tool.
pub fn tool_uses(tool: Tool, kind: Module) -> bool {
    use Module as M;
    if matches!(kind, M::Battery | M::WorkSpeed) {
        return true;
    }
    match tool {
        Tool::Saw | Tool::Reaper | Tool::Grab | Tool::Scoop => matches!(kind, M::Radius | M::Storage),
        Tool::Bore => matches!(kind, M::Tunnel | M::Height | M::Lava | M::Build | M::Storage),
        Tool::Map => kind == M::Radius,
        Tool::Hold | Tool::Shooter => false,
    }
}

// ---- Sockets (A5, guide `swap` table) ----

/// The most module sockets a machine has (tiers 3 and 4).
pub const MAX_SOCKETS: usize = 4;
/// The most sockets a worker's chassis has (the starsteel one).
pub const MAX_CHASSIS_SOCKETS: usize = 7;

/// Module sockets of a machine of `tier` (1–4): 3 / 3 / 4 / 4.
pub fn sockets(tier: u8) -> usize {
    if tier >= 3 { 4 } else { 3 }
}

/// Sockets of a worker's chassis of `tier` (2 sparksteel, 3 crystallite, 4 starsteel): 3 / 5 / 7.
pub fn chassis_sockets(tier: u8) -> usize {
    match tier {
        0..=2 => 3,
        3 => 5,
        _ => 7,
    }
}

// ---- Reading the slots ----

/// How many of `kind` the slots hold.
pub fn count(slots: &[Option<Module>], kind: Module) -> u32 {
    slots.iter().flatten().filter(|m| **m == kind).count() as u32
}

/// The modules in `items` (item ids or nothing), unknown items left out.
pub fn read(items: &[Option<String>]) -> Vec<Option<Module>> {
    items
        .iter()
        .map(|i| i.as_deref().and_then(Module::of))
        .collect()
}

// ---- Effects (A7, A9 and each module's guide entry) ----

/// (work, energy) factors of the speed and economy modules: speed ×2 work for ×2.5 energy, economy
/// ×0.85 work for ×0.6 energy, each module in full. A work mode multiplies on top.
pub fn factors(slots: &[Option<Module>], t: &Tuning) -> (f64, f64) {
    let speed = count(slots, Module::Speed) as i32;
    let eco = count(slots, Module::Economy) as i32;
    (
        t.speed_work.powi(speed) * t.economy_work.powi(eco),
        t.speed_energy.powi(speed) * t.economy_energy.powi(eco),
    )
}

/// Ticks between a machine's steps after its modules: `step_ticks` over the work factor, at least 1.
pub fn paced_ticks(step_ticks: u64, slots: &[Option<Module>], t: &Tuning) -> u64 {
    let (work, _) = factors(slots, t);
    ((step_ticks as f64 / work).round() as u64).max(1)
}

/// Ingredient queue slots the capacity modules add: two each (guide `mod_cap`).
pub fn capacity_slots(slots: &[Option<Module>]) -> usize {
    2 * count(slots, Module::Capacity) as usize
}

/// Recipe pages the capacity modules add to an autocrafter, kitchen or brewer: one each.
pub fn capacity_pages(slots: &[Option<Module>]) -> usize {
    count(slots, Module::Capacity) as usize
}

/// The highest stack module tier in the slots, 0 for none.
pub fn stack_tier(slots: &[Option<Module>]) -> u8 {
    slots.iter().flatten().map(|m| m.stack_tier()).max().unwrap_or(0)
}

/// A machine's or the miner's slot size for an item the game stacks to `base_max`: any stack
/// module makes 64 into 256 (and 16 into 64); a single item stays single.
pub fn machine_stack(slots: &[Option<Module>], base_max: u32) -> u32 {
    if base_max <= 1 || stack_tier(slots) == 0 {
        base_max
    } else {
        base_max * 4
    }
}

/// The most a backpack's slot stacks (guide `mod_stack`).
pub const BACKPACK_STACK_MAX: u32 = 1024;

/// A backpack's slot size: as the storage's for the module's tier, at most 1024 for a 64 item.
pub fn backpack_stack(slots: &[Option<Module>], base_max: u32) -> u32 {
    if base_max <= 1 {
        return base_max;
    }
    let table = crate::storage::STACK_BY_MODULE;
    let module = table[usize::from(stack_tier(slots)).min(table.len() - 1)].min(BACKPACK_STACK_MAX);
    base_max * module / 64
}

/// The yield module's chance for one operation: 15% a module, at most certain.
pub const YIELD_CHANCE: f64 = 0.15;

pub fn yield_chance(slots: &[Option<Module>]) -> f64 {
    (YIELD_CHANCE * f64::from(count(slots, Module::Yield))).min(1.0)
}

/// Whether this operation is lucky, for a roll in 0..1.
pub fn yield_lucky(slots: &[Option<Module>], roll: f64) -> bool {
    roll < yield_chance(slots)
}

/// The storage modules' factor: ×2 each (guide `mod_storage`).
pub fn storage_factor(slots: &[Option<Module>]) -> usize {
    1usize << count(slots, Module::Storage).min(8)
}

/// The line modules' factor on the throughput of a node's stretch: +50% each (guide `mod_line`).
pub fn line_factor(slots: &[Option<Module>]) -> f64 {
    1.0 + 0.5 * f64::from(count(slots, Module::Line))
}

/// The bore's height module (guide `mod_height`): how far the next rail climbs (+1), drops (−1) or
/// stays (0) to reach the height `target` set in the chassis window, from a rail at `y`. Without the
/// module or a set height the way stays on its level.
pub fn height_step(module: bool, target: Option<i32>, y: i32) -> i32 {
    match target {
        Some(t) if module => (t - y).signum(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Module as M;

    const STACKS: [Module; 3] = [M::Stack, M::Stack2, M::Stack3];

    fn all_modules() -> Vec<Module> {
        ITEMS.iter().map(|(m, _)| *m).collect()
    }

    /// The holder takes exactly `set` and nothing else.
    fn exactly(h: Holder, set: &[Module]) {
        for m in all_modules() {
            assert_eq!(h.takes(m), set.contains(&m), "{h:?} and {m:?}");
        }
    }

    fn with(base: &[Module], stack: bool) -> Vec<Module> {
        let mut v = base.to_vec();
        if stack {
            v.extend(STACKS);
        }
        v
    }

    #[test]
    fn every_module_has_one_item_and_back() {
        for (m, item) in ITEMS {
            assert_eq!(Module::of(item), Some(m));
            assert_eq!(m.item(), item);
        }
        assert_eq!(Module::of("minecraft:stone"), None);
        let mut items: Vec<_> = ITEMS.iter().map(|(_, i)| *i).collect();
        items.sort();
        items.dedup();
        assert_eq!(items.len(), ITEMS.len());
    }

    #[test]
    fn each_holder_takes_exactly_the_handbook_set() {
        // data.js, the MODS(...) row of each entry.
        exactly(Holder::Processor, &with(&[M::Speed, M::Economy, M::Capacity, M::Yield], true));
        exactly(Holder::Pump, &[M::Speed, M::Economy]);
        exactly(Holder::Crafter, &with(&[M::Speed, M::Economy, M::Capacity], true));
        exactly(Holder::Mixer, &with(&[M::Speed, M::Economy, M::Capacity], true));
        exactly(Holder::Placer, &[M::Speed, M::Economy]);
        exactly(Holder::Battery, &[M::StoreCapacity, M::Output]);
        exactly(Holder::Node, &[M::Line]);
        exactly(Holder::Miner, &with(&[M::Speed, M::Economy, M::Scout, M::Area, M::Storage], true));
        exactly(Holder::Terminal, &with(&[M::Recipes], true));
        exactly(Holder::Backpack, &STACKS);
        exactly(Holder::Anchor, &[M::Radius, M::Economy]);
        exactly(Holder::Butcher, &[M::Economy, M::Radius]);
        exactly(
            Holder::Turret,
            &[M::Bleed, M::Poison, M::Slow, M::Heal, M::Guard, M::Power, M::Haste, M::Speed, M::Economy, M::Radius],
        );
        exactly(
            Holder::Worker,
            &[M::Battery, M::WorkSpeed, M::Storage, M::Radius, M::Tunnel, M::Height, M::Lava, M::Build],
        );
        let common = [M::Speed, M::Economy];
        for u in [Util::Trash, Util::Charger, Util::Enchanter, Util::Archeo, Util::Builder] {
            exactly(Holder::Util(u), &common);
        }
        exactly(Holder::Util(Util::Disintegrator), &[M::Speed, M::Economy, M::Yield]);
        exactly(Holder::Util(Util::Repair), &[M::Speed, M::Economy, M::Capacity]);
        exactly(Holder::Util(Util::Kitchen), &with(&[M::Speed, M::Economy, M::Capacity], true));
        exactly(Holder::Util(Util::Brewer), &with(&[M::Speed, M::Economy, M::Capacity], true));
        exactly(Holder::Util(Util::Fishery), &[M::Economy, M::Storage]);
        for u in [Util::Feeder, Util::Shearer, Util::Apiary] {
            exactly(Holder::Util(u), &[M::Economy, M::Radius]);
        }
        exactly(Holder::Util(Util::Vein), &[M::Speed, M::Economy, M::Rare, M::Pick, M::Stratum]);
        for u in [Util::ChargePlate, Util::XpTank, Util::Projector, Util::PortalDock] {
            exactly(Holder::Util(u), &[]);
            assert!(!Holder::Util(u).has_sockets());
        }
        assert!(Holder::Node.has_sockets());
    }

    #[test]
    fn economy_fits_every_machine_and_speed_all_but_animals_and_fishing() {
        // Guide `modules`: economy for all machines; speed for all but the animal and fishing ones.
        let machines = [Holder::Processor, Holder::Pump, Holder::Crafter, Holder::Mixer, Holder::Placer, Holder::Miner];
        for h in machines {
            assert!(h.takes(M::Economy) && h.takes(M::Speed), "{h:?}");
        }
        for u in Util::ALL {
            let h = Holder::Util(u);
            if !h.has_sockets() {
                continue;
            }
            assert!(h.takes(M::Economy), "{u:?}");
            let animal = matches!(u, Util::Fishery | Util::Feeder | Util::Shearer | Util::Apiary);
            assert_eq!(h.takes(M::Speed), !animal, "{u:?}");
        }
        assert!(Holder::Butcher.takes(M::Economy) && !Holder::Butcher.takes(M::Speed));
    }

    #[test]
    fn sockets_by_tier() {
        assert_eq!([1, 2, 3, 4].map(sockets), [3, 3, 4, 4]);
        assert_eq!([2, 3, 4].map(chassis_sockets), [3, 5, 7]);
        assert_eq!(sockets(4), MAX_SOCKETS);
        assert_eq!(chassis_sockets(4), MAX_CHASSIS_SOCKETS);
    }

    #[test]
    fn tools_use_their_own_chassis_modules() {
        assert!(tool_uses(Tool::Reaper, M::Radius) && tool_uses(Tool::Reaper, M::Storage));
        assert!(!tool_uses(Tool::Reaper, M::Tunnel));
        assert!(tool_uses(Tool::Bore, M::Height) && tool_uses(Tool::Bore, M::Storage));
        assert!(!tool_uses(Tool::Bore, M::Radius));
        assert!(tool_uses(Tool::Map, M::Radius) && !tool_uses(Tool::Map, M::Storage));
        assert!(tool_uses(Tool::Saw, M::Battery) && tool_uses(Tool::Bore, M::WorkSpeed));
    }

    #[test]
    fn speed_and_economy_stack_in_full_and_economy_slows_work() {
        let t = Tuning::default();
        let (w, e) = factors(&[Some(M::Speed), Some(M::Speed)], &t);
        assert!((w - 4.0).abs() < 1e-9 && (e - 6.25).abs() < 1e-9, "A9: ×4 and ×6.25");
        let (w, e) = factors(&[Some(M::Economy), None], &t);
        assert!((w - 0.85).abs() < 1e-9 && (e - 0.6).abs() < 1e-9);
        assert_eq!(paced_ticks(100, &[], &t), 100);
        assert_eq!(paced_ticks(100, &[Some(M::Speed)], &t), 50);
        assert_eq!(paced_ticks(100, &[Some(M::Economy)], &t), 118, "100 / 0.85");
        assert_eq!(paced_ticks(1, &[Some(M::Speed); 4], &t), 1);
    }

    #[test]
    fn capacity_adds_two_slots_and_a_page_a_module() {
        let s = [Some(M::Capacity), Some(M::Capacity), Some(M::Speed)];
        assert_eq!(capacity_slots(&s), 4);
        assert_eq!(capacity_pages(&s), 2);
        assert_eq!(capacity_slots(&[]), 0);
    }

    #[test]
    fn stack_tiers_and_slot_sizes() {
        assert_eq!(stack_tier(&[None, Some(M::Speed)]), 0);
        assert_eq!(stack_tier(&[Some(M::Stack), Some(M::Stack3)]), 3);
        assert_eq!(machine_stack(&[], 64), 64);
        assert_eq!(machine_stack(&[Some(M::Stack3)], 64), 256, "a machine holds 256 whatever the tier");
        assert_eq!(machine_stack(&[Some(M::Stack)], 16), 64);
        assert_eq!(machine_stack(&[Some(M::Stack)], 1), 1);
        assert_eq!(backpack_stack(&[], 64), 64);
        assert_eq!(backpack_stack(&[Some(M::Stack)], 64), 256);
        assert_eq!(backpack_stack(&[Some(M::Stack2)], 64), 1024);
        assert_eq!(backpack_stack(&[Some(M::Stack3)], 64), 1024, "a backpack stacks 1024 at most");
        assert_eq!(backpack_stack(&[Some(M::Stack3)], 16), 256);
    }

    #[test]
    fn yield_is_15_percent_a_module() {
        assert_eq!(yield_chance(&[]), 0.0);
        assert!((yield_chance(&[Some(M::Yield)]) - 0.15).abs() < 1e-9);
        assert!((yield_chance(&[Some(M::Yield), Some(M::Yield)]) - 0.30).abs() < 1e-9);
        assert!(yield_lucky(&[Some(M::Yield)], 0.1));
        assert!(!yield_lucky(&[Some(M::Yield)], 0.2));
        assert!(!yield_lucky(&[], 0.0));
    }

    /// The recipe of `item` in the content mod: (pattern rows, key letter → item).
    fn recipe(item: &str) -> (Vec<String>, Vec<(String, String)>) {
        let name = item.trim_start_matches("rustcraft:");
        let path = format!("{}/../../content/data/rustcraft/recipe/{name}.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let json: serde_json::Value = serde_json::from_str(&text).expect("recipe json");
        assert_eq!(json["result"]["id"], item, "{path}");
        let rows = json["pattern"].as_array().expect("pattern").iter().map(|r| r.as_str().unwrap().to_owned()).collect();
        let mut key: Vec<(String, String)> = json["key"]
            .as_object()
            .expect("key")
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
            .collect();
        key.sort();
        (rows, key)
    }

    fn keyed(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut v: Vec<_> = pairs.iter().map(|(k, i)| ((*k).to_owned(), (*i).to_owned())).collect();
        v.sort();
        v
    }

    #[test]
    fn no_recipe_takes_the_amethyst_shard() {
        // Slice 18: data.js has no amethyst; every `shard` of its recipes is the crystal shard.
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/data/rustcraft/recipe");
        let amethyst: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                std::fs::read_to_string(e.path()).is_ok_and(|t| t.contains("minecraft:amethyst_shard"))
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(amethyst.is_empty(), "{amethyst:?}");
    }

    /// A crafting recipe as the grid sees it: shaped rows of item ids (" " for empty), or the
    /// sorted ingredients of a shapeless one.
    fn grid_of(json: &serde_json::Value) -> Option<Vec<String>> {
        fn item(v: &serde_json::Value) -> String {
            match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Array(a) => {
                    let mut all: Vec<String> = a.iter().map(item).collect();
                    all.sort();
                    all.join("|")
                }
                other => other["item"].as_str().map_or_else(|| format!("#{}", other["tag"]), str::to_owned),
            }
        }
        let kind = json["type"].as_str()?;
        if kind.ends_with("crafting_shapeless") {
            let mut all: Vec<String> = json["ingredients"].as_array()?.iter().map(item).collect();
            all.sort();
            return Some(all);
        }
        if !kind.ends_with("crafting_shaped") {
            return None;
        }
        let key = json["key"].as_object()?;
        let rows = json["pattern"].as_array()?;
        Some(
            rows.iter()
                .map(|row| {
                    row.as_str()
                        .unwrap_or_default()
                        .chars()
                        .map(|c| if c == ' ' { " ".to_owned() } else { item(&key[&c.to_string()]) })
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect(),
        )
    }

    #[test]
    fn no_two_crafting_recipes_share_a_grid() {
        // 0.18.2: the battery and its capacity module, the anchor and the economy module, the
        // chassis and the chestplates, three turret modules and three suit modules had the same
        // grid, so the table gave only one of each pair. A mirrored grid crafts too.
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/data/rustcraft/recipe");
        let mut seen: std::collections::BTreeMap<Vec<String>, String> = Default::default();
        let mut clashes = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(entry.path()).unwrap();
            let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
            let Some(grid) = grid_of(&json) else { continue };
            let shaped = json["type"].as_str().is_some_and(|t| t.ends_with("crafting_shaped"));
            let mirrored: Vec<String> = grid
                .iter()
                .map(|row| row.split(',').rev().collect::<Vec<_>>().join(","))
                .collect();
            let mut forms = vec![grid];
            if shaped && mirrored != forms[0] {
                forms.push(mirrored);
            }
            for form in forms {
                if let Some(other) = seen.get(&form) {
                    if *other != name {
                        clashes.push(format!("{other} = {name}"));
                    }
                } else {
                    seen.insert(form, name.clone());
                }
            }
        }
        clashes.sort();
        clashes.dedup();
        assert!(clashes.is_empty(), "recipes with the same grid: {clashes:?}");
    }

    #[test]
    fn every_module_has_a_recipe_and_the_slice_18_ones_are_the_handbooks() {
        for (_, item) in ITEMS {
            recipe(item);
        }
        // data.js; its shard is the crystal shard (stream 2's `rustcraft:crystal_shard`).
        let (a2, a3, a4) = ("rustcraft:sparksteel", "rustcraft:crystallite", "rustcraft:starsteel");
        let shard = "rustcraft:crystal_shard";
        let modr = |a: &str, x: &str| (vec!["axa", "xsx", "axa"], keyed(&[("a", a), ("x", x), ("s", shard)]));
        type Shaped<'a> = (Vec<&'a str>, Vec<(String, String)>);
        let cases: Vec<(Module, Shaped)> = vec![
            (M::Stack, (vec!["b b", " c ", "b b"], keyed(&[("b", a2), ("c", "minecraft:chest")]))),
            (M::Stack2, (vec!["b b", " c ", "b b"], keyed(&[("b", a3), ("c", "minecraft:chest")]))),
            (M::Stack3, (vec!["b b", " c ", "b b"], keyed(&[("b", a4), ("c", "minecraft:chest")]))),
            (M::Yield, modr(a3, "minecraft:gold_ingot")),
            (M::Line, modr(a2, "minecraft:hopper")),
            (M::Lava, modr(a3, "minecraft:magma_block")),
            (M::Build, modr(a3, "minecraft:dropper")),
            (
                M::Stratum,
                (vec!["asa", "bdb", "asa"], keyed(&[("a", a4), ("s", shard), ("b", "minecraft:brush"), ("d", "minecraft:diamond")])),
            ),
            (
                M::Height,
                (vec!["aca", "rsr", "aca"], keyed(&[("a", a3), ("c", "minecraft:compass"), ("r", "minecraft:redstone"), ("s", shard)])),
            ),
            (
                M::Recipes,
                (vec!["aba", "tst", "aba"], keyed(&[("a", a3), ("b", "minecraft:book"), ("t", "minecraft:crafting_table"), ("s", shard)])),
            ),
        ];
        for (m, (rows, key)) in cases {
            let (got_rows, got_key) = recipe(m.item());
            assert_eq!(got_rows, rows, "{m:?}");
            assert_eq!(got_key, key, "{m:?}");
        }
    }

    #[test]
    fn storage_doubles_line_adds_half_height_steps_toward_its_target() {
        assert_eq!(storage_factor(&[]), 1);
        assert_eq!(storage_factor(&[Some(M::Storage), Some(M::Storage)]), 4);
        assert!((line_factor(&[Some(M::Line)]) - 1.5).abs() < 1e-9);
        assert!((line_factor(&[Some(M::Line), Some(M::Line)]) - 2.0).abs() < 1e-9);
        assert_eq!(height_step(true, Some(40), 45), -1);
        assert_eq!(height_step(true, Some(50), 45), 1);
        assert_eq!(height_step(true, Some(45), 45), 0);
        assert_eq!(height_step(false, Some(40), 45), 0);
        assert_eq!(height_step(true, None, 45), 0);
    }
}
