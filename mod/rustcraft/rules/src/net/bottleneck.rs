//! The wrench's bottlenecks (guide `wrench`, `ch_item`: «ключ подсвечивает узкое место»): on a
//! channel net whose channels are not all of one tier, the housings whose channel in that slot is
//! of the net's lowest tier hold the whole net back (M10). Each housing gets a mask of such slots;
//! a client with a wrench in hand marks them.

use std::collections::BTreeMap;

use crate::geom::Pos;
use crate::net::topology::{Layout, Topology};

/// Each housing with a bottleneck: bit `slot` set for each slot whose channel is its net's weakest
/// while a stronger one is on the same net.
pub fn weak_slots(topology: &Topology, layout: &Layout) -> BTreeMap<Pos, u8> {
    let mut out: BTreeMap<Pos, u8> = BTreeMap::new();
    for net in &topology.nets {
        let tier = |h: &Pos| layout.housings.get(h).and_then(|x| x.slots[net.slot]).map_or(0, |c| c.tier);
        let max = net.housings.iter().map(tier).max().unwrap_or(0);
        if max <= net.min_tier {
            continue;
        }
        for h in net.housings.iter().filter(|h| tier(h) == net.min_tier) {
            *out.entry(*h).or_default() |= 1 << net.slot;
        }
    }
    out
}

/// The colour a channel's strip is drawn in on the housing (data.js: the tubes' icons).
pub fn strip_colour(kind: crate::net::topology::ChannelKind) -> u32 {
    use crate::net::topology::ChannelKind::*;
    match kind {
        Item => 0xFFA53C,
        Fluid => 0x4C9BF5,
        Energy => 0xFFE14A,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::topology::{Channel, ChannelKind, Housing, Node, analyse};
    use crate::tuning::Tuning;

    fn h(tier: u8) -> Housing {
        Housing { slots: [Some(Channel { kind: ChannelKind::Item, tier }), None, None, None] }
    }

    #[test]
    fn the_weakest_channels_of_a_mixed_net_are_the_bottleneck() {
        let mut l = Layout::default();
        l.nodes.insert([0, 0, 0], Node { tier: 1 });
        for (x, t) in [(1, 3), (2, 1), (3, 1), (4, 3)] {
            l.housings.insert([x, 0, 0], h(t));
        }
        let topo = analyse(&l, &Tuning::default());
        let weak = weak_slots(&topo, &l);
        assert_eq!(weak.keys().copied().collect::<Vec<_>>(), vec![[2, 0, 0], [3, 0, 0]]);
        assert_eq!(weak[&[2, 0, 0]], 1);
    }

    #[test]
    fn a_net_of_one_tier_has_no_bottleneck() {
        let mut l = Layout::default();
        l.nodes.insert([0, 0, 0], Node { tier: 1 });
        l.housings.insert([1, 0, 0], h(2));
        l.housings.insert([2, 0, 0], h(2));
        let topo = analyse(&l, &Tuning::default());
        assert!(weak_slots(&topo, &l).is_empty());
        assert_eq!(strip_colour(ChannelKind::Fluid), 0x4C9BF5);
    }
}
