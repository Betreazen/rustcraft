//! 0.21 (spec 46, LP4–LP7; the owner 2026-11-19: «турели убивают врагов, мне дается опыт, а
//! также предметы тоже засасываются»): a turret vacuums up the drops of the mobs it killed into
//! its loot slots, which the magistral takes from like any machine's products. Its owner still gets
//! the kill's experience as before (rustcraft_base).
//!
//! These rules say what a turret takes: an item lying near where one of its kills died, a short
//! while after, that is not ammunition. The bundle remembers the kills, finds the items and stores
//! them.

/// Its loot slots: one row beside its target chips (machines::sheet::Shape::beside).
pub const LOOT_SLOTS: usize = 6;

/// How often a turret looks for its kills' drops, ticks.
pub const GATHER_EVERY: u64 = 10;

/// How long after a kill its drops are the turret's, ticks: a mob's drops fall the tick it dies,
/// and a flier's fall a while.
pub const KILL_TICKS: u64 = 100;

/// How far across from where the mob died its drops may lie, blocks (a drop's spread and the
/// death's knock-back).
pub const KILL_REACH: f64 = 3.0;

/// How far below where it died a drop may have fallen (a phantom or a blaze shot down), and how
/// far above it may have bounced, blocks.
pub const FALL_DEPTH: f64 = 24.0;
pub const RISE: f64 = 2.0;

/// LP6: what is ammunition, not loot: arrows of every kind, which a turret leaves where they lie.
pub fn is_ammo(item: &str) -> bool {
    matches!(
        item,
        "minecraft:arrow" | "minecraft:spectral_arrow" | "minecraft:tipped_arrow"
    )
}

/// A mob a turret killed: where it died and the tick it died (the bundle's tick count).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kill {
    pub at: [f64; 3],
    pub tick: u64,
}

/// LP5: whether `item` lying at `at` at tick `now` is the drop of one of `kills`: within
/// [`KILL_REACH`] across of where it died, no more than [`FALL_DEPTH`] below or [`RISE`] above,
/// in the [`KILL_TICKS`] after its death; never ammunition (LP6).
pub fn is_kill_drop(kills: &[Kill], now: u64, at: [f64; 3], item: &str) -> bool {
    !is_ammo(item)
        && kills.iter().any(|k| {
            let across = (at[0] - k.at[0]).powi(2) + (at[2] - k.at[2]).powi(2);
            let up = at[1] - k.at[1];
            now >= k.tick
                && now - k.tick <= KILL_TICKS
                && across <= KILL_REACH * KILL_REACH
                && (-FALL_DEPTH..=RISE).contains(&up)
        })
}

/// The kills still worth looking round at tick `now`.
pub fn recent(kills: &[Kill], now: u64) -> Vec<Kill> {
    kills
        .iter()
        .filter(|k| now >= k.tick && now - k.tick <= KILL_TICKS)
        .copied()
        .collect()
}

/// The box round `kills` an item of theirs may lie in: (min, max); none without kills.
pub fn search_box(kills: &[Kill]) -> Option<([f64; 3], [f64; 3])> {
    let first = kills.first()?;
    let mut lo = first.at;
    let mut hi = first.at;
    for k in kills {
        for i in 0..3 {
            lo[i] = lo[i].min(k.at[i]);
            hi[i] = hi[i].max(k.at[i]);
        }
    }
    Some((
        [lo[0] - KILL_REACH, lo[1] - FALL_DEPTH, lo[2] - KILL_REACH],
        [hi[0] + KILL_REACH, hi[1] + RISE, hi[2] + KILL_REACH],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIED: Kill = Kill { at: [10.0, 70.0, -4.0], tick: 1000 };

    #[test]
    fn it_takes_what_lies_where_its_kill_died_for_a_while() {
        let k = [DIED];
        assert!(is_kill_drop(&k, 1001, [10.3, 70.1, -4.2], "minecraft:rotten_flesh"));
        assert!(is_kill_drop(&k, 1000 + KILL_TICKS, [12.0, 70.0, -2.0], "minecraft:bone"));
        // A phantom's membrane fell to the ground below.
        assert!(is_kill_drop(&k, 1040, [10.0, 52.0, -4.0], "minecraft:phantom_membrane"));
        // An item with components (a zombie's damaged sword) is loot too.
        assert!(is_kill_drop(&k, 1005, [10.0, 70.0, -4.0], "minecraft:iron_sword"));
    }

    #[test]
    fn it_leaves_what_lies_elsewhere_or_later_and_arrows() {
        let k = [DIED];
        assert!(!is_kill_drop(&k, 1010, [14.0, 70.0, -4.0], "minecraft:bone"), "too far across");
        assert!(!is_kill_drop(&k, 1010, [10.0, 73.0, -4.0], "minecraft:bone"), "above it");
        assert!(!is_kill_drop(&k, 1010, [10.0, 70.0 - FALL_DEPTH - 1.0, -4.0], "minecraft:bone"), "too deep");
        assert!(!is_kill_drop(&k, 1001 + KILL_TICKS, [10.0, 70.0, -4.0], "minecraft:bone"), "too late");
        assert!(!is_kill_drop(&k, 999, [10.0, 70.0, -4.0], "minecraft:bone"), "before the kill");
        assert!(!is_kill_drop(&[], 1001, [10.0, 70.0, -4.0], "minecraft:bone"), "no kill");
        for arrow in ["minecraft:arrow", "minecraft:spectral_arrow", "minecraft:tipped_arrow"] {
            assert!(!is_kill_drop(&k, 1001, [10.0, 70.0, -4.0], arrow), "{arrow}");
        }
    }

    #[test]
    fn old_kills_go_and_the_box_holds_every_kills_reach() {
        let later = Kill { at: [-2.0, 64.0, 5.0], tick: 1050 };
        assert_eq!(recent(&[DIED, later], 1000 + KILL_TICKS + 1), vec![later]);
        assert_eq!(search_box(&[]), None);
        let (lo, hi) = search_box(&[DIED, later]).unwrap();
        assert_eq!(lo, [-2.0 - KILL_REACH, 64.0 - FALL_DEPTH, -4.0 - KILL_REACH]);
        assert_eq!(hi, [10.0 + KILL_REACH, 70.0 + RISE, 5.0 + KILL_REACH]);
    }
}
