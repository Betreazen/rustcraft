//! Slice 18 (guide `orders`, the handbook's scene of items moving in the housings): an order's
//! items seen riding the housings.
//!
//! A shipment (`ship`) is a model of the ride with no cells of its own; here it is laid on the
//! housings between the two joints (`net::topology::pipe_path`). Its last item stands at the
//! first housing while it loads (as long as its net would load it alone), then rides the path at an even pace: `Shipment::progress` of
//! the way at the server's game tick now, the whole way in `full_ride` ticks. Each housing it
//! will still ride through gets a `Span`: the face it comes in by, the face it leaves by (a
//! path's end may have neither: the centre) and the game ticks it comes in and leaves. The
//! server writes them into the housing's saved compound once a second (`encode`) and sends them
//! to clients, which draw the item at its place between those ticks (`place`) from the frame's
//! game time and partial tick, so it moves smoothly though the server writes once a second.
//!
//! Plain item-net moves (`net::items`) are instant per tick and have no riding item, so only
//! orders' shipments are drawn (recorded under «Пределы движка» in the plan).

use crate::geom::{Face, Pos, step};
use crate::net::items::ItemKey;
use crate::net::topology::{Layout, link_face};
use crate::orders::ship::Shipment;

/// The housing's saved compound keeps its riding items' spans under this key, three ints each.
pub const RIDE_KEY: &str = "ride";
/// …and the items, in the same order, under this one.
pub const RIDE_ITEMS_KEY: &str = "ride_items";
/// The face code of the housing's centre (faces are `Face::index`, 0–5).
pub const CENTRE: u8 = 6;
/// Proposal: the kinds of item of one shipment drawn, one behind the other.
pub const MAX_KINDS: usize = 3;
/// Proposal: ticks each further kind of a shipment rides behind the one before it.
pub const LAG_TICKS: i32 = 4;
/// The longest a loading shipment is shown standing at its start, in ticks.
pub const MAX_WAIT_TICKS: u64 = 12_000;
/// Proposal: the most riding items one housing is drawn with.
pub const MAX_PER_HOUSING: usize = 8;

/// An item's ride through one housing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub face_in: u8,
    pub face_out: u8,
    /// Game ticks it comes in and leaves, as the low 32 bits of the game time.
    pub t0: i32,
    pub t1: i32,
}

fn code(face: Option<Face>) -> u8 {
    face.map_or(CENTRE, Face::index)
}

/// The face of the housing at `housing` that touches the block at `block`, if any.
fn face_to(housing: Pos, block: Pos) -> Option<Face> {
    Face::ALL.into_iter().find(|&f| step(housing, f) == block)
}

/// The housings `ship`'s last item will still ride through, each with its span, `lag` ticks
/// behind: `path` is the housings from the joint at the block `from` to the joint at the block
/// `to`, with their pipe so far (`pipe_path`), and `now` the server's game tick. Each housing is a
/// block of the way and a node between two is another, as the ride counts them (`Route`).
pub fn spans(
    layout: &Layout,
    path: &[(Pos, u32)],
    from: Pos,
    to: Pos,
    ship: &Shipment,
    now: i64,
    lag: i32,
) -> Vec<(Pos, Span)> {
    let Some(&(first, _)) = path.first() else {
        return Vec::new();
    };
    let now32 = now as i32;
    let entry = code(face_to(first, from));
    let mut out = Vec::new();
    // While it loads it stands at the first housing, for as long as its net would load it alone
    // (one queued behind others is put right by the next second's write), then rides on.
    let wait = ship
        .to_load
        .div_ceil(ship.load_rate.max(1))
        .min(MAX_WAIT_TICKS) as i64;
    if wait > 0 {
        let t0 = now32.wrapping_add(lag);
        let span = Span {
            face_in: entry,
            face_out: entry,
            t0,
            t1: t0.wrapping_add(wait as i32),
        };
        out.push((first, span));
    }
    if ship.full_ride == 0 {
        return out;
    }
    let length = f64::from(path.last().map_or(0, |p| p.1) + 1);
    let per_block = f64::from(ship.full_ride) / length;
    let done = if wait > 0 {
        -(wait as f64)
    } else {
        ship.progress() * f64::from(ship.full_ride)
    };
    let at = |blocks: u32| (f64::from(blocks) * per_block - done).round() as i64 + i64::from(lag);
    let last = path.len() - 1;
    let riding = path.iter().enumerate().filter_map(|(i, &(housing, d))| {
        let (enter, leave) = (at(d), at(d + 1));
        if leave <= 0 {
            return None;
        }
        let face_in = if i == 0 {
            entry
        } else {
            code(link_face(layout, housing, path[i - 1].0))
        };
        let face_out = if i == last {
            code(face_to(housing, to))
        } else {
            code(link_face(layout, housing, path[i + 1].0))
        };
        let span = Span {
            face_in,
            face_out,
            t0: now32.wrapping_add(enter as i32),
            t1: now32.wrapping_add(leave as i32),
        };
        Some((housing, span))
    });
    out.extend(riding);
    out
}

/// The point of face code `code` in the housing, in blocks from its corner.
fn point(code: u8) -> [f32; 3] {
    match Face::from_index(code) {
        Some(f) => {
            let n = f.offset();
            [0, 1, 2].map(|i| 0.5 + 0.5 * n[i] as f32)
        }
        None => [0.5; 3],
    }
}

fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// Where in its housing an item riding `span` is at game time `now` and partial tick `partial`,
/// in blocks from the housing's corner: from its face in to the centre, then to its face out;
/// none before it comes in or after it leaves. One that stands (in and out by one face) stays.
pub fn place(span: Span, now: i64, partial: f32) -> Option<[f32; 3]> {
    let elapsed = (now as i32).wrapping_sub(span.t0) as f32 + partial;
    let length = span.t1.wrapping_sub(span.t0) as f32;
    if elapsed < 0.0 || elapsed > length {
        return None;
    }
    if span.face_in == span.face_out {
        return Some(point(span.face_in));
    }
    let t = if length > 0.0 { elapsed / length } else { 1.0 };
    let centre = point(CENTRE);
    Some(if t < 0.5 {
        lerp(point(span.face_in), centre, t * 2.0)
    } else {
        lerp(centre, point(span.face_out), (t - 0.5) * 2.0)
    })
}

/// A housing's riding items as its saved compound keeps them: three ints a span, and the items.
pub fn encode(riding: &[(ItemKey, Span)]) -> (Vec<i32>, Vec<ItemKey>) {
    riding
        .iter()
        .take(MAX_PER_HOUSING)
        .map(|(item, s)| {
            let faces = i32::from(s.face_in) | (i32::from(s.face_out) << 8);
            ([faces, s.t0, s.t1], item.clone())
        })
        .fold((Vec::new(), Vec::new()), |(mut ints, mut items), (three, item)| {
            ints.extend(three);
            items.push(item);
            (ints, items)
        })
}

/// What `encode` wrote; a span without its item, or an item without its span, is left out.
pub fn decode(ints: &[i32], items: &[ItemKey]) -> Vec<(ItemKey, Span)> {
    ints.chunks_exact(3)
        .zip(items)
        .map(|(c, item)| {
            let span = Span {
                face_in: (c[0] & 0xFF) as u8,
                face_out: ((c[0] >> 8) & 0xFF) as u8,
                t0: c[1],
                t1: c[2],
            };
            (item.clone(), span)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::topology::{Channel, ChannelKind, Housing, Node};
    use crate::orders::ship::{Leg, Route};
    use crate::tuning::Tuning;

    const IRON: &str = "minecraft:iron_ingot";

    fn layout(xs: &[i32], nodes: &[i32]) -> Layout {
        let mut l = Layout::default();
        let channel = Some(Channel {
            kind: ChannelKind::Item,
            tier: 2,
        });
        for &x in xs {
            l.housings.insert(
                [x, 0, 0],
                Housing {
                    slots: [channel, None, None, None],
                },
            );
        }
        for &x in nodes {
            l.nodes.insert([x, 0, 0], Node { tier: 3 });
        }
        l
    }

    fn shipment(full_ride: u32, ride: u32, to_load: u64) -> Shipment {
        let route = Route::of_tier((0, [1, 0, 0]), 3, 2, &Tuning::default());
        let mut s = Shipment::new(
            1,
            1,
            Leg::Machine { machine: [5, 0, 0] },
            vec![(IRON.into(), 4)],
            &route,
            20.0,
        );
        s.full_ride = full_ride;
        s.ride = ride;
        s.to_load = to_load;
        s
    }

    /// Housings 1–4 along x, the source block at x 0 and the target at x 5.
    fn path() -> Vec<(Pos, u32)> {
        vec![([1, 0, 0], 0), ([2, 0, 0], 1), ([3, 0, 0], 2), ([4, 0, 0], 3)]
    }

    #[test]
    fn a_loading_shipment_stands_at_its_first_housing_then_rides_on() {
        let l = layout(&[1, 2, 3, 4], &[]);
        // 2 items to load at 100 thousandths a tick: 20 ticks standing, then 25 ticks a housing.
        let mut s = shipment(100, 100, 2000);
        s.load_rate = 100;
        let got = spans(&l, &path(), [0, 0, 0], [5, 0, 0], &s, 1000, 0);
        let (xn, xp) = (Face::XNeg.index(), Face::XPos.index());
        assert_eq!(got.len(), 1 + 4);
        assert_eq!(
            got[0],
            (
                [1, 0, 0],
                Span {
                    face_in: xn,
                    face_out: xn,
                    t0: 1000,
                    t1: 1020
                }
            )
        );
        assert_eq!(
            got[1],
            (
                [1, 0, 0],
                Span {
                    face_in: xn,
                    face_out: xp,
                    t0: 1020,
                    t1: 1045
                }
            )
        );
        assert_eq!((got[4].1.t0, got[4].1.t1), (1095, 1120));
        // It stands at the face that touches the block it leaves.
        let p = place(got[0].1, 1010, 0.5).unwrap();
        assert_eq!(p, [0.0, 0.5, 0.5]);
        assert_eq!(place(got[0].1, 999, 0.0), None);
    }

    #[test]
    fn a_riding_shipment_goes_through_each_housing_it_has_not_passed_yet() {
        let l = layout(&[1, 2, 3, 4], &[]);
        // Four blocks in 100 ticks, half done: through housing 3 in the next 25 ticks, then 4.
        let s = shipment(100, 50, 0);
        let got = spans(&l, &path(), [0, 0, 0], [5, 0, 0], &s, 1000, 0);
        let (xn, xp) = (Face::XNeg.index(), Face::XPos.index());
        assert_eq!(
            got,
            vec![
                (
                    [3, 0, 0],
                    Span {
                        face_in: xn,
                        face_out: xp,
                        t0: 1000,
                        t1: 1025
                    }
                ),
                (
                    [4, 0, 0],
                    Span {
                        face_in: xn,
                        face_out: xp,
                        t0: 1025,
                        t1: 1050
                    }
                ),
            ]
        );
        // A second later the same ride gives the same ticks: nothing new to send.
        let later = shipment(100, 30, 0);
        let again = spans(&l, &path(), [0, 0, 0], [5, 0, 0], &later, 1020, 0);
        assert_eq!(again, got);
        // A kind riding behind it, LAG_TICKS later.
        let behind = spans(&l, &path(), [0, 0, 0], [5, 0, 0], &s, 1000, LAG_TICKS);
        // It is still in housing 2, and comes into housing 3 that much later.
        assert_eq!(behind[0].0, [2, 0, 0]);
        assert_eq!((behind[1].0, behind[1].1.t0), ([3, 0, 0], 1000 + LAG_TICKS));
        // Arrived: nothing left to ride.
        assert!(spans(&l, &path(), [0, 0, 0], [5, 0, 0], &shipment(100, 0, 0), 1000, 0).is_empty());
    }

    #[test]
    fn a_node_is_a_block_of_the_way_and_an_end_without_a_touching_block_is_the_centre() {
        // Housings 1, 2, a node at 3, housing 4; the target block is not beside housing 4.
        let l = layout(&[1, 2, 4], &[3]);
        let path = vec![([1, 0, 0], 0), ([2, 0, 0], 1), ([4, 0, 0], 3)];
        let s = shipment(80, 80, 0);
        let got = spans(&l, &path, [0, 0, 0], [9, 9, 9], &s, 0, 0);
        // Four blocks, 20 ticks each: the node takes ticks 40–60.
        let ticks: Vec<(i32, i32)> = got.iter().map(|(_, s)| (s.t0, s.t1)).collect();
        assert_eq!(ticks, vec![(0, 20), (20, 40), (60, 80)]);
        assert_eq!(got[1].1.face_out, Face::XPos.index());
        assert_eq!(got[2].1.face_in, Face::XNeg.index());
        assert_eq!(got[2].1.face_out, CENTRE);
    }

    #[test]
    fn an_item_moves_from_its_face_in_through_the_centre_to_its_face_out() {
        let span = Span {
            face_in: Face::XNeg.index(),
            face_out: Face::YPos.index(),
            t0: 100,
            t1: 120,
        };
        assert_eq!(place(span, 100, 0.0), Some([0.0, 0.5, 0.5]));
        assert_eq!(place(span, 105, 0.0), Some([0.25, 0.5, 0.5]));
        assert_eq!(place(span, 110, 0.0), Some([0.5, 0.5, 0.5]));
        assert_eq!(place(span, 119, 1.0), Some([0.5, 1.0, 0.5]));
        assert_eq!(place(span, 121, 0.0), None);
        // The game time's low bits wrap: a ride over the wrap still moves.
        let wrap = Span {
            t0: i32::MAX - 4,
            t1: (i32::MAX).wrapping_add(16),
            ..span
        };
        let now = i64::from(i32::MAX) + 6;
        assert_eq!(place(wrap, now, 0.0), Some([0.5, 0.5, 0.5]));
    }

    #[test]
    fn a_housings_riding_items_come_back_as_they_went() {
        let riding: Vec<(ItemKey, Span)> = (0..10)
            .map(|k| {
                let span = Span {
                    face_in: (k % 7) as u8,
                    face_out: CENTRE,
                    t0: -5 + k,
                    t1: 40 + k,
                };
                (format!("minecraft:item_{k}"), span)
            })
            .collect();
        let (ints, items) = encode(&riding);
        assert_eq!(items.len(), MAX_PER_HOUSING);
        assert_eq!(ints.len(), 3 * MAX_PER_HOUSING);
        assert_eq!(decode(&ints, &items), riding[..MAX_PER_HOUSING].to_vec());
        assert!(decode(&ints[..2], &items).is_empty());
        assert_eq!(decode(&ints, &items[..1]).len(), 1);
    }
}
