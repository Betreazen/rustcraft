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

/// 0.21 (round 32): the food the feeder can use now: what the kinds round it eat that it may feed,
/// a pair of adults under the herd's limit or a baby; sorted, each once. The magistral brings it
/// only these (`feeder_net_takes`). Before, it took any feed, and the wheat, carrots and seeds
/// that rode to the storage sat in a feeder whose animals eat none of them.
pub fn eaten_now(animals: &[Fed]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = crate::butcher::ANIMALS
        .iter()
        .filter(|kind| {
            let of_kind = || animals.iter().filter(|a| a.animal.kind == **kind);
            let adults = of_kind().filter(|a| !a.animal.baby).count();
            (2..HERD_MAX).contains(&adults) || of_kind().any(|a| a.animal.baby)
        })
        .flat_map(|kind| food_of(kind).iter().copied())
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// 0.21: whether the magistral brings the feeder `item`: food its animals eat now (`eaten`, its
/// last look round, `eaten_now`). A hopper and a hand still give it any feed (`is_feed`).
pub fn feeder_net_takes(eaten: &[String], item: &str) -> bool {
    eaten.iter().any(|e| e == item)
}

/// 0.21 (round 32): whether the apiary collector takes glass bottles from the magistral: only
/// while its window chooses honey (`choice` 0, `harvest_of`); for combs it needs none, and
/// before it kept the bottles that rode to the storage.
pub fn apiary_takes_bottles(choice: usize) -> bool {
    choice != 1
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
    open_water_beside(depths, &[])
}

/// 0.18.4: as `open_water`, but a cell `line` marks (the magistral comes to the station over it)
/// may stand on a pier: the line can only reach the station from its side, so its cells and what
/// holds them up do not count. The station's own cell, the middle one, never does.
pub fn open_water_beside(depths: &[i32], line: &[bool]) -> bool {
    let middle = depths.len() / 2;
    depths.len() == (WATER_SIDE * WATER_SIDE) as usize
        && depths
            .iter()
            .enumerate()
            .all(|(i, d)| *d >= WATER_DEPTH || (i != middle && line.get(i).copied().unwrap_or(false)))
}

/// 0.21 (owner 2026-11-19: «смягчи правило»): the open water cells a pond must have when the
/// 5 × 5 under a station is not all water: a station over water 2 deep fishes when the water of
/// that depth joined to the cell under it has at least this many cells.
pub const POND_CELLS: usize = 25;
/// How far from the station the pond is searched, blocks either way.
pub const POND_REACH: i32 = 16;

/// The cells of the pond under a station: from `start`, the cells joined side by side for which
/// `deep` holds (water 2 deep), no farther than `POND_REACH`; counting stops at `need`.
pub fn pond_cells(start: (i32, i32), deep: impl Fn(i32, i32) -> bool, need: usize) -> usize {
    if !deep(start.0, start.1) {
        return 0;
    }
    let mut seen = std::collections::BTreeSet::from([start]);
    let mut todo = vec![start];
    while let Some((x, z)) = todo.pop() {
        if seen.len() >= need {
            break;
        }
        for (nx, nz) in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
            let near = (nx - start.0).abs() <= POND_REACH && (nz - start.1).abs() <= POND_REACH;
            if near && !seen.contains(&(nx, nz)) && deep(nx, nz) {
                seen.insert((nx, nz));
                todo.push((nx, nz));
            }
        }
    }
    seen.len().min(need)
}

/// 0.21: whether a station has water enough: the 5 × 5 under it open as before (0.18.4, the line
/// aside), or the cell under it 2 deep in a pond of `POND_CELLS` (`pond`, from `pond_cells`).
pub fn water_enough(depths: &[i32], line: &[bool], pond: usize) -> bool {
    let middle = depths.len() / 2;
    open_water_beside(depths, line)
        || (depths.get(middle).is_some_and(|d| *d >= WATER_DEPTH) && pond >= POND_CELLS)
}

/// The fishing station's next wait, from a random number 0..1; slice 18 (guide `fishery`): Lure
/// takes 5 s off a level (with Lure III 15–45 s).
pub fn fish_wait(random: f64, lure: u32) -> u64 {
    let off = LURE_TICKS * u64::from(lure.min(3));
    FISH_MIN_TICKS - off + ((FISH_MAX_TICKS - FISH_MIN_TICKS) as f64 * random.clamp(0.0, 1.0)) as u64
}

/// Ticks a level of Lure takes off the wait.
pub const LURE_TICKS: u64 = 100;
/// The catch buffer: 9 slots, twice as many a storage module (A9: modules stack in full).
pub const FISH_BUFFER: usize = 9;
/// The catch slots its window has: 18 (two rows).
pub const FISH_SHOWN: usize = 2 * FISH_BUFFER;

/// The catch cells a station with `storage` storage modules has: 9, 18, 36, 72 (slice 18, wave 2:
/// ×2 each, guide `mod_storage`).
pub fn fish_buffer(storage: u32) -> usize {
    FISH_BUFFER << storage.min(3)
}

/// Of them, the cells past the window's 18, kept in the block entity (a menu slot holds at most
/// 99, SRC/docs/native-mods.md:4617-4627, and the window has room for two rows).
pub fn fish_reserve(storage: u32) -> usize {
    fish_buffer(storage).saturating_sub(FISH_SHOWN)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_station_at_the_edge_of_a_big_pond_has_water_enough() {
        // A strip 6 wide and 10 long, 2 deep: 60 cells; a station at its edge has water under it
        // but not the 5 × 5.
        let deep = |x: i32, z: i32| (0..10).contains(&x) && (0..6).contains(&z);
        assert_eq!(pond_cells((0, 0), deep, POND_CELLS), POND_CELLS);
        assert_eq!(pond_cells((0, 0), deep, 100), 60);
        assert_eq!(pond_cells((20, 20), deep, POND_CELLS), 0, "no water under it");
        let small = |x: i32, z: i32| (0..2).contains(&x) && (0..3).contains(&z);
        assert_eq!(pond_cells((0, 0), small, POND_CELLS), 6);
        let mut edge = vec![0; 25];
        edge[12] = 3;
        assert!(water_enough(&edge, &[], POND_CELLS), "the pond is big enough");
        assert!(!water_enough(&edge, &[], 6), "a pond of 6 is not");
        edge[12] = 1;
        assert!(!water_enough(&edge, &[], POND_CELLS), "1 deep under it is not");
        assert!(water_enough(&[2; 25], &[], 0), "the 5 x 5 as before");
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
        // Slice 18, wave 2: ×2 for each module; past 18 the cells are hidden.
        assert_eq!([2, 3, 4].map(fish_buffer), [36, 72, 72]);
        assert_eq!([0, 1, 2, 3].map(fish_reserve), [0, 0, 18, 54]);
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
    fn the_magistral_brings_the_feeder_the_food_of_its_animals_only() {
        // 0.21: a feeder by cows took the carrots and seeds that rode to the storage.
        let cows = [fed(1, "minecraft:cow", true), fed(2, "minecraft:cow", false)];
        assert_eq!(eaten_now(&cows), vec!["minecraft:wheat"]);
        let eaten: Vec<String> = eaten_now(&cows).into_iter().map(str::to_owned).collect();
        assert!(feeder_net_takes(&eaten, "minecraft:wheat"));
        for item in ["minecraft:carrot", "minecraft:wheat_seeds", "minecraft:potato"] {
            assert!(!feeder_net_takes(&eaten, item), "{item}");
            assert!(is_feed(item), "{item}: a hopper still gives it");
        }
        // One cow is no pair: nothing to feed it yet; a calf eats.
        assert!(eaten_now(&cows[..1]).is_empty());
        let mut calf = fed(3, "minecraft:cow", true);
        calf.animal.baby = true;
        assert_eq!(eaten_now(&[calf]), vec!["minecraft:wheat"]);
        // A full herd is bred no more.
        let full: Vec<Fed> = (0..HERD_MAX as u32).map(|i| fed(i, "minecraft:cow", true)).collect();
        assert!(eaten_now(&full).is_empty());
        // Pigs and chickens: their foods, each once and sorted.
        let farm = [
            fed(1, "minecraft:pig", true),
            fed(2, "minecraft:pig", true),
            fed(3, "minecraft:chicken", true),
            fed(4, "minecraft:chicken", true),
        ];
        let foods = eaten_now(&farm);
        assert!(foods.contains(&"minecraft:carrot") && foods.contains(&"minecraft:wheat_seeds"));
        assert!(!foods.contains(&"minecraft:wheat"));
        assert!(foods.windows(2).all(|w| w[0] < w[1]));
        assert!(eaten_now(&[]).is_empty());
    }

    #[test]
    fn the_apiary_takes_bottles_only_for_honey() {
        assert!(apiary_takes_bottles(0));
        assert!(!apiary_takes_bottles(1), "combs need no bottle");
    }

    #[test]
    fn open_water_needs_every_cell_two_deep() {
        assert!(open_water(&[2; 25]));
        let mut shallow = [3; 25];
        shallow[7] = 1;
        assert!(!open_water(&shallow));
        assert!(!open_water(&[3; 9]));
        // 0.18.4: a pier under the line beside the station is no water, and does not matter.
        let mut pier = [2; 25];
        let mut line = [false; 25];
        for i in [2, 7] {
            pier[i] = 0;
            line[i] = true;
        }
        assert!(!open_water(&pier));
        assert!(open_water_beside(&pier, &line));
        pier[17] = 0;
        assert!(!open_water_beside(&pier, &line), "a pier off the line still counts");
        let mut under = [2; 25];
        under[12] = 0;
        let all = [true; 25];
        assert!(!open_water_beside(&under, &all), "the station itself stands over water");
        assert!((FISH_MIN_TICKS..=FISH_MAX_TICKS).contains(&fish_wait(0.5, 0)));
        assert_eq!(harvest(false), ("minecraft:honeycomb", 3));
        assert!(is_feed("minecraft:carrot") && !is_feed("minecraft:stone"));
    }
}
