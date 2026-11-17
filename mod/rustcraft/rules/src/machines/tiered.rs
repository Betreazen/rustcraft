//! Slice 18: the slice-12 machines' numbers that change with their tier (guide `trash`,
//! `disintegrator`) and with the work mode (UT7: every machine has the three modes).

use crate::machines::processor::WorkMode;
use crate::net::items::{Filter, Mode};
use crate::net::settings::{JointSetting, PRIORITY_LIMIT};
use crate::utility::Util;

/// A trash's joint as it is placed (guide `trash`): it takes in, its allow list is empty, so it
/// takes nothing until the wrench names what to destroy, and its priority is the lowest, so only
/// what no storage or machine took reaches it.
pub fn trash_joint() -> JointSetting {
    JointSetting {
        mode: Mode::Into,
        filter: Filter {
            allow_only: true,
            items: Default::default(),
        },
        priority: -PRIORITY_LIMIT,
    }
}

/// Whether a trash joint's setting names what to destroy: an allow list with something in it,
/// or a block list (everything but what it names).
pub fn trash_filter_set(s: &JointSetting) -> bool {
    !s.filter.allow_only || !s.filter.items.is_empty()
}

/// Slice 18, wave 2 (guide `trash`: «Берёт только то, что пропускает его фильтр»): whether the
/// trash takes `item`: some joint of a housing beside it names what to destroy
/// ([`trash_filter_set`]) and lets it through. With none set it takes nothing.
pub fn trash_passes(filters: &[Filter], item: &str) -> bool {
    filters
        .iter()
        .any(|f| (!f.allow_only || !f.items.is_empty()) && f.passes(item))
}

/// Slice 18, wave 2: the trash keeps a copy of its joints' filters in its block entity (a slot
/// rule is asked without the world), as JSON: `[[allow_only, [item, …]], …]`.
pub fn filters_to_json(filters: &[Filter]) -> String {
    let v: Vec<(bool, Vec<&str>)> = filters
        .iter()
        .map(|f| (f.allow_only, f.items.iter().map(String::as_str).collect()))
        .collect();
    serde_json::to_string(&v).unwrap_or_default()
}

pub fn filters_from_json(json: &str) -> Vec<Filter> {
    serde_json::from_str::<Vec<(bool, Vec<String>)>>(json)
        .unwrap_or_default()
        .into_iter()
        .map(|(allow_only, items)| Filter { allow_only, items: items.into_iter().collect() })
        .collect()
}

/// Slice 18, wave 2: the trash's rate on `tier` with its modules and work mode, items and buckets
/// alike (guide `trash`): (units a second, energy a unit). 8 / 16 / 32 / 64 a second, a speed
/// module ×2 each, the mode's speed on top; 1 E a unit, an economy module ×0.6 (and a speed
/// module ×2.5), the mode's cost on top.
pub fn trash_rate(
    tier: u8,
    slots: &[Option<crate::modules::Module>],
    mode: WorkMode,
    t: &crate::tuning::Tuning,
) -> (f64, f64) {
    let (work, energy) = crate::modules::factors(slots, t);
    let (speed, cost) = mode.factors();
    (
        f64::from(trash_batch(tier)) * work * speed,
        crate::utility::step_energy(Util::Trash) * energy * cost,
    )
}

/// Items (or buckets) the trash destroys a step of one second on `tier` (guide `trash`:
/// 8 / 16 / 32 / 64 a second on tiers 1–4). A speed module shortens the step instead (×2 each).
pub fn trash_batch(tier: u8) -> u32 {
    8u32 << tier.clamp(1, 4).saturating_sub(1)
}

/// Ticks between a slice-12 machine's steps on `tier` before its modules: the disintegrator
/// takes 5 s on tier 3 and 3 s on tier 4 (guide `disintegrator`); the others as they are.
pub fn step_ticks(u: Util, tier: u8) -> u64 {
    match u {
        Util::Disintegrator if tier >= 4 => 60,
        _ => crate::utility::step_ticks(u),
    }
}

/// A step's ticks and energy in work mode `mode` (A1): economy ×0.5 speed for ×0.35 energy,
/// boost ×2 for ×3.
pub fn in_mode(ticks: u64, energy: f64, mode: WorkMode) -> (u64, f64) {
    let (speed, cost) = mode.factors();
    (((ticks as f64 / speed).round() as u64).max(1), energy * cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_trash_joint_takes_nothing_and_comes_last() {
        let j = trash_joint();
        assert!(!j.filter.passes("minecraft:cobblestone"));
        assert_eq!(j.priority, -PRIORITY_LIMIT);
        assert!(!trash_filter_set(&j));
        let mut named = j.clone();
        named.filter.items.insert("minecraft:cobblestone".into());
        assert!(named.filter.passes("minecraft:cobblestone"));
        assert!(trash_filter_set(&named));
    }

    #[test]
    fn trash_passes_only_what_a_set_joint_filter_lets_through() {
        let none = trash_joint().filter;
        assert!(!trash_passes(&[], "minecraft:cobblestone"));
        assert!(!trash_passes(&[none.clone()], "minecraft:cobblestone"));
        let mut cobble = none.clone();
        cobble.items.insert("minecraft:cobblestone".into());
        assert!(trash_passes(&[none.clone(), cobble.clone()], "minecraft:cobblestone"));
        assert!(!trash_passes(&[cobble], "minecraft:dirt"));
        let mut not_dirt = Filter { allow_only: false, items: Default::default() };
        not_dirt.items.insert("minecraft:dirt".into());
        assert!(trash_passes(&[not_dirt.clone()], "minecraft:cobblestone"));
        assert!(!trash_passes(&[not_dirt.clone()], "minecraft:dirt"));
        let back = filters_from_json(&filters_to_json(&[none.clone(), not_dirt.clone()]));
        assert_eq!(back, vec![none, not_dirt]);
        assert!(filters_from_json("garbage").is_empty());
    }

    #[test]
    fn trash_rate_follows_tier_modules_and_mode() {
        use crate::modules::Module as M;
        let t = crate::tuning::Tuning::default();
        assert_eq!(trash_rate(1, &[], WorkMode::Normal, &t), (8.0, 1.0));
        assert_eq!(trash_rate(4, &[], WorkMode::Normal, &t).0, 64.0);
        assert_eq!(trash_rate(2, &[Some(M::Speed)], WorkMode::Normal, &t).0, 32.0);
        let (_, e) = trash_rate(1, &[Some(M::Economy)], WorkMode::Normal, &t);
        assert!((e - 0.6).abs() < 1e-9);
        let (n, e) = trash_rate(1, &[], WorkMode::Boost, &t);
        assert_eq!((n, e), (16.0, 3.0));
        assert_eq!(trash_rate(1, &[], WorkMode::Economy, &t).0, 4.0);
    }

    #[test]
    fn trash_rate_by_tier() {
        assert_eq!([1, 2, 3, 4].map(trash_batch), [8, 16, 32, 64]);
    }

    #[test]
    fn disintegrator_ticks_by_tier() {
        assert_eq!(step_ticks(Util::Disintegrator, 3), 100);
        assert_eq!(step_ticks(Util::Disintegrator, 4), 60);
        assert_eq!(step_ticks(Util::Trash, 4), 20);
    }

    #[test]
    fn modes_scale_a_step() {
        assert_eq!(in_mode(20, 1.0, WorkMode::Normal), (20, 1.0));
        assert_eq!(in_mode(20, 1.0, WorkMode::Boost), (10, 3.0));
        let (t, e) = in_mode(20, 1.0, WorkMode::Economy);
        assert_eq!(t, 40);
        assert!((e - 0.35).abs() < 1e-9);
    }
}
