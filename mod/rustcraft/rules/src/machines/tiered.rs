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
