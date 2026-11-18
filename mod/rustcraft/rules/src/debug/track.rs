//! The status-change tracker: the last status seen of each machine (or worker), so the journal
//! gets a line only when one changes. The first sight of a thing is its baseline, not a change:
//! a restart does not write every machine of the world.

use std::collections::{BTreeMap, BTreeSet};

/// What an observation found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seen {
    /// Not seen before: noted, no event.
    New,
    Same,
    /// The status it had before.
    Changed(String),
}

/// The last status of each thing, by its key.
#[derive(Clone, Debug)]
pub struct Tracker<K: Ord + Clone> {
    last: BTreeMap<K, String>,
}

impl<K: Ord + Clone> Default for Tracker<K> {
    fn default() -> Self {
        Tracker {
            last: BTreeMap::new(),
        }
    }
}

impl<K: Ord + Clone> Tracker<K> {
    /// `status` of `key` noted.
    pub fn observe(&mut self, key: K, status: &str) -> Seen {
        match self.last.insert(key, status.to_owned()) {
            None => Seen::New,
            Some(old) if old == status => Seen::Same,
            Some(old) => Seen::Changed(old),
        }
    }

    /// The last status noted of `key`.
    pub fn last(&self, key: &K) -> Option<&str> {
        self.last.get(key).map(String::as_str)
    }

    /// Only the keys `keep` says yes to stay; the ones dropped, in order.
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) -> Vec<K> {
        let gone: Vec<K> = self.last.keys().filter(|k| !keep(k)).cloned().collect();
        for k in &gone {
            self.last.remove(k);
        }
        gone
    }

    pub fn len(&self) -> usize {
        self.last.len()
    }

    pub fn is_empty(&self) -> bool {
        self.last.is_empty()
    }
}

/// A round of a big set a few at a time: the next `batch` after `cursor`, wrapping round, and
/// the cursor for the next round. Machines are looked at this way, so a large base costs the
/// same each second.
pub fn next_batch<K: Ord + Clone>(all: &BTreeSet<K>, cursor: Option<&K>, batch: usize) -> (Vec<K>, Option<K>) {
    if all.is_empty() || batch == 0 {
        return (Vec::new(), None);
    }
    let after: Vec<K> = match cursor {
        Some(c) => all
            .range((std::ops::Bound::Excluded(c.clone()), std::ops::Bound::Unbounded))
            .take(batch)
            .cloned()
            .collect(),
        None => all.iter().take(batch).cloned().collect(),
    };
    let mut out = after;
    if out.len() < batch {
        // Wrap round to the start, without taking any twice.
        let first_taken = out.first().cloned();
        for k in all.iter() {
            if out.len() >= batch || Some(k) == first_taken.as_ref() || out.contains(k) {
                break;
            }
            out.push(k.clone());
        }
    }
    let next = out.last().cloned();
    (out, next)
}

/// What `now` has that `before` had not, and what `before` had that `now` has not.
pub fn set_diff<T: Ord + Clone>(before: &BTreeSet<T>, now: &BTreeSet<T>) -> (Vec<T>, Vec<T>) {
    (
        now.difference(before).cloned().collect(),
        before.difference(now).cloned().collect(),
    )
}

/// An order as the journal compares it from one second to the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSeen {
    pub item: String,
    pub count: u32,
    pub done: u32,
    /// What it waits for, as text.
    pub wait: String,
}

/// The journal's events between two looks at a level's orders (keyed by storage and order id):
/// an order started, its wait changed, finished (gone with its count made) or gone otherwise
/// (cancelled, or a «держать» order no longer needed). Each is (key, kind, details).
pub fn order_events(
    before: &BTreeMap<(u32, u16), OrderSeen>,
    now: &BTreeMap<(u32, u16), OrderSeen>,
) -> Vec<((u32, u16), &'static str, String)> {
    let name = |k: &(u32, u16), o: &OrderSeen| {
        format!("storage#{} order#{} {} {}/{}", k.0, k.1, o.item, o.done, o.count)
    };
    let mut out = Vec::new();
    for (k, o) in now {
        match before.get(k) {
            None => out.push((*k, "order_started", format!("{} {}", name(k, o), o.wait))),
            Some(old) if old.wait != o.wait => out.push((
                *k,
                "order_wait",
                format!("{} {} -> {}", name(k, o), old.wait, o.wait),
            )),
            Some(_) => {}
        }
    }
    for (k, o) in before {
        if !now.contains_key(k) {
            let kind = if o.done >= o.count { "order_finished" } else { "order_gone" };
            out.push((*k, kind, name(k, o)));
        }
    }
    out
}
