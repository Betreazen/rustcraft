//! Wave 2 (M5: «клиент рисует поток в окошках корпуса»; the handbook's scenes draw the flow as
//! dots moving along the housings): the plain item net's moves seen in the housings.
//!
//! A net moves its items at once (`net::items`); nothing rides in the pipe. So the flow is drawn
//! as the server saw it: each second the bundle takes that second's moves, keeps the first item
//! of each pair of blocks (`pick`, a few pairs a net) and lays a stream of that item on the
//! housings between the two joints (`spans`, on `topology::pipe_path`): one item set off each
//! second, riding `PER_BLOCK` ticks a block, those already under way drawn where they are. The
//! next second's write continues the same streams, so the dots move on without a jump; when the
//! net stops moving, the next write clears them. Clients draw them as an order's riding items
//! (`orders::ride::place`), under keys of their own.

use std::collections::BTreeMap;

use crate::geom::{Face, Pos, step};
use crate::net::items::ItemKey;
use crate::net::topology::{Layout, link_face};
use crate::orders::ride::{CENTRE, Span};
use crate::orders::ship::NetKey;

/// The housing's saved compound keeps its flow's spans under this key, three ints each…
pub const FLOW_KEY: &str = "flow";
/// …and the items, in the same order, under this one.
pub const FLOW_ITEMS_KEY: &str = "flow_items";
/// Proposal: the pairs of blocks drawn on one net each second.
pub const MAX_PAIRS: usize = 4;
/// Proposal: game ticks a drawn item takes through one block of pipe.
pub const PER_BLOCK: i32 = 4;
/// The ticks between two writes (a second): one item of a stream sets off each.
pub const EVERY: i32 = 20;
/// Proposal: the most housings of a level the flow is written on each second.
pub const MAX_HOUSINGS: usize = 512;

/// One move of a second: its net, the block it went from and to, the item.
pub type Move = (NetKey, Pos, Pos, ItemKey);

/// The moves to draw of `moves`: the first item of each pair of blocks of a net, no more than
/// `MAX_PAIRS` pairs a net, in the order they were made.
pub fn pick(moves: &[Move]) -> Vec<Move> {
    let mut per_net: BTreeMap<NetKey, usize> = BTreeMap::new();
    let mut out: Vec<Move> = Vec::new();
    for m in moves {
        if out.iter().any(|o| o.0 == m.0 && o.1 == m.1 && o.2 == m.2) {
            continue;
        }
        let n = per_net.entry(m.0).or_insert(0);
        if *n >= MAX_PAIRS {
            continue;
        }
        *n += 1;
        out.push(m.clone());
    }
    out
}

fn face_to(housing: Pos, block: Pos) -> u8 {
    Face::ALL
        .into_iter()
        .find(|&f| step(housing, f) == block)
        .map_or(CENTRE, Face::index)
}

/// The spans of a stream from the block `from` to the block `to` along `path` (housings with
/// their pipe from the first, `pipe_path`) at game tick `now`: for every item set off at `now`,
/// a second before, two before… that is still on its way, its ride through each housing still
/// to come or under way.
pub fn spans(layout: &Layout, path: &[(Pos, u32)], from: Pos, to: Pos, now: i64) -> Vec<(Pos, Span)> {
    let Some(&(_, last)) = path.last() else {
        return Vec::new();
    };
    let ride = (last as i32 + 1) * PER_BLOCK;
    let now32 = now as i32;
    let mut out = Vec::new();
    let end = path.len() - 1;
    for j in 0..=(ride / EVERY) {
        let start = now32.wrapping_sub(j * EVERY);
        for (i, &(housing, d)) in path.iter().enumerate() {
            let t0 = start.wrapping_add(d as i32 * PER_BLOCK);
            let t1 = t0.wrapping_add(PER_BLOCK);
            if t1.wrapping_sub(now32) <= 0 {
                continue;
            }
            let face_in = if i == 0 {
                face_to(housing, from)
            } else {
                link_face(layout, housing, path[i - 1].0).map_or(CENTRE, Face::index)
            };
            let face_out = if i == end {
                face_to(housing, to)
            } else {
                link_face(layout, housing, path[i + 1].0).map_or(CENTRE, Face::index)
            };
            out.push((housing, Span { face_in, face_out, t0, t1 }));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::topology::{Housing, Node};

    fn moved(net: usize, from: i32, to: i32, item: &str) -> Move {
        ((net, [0, 0, 0]), [from, 0, 0], [to, 0, 0], item.to_owned())
    }

    #[test]
    fn a_pair_is_drawn_once_and_a_net_a_few_pairs() {
        let moves = [
            moved(0, 1, 2, "minecraft:cobblestone"),
            moved(0, 1, 2, "minecraft:dirt"),
            moved(0, 1, 3, "minecraft:dirt"),
            moved(1, 1, 2, "minecraft:dirt"),
        ];
        let p = pick(&moves);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].3, "minecraft:cobblestone", "the first item of the pair");
        let many: Vec<Move> = (0..10).map(|k| moved(0, 1, 10 + k, "x")).collect();
        assert_eq!(pick(&many).len(), MAX_PAIRS);
    }

    #[test]
    fn a_stream_rides_the_housings_from_one_joint_to_the_other() {
        // A chest at z -1 of housing x 1, three housings x 1..=3, a chest at z -1 of x 3.
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        for x in 1..=3 {
            layout.housings.insert([x, 0, 0], Housing { slots: [None; 4] });
        }
        let path = [([1, 0, 0], 0), ([2, 0, 0], 1), ([3, 0, 0], 2)];
        let s = spans(&layout, &path, [1, 0, -1], [3, 0, -1], 1000);
        // The item set off now: through x 1 over ticks 1000..1004, x 3 over 1008..1012.
        assert!(s.contains(&([1, 0, 0], Span { face_in: Face::ZNeg.index(), face_out: Face::XPos.index(), t0: 1000, t1: 1004 })));
        assert!(s.contains(&([3, 0, 0], Span { face_in: Face::XNeg.index(), face_out: Face::ZNeg.index(), t0: 1008, t1: 1012 })));
        // The ride is 12 ticks: the one set off a second before is through already.
        assert!(s.iter().all(|(_, sp)| sp.t1 > 1000));
        assert_eq!(s.len(), 3);
        // A long line: the items of earlier seconds are still on their way.
        let long: Vec<(Pos, u32)> = (1..=12).map(|x| ([x, 0, 0], x as u32 - 1)).collect();
        for x in 4..=12 {
            layout.housings.insert([x, 0, 0], Housing { slots: [None; 4] });
        }
        let s = spans(&layout, &long, [1, 0, -1], [12, 0, -1], 1000);
        assert!(s.iter().any(|(p, sp)| *p == [12, 0, 0] && sp.t0 < 1000 + EVERY), "one set off before is near the end");
        assert!(spans(&layout, &[], [0; 3], [0; 3], 0).is_empty());
    }
}
