//! The worker's hold (spec 4.7: «трюм»): slots of plain items, filled by its tool and emptied at
//! an unload dock. Items keep no components: what a saw or a reaper breaks has none.

use serde::{Deserialize, Serialize};

/// One slot: an item and a count.
pub type Stack = (String, u32);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hold {
    pub slots: Vec<Option<Stack>>,
}

impl Hold {
    pub fn new(size: usize) -> Self {
        Hold {
            slots: vec![None; size],
        }
    }

    /// The hold with `size` slots: stacks past the end come back to be dropped.
    pub fn resized(mut self, size: usize) -> (Hold, Vec<Stack>) {
        let mut out = Vec::new();
        if self.slots.len() > size {
            out = self.slots.split_off(size).into_iter().flatten().collect();
        }
        self.slots.resize(size, None);
        (self, out)
    }

    /// Puts `count` of `item` in, onto its own stacks first, then into empty slots; what did not
    /// fit.
    pub fn add(&mut self, item: &str, mut count: u32, max_stack: u32) -> u32 {
        let max = max_stack.max(1);
        for slot in self.slots.iter_mut().flatten() {
            if count == 0 {
                break;
            }
            if slot.0 == item && slot.1 < max {
                let n = (max - slot.1).min(count);
                slot.1 += n;
                count -= n;
            }
        }
        for slot in &mut self.slots {
            if count == 0 {
                break;
            }
            if slot.is_none() {
                let n = max.min(count);
                *slot = Some((item.to_owned(), n));
                count -= n;
            }
        }
        count
    }

    /// Takes up to `count` of `item` out; how many it took.
    pub fn take(&mut self, item: &str, count: u32) -> u32 {
        let mut took = 0;
        for slot in self.slots.iter_mut().rev() {
            if took == count {
                break;
            }
            if let Some((i, n)) = slot
                && i == item
            {
                let k = (*n).min(count - took);
                *n -= k;
                took += k;
                if *n == 0 {
                    *slot = None;
                }
            }
        }
        took
    }

    pub fn count(&self, item: &str) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|(i, _)| i == item)
            .map(|(_, n)| n)
            .sum()
    }

    /// The first item matching `pick`.
    pub fn find(&self, pick: impl Fn(&str) -> bool) -> Option<String> {
        self.slots
            .iter()
            .flatten()
            .find(|(i, _)| pick(i))
            .map(|(i, _)| i.clone())
    }

    pub fn used(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// W4.a: no empty slot left.
    pub fn is_full(&self) -> bool {
        self.slots.iter().all(Option::is_some)
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }

    /// The slot an unload dock takes next: the last slot with something that `keep` does not
    /// keep. The first stack of each kept item stays for the tool (saplings, seeds).
    pub fn next_to_unload(&self, keep: impl Fn(&str) -> bool) -> Option<usize> {
        let mut first_kept: Vec<&str> = Vec::new();
        let mut keep_at = vec![false; self.slots.len()];
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some((item, _)) = slot
                && keep(item)
                && !first_kept.contains(&item.as_str())
            {
                first_kept.push(item);
                keep_at[i] = true;
            }
        }
        (0..self.slots.len())
            .rev()
            .find(|&i| self.slots[i].is_some() && !keep_at[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_go_onto_their_stacks_then_into_empty_slots() {
        let mut h = Hold::new(2);
        assert_eq!(h.add("a", 60, 64), 0);
        assert_eq!(h.add("a", 10, 64), 0);
        assert_eq!(h.slots[0], Some(("a".into(), 64)));
        assert_eq!(h.slots[1], Some(("a".into(), 6)));
        assert_eq!(h.add("b", 5, 64), 5);
        assert!(h.is_full());
        assert_eq!(h.take("a", 7), 7);
        assert_eq!(h.count("a"), 63);
    }

    #[test]
    fn the_unload_dock_leaves_one_stack_of_seeds() {
        let mut h = Hold::new(4);
        h.add("seed", 10, 64);
        h.add("wheat", 64, 64);
        h.add("seed", 64, 64);
        let keep = |i: &str| i == "seed";
        let mut taken = Vec::new();
        while let Some(i) = h.next_to_unload(keep) {
            taken.push(h.slots[i].take().unwrap());
        }
        assert_eq!(h.count("seed"), 64);
        assert_eq!(taken.len(), 2);
    }

    #[test]
    fn a_smaller_hold_gives_back_what_no_longer_fits() {
        let mut h = Hold::new(3);
        h.slots[2] = Some(("x".into(), 3));
        let (h, out) = h.resized(2);
        assert_eq!(h.slots.len(), 2);
        assert_eq!(out, vec![("x".to_owned(), 3)]);
    }
}
