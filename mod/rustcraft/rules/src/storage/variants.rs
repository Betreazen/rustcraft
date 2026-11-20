//! 0.21.1 (spec 47, OT13): the stored stacks with components (slice 18: enchanted books and gear,
//! named stacks, potions), sent apart from the storage's list. The list (`wire`) carries each
//! such stack as a line naming its place among its item's, and the stack's saved form in a tail
//! section in whatever room the lines left; a storage of a few hundred kinds with many enchanted
//! stacks left too little, and the stacks past it lost their names and enchantments in the window.
//!
//! The server sends a window's player the whole list of them in parts under one stamp
//! (`storage::parts`, a hash of the list) when the window opens and again whenever the stacks
//! change, before the list; the list names the stamp (`Listing::variants_stamp`) and still keeps
//! what room is left of them, for a client older than 0.21.1. The client takes the whole list in
//! place of the list's own only when the stamps agree, so a list never meets stacks of another.

use crate::storage::parts::{self, Entry, u16_from};
use crate::storage::wire::{Listing, Variant};

/// Server to client: a part of a terminal's component stacks.
pub const CHANNEL: &str = "rustcraft:terminal_variants";
/// Server to client: a part of the storage tablet's component stacks.
pub const TABLET_CHANNEL: &str = "rustcraft:tablet_variants";

impl Entry for Variant {
    fn size(&self) -> usize {
        6 + self.payload.len()
    }
    fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.item.to_le_bytes());
        out.extend(self.index.to_le_bytes());
        out.extend((self.payload.len() as u16).to_le_bytes());
        out.extend(self.payload.as_bytes());
    }
    fn read(bytes: &mut &[u8]) -> Option<Self> {
        let item = u16_from(bytes)?;
        let index = u16_from(bytes)?;
        let len = usize::from(u16_from(bytes)?);
        let (text, rest) = bytes.split_at_checked(len)?;
        *bytes = rest;
        Some(Variant {
            item,
            index,
            payload: String::from_utf8_lossy(text).into_owned(),
        })
    }
}

/// The client's copy of a window's stacks.
pub type Assembler = parts::Assembler<Variant>;

/// What names one list of stacks.
pub fn stamp(variants: &[Variant]) -> u32 {
    let mut bytes = (variants.len() as u32).to_le_bytes().to_vec();
    for v in variants {
        v.write(&mut bytes);
    }
    parts::hash(bytes)
}

/// The messages that carry `variants`.
pub fn encode(variants: &[Variant]) -> Vec<Vec<u8>> {
    parts::encode(stamp(variants), variants)
}

/// The client's list with the whole list of stacks in place of what the list had room for, when
/// the list names that list's stamp.
pub fn complete(listing: &mut Listing, whole: &Assembler) {
    if let (Some(want), Some(have), Some(list)) = (listing.variants_stamp, whole.stamp(), whole.list())
        && want == have
    {
        listing.variants = list.to_vec();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::orderable;
    use crate::storage::terminal::VARIANT;
    use crate::storage::wire::{self, Line, MAX_BYTES};

    fn variant(i: usize, payload: usize) -> Variant {
        Variant {
            item: i as u16,
            index: i as u16,
            payload: format!("{i:04x}").repeat(payload / 4),
        }
    }

    #[test]
    fn stacks_come_back_whole_in_parts() {
        let all: Vec<Variant> = (0..600).map(|i| variant(i, 2048)).collect();
        let messages = encode(&all);
        assert!(messages.len() > 1);
        assert!(messages.iter().all(|m| m.len() <= MAX_BYTES));
        let mut a = Assembler::default();
        for m in messages.iter().rev() {
            a.take(m);
        }
        assert_eq!(a.list(), Some(all.as_slice()));
        assert_eq!(a.stamp(), Some(stamp(&all)));
        assert_ne!(stamp(&all[..599]), stamp(&all));
    }

    #[test]
    fn a_list_takes_only_stacks_of_its_own_stamp() {
        let all: Vec<Variant> = (0..3).map(|i| variant(i, 40)).collect();
        let mut a = Assembler::default();
        a.take(&encode(&all)[0]);
        let mut other = Listing {
            variants_stamp: Some(stamp(&all) ^ 1),
            variants: vec![variant(9, 8)],
            ..Listing::default()
        };
        complete(&mut other, &a);
        assert_eq!(other.variants, vec![variant(9, 8)], "another list's stamp");
        let mut old = Listing {
            variants: vec![variant(9, 8)],
            ..Listing::default()
        };
        complete(&mut old, &a);
        assert_eq!(old.variants.len(), 1, "an older server names no stamp");
        let mut mine = Listing {
            variants_stamp: Some(stamp(&all)),
            ..Listing::default()
        };
        complete(&mut mine, &a);
        assert_eq!(mine.variants, all);
    }

    /// The owner's storage (0.21.0): about 400 kinds, many of them enchanted books and gear, and
    /// a book of 1485 recipes.
    fn big_storage(kinds: usize, variants: usize, payload: usize) -> Listing {
        let all: Vec<Variant> = (0..variants).map(|i| variant(i, payload)).collect();
        Listing {
            book: true,
            lines: (0..kinds)
                .map(|i| Line {
                    item: i as u16,
                    damage: if i < variants { VARIANT | i as u16 } else { 0 },
                    count: 1 + i as u64,
                    ..Line::default()
                })
                .collect(),
            variants_stamp: Some(stamp(&all)),
            variants: all,
            orderable: (2000..2000 + 1485).collect(),
            ..Listing::default()
        }
    }

    #[test]
    fn a_big_storage_with_many_enchanted_stacks_still_shows_and_orders_everything() {
        let l = big_storage(400, 150, 160);
        let bytes = wire::encode(&l);
        assert!(bytes.len() <= MAX_BYTES);
        let sent = wire::decode(&bytes).unwrap();
        assert_eq!(sent.lines.len(), 400, "every stored line goes");
        // The list itself has no room for every stack nor what the book makes (the 0.21.0 bug).
        assert!(sent.variants.len() < l.variants.len(), "{}", sent.variants.len());
        assert!(sent.orderable.len() < l.orderable.len(), "{}", sent.orderable.len());
        assert_eq!(sent.variants_stamp, l.variants_stamp);
        // Their own messages bring every one, and the window's list has them all.
        let mut stacks = Assembler::default();
        for m in encode(&l.variants) {
            assert!(m.len() <= MAX_BYTES);
            stacks.take(&m);
        }
        let mut items = orderable::Assembler::default();
        for m in orderable::encode(&l.orderable) {
            items.take(&m);
        }
        let mut shown = sent;
        complete(&mut shown, &stacks);
        orderable::complete(&mut shown, items.list());
        assert_eq!(shown.variants, l.variants);
        assert_eq!(shown.orderable, l.orderable);
        assert!(!shown.orderable_cut);
        // Every line of a stack with components has its stack.
        for line in shown.lines.iter().filter(|x| x.damage & VARIANT != 0) {
            let index = line.damage & !VARIANT;
            assert!(shown.variants.iter().any(|v| v.item == line.item && v.index == index));
        }
        // Nothing yet: the list stays as it came.
        let mut early = wire::decode(&bytes).unwrap();
        let before = early.clone();
        complete(&mut early, &Assembler::default());
        orderable::complete(&mut early, None);
        assert_eq!(early, before);
    }

    /// The numbers for the report: what the lines and the component stacks take of the 30 000.
    #[test]
    fn what_a_big_storage_takes_of_the_message() {
        let mut plain = big_storage(400, 150, 160);
        plain.variants.clear();
        plain.orderable.clear();
        plain.variants_stamp = None;
        let lines = wire::encode(&plain).len();
        let full = big_storage(400, 150, 160);
        let mut no_book = full.clone();
        no_book.orderable.clear();
        let with_variants = wire::encode(&no_book).len();
        let sent = wire::decode(&wire::encode(&full)).unwrap();
        let apart: usize = encode(&full.variants).iter().map(Vec::len).sum();
        eprintln!(
            "400 kinds: lines {lines} B, + 150 enchanted stacks of 160 hex = {with_variants} B, \
             variants in the list {}/150, orderable in the list {}/1485; \
             the stacks apart: {apart} B in {} part(s)",
            sent.variants.len(),
            sent.orderable.len(),
            encode(&full.variants).len()
        );
        assert!(lines < MAX_BYTES / 2);
        assert!(with_variants > MAX_BYTES - 2 * 100);
    }
}
