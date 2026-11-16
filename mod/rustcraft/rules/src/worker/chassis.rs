//! The chassis's tiers (guide `chassis`, spec 12.1 U4): sparksteel with 3 sockets for upgrades
//! and modules, crystallite with 5, starsteel with 7. A higher chassis clicked on a worker takes
//! the place of its old one: tool, upgrades and hold stay, the old chassis goes back to the
//! player.
//!
//! Stream 3 of slice 18 builds the general tier mechanism `rustcraft_rules::tiers` (tier items,
//! the U2 swap keeping contents). Until it is merged this module keeps the same shape — the
//! tier's item, the item's tier, and a swap that answers what goes back — and the integrator
//! switches it over after the merge.

use super::Worker;
use crate::tuning::Tuning;

/// The chassis items by tier, 1 to 3.
pub const ITEMS: [&str; 3] = ["rustcraft:chassis", "rustcraft:chassis_2", "rustcraft:chassis_3"];
/// Sockets for upgrades and modules by tier (guide: «3 у шасси из искростали, 5 из кристаллита,
/// 7 из звёздной стали»).
pub const SOCKETS: [usize; 3] = [3, 5, 7];
/// The most sockets a chassis has: the window's row of them.
pub const MAX_SOCKETS: usize = 7;

/// The tier of a chassis item.
pub fn tier_of(item: &str) -> Option<u8> {
    ITEMS.iter().position(|i| *i == item).map(|i| i as u8 + 1)
}

/// The item of a chassis of `tier` (held to 1..=3).
pub fn item(tier: u8) -> &'static str {
    ITEMS[usize::from(tier.clamp(1, 3)) - 1]
}

/// Sockets of a chassis of `tier` (held to 1..=3).
pub fn sockets(tier: u8) -> usize {
    SOCKETS[usize::from(tier.clamp(1, 3)) - 1]
}

/// Why a chassis does not take another's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not a chassis item.
    NotAChassis,
    /// The same tier or a lower one: it would only lose sockets.
    NotHigher,
}

/// U4: the chassis `new_item` takes worker `w`'s place: its tier and sockets grow, its tool,
/// upgrades and hold stay. The old chassis's item, which goes back to the player.
pub fn swap(w: &mut Worker, new_item: &str, t: &Tuning) -> Result<&'static str, Refused> {
    let tier = tier_of(new_item).ok_or(Refused::NotAChassis)?;
    if tier <= w.tier {
        return Err(Refused::NotHigher);
    }
    let old = item(w.tier);
    w.set_tier(tier, t);
    Ok(old)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;
    use crate::worker::Tool;

    #[test]
    fn each_tier_has_its_sockets() {
        let t = Tuning::default();
        assert_eq!(SOCKETS[0], t.worker_upgrade_slots);
        for tier in 1..=3u8 {
            assert_eq!(tier_of(item(tier)), Some(tier));
            let mut w = Worker::new(1, 7, [0, 0, 0], &t);
            w.set_tier(tier, &t);
            assert_eq!(w.upgrades.len(), sockets(tier));
        }
        assert_eq!(tier_of("rustcraft:tool_saw"), None);
        assert_eq!(MAX_SOCKETS, SOCKETS[2]);
    }

    #[test]
    fn a_higher_chassis_swaps_keeping_tool_and_hold() {
        let t = Tuning::default();
        let mut w = Worker::new(1, 7, [0, 0, 0], &t);
        w.tool = Some(Tool::Saw);
        w.upgrades = vec![Some(Module::Battery), None, Some(Module::Storage)];
        w.refit(&t);
        w.hold.add("minecraft:oak_sapling", 5, 64);
        assert_eq!(swap(&mut w, "rustcraft:chassis_3", &t), Ok("rustcraft:chassis"));
        assert_eq!(w.tier, 3);
        assert_eq!(w.upgrades.len(), 7);
        assert_eq!(&w.upgrades[..3], &[Some(Module::Battery), None, Some(Module::Storage)]);
        assert_eq!((w.tool, w.hold.count("minecraft:oak_sapling")), (Some(Tool::Saw), 5));
    }

    #[test]
    fn a_lower_or_same_chassis_does_not_swap() {
        let t = Tuning::default();
        let mut w = Worker::new(1, 7, [0, 0, 0], &t);
        w.set_tier(2, &t);
        assert_eq!(swap(&mut w, "rustcraft:chassis_2", &t), Err(Refused::NotHigher));
        assert_eq!(swap(&mut w, "rustcraft:chassis", &t), Err(Refused::NotHigher));
        assert_eq!(swap(&mut w, "minecraft:stone", &t), Err(Refused::NotAChassis));
        assert_eq!(w.upgrades.len(), 5);
    }
}
