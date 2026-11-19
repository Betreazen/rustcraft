//! The wrench's bottlenecks (guide `wrench`: «ключ подсвечивает узкое место»): on a net whose
//! housings are not all of one tier, the housings of the net's lowest tier hold the whole net back
//! (M10; round 31: a housing's tier is its channels'). A client with a wrench in hand marks them.

use std::collections::BTreeSet;

use crate::geom::Pos;
use crate::net::topology::{ChannelKind, Layout, Topology};

/// The housings of their net's weakest tier while a stronger housing is on the same net.
pub fn weak_housings(topology: &Topology, layout: &Layout) -> BTreeSet<Pos> {
    let tier = |h: &Pos| layout.housings.get(h).map_or(0, |x| x.tier);
    // Every kind's nets are the same housings (round 31): the item nets name them all.
    topology
        .nets
        .iter()
        .filter(|net| net.kind == ChannelKind::Item)
        .filter(|net| net.housings.iter().map(tier).max().unwrap_or(0) > net.min_tier)
        .flat_map(|net| {
            net.housings
                .iter()
                .copied()
                .filter(move |h| tier(h) == net.min_tier)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::topology::{Housing, Node, analyse};
    use crate::tuning::Tuning;

    #[test]
    fn the_weakest_housings_of_a_mixed_net_are_the_bottleneck() {
        let mut l = Layout::default();
        l.nodes.insert([0, 0, 0], Node { tier: 1 });
        for (x, t) in [(1, 3), (2, 1), (3, 1), (4, 3)] {
            l.housings.insert([x, 0, 0], Housing::new(t));
        }
        let topo = analyse(&l, &Tuning::default());
        let weak = weak_housings(&topo, &l);
        assert_eq!(weak.into_iter().collect::<Vec<_>>(), vec![[2, 0, 0], [3, 0, 0]]);
    }

    #[test]
    fn a_net_of_one_tier_has_no_bottleneck() {
        let mut l = Layout::default();
        l.nodes.insert([0, 0, 0], Node { tier: 1 });
        l.housings.insert([1, 0, 0], Housing::new(2));
        l.housings.insert([2, 0, 0], Housing::new(2));
        let topo = analyse(&l, &Tuning::default());
        assert!(weak_housings(&topo, &l).is_empty());
    }
}
