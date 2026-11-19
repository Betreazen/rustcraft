//! 0.21 (spec 45, CR1–CR6; the owner: «Вырежи из игры вообще каналы … если уже что-то скрафчено,
//! то пускай разберутся на блоки, из которых делаются»): the channel items of 0.19 are gone.
//!
//! Since round 31 every housing has its three channels built in, and the items that once carried
//! them were only taken apart. They stay registered, so a saved stack does not turn into air, but
//! no list shows them (CR1), and wherever the mod sees one it becomes what it was made of (CR2):
//! eight channels of a tier were four ingots of its alloy and a hopper, so two channels give one
//! ingot. An odd channel gives a whole ingot too (CR3): no channel is made any more, so nothing
//! can be gained by it over and over. A recipe that takes an old channel is no way to make
//! anything (CR5): not for the handbook's tables, the order planner or the disintegrator.

use std::collections::BTreeMap;

use crate::net::topology::{Channel, ChannelKind};
use crate::storage::{Store, keys};
use crate::tiers::ALLOYS;
use crate::worker::hold::Hold;

/// Every old channel item: its id, its kind and its tier. Its alloy is its tier's.
pub const OLD_CHANNELS: [(&str, ChannelKind, u8); 12] = [
    ("rustcraft:item_channel", ChannelKind::Item, 1),
    ("rustcraft:item_channel_2", ChannelKind::Item, 2),
    ("rustcraft:item_channel_3", ChannelKind::Item, 3),
    ("rustcraft:item_channel_4", ChannelKind::Item, 4),
    ("rustcraft:fluid_channel", ChannelKind::Fluid, 1),
    ("rustcraft:fluid_channel_2", ChannelKind::Fluid, 2),
    ("rustcraft:fluid_channel_3", ChannelKind::Fluid, 3),
    ("rustcraft:fluid_channel_4", ChannelKind::Fluid, 4),
    ("rustcraft:energy_channel", ChannelKind::Energy, 1),
    ("rustcraft:energy_channel_2", ChannelKind::Energy, 2),
    ("rustcraft:energy_channel_3", ChannelKind::Energy, 3),
    ("rustcraft:energy_channel_4", ChannelKind::Energy, 4),
];

/// How many channels make one ingot of their alloy.
pub const PER_INGOT: u32 = 2;

/// Whether `item` (an id, or a storage's key of it) is an old channel item.
pub fn is_old_channel(item: &str) -> bool {
    parts_of(item).is_some()
}

/// The alloy an old channel item is made of; none for any other item.
pub fn parts_of(item: &str) -> Option<&'static str> {
    // Asked for every carried stack every tick: most items stop here.
    if !item.contains("_channel") {
        return None;
    }
    let id = keys::item_of(item);
    OLD_CHANNELS
        .iter()
        .find(|(c, _, _)| *c == id)
        .and_then(|(_, _, tier)| ALLOYS.get(usize::from(*tier) - 1).copied())
}

/// The item that carried a channel of `kind` and `tier`; a tier past 4 gives the highest.
pub fn item_of(kind: ChannelKind, tier: u8) -> &'static str {
    OLD_CHANNELS
        .iter()
        .filter(|(_, k, t)| *k == kind && *t <= tier.max(1))
        .last()
        .map_or(OLD_CHANNELS[0].0, |(id, _, _)| id)
}

/// Ingots `count` channels give (CR2, CR3): one for every two, an odd one rounded up.
pub fn ingots(count: u32) -> u32 {
    count.div_ceil(PER_INGOT)
}

/// What a stack of `count` of `item` becomes: its alloy and how many; none when it is no old
/// channel (or empty).
pub fn convert(item: &str, count: u32) -> Option<(&'static str, u32)> {
    let alloy = parts_of(item)?;
    (count > 0).then(|| (alloy, ingots(count)))
}

/// What the channels of a housing saved by 0.19 give when it is broken before it was read
/// again: the alloys of all of them, the same kind and tier counted together (CR4).
pub fn parts_of_channels(channels: impl IntoIterator<Item = Channel>) -> Vec<(&'static str, u32)> {
    let mut by_item: BTreeMap<&'static str, u32> = BTreeMap::new();
    for c in channels {
        *by_item.entry(item_of(c.kind, c.tier)).or_default() += 1;
    }
    let mut out: BTreeMap<&'static str, u32> = BTreeMap::new();
    for (item, n) in by_item {
        if let Some((alloy, k)) = convert(item, n) {
            *out.entry(alloy).or_default() += k;
        }
    }
    out.into_iter().collect()
}

/// A storage's old channels turned into their alloy (CR2), whatever its room and limits: fewer
/// slots are taken after than before. How many channels went.
pub fn convert_store(store: &mut Store) -> u64 {
    let old: Vec<(String, u64)> = store
        .items
        .iter()
        .filter(|(k, _)| is_old_channel(k))
        .map(|(k, n)| (k.clone(), *n))
        .collect();
    let mut gone = 0;
    for (key, n) in old {
        let Some(alloy) = parts_of(&key) else { continue };
        store.items.remove(&key);
        store.limits.remove(&key);
        store.keep.remove(&key);
        store.restore(alloy, n.div_ceil(u64::from(PER_INGOT)));
        gone += n;
    }
    gone
}

/// A worker's hold with its old channels turned into their alloy, each slot in place (CR2):
/// whether any changed.
pub fn convert_hold(hold: &mut Hold) -> bool {
    let mut changed = false;
    for slot in hold.slots.iter_mut().flatten() {
        if let Some((alloy, n)) = convert(&slot.0, slot.1) {
            *slot = (alloy.to_owned(), n);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Limit;

    fn repo_file(rest: &str) -> Option<String> {
        std::fs::read_to_string(format!("{}/../../../{rest}", env!("CARGO_MANIFEST_DIR"))).ok()
    }

    #[test]
    fn every_channel_and_tier_gives_its_tiers_alloy() {
        let want = [
            ("rustcraft:item_channel", "rustcraft:coppersteel"),
            ("rustcraft:item_channel_2", "rustcraft:sparksteel"),
            ("rustcraft:item_channel_3", "rustcraft:crystallite"),
            ("rustcraft:item_channel_4", "rustcraft:starsteel"),
            ("rustcraft:fluid_channel", "rustcraft:coppersteel"),
            ("rustcraft:fluid_channel_2", "rustcraft:sparksteel"),
            ("rustcraft:fluid_channel_3", "rustcraft:crystallite"),
            ("rustcraft:fluid_channel_4", "rustcraft:starsteel"),
            ("rustcraft:energy_channel", "rustcraft:coppersteel"),
            ("rustcraft:energy_channel_2", "rustcraft:sparksteel"),
            ("rustcraft:energy_channel_3", "rustcraft:crystallite"),
            ("rustcraft:energy_channel_4", "rustcraft:starsteel"),
        ];
        assert_eq!(want.len(), OLD_CHANNELS.len());
        for (item, alloy) in want {
            assert_eq!(parts_of(item), Some(alloy), "{item}");
            assert_eq!(convert(item, 2), Some((alloy, 1)), "{item}");
        }
        for kind in ChannelKind::ALL {
            for tier in 1..=4 {
                assert!(is_old_channel(item_of(kind, tier)));
            }
            assert_eq!(item_of(kind, 9), item_of(kind, 4));
        }
        assert_eq!(parts_of("rustcraft:housing"), None);
        assert_eq!(parts_of("rustcraft:coppersteel"), None);
        assert_eq!(convert("minecraft:stone", 5), None);
    }

    #[test]
    fn two_channels_make_an_ingot_and_an_odd_one_a_whole_one() {
        assert_eq!(ingots(1), 1);
        assert_eq!(ingots(2), 1);
        assert_eq!(ingots(3), 2);
        assert_eq!(ingots(64), 32);
        assert_eq!(convert("rustcraft:item_channel", 0), None);
    }

    #[test]
    fn the_tables_match_the_content_mods_apart_recipes() {
        // The `_apart` recipes stay for stacks in plain chests: two of one channel, its alloy.
        for (item, _, _) in OLD_CHANNELS {
            let name = item.trim_start_matches("rustcraft:");
            let path = format!("mod/content/data/rustcraft/recipe/{name}_apart.json");
            let Some(text) = repo_file(&path) else {
                panic!("no {path}");
            };
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            let ingredients = json["ingredients"].as_array().unwrap();
            assert_eq!(ingredients.len(), PER_INGOT as usize, "{path}");
            assert!(ingredients.iter().all(|i| i == item), "{path}");
            assert_eq!(json["result"]["id"], parts_of(item).unwrap(), "{path}");
            assert_eq!(json["result"]["count"], 1, "{path}");
        }
    }

    #[test]
    fn a_storage_keeps_their_alloy_and_forgets_their_limits() {
        let mut store = Store { cells: 1, ..Store::default() };
        store.items.insert("rustcraft:item_channel".into(), 5);
        store.items.insert("rustcraft:energy_channel".into(), 2);
        store.items.insert("rustcraft:fluid_channel_3".into(), 4);
        store.items.insert("rustcraft:coppersteel".into(), 10);
        store.items.insert("minecraft:stone".into(), 7);
        store.limits.insert("rustcraft:item_channel".into(), Limit { max: 0, destroy_surplus: true });
        store.keep.insert("rustcraft:item_channel".into(), 3);
        assert_eq!(convert_store(&mut store), 11);
        assert_eq!(store.count("rustcraft:coppersteel"), 10 + 3 + 1);
        assert_eq!(store.count("rustcraft:crystallite"), 2);
        assert_eq!(store.count("minecraft:stone"), 7);
        assert!(store.items.keys().all(|k| !is_old_channel(k)));
        assert!(store.limits.is_empty() && store.keep.is_empty());
        assert_eq!(convert_store(&mut store), 0, "a second pass finds none");
    }

    #[test]
    fn a_hold_turns_each_slot_in_place() {
        let mut hold = Hold::new(3);
        hold.slots[0] = Some(("rustcraft:item_channel_4".into(), 3));
        hold.slots[2] = Some(("minecraft:oak_log".into(), 9));
        assert!(convert_hold(&mut hold));
        assert_eq!(hold.slots[0], Some(("rustcraft:starsteel".into(), 2)));
        assert_eq!(hold.slots[1], None);
        assert_eq!(hold.slots[2], Some(("minecraft:oak_log".into(), 9)));
        assert!(!convert_hold(&mut hold));
    }

    #[test]
    fn an_old_housings_channels_come_back_as_alloy() {
        let c = |kind, tier| Channel { kind, tier };
        let parts = parts_of_channels([
            c(ChannelKind::Item, 1),
            c(ChannelKind::Fluid, 1),
            c(ChannelKind::Energy, 2),
            c(ChannelKind::Item, 1),
        ]);
        // Two item channels of tier 1 one ingot, the lone fluid channel one, the energy one of
        // tier 2 one sparksteel.
        assert_eq!(parts, vec![("rustcraft:coppersteel", 2), ("rustcraft:sparksteel", 1)]);
        assert!(parts_of_channels([]).is_empty());
    }
}
