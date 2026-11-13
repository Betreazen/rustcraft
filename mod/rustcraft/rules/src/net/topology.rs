//! Lines of the magistral and the channel networks inside them.
//!
//! A line is a group of housings joined face to face (N5: housings join in every direction) or
//! through a node. A node of tiers 1–3 joins only opposite faces, each axis a separate line (N4);
//! a node of the hub tier joins all six faces (M12). A housing works if the length of pipe from it
//! to some node is within that node's range (N3); otherwise it shows a diagnosis and takes no part
//! in any channel network. A channel continues into the next housing only in the same slot and
//! only with a channel of the same kind (M3).

use std::collections::{BTreeSet, BinaryHeap, HashMap};

use crate::geom::{Face, Pos, step};
use crate::tuning::Tuning;

/// What a channel carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChannelKind {
    Item,
    Fluid,
    Energy,
}

/// A channel set into one of a housing's four slots (M1, M2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    pub kind: ChannelKind,
    pub tier: u8,
}

/// Number of slots in a housing (M1).
pub const SLOTS: usize = 4;

/// A housing of the magistral.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Housing {
    pub slots: [Option<Channel>; SLOTS],
}

/// A node of the magistral.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub tier: u8,
}

/// The blocks of the magistral in a region of the world.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub housings: HashMap<Pos, Housing>,
    pub nodes: HashMap<Pos, Node>,
}

/// Why a housing works or does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// Within `limit` of the node at `node`, `distance` blocks of pipe away.
    Served {
        node: Pos,
        distance: u32,
        limit: u32,
    },
    /// The best node is `distance` away but serves only `limit` (diagnosis «Дальше предела узла»).
    TooFar { distance: u32, limit: u32 },
    /// No node on the line (diagnosis «Нет узла магистрали»).
    NoNode,
}

impl Reach {
    pub fn works(self) -> bool {
        matches!(self, Reach::Served { .. })
    }
}

/// The working housings that carry one channel together: same slot, same kind, joined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelNet {
    pub kind: ChannelKind,
    pub slot: usize,
    pub housings: BTreeSet<Pos>,
    /// M10: the weakest channel on the net limits what it carries.
    pub min_tier: u8,
}

/// The lines, reaches and channel networks of a layout.
#[derive(Clone, Debug, Default)]
pub struct Topology {
    /// Each line as its set of housings.
    pub lines: Vec<BTreeSet<Pos>>,
    pub reach: HashMap<Pos, Reach>,
    pub nets: Vec<ChannelNet>,
}

impl Topology {
    /// The index of the line a housing belongs to.
    pub fn line_of(&self, housing: Pos) -> Option<usize> {
        self.lines.iter().position(|l| l.contains(&housing))
    }

    /// The channel net that carries `slot` of `housing`, if the housing works and has a channel there.
    pub fn net_of(&self, housing: Pos, slot: usize) -> Option<&ChannelNet> {
        self.nets
            .iter()
            .find(|n| n.slot == slot && n.housings.contains(&housing))
    }
}

/// The steps from a housing to the housings joined to it along the pipe, with the blocks of pipe
/// each covers: 1 to a touching housing, 2 through a node (the node is a block of pipe too).
fn links(layout: &Layout, tuning: &Tuning, from: Pos) -> Vec<(Pos, u32)> {
    let mut out = Vec::new();
    for face in Face::ALL {
        let next = step(from, face);
        if layout.housings.contains_key(&next) {
            out.push((next, 1));
        } else if let Some(node) = layout.nodes.get(&next) {
            if node.tier >= tuning.hub_tier {
                // M12: a hub joins every face.
                for other in Face::ALL {
                    let beyond = step(next, other);
                    if other != face.opposite() && layout.housings.contains_key(&beyond) {
                        out.push((beyond, 2));
                    }
                }
            } else {
                // N4: the opposite face only.
                let beyond = step(next, face);
                if layout.housings.contains_key(&beyond) {
                    out.push((beyond, 2));
                }
            }
        }
    }
    out
}

/// The housings in a stable order, so results do not depend on hash order.
fn sorted_housings(layout: &Layout) -> Vec<Pos> {
    let mut all: Vec<Pos> = layout.housings.keys().copied().collect();
    all.sort_unstable();
    all
}

fn find_lines(layout: &Layout, tuning: &Tuning) -> Vec<BTreeSet<Pos>> {
    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();
    for start in sorted_housings(layout) {
        if !seen.insert(start) {
            continue;
        }
        let mut line = BTreeSet::from([start]);
        let mut stack = vec![start];
        while let Some(h) = stack.pop() {
            for (next, _) in links(layout, tuning, h) {
                if seen.insert(next) {
                    line.insert(next);
                    stack.push(next);
                }
            }
        }
        lines.push(line);
    }
    lines
}

/// The best node of each housing: the one that leaves the most range to spare (negative when
/// even the best is too far). Housings no node reaches are left out.
fn find_reach(layout: &Layout, tuning: &Tuning) -> HashMap<Pos, Reach> {
    // (spare range, distance, node, limit) of the best node found so far.
    let mut best: HashMap<Pos, (i64, u32, Pos, u32)> = HashMap::new();
    let mut queue = BinaryHeap::new();
    let mut nodes: Vec<(&Pos, &Node)> = layout.nodes.iter().collect();
    nodes.sort_unstable_by_key(|(p, _)| **p);
    for (&node, n) in nodes {
        let limit = tuning.range_of(n.tier);
        for face in Face::ALL {
            let h = step(node, face);
            if layout.housings.contains_key(&h) {
                queue.push((
                    i64::from(limit) - 1,
                    std::cmp::Reverse(1u32),
                    h,
                    node,
                    limit,
                ));
            }
        }
    }
    while let Some((spare, std::cmp::Reverse(distance), h, node, limit)) = queue.pop() {
        if best.get(&h).is_some_and(|b| b.0 >= spare) {
            continue;
        }
        best.insert(h, (spare, distance, node, limit));
        for (next, cost) in links(layout, tuning, h) {
            let s = spare - i64::from(cost);
            if best.get(&next).is_none_or(|b| b.0 < s) {
                queue.push((s, std::cmp::Reverse(distance + cost), next, node, limit));
            }
        }
    }
    best.into_iter()
        .map(|(h, (spare, distance, node, limit))| {
            let reach = if spare >= 0 {
                Reach::Served {
                    node,
                    distance,
                    limit,
                }
            } else {
                Reach::TooFar { distance, limit }
            };
            (h, reach)
        })
        .collect()
}

fn find_nets(layout: &Layout, tuning: &Tuning, reach: &HashMap<Pos, Reach>) -> Vec<ChannelNet> {
    let works = |h: &Pos| reach.get(h).is_some_and(|r| r.works());
    let mut nets = Vec::new();
    for slot in 0..SLOTS {
        let mut seen = BTreeSet::new();
        for start in sorted_housings(layout) {
            let Some(channel) = layout.housings[&start].slots[slot] else {
                continue;
            };
            if !works(&start) || !seen.insert(start) {
                continue;
            }
            let mut net = ChannelNet {
                kind: channel.kind,
                slot,
                housings: BTreeSet::from([start]),
                min_tier: channel.tier,
            };
            let mut stack = vec![start];
            while let Some(h) = stack.pop() {
                for (next, _) in links(layout, tuning, h) {
                    let Some(c) = layout.housings[&next].slots[slot] else {
                        continue;
                    };
                    if c.kind == channel.kind && works(&next) && seen.insert(next) {
                        net.housings.insert(next);
                        net.min_tier = net.min_tier.min(c.tier);
                        stack.push(next);
                    }
                }
            }
            nets.push(net);
        }
    }
    nets
}

/// Works out the lines, reaches and channel networks of `layout`.
pub fn analyse(layout: &Layout, tuning: &Tuning) -> Topology {
    let lines = find_lines(layout, tuning);
    let mut reach = find_reach(layout, tuning);
    for h in layout.housings.keys() {
        reach.entry(*h).or_insert(Reach::NoNode);
    }
    let nets = find_nets(layout, tuning, &reach);
    Topology { lines, reach, nets }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(tier: u8) -> Option<Channel> {
        Some(Channel {
            kind: ChannelKind::Item,
            tier,
        })
    }

    fn fluid(tier: u8) -> Option<Channel> {
        Some(Channel {
            kind: ChannelKind::Fluid,
            tier,
        })
    }

    fn housing(slots: [Option<Channel>; SLOTS]) -> Housing {
        Housing { slots }
    }

    /// A row of `n` housings along x starting at `x0`, every one with an item channel in slot 0.
    fn row(layout: &mut Layout, x0: i32, n: i32) {
        for x in x0..x0 + n {
            layout
                .housings
                .insert([x, 0, 0], housing([item(1), None, None, None]));
        }
    }

    #[test]
    fn a_tier_one_node_serves_thirty_two_blocks_of_pipe() {
        // Rule M9/N3, example «труба длиннее предела».
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        row(&mut layout, 1, 33);
        let t = analyse(&layout, &Tuning::default());
        assert_eq!(
            t.reach[&[32, 0, 0]],
            Reach::Served {
                node: [0, 0, 0],
                distance: 32,
                limit: 32
            }
        );
        assert_eq!(
            t.reach[&[33, 0, 0]],
            Reach::TooFar {
                distance: 33,
                limit: 32
            }
        );
        // The housing beyond the limit carries nothing.
        assert!(t.net_of([33, 0, 0], 0).is_none());
        assert_eq!(t.net_of([1, 0, 0], 0).unwrap().housings.len(), 32);
    }

    #[test]
    fn a_second_node_extends_the_line() {
        // Rule N3, example «второй узел продлевает линию».
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        row(&mut layout, 1, 32);
        layout.nodes.insert([33, 0, 0], Node { tier: 1 });
        row(&mut layout, 34, 32);
        let t = analyse(&layout, &Tuning::default());
        assert!(t.reach.values().all(|r| r.works()));
        assert_eq!(t.reach.len(), 64);
        // Through the node the item channel goes on: one net of 64 housings.
        assert_eq!(t.net_of([1, 0, 0], 0).unwrap().housings.len(), 64);
        assert_eq!(t.lines.len(), 1);
    }

    #[test]
    fn a_line_without_a_node_does_not_work() {
        // Rule N3.a.
        let mut layout = Layout::default();
        row(&mut layout, 0, 3);
        let t = analyse(&layout, &Tuning::default());
        assert!(t.reach.values().all(|r| *r == Reach::NoNode));
        assert!(t.nets.is_empty());
    }

    #[test]
    fn a_node_at_the_end_or_beside_a_housing_serves_the_line() {
        // Rule N6.
        let mut layout = Layout::default();
        row(&mut layout, 0, 3);
        layout.nodes.insert([1, 1, 0], Node { tier: 1 });
        let t = analyse(&layout, &Tuning::default());
        assert_eq!(
            t.reach[&[1, 0, 0]],
            Reach::Served {
                node: [1, 1, 0],
                distance: 1,
                limit: 32
            }
        );
        assert!(t.reach.values().all(|r| r.works()));
    }

    #[test]
    fn lines_crossing_a_node_do_not_mix() {
        // Rule N4, example «пересечение без смешивания».
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        for x in [-2, -1, 1, 2] {
            layout
                .housings
                .insert([x, 0, 0], housing([item(1), None, None, None]));
        }
        for y in [-2, -1, 1, 2] {
            layout
                .housings
                .insert([0, y, 0], housing([fluid(1), None, None, None]));
        }
        let t = analyse(&layout, &Tuning::default());
        assert_eq!(t.lines.len(), 2);
        let a = t.net_of([-2, 0, 0], 0).unwrap();
        assert_eq!(a.kind, ChannelKind::Item);
        assert_eq!(a.housings.len(), 4);
        let b = t.net_of([0, -2, 0], 0).unwrap();
        assert_eq!(b.kind, ChannelKind::Fluid);
        assert!(a.housings.is_disjoint(&b.housings));
    }

    #[test]
    fn a_line_does_not_turn_on_a_node() {
        // Rule N5, example «поворот на узле».
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 2 });
        layout
            .housings
            .insert([-1, 0, 0], housing([item(1), None, None, None]));
        layout
            .housings
            .insert([0, 1, 0], housing([item(1), None, None, None]));
        let t = analyse(&layout, &Tuning::default());
        assert_ne!(t.line_of([-1, 0, 0]), t.line_of([0, 1, 0]));
    }

    #[test]
    fn a_hub_joins_all_six_faces() {
        // Rule M12: the last tier's node is a junction.
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 4 });
        layout
            .housings
            .insert([-1, 0, 0], housing([item(1), None, None, None]));
        layout
            .housings
            .insert([0, 1, 0], housing([item(1), None, None, None]));
        layout
            .housings
            .insert([0, 0, 1], housing([item(1), None, None, None]));
        let t = analyse(&layout, &Tuning::default());
        assert_eq!(t.lines.len(), 1);
        assert_eq!(t.net_of([-1, 0, 0], 0).unwrap().housings.len(), 3);
    }

    #[test]
    fn a_channel_goes_on_only_in_the_same_slot_and_kind() {
        // Rule M3.
        let mut layout = Layout::default();
        layout.nodes.insert([0, 1, 0], Node { tier: 1 });
        layout
            .housings
            .insert([0, 0, 0], housing([item(1), fluid(1), None, None]));
        layout
            .housings
            .insert([1, 0, 0], housing([fluid(1), item(1), None, None]));
        layout
            .housings
            .insert([2, 0, 0], housing([fluid(1), item(2), None, None]));
        let t = analyse(&layout, &Tuning::default());
        // Slot 0: item alone in the first housing, fluid in the other two.
        assert_eq!(t.net_of([0, 0, 0], 0).unwrap().housings.len(), 1);
        assert_eq!(t.net_of([1, 0, 0], 0).unwrap().kind, ChannelKind::Fluid);
        assert_eq!(t.net_of([1, 0, 0], 0).unwrap().housings.len(), 2);
        // Slot 1: fluid alone, then items of tiers 1 and 2 join; the weaker tier limits the net.
        let items = t.net_of([1, 0, 0], 1).unwrap();
        assert_eq!(items.housings.len(), 2);
        assert_eq!(items.min_tier, 1);
    }

    #[test]
    fn a_housing_beyond_range_cuts_the_channel() {
        // N3: a housing that does not work takes no part in any network.
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        row(&mut layout, 1, 40);
        layout.nodes.insert([41, 0, 0], Node { tier: 1 });
        row(&mut layout, 42, 2);
        let t = analyse(&layout, &Tuning::default());
        // 33..=40 are 8 blocks past the first node; the second node serves 40 down to 10, so all work.
        assert!(t.reach.values().all(|r| r.works()));
        // Now a 100-long run with nodes only at its ends: the middle does not work and splits the net.
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        row(&mut layout, 1, 100);
        layout.nodes.insert([101, 0, 0], Node { tier: 1 });
        let t = analyse(&layout, &Tuning::default());
        assert!(!t.reach[&[50, 0, 0]].works());
        assert_ne!(
            t.net_of([1, 0, 0], 0).map(|n| n.housings.clone()),
            t.net_of([100, 0, 0], 0).map(|n| n.housings.clone())
        );
        assert_eq!(t.net_of([1, 0, 0], 0).unwrap().housings.len(), 32);
    }
}
