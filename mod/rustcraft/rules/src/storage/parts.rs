//! 0.21.1 (spec 47, OT12, OT13): a list sent apart from the storage's list (`wire`), in parts
//! of at most `wire::MAX_BYTES` under one stamp: the items the recipe book makes
//! (`storage::orderable`) and the stored stacks with components (`storage::variants`).
//!
//! A message is `version, stamp (u32), part (u16), parts (u16), count (u16), count entries`; the
//! client puts the parts of one stamp together (`Assembler`). A reader takes a newer version's
//! fields it knows and skips what follows them, so an older client reads a newer server's message.

use crate::storage::wire::MAX_BYTES;

const VERSION: u8 = 1;
const HEADER: usize = 1 + 4 + 2 + 2 + 2;
/// The most parts a list is read in: past it a message is taken as broken.
pub const MAX_PARTS: u16 = 64;

/// An entry of a list sent in parts.
pub trait Entry: Sized + Clone {
    /// Its bytes in a message.
    fn size(&self) -> usize;
    fn write(&self, out: &mut Vec<u8>);
    /// One entry from the front of `bytes`, which move past it.
    fn read(bytes: &mut &[u8]) -> Option<Self>;
}

pub(crate) fn u16_from(bytes: &mut &[u8]) -> Option<u16> {
    let (head, rest) = bytes.split_first_chunk::<2>()?;
    *bytes = rest;
    Some(u16::from_le_bytes(*head))
}

impl Entry for u16 {
    fn size(&self) -> usize {
        2
    }
    fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.to_le_bytes());
    }
    fn read(bytes: &mut &[u8]) -> Option<Self> {
        u16_from(bytes)
    }
}

/// FNV-1a over bytes: what names one list.
pub fn hash(bytes: impl IntoIterator<Item = u8>) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in bytes {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// One message of a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part<T> {
    pub stamp: u32,
    pub index: u16,
    pub parts: u16,
    pub entries: Vec<T>,
}

/// The messages that carry `entries` under `stamp`, each at most MAX_BYTES: one for an empty
/// list. An entry larger than a message's room goes alone (no entry the storage keeps is).
pub fn encode<T: Entry>(stamp: u32, entries: &[T]) -> Vec<Vec<u8>> {
    let room = MAX_BYTES - HEADER;
    let mut chunks: Vec<&[T]> = Vec::new();
    let (mut start, mut used) = (0, 0);
    for (i, e) in entries.iter().enumerate() {
        if used + e.size() > room && i > start {
            chunks.push(&entries[start..i]);
            (start, used) = (i, 0);
        }
        used += e.size();
    }
    if start < entries.len() || chunks.is_empty() {
        chunks.push(&entries[start..]);
    }
    let parts = u16::try_from(chunks.len()).unwrap_or(u16::MAX);
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut out = Vec::with_capacity(HEADER + chunk.iter().map(Entry::size).sum::<usize>());
            out.push(VERSION);
            out.extend(stamp.to_le_bytes());
            out.extend((i as u16).to_le_bytes());
            out.extend(parts.to_le_bytes());
            out.extend((chunk.len() as u16).to_le_bytes());
            for e in *chunk {
                e.write(&mut out);
            }
            out
        })
        .collect()
}

/// What `encode` wrote; none for a message cut short, of no known version, or whose part is out
/// of its count.
pub fn decode<T: Entry>(bytes: &[u8]) -> Option<Part<T>> {
    let (&version, mut r) = bytes.split_first()?;
    if version < VERSION {
        return None;
    }
    let (stamp, rest) = r.split_first_chunk::<4>()?;
    let stamp = u32::from_le_bytes(*stamp);
    r = rest;
    let index = u16_from(&mut r)?;
    let parts = u16_from(&mut r)?;
    let count = u16_from(&mut r)?;
    if parts == 0 || parts > MAX_PARTS || index >= parts {
        return None;
    }
    let entries = (0..count).map(|_| T::read(&mut r)).collect::<Option<Vec<T>>>()?;
    Some(Part {
        stamp,
        index,
        parts,
        entries,
    })
}

/// The client's copy of a list: the parts of the stamp being put together, and the last whole
/// list.
#[derive(Clone, Debug)]
pub struct Assembler<T> {
    pending: Option<(u32, Vec<Option<Vec<T>>>)>,
    whole: Option<(u32, Vec<T>)>,
}

impl<T> Default for Assembler<T> {
    fn default() -> Self {
        Assembler {
            pending: None,
            whole: None,
        }
    }
}

impl<T: Entry> Assembler<T> {
    /// Takes a message; true when it completed a list.
    pub fn take(&mut self, bytes: &[u8]) -> bool {
        let Some(part) = decode::<T>(bytes) else {
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
        slots[usize::from(part.index)] = Some(part.entries);
        if slots.iter().any(Option::is_none) {
            return false;
        }
        let entries = slots.iter_mut().flat_map(|s| s.take().unwrap_or_default()).collect();
        self.whole = Some((*stamp, entries));
        self.pending = None;
        true
    }

    /// The last whole list.
    pub fn list(&self) -> Option<&[T]> {
        self.whole.as_ref().map(|(_, n)| n.as_slice())
    }

    /// The stamp of the last whole list.
    pub fn stamp(&self) -> Option<u32> {
        self.whole.as_ref().map(|(s, _)| *s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_never_pass_a_message_and_come_back_whole() {
        let numbers: Vec<u16> = (0..40_000u32).map(|i| i as u16).collect();
        let messages = encode(7, &numbers);
        assert!(messages.iter().all(|m| m.len() <= MAX_BYTES));
        let mut a = Assembler::<u16>::default();
        for m in messages.iter().rev() {
            a.take(m);
        }
        assert_eq!(a.list(), Some(numbers.as_slice()));
        assert_eq!(a.stamp(), Some(7));
    }
}
