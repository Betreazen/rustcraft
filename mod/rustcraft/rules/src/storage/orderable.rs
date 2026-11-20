//! 0.21.1 (spec 47, OT12): the items the recipe book can make (O2), sent apart from the storage's
//! list. They depend only on the book, not on what is stored, so the server sends them to a player
//! once when a terminal or the tablet opens and again only when the book is built anew; the list
//! (`wire`) then keeps its 30 000 bytes for the stored lines and the component stacks. Before
//! 0.21.1 they rode at the end of the list in whatever room the rest left, and a storage of a few
//! hundred kinds with many enchanted books left none: the search found nothing to order.
//!
//! The numbers go in parts under one stamp (`storage::parts`), and the client puts them together.

use crate::storage::parts;
use crate::storage::wire::{Listing, MAX_BYTES};

/// Server to client: a part of the list of what the recipe book can make.
pub const CHANNEL: &str = "rustcraft:terminal_orderable";
/// Numbers in one part.
pub const PER_PART: usize = (MAX_BYTES - (1 + 4 + 2 + 2 + 2)) / 2;

/// The client's copy of the list.
pub type Assembler = parts::Assembler<u16>;

/// What names one list: its parts carry it, and the server sends a player the list again only when
/// it changed.
pub fn stamp(numbers: &[u16]) -> u32 {
    let len = u32::try_from(numbers.len()).unwrap_or(u32::MAX);
    parts::hash(
        len.to_le_bytes()
            .into_iter()
            .chain(numbers.iter().flat_map(|n| n.to_le_bytes())),
    )
}

/// The messages that carry `numbers`: one for an empty list.
pub fn encode(numbers: &[u16]) -> Vec<Vec<u8>> {
    parts::encode(stamp(numbers), numbers)
}

/// What `encode` wrote; none for a broken message.
pub fn decode(bytes: &[u8]) -> Option<parts::Part<u16>> {
    parts::decode(bytes)
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
        assert_eq!(decode(&bytes).map(|p| p.entries), Some(vec![7, 8]));
    }
}
