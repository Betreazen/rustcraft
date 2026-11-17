//! Slice 18, O11 (spec 19: «промежуточный предмет идёт прямо в следующую машину заказа, если она
//! на той же магистрали»): a product a step made for a later step of the same order does not ride
//! to the storage. It leaves the machine's output slots into a record «held for order X at
//! machine A» and, when the next step's job starts, rides from machine A straight to that job's
//! machine (the bundle's `deliver`).
//!
//! Held items are the order's own stock for its rounds (`queue::Queue::round_with`) and no other
//! order's; they are not in the storage's counts, so no player takes them. When the order is
//! cancelled or done, or its plan no longer needs them, they go to the storage (`release`), so
//! nothing is lost or made twice. The storage's record keeps them over a restart (`saved`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geom::Pos;
use crate::net::items::ItemKey;

/// Items of one kind a step of an order made, waiting at the machine that made them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    /// The storage the order is of (the record keeps it per storage, so it is read back as 0).
    #[serde(default, skip_serializing)]
    pub storage: u32,
    pub order: u16,
    pub machine: Pos,
    pub item: ItemKey,
    pub count: u64,
}

/// `n` of `item` made by the machine at `machine` for the order `order` of `storage` wait there.
pub fn add(held: &mut Vec<Held>, storage: u32, order: u16, machine: Pos, item: &str, n: u64) {
    if n == 0 {
        return;
    }
    if let Some(h) = held
        .iter_mut()
        .find(|h| h.storage == storage && h.order == order && h.machine == machine && h.item == item)
    {
        h.count += n;
        return;
    }
    held.push(Held {
        storage,
        order,
        machine,
        item: item.to_owned(),
        count: n,
    });
}

/// What the storage's orders hold at machines, order by order.
pub fn by_order(held: &[Held], storage: u32) -> BTreeMap<u16, BTreeMap<ItemKey, u64>> {
    let mut out: BTreeMap<u16, BTreeMap<ItemKey, u64>> = BTreeMap::new();
    for h in held.iter().filter(|h| h.storage == storage) {
        *out.entry(h.order).or_default().entry(h.item.clone()).or_insert(0) += h.count;
    }
    out
}

/// Up to `n` of `item` the order holds, taken out of the record: from which machines, how many.
pub fn take(held: &mut Vec<Held>, storage: u32, order: u16, item: &str, n: u64) -> Vec<(Pos, u64)> {
    let mut left = n;
    let mut out = Vec::new();
    for h in held
        .iter_mut()
        .filter(|h| h.storage == storage && h.order == order && h.item == item)
    {
        if left == 0 {
            break;
        }
        let part = h.count.min(left);
        h.count -= part;
        left -= part;
        out.push((h.machine, part));
    }
    held.retain(|h| h.count > 0);
    out
}

/// What the storage's orders hold beyond what each still needs, taken out of the record to go to
/// the storage: all of an order `need` gives None for (it is gone), and of an order with a need
/// whatever is beyond it, item by item. The order's held items its round did not look at
/// (`need` gives `Some(None)`) stay.
pub fn release(
    held: &mut Vec<Held>,
    storage: u32,
    need: &dyn Fn(u16) -> Option<Option<BTreeMap<ItemKey, u64>>>,
) -> Vec<Held> {
    let mut out = Vec::new();
    let mut kept_of: BTreeMap<(u16, ItemKey), u64> = BTreeMap::new();
    for h in std::mem::take(held) {
        if h.storage != storage {
            held.push(h);
            continue;
        }
        let keep = match need(h.order) {
            None => 0,
            Some(None) => u64::MAX,
            Some(Some(map)) => {
                let wanted = map.get(&h.item).copied().unwrap_or(0);
                let kept = kept_of.entry((h.order, h.item.clone())).or_insert(0);
                let here = wanted.saturating_sub(*kept).min(h.count);
                *kept += here;
                here
            }
        };
        let stay = keep.min(h.count);
        if stay < h.count {
            out.push(Held {
                count: h.count - stay,
                ..h.clone()
            });
        }
        if stay > 0 {
            held.push(Held { count: stay, ..h });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const IRON: &str = "minecraft:iron_ingot";
    const GOLD: &str = "minecraft:gold_ingot";
    const A: Pos = [1, 64, 0];
    const B: Pos = [2, 64, 0];

    #[test]
    fn products_of_one_order_at_one_machine_add_up() {
        let mut held = Vec::new();
        add(&mut held, 3, 7, A, IRON, 2);
        add(&mut held, 3, 7, A, IRON, 3);
        add(&mut held, 3, 7, B, IRON, 1);
        add(&mut held, 3, 8, A, IRON, 4);
        add(&mut held, 4, 7, A, IRON, 5);
        add(&mut held, 3, 7, A, GOLD, 0);
        assert_eq!(held.len(), 4);
        assert_eq!(held[0].count, 5);
        let mine = by_order(&held, 3);
        assert_eq!(mine[&7], BTreeMap::from([(IRON.to_string(), 6)]));
        assert_eq!(mine[&8], BTreeMap::from([(IRON.to_string(), 4)]));
        assert_eq!(mine.len(), 2);
    }

    #[test]
    fn taking_spans_machines_and_never_more_than_held() {
        let mut held = Vec::new();
        add(&mut held, 3, 7, A, IRON, 2);
        add(&mut held, 3, 7, B, IRON, 3);
        add(&mut held, 3, 8, A, IRON, 9);
        assert_eq!(take(&mut held, 3, 7, IRON, 4), vec![(A, 2), (B, 2)]);
        assert_eq!(by_order(&held, 3)[&7][IRON], 1);
        assert_eq!(take(&mut held, 3, 7, IRON, 10), vec![(B, 1)]);
        assert!(take(&mut held, 3, 7, IRON, 1).is_empty());
        // The other order's are untouched.
        assert_eq!(by_order(&held, 3)[&8][IRON], 9);
        assert_eq!(held.len(), 1);
    }

    #[test]
    fn a_gone_order_lets_all_go_and_a_planned_one_what_it_no_longer_needs() {
        let mut held = Vec::new();
        add(&mut held, 3, 7, A, IRON, 2);
        add(&mut held, 3, 7, B, IRON, 3);
        add(&mut held, 3, 7, B, GOLD, 1);
        add(&mut held, 3, 8, A, IRON, 4);
        add(&mut held, 3, 9, A, IRON, 6);
        add(&mut held, 5, 8, A, IRON, 1);
        let need = |order: u16| match order {
            // Order 7 still needs 4 iron and no gold.
            7 => Some(Some(BTreeMap::from([(IRON.to_string(), 4)]))),
            // Order 9 ran this round: nothing judged.
            9 => Some(None),
            _ => None,
        };
        let out = release(&mut held, 3, &need);
        let total = |v: &[Held], order: u16, item: &str| -> u64 {
            v.iter()
                .filter(|h| h.order == order && h.item == item && h.storage == 3)
                .map(|h| h.count)
                .sum()
        };
        assert_eq!(total(&out, 7, IRON), 1);
        assert_eq!(total(&out, 7, GOLD), 1);
        assert_eq!(total(&out, 8, IRON), 4);
        assert_eq!(total(&out, 9, IRON), 0);
        assert_eq!(total(&held, 7, IRON), 4);
        assert_eq!(total(&held, 9, IRON), 6);
        // Another storage's are not this round's.
        assert!(held.iter().any(|h| h.storage == 5 && h.count == 1));
        // Nothing lost, nothing made: what went and what stayed is what was.
        let all: u64 = out.iter().chain(&held).map(|h| h.count).sum();
        assert_eq!(all, 2 + 3 + 1 + 4 + 6 + 1);
    }
}
