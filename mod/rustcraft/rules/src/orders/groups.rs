//! 0.21 (spec 47, OT4): the groups of items a recipe input may take any of — the game's item
//! tags (`#minecraft:planks`) as the recipe book holds them, a list of items. When nothing of such
//! an input is in the storage and nothing makes it, the order names the group («доски любого
//! дерева»), not the first item of the list (the owner: «ждёт только определённое бревно»).
//!
//! A group's key is `group:` and its name (`group:planks`); the terminal's list numbers group `k`
//! of GROUPS by GROUP_NUMBERS plus `k`, below the fluids' numbers and past every item's.

use crate::net::items::ItemKey;

/// A group's key starts with this, so no item id (`namespace:path`) and no fluid's key is one.
pub const GROUP_PREFIX: &str = "group:";
/// The groups an order can name, in the order the terminal's list numbers them: new ones last.
pub const GROUPS: [&str; 5] = ["planks", "logs", "wool", "coals", "stone"];
/// The terminal's number of group `k` of GROUPS is this plus `k`.
pub const GROUP_NUMBERS: u16 = 0xFE00;

/// What each of the stone tools' and the furnace's materials may be.
const STONES: [&str; 3] = [
    "minecraft:cobblestone",
    "minecraft:blackstone",
    "minecraft:cobbled_deepslate",
];
const COALS: [&str; 2] = ["minecraft:coal", "minecraft:charcoal"];
/// The endings of a log's id, of every wood (the nether's stems and hyphae too).
const LOG_ENDS: [&str; 4] = ["_log", "_wood", "_stem", "_hyphae"];

/// The wood of a log's id (`minecraft:stripped_birch_wood` → `birch`); none for no log.
fn wood_of(item: &str) -> Option<&str> {
    let path = item.rsplit(':').next().unwrap_or(item);
    let path = path.strip_prefix("stripped_").unwrap_or(path);
    LOG_ENDS.iter().find_map(|end| path.strip_suffix(end))
}

/// The group the items of one input make up; none for a list of one item or one no group names
/// (the logs of one wood are «дубовое бревно», not a group).
pub fn group_of(alts: &[ItemKey]) -> Option<&'static str> {
    if alts.len() < 2 {
        return None;
    }
    let all = |f: &dyn Fn(&str) -> bool| alts.iter().all(|a| f(a));
    if all(&|a| a.ends_with("_planks")) {
        return Some(GROUPS[0]);
    }
    if all(&|a| wood_of(a).is_some()) {
        let mut woods: Vec<&str> = alts.iter().filter_map(|a| wood_of(a)).collect();
        woods.sort_unstable();
        woods.dedup();
        return (woods.len() > 1).then_some(GROUPS[1]);
    }
    if all(&|a| a.ends_with("_wool")) {
        return Some(GROUPS[2]);
    }
    if all(&|a| COALS.contains(&a)) {
        return Some(GROUPS[3]);
    }
    if all(&|a| STONES.contains(&a)) {
        return Some(GROUPS[4]);
    }
    None
}

/// The key of a group (`planks` → `group:planks`).
pub fn group_key(group: &str) -> ItemKey {
    format!("{GROUP_PREFIX}{group}")
}

/// The group a key names; none for an item's or a fluid's key.
pub fn group_of_key(key: &str) -> Option<&str> {
    key.strip_prefix(GROUP_PREFIX)
}

/// The number the terminal's list gives a group's key; none for any other key.
pub fn group_number(key: &str) -> Option<u16> {
    let group = group_of_key(key)?;
    GROUPS
        .iter()
        .position(|g| *g == group)
        .map(|k| GROUP_NUMBERS + k as u16)
}

/// The group a number of the terminal's list stands for; none for an item's or a fluid's.
pub fn group_of_number(number: u16) -> Option<&'static str> {
    GROUPS
        .get(usize::from(number.checked_sub(GROUP_NUMBERS)?))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orders::{FLUID_NUMBERS, FLUIDS};

    fn keys(items: &[&str]) -> Vec<ItemKey> {
        items.iter().map(|i| (*i).to_string()).collect()
    }

    #[test]
    fn the_game_tags_of_recipes_are_named_groups() {
        let planks = keys(&["minecraft:oak_planks", "minecraft:birch_planks", "minecraft:crimson_planks"]);
        assert_eq!(group_of(&planks), Some("planks"));
        let logs = keys(&["minecraft:oak_log", "minecraft:stripped_birch_wood", "minecraft:warped_stem"]);
        assert_eq!(group_of(&logs), Some("logs"));
        let wool = keys(&["minecraft:white_wool", "minecraft:red_wool"]);
        assert_eq!(group_of(&wool), Some("wool"));
        assert_eq!(group_of(&keys(&["minecraft:coal", "minecraft:charcoal"])), Some("coals"));
        let stone = keys(&["minecraft:cobblestone", "minecraft:blackstone", "minecraft:cobbled_deepslate"]);
        assert_eq!(group_of(&stone), Some("stone"));
    }

    #[test]
    fn one_item_or_the_logs_of_one_wood_are_no_group() {
        assert_eq!(group_of(&keys(&["minecraft:oak_planks"])), None);
        let oak = keys(&[
            "minecraft:oak_log",
            "minecraft:oak_wood",
            "minecraft:stripped_oak_log",
            "minecraft:stripped_oak_wood",
        ]);
        assert_eq!(group_of(&oak), None);
        assert_eq!(group_of(&keys(&["minecraft:coal", "minecraft:iron_ingot"])), None);
    }

    #[test]
    fn group_numbers_lie_between_the_items_and_the_fluids() {
        for (k, g) in GROUPS.iter().enumerate() {
            let n = group_number(&group_key(g)).unwrap();
            assert_eq!(n, GROUP_NUMBERS + k as u16);
            assert_eq!(group_of_number(n), Some(*g));
            assert!(n < FLUID_NUMBERS);
        }
        assert_eq!(group_number("minecraft:oak_planks"), None);
        assert_eq!(group_of_number(FLUID_NUMBERS), None);
        assert_eq!(group_of_number(GROUP_NUMBERS - 1), None);
        assert!(FLUIDS.len() < usize::from(u16::MAX - FLUID_NUMBERS));
    }
}
