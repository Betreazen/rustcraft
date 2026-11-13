//! The module and upgrade items of slice 7 (spec 18.3 A7, 27 TR1–TR4, guide `modules`,
//! `upgrades`): what each item is, and which blocks take it. Two modules of a kind in one block
//! stack in full (A9); the rules of each block say what stacking means for it.

use serde::{Deserialize, Serialize};

/// A module or an upgrade, by what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Module {
    /// Common to machines (A7): work ×2.
    Speed,
    /// Common to machines (A7): energy ×0.6.
    Economy,
    /// Zone ×2: the worker's tool, the turret, the anchor (A7, TR4, B5).
    Radius,
    /// The worker's hold is bigger (A7 «хранилище»).
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
}

/// Every module with its item id.
pub const ITEMS: [(Module, &str); 17] = [
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
}

/// The blocks and carts that take modules, each with its own list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    Turret,
    Anchor,
    Worker,
    /// Slice 8: the battery, the miner, and the block placer and breaker.
    Battery,
    Miner,
    Placer,
}

impl Holder {
    /// Whether a module of `kind` goes into this holder's slots.
    pub fn takes(self, kind: Module) -> bool {
        match self {
            // TR1, TR3, TR4.
            Holder::Turret => {
                kind.is_effect()
                    || kind.is_support()
                    || matches!(kind, Module::Speed | Module::Economy | Module::Radius)
            }
            // Guide `anchor`: radius and economy.
            Holder::Anchor => matches!(kind, Module::Radius | Module::Economy),
            // Guide `upgrades`: battery and speed upgrades, storage and radius modules.
            Holder::Worker => matches!(
                kind,
                Module::Battery | Module::WorkSpeed | Module::Storage | Module::Radius
            ),
            // Guide `battery`: capacity and output.
            Holder::Battery => matches!(kind, Module::StoreCapacity | Module::Output),
            // Guide `miner`: speed, economy, scout, area, storage (the stack module is not made).
            Holder::Miner => matches!(
                kind,
                Module::Speed | Module::Economy | Module::Scout | Module::Area | Module::Storage
            ),
            // Guide `placer`, `breaker`: speed and economy.
            Holder::Placer => matches!(kind, Module::Speed | Module::Economy),
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_module_has_one_item_and_back() {
        for (m, item) in ITEMS {
            assert_eq!(Module::of(item), Some(m));
            assert_eq!(m.item(), item);
        }
        assert_eq!(Module::of("minecraft:stone"), None);
    }

    #[test]
    fn each_holder_takes_its_own_list() {
        assert!(Holder::Turret.takes(Module::Bleed));
        assert!(Holder::Turret.takes(Module::Heal));
        assert!(Holder::Turret.takes(Module::Radius));
        assert!(!Holder::Turret.takes(Module::Storage));
        assert!(Holder::Anchor.takes(Module::Radius));
        assert!(!Holder::Anchor.takes(Module::Speed));
        assert!(Holder::Worker.takes(Module::Battery));
        assert!(!Holder::Worker.takes(Module::Poison));
        assert!(Holder::Battery.takes(Module::Output));
        assert!(!Holder::Battery.takes(Module::Speed));
        assert!(Holder::Miner.takes(Module::Area));
        assert!(!Holder::Miner.takes(Module::Radius));
        assert!(Holder::Placer.takes(Module::Economy));
        assert!(!Holder::Placer.takes(Module::Scout));
    }
}
