//! Experience for the condenser and the enchanting machine (spec 22), in the game's points: the
//! condenser keeps points, a player gives and takes whole levels.

/// The most points a condenser keeps: what level 100 is worth from 0 (guide `xp_tank`: 30 970).
pub const CAPACITY: u32 = 30_970;
/// Slice 18: E a give or a take costs (2 E/s while it takes or gives, a press a second).
pub const PRESS_ENERGY: f64 = 2.0;

/// Slice 18 (guide `xp_tank`: «сдать по 1, по 10 или все; забрать так же»): the condenser's six
/// buttons, in its window's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XpPress {
    Give(Amount),
    Take(Amount),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount {
    One,
    Ten,
    All,
}

pub const PRESSES: [XpPress; 6] = [
    XpPress::Give(Amount::One),
    XpPress::Give(Amount::Ten),
    XpPress::Give(Amount::All),
    XpPress::Take(Amount::One),
    XpPress::Take(Amount::Ten),
    XpPress::Take(Amount::All),
];

/// The levels a give of `amount` takes from a player at `level` into a condenser with `room`
/// points free, and the points they are worth; fewer when the room is short.
pub fn give_levels(level: u32, amount: Amount, room: u32) -> (u32, u32) {
    let want = match amount {
        Amount::One => 1,
        Amount::Ten => 10,
        Amount::All => level,
    }
    .min(level);
    let mut n = want;
    while n > 0 && give(level, n) > room {
        n -= 1;
    }
    (n, give(level, n))
}

/// The points a take of `amount` gives a player at `level` from `stored` points.
pub fn take_points(level: u32, amount: Amount, stored: u32) -> u32 {
    match amount {
        Amount::One => to_next(level).min(stored),
        Amount::Ten => (level..level + 10).map(to_next).sum::<u32>().min(stored),
        Amount::All => stored,
    }
}

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

/// Slice 18 (guide `enchanter`): the enchanting level a player chooses, 1 to 30.
pub const LEVEL_MIN: u32 = 1;
pub const LEVEL_MAX: u32 = 30;
/// The enchanting machine's level buttons: −10, −1, +1, +10.
pub const LEVEL_STEPS: [i32; 4] = [-10, -1, 1, 10];

/// The level after a button, kept in 1..=30.
pub fn step_level(level: u32, by: i32) -> u32 {
    (level as i32 + by).clamp(LEVEL_MIN as i32, LEVEL_MAX as i32) as u32
}

/// What enchanting at `level` costs: 1–10 one level and one lapis, 11–20 two and two, 21–30
/// three and three (the table's three rows).
pub fn enchant_cost(level: u32) -> u32 {
    level.clamp(LEVEL_MIN, LEVEL_MAX).div_ceil(10)
}

/// «В конденсаторе не меньше уровней, чем уровень чар»: the condenser holds what `level` levels
/// are worth from zero.
pub fn enough_for(stored: u32, level: u32) -> bool {
    stored >= total(level)
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
        assert_eq!(total(50), 5345);
        assert_eq!((enchant_cost(1), enchant_cost(10), enchant_cost(11), enchant_cost(30)), (1, 1, 2, 3));
        assert_eq!(step_level(30, 10), 30);
        assert_eq!(step_level(5, -10), 1);
        assert!(enough_for(total(30), 30) && !enough_for(total(30) - 1, 30));
        assert_eq!(total(100), CAPACITY, "the guide's 30 970: 100 levels from 0");
        assert_eq!(give_levels(30, Amount::Ten, CAPACITY), (10, total(30) - total(20)));
        assert_eq!(give_levels(30, Amount::All, CAPACITY).0, 30);
        assert_eq!(give_levels(30, Amount::One, 5).0, 0, "no room");
        assert_eq!(take_points(0, Amount::All, 900), 900);
        assert_eq!(take_points(0, Amount::One, 900), 7);
        assert_eq!(take_points(0, Amount::Ten, 3), 3);
        assert_eq!(give(30, 1), to_next(29));
        assert_eq!(give(3, 10), total(3), "never below 0");
        assert_eq!(levels_from(0, 16), 2);
        assert_eq!(enchant_points(30, 3), total(30) - total(27));
    }
}
