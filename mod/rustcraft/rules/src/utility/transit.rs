//! The portal dock (spec 12.2, PD1–PD4): two docks in two worlds, paired with the wrench. What
//! the net gives one goes to the other through a store of the server's, kept with the world: a
//! dock whose world is not loaded receives it when it is (PD3).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The most stacks on the way to one dock (PD3: its buffer of 27 slots).
pub const MAX_STACKS: usize = 27;

/// A stack on its way, by item id and count (components are not carried).
pub type Stack = (String, u32);

/// The pairs and what is on the way, by dock id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transit {
    pub pairs: BTreeMap<u64, u64>,
    pub waiting: BTreeMap<u64, Vec<Stack>>,
    /// A dock the wrench clicked first, by player, waiting for its pair.
    pub pending: BTreeMap<u128, u64>,
}

impl Transit {
    /// A wrench click on dock `id` by `player`: the first click remembers it, the second pairs it
    /// with the first (any earlier pairs of both are undone). Some(other) when they paired.
    pub fn click(&mut self, player: u128, id: u64) -> Option<u64> {
        match self.pending.remove(&player) {
            Some(first) if first != id => {
                for d in [first, id] {
                    if let Some(old) = self.pairs.remove(&d) {
                        self.pairs.remove(&old);
                    }
                }
                self.pairs.insert(first, id);
                self.pairs.insert(id, first);
                Some(first)
            }
            _ => {
                self.pending.insert(player, id);
                None
            }
        }
    }

    pub fn partner(&self, id: u64) -> Option<u64> {
        self.pairs.get(&id).copied()
    }

    /// Room for how many of `item` on the way to `to`, stacks of `max` each.
    pub fn room(&self, to: u64, item: &str, max: u32) -> u32 {
        let list = self.waiting.get(&to).map_or(&[][..], Vec::as_slice);
        let partial: u32 = list.iter().filter(|(i, _)| i == item).map(|(_, n)| max.saturating_sub(*n)).sum();
        partial + (MAX_STACKS.saturating_sub(list.len()) as u32) * max
    }

    /// Sends `count` of `item` to `to`, filling its partial stacks first; returns how many went.
    pub fn send(&mut self, to: u64, item: &str, count: u32, max: u32) -> u32 {
        let fit = count.min(self.room(to, item, max));
        let list = self.waiting.entry(to).or_default();
        let mut left = fit;
        for (i, n) in list.iter_mut() {
            if i == item && *n < max && left > 0 {
                let add = left.min(max - *n);
                *n += add;
                left -= add;
            }
        }
        while left > 0 {
            let add = left.min(max);
            list.push((item.to_owned(), add));
            left -= add;
        }
        fit
    }

    /// Takes what waits for `id`.
    pub fn take(&mut self, id: u64) -> Vec<Stack> {
        self.waiting.remove(&id).unwrap_or_default()
    }

    /// Gives back stacks a dock could not take, ahead of the rest.
    pub fn put_back(&mut self, id: u64, mut stacks: Vec<Stack>) {
        if stacks.is_empty() {
            return;
        }
        let list = self.waiting.entry(id).or_default();
        stacks.append(list);
        *list = stacks;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_clicks_pair_and_a_new_pair_undoes_the_old() {
        let mut t = Transit::default();
        assert_eq!(t.click(1, 10), None);
        assert_eq!(t.click(1, 20), Some(10));
        assert_eq!((t.partner(10), t.partner(20)), (Some(20), Some(10)));
        t.click(1, 30);
        t.click(1, 10);
        assert_eq!(t.partner(10), Some(30));
        assert_eq!(t.partner(20), None);
    }

    #[test]
    fn what_is_sent_waits_in_stacks_up_to_the_buffer() {
        let mut t = Transit::default();
        assert_eq!(t.send(7, "minecraft:dirt", 100, 64), 100);
        assert_eq!(t.waiting[&7], vec![("minecraft:dirt".into(), 64), ("minecraft:dirt".into(), 36)]);
        assert_eq!(t.room(7, "minecraft:dirt", 64), 28 + 25 * 64);
        let got = t.take(7);
        assert_eq!(got.len(), 2);
        t.put_back(7, vec![("minecraft:stone".into(), 1)]);
        assert_eq!(t.take(7).len(), 1);
    }
}
