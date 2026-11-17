//! What a stored item goes under. The storage keeps counts, not stacks, so two stacks that differ
//! must have different keys: a worn tool is kept apart from a whole one (TM1: «предметы с разной
//! прочностью показываются отдельно»). The key is the item's id, with its damage after `@` when
//! it is worn.
//!
//! Slice 18: a stack with any other component (a name, enchantments, a potion's contents, a book's
//! pages) goes under its id, `#` and its whole stack as the game saves it (the network NBT of one
//! item, components and damage included) in hex. The storage keeps it as any other key, and a
//! stack made from the key has every component back. A stack whose saved form is longer than
//! MAX_PAYLOAD goes under its id and a bare `#`: other blocks take it as any stack of the item,
//! the storage does not, and it stays where it is (M6), so it never stands first in a source's
//! queue as something the storage seems to want.

use crate::net::items::ItemKey;

/// The mark between an item's id and its damage; no item id has it.
const DAMAGE_MARK: char = '@';
/// The mark after an item's id for a stack with components.
const COMPONENTS_MARK: char = '#';
/// The longest saved stack (hex characters) a key carries: an enchanted book is about 150, a
/// potion about 100; a written book with many pages is past it and stays out of the storage.
pub const MAX_PAYLOAD: usize = 2048;

/// The key of an item with `damage`.
pub fn key(item: &str, damage: u32) -> ItemKey {
    if damage == 0 {
        item.to_owned()
    } else {
        format!("{item}{DAMAGE_MARK}{damage}")
    }
}

/// The key of a stack on an item net: as `key`, or the bare mark when it has other components
/// (a net that has the stack's saved form uses `component_key`).
pub fn stack_key(item: &str, damage: u32, components: bool) -> ItemKey {
    if components {
        format!("{item}{COMPONENTS_MARK}")
    } else {
        key(item, damage)
    }
}

/// Slice 18: the key of a stack with components, `payload` its saved form in hex; the bare mark
/// (not storable) when the payload is empty or longer than MAX_PAYLOAD.
pub fn component_key(item: &str, payload: &str) -> ItemKey {
    if payload.is_empty() || payload.len() > MAX_PAYLOAD {
        format!("{item}{COMPONENTS_MARK}")
    } else {
        format!("{item}{COMPONENTS_MARK}{payload}")
    }
}

/// The saved stack a component key carries, in hex; none for a plain key or a bare mark.
pub fn payload(key: &str) -> Option<&str> {
    key.split_once(COMPONENTS_MARK)
        .map(|(_, p)| p)
        .filter(|p| !p.is_empty())
}

/// Whether the storage can keep what `key` names: anything but a bare components mark.
pub fn storable(key: &str) -> bool {
    match key.split_once(COMPONENTS_MARK) {
        Some((_, payload)) => !payload.is_empty(),
        None => true,
    }
}

/// Bytes as lower-case hex.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Hex back to bytes; none for an odd length or a character that is no hex digit.
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// The item and the damage a key names (a component key's damage is in its payload: 0 here).
pub fn parse(key: &str) -> (&str, u32) {
    if let Some((item, _)) = key.split_once(COMPONENTS_MARK) {
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
    fn a_bare_components_mark_is_not_storable() {
        let named = stack_key("minecraft:diamond_sword", 0, true);
        assert!(!storable(&named));
        assert_eq!(item_of(&named), "minecraft:diamond_sword");
        let worn = stack_key("minecraft:iron_pickaxe", 50, false);
        assert!(storable(&worn));
        assert_eq!(worn, key("minecraft:iron_pickaxe", 50));
    }

    #[test]
    fn a_stack_with_components_keeps_them_in_its_key() {
        // Slice 18: two enchanted books with other enchantments are two keys of one item.
        let a = component_key("minecraft:enchanted_book", &hex(&[1, 2, 0xab]));
        let b = component_key("minecraft:enchanted_book", &hex(&[1, 2, 0xac]));
        assert_ne!(a, b);
        assert!(storable(&a));
        assert_eq!(item_of(&a), "minecraft:enchanted_book");
        assert_eq!(parse(&a), ("minecraft:enchanted_book", 0));
        assert_eq!(payload(&a), Some("0102ab"));
        assert_eq!(unhex(payload(&a).unwrap()), Some(vec![1, 2, 0xab]));
        assert_eq!(payload("minecraft:stone"), None);
        assert_eq!(payload(&key("minecraft:iron_pickaxe", 4)), None);
    }

    #[test]
    fn a_payload_too_long_or_empty_is_the_bare_mark() {
        let long = "ab".repeat(MAX_PAYLOAD);
        let k = component_key("minecraft:written_book", &long);
        assert!(!storable(&k));
        assert_eq!(payload(&k), None);
        assert!(!storable(&component_key("minecraft:stone", "")));
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }

    #[test]
    fn a_key_without_a_number_after_the_mark_is_the_whole_item() {
        assert_eq!(parse("mod:odd@name"), ("mod:odd@name", 0));
    }
}
