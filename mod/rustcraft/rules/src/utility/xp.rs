//! Experience for the condenser and the enchanting machine (spec 22), in the game's points: the
//! condenser keeps points, a player gives and takes whole levels.

/// The most points a condenser keeps: what level 50 is worth from 0.
pub const CAPACITY: u32 = 5345;

/// Points from one level to the next, as the game counts them (`Player.getXpNeededForNextLevel`).
pub fn to_next(level: u32) -> u32 {
    if level >= 30 {
        112 + (level - 30) * 9
    } else if level >= 15 {
        37 + (level - 15) * 5
    } else {
        7 + level * 2
    }
}

/// Points worth from level 0 to `level`.
pub fn total(level: u32) -> u32 {
    (0..level).map(to_next).sum()
}

/// The points a player at `level` (with `progress` 0..1 to the next) gives when giving
/// `levels` levels: the whole levels above `level - levels`, not their progress.
pub fn give(level: u32, levels: u32) -> u32 {
    let to = level.saturating_sub(levels);
    total(level) - total(to)
}

/// How many whole levels a player at `level` gains from `points` (the rest goes to progress, as
/// the game's orbs).
pub fn levels_from(level: u32, points: u32) -> u32 {
    let mut left = points;
    let mut l = level;
    while left >= to_next(l) {
        left -= to_next(l);
        l += 1;
    }
    l - level
}

/// The points an enchantment with `cost` (the table's offer, up to 30) for `levels` levels takes
/// from the condenser: what those levels are worth at that cost.
pub fn enchant_points(cost: u32, levels: u32) -> u32 {
    give(cost.max(levels), levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_games_level_curve() {
        assert_eq!(to_next(0), 7);
        assert_eq!(total(16), 352);
        assert_eq!(total(30), 1395);
        assert_eq!(total(50), CAPACITY);
        assert_eq!(give(30, 1), to_next(29));
        assert_eq!(give(3, 10), total(3), "never below 0");
        assert_eq!(levels_from(0, 16), 2);
        assert_eq!(enchant_points(30, 3), total(30) - total(27));
    }
}
