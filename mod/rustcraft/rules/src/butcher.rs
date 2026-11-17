//! Slice 11: the butcher (spec 23, round 26, K1–K8). A machine of tier 2 in a pen: every
//! `Tuning::butcher_ticks` it strikes one adult farm animal of a kind that has more adults in its
//! radius than the number its window keeps (K3, at least 2 so the herd goes on breeding). It
//! leaves babies, tamed, named and leashed animals, villagers and players alone (K2). It needs a
//! sword in its sword slot (K5): a plain sword loses a point of durability a strike. The drops go
//! into its nine slots, which the net and hoppers take from (K4). Each kill is the herdsman's:
//! Path-of-craft-tree hears `slaughter`, never `kill` (contract, section 23).
//!
//! These rules choose the animal and say why the butcher stands; the bundle finds the animals,
//! strikes, gathers the drops and wears the sword.

use serde::{Deserialize, Serialize};

use crate::tuning::Tuning;

pub const BLOCK: &str = "rustcraft:butcher";
/// Its slots: nine for the drops, then the sword.
pub const LOOT_SLOTS: usize = 9;
pub const SWORD_SLOT: usize = 9;
pub const SLOTS: usize = 10;

/// The farm animals it culls (K2), in the order its window lists them.
pub const ANIMALS: [&str; 7] = [
    "minecraft:cow",
    "minecraft:sheep",
    "minecraft:pig",
    "minecraft:chicken",
    "minecraft:goat",
    "minecraft:rabbit",
    "minecraft:mooshroom",
];

/// The spawn egg the window shows for each animal.
pub fn icon(kind: &str) -> String {
    format!("{kind}_spawn_egg")
}

/// Whether `item` goes into the sword slot: any sword (K5).
pub fn is_sword(item: &str) -> bool {
    item.ends_with("_sword") || is_blade(item)
}

/// Slice 18 (guide `butcher`): the energy blade, which spends its charge instead of wearing.
pub fn is_blade(item: &str) -> bool {
    item.starts_with("rustcraft:blade_")
}

/// Slice 18: the butcher's buffer of energy and the most it fills a second (it spends 4 E/s:
/// 40 E a strike every 10 s).
pub const BUFFER: f64 = 200.0;
pub const DRAW: f64 = 8.0;
pub const ENERGY_PER_SECOND: f64 = 4.0;
/// The charge a blade spends a strike.
pub const BLADE_CHARGE: f64 = 25.0;

/// The energy one strike costs, every `ticks`, with `economy` economy modules (×0.6 each).
pub fn strike_energy(ticks: u64, economy: u32) -> f64 {
    ENERGY_PER_SECOND * ticks as f64 / 20.0 * 0.6f64.powi(economy as i32)
}

/// Slice 18, wave 2 (A9, guide `mod_eco`: работа ×0,85): ticks between a butcher's strikes
/// with `economy` economy modules: `base` over `economy_work` (0.85) for each. A strike still
/// costs [`strike_energy`] of `base`, so its energy is ×0.6 a strike.
pub fn strike_ticks(base: u64, economy: u32, economy_work: f64) -> u64 {
    ((base as f64 / economy_work.powi(economy as i32)).round() as u64).max(1)
}

/// Slice 18: what Looting adds to a farm animal's drops, as the game's loot tables add it (0 to
/// `level` more of each looting drop, `random` 0..1 a drop); with Fire Aspect the meat cooked.
pub fn looting_bonus(kind: &str, level: u32, fire: bool, random: &mut dyn FnMut() -> f64) -> Vec<(String, u32)> {
    let drops: &[(&str, &str)] = match kind {
        "minecraft:cow" | "minecraft:mooshroom" => &[("minecraft:beef", "minecraft:cooked_beef"), ("minecraft:leather", "minecraft:leather")],
        "minecraft:sheep" => &[("minecraft:mutton", "minecraft:cooked_mutton")],
        "minecraft:pig" => &[("minecraft:porkchop", "minecraft:cooked_porkchop")],
        "minecraft:chicken" => &[("minecraft:chicken", "minecraft:cooked_chicken"), ("minecraft:feather", "minecraft:feather")],
        "minecraft:rabbit" => &[("minecraft:rabbit", "minecraft:cooked_rabbit"), ("minecraft:rabbit_hide", "minecraft:rabbit_hide")],
        _ => &[],
    };
    if level == 0 {
        return Vec::new();
    }
    drops
        .iter()
        .filter_map(|(raw, cooked)| {
            let n = (random() * f64::from(level + 1)).floor().min(f64::from(level)) as u32;
            (n > 0).then(|| ((if fire { *cooked } else { *raw }).to_owned(), n))
        })
        .collect()
}

/// An animal in its radius, as the bundle saw it.
#[derive(Clone, Debug, PartialEq)]
pub struct Animal {
    pub id: u32,
    pub kind: String,
    pub baby: bool,
    pub tamed: bool,
    pub named: bool,
    pub leashed: bool,
}

impl Animal {
    /// K2: an adult farm animal nobody keeps.
    pub fn cullable(&self) -> bool {
        ANIMALS.contains(&self.kind.as_str())
            && !self.baby
            && !self.tamed
            && !self.named
            && !self.leashed
    }
}

/// How many adults of each animal of [`ANIMALS`] it leaves (K3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keep(pub Vec<u32>);

impl Keep {
    pub fn new(t: &Tuning) -> Keep {
        Keep(vec![t.butcher_keep_default; ANIMALS.len()])
    }

    /// The number for animal `i`, never under the least (K3).
    pub fn of(&self, i: usize, t: &Tuning) -> u32 {
        self.0
            .get(i)
            .copied()
            .unwrap_or(t.butcher_keep_default)
            .clamp(t.butcher_keep_min, t.butcher_keep_max.max(t.butcher_keep_min))
    }

    /// A press on animal `i`'s toggle: one more to keep, back to the least after the most.
    pub fn step(&mut self, i: usize, t: &Tuning) {
        if i >= ANIMALS.len() {
            return;
        }
        self.0.resize(ANIMALS.len(), t.butcher_keep_default);
        let now = self.of(i, t);
        self.0[i] = if now >= t.butcher_keep_max {
            t.butcher_keep_min
        } else {
            now + 1
        };
    }
}

/// Why the butcher stands, or what it does (K8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shown {
    /// It struck this animal last, or waits for its next strike.
    Working(String),
    /// «Нет меча».
    NoSword,
    /// «Некого забивать: взрослых не больше, чем оставить».
    NothingToCull,
    /// «Добыче некуда деться»: its drop slots are full.
    OutputFull,
    /// Slice 18: «Нет энергии»: its buffer, or its energy blade's charge, holds less than a strike.
    NoEnergy,
}

/// The adults of each animal of [`ANIMALS`] in `animals` it may cull.
pub fn adults(animals: &[Animal]) -> Vec<u32> {
    ANIMALS
        .iter()
        .map(|k| {
            animals
                .iter()
                .filter(|a| a.cullable() && a.kind == *k)
                .count() as u32
        })
        .collect()
}

/// The animal to strike now: one of the kind with the most adults over its number to keep, the
/// lowest id of that kind; none when every kind is at or under its number.
pub fn victim<'a>(animals: &'a [Animal], keep: &Keep, t: &Tuning) -> Option<&'a Animal> {
    let counts = adults(animals);
    let (i, _) = counts
        .iter()
        .enumerate()
        .map(|(i, &n)| (i, n.saturating_sub(keep.of(i, t))))
        .filter(|(_, over)| *over > 0)
        .max_by_key(|&(i, over)| (over, std::cmp::Reverse(i)))?;
    animals
        .iter()
        .filter(|a| a.cullable() && a.kind == ANIMALS[i])
        .min_by_key(|a| a.id)
}

/// What the butcher does this strike: no sword, no room or no animal to cull say why it stands.
pub fn decide<'a>(
    has_sword: bool,
    full: bool,
    animals: &'a [Animal],
    keep: &Keep,
    t: &Tuning,
) -> Result<&'a Animal, Shown> {
    if !has_sword {
        return Err(Shown::NoSword);
    }
    if full {
        return Err(Shown::OutputFull);
    }
    victim(animals, keep, t).ok_or(Shown::NothingToCull)
}

/// A plain sword after one strike: its new damage, or none when it broke.
pub fn worn(damage: u32, max_damage: u32) -> Option<u32> {
    if max_damage == 0 {
        return Some(damage);
    }
    let next = damage + 1;
    (next < max_damage).then_some(next)
}

/// Wave 2 (w2-craft, guide `machine`: «машина следующей ступени работает быстрее»): a strike
/// every `ticks` on its first tier comes `tier_speed` (`tiers::speed`) times as often.
pub fn tier_strike_ticks(ticks: u64, tier_speed: f64) -> u64 {
    ((ticks as f64 / tier_speed.max(1e-6)).round() as u64).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economy_slows_the_strike() {
        let t = crate::tuning::Tuning::default();
        assert_eq!(super::strike_ticks(t.butcher_ticks, 0, t.economy_work), t.butcher_ticks);
        let one = super::strike_ticks(200, 1, t.economy_work);
        assert_eq!(one, 235);
        assert!(super::strike_ticks(200, 2, t.economy_work) > one);
    }

    #[test]
    fn the_blade_strikes_and_looting_adds_fire_cooks() {
        assert!(is_sword("rustcraft:blade_3") && is_sword("minecraft:iron_sword"));
        assert!(!is_sword("minecraft:stick"));
        assert!((strike_energy(200, 0) - 40.0).abs() < 1e-9);
        assert!((strike_energy(200, 1) - 24.0).abs() < 1e-9);
        let mut top = || 0.99;
        assert_eq!(
            looting_bonus("minecraft:cow", 3, true, &mut top),
            vec![("minecraft:cooked_beef".to_owned(), 3), ("minecraft:leather".to_owned(), 3)]
        );
        let mut low = || 0.0;
        assert!(looting_bonus("minecraft:cow", 3, false, &mut low).is_empty());
        assert!(looting_bonus("minecraft:cow", 0, false, &mut top).is_empty());
    }

    fn animal(id: u32, kind: &str) -> Animal {
        Animal {
            id,
            kind: kind.into(),
            baby: false,
            tamed: false,
            named: false,
            leashed: false,
        }
    }

    #[test]
    fn it_leaves_two_adults_of_a_kind_by_default() {
        let t = Tuning::default();
        let keep = Keep::new(&t);
        let two = [animal(1, "minecraft:cow"), animal(2, "minecraft:cow")];
        assert_eq!(victim(&two, &keep, &t), None);
        let three = [
            animal(5, "minecraft:cow"),
            animal(2, "minecraft:cow"),
            animal(9, "minecraft:cow"),
        ];
        assert_eq!(victim(&three, &keep, &t).map(|a| a.id), Some(2));
    }

    #[test]
    fn babies_tamed_named_and_leashed_animals_and_villagers_are_left_alone() {
        // K2.
        let t = Tuning::default();
        let keep = Keep::new(&t);
        let mut herd = vec![animal(1, "minecraft:sheep"), animal(2, "minecraft:sheep")];
        let mut baby = animal(3, "minecraft:sheep");
        baby.baby = true;
        let mut named = animal(4, "minecraft:sheep");
        named.named = true;
        let mut leashed = animal(5, "minecraft:sheep");
        leashed.leashed = true;
        let mut tamed = animal(6, "minecraft:sheep");
        tamed.tamed = true;
        herd.extend([baby, named, leashed, tamed, animal(7, "minecraft:villager")]);
        assert_eq!(victim(&herd, &keep, &t), None);
        assert_eq!(adults(&herd)[1], 2);
    }

    #[test]
    fn the_kind_most_over_its_number_goes_first() {
        let t = Tuning::default();
        let keep = Keep::new(&t);
        let mut herd: Vec<Animal> = (0..3).map(|i| animal(i, "minecraft:cow")).collect();
        herd.extend((10..15).map(|i| animal(i, "minecraft:pig")));
        assert_eq!(victim(&herd, &keep, &t).map(|a| a.kind.as_str()), Some("minecraft:pig"));
    }

    #[test]
    fn the_number_to_keep_steps_up_and_wraps_and_never_goes_under_two() {
        // K3.
        let t = Tuning::default();
        let mut keep = Keep::new(&t);
        assert_eq!(keep.of(0, &t), 2);
        keep.step(0, &t);
        assert_eq!(keep.of(0, &t), 3);
        for _ in 0..13 {
            keep.step(0, &t);
        }
        assert_eq!(keep.of(0, &t), 16);
        keep.step(0, &t);
        assert_eq!(keep.of(0, &t), 2);
        let low = Keep(vec![0; ANIMALS.len()]);
        assert_eq!(low.of(3, &t), 2, "a saved 0 still leaves two");
        let mut keep = Keep(Vec::new());
        keep.step(2, &t);
        assert_eq!(keep.of(2, &t), 3);
        assert_eq!(keep.of(0, &t), 2);
    }

    #[test]
    fn no_sword_and_full_slots_have_their_diagnoses() {
        // K8.
        let t = Tuning::default();
        let keep = Keep::new(&t);
        let herd: Vec<Animal> = (0..4).map(|i| animal(i, "minecraft:cow")).collect();
        assert_eq!(decide(false, false, &herd, &keep, &t), Err(Shown::NoSword));
        assert_eq!(decide(true, true, &herd, &keep, &t), Err(Shown::OutputFull));
        assert_eq!(decide(true, false, &herd[..2], &keep, &t), Err(Shown::NothingToCull));
        assert_eq!(decide(true, false, &herd, &keep, &t).map(|a| a.id), Ok(0));
    }

    #[test]
    fn a_plain_sword_wears_out() {
        assert_eq!(worn(0, 250), Some(1));
        assert_eq!(worn(248, 250), Some(249));
        assert_eq!(worn(249, 250), None);
        assert_eq!(worn(3, 0), Some(3), "an unbreakable sword stays");
        assert!(is_sword("minecraft:iron_sword"));
        assert!(!is_sword("minecraft:iron_axe"));
        assert_eq!(icon("minecraft:cow"), "minecraft:cow_spawn_egg");
    }

    #[test]
    fn a_higher_tier_strikes_more_often() {
        assert_eq!(tier_strike_ticks(200, 1.0), 200);
        assert_eq!(tier_strike_ticks(200, 2.0), 100, "tier 4 of a tier-2 machine");
    }
}
