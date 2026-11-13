//! The list of a storage the server sends the players who have its terminal open, as one mod
//! message (at most 30 000 bytes): the storage's state, its cells and slots, the cells its shape
//! misses (S8), and every stored item with its count and limit. Items go as their number in the
//! item table both sides share and their damage, so a worn tool is its own line (TM1).

use crate::geom::Pos;
use crate::storage::Limit;

/// The largest message a mod may send.
pub const MAX_BYTES: usize = 30_000;
/// Holes named in one message: the window names a few, the rest is a count.
pub const MAX_HOLES: usize = 16;
const VERSION: u8 = 1;
const HEADER: usize = 1 + 12 + 1 + 4 + 8 + 8 + 1 + 2 + 4;
const HOLE: usize = 12;
const ENTRY: usize = 2 + 2 + 8 + 1 + 8;

/// Whether the storage works.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Works,
    /// S8: the cells are no solid box; it neither takes nor gives, and loses nothing.
    Holes,
    /// The terminal touches no cell.
    NoCells,
}

impl State {
    fn code(self) -> u8 {
        match self {
            State::Works => 0,
            State::Holes => 1,
            State::NoCells => 2,
        }
    }

    fn of(code: u8) -> State {
        match code {
            1 => State::Holes,
            2 => State::NoCells,
            _ => State::Works,
        }
    }
}

/// One stored item: its number in the item table, its damage, how many, its limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub item: u16,
    pub damage: u16,
    pub count: u64,
    pub limit: Option<Limit>,
}

/// What a terminal's window shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    pub terminal: Pos,
    pub state: State,
    pub cells: u32,
    pub used: u64,
    pub total: u64,
    /// Holes in the box, at most MAX_HOLES of them, and how many there are in all.
    pub holes: Vec<Pos>,
    pub hole_count: u32,
    pub lines: Vec<Line>,
}

/// How many lines fit in one message.
pub const MAX_LINES: usize = (MAX_BYTES - HEADER - MAX_HOLES * HOLE) / ENTRY;

/// The message for `listing`: lines past MAX_LINES are left out (the caller sends the most
/// numerous first).
pub fn encode(listing: &Listing) -> Vec<u8> {
    let holes = &listing.holes[..listing.holes.len().min(MAX_HOLES)];
    let lines = &listing.lines[..listing.lines.len().min(MAX_LINES)];
    let mut out = Vec::with_capacity(HEADER + holes.len() * HOLE + lines.len() * ENTRY);
    out.push(VERSION);
    for c in listing.terminal {
        out.extend(c.to_le_bytes());
    }
    out.push(listing.state.code());
    out.extend(listing.cells.to_le_bytes());
    out.extend(listing.used.to_le_bytes());
    out.extend(listing.total.to_le_bytes());
    out.push(holes.len() as u8);
    let hole_count = u16::try_from(listing.hole_count).unwrap_or(u16::MAX);
    out.extend(hole_count.to_le_bytes());
    out.extend((lines.len() as u32).to_le_bytes());
    for h in holes {
        for c in h {
            out.extend(c.to_le_bytes());
        }
    }
    for l in lines {
        out.extend(l.item.to_le_bytes());
        out.extend(l.damage.to_le_bytes());
        out.extend(l.count.to_le_bytes());
        match l.limit {
            Some(limit) => {
                out.push(1 + u8::from(limit.destroy_surplus));
                out.extend(limit.max.to_le_bytes());
            }
            None => {
                out.push(0);
                out.extend(0u64.to_le_bytes());
            }
        }
    }
    out
}

/// A reader over a message's bytes.
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
    fn u64(&mut self) -> Option<u64> {
        self.take().map(u64::from_le_bytes)
    }
    fn pos(&mut self) -> Option<Pos> {
        Some([self.i32()?, self.i32()?, self.i32()?])
    }
}

/// What `encode` wrote; none for a message of another version or cut short.
pub fn decode(bytes: &[u8]) -> Option<Listing> {
    let mut r = Reader(bytes);
    if r.u8()? != VERSION {
        return None;
    }
    let terminal = r.pos()?;
    let state = State::of(r.u8()?);
    let cells = r.u32()?;
    let used = r.u64()?;
    let total = r.u64()?;
    let holes_sent = usize::from(r.u8()?);
    let hole_count = u32::from(r.u16()?);
    let line_count = r.u32()? as usize;
    let holes = (0..holes_sent)
        .map(|_| r.pos())
        .collect::<Option<Vec<_>>>()?;
    let mut lines = Vec::with_capacity(line_count.min(MAX_LINES));
    for _ in 0..line_count {
        let item = r.u16()?;
        let damage = r.u16()?;
        let count = r.u64()?;
        let flag = r.u8()?;
        let max = r.u64()?;
        let limit = (flag > 0).then_some(Limit {
            max,
            destroy_surplus: flag == 2,
        });
        lines.push(Line {
            item,
            damage,
            count,
            limit,
        });
    }
    Some(Listing {
        terminal,
        state,
        cells,
        used,
        total,
        holes,
        hole_count,
        lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(lines: usize) -> Listing {
        Listing {
            terminal: [10, 64, -3],
            state: State::Holes,
            cells: 3,
            used: 6,
            total: 81,
            holes: vec![[11, 64, -3]],
            hole_count: 1,
            lines: (0..lines)
                .map(|i| Line {
                    item: i as u16,
                    damage: (i % 3) as u16,
                    count: 1 << (i % 40),
                    limit: (i % 2 == 0).then_some(Limit {
                        max: 512,
                        destroy_surplus: i % 4 == 0,
                    }),
                })
                .collect(),
        }
    }

    #[test]
    fn a_listing_comes_back_as_it_went() {
        let l = listing(40);
        assert_eq!(decode(&encode(&l)), Some(l));
    }

    #[test]
    fn a_big_storage_fits_one_message() {
        let l = listing(MAX_LINES + 100);
        let bytes = encode(&l);
        assert!(bytes.len() <= MAX_BYTES);
        assert_eq!(decode(&bytes).map(|d| d.lines.len()), Some(MAX_LINES));
    }

    #[test]
    fn a_cut_message_reads_as_none() {
        let bytes = encode(&listing(3));
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode(&[9]), None);
    }
}
