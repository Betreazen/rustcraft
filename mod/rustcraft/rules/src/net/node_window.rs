//! The node's window (guide `node`, ui.js `node`) and the line-boost module (guide `mod_line`).
//!
//! The window shows one line through the node: tiers 1–3 join opposite faces, so it has three
//! lines, one an axis (west–east, north–south, up–down); a hub (tier 4) has one line on all six
//! faces. For the line it shows how many housings it has, the farthest housing this node serves
//! and its range, the item throughput of the section with the node's modules, the energy balance
//! of the circuits on it, and the node's module sockets.
//!
//! A line-boost module in a node raises what every channel carries on the housings that node
//! serves by half (`boost`); a net running through sections of several nodes is held to its
//! weakest section, as a net is held to its weakest channel (M10).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geom::{Face, Pos, step};
use crate::net::topology::{ChannelKind, Layout, Reach, Topology};
use crate::tuning::Tuning;

/// The mod message the window goes in, server to the player who opened it.
pub const CHANNEL: &str = "rustcraft:node";
/// The mod message of a press in the window, client to server.
pub const PRESS: &str = "rustcraft:node_press";
/// The node's module sockets (ui.js: three).
pub const SOCKETS: u8 = 3;
/// The line-boost module's item. Stream 4 of slice 18 declares it (`Module::Line`); the node only
/// names it.
pub const LINE_MODULE: &str = "rustcraft:module_line";
/// What one line-boost module adds to a channel's throughput (guide `mod_line`: «+50% за модуль»).
pub const BOOST_PER_MODULE: f64 = 0.5;

/// The throughput factor of a node with `modules` line-boost modules.
pub fn boost(modules: u8) -> f64 {
    1.0 + BOOST_PER_MODULE * f64::from(modules.min(SOCKETS))
}

/// Wave 2: what a broken node with `modules` line-boost modules gives back of them: one item a
/// module, no more than its sockets hold.
pub fn returned_modules(modules: u8) -> Vec<&'static str> {
    vec![LINE_MODULE; usize::from(modules.min(SOCKETS))]
}

/// The axes of a node of tiers 1–3, as the window names them: west–east, north–south, up–down.
pub const AXES: [[Face; 2]; 3] = [
    [Face::XNeg, Face::XPos],
    [Face::ZNeg, Face::ZPos],
    [Face::YNeg, Face::YPos],
];

/// The faces of the node the chosen line leaves by: an axis's two, or all six on a hub.
pub fn faces(hub: bool, axis: usize) -> Vec<Face> {
    if hub {
        Face::ALL.to_vec()
    } else {
        AXES[axis.min(2)].to_vec()
    }
}

/// What the window shows. Numbers only: the client writes the words in its own language.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeView {
    pub pos: Pos,
    pub tier: u8,
    pub hub: bool,
    /// The line shown (an index into `AXES`; 0 on a hub).
    pub axis: usize,
    /// Housings on the line.
    pub housings: usize,
    /// The farthest housing of the line this node serves, blocks of pipe, and its range.
    pub farthest: u32,
    pub limit: u32,
    /// Items a second the line's item channels carry on this node's section without and with its
    /// modules (none without an item channel).
    pub items: Option<(f64, f64)>,
    /// The energy balance of the circuits on the line: made, used, stored and what can be stored,
    /// E and E a second.
    pub made: f64,
    pub used: f64,
    pub stored: f64,
    pub capacity: f64,
    /// Line-boost modules in its sockets.
    pub modules: u8,
    /// Whether the window is to open (the first message) or only refresh.
    pub open: bool,
}

impl NodeView {
    /// The status line: 0 works, 1 works worse (energy short), 2 stands (no housing on the line).
    pub fn severity(&self) -> u8 {
        if self.housings == 0 {
            2
        } else if self.used > self.made + 1e-9 && self.stored <= 0.0 {
            1
        } else {
            0
        }
    }

    /// How much slower the machines on the line work, percent (0 when the energy is enough).
    pub fn slowed_percent(&self) -> u32 {
        if self.used <= self.made || self.used <= 0.0 || self.stored > 0.0 {
            return 0;
        }
        ((1.0 - self.made / self.used) * 100.0).round() as u32
    }
}

/// A press in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Press {
    /// Show the line of `AXES[i]`.
    Axis(usize),
    /// A click on socket `i`: a module from the inventory into an empty one, or back out of a full one.
    Socket(u8),
}

#[derive(Serialize, Deserialize)]
struct Wire {
    pos: Pos,
    press: Press,
}

pub fn encode(view: &NodeView) -> Vec<u8> {
    serde_json::to_vec(view).unwrap_or_default()
}

pub fn decode(bytes: &[u8]) -> Option<NodeView> {
    serde_json::from_slice(bytes).ok()
}

pub fn encode_press(pos: Pos, press: Press) -> Vec<u8> {
    serde_json::to_vec(&Wire { pos, press }).unwrap_or_default()
}

pub fn decode_press(bytes: &[u8]) -> Option<(Pos, Press)> {
    serde_json::from_slice::<Wire>(bytes).ok().map(|w| (w.pos, w.press))
}

/// The housings of the line through `node` that leaves by `faces`: every line of the topology
/// that has a housing touching one of them.
pub fn line_housings(topology: &Topology, layout: &Layout, node: Pos, faces: &[Face]) -> BTreeSet<Pos> {
    let mut out = BTreeSet::new();
    for &f in faces {
        let h = step(node, f);
        if !layout.housings.contains_key(&h) {
            continue;
        }
        if let Some(i) = topology.line_of(h) {
            out.extend(topology.lines[i].iter().copied());
        }
    }
    out
}

/// The window of the node at `node` showing line `axis`, with `modules` in its sockets; the
/// energy numbers are the bundle's to fill.
pub fn view(
    topology: &Topology,
    layout: &Layout,
    tuning: &Tuning,
    node: Pos,
    axis: usize,
    modules: u8,
) -> NodeView {
    let tier = layout.nodes.get(&node).map_or(1, |n| n.tier);
    let hub = tier >= tuning.hub_tier;
    let axis = if hub { 0 } else { axis.min(2) };
    let housings = line_housings(topology, layout, node, &faces(hub, axis));
    let farthest = housings
        .iter()
        .filter_map(|h| match topology.reach.get(h) {
            Some(Reach::Served { node: n, distance, .. }) if *n == node => Some(*distance),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let item_tier = topology
        .nets
        .iter()
        .filter(|n| n.kind == ChannelKind::Item && !n.housings.is_disjoint(&housings))
        .map(|n| n.min_tier)
        .min();
    NodeView {
        pos: node,
        tier,
        hub,
        axis,
        housings: housings.len(),
        farthest,
        limit: tuning.range_of(tier),
        items: item_tier.map(|t| {
            let base = tuning.items_of(t);
            (base, base * boost(modules))
        }),
        modules,
        ..NodeView::default()
    }
}

/// The throughput factor of a channel net over `housings`: the weakest boost among the nodes that
/// serve them (a housing no node serves takes no part in a net). `modules` are the nodes' module
/// counts.
pub fn net_boost(
    housings: &BTreeSet<Pos>,
    reach: &std::collections::HashMap<Pos, Reach>,
    modules: &BTreeMap<Pos, u8>,
) -> f64 {
    housings
        .iter()
        .filter_map(|h| match reach.get(h) {
            Some(Reach::Served { node, .. }) => Some(boost(modules.get(node).copied().unwrap_or(0))),
            _ => None,
        })
        .fold(None, |acc: Option<f64>, b| Some(acc.map_or(b, |a| a.min(b))))
        .unwrap_or(1.0)
}

// ---- Layout, in GUI pixels from the window's top left (docs/design/04-window-style.md) ----

/// A rectangle: x, y, width, height.
pub type Rect = [f32; 4];

pub const WIDTH: f32 = 214.0;
pub const HEIGHT: f32 = 176.0;
const LEFT: f32 = 8.0;
/// The «Линия» label and the three axis buttons under it.
pub const LINE_LABEL_Y: f32 = 18.0;
pub const AXIS_Y: f32 = 28.0;
const AXIS_H: f32 = 14.0;
const AXIS_GAP: f32 = 2.0;
/// The balance's four rows, then the line's three.
pub const BALANCE_Y: f32 = 47.0;
pub const LINE_ROWS_Y: f32 = 90.0;
/// Wave 2: the balance's rows (ui.js `balance`: Производство, Потребление, В запасе, Хватает).
pub const BALANCE_ROWS: usize = 4;
pub const ROW: f32 = 10.0;
/// The «Модули» label and the sockets' items.
pub const MODULES_LABEL_Y: f32 = 123.0;
pub const SOCKETS_Y: f32 = 134.0;
pub const STATUS: Rect = [LEFT, 158.0, WIDTH - 2.0 * LEFT, 12.0];

/// The axis buttons, west–east, north–south, up–down.
pub fn axis_rects() -> [Rect; 3] {
    let w = (WIDTH - 2.0 * LEFT - 2.0 * AXIS_GAP) / 3.0;
    std::array::from_fn(|i| [LEFT + (w + AXIS_GAP) * i as f32, AXIS_Y, w, AXIS_H])
}

/// Where each socket's item goes (its well is a pixel round it).
pub fn socket_items() -> [[f32; 2]; 3] {
    std::array::from_fn(|i| [LEFT + 1.0 + 20.0 * i as f32, SOCKETS_Y])
}

fn inside(r: Rect, at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

/// What a click at `at` (window pixels) presses; a hub has no axes to choose.
pub fn press_at(at: [f32; 2], hub: bool) -> Option<Press> {
    if !hub && let Some(i) = axis_rects().iter().position(|r| inside(*r, at)) {
        return Some(Press::Axis(i));
    }
    socket_items()
        .iter()
        .position(|p| inside([p[0] - 1.0, p[1] - 1.0, 18.0, 18.0], at))
        .map(|i| Press::Socket(i as u8))
}

/// The window's top left on a screen `size` GUI pixels big: in the middle.
pub fn origin(size: [f32; 2]) -> [f32; 2] {
    [((size[0] - WIDTH) / 2.0).floor().max(0.0), ((size[1] - HEIGHT) / 2.0).floor().max(0.0)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::topology::{Channel, Housing, Node, analyse};

    fn item() -> Option<Channel> {
        Some(Channel { kind: ChannelKind::Item, tier: 1 })
    }

    /// A node at the origin, `west` housings to its west and `east` to its east, and `up` above.
    fn cross(tier: u8, west: i32, east: i32, up: i32) -> Layout {
        let mut l = Layout::default();
        l.nodes.insert([0, 0, 0], Node { tier });
        let h = Housing { slots: [item(), None, None, None] };
        for x in 1..=west {
            l.housings.insert([-x, 0, 0], h);
        }
        for x in 1..=east {
            l.housings.insert([x, 0, 0], h);
        }
        for y in 1..=up {
            l.housings.insert([0, y, 0], h);
        }
        l
    }

    #[test]
    fn a_tier_one_node_shows_one_axis_line() {
        // ui.js: «Корпусов 41 · Самый дальний 27 из 32 блоков».
        let t = Tuning::default();
        let l = cross(1, 5, 10, 3);
        let topo = analyse(&l, &t);
        let v = view(&topo, &l, &t, [0, 0, 0], 0, 0);
        assert_eq!(v.housings, 15, "west and east are one line through a tier-1 node");
        assert_eq!(v.farthest, 10);
        assert_eq!(v.limit, 32);
        assert!(!v.hub);
        let up = view(&topo, &l, &t, [0, 0, 0], 2, 0);
        assert_eq!(up.housings, 3);
        assert_eq!(up.farthest, 3);
    }

    #[test]
    fn a_hub_shows_one_line_on_all_six_faces() {
        let t = Tuning::default();
        let l = cross(4, 5, 10, 3);
        let topo = analyse(&l, &t);
        let v = view(&topo, &l, &t, [0, 0, 0], 2, 0);
        assert!(v.hub);
        assert_eq!(v.axis, 0);
        assert_eq!(v.housings, 18);
        assert_eq!(v.limit, 256);
    }

    #[test]
    fn modules_raise_the_sections_throughput_by_half_each() {
        // ui.js: «Предметный канал на участке 4 → 6 предметов/с» with one module.
        let t = Tuning::default();
        let l = cross(1, 2, 2, 0);
        let topo = analyse(&l, &t);
        let v = view(&topo, &l, &t, [0, 0, 0], 0, 1);
        assert_eq!(v.items, Some((4.0, 6.0)));
        assert_eq!(boost(0), 1.0);
        assert_eq!(boost(3), 2.5);
        assert_eq!(boost(9), 2.5, "no more than three sockets");
    }

    #[test]
    fn a_broken_node_gives_back_one_item_a_module() {
        assert!(returned_modules(0).is_empty());
        assert_eq!(returned_modules(2), [LINE_MODULE, LINE_MODULE]);
        assert_eq!(returned_modules(9).len(), 3, "no more than its sockets");
    }

    #[test]
    fn a_net_takes_its_weakest_sections_boost() {
        let t = Tuning::default();
        // Two nodes, a line between: housings at x 1..=4, nodes at x 0 and x 5.
        let mut l = cross(1, 0, 4, 0);
        l.nodes.insert([5, 0, 0], Node { tier: 1 });
        let topo = analyse(&l, &t);
        let net = &topo.nets[0].housings;
        let one = BTreeMap::from([([0, 0, 0], 2u8), ([5, 0, 0], 0u8)]);
        assert_eq!(net_boost(net, &topo.reach, &one), 1.0);
        let both = BTreeMap::from([([0, 0, 0], 2u8), ([5, 0, 0], 1u8)]);
        assert_eq!(net_boost(net, &topo.reach, &both), 1.5);
        assert_eq!(net_boost(&BTreeSet::new(), &topo.reach, &both), 1.0);
    }

    #[test]
    fn the_window_says_how_much_slower_the_machines_work() {
        // ui.js: «нет, машины замедлены на 17%» for 20 made and 24 used.
        let v = NodeView { housings: 1, made: 20.0, used: 24.0, ..NodeView::default() };
        assert_eq!(v.severity(), 1);
        assert_eq!(v.slowed_percent(), 17);
        let ok = NodeView { housings: 1, made: 30.0, used: 24.0, ..NodeView::default() };
        assert_eq!((ok.severity(), ok.slowed_percent()), (0, 0));
        assert_eq!(NodeView::default().severity(), 2);
    }

    #[test]
    fn a_click_presses_an_axis_or_a_socket() {
        let axes = axis_rects();
        let mid = |r: Rect| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
        assert_eq!(press_at(mid(axes[1]), false), Some(Press::Axis(1)));
        assert_eq!(press_at(mid(axes[1]), true), None, "a hub has one line");
        let s = socket_items()[2];
        assert_eq!(press_at([s[0] + 8.0, s[1] + 8.0], false), Some(Press::Socket(2)));
        assert_eq!(press_at([1.0, 1.0], false), None);
        assert!(STATUS[1] + STATUS[3] <= HEIGHT);
        assert!(BALANCE_Y + ROW * BALANCE_ROWS as f32 <= LINE_ROWS_Y, "the four balance rows fit");
    }

    #[test]
    fn a_view_and_a_press_go_over_the_wire() {
        let v = NodeView { pos: [1, 2, 3], housings: 4, open: true, ..NodeView::default() };
        assert_eq!(decode(&encode(&v)), Some(v));
        assert_eq!(decode_press(&encode_press([1, 2, 3], Press::Socket(2))), Some(([1, 2, 3], Press::Socket(2))));
    }
}
