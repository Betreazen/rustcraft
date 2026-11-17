//! The portal dock (spec 12.2, PD1–PD4): two docks in two worlds, paired with the wrench. What
//! the net gives one goes to the other through a store of the server's, kept with the world: a
//! dock whose world is not loaded receives it when it is (PD3).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The most stacks on the way to one dock (PD3: its buffer of 27 slots).
pub const MAX_STACKS: usize = 27;
/// Wave 2 (guide `portal_dock`: «Буфер: 27 слотов и 16 вёдер»): the most fluid on the way to one
/// dock, mB.
pub const FLUID_ROOM: u64 = 16_000;
/// Wave 2: a dock not seen stepping for this long (ms) is in a world that is not loaded.
pub const UNLOADED_AFTER_MS: u64 = 3_000;

/// A stack on its way, by item id and count (components are not carried).
pub type Stack = (String, u32);

/// The pairs and what is on the way, by dock id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transit {
    pub pairs: BTreeMap<u64, u64>,
    pub waiting: BTreeMap<u64, Vec<Stack>>,
    /// A dock the wrench clicked first, by player, waiting for its pair.
    pub pending: BTreeMap<u128, u64>,
    /// Wave 2: fluid on the way to each dock, (fluid, mB), at most FLUID_ROOM in all.
    #[serde(default)]
    pub fluids: BTreeMap<u64, Vec<(String, u64)>>,
    /// Wave 2: the world (dimension) each dock was last seen in.
    #[serde(default)]
    pub worlds: BTreeMap<u64, String>,
    /// Wave 2: when each dock last stepped (ms), this server run only.
    #[serde(skip)]
    pub seen: BTreeMap<u64, u64>,
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

    /// Wave 2: room for how much fluid (mB) on the way to `to`.
    pub fn fluid_room(&self, to: u64) -> u64 {
        FLUID_ROOM.saturating_sub(self.fluid_held(to))
    }

    /// Wave 2: how much fluid (mB) is on the way to `to`.
    pub fn fluid_held(&self, to: u64) -> u64 {
        self.fluids.get(&to).map_or(0, |l| l.iter().map(|(_, n)| *n).sum())
    }

    /// Wave 2: sends up to `mb` of `fluid` to `to`, as far as its room goes; how much went.
    pub fn send_fluid(&mut self, to: u64, fluid: &str, mb: u64) -> u64 {
        let fit = mb.min(self.fluid_room(to));
        if fit == 0 {
            return 0;
        }
        let list = self.fluids.entry(to).or_default();
        match list.iter_mut().find(|(f, _)| f == fluid) {
            Some((_, n)) => *n += fit,
            None => list.push((fluid.to_owned(), fit)),
        }
        fit
    }

    /// Wave 2: takes the fluid that waits for `id`.
    pub fn take_fluids(&mut self, id: u64) -> Vec<(String, u64)> {
        self.fluids.remove(&id).unwrap_or_default()
    }

    /// Wave 2: gives back fluid a dock could not unload.
    pub fn put_back_fluids(&mut self, id: u64, left: Vec<(String, u64)>) {
        for (f, n) in left.into_iter().filter(|(_, n)| *n > 0) {
            let list = self.fluids.entry(id).or_default();
            match list.iter_mut().find(|(g, _)| *g == f) {
                Some((_, m)) => *m += n,
                None => list.push((f, n)),
            }
        }
    }

    /// Wave 2: dock `id` stepped at `now_ms` in `world`.
    pub fn saw(&mut self, id: u64, world: &str, now_ms: u64) {
        self.seen.insert(id, now_ms);
        if self.worlds.get(&id).map(String::as_str) != Some(world) {
            self.worlds.insert(id, world.to_owned());
        }
    }

    /// Wave 2: whether dock `id`'s world is loaded: it stepped lately.
    pub fn loaded(&self, id: u64, now_ms: u64) -> bool {
        self.seen.get(&id).is_some_and(|t| now_ms.saturating_sub(*t) <= UNLOADED_AFTER_MS)
    }

    /// Wave 2: whether anything (items or fluid) waits to go to `to`.
    pub fn holds_for(&self, to: u64) -> bool {
        self.waiting.get(&to).is_some_and(|l| !l.is_empty()) || self.fluid_held(to) > 0
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
    fn fluid_goes_to_the_pair_up_to_sixteen_buckets() {
        let mut t = Transit::default();
        assert_eq!(t.send_fluid(7, "minecraft:water", 10_000), 10_000);
        assert_eq!(t.send_fluid(7, "rustcraft:milk", 10_000), 6_000, "16 buckets in all");
        assert_eq!(t.fluid_room(7), 0);
        assert!(t.holds_for(7));
        let got = t.take_fluids(7);
        assert_eq!(got, vec![("minecraft:water".into(), 10_000), ("rustcraft:milk".into(), 6_000)]);
        t.put_back_fluids(7, vec![("rustcraft:milk".into(), 1_000), ("minecraft:water".into(), 0)]);
        assert_eq!(t.fluid_held(7), 1_000);
    }

    #[test]
    fn a_dock_not_seen_lately_is_in_an_unloaded_world() {
        let mut t = Transit::default();
        assert!(!t.loaded(5, 1_000), "never seen");
        t.saw(5, "minecraft:the_nether", 1_000);
        assert!(t.loaded(5, 2_000));
        assert!(!t.loaded(5, 1_000 + UNLOADED_AFTER_MS + 1));
        assert_eq!(t.worlds.get(&5).map(String::as_str), Some("minecraft:the_nether"));
        // The store read back from the world keeps the worlds and the fluid, not the times.
        let json = serde_json::to_string(&t).unwrap();
        let back: Transit = serde_json::from_str(&json).unwrap();
        assert!(back.seen.is_empty());
        assert_eq!(back.worlds, t.worlds);
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
