//! The list of a storage the server sends the players who have its terminal open, as one mod
//! message (at most 30 000 bytes): the storage's state, its cells and slots, the cells its shape
//! misses (S8), and every stored item with its count, limit and «держать N». Items go as their
//! number in the item table both sides share and their damage, so a worn tool is its own line
//! (TM1). Since slice 15 it also says whether the terminal holds the recipe book, whether the
//! storage's line takes orders (M15), the queue of orders (O8) and, with the book, the items the
//! book can make (O2), as many as the message has room for after the rest. Version 3 adds an
//! order's wait for energy; a fluid (O12) goes by its number past the item table
//! (`orders::FLUID_NUMBERS`) wherever an item's number may stand.

use crate::geom::Pos;
use crate::orders::queue::MAX_ORDERS;
use crate::storage::Limit;

/// The largest message a mod may send.
pub const MAX_BYTES: usize = 30_000;
/// Holes named in one message: the window names a few, the rest is a count.
pub const MAX_HOLES: usize = 16;
/// Missing items or machines an order's line names.
pub const MAX_WANTS: usize = 3;
const VERSION: u8 = 3;
/// Version 2 (v0.15.0) is laid out as 3 is; it only knew no wait for energy.
const OLDER: u8 = 2;
const HEADER: usize = 1 + 12 + 1 + 4 + 8 + 8 + 1 + 2 + 4 + 1 + 1 + 1;
const HOLE: usize = 12;
const ENTRY: usize = 2 + 2 + 8 + 1 + 8 + 8;
const ORDER: usize = 2 + 2 + 4 + 4 + 1 + 1 + 1 + MAX_WANTS * 6;
const ORDERS: usize = MAX_ORDERS * ORDER;

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

/// M15: whether the storage's line takes orders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Smart {
    #[default]
    Works,
    /// No node of the smart tier on the line of the storage's item net.
    LowTier,
    /// The storage is on no item net.
    NoNet,
}

impl Smart {
    fn code(self) -> u8 {
        match self {
            Smart::Works => 0,
            Smart::LowTier => 1,
            Smart::NoNet => 2,
        }
    }

    fn of(code: u8) -> Smart {
        match code {
            1 => Smart::LowTier,
            2 => Smart::NoNet,
            _ => Smart::Works,
        }
    }
}

/// One stored item: its number in the item table, its damage, how many, its limit, its N.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub item: u16,
    pub damage: u16,
    pub count: u64,
    pub limit: Option<Limit>,
    /// O14: «держать N», 0 for none.
    pub keep: u64,
}

/// What an order waits for, as the window shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OrderWait {
    #[default]
    New,
    Running,
    /// O6: items (numbers) and how many are short, at most MAX_WANTS of them.
    Items(Vec<(u16, u32)>),
    /// O13: missing machine kinds (`orders::KINDS` places).
    Machines(Vec<u8>),
    /// Every machine of this kind is busy.
    Busy(u8),
    /// Its jobs stand on machines of this kind without energy or fuel.
    Energy(u8),
}

/// One order of the queue (O8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderLine {
    pub id: u16,
    pub item: u16,
    pub count: u32,
    pub done: u32,
    pub keep: bool,
    pub wait: OrderWait,
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
    /// O1: the terminal holds the recipe book.
    pub book: bool,
    pub smart: Smart,
    /// O8: the storage's orders, at most MAX_ORDERS.
    pub orders: Vec<OrderLine>,
    /// O2: items the recipe book can make, as numbers.
    pub orderable: Vec<u16>,
    /// Slice 18: the component stacks among `lines` (a line's damage with `terminal::VARIANT`),
    /// each with its saved form so the window draws and names it.
    pub variants: Vec<Variant>,
}

/// Slice 18: a component stack of the list: its item, its place among the item's
/// (`Store::variants_of`), and its saved stack in hex (`keys::payload`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Variant {
    pub item: u16,
    pub index: u16,
    pub payload: String,
}

/// The tail sections after the orderable items, each `tag, length (u16), bytes`; a reader skips
/// the tags it does not know, and a message without a tail (versions 2 and 3 before slice 18)
/// has none of them.
const TAIL_VARIANTS: u8 = 1;

/// How many lines fit in one message.
pub const MAX_LINES: usize = (MAX_BYTES - HEADER - MAX_HOLES * HOLE - ORDERS - 2) / ENTRY;

/// The message for `listing`: lines past MAX_LINES are left out (the caller sends the most
/// numerous first), and orderable items past the room the rest leaves.
pub fn encode(listing: &Listing) -> Vec<u8> {
    let holes = &listing.holes[..listing.holes.len().min(MAX_HOLES)];
    let lines = &listing.lines[..listing.lines.len().min(MAX_LINES)];
    let orders = &listing.orders[..listing.orders.len().min(MAX_ORDERS)];
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
    out.push(u8::from(listing.book));
    out.push(listing.smart.code());
    out.push(orders.len() as u8);
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
        out.extend(l.keep.to_le_bytes());
    }
    for o in orders {
        encode_order(&mut out, o);
    }
    let tail = encode_tail(listing, MAX_BYTES.saturating_sub(out.len() + 2));
    let room = MAX_BYTES.saturating_sub(out.len() + 2 + tail.len()) / 2;
    let orderable = &listing.orderable[..listing.orderable.len().min(room)];
    out.extend((orderable.len() as u16).to_le_bytes());
    for n in orderable {
        out.extend(n.to_le_bytes());
    }
    out.extend(tail);
    out
}

/// The tail sections, in at most `room` bytes: the variants that fit.
fn encode_tail(listing: &Listing, room: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut body = Vec::new();
    let mut n: u16 = 0;
    for v in &listing.variants {
        let len = 2 + 2 + 2 + v.payload.len();
        if 3 + 2 + body.len() + len > room.min(usize::from(u16::MAX)) {
            break;
        }
        body.extend(v.item.to_le_bytes());
        body.extend(v.index.to_le_bytes());
        body.extend((v.payload.len() as u16).to_le_bytes());
        body.extend(v.payload.as_bytes());
        n += 1;
    }
    if n > 0 {
        out.push(TAIL_VARIANTS);
        out.extend(((body.len() + 2) as u16).to_le_bytes());
        out.extend(n.to_le_bytes());
        out.extend(body);
    }
    out
}

/// Reads the tail sections into `listing`; a cut one ends the reading.
fn decode_tail(r: &mut Reader<'_>, listing: &mut Listing) {
    while let (Some(tag), Some(len)) = (r.u8(), r.u16()) {
        let Some((section, rest)) = r.0.split_at_checked(usize::from(len)) else {
            return;
        };
        r.0 = rest;
        let mut s = Reader(section);
        if tag == TAIL_VARIANTS {
            let n = s.u16().unwrap_or(0);
            for _ in 0..n {
                let (Some(item), Some(index), Some(len)) = (s.u16(), s.u16(), s.u16()) else {
                    break;
                };
                let Some((text, rest)) = s.0.split_at_checked(usize::from(len)) else {
                    break;
                };
                s.0 = rest;
                listing.variants.push(Variant {
                    item,
                    index,
                    payload: String::from_utf8_lossy(text).into_owned(),
                });
            }
        }
    }
}

fn encode_order(out: &mut Vec<u8>, o: &OrderLine) {
    out.extend(o.id.to_le_bytes());
    out.extend(o.item.to_le_bytes());
    out.extend(o.count.to_le_bytes());
    out.extend(o.done.to_le_bytes());
    out.push(u8::from(o.keep));
    match &o.wait {
        OrderWait::New => out.extend([0, 0]),
        OrderWait::Running => out.extend([1, 0]),
        OrderWait::Items(items) => {
            let items = &items[..items.len().min(MAX_WANTS)];
            out.extend([2, items.len() as u8]);
            for (item, n) in items {
                out.extend(item.to_le_bytes());
                out.extend(n.to_le_bytes());
            }
        }
        OrderWait::Machines(kinds) => {
            let kinds = &kinds[..kinds.len().min(MAX_WANTS)];
            out.extend([3, kinds.len() as u8]);
            out.extend(kinds);
        }
        OrderWait::Busy(kind) => out.extend([4, *kind]),
        OrderWait::Energy(kind) => out.extend([5, *kind]),
    }
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

    fn order(&mut self) -> Option<OrderLine> {
        let id = self.u16()?;
        let item = self.u16()?;
        let count = self.u32()?;
        let done = self.u32()?;
        let keep = self.u8()? == 1;
        let code = self.u8()?;
        let arg = self.u8()?;
        let wait = match code {
            1 => OrderWait::Running,
            2 => OrderWait::Items(
                (0..arg.min(MAX_WANTS as u8))
                    .map(|_| Some((self.u16()?, self.u32()?)))
                    .collect::<Option<Vec<_>>>()?,
            ),
            3 => OrderWait::Machines(
                (0..arg.min(MAX_WANTS as u8))
                    .map(|_| self.u8())
                    .collect::<Option<Vec<_>>>()?,
            ),
            4 => OrderWait::Busy(arg),
            5 => OrderWait::Energy(arg),
            _ => OrderWait::New,
        };
        Some(OrderLine {
            id,
            item,
            count,
            done,
            keep,
            wait,
        })
    }
}

/// What `encode` wrote; none for a message of another version or cut short.
pub fn decode(bytes: &[u8]) -> Option<Listing> {
    let mut r = Reader(bytes);
    if !matches!(r.u8()?, VERSION | OLDER) {
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
    let book = r.u8()? == 1;
    let smart = Smart::of(r.u8()?);
    let order_count = usize::from(r.u8()?);
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
        let keep = r.u64()?;
        let limit = (flag > 0).then_some(Limit {
            max,
            destroy_surplus: flag == 2,
        });
        lines.push(Line {
            item,
            damage,
            count,
            limit,
            keep,
        });
    }
    let orders = (0..order_count)
        .map(|_| r.order())
        .collect::<Option<Vec<_>>>()?;
    let orderable_count = usize::from(r.u16()?);
    let orderable = (0..orderable_count)
        .map(|_| r.u16())
        .collect::<Option<Vec<_>>>()?;
    let mut listing = Listing {
        terminal,
        state,
        cells,
        used,
        total,
        holes,
        hole_count,
        lines,
        book,
        smart,
        orders,
        orderable,
        variants: Vec::new(),
    };
    decode_tail(&mut r, &mut listing);
    Some(listing)
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
                    keep: (i % 5) as u64,
                })
                .collect(),
            book: true,
            smart: Smart::LowTier,
            orders: vec![
                OrderLine {
                    id: 3,
                    item: 812,
                    count: 4,
                    done: 2,
                    keep: false,
                    wait: OrderWait::Running,
                },
                OrderLine {
                    id: 4,
                    item: 9,
                    count: 64,
                    done: 0,
                    keep: true,
                    wait: OrderWait::Items(vec![(5, 2), (6, 7)]),
                },
                OrderLine {
                    id: 5,
                    item: 9,
                    count: 1,
                    done: 0,
                    keep: false,
                    wait: OrderWait::Machines(vec![3]),
                },
                OrderLine {
                    id: 6,
                    item: 9,
                    count: 1,
                    done: 0,
                    keep: false,
                    wait: OrderWait::Busy(0),
                },
                OrderLine {
                    id: 7,
                    item: crate::orders::FLUID_NUMBERS + 2,
                    count: 1,
                    done: 0,
                    keep: false,
                    wait: OrderWait::Energy(4),
                },
            ],
            orderable: vec![1, 2, 812],
            variants: vec![Variant {
                item: 3,
                index: 1,
                payload: "0a0b0c".to_owned(),
            }],
        }
    }

    #[test]
    fn variants_ride_in_the_tail_and_a_cut_tail_is_left_out() {
        let l = listing(2);
        let bytes = encode(&l);
        assert_eq!(decode(&bytes).unwrap().variants, l.variants);
        let cut = decode(&bytes[..bytes.len() - 1]).unwrap();
        assert!(cut.variants.is_empty());
    }

    #[test]
    fn a_listing_comes_back_as_it_went() {
        let l = listing(40);
        assert_eq!(decode(&encode(&l)), Some(l));
        let empty = Listing::default();
        assert_eq!(decode(&encode(&empty)), Some(empty));
    }

    #[test]
    fn a_big_storage_fits_one_message() {
        let mut l = listing(MAX_LINES + 100);
        l.orders = vec![
            OrderLine {
                wait: OrderWait::Items(vec![(1, 1); MAX_WANTS]),
                ..OrderLine::default()
            };
            MAX_ORDERS
        ];
        l.orderable = (0..20_000).collect();
        let bytes = encode(&l);
        assert!(bytes.len() <= MAX_BYTES, "{}", bytes.len());
        let back = decode(&bytes).unwrap();
        assert_eq!(back.lines.len(), MAX_LINES);
        assert_eq!(back.orders.len(), MAX_ORDERS);
        // The orderable items fill what is left: in a small storage, thousands of them.
        let mut small = listing(100);
        small.orderable = (0..20_000).collect();
        let bytes = encode(&small);
        assert!(bytes.len() <= MAX_BYTES);
        assert!(bytes.len() + 2 > MAX_BYTES);
        assert!(decode(&bytes).unwrap().orderable.len() > 10_000);
    }

    #[test]
    fn a_message_of_version_2_still_reads() {
        // v0.15.0 sent the same layout under version 2, with no wait for energy.
        let mut l = listing(3);
        l.orders.retain(|o| !matches!(o.wait, OrderWait::Energy(_)));
        l.variants.clear();
        let mut bytes = encode(&l);
        bytes[0] = 2;
        assert_eq!(decode(&bytes), Some(l));
        bytes[0] = 1;
        assert_eq!(decode(&bytes), None);
    }

    #[test]
    fn a_cut_message_reads_as_none() {
        let mut l = listing(3);
        l.variants.clear();
        let bytes = encode(&l);
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode(&[9]), None);
    }
}
