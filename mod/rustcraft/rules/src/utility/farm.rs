//! The farm machines of spec 22: the feeder, the shearer-milker, the apiary and the fishing
//! station. They work slower than a player and only on honest conditions (open water, live
//! animals in their radius), so they are no free farm.

use crate::butcher::Animal;

/// The most adults of a kind the feeder keeps (K7).
pub const HERD_MAX: usize = 16;
/// Ticks a fed animal is in love, as the game's own feeding.
pub const LOVE_TICKS: i32 = 600;
/// Ticks a milked cow rests before the shearer milks it again (its 5 minutes, shared with the
/// player in Path-of-craft-tree).
pub const MILK_REST_TICKS: u64 = 6000;
/// The full honey level of a hive or nest.
pub const HONEY_FULL: u8 = 5;
/// The honeycombs a full hive gives without a bottle, as shears give them.
pub const COMBS: u32 = 3;
/// The fishing station's wait between catches, ticks: 30 to 60 s (spec 22).
pub const FISH_MIN_TICKS: u64 = 600;
pub const FISH_MAX_TICKS: u64 = 1200;
/// Open water it needs: 5 × 5 blocks of water, at least 2 deep, under it.
pub const WATER_SIDE: i32 = 5;
pub const WATER_DEPTH: i32 = 2;

/// What each animal eats to breed, as the game feeds them.
pub fn food_of(kind: &str) -> &'static [&'static str] {
    match kind {
        "minecraft:cow" | "minecraft:mooshroom" | "minecraft:sheep" | "minecraft:goat" => &["minecraft:wheat"],
        "minecraft:pig" => &["minecraft:carrot", "minecraft:potato", "minecraft:beetroot"],
        "minecraft:chicken" => &[
            "minecraft:wheat_seeds",
            "minecraft:melon_seeds",
            "minecraft:pumpkin_seeds",
            "minecraft:beetroot_seeds",
            "minecraft:torchflower_seeds",
            "minecraft:pitcher_pod",
        ],
        "minecraft:rabbit" => &["minecraft:carrot", "minecraft:golden_carrot", "minecraft:dandelion"],
        _ => &[],
    }
}

/// Whether `item` feeds any animal the feeder breeds.
pub fn is_feed(item: &str) -> bool {
    crate::butcher::ANIMALS.iter().any(|k| food_of(k).contains(&item))
}

/// An animal as the feeder sees it: whether it takes food now (an adult not in love, not on its
/// breeding cool-down).
#[derive(Clone, Debug, PartialEq)]
pub struct Fed {
    pub animal: Animal,
    pub takes_food: bool,
}

/// The pair to feed next, and the food: two adults of the first kind under the herd's limit
/// that both take food and whose food the feeder holds. None when no pair can breed.
pub fn pair_to_feed<'a>(animals: &'a [Fed], foods: &[String]) -> Option<(&'a Fed, &'a Fed, &'static str)> {
    for kind in crate::butcher::ANIMALS {
        let Some(food) = food_of(kind).iter().find(|f| foods.iter().any(|g| g == *f)) else {
            continue;
        };
        let adults = animals.iter().filter(|a| a.animal.kind == kind && !a.animal.baby).count();
        if adults >= HERD_MAX {
            continue;
        }
        let mut ready = animals
            .iter()
            .filter(|a| a.animal.kind == kind && !a.animal.baby && a.takes_food);
        if let (Some(a), Some(b)) = (ready.next(), ready.next()) {
            return Some((a, b, food));
        }
    }
    None
}

/// Whether a column of `depths` (water blocks under each of the 5 × 5 cells) is open water.
pub fn open_water(depths: &[i32]) -> bool {
    depths.len() == (WATER_SIDE * WATER_SIDE) as usize && depths.iter().all(|d| *d >= WATER_DEPTH)
}

/// The fishing station's next wait, from a random number 0..1.
pub fn fish_wait(random: f64) -> u64 {
    FISH_MIN_TICKS + ((FISH_MAX_TICKS - FISH_MIN_TICKS) as f64 * random.clamp(0.0, 1.0)) as u64
}

/// What a full hive gives: a honey bottle for a glass bottle, else its combs.
pub fn harvest(has_bottle: bool) -> (&'static str, u32) {
    if has_bottle {
        ("minecraft:honey_bottle", 1)
    } else {
        ("minecraft:honeycomb", COMBS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fed(id: u32, kind: &str, takes: bool) -> Fed {
        Fed {
            animal: Animal {
                id,
                kind: kind.into(),
                baby: false,
                tamed: false,
                named: false,
                leashed: false,
            },
            takes_food: takes,
        }
    }

    #[test]
    fn a_pair_is_fed_only_with_its_food_and_under_the_herds_limit() {
        let wheat = vec!["minecraft:wheat".to_owned()];
        let herd = [fed(1, "minecraft:cow", true), fed(2, "minecraft:cow", true)];
        let (a, b, food) = pair_to_feed(&herd, &wheat).unwrap();
        assert_eq!((a.animal.id, b.animal.id, food), (1, 2, "minecraft:wheat"));
        assert!(pair_to_feed(&herd, &["minecraft:carrot".to_owned()]).is_none());
        let one_ready = [fed(1, "minecraft:cow", true), fed(2, "minecraft:cow", false)];
        assert!(pair_to_feed(&one_ready, &wheat).is_none());
        let full: Vec<Fed> = (0..16).map(|i| fed(i, "minecraft:cow", true)).collect();
        assert!(pair_to_feed(&full, &wheat).is_none(), "K7: 16 heads at most");
    }

    #[test]
    fn open_water_needs_every_cell_two_deep() {
        assert!(open_water(&[2; 25]));
        let mut shallow = [3; 25];
        shallow[7] = 1;
        assert!(!open_water(&shallow));
        assert!(!open_water(&[3; 9]));
        assert!((FISH_MIN_TICKS..=FISH_MAX_TICKS).contains(&fish_wait(0.5)));
        assert_eq!(harvest(false), ("minecraft:honeycomb", 3));
        assert!(is_feed("minecraft:carrot") && !is_feed("minecraft:stone"));
    }
}
