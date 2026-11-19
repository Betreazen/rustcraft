//! Lines of the magistral and the channel networks inside them.
//!
//! A line is a group of housings joined face to face (N5: housings join in every direction) or
//! through a node. A node of tiers 1–3 joins only opposite faces, each axis a separate line (N4);
//! a node of the hub tier joins all six faces (M12). A housing works if the length of pipe from it
//! to some node is within that node's range (N3); otherwise it shows a diagnosis and takes no part
//! in any channel network.
//!
//! Round 31: every housing carries its channels built in (items, fluid, energy, each in a fixed
//! slot), and has a tier of its own, 1 to 4. The working housings joined to each other carry one
//! net of each kind together; the weakest housing on a net sets what it carries (M10). 0.21 (spec
//! 44, XC1): a fourth channel, experience, in the fourth slot.

use std::collections::{BTreeSet, BinaryHeap, HashMap};

use crate::geom::{Face, Pos, step};
use crate::tuning::Tuning;

/// What a channel carries. The order is the order of a housing's slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChannelKind {
    Item,
    Fluid,
    Energy,
    /// 0.21 (spec 44, XC1; owner: «четвертым каналом … зеленый канал. Ну, типа опыт»):
    /// experience points between the blocks that make it, the condensers and the enchanting
    /// machine (`net::xp`).
    Xp,
}

impl ChannelKind {
    /// Every kind, in the order of a housing's slots.
    pub const ALL: [ChannelKind; SLOTS] =
        [ChannelKind::Item, ChannelKind::Fluid, ChannelKind::Energy, ChannelKind::Xp];

    /// The slot of a housing this kind is built into.
    pub const fn slot(self) -> usize {
        match self {
            ChannelKind::Item => ITEM_SLOT,
            ChannelKind::Fluid => FLUID_SLOT,
            ChannelKind::Energy => ENERGY_SLOT,
            ChannelKind::Xp => XP_SLOT,
        }
    }

    /// Whether a joint of this kind has a filter and a priority: items and fluids do, energy and
    /// experience only a direction.
    pub const fn filtered(self) -> bool {
        matches!(self, ChannelKind::Item | ChannelKind::Fluid)
    }

    /// The kind built into `slot`; none past the last slot.
    pub fn of_slot(slot: usize) -> Option<ChannelKind> {
        Self::ALL.get(slot).copied()
    }
}

/// A channel of a housing: its kind, and the tier it carries at (the housing's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    pub kind: ChannelKind,
    pub tier: u8,
}

/// Number of channels in a housing: one of each kind, built in (round 31: three; M1 had four free
/// slots; 0.21, XC1: the fourth is experience).
pub const SLOTS: usize = 4;
/// The slot of each kind: the joints' settings are kept per face and per slot (M7).
pub const ITEM_SLOT: usize = 0;
pub const FLUID_SLOT: usize = 1;
pub const ENERGY_SLOT: usize = 2;
/// 0.21 (XC1): a housing saved with three slots has no settings here, so its experience joints
/// work in their default.
pub const XP_SLOT: usize = 3;
/// Housing tiers run from 1 to this, as the machines' (`tiers::MAX_TIER`).
pub const MAX_TIER: u8 = crate::tiers::MAX_TIER;

/// A housing of the magistral: its channels are built in, and its tier sets how much they
/// carry (round 31).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Housing {
    pub tier: u8,
}

impl Default for Housing {
    fn default() -> Self {
        Housing { tier: 1 }
    }
}

impl Housing {
    /// A housing of `tier`, brought into 1–4.
    pub const fn new(tier: u8) -> Housing {
        let tier = if tier < 1 {
            1
        } else if tier > MAX_TIER {
            MAX_TIER
        } else {
            tier
        };
        Housing { tier }
    }

    /// The channel in `slot`; none past the last slot.
    pub fn channel(self, slot: usize) -> Option<Channel> {
        ChannelKind::of_slot(slot).map(|kind| Channel {
            kind,
            tier: self.tier,
        })
    }

    /// Its channels, in slot order.
    pub fn channels(self) -> [Channel; SLOTS] {
        ChannelKind::ALL.map(|kind| Channel {
            kind,
            tier: self.tier,
        })
    }
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

/// The working housings that carry one channel together: joined working housings, one net of each
/// kind (round 31: every housing has every channel, each kind in its own slot).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelNet {
    pub kind: ChannelKind,
    /// `kind.slot()`: the slot the joints' settings of this net are kept under.
    pub slot: usize,
    pub housings: BTreeSet<Pos>,
    /// M10, round 31: the tier of the weakest housing on the net limits what it carries.
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

    /// The channel net that carries `slot` of `housing`, if the housing works.
    pub fn net_of(&self, housing: Pos, slot: usize) -> Option<&ChannelNet> {
        self.nets
            .iter()
            .find(|n| n.slot == slot && n.housings.contains(&housing))
    }

    /// The net of `kind` that `housing` is on, if the housing works.
    pub fn net_of_kind(&self, housing: Pos, kind: ChannelKind) -> Option<&ChannelNet> {
        self.net_of(housing, kind.slot())
    }
}

/// The steps from a housing to the housings joined to it along the pipe, with the blocks of pipe
/// each covers: 1 to a touching housing, 2 through a node (the node is a block of pipe too).
pub(crate) fn links(layout: &Layout, tuning: &Tuning, from: Pos) -> Vec<(Pos, u32)> {
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
    // Every working housing has every channel: the working housings joined to each other carry
    // one net of each kind together, held to the weakest housing among them (M10).
    let mut groups: Vec<(BTreeSet<Pos>, u8)> = Vec::new();
    let mut seen = BTreeSet::new();
    for start in sorted_housings(layout) {
        if !works(&start) || !seen.insert(start) {
            continue;
        }
        let mut housings = BTreeSet::from([start]);
        let mut min_tier = layout.housings[&start].tier;
        let mut stack = vec![start];
        while let Some(h) = stack.pop() {
            for (next, _) in links(layout, tuning, h) {
                if works(&next) && seen.insert(next) {
                    housings.insert(next);
                    min_tier = min_tier.min(layout.housings[&next].tier);
                    stack.push(next);
                }
            }
        }
        groups.push((housings, min_tier));
    }
    ChannelKind::ALL
        .into_iter()
        .flat_map(|kind| {
            groups.iter().map(move |(housings, min_tier)| ChannelNet {
                kind,
                slot: kind.slot(),
                housings: housings.clone(),
                min_tier: *min_tier,
            })
        })
        .collect()
}

/// The blocks of pipe from the nearest of `from` to the nearest of `to`, going only through
/// `housings` (one channel net's): 0 for a housing in both, 1 for a touching housing further, 2
/// more through a node. None when no way through the net joins them.
pub fn pipe_length(
    layout: &Layout,
    tuning: &Tuning,
    housings: &BTreeSet<Pos>,
    from: &[Pos],
    to: &[Pos],
) -> Option<u32> {
    let targets: BTreeSet<Pos> = to
        .iter()
        .copied()
        .filter(|p| housings.contains(p))
        .collect();
    if targets.is_empty() {
        return None;
    }
    let mut best: HashMap<Pos, u32> = HashMap::new();
    let mut queue = BinaryHeap::new();
    for &h in from.iter().filter(|p| housings.contains(*p)) {
        queue.push((std::cmp::Reverse(0u32), h));
    }
    while let Some((std::cmp::Reverse(d), h)) = queue.pop() {
        if best.get(&h).is_some_and(|&b| b <= d) {
            continue;
        }
        best.insert(h, d);
        if targets.contains(&h) {
            return Some(d);
        }
        for (next, cost) in links(layout, tuning, h) {
            if housings.contains(&next) && best.get(&next).is_none_or(|&b| b > d + cost) {
                queue.push((std::cmp::Reverse(d + cost), next));
            }
        }
    }
    None
}

/// Slice 18: the way `pipe_length` measures, housing by housing: each housing from the nearest
/// of `from` to the nearest of `to`, with the blocks of pipe from the first to it (1 more for a
/// touching housing, 2 through a node, as `links` counts). None when no way joins them.
pub fn pipe_path(
    layout: &Layout,
    tuning: &Tuning,
    housings: &BTreeSet<Pos>,
    from: &[Pos],
    to: &[Pos],
) -> Option<Vec<(Pos, u32)>> {
    let targets: BTreeSet<Pos> = to
        .iter()
        .copied()
        .filter(|p| housings.contains(p))
        .collect();
    if targets.is_empty() {
        return None;
    }
    let mut best: HashMap<Pos, (u32, Option<Pos>)> = HashMap::new();
    let mut queue = BinaryHeap::new();
    for &h in from.iter().filter(|p| housings.contains(*p)) {
        queue.push((std::cmp::Reverse(0u32), h, None));
    }
    while let Some((std::cmp::Reverse(d), h, came)) = queue.pop() {
        if best.get(&h).is_some_and(|b| b.0 <= d) {
            continue;
        }
        best.insert(h, (d, came));
        if targets.contains(&h) {
            let mut path = vec![(h, d)];
            let mut at = came;
            while let Some(p) = at {
                let (dp, before) = best[&p];
                path.push((p, dp));
                at = before;
            }
            path.reverse();
            return Some(path);
        }
        for (next, cost) in links(layout, tuning, h) {
            if housings.contains(&next) && best.get(&next).is_none_or(|b| b.0 > d + cost) {
                queue.push((std::cmp::Reverse(d + cost), next, Some(h)));
            }
        }
    }
    None
}

/// Slice 18: the face of the housing at `from` the pipe leaves by to the housing at `to` next on
/// its way (touching it, or behind a node it touches); none when they are not so joined.
pub fn link_face(layout: &Layout, from: Pos, to: Pos) -> Option<Face> {
    Face::ALL.into_iter().find(|&f| {
        let next = step(from, f);
        next == to
            || (layout.nodes.contains_key(&next) && Face::ALL.into_iter().any(|g| step(next, g) == to))
    })
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

    fn housing(tier: u8) -> Housing {
        Housing::new(tier)
    }

    /// A row of `n` housings of tier 1 along x starting at `x0`.
    fn row(layout: &mut Layout, x0: i32, n: i32) {
        for x in x0..x0 + n {
            layout.housings.insert([x, 0, 0], housing(1));
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
            layout.housings.insert([x, 0, 0], housing(1));
        }
        for y in [-2, -1, 1, 2] {
            layout.housings.insert([0, y, 0], housing(1));
        }
        let t = analyse(&layout, &Tuning::default());
        assert_eq!(t.lines.len(), 2);
        let a = t.net_of_kind([-2, 0, 0], ChannelKind::Item).unwrap();
        assert_eq!(a.kind, ChannelKind::Item);
        assert_eq!(a.housings.len(), 4);
        let b = t.net_of_kind([0, -2, 0], ChannelKind::Fluid).unwrap();
        assert_eq!(b.kind, ChannelKind::Fluid);
        assert!(a.housings.is_disjoint(&b.housings));
        // Line B's items do not reach line A's receivers either.
        let b_items = t.net_of_kind([0, -2, 0], ChannelKind::Item).unwrap();
        assert!(a.housings.is_disjoint(&b_items.housings));
    }

    #[test]
    fn a_line_does_not_turn_on_a_node() {
        // Rule N5, example «поворот на узле».
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 2 });
        layout.housings.insert([-1, 0, 0], housing(1));
        layout.housings.insert([0, 1, 0], housing(1));
        let t = analyse(&layout, &Tuning::default());
        assert_ne!(t.line_of([-1, 0, 0]), t.line_of([0, 1, 0]));
    }

    #[test]
    fn a_hub_joins_all_six_faces() {
        // Rule M12: the last tier's node is a junction.
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 4 });
        layout.housings.insert([-1, 0, 0], housing(1));
        layout.housings.insert([0, 1, 0], housing(1));
        layout.housings.insert([0, 0, 1], housing(1));
        let t = analyse(&layout, &Tuning::default());
        assert_eq!(t.lines.len(), 1);
        assert_eq!(t.net_of([-1, 0, 0], 0).unwrap().housings.len(), 3);
    }

    #[test]
    fn every_housing_carries_every_channel_at_the_weakest_tier_of_its_net() {
        // Round 31: housings of tiers 3, 1 and 2 in a row carry one net of each kind, in the
        // kind's own slot; the tier-1 housing holds them all back (M10).
        let mut layout = Layout::default();
        layout.nodes.insert([0, 1, 0], Node { tier: 1 });
        for (x, tier) in [(0, 3), (1, 1), (2, 2)] {
            layout.housings.insert([x, 0, 0], housing(tier));
        }
        let t = analyse(&layout, &Tuning::default());
        assert_eq!(t.nets.len(), SLOTS);
        for kind in ChannelKind::ALL {
            let net = t.net_of_kind([2, 0, 0], kind).unwrap();
            assert_eq!((net.kind, net.slot), (kind, kind.slot()));
            assert_eq!(net.housings.len(), 3);
            assert_eq!(net.min_tier, 1);
        }
        assert_eq!(
            [ITEM_SLOT, FLUID_SLOT, ENERGY_SLOT, XP_SLOT].map(ChannelKind::of_slot),
            ChannelKind::ALL.map(Some)
        );
        assert_eq!(ChannelKind::of_slot(SLOTS), None);
        // Without the weak housing, the two ends are apart: two nets of each kind.
        layout.housings.remove(&[1, 0, 0]);
        layout.nodes.insert([2, 1, 0], Node { tier: 1 });
        let t = analyse(&layout, &Tuning::default());
        assert_eq!(t.net_of_kind([0, 0, 0], ChannelKind::Energy).unwrap().min_tier, 3);
        assert_eq!(t.net_of_kind([2, 0, 0], ChannelKind::Energy).unwrap().min_tier, 2);
    }

    #[test]
    fn every_housing_carries_an_experience_channel_in_its_fourth_slot() {
        // 0.21 (XC1): four slots; the experience net is the item net's housings, held to the same
        // weakest tier, and only experience and energy go without a filter.
        assert_eq!(SLOTS, 4);
        assert_eq!(ChannelKind::Xp.slot(), XP_SLOT);
        assert_eq!(ChannelKind::of_slot(XP_SLOT), Some(ChannelKind::Xp));
        assert_eq!(
            ChannelKind::ALL.map(ChannelKind::filtered),
            [true, true, false, false]
        );
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 2 });
        for (x, tier) in [(1, 3), (2, 2), (3, 4)] {
            layout.housings.insert([x, 0, 0], housing(tier));
        }
        let t = analyse(&layout, &Tuning::default());
        let xp = t.net_of([3, 0, 0], XP_SLOT).unwrap();
        let items = t.net_of_kind([3, 0, 0], ChannelKind::Item).unwrap();
        assert_eq!((xp.kind, xp.min_tier), (ChannelKind::Xp, 2));
        assert_eq!(xp.housings, items.housings);
        assert_eq!(
            Housing::new(2).channel(XP_SLOT),
            Some(Channel { kind: ChannelKind::Xp, tier: 2 })
        );
        assert_eq!(Housing::new(2).channels().len(), 4);
    }

    #[test]
    fn a_housing_tier_stays_within_one_to_four() {
        assert_eq!(Housing::new(0).tier, 1);
        assert_eq!(Housing::new(9).tier, MAX_TIER);
        assert_eq!(Housing::default(), Housing::new(1));
        let h = Housing::new(3);
        assert_eq!(h.channel(FLUID_SLOT), Some(Channel { kind: ChannelKind::Fluid, tier: 3 }));
        assert_eq!(h.channel(SLOTS), None);
        assert!(h.channels().iter().all(|c| c.tier == 3));
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

    #[test]
    fn the_pipe_between_two_housings_of_a_net_is_counted_through_its_node() {
        // Slice 17: an order's items ride this many blocks of pipe.
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 3 });
        row(&mut layout, 1, 10);
        layout.nodes.insert([11, 0, 0], Node { tier: 3 });
        row(&mut layout, 12, 3);
        let tuning = Tuning::default();
        let t = analyse(&layout, &tuning);
        let net = t.net_of([1, 0, 0], 0).unwrap();
        let length =
            |from: &[Pos], to: &[Pos]| pipe_length(&layout, &tuning, &net.housings, from, to);
        assert_eq!(length(&[[3, 0, 0]], &[[3, 0, 0]]), Some(0));
        assert_eq!(length(&[[3, 0, 0]], &[[9, 0, 0]]), Some(6));
        // The node between 10 and 12 is a block of pipe too.
        assert_eq!(length(&[[1, 0, 0]], &[[14, 0, 0]]), Some(9 + 2 + 2));
        // The nearest of several ends.
        assert_eq!(
            length(&[[1, 0, 0], [8, 0, 0]], &[[2, 0, 0], [10, 0, 0]]),
            Some(1)
        );
        // A housing off the net is no end.
        assert_eq!(length(&[[1, 0, 0]], &[[40, 0, 0]]), None);
        let half: BTreeSet<Pos> = net.housings.iter().copied().filter(|p| p[0] <= 5).collect();
        assert_eq!(
            pipe_length(&layout, &tuning, &half, &[[1, 0, 0]], &[[9, 0, 0]]),
            None
        );
    }

    #[test]
    fn the_path_between_two_housings_lists_each_housing_with_its_pipe_so_far() {
        // Slice 18 (guide `orders`: the order's items seen riding the housings).
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 3 });
        row(&mut layout, 1, 10);
        layout.nodes.insert([11, 0, 0], Node { tier: 3 });
        row(&mut layout, 12, 3);
        // A bend: two housings up from housing 4.
        for y in 1..=2 {
            layout.housings.insert([4, y, 0], housing(1));
        }
        let tuning = Tuning::default();
        let t = analyse(&layout, &tuning);
        let net = t.net_of([1, 0, 0], 0).unwrap();
        let path = |from: &[Pos], to: &[Pos]| pipe_path(&layout, &tuning, &net.housings, from, to);
        assert_eq!(path(&[[3, 0, 0]], &[[3, 0, 0]]), Some(vec![([3, 0, 0], 0)]));
        assert_eq!(
            path(&[[2, 0, 0]], &[[4, 2, 0]]),
            Some(vec![
                ([2, 0, 0], 0),
                ([3, 0, 0], 1),
                ([4, 0, 0], 2),
                ([4, 1, 0], 3),
                ([4, 2, 0], 4)
            ])
        );
        // Through the node: 2 blocks of pipe, as pipe_length counts.
        let through = path(&[[9, 0, 0]], &[[13, 0, 0]]).unwrap();
        assert_eq!(
            through,
            vec![([9, 0, 0], 0), ([10, 0, 0], 1), ([12, 0, 0], 3), ([13, 0, 0], 4)]
        );
        assert_eq!(
            through.last().map(|p| p.1),
            pipe_length(&layout, &tuning, &net.housings, &[[9, 0, 0]], &[[13, 0, 0]])
        );
        assert_eq!(path(&[[1, 0, 0]], &[[40, 0, 0]]), None);
        // The face each step leaves by, a node's too.
        assert_eq!(link_face(&layout, [3, 0, 0], [4, 0, 0]), Some(Face::XPos));
        assert_eq!(link_face(&layout, [4, 0, 0], [4, 1, 0]), Some(Face::YPos));
        assert_eq!(link_face(&layout, [10, 0, 0], [12, 0, 0]), Some(Face::XPos));
        assert_eq!(link_face(&layout, [12, 0, 0], [10, 0, 0]), Some(Face::XNeg));
        assert_eq!(link_face(&layout, [1, 0, 0], [3, 0, 0]), None);
    }
}
