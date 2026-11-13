//! The turret (spec 4.13 P1, P3, D5; round 30, spec 27 TR1–TR6 and 27.1; guide `turret`). What
//! its modules make of it: its role, its shots and their effects on each kind of mob, its support
//! buffs and its diagnosis. The bundle finds the targets, hurts them and gives the effects.
//!
//! - Attack: it shoots the nearest hostile mob in range; every effect module adds its effect to
//!   each shot and 10 E to its cost (TR1). Different effects act together, equal ones stack in
//!   full (A9), slowness never past its cap (TR2).
//! - Support: any support module switches it (TR3): no shots, buffs for the players in range;
//!   effect modules in it do nothing and its status line says so.
//! - Speed, economy and radius modules suit both roles (TR4).
//! - Resistances (TR5): SoulsCraft's `turret_effects` multiply an effect's length per mob kind
//!   (0 immune, a missing key 1); poison never works on the undead (the base game's tag).

use crate::modules::{self, Module};
use crate::tuning::Tuning;
use std::collections::BTreeMap;

pub const TURRET: &str = "rustcraft:turret";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Attack,
    Support,
}

/// How many modules of each kind a turret holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Loadout {
    pub speed: u32,
    pub economy: u32,
    pub radius: u32,
    pub bleed: u32,
    pub poison: u32,
    pub slow: u32,
    pub heal: u32,
    pub guard: u32,
    pub power: u32,
    pub haste: u32,
}

impl Loadout {
    pub fn of(slots: &[Option<Module>]) -> Self {
        let c = |k| modules::count(slots, k);
        Loadout {
            speed: c(Module::Speed),
            economy: c(Module::Economy),
            radius: c(Module::Radius),
            bleed: c(Module::Bleed),
            poison: c(Module::Poison),
            slow: c(Module::Slow),
            heal: c(Module::Heal),
            guard: c(Module::Guard),
            power: c(Module::Power),
            haste: c(Module::Haste),
        }
    }

    fn effect_modules(&self) -> u32 {
        self.bleed + self.poison + self.slow
    }

    fn support_modules(&self) -> u32 {
        self.heal + self.guard + self.power + self.haste
    }

    /// TR3: one support module makes it a support turret.
    pub fn role(&self) -> Role {
        if self.support_modules() > 0 {
            Role::Support
        } else {
            Role::Attack
        }
    }

    /// TR3: effect modules standing idle in a support turret.
    pub fn idle_effects(&self) -> bool {
        self.role() == Role::Support && self.effect_modules() > 0
    }

    /// Work ×2 a speed module, ×0.85 an economy module (A7, A9).
    pub fn work(&self, t: &Tuning) -> f64 {
        t.speed_work.powi(self.speed as i32) * t.economy_work.powi(self.economy as i32)
    }

    /// Energy a second ×2.5 a speed module, ×0.6 an economy module (A7, A9).
    pub fn energy(&self, t: &Tuning) -> f64 {
        t.speed_energy.powi(self.speed as i32) * t.economy_energy.powi(self.economy as i32)
    }

    /// TR4: the zone ×2 a radius module.
    fn reach(&self) -> f64 {
        2f64.powi(self.radius as i32)
    }

    /// Blocks it reaches in its role: attack 12, support 6, each ×2 a radius module.
    pub fn range(&self, t: &Tuning) -> f64 {
        let base = match self.role() {
            Role::Attack => t.turret_range,
            Role::Support => t.support_range,
        };
        base * self.reach()
    }

    /// Ticks between shots, or between support refreshes, faster with speed modules.
    pub fn interval(&self, t: &Tuning) -> f64 {
        (t.turret_shot_ticks / self.work(t)).max(1.0)
    }

    /// E a shot costs: 40 and 10 an effect module (TR1); the modules' energy a second spread
    /// over their faster or slower shots.
    pub fn shot_energy(&self, t: &Tuning) -> f64 {
        let base = t.turret_shot_energy + t.turret_effect_energy * f64::from(self.effect_modules());
        base * self.energy(t) / self.work(t)
    }

    /// E a second the support modules draw while there is someone to act on.
    pub fn support_draw(&self, t: &Tuning) -> f64 {
        t.support_energy * f64::from(self.support_modules()) * self.energy(t)
    }
}

/// The effects a turret's shots can give.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effect {
    Bleed,
    Poison,
    Slow,
}

impl Effect {
    /// The key of SoulsCraft's `turret_effects` (spec 27.1).
    pub fn key(self) -> &'static str {
        match self {
            Effect::Bleed => "bleed",
            Effect::Poison => "poison",
            Effect::Slow => "slow",
        }
    }
}

/// An effect as one shot gives it to one mob.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Given {
    pub effect: Effect,
    /// Its length, ticks.
    pub ticks: u32,
    /// The base game's amplifier: level − 1.
    pub amplifier: u8,
    /// Bleeding: damage a second, for the turret's own timer when SoulsCraft is not there.
    pub damage: f64,
}

/// A mob's resistances: duration multipliers by effect key (0 immune, a missing key 1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resist(pub BTreeMap<String, f64>);

impl Resist {
    pub fn factor(&self, e: Effect) -> f64 {
        self.0.get(e.key()).copied().unwrap_or(1.0).max(0.0)
    }
}

/// The slowness level that comes nearest to `percent` in the base game's steps of 15% a level.
pub fn slowness_amplifier(percent: f64) -> u8 {
    ((percent / 15.0).round().max(1.0) - 1.0) as u8
}

/// TR1, TR2, TR5: what one shot of `l` gives a mob with `resist`, undead or not.
pub fn effects(l: &Loadout, t: &Tuning, resist: &Resist, undead: bool) -> Vec<Given> {
    if l.role() != Role::Attack {
        return Vec::new();
    }
    let ticks = |e: Effect, seconds: f64| -> u32 {
        (seconds * t.ticks_per_second * resist.factor(e)).round() as u32
    };
    let mut out = Vec::new();
    if l.bleed > 0 {
        out.push(Given {
            effect: Effect::Bleed,
            ticks: ticks(Effect::Bleed, t.bleed_seconds),
            amplifier: (l.bleed - 1).min(255) as u8,
            damage: t.bleed_damage * f64::from(l.bleed),
        });
    }
    if l.poison > 0 && !undead {
        out.push(Given {
            effect: Effect::Poison,
            ticks: ticks(Effect::Poison, t.poison_seconds),
            amplifier: (l.poison - 1).min(255) as u8,
            damage: 0.0,
        });
    }
    if l.slow > 0 {
        let percent = (t.slow_percent * f64::from(l.slow)).min(t.slow_cap);
        out.push(Given {
            effect: Effect::Slow,
            ticks: ticks(Effect::Slow, t.slow_seconds),
            amplifier: slowness_amplifier(percent),
            damage: 0.0,
        });
    }
    out.retain(|g| g.ticks > 0);
    out
}

/// What a support turret gives the players in its range (spec 27 table).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Buffs {
    /// Health a heal gives the most hurt player, and ticks between heals.
    pub heal: f64,
    pub heal_ticks: f64,
    /// Incoming damage taken off, a fraction (capped); outgoing damage and speed added.
    pub guard: f64,
    pub power: f64,
    pub haste: f64,
}

pub fn buffs(l: &Loadout, t: &Tuning) -> Buffs {
    if l.role() != Role::Support {
        return Buffs::default();
    }
    Buffs {
        heal: t.heal_amount * f64::from(l.heal),
        heal_ticks: (t.heal_ticks / l.work(t)).max(1.0),
        guard: (t.guard_percent * f64::from(l.guard)).min(t.guard_cap) / 100.0,
        power: t.power_percent * f64::from(l.power) / 100.0,
        haste: t.haste_percent * f64::from(l.haste) / 100.0,
    }
}

/// The player a heal goes to: the most hurt below their maximum, by (id, health, maximum).
pub fn most_hurt(players: &[(u128, f64, f64)]) -> Option<u128> {
    players
        .iter()
        .filter(|(_, h, max)| h < max && *h > 0.0)
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
        .map(|p| p.0)
}

/// Guards of several turrets on one player add up to the cap, as modules do.
pub fn guard_of(fractions: &[f64], t: &Tuning) -> f64 {
    fractions.iter().sum::<f64>().min(t.guard_cap / 100.0)
}

/// What the turret shows (A4, guide `turret`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shown {
    /// Attack, shooting at a mob of this kind.
    Shooting(String),
    /// Attack, nothing to shoot.
    NoTargets,
    /// Support, healing this player (by name) or buffing.
    Healing(String),
    Supporting,
    NoPlayers,
    NoEnergy,
    /// TR3: an effect module in a support turret.
    IdleEffects,
}

/// The diagnosis: idle effect modules first, then energy, then what it does.
pub fn shown(l: &Loadout, has_energy: bool, doing: Shown) -> Shown {
    if l.idle_effects() {
        Shown::IdleEffects
    } else if !has_energy {
        Shown::NoEnergy
    } else {
        doing
    }
}

/// What SoulsCraft's mobs.json gives the turret (spec 27.1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SoulsData {
    /// `turret_effects` by mob kind (`soulscraft:revenant`).
    pub resist: BTreeMap<String, Resist>,
    /// `effects.bleeding.id`, `damage` and `period_ticks`.
    pub bleeding: Option<(String, f64, u32)>,
}

/// Reads SoulsCraft's mobs.json; None when it does not read.
pub fn parse_souls(json: &str) -> Option<SoulsData> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let mut out = SoulsData::default();
    for mob in v.get("mobs")?.as_array()? {
        let Some(key) = mob.get("key").and_then(|k| k.as_str()) else {
            continue;
        };
        let mut r = BTreeMap::new();
        if let Some(map) = mob.get("turret_effects").and_then(|m| m.as_object()) {
            for (k, f) in map {
                if let Some(f) = f.as_f64() {
                    r.insert(k.clone(), f);
                }
            }
        }
        out.resist.insert(key.to_owned(), Resist(r));
    }
    if let Some(b) = v.get("effects").and_then(|e| e.get("bleeding")) {
        let id = b
            .get("id")
            .and_then(|i| i.as_str())
            .unwrap_or("soulscraft:bleeding");
        let damage = b.get("damage").and_then(|d| d.as_f64()).unwrap_or(1.0);
        let period = b.get("period_ticks").and_then(|p| p.as_u64()).unwrap_or(20) as u32;
        out.bleeding = Some((id.to_owned(), damage, period));
    }
    Some(out)
}

/// The undead of the base game, for when the world's `#minecraft:undead` tag is not there.
pub const UNDEAD: [&str; 14] = [
    "minecraft:zombie",
    "minecraft:husk",
    "minecraft:drowned",
    "minecraft:zombie_villager",
    "minecraft:zombified_piglin",
    "minecraft:skeleton",
    "minecraft:stray",
    "minecraft:bogged",
    "minecraft:wither_skeleton",
    "minecraft:wither",
    "minecraft:phantom",
    "minecraft:skeleton_horse",
    "minecraft:zombie_horse",
    "minecraft:zoglin",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn load(m: &[Module]) -> Loadout {
        let slots: Vec<Option<Module>> = m.iter().copied().map(Some).collect();
        Loadout::of(&slots)
    }

    #[test]
    fn a_bare_turret_shoots_for_40_e_in_12_blocks() {
        let t = Tuning::default();
        let l = load(&[]);
        assert_eq!(l.role(), Role::Attack);
        assert_eq!(l.range(&t), 12.0);
        assert_eq!(l.shot_energy(&t), 40.0);
        assert!(effects(&l, &t, &Resist::default(), false).is_empty());
    }

    #[test]
    fn effect_modules_combine_and_each_adds_10_e() {
        // TR1, TR2: one shot bleeds, poisons and slows; 40 + 3 × 10 = 70 E (guide window).
        let t = Tuning::default();
        let l = load(&[Module::Bleed, Module::Poison, Module::Slow]);
        assert_eq!(l.shot_energy(&t), 70.0);
        let e = effects(&l, &t, &Resist::default(), false);
        let kinds: Vec<Effect> = e.iter().map(|g| g.effect).collect();
        assert_eq!(kinds, vec![Effect::Bleed, Effect::Poison, Effect::Slow]);
        assert_eq!(e[0].ticks, 80);
        assert_eq!(e[0].damage, 1.0);
        assert_eq!(e[2].ticks, 60);
    }

    #[test]
    fn equal_modules_stack_and_slowness_stops_at_its_cap() {
        let t = Tuning::default();
        let two = effects(
            &load(&[Module::Bleed, Module::Bleed]),
            &t,
            &Resist::default(),
            false,
        );
        assert_eq!(two[0].damage, 2.0);
        assert_eq!(two[0].amplifier, 1);
        let slows = |n: usize| {
            let l = load(&vec![Module::Slow; n]);
            effects(&l, &t, &Resist::default(), false)[0].amplifier
        };
        // 20% → level 1 (15%), 40% → level 3 (45%), 60% → level 4 (60%), 80% still 60%.
        assert_eq!(slows(1), 0);
        assert_eq!(slows(3), 3);
        assert_eq!(slows(4), slows(3));
    }

    #[test]
    fn poison_never_works_on_the_undead_and_resistances_shorten() {
        let t = Tuning::default();
        let l = load(&[Module::Bleed, Module::Poison, Module::Slow]);
        let undead = effects(&l, &t, &Resist::default(), true);
        assert!(undead.iter().all(|g| g.effect != Effect::Poison));
        // The brute: everything half as long. The revenant: no poison.
        let brute = Resist(
            [("bleed", 0.5), ("poison", 0.5), ("slow", 0.5)]
                .iter()
                .map(|(k, v)| ((*k).into(), *v))
                .collect(),
        );
        let e = effects(&l, &t, &brute, false);
        assert_eq!(e[0].ticks, 40);
        assert_eq!(e[2].ticks, 30);
        let revenant = Resist([("poison".to_owned(), 0.0)].into_iter().collect());
        assert!(
            effects(&l, &t, &revenant, false)
                .iter()
                .all(|g| g.effect != Effect::Poison)
        );
    }

    #[test]
    fn a_support_module_makes_a_support_turret_and_effects_idle() {
        // TR3 and the guide's third window: heal and haste work, poison does not.
        let t = Tuning::default();
        let l = load(&[Module::Heal, Module::Poison, Module::Haste]);
        assert_eq!(l.role(), Role::Support);
        assert!(l.idle_effects());
        assert!(effects(&l, &t, &Resist::default(), false).is_empty());
        assert_eq!(l.range(&t), 6.0);
        assert_eq!(shown(&l, true, Shown::Supporting), Shown::IdleEffects);
        let b = buffs(&l, &t);
        assert_eq!(b.heal, 1.0);
        assert!((b.haste - 0.1).abs() < 1e-9);
        assert_eq!(l.support_draw(&t), 16.0);
    }

    #[test]
    fn guard_stops_at_30_percent_and_radius_doubles_the_zone() {
        let t = Tuning::default();
        let l = load(&[Module::Guard, Module::Guard, Module::Guard, Module::Guard]);
        assert!((buffs(&l, &t).guard - 0.3).abs() < 1e-9);
        assert!((guard_of(&[0.2, 0.2], &t) - 0.3).abs() < 1e-9);
        assert_eq!(load(&[Module::Heal, Module::Radius]).range(&t), 12.0);
        assert_eq!(load(&[Module::Radius]).range(&t), 24.0);
    }

    #[test]
    fn speed_shoots_twice_as_often_and_economy_costs_less() {
        let t = Tuning::default();
        let fast = load(&[Module::Speed]);
        assert_eq!(fast.interval(&t), 10.0);
        assert_eq!(fast.shot_energy(&t), 40.0 * 2.5 / 2.0);
        let eco = load(&[Module::Economy]);
        assert!(eco.shot_energy(&t) < 40.0);
    }

    #[test]
    fn the_most_hurt_player_gets_the_heal() {
        assert_eq!(
            most_hurt(&[(1, 20.0, 20.0), (2, 6.0, 20.0), (3, 9.0, 20.0)]),
            Some(2)
        );
        assert_eq!(most_hurt(&[(1, 20.0, 20.0)]), None);
    }

    #[test]
    fn soulscrafts_mobs_json_gives_resistances_and_bleeding() {
        let json = r#"{"effects": {"bleeding": {"id": "soulscraft:bleeding", "damage": 1.0, "period_ticks": 20}},
            "mobs": [{"key": "soulscraft:revenant", "turret_effects": {"poison": 0}},
                     {"key": "soulscraft:brute", "turret_effects": {"bleed": 0.5}},
                     {"key": "soulscraft:shrike", "turret_effects": {}}]}"#;
        let d = parse_souls(json).unwrap();
        assert_eq!(d.resist["soulscraft:revenant"].factor(Effect::Poison), 0.0);
        assert_eq!(d.resist["soulscraft:brute"].factor(Effect::Bleed), 0.5);
        assert_eq!(d.resist["soulscraft:shrike"].factor(Effect::Slow), 1.0);
        assert_eq!(d.bleeding, Some(("soulscraft:bleeding".into(), 1.0, 20)));
        assert_eq!(parse_souls("not json"), None);
    }
}
