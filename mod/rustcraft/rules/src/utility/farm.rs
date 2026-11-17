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
/// Slice 18 (guide `shearer`): the animals it milks, a bucket each into its tank.
pub fn milked(kind: &str) -> bool {
    matches!(kind, "minecraft:cow" | "minecraft:mooshroom" | "minecraft:goat")
}

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

/// Slice 18 (guide `feeder`): «детёнышей кормушка подкармливает, и они растут быстрее»: a fed
/// baby's age after it eats, as the game's own feeding ages it (`AgeableMob.ageUp`: a tenth of
/// the time left, in whole seconds).
pub fn baby_fed(age: i32) -> i32 {
    if age >= 0 {
        return age;
    }
    let seconds = ((-age / 20) as f32 * 0.1) as i32;
    (age + seconds * 20).min(0)
}

/// The first baby of a kind whose food the feeder holds that takes food, and the food.
pub fn baby_to_feed<'a>(animals: &'a [Fed], foods: &[String]) -> Option<(&'a Fed, &'static str)> {
    animals.iter().filter(|a| a.animal.baby && a.takes_food).find_map(|a| {
        food_of(&a.animal.kind)
            .iter()
            .find(|f| foods.iter().any(|g| g == *f))
            .map(|f| (a, *f))
    })
}

/// «Предел животных»: a kind the feeder has food for is at the herd's limit.
pub fn herd_full(animals: &[Fed], foods: &[String]) -> bool {
    crate::butcher::ANIMALS.iter().any(|kind| {
        food_of(kind).iter().any(|f| foods.iter().any(|g| g == *f))
            && animals.iter().filter(|a| a.animal.kind == *kind && !a.animal.baby).count() >= HERD_MAX
    })
}

/// Heads of each kind round the feeder, adults and babies, kinds with none left out (guide: «в
/// окне видно, сколько голов каждого вида рядом»).
pub fn heads(animals: &[Fed]) -> Vec<(&'static str, usize)> {
    crate::butcher::ANIMALS
        .iter()
        .map(|k| (*k, animals.iter().filter(|a| a.animal.kind == *k).count()))
        .filter(|(_, n)| *n > 0)
        .collect()
}

/// Whether a column of `depths` (water blocks under each of the 5 × 5 cells) is open water.
pub fn open_water(depths: &[i32]) -> bool {
    depths.len() == (WATER_SIDE * WATER_SIDE) as usize && depths.iter().all(|d| *d >= WATER_DEPTH)
}

/// The fishing station's next wait, from a random number 0..1; slice 18 (guide `fishery`): Lure
/// takes 5 s off a level (with Lure III 15–45 s).
pub fn fish_wait(random: f64, lure: u32) -> u64 {
    let off = LURE_TICKS * u64::from(lure.min(3));
    FISH_MIN_TICKS - off + ((FISH_MAX_TICKS - FISH_MIN_TICKS) as f64 * random.clamp(0.0, 1.0)) as u64
}

/// Ticks a level of Lure takes off the wait.
pub const LURE_TICKS: u64 = 100;
/// The catch buffer: 9 slots, 18 with a storage module.
pub const FISH_BUFFER: usize = 9;

/// The catch slots a station with `storage` storage modules uses.
pub fn fish_buffer(storage: u32) -> usize {
    if storage > 0 { 2 * FISH_BUFFER } else { FISH_BUFFER }
}

/// Slice 18: the rod's last point is not spent: a rod one catch from breaking stops the station
/// («Удочка изношена»), to be mended at the repair stand.
pub fn rod_worn(damage: u32, max_damage: u32) -> bool {
    max_damage > 0 && damage + 1 >= max_damage
}

/// Slice 18: «без блоков над водой»: every cell over the water is empty.
pub fn open_sky(above: &[bool]) -> bool {
    above.iter().all(|filled| !filled)
}

/// Slice 18: what the collector takes, as its window chooses: honey (0, a glass bottle each) or
/// combs (1, nothing needed). None when it takes honey and has no bottle («Нет пустых бутылок»).
pub fn harvest_of(choice: usize, has_bottle: bool) -> Option<(&'static str, u32)> {
    match choice {
        1 => Some(harvest(false)),
        _ => has_bottle.then(|| harvest(true)),
    }
}

/// What a full hive gives: a honey bottle for a glass bottle, else its combs.
pub fn harvest(has_bottle: bool) -> (&'static str, u32) {
    if has_bottle {
        ("minecraft:honey_bottle", 1)
    } else {
        ("minecraft:honeycomb", COMBS)
    }
}

/// Wave 2 (guide `fishery`: «каждой станции нужен свой участок воды 5×5»): whether the station
/// at `me` shares its 5 × 5 with one of `others` (the level's other stations) that keeps it: their
/// patches overlap at the same height and the other comes first in block order. So of two
/// stations on one patch one fishes and the other says why it does not.
pub fn water_taken(me: crate::geom::Pos, others: &[crate::geom::Pos]) -> bool {
    others.iter().any(|o| {
        *o != me
            && o[1] == me[1]
            && (o[0] - me[0]).abs() < WATER_SIDE
            && (o[2] - me[2]).abs() < WATER_SIDE
            && *o < me
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_stations_on_one_patch_one_fishes() {
        let a = [0, 64, 0];
        let b = [3, 64, 2];
        assert!(!water_taken(a, &[a, b]), "the first keeps it");
        assert!(water_taken(b, &[a, b]), "the second shares it");
        assert!(!water_taken([5, 64, 0], &[a]), "five apart: patches of their own");
        assert!(!water_taken([2, 70, 0], &[a]), "another height");
    }

    #[test]
    fn the_window_chooses_honey_or_combs() {
        assert_eq!(harvest_of(0, true), Some(("minecraft:honey_bottle", 1)));
        assert_eq!(harvest_of(0, false), None);
        assert_eq!(harvest_of(1, true), Some(("minecraft:honeycomb", COMBS)));
    }

    #[test]
    fn cows_mooshrooms_and_goats_are_milked() {
        assert!(milked("minecraft:goat") && milked("minecraft:cow") && milked("minecraft:mooshroom"));
        assert!(!milked("minecraft:sheep"));
    }

    #[test]
    fn babies_are_fed_heads_counted_and_a_full_herd_named() {
        assert_eq!(baby_fed(-24000), -24000 + 120 * 20);
        assert_eq!(baby_fed(-100), -100, "less than a tenth of a second's worth");
        assert_eq!(baby_fed(5), 5);
        let mut calf = fed(9, "minecraft:cow", true);
        calf.animal.baby = true;
        let animals = vec![fed(1, "minecraft:cow", false), calf];
        let wheat = vec!["minecraft:wheat".to_owned()];
        assert_eq!(baby_to_feed(&animals, &wheat).map(|(a, f)| (a.animal.id, f)), Some((9, "minecraft:wheat")));
        assert_eq!(heads(&animals), vec![("minecraft:cow", 2)]);
        let full: Vec<Fed> = (0..HERD_MAX as u32).map(|i| fed(i, "minecraft:cow", true)).collect();
        assert!(herd_full(&full, &wheat));
        assert!(!herd_full(&animals, &wheat));
    }

    #[test]
    fn lure_shortens_the_wait_and_the_rod_keeps_its_last_point() {
        assert_eq!(fish_wait(0.0, 0), 600);
        assert_eq!(fish_wait(1.0, 0), 1200);
        // Lure III: 15–45 s.
        assert_eq!(fish_wait(0.0, 3), 300);
        assert_eq!(fish_wait(1.0, 3), 900);
        assert_eq!(fish_wait(0.0, 9), 300, "no more than III counts");
        assert!(rod_worn(63, 64));
        assert!(!rod_worn(62, 64));
        assert!(!rod_worn(5, 0));
        assert_eq!(fish_buffer(0), 9);
        assert_eq!(fish_buffer(1), 18);
        assert!(open_sky(&[false; 24]));
        assert!(!open_sky(&[false, true]));
    }

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
        assert!((FISH_MIN_TICKS..=FISH_MAX_TICKS).contains(&fish_wait(0.5, 0)));
        assert_eq!(harvest(false), ("minecraft:honeycomb", 3));
        assert!(is_feed("minecraft:carrot") && !is_feed("minecraft:stone"));
    }
}
