//! What a stored item goes under. The storage keeps counts, not stacks, so two stacks that differ
//! must have different keys: a worn tool is kept apart from a whole one (TM1: «предметы с разной
//! прочностью показываются отдельно»). The key is the item's id, with its damage after `@` when
//! it is worn. Stacks with any other component (a name, enchantments, a book's pages) go on the
//! item nets under their id with a mark after `#`: other blocks take them as any stack of the
//! item, but the storage, which keeps no components, does not, and they stay where they are (M6).
//! So such a stack never stands first in a source's queue as something the storage seems to want.

use crate::net::items::ItemKey;

/// The mark between an item's id and its damage; no item id has it.
const DAMAGE_MARK: char = '@';
/// The mark after an item's id for a stack with components the storage cannot keep.
const COMPONENTS_MARK: char = '#';

/// The key of an item with `damage`.
pub fn key(item: &str, damage: u32) -> ItemKey {
    if damage == 0 {
        item.to_owned()
    } else {
        format!("{item}{DAMAGE_MARK}{damage}")
    }
}

/// The key of a stack on an item net: as `key`, or marked when it has other components.
pub fn stack_key(item: &str, damage: u32, components: bool) -> ItemKey {
    if components {
        format!("{item}{COMPONENTS_MARK}")
    } else {
        key(item, damage)
    }
}

/// Whether the storage can keep what `key` names: no components besides the damage.
pub fn storable(key: &str) -> bool {
    !key.contains(COMPONENTS_MARK)
}

/// The item and the damage a key names.
pub fn parse(key: &str) -> (&str, u32) {
    if let Some(item) = key.strip_suffix(COMPONENTS_MARK) {
        return (item, 0);
    }
    match key.rsplit_once(DAMAGE_MARK) {
        Some((item, damage)) => match damage.parse() {
            Ok(damage) => (item, damage),
            Err(_) => (key, 0),
        },
        None => (key, 0),
    }
}

/// The item a key names, without its damage.
pub fn item_of(key: &str) -> &str {
    parse(key).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_worn_tool_has_a_key_of_its_own() {
        // Rule TM1.
        let whole = key("minecraft:iron_pickaxe", 0);
        let worn = key("minecraft:iron_pickaxe", 12);
        assert_ne!(whole, worn);
        assert_eq!(parse(&whole), ("minecraft:iron_pickaxe", 0));
        assert_eq!(parse(&worn), ("minecraft:iron_pickaxe", 12));
        assert_eq!(item_of(&worn), "minecraft:iron_pickaxe");
    }

    #[test]
    fn a_stack_with_components_is_marked_and_not_storable() {
        let named = stack_key("minecraft:diamond_sword", 0, true);
        assert!(!storable(&named));
        assert_eq!(item_of(&named), "minecraft:diamond_sword");
        let worn = stack_key("minecraft:iron_pickaxe", 50, false);
        assert!(storable(&worn));
        assert_eq!(worn, key("minecraft:iron_pickaxe", 50));
    }

    #[test]
    fn a_key_without_a_number_after_the_mark_is_the_whole_item() {
        assert_eq!(parse("mod:odd@name"), ("mod:odd@name", 0));
    }
}
