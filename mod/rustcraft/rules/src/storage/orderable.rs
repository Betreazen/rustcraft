//! 0.21.1 (spec 47, OT12): the items the recipe book can make (O2), sent apart from the storage's
//! list. They depend only on the book, not on what is stored, so the server sends them to a player
//! once when a terminal or the tablet opens and again only when the book is built anew; the list
//! (`wire`) then keeps its 30 000 bytes for the stored lines and the component stacks. Before
//! 0.21.1 they rode at the end of the list in whatever room the rest left, and a storage of a few
//! hundred kinds with many enchanted books left none: the search found nothing to order.
//!
//! A message is `version, stamp (u32), part (u16), parts (u16), count (u16), count numbers (u16)`;
//! a list longer than one message goes in several parts under one stamp, and the client puts them
//! together (`Assembler`). A reader takes a newer version's fields it knows and skips what follows
//! them, so an older client reads a newer server's message.

use crate::storage::wire::{Listing, MAX_BYTES};

/// Server to client: a part of the list of what the recipe book can make.
pub const CHANNEL: &str = "rustcraft:terminal_orderable";
const VERSION: u8 = 1;
const HEADER: usize = 1 + 4 + 2 + 2 + 2;
/// Numbers in one part.
pub const PER_PART: usize = (MAX_BYTES - HEADER) / 2;
/// The most parts a list is read in: past it a message is taken as broken (a list of item
/// numbers never needs more than five).
pub const MAX_PARTS: u16 = 64;

/// What names one list: its parts carry it, and the server sends a player the list again only when
/// it changed.
pub fn stamp(numbers: &[u16]) -> u32 {
    // FNV-1a over the count and the numbers.
    let mut h: u32 = 0x811c_9dc5;
    let len = u32::try_from(numbers.len()).unwrap_or(u32::MAX);
    for b in len
        .to_le_bytes()
        .into_iter()
        .chain(numbers.iter().flat_map(|n| n.to_le_bytes()))
    {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// One message of a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub stamp: u32,
    pub index: u16,
    pub parts: u16,
    pub numbers: Vec<u16>,
}

/// The messages that carry `numbers`: one for an empty list.
pub fn encode(numbers: &[u16]) -> Vec<Vec<u8>> {
    let stamp = stamp(numbers);
    let chunks: Vec<&[u16]> = if numbers.is_empty() {
        vec![&[]]
    } else {
        numbers.chunks(PER_PART).collect()
    };
    let parts = u16::try_from(chunks.len()).unwrap_or(u16::MAX);
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut out = Vec::with_capacity(HEADER + chunk.len() * 2);
            out.push(VERSION);
            out.extend(stamp.to_le_bytes());
            out.extend((i as u16).to_le_bytes());
            out.extend(parts.to_le_bytes());
            out.extend((chunk.len() as u16).to_le_bytes());
            for n in *chunk {
                out.extend(n.to_le_bytes());
            }
            out
        })
        .collect()
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

/// What `encode` wrote; none for a message cut short, of no known version, or whose part is out
/// of its count.
pub fn decode(bytes: &[u8]) -> Option<Part> {
    if *bytes.first()? < VERSION {
        return None;
    }
    let stamp = u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?);
    let index = u16_at(bytes, 5)?;
    let parts = u16_at(bytes, 7)?;
    let count = usize::from(u16_at(bytes, 9)?);
    if parts == 0 || parts > MAX_PARTS || index >= parts {
        return None;
    }
    let numbers = (0..count)
        .map(|i| u16_at(bytes, HEADER + 2 * i))
        .collect::<Option<Vec<u16>>>()?;
    Some(Part {
        stamp,
        index,
        parts,
        numbers,
    })
}

/// The client's copy of the list: the parts of the stamp being put together, and the last whole
/// list.
#[derive(Clone, Debug, Default)]
pub struct Assembler {
    pending: Option<(u32, Vec<Option<Vec<u16>>>)>,
    whole: Option<(u32, Vec<u16>)>,
}

impl Assembler {
    /// Takes a message; true when it completed a list.
    pub fn take(&mut self, bytes: &[u8]) -> bool {
        let Some(part) = decode(bytes) else {
            return false;
        };
        let parts = usize::from(part.parts);
        let fresh = !matches!(&self.pending, Some((s, p)) if *s == part.stamp && p.len() == parts);
        if fresh {
            self.pending = Some((part.stamp, vec![None; parts]));
        }
        let Some((stamp, slots)) = self.pending.as_mut() else {
            return false;
        };
        slots[usize::from(part.index)] = Some(part.numbers);
        if slots.iter().any(Option::is_none) {
            return false;
        }
        let numbers = slots.iter_mut().flat_map(|s| s.take().unwrap_or_default()).collect();
        self.whole = Some((*stamp, numbers));
        self.pending = None;
        true
    }

    /// The last whole list.
    pub fn list(&self) -> Option<&[u16]> {
        self.whole.as_ref().map(|(_, n)| n.as_slice())
    }

    /// The stamp of the last whole list.
    pub fn stamp(&self) -> Option<u32> {
        self.whole.as_ref().map(|(s, _)| *s)
    }
}

/// The client's list with the whole list of what the book makes in place of whatever the
/// message had room for; a list cut by an older server (OT10) is cut no more.
pub fn complete(listing: &mut Listing, whole: Option<&[u16]>) {
    if let Some(whole) = whole {
        listing.orderable = whole.to_vec();
        listing.orderable_cut = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::wire::{self, Line, Variant};

    #[test]
    fn a_list_comes_back_in_one_part_and_in_many() {
        for n in [0usize, 1, 1485, PER_PART, PER_PART + 1, 40_000] {
            let numbers: Vec<u16> = (0..n).map(|i| (i % 65_000) as u16).collect();
            let messages = encode(&numbers);
            assert_eq!(messages.len(), n.div_ceil(PER_PART).max(1), "{n}");
            assert!(messages.iter().all(|m| m.len() <= MAX_BYTES), "{n}");
            let mut a = Assembler::default();
            // Parts may come in any order; the list is whole after the last.
            for (i, m) in messages.iter().enumerate().rev() {
                assert_eq!(a.take(m), i == 0, "{n} part {i}");
            }
            assert_eq!(a.list(), Some(numbers.as_slice()), "{n}");
            assert_eq!(a.stamp(), Some(stamp(&numbers)));
        }
    }

    #[test]
    fn a_new_list_replaces_the_old_and_broken_messages_change_nothing() {
        let old: Vec<u16> = vec![1, 2, 3];
        let new: Vec<u16> = vec![4, 5];
        assert_ne!(stamp(&old), stamp(&new));
        let mut a = Assembler::default();
        assert!(a.take(&encode(&old)[0]));
        let bytes = &encode(&new)[0];
        assert!(!a.take(&bytes[..bytes.len() - 1]), "cut short");
        assert!(!a.take(&[0, 1, 2]), "no known version");
        let mut bad = bytes.clone();
        bad[5] = 7; // part 7 of 1
        assert!(!a.take(&bad));
        assert_eq!(a.list(), Some(old.as_slice()));
        assert!(a.take(bytes));
        assert_eq!(a.list(), Some(new.as_slice()));
    }

    #[test]
    fn a_newer_version_still_reads() {
        let mut bytes = encode(&[7, 8])[0].clone();
        bytes[0] = 9;
        bytes.extend([1, 2, 3]);
        assert_eq!(decode(&bytes).map(|p| p.numbers), Some(vec![7, 8]));
    }

    /// The owner's storage (0.21.0): about 400 kinds, many of them enchanted books and gear, and
    /// a book of 1485 recipes.
    fn big_storage(kinds: usize, variants: usize, payload: usize) -> Listing {
        Listing {
            book: true,
            lines: (0..kinds)
                .map(|i| Line {
                    item: i as u16,
                    damage: if i < variants { crate::storage::terminal::VARIANT | i as u16 } else { 0 },
                    count: 1 + i as u64,
                    ..Line::default()
                })
                .collect(),
            variants: (0..variants)
                .map(|i| Variant {
                    item: i as u16,
                    index: i as u16,
                    payload: "ab".repeat(payload / 2),
                })
                .collect(),
            orderable: (2000..2000 + 1485).collect(),
            ..Listing::default()
        }
    }

    #[test]
    fn a_big_storage_with_many_enchanted_stacks_still_orders_everything() {
        let l = big_storage(400, 150, 160);
        let bytes = wire::encode(&l);
        assert!(bytes.len() <= MAX_BYTES);
        let sent = wire::decode(&bytes).unwrap();
        assert_eq!(sent.lines.len(), 400, "every stored line goes");
        // The list itself has no room left for what the book makes (the 0.21.0 bug).
        assert!(sent.orderable.len() < l.orderable.len(), "{}", sent.orderable.len());
        // Its own message brings every number, and the window's list has them all.
        let mut a = Assembler::default();
        for m in encode(&l.orderable) {
            a.take(&m);
        }
        let mut shown = sent;
        complete(&mut shown, a.list());
        assert_eq!(shown.orderable, l.orderable);
        assert!(!shown.orderable_cut);
        // Nothing yet: the list stays as it came.
        let mut early = wire::decode(&bytes).unwrap();
        let before = early.clone();
        complete(&mut early, None);
        assert_eq!(early, before);
    }

    /// The numbers for the report: what the lines and the component stacks take of the 30 000.
    #[test]
    fn what_a_big_storage_takes_of_the_message() {
        let mut plain = big_storage(400, 150, 160);
        plain.variants.clear();
        plain.orderable.clear();
        let lines = wire::encode(&plain).len();
        let full = big_storage(400, 150, 160);
        let mut no_book = full.clone();
        no_book.orderable.clear();
        let with_variants = wire::encode(&no_book).len();
        let sent = wire::decode(&wire::encode(&full)).unwrap();
        eprintln!(
            "400 kinds: lines {lines} B, + 150 enchanted stacks of 160 hex = {with_variants} B, \
             variants sent {}/150, orderable sent {}/1485",
            sent.variants.len(),
            sent.orderable.len()
        );
        assert!(lines < MAX_BYTES / 2);
        assert!(with_variants > MAX_BYTES - 2 * 100);
    }
}
