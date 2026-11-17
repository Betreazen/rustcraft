//! Slice 18: module sockets by tier (A5) for every machine, the sheets' and the drawn windows'
//! alike, and how a drawn window without a sheet (the furnace, the crusher, the alloy smelter, the
//! pump) learns its sockets through its menu's data slots.
//!
//! A tiered block's tier is its state's (rustcraft_rules::tiers); its block entity keeps a copy
//! under [`TIER_KEY`] so that the code that sees only the block entity (a window's button) knows
//! how many sockets are open.

use crate::modules::{ITEMS, Module};
use crate::tiers;

/// The block entity's copy of its block's tier.
pub const TIER_KEY: &str = "tier";
/// Data slots a socket window carries: the open sockets, then each socket's module.
pub const DATA: usize = 1 + tiers::MAX_SOCKETS;

/// How many sockets the block `block` has open, its block entity saying `saved` for its tier:
/// 3 / 3 / 4 / 4 by tier for a tiered block, 3 for any other.
pub fn open(block: &str, saved: Option<i32>) -> usize {
    match tiers::family_of_block(block) {
        Some(f) => tiers::sockets(f.tier_from(saved.map(|t| t.to_string()).as_deref())),
        None => crate::machines::sheet::SOCKETS,
    }
}

/// A module as a data slot: 0 for none, else 1 + its place in `modules::ITEMS`.
pub fn code(m: Option<Module>) -> i32 {
    m.and_then(|m| ITEMS.iter().position(|(x, _)| *x == m))
        .map_or(0, |i| i as i32 + 1)
}

pub fn module_of(code: i32) -> Option<Module> {
    usize::try_from(code - 1).ok().and_then(|i| ITEMS.get(i)).map(|(m, _)| *m)
}

/// The data slots of `open` sockets holding `slots`.
pub fn encode(open: usize, slots: &[Option<Module>]) -> [i32; DATA] {
    let mut out = [0; DATA];
    out[0] = open.min(tiers::MAX_SOCKETS) as i32;
    for (k, m) in slots.iter().take(tiers::MAX_SOCKETS).enumerate() {
        out[1 + k] = code(*m);
    }
    out
}

/// The open sockets and each socket's module, from the data slots at `data` (3 open and empty
/// before the server's first values).
pub fn decode(data: &[i32]) -> (usize, [Option<Module>; tiers::MAX_SOCKETS]) {
    let open = data
        .first()
        .and_then(|v| usize::try_from(*v).ok())
        .filter(|n| (1..=tiers::MAX_SOCKETS).contains(n))
        .unwrap_or(crate::machines::sheet::SOCKETS);
    let slots = std::array::from_fn(|k| data.get(1 + k).copied().and_then(module_of));
    (open, slots)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_sockets_by_tier() {
        assert_eq!(open("rustcraft:furnace", None), 3);
        assert_eq!(open("rustcraft:furnace", Some(2)), 3);
        assert_eq!(open("rustcraft:furnace", Some(3)), 4);
        assert_eq!(open("rustcraft:disintegrator", None), 4, "it opens on tier 3");
        assert_eq!(open("rustcraft:turret", Some(4)), 3, "no tiers: three");
    }

    #[test]
    fn sockets_survive_the_data_slots() {
        let slots = [Some(Module::Speed), None, Some(Module::Yield), Some(Module::Stack3)];
        let data = encode(4, &slots);
        assert!(data.iter().all(|v| (0..=i32::from(i16::MAX)).contains(v)));
        assert_eq!(decode(&data), (4, slots));
        assert_eq!(decode(&[]), (3, [None; 4]));
        assert_eq!(code(None), 0);
        for (m, _) in ITEMS {
            assert_eq!(module_of(code(Some(m))), Some(m));
        }
    }
}
