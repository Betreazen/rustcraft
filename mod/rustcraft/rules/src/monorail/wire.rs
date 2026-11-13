//! The monorail's mod messages (at most 30 000 bytes each):
//!
//! - the station's window (`Listing`), server to the players who have it open: its name and
//!   energy, every station it sees or saw with the trip there or why there is none (T7, T8.a,
//!   T8.b), and the rails as lines for the scheme, seen from above;
//! - a capsule (`CapsuleNote`), server to every player: which seat mob is a capsule and the
//!   corners of its way, so each client draws it with the right heading and bracket;
//! - a new name (`Rename`), a client to the server.

use super::route::Point;
use crate::geom::{Face, Pos};

/// The largest message a mod may send.
pub const MAX_BYTES: usize = 30_000;
/// The longest name a station keeps, in characters.
pub const NAME_MAX: usize = 24;
/// Stations one window lists.
pub const MAX_DESTINATIONS: usize = 96;
const VERSION: u8 = 1;
const SEGMENT: usize = 16;
const POINT: usize = 25;

/// A station's name as typed: trimmed, no control characters, at most NAME_MAX characters;
/// none when nothing is left.
pub fn clean_name(typed: &str) -> Option<String> {
    let kept: String = typed
        .chars()
        .filter(|c| !c.is_control())
        .take(NAME_MAX)
        .collect();
    let trimmed = kept.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Whether a trip to a station can start, and if not, why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reach {
    Ready,
    /// T8.a: the station lacks this much E.
    Short(f64),
    /// T8.b: the way there is broken; it ends nearest to it here.
    Broken(Pos),
    /// T8.b: the cabin has no room here.
    NoRoom(Pos),
}

/// A station the window lists.
#[derive(Clone, Debug, PartialEq)]
pub struct Destination {
    pub station: Pos,
    /// Empty: the station has no name yet.
    pub name: String,
    pub reach: Reach,
    /// Blocks of rail to it, ticks the trip takes, E it costs; 0 when it cannot be reached.
    pub length: u32,
    pub ticks: u32,
    pub cost: f64,
}

/// What a station's window shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listing {
    pub station: Pos,
    pub name: String,
    pub energy: f64,
    pub capacity: f64,
    /// The station touches a rail.
    pub on_rail: bool,
    pub destinations: Vec<Destination>,
    /// The rails seen from above, as lines from (x, z) to (x, z).
    pub segments: Vec<[i32; 4]>,
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend(v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend(v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend(v.to_le_bytes());
    }
    fn f32(&mut self, v: f64) {
        self.0.extend((v as f32).to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.0.extend(v.to_le_bytes());
    }
    fn pos(&mut self, p: Pos) {
        for c in p {
            self.i32(c);
        }
    }
    fn text(&mut self, s: &str) {
        let mut end = s.len().min(u8::MAX as usize);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        self.u8(end as u8);
        self.0.extend(&s.as_bytes()[..end]);
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_le_bytes)
    }
    fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }
    fn i32(&mut self) -> Option<i32> {
        self.take().map(i32::from_le_bytes)
    }
    fn f32(&mut self) -> Option<f64> {
        self.take().map(f32::from_le_bytes).map(f64::from)
    }
    fn f64(&mut self) -> Option<f64> {
        self.take().map(f64::from_le_bytes)
    }
    fn pos(&mut self) -> Option<Pos> {
        Some([self.i32()?, self.i32()?, self.i32()?])
    }
    fn text(&mut self) -> Option<String> {
        let n = usize::from(self.u8()?);
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        String::from_utf8(head.to_vec()).ok()
    }
}

/// The window's message: destinations past MAX_DESTINATIONS and lines past what fits are left
/// out.
pub fn encode_listing(l: &Listing) -> Vec<u8> {
    let mut w = Writer(Vec::new());
    w.u8(VERSION);
    w.pos(l.station);
    w.text(&l.name);
    w.f32(l.energy);
    w.f32(l.capacity);
    w.u8(u8::from(l.on_rail));
    let dests = &l.destinations[..l.destinations.len().min(MAX_DESTINATIONS)];
    w.u16(dests.len() as u16);
    for d in dests {
        w.pos(d.station);
        w.text(&d.name);
        match d.reach {
            Reach::Ready => {
                w.u8(0);
                w.pos([0; 3]);
                w.f32(0.0);
            }
            Reach::Short(short) => {
                w.u8(1);
                w.pos([0; 3]);
                w.f32(short);
            }
            Reach::Broken(at) => {
                w.u8(2);
                w.pos(at);
                w.f32(0.0);
            }
            Reach::NoRoom(at) => {
                w.u8(3);
                w.pos(at);
                w.f32(0.0);
            }
        }
        w.u32(d.length);
        w.u32(d.ticks);
        w.f32(d.cost);
    }
    let room = MAX_BYTES.saturating_sub(w.0.len() + 2) / SEGMENT;
    let segments = &l.segments[..l.segments.len().min(room)];
    w.u16(segments.len() as u16);
    for s in segments {
        for c in s {
            w.i32(*c);
        }
    }
    w.0
}

pub fn decode_listing(bytes: &[u8]) -> Option<Listing> {
    let mut r = Reader(bytes);
    if r.u8()? != VERSION {
        return None;
    }
    let station = r.pos()?;
    let name = r.text()?;
    let energy = r.f32()?;
    let capacity = r.f32()?;
    let on_rail = r.u8()? != 0;
    let count = usize::from(r.u16()?);
    let mut destinations = Vec::with_capacity(count);
    for _ in 0..count {
        let station = r.pos()?;
        let name = r.text()?;
        let code = r.u8()?;
        let at = r.pos()?;
        let short = r.f32()?;
        let reach = match code {
            0 => Reach::Ready,
            1 => Reach::Short(short),
            2 => Reach::Broken(at),
            3 => Reach::NoRoom(at),
            _ => return None,
        };
        destinations.push(Destination {
            station,
            name,
            reach,
            length: r.u32()?,
            ticks: r.u32()?,
            cost: r.f32()?,
        });
    }
    let count = usize::from(r.u16()?);
    let segments = (0..count)
        .map(|_| Some([r.i32()?, r.i32()?, r.i32()?, r.i32()?]))
        .collect::<Option<Vec<_>>>()?;
    Some(Listing {
        station,
        name,
        energy,
        capacity,
        on_rail,
        destinations,
        segments,
    })
}

/// The rails as lines seen from above: one line a joined pair of rails one block apart across,
/// straight runs merged. A step straight up or down is no line.
pub fn scheme(rails: &[Pos], joined: impl Fn(Pos, Pos) -> bool) -> Vec<[i32; 4]> {
    use std::collections::BTreeSet;
    let mut pairs: BTreeSet<([i32; 2], [i32; 2])> = BTreeSet::new();
    let set: BTreeSet<Pos> = rails.iter().copied().collect();
    for &p in &set {
        for d in [[1, 0], [0, 1]] {
            for dy in [-1, 0, 1] {
                let q = [p[0] + d[0], p[1] + dy, p[2] + d[1]];
                if set.contains(&q) && joined(p, q) {
                    pairs.insert(([p[0], p[2]], [q[0], q[2]]));
                }
            }
        }
    }
    // Merge runs along each axis.
    let mut out = Vec::new();
    for axis in 0..2 {
        let mut along: Vec<([i32; 2], [i32; 2])> = pairs
            .iter()
            .copied()
            .filter(|(a, b)| a[1 - axis] == b[1 - axis])
            .collect();
        along.sort_by_key(|(a, _)| (a[1 - axis], a[axis]));
        let mut run: Option<([i32; 2], [i32; 2])> = None;
        for (a, b) in along {
            run = match run {
                Some((s, e)) if e == a => Some((s, b)),
                Some((s, e)) => {
                    out.push([s[0], s[1], e[0], e[1]]);
                    Some((a, b))
                }
                None => Some((a, b)),
            };
        }
        if let Some((s, e)) = run {
            out.push([s[0], s[1], e[0], e[1]]);
        }
    }
    out
}

/// What the server tells every player about a capsule.
#[derive(Clone, Debug, PartialEq)]
pub enum CapsuleNote {
    /// The mob `seat` is a capsule; its passenger's feet are `rise` over it and go along
    /// `points`.
    Ride {
        seat: u32,
        rise: f64,
        points: Vec<Point>,
    },
    /// The capsule is gone.
    Gone { seat: u32 },
}

/// Points one message carries; a way with more keeps every n-th and its last.
pub const MAX_POINTS: usize = (MAX_BYTES - 16) / POINT;

pub fn encode_capsule(note: &CapsuleNote) -> Vec<u8> {
    let mut w = Writer(Vec::new());
    w.u8(VERSION);
    match note {
        CapsuleNote::Gone { seat } => {
            w.u8(0);
            w.u32(*seat);
        }
        CapsuleNote::Ride { seat, rise, points } => {
            w.u8(1);
            w.u32(*seat);
            w.f32(*rise);
            let every = points.len().div_ceil(MAX_POINTS).max(1);
            let kept: Vec<&Point> = points
                .iter()
                .enumerate()
                .filter(|(i, _)| i % every == 0 || *i + 1 == points.len())
                .map(|(_, p)| p)
                .collect();
            w.u16(kept.len() as u16);
            for p in kept {
                for c in p.at {
                    w.f64(c);
                }
                w.u8(p.mount.index());
            }
        }
    }
    w.0
}

pub fn decode_capsule(bytes: &[u8]) -> Option<CapsuleNote> {
    let mut r = Reader(bytes);
    if r.u8()? != VERSION {
        return None;
    }
    match r.u8()? {
        0 => Some(CapsuleNote::Gone { seat: r.u32()? }),
        1 => {
            let seat = r.u32()?;
            let rise = r.f32()?;
            let count = usize::from(r.u16()?);
            let points = (0..count)
                .map(|_| {
                    let at = [r.f64()?, r.f64()?, r.f64()?];
                    let mount = Face::from_index(r.u8()?)?;
                    Some(Point {
                        at,
                        mount,
                        speed: 0.0,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(CapsuleNote::Ride { seat, rise, points })
        }
        _ => None,
    }
}

/// A player's new name for a station.
#[derive(Clone, Debug, PartialEq)]
pub struct Rename {
    pub station: Pos,
    pub name: String,
}

pub fn encode_rename(r: &Rename) -> Vec<u8> {
    let mut w = Writer(Vec::new());
    w.u8(VERSION);
    w.pos(r.station);
    w.text(&r.name);
    w.0
}

pub fn decode_rename(bytes: &[u8]) -> Option<Rename> {
    let mut r = Reader(bytes);
    if r.u8()? != VERSION {
        return None;
    }
    Some(Rename {
        station: r.pos()?,
        name: r.text()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listing_survives_its_message() {
        let l = Listing {
            station: [1, 2, 3],
            name: "Дом".into(),
            energy: 1440.0,
            capacity: 2000.0,
            on_rail: true,
            destinations: vec![
                Destination {
                    station: [10, 2, 3],
                    name: "Ферма".into(),
                    reach: Reach::Ready,
                    length: 150,
                    ticks: 380,
                    cost: 37.5,
                },
                Destination {
                    station: [-4, 2, 30],
                    name: String::new(),
                    reach: Reach::Broken([-4, 2, 20]),
                    length: 0,
                    ticks: 0,
                    cost: 0.0,
                },
            ],
            segments: vec![[0, 0, 10, 0], [5, 0, 5, 7]],
        };
        assert_eq!(decode_listing(&encode_listing(&l)), Some(l));
        assert_eq!(decode_listing(&[9]), None);
    }

    #[test]
    fn a_huge_scheme_is_cut_to_one_message() {
        let l = Listing {
            segments: (0..5000).map(|i| [i, 0, i, 1]).collect(),
            ..Listing::default()
        };
        let bytes = encode_listing(&l);
        assert!(bytes.len() <= MAX_BYTES);
        assert!(decode_listing(&bytes).unwrap().segments.len() > 1000);
    }

    #[test]
    fn a_capsule_note_survives_its_message() {
        let note = CapsuleNote::Ride {
            seat: 77,
            rise: 0.25,
            points: vec![
                Point {
                    at: [0.5, 64.05, 0.5],
                    mount: Face::YNeg,
                    speed: 0.0,
                },
                Point {
                    at: [10.5, 64.05, 0.5],
                    mount: Face::ZPos,
                    speed: 0.0,
                },
            ],
        };
        assert_eq!(decode_capsule(&encode_capsule(&note)), Some(note));
        let gone = CapsuleNote::Gone { seat: 3 };
        assert_eq!(decode_capsule(&encode_capsule(&gone)), Some(gone));
    }

    #[test]
    fn a_name_is_trimmed_and_cut() {
        assert_eq!(clean_name("  Дом \n"), Some("Дом".into()));
        assert_eq!(clean_name("   "), None);
        assert_eq!(
            clean_name(&"ж".repeat(40)).unwrap().chars().count(),
            NAME_MAX
        );
        let r = Rename {
            station: [1, -2, 3],
            name: "Шахта".into(),
        };
        assert_eq!(decode_rename(&encode_rename(&r)), Some(r));
    }

    #[test]
    fn the_scheme_merges_straight_runs_and_skips_lifts() {
        let mut rails: Vec<Pos> = (0..5).map(|x| [x, 0, 0]).collect();
        rails.extend((1..4).map(|z| [2, 0, z]));
        rails.extend((1..4).map(|y| [4, y, 0]));
        let lines = scheme(&rails, |_, _| true);
        assert!(lines.contains(&[0, 0, 4, 0]));
        assert!(lines.contains(&[2, 0, 2, 3]));
        assert_eq!(lines.len(), 2);
    }
}
