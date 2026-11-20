//! Slice 14: the storage tablet (S7, guide `tablet`). It opens, without wires, the storage of the
//! magistral the player stands within a node's range of: the storage is a joint of an item net
//! (one of its cells or terminals touches a housing of the net), the nodes that serve that net's
//! housings are the net's nodes, and the player must be within a node's range (its pipe length,
//! counted here as a straight line from the node) of one of them. Between bases it does not
//! reach, as the node's range does not.
//!
//! The window is a chest of six rows: five rows of stored items, most numerous first, then a row
//! with the page buttons. A click on an item takes one stack of it into the inventory.

use std::collections::BTreeSet;

use crate::geom::{Face, Pos, step};
use crate::net::items::ItemKey;
use crate::net::topology::{ChannelKind, Reach, Topology};

/// The window's rows and the items a page shows (five rows of nine).
pub const ROWS: u8 = 6;
pub const PER_PAGE: usize = 45;
/// The bottom row: the previous page, the page's name, the next page.
pub const PREV_SLOT: usize = 45;
pub const INFO_SLOT: usize = 49;
pub const NEXT_SLOT: usize = 53;

/// A storage the tablet may open: its id and the blocks it is joined by (cells and terminals).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub id: u32,
    pub blocks: Vec<Pos>,
}

fn node_distance(player: [f64; 3], node: Pos) -> f64 {
    let c = node.map(|v| f64::from(v) + 0.5);
    ((c[0] - player[0]).powi(2) + (c[1] - player[1]).powi(2) + (c[2] - player[2]).powi(2)).sqrt()
}

/// The nodes that serve the item nets the blocks `blocks` touch, with their ranges.
fn nodes_of(blocks: &[Pos], topology: &Topology) -> BTreeSet<(Pos, u32)> {
    let touched: BTreeSet<Pos> = blocks
        .iter()
        .flat_map(|b| Face::ALL.map(|f| step(*b, f)))
        .collect();
    topology
        .nets
        .iter()
        .filter(|n| n.kind == ChannelKind::Item && n.housings.iter().any(|h| touched.contains(h)))
        .flat_map(|n| n.housings.iter())
        .filter_map(|h| match topology.reach.get(h) {
            Some(Reach::Served { node, limit, .. }) => Some((*node, *limit)),
            _ => None,
        })
        .collect()
}

/// The storage the tablet opens for a player at `player`: of the storages whose magistral has a
/// node within its range of the player, the one with the nearest such node. None when none.
pub fn pick(player: [f64; 3], storages: &[Candidate], topology: &Topology) -> Option<u32> {
    storages
        .iter()
        .filter_map(|s| {
            nodes_of(&s.blocks, topology)
                .into_iter()
                .map(|(node, limit)| (node_distance(player, node), f64::from(limit)))
                .filter(|(d, limit)| d <= limit)
                .map(|(d, _)| d)
                .min_by(f64::total_cmp)
                .map(|d| (d, s.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, id)| id)
}

/// The items the window lists: those stored, most numerous first, then by key.
pub fn entries(items: &std::collections::BTreeMap<ItemKey, u64>) -> Vec<(ItemKey, u64)> {
    let mut out: Vec<(ItemKey, u64)> = items
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(k, n)| (k.clone(), *n))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// How many pages `n` items fill; one even for none.
pub fn pages(n: usize) -> usize {
    n.div_ceil(PER_PAGE).max(1)
}

/// What a click on slot `slot` of page `page` does, for a list of `n` items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// Take a stack of the item at this index of the list.
    Take(usize),
    /// Show this page.
    Page(usize),
    Nothing,
}

pub fn click(slot: usize, page: usize, n: usize) -> Click {
    let last = pages(n) - 1;
    let page = page.min(last);
    match slot {
        s if s < PER_PAGE => {
            let i = page * PER_PAGE + s;
            if i < n { Click::Take(i) } else { Click::Nothing }
        }
        PREV_SLOT if page > 0 => Click::Page(page - 1),
        NEXT_SLOT if page < last => Click::Page(page + 1),
        _ => Click::Nothing,
    }
}

// ---- Slice 18: the drawn window ----
//
// The tablet's window is the mod's own screen on the client (API 111), drawn as the terminal's:
// tabs, the search, the list, the limits. The server sends the storage's list (`wire`) on
// LIST_CHANNEL and asks the client to open or close the window on OPEN_CHANNEL; the client's
// presses come back on PRESS_CHANNEL.

/// Server to client: the storage's list, as the terminal's (`wire::encode`).
pub const LIST_CHANNEL: &str = "rustcraft:tablet_list";
/// Server to client: one byte, 1 to open the window, 0 to close it.
pub const OPEN_CHANNEL: &str = "rustcraft:tablet_open";
/// Client to server: a press of the window.
pub const PRESS_CHANNEL: &str = "rustcraft:tablet_press";

/// A press of the tablet's window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// A terminal press (`terminal::encode`): a stored item's stack or one into the inventory, a
    /// limit button.
    Terminal(i32),
    /// The stack in this inventory slot (0..9 the hotbar, 9..36 the rest) into the storage.
    Deposit(u8),
    /// The player closed the window.
    Closed,
    /// 0.21.2 (spec 49): a limit typed into the window (`typed`).
    Set(crate::storage::typed::Set),
}

pub fn encode_press(p: Press) -> Vec<u8> {
    match p {
        Press::Terminal(id) => {
            let mut out = vec![0];
            out.extend(id.to_le_bytes());
            out
        }
        Press::Deposit(slot) => vec![1, slot],
        Press::Closed => vec![2],
        Press::Set(set) => [vec![3], crate::storage::typed::encode_set(set)].concat(),
    }
}

/// What `encode_press` wrote; none for anything else, or a slot past the inventory's 36.
pub fn decode_press(bytes: &[u8]) -> Option<Press> {
    match bytes {
        [0, a, b, c, d] => Some(Press::Terminal(i32::from_le_bytes([*a, *b, *c, *d]))),
        [1, slot] if *slot < 36 => Some(Press::Deposit(*slot)),
        [2] => Some(Press::Closed),
        [3, set @ ..] => crate::storage::typed::decode_set(set).map(Press::Set),
        _ => None,
    }
}

/// The count a listed stack shows: what there is, up to a stack (at least one).
pub fn shown_count(count: u64, max_stack: u32) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX).clamp(1, max_stack.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::topology::{Housing, Layout, Node, analyse};
    use crate::tuning::Tuning;
    use std::collections::BTreeMap;

    /// A tier-1 node at the origin and `n` housings along x after it.
    fn line(n: i32) -> Topology {
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        for x in 1..=n {
            layout.housings.insert([x, 0, 0], Housing::default());
        }
        analyse(&layout, &Tuning::default())
    }

    #[test]
    fn a_storage_on_the_magistral_opens_within_the_node_range() {
        let t = line(10);
        // A cell beside the last housing.
        let s = vec![Candidate { id: 7, blocks: vec![[10, 1, 0]] }];
        assert_eq!(pick([5.0, 1.0, 0.5], &s, &t), Some(7));
        assert_eq!(pick([30.0, 1.0, 0.5], &s, &t), Some(7), "32 blocks from the node");
        assert_eq!(pick([40.0, 1.0, 0.5], &s, &t), None, "past the node's 32");
    }

    #[test]
    fn a_storage_off_the_magistral_never_opens() {
        let t = line(10);
        let s = vec![Candidate { id: 3, blocks: vec![[10, 5, 0]] }];
        assert_eq!(pick([5.0, 1.0, 0.5], &s, &t), None);
    }

    #[test]
    fn the_nearest_node_wins() {
        let mut layout = Layout::default();
        for (node, x0) in [([0, 0, 0], 1), ([100, 0, 0], 101)] {
            layout.nodes.insert(node, Node { tier: 1 });
            for x in x0..x0 + 3 {
                layout.housings.insert([x, 0, 0], Housing::default());
            }
        }
        let t = analyse(&layout, &Tuning::default());
        let s = vec![
            Candidate { id: 1, blocks: vec![[2, 1, 0]] },
            Candidate { id: 2, blocks: vec![[102, 1, 0]] },
        ];
        assert_eq!(pick([90.0, 0.0, 0.0], &s, &t), Some(2));
        assert_eq!(pick([10.0, 0.0, 0.0], &s, &t), Some(1));
        assert_eq!(pick([50.0, 0.0, 0.0], &s, &t), None, "between the bases");
    }

    #[test]
    fn every_window_press_comes_back() {
        // 0.21.2 (spec 49): a typed limit.
        let set = crate::storage::typed::Set {
            setting: crate::storage::typed::Setting::Limit,
            item: (812, 0),
            value: 3938,
        };
        for p in [
            Press::Terminal(-123_456),
            Press::Terminal(5),
            Press::Deposit(35),
            Press::Closed,
            Press::Set(set),
        ] {
            assert_eq!(decode_press(&encode_press(p)), Some(p));
        }
        assert_eq!(decode_press(&[3, 0, 1]), None);
        assert_eq!(decode_press(&[1, 36]), None);
        assert_eq!(decode_press(&[9]), None);
        assert_eq!(decode_press(&[]), None);
    }

    #[test]
    fn the_list_and_its_pages() {
        let items = BTreeMap::from([
            ("minecraft:dirt".to_owned(), 5),
            ("minecraft:stone".to_owned(), 900),
            ("minecraft:air_gone".to_owned(), 0),
        ]);
        let e = entries(&items);
        assert_eq!(e, vec![("minecraft:stone".to_owned(), 900), ("minecraft:dirt".to_owned(), 5)]);
        assert_eq!(pages(0), 1);
        assert_eq!(pages(45), 1);
        assert_eq!(pages(46), 2);
        assert_eq!(click(0, 0, 2), Click::Take(0));
        assert_eq!(click(2, 0, 2), Click::Nothing);
        assert_eq!(click(3, 1, 50), Click::Take(48));
        assert_eq!(click(NEXT_SLOT, 0, 50), Click::Page(1));
        assert_eq!(click(NEXT_SLOT, 1, 50), Click::Nothing);
        assert_eq!(click(PREV_SLOT, 1, 50), Click::Page(0));
        assert_eq!(click(PREV_SLOT, 0, 50), Click::Nothing);
        assert_eq!(shown_count(900, 64), 64);
        assert_eq!(shown_count(5, 64), 5);
        assert_eq!(shown_count(3, 1), 1);
    }
}
