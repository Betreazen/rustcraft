//! A snapshot of the inventories behind an item net's joints, for `items::tick` to plan on. The
//! bundle reads each block's slots once, the net decides on the snapshot, and the bundle then
//! makes the moves the net decided through the game's own item mover, which refuses what does not
//! fit; so a snapshot that is slightly off moves less, never loses an item.
//!
//! A storage on the net (S1) is no block with slots: it keeps counts (`storage::Store`), takes
//! what it has room for under its limits (S5) and gives everything it holds. While its shape is
//! broken (S8) it does neither.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::geom::Pos;
use crate::net::items::{Inventories, ItemKey};
use crate::storage::{Store, keys};

/// The stack size of an item the snapshot was not told about.
pub const DEFAULT_STACK: u32 = 64;

/// One slot as a face of its block reaches it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StockSlot {
    pub item: Option<ItemKey>,
    pub count: u32,
    /// The net may take this slot's items (a chest's slot, a machine's product).
    pub can_take: bool,
    /// The net may put items into this slot (a chest's slot, a machine's input).
    pub can_put: bool,
    /// The only items this slot takes (an alloy smelter's fuel slot takes fuel); any when none.
    pub only: Option<BTreeSet<ItemKey>>,
}

impl StockSlot {
    /// Whether the net may put `item` into this slot.
    fn takes(&self, item: &str) -> bool {
        self.can_put && self.only.as_ref().is_none_or(|o| o.contains(item))
    }
}

/// One block's slots and, for a machine, the only items it takes (M17: an electric furnace
/// takes only what it can smelt). `None` takes anything.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stock {
    pub slots: Vec<StockSlot>,
    pub accepts: Option<BTreeSet<ItemKey>>,
}

/// A storage behind a joint, as the net sees it this tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetStore {
    pub store: Store,
    /// Its cells are a solid box (S8): it takes and gives.
    pub works: bool,
}

/// Every block of one net's joints, and the stack sizes of the items that may move.
#[derive(Clone, Debug, Default)]
pub struct Stocks {
    pub blocks: HashMap<Pos, Stock>,
    /// The storages on the net, each at the block that stands for it.
    pub stores: HashMap<Pos, NetStore>,
    pub max_stack: HashMap<ItemKey, u32>,
}

/// The stack size of `item` (a storage's key names a worn item after its id).
fn stack_in(max_stack: &HashMap<ItemKey, u32>, item: &str) -> u32 {
    max_stack
        .get(item)
        .or_else(|| max_stack.get(keys::item_of(item)))
        .copied()
        .unwrap_or(DEFAULT_STACK)
        .max(1)
}

/// A count as the net counts it.
fn clamp(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Stocks {
    fn stack_of(&self, item: &str) -> u32 {
        stack_in(&self.max_stack, item)
    }
}

impl Inventories for Stocks {
    fn offered(&self, block: Pos) -> Vec<(ItemKey, u32)> {
        if let Some(s) = self.stores.get(&block) {
            return if s.works {
                s.store
                    .items
                    .iter()
                    .map(|(k, &n)| (k.clone(), clamp(n)))
                    .collect()
            } else {
                Vec::new()
            };
        }
        let mut out: Vec<(ItemKey, u32)> = Vec::new();
        let Some(stock) = self.blocks.get(&block) else {
            return out;
        };
        for slot in stock.slots.iter().filter(|s| s.can_take && s.count > 0) {
            let Some(item) = &slot.item else { continue };
            match out.iter_mut().find(|(i, _)| i == item) {
                Some((_, n)) => *n += slot.count,
                None => out.push((item.clone(), slot.count)),
            }
        }
        out
    }

    fn room_for(&self, block: Pos, item: &str) -> u32 {
        if let Some(s) = self.stores.get(&block) {
            let stack = |k: &str| stack_in(&self.max_stack, k);
            return if s.works && keys::storable(item) {
                clamp(s.store.room_for(item, &stack))
            } else {
                0
            };
        }
        let Some(stock) = self.blocks.get(&block) else {
            return 0;
        };
        if stock.accepts.as_ref().is_some_and(|a| !a.contains(item)) {
            return 0;
        }
        let stack = self.stack_of(item);
        stock
            .slots
            .iter()
            .filter(|s| s.takes(item))
            .map(|s| match &s.item {
                None => stack,
                Some(i) if i == item => stack.saturating_sub(s.count),
                Some(_) => 0,
            })
            .sum()
    }

    fn take(&mut self, block: Pos, item: &str, count: u32) -> u32 {
        if let Some(s) = self.stores.get_mut(&block) {
            return if s.works {
                clamp(s.store.take(item, u64::from(count)))
            } else {
                0
            };
        }
        let Some(stock) = self.blocks.get_mut(&block) else {
            return 0;
        };
        let mut left = count;
        for slot in stock.slots.iter_mut().filter(|s| s.can_take) {
            if left == 0 {
                break;
            }
            if slot.item.as_deref() != Some(item) {
                continue;
            }
            let n = slot.count.min(left);
            slot.count -= n;
            left -= n;
            if slot.count == 0 {
                slot.item = None;
            }
        }
        count - left
    }

    fn put(&mut self, block: Pos, item: &str, count: u32) -> u32 {
        if let Some(s) = self.stores.get_mut(&block) {
            let max_stack = &self.max_stack;
            let stack = |k: &str| stack_in(max_stack, k);
            return if s.works && keys::storable(item) {
                clamp(s.store.put(item, u64::from(count), &stack))
            } else {
                0
            };
        }
        let stack = self.stack_of(item);
        let Some(stock) = self.blocks.get_mut(&block) else {
            return 0;
        };
        if stock.accepts.as_ref().is_some_and(|a| !a.contains(item)) {
            return 0;
        }
        let mut left = count;
        // Onto stacks of the same item first, then into empty slots, as a hopper fills.
        for same in [true, false] {
            for slot in stock.slots.iter_mut().filter(|s| s.takes(item)) {
                if left == 0 {
                    break;
                }
                let fits = match &slot.item {
                    Some(i) if same && i == item => stack.saturating_sub(slot.count),
                    None if !same => stack,
                    _ => 0,
                };
                let n = fits.min(left);
                if n > 0 {
                    slot.item = Some(item.to_owned());
                    slot.count += n;
                    left -= n;
                }
            }
        }
        count - left
    }
}

/// What a net's moves add up to per (from, to, item), in the order first made: the moves the
/// bundle asks the game to make.
pub fn totals(transfers: &[crate::net::items::Transfer]) -> BTreeMap<(Pos, Pos, ItemKey), u32> {
    let mut out = BTreeMap::new();
    for t in transfers {
        *out.entry((t.from, t.to, t.item.clone())).or_insert(0) += t.count;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Face;
    use crate::net::items::{Filter, Joint, JointNote, Mode, NetState, tick};

    fn chest(slots: usize) -> Stock {
        Stock {
            slots: vec![
                StockSlot {
                    can_take: true,
                    can_put: true,
                    ..StockSlot::default()
                };
                slots
            ],
            accepts: None,
        }
    }

    fn filled(mut stock: Stock, item: &str, counts: &[u32]) -> Stock {
        for (slot, &n) in stock.slots.iter_mut().zip(counts) {
            if n > 0 {
                slot.item = Some(item.to_owned());
                slot.count = n;
            }
        }
        stock
    }

    /// An electric furnace: input slot (put only, smeltable only) and output slot (take only).
    fn furnace(input: Option<(&str, u32)>, output: Option<(&str, u32)>, smelts: &[&str]) -> Stock {
        let slot = |s: Option<(&str, u32)>, can_take, can_put| StockSlot {
            item: s.map(|(i, _)| i.to_owned()),
            count: s.map_or(0, |(_, n)| n),
            can_take,
            can_put,
            only: None,
        };
        Stock {
            slots: vec![slot(input, false, true), slot(output, true, false)],
            accepts: Some(smelts.iter().map(|s| s.to_string()).collect()),
        }
    }

    fn joint(block: Pos, mode: Mode) -> Joint {
        Joint {
            housing: [block[0], block[1] + 1, block[2]],
            face: Face::YNeg,
            block,
            mode,
            filter: Filter::default(),
            priority: 0,
        }
    }

    const A: Pos = [0, 0, 0];
    const F: Pos = [1, 0, 0];
    const B: Pos = [2, 0, 0];
    const RAW: &str = "minecraft:raw_iron";
    const INGOT: &str = "minecraft:iron_ingot";

    #[test]
    fn a_chest_offers_its_items_by_kind_in_slot_order() {
        let mut stocks = Stocks::default();
        let mut c = filled(chest(3), RAW, &[5, 0, 7]);
        c.slots[1].item = Some(INGOT.into());
        c.slots[1].count = 2;
        stocks.blocks.insert(A, c);
        assert_eq!(stocks.offered(A), vec![(RAW.into(), 12), (INGOT.into(), 2)]);
    }

    #[test]
    fn room_counts_partial_stacks_and_empty_slots_by_stack_size() {
        let mut stocks = Stocks::default();
        stocks.blocks.insert(A, filled(chest(3), RAW, &[60, 0, 64]));
        assert_eq!(stocks.room_for(A, RAW), 4 + 64);
        assert_eq!(stocks.room_for(A, INGOT), 64);
        stocks.max_stack.insert("minecraft:ender_pearl".into(), 16);
        assert_eq!(stocks.room_for(A, "minecraft:ender_pearl"), 16);
    }

    #[test]
    fn the_furnace_takes_only_what_it_smelts_and_gives_only_its_output() {
        // M17: through one joint the furnace takes ore and gives ingots, never the ore back.
        let mut stocks = Stocks::default();
        stocks
            .blocks
            .insert(F, furnace(Some((RAW, 3)), Some((INGOT, 2)), &[RAW]));
        assert_eq!(stocks.offered(F), vec![(INGOT.into(), 2)]);
        assert_eq!(stocks.room_for(F, RAW), 61);
        assert_eq!(stocks.room_for(F, "minecraft:dirt"), 0);
        assert_eq!(stocks.room_for(F, INGOT), 0);
    }

    #[test]
    fn ore_goes_from_the_chest_into_the_furnace_and_ingots_on_to_the_last_chest() {
        // In-game check (a) on the rules: chest(From) → furnace(Both) → chest(Into), one net.
        let mut stocks = Stocks::default();
        stocks.blocks.insert(A, filled(chest(2), RAW, &[10, 0]));
        stocks
            .blocks
            .insert(F, furnace(None, Some((INGOT, 3)), &[RAW]));
        stocks.blocks.insert(B, chest(2));
        let joints = [
            joint(A, Mode::From),
            joint(F, Mode::Both),
            joint(B, Mode::Into),
        ];
        let mut state = NetState::default();
        let r = tick(&joints, 2000.0, 20.0, &mut state, &mut stocks);
        let moved = totals(&r.transfers);
        assert_eq!(moved.get(&(F, B, INGOT.into())), Some(&3));
        // On one net the last chest takes ore too (it takes anything), so M8 shares the ore in
        // turn between it and the furnace; in the game the two sides are separate nets.
        assert_eq!(moved.get(&(A, F, RAW.into())), Some(&5));
        assert_eq!(moved.get(&(A, B, RAW.into())), Some(&5));
        // Ingots never go back into the furnace.
        assert!(!moved.keys().any(|(_, to, item)| *to == F && item == INGOT));
    }

    #[test]
    fn a_full_last_chest_keeps_the_ingots_in_the_furnace() {
        // In-game check (d): «Приёмник полон», nothing leaves the furnace.
        let mut stocks = Stocks::default();
        stocks
            .blocks
            .insert(F, furnace(None, Some((INGOT, 5)), &[RAW]));
        stocks
            .blocks
            .insert(B, filled(chest(2), "minecraft:stone", &[64, 64]));
        let joints = [joint(F, Mode::Both), joint(B, Mode::Into)];
        let mut state = NetState::default();
        let r = tick(&joints, 2000.0, 20.0, &mut state, &mut stocks);
        assert!(r.transfers.is_empty());
        assert_eq!(stocks.offered(F), vec![(INGOT.into(), 5)]);
        assert_eq!(r.notes, vec![(1, JointNote::ReceiverFull)]);
    }

    #[test]
    fn put_fills_same_stacks_first() {
        let mut stocks = Stocks::default();
        stocks.blocks.insert(A, filled(chest(3), RAW, &[0, 60, 0]));
        assert_eq!(stocks.put(A, RAW, 10), 10);
        let slots = &stocks.blocks[&A].slots;
        assert_eq!((slots[1].count, slots[0].count), (64, 6));
        assert_eq!(stocks.take(A, RAW, 100), 70);
        assert!(stocks.blocks[&A].slots.iter().all(|s| s.item.is_none()));
    }

    #[test]
    fn a_slot_that_takes_only_fuel_gets_only_fuel() {
        // An alloy smelter: one input that takes gold, one fuel slot that takes coal.
        let only = |items: &[&str]| Some(items.iter().map(|i| (*i).to_owned()).collect());
        let mut stocks = Stocks::default();
        let slot = |o| StockSlot {
            can_put: true,
            only: o,
            ..StockSlot::default()
        };
        stocks.blocks.insert(
            A,
            Stock {
                slots: vec![slot(only(&["gold"])), slot(only(&["coal"]))],
                accepts: None,
            },
        );
        assert_eq!(stocks.room_for(A, "coal"), 64);
        assert_eq!(stocks.room_for(A, "dirt"), 0);
        assert_eq!(stocks.put(A, "coal", 70), 64);
        let slots = &stocks.blocks[&A].slots;
        assert_eq!((slots[0].item.as_deref(), slots[1].count), (None, 64));
    }

    fn storage(cells: u32, works: bool) -> NetStore {
        NetStore {
            store: Store {
                cells,
                ..Store::default()
            },
            works,
        }
    }

    #[test]
    fn a_storage_takes_from_a_chest_line_up_to_its_limit() {
        // In-game checks: a storage takes items from a chest line; a limit stops intake (S5).
        let mut stocks = Stocks::default();
        stocks.blocks.insert(A, filled(chest(2), RAW, &[64, 30]));
        let mut s = storage(4, true);
        s.store.limits.insert(
            RAW.into(),
            crate::storage::Limit {
                max: 50,
                destroy_surplus: false,
            },
        );
        stocks.stores.insert(B, s);
        let joints = [joint(A, Mode::From), joint(B, Mode::Into)];
        let mut state = NetState::default();
        let r = tick(&joints, 2000.0, 20.0, &mut state, &mut stocks);
        assert_eq!(totals(&r.transfers).get(&(A, B, RAW.into())), Some(&50));
        assert_eq!(stocks.stores[&B].store.count(RAW), 50);
        assert_eq!(stocks.room_for(B, RAW), 0);
    }

    #[test]
    fn a_storage_on_from_gives_what_it_holds() {
        let mut stocks = Stocks::default();
        let mut s = storage(1, true);
        s.store.items.insert(INGOT.into(), 10);
        stocks.stores.insert(A, s);
        stocks.blocks.insert(B, chest(1));
        let joints = [joint(A, Mode::From), joint(B, Mode::Into)];
        let mut state = NetState::default();
        tick(&joints, 2000.0, 20.0, &mut state, &mut stocks);
        assert_eq!(stocks.offered(B), vec![(INGOT.into(), 10)]);
        assert_eq!(stocks.stores[&A].store.count(INGOT), 0);
    }

    #[test]
    fn a_storage_with_a_hole_neither_takes_nor_gives() {
        // Rule S8: nothing moves and nothing is lost.
        let mut stocks = Stocks::default();
        let mut s = storage(3, false);
        s.store.items.insert(INGOT.into(), 5);
        stocks.stores.insert(B, s);
        stocks.blocks.insert(A, filled(chest(1), RAW, &[8]));
        assert_eq!(stocks.room_for(B, RAW), 0);
        assert!(stocks.offered(B).is_empty());
        assert_eq!(stocks.put(B, RAW, 3), 0);
        assert_eq!(stocks.stores[&B].store.count(INGOT), 5);
    }

    #[test]
    fn a_worn_tool_stacks_as_its_item() {
        let mut stocks = Stocks::default();
        stocks.max_stack.insert("minecraft:iron_pickaxe".into(), 1);
        stocks.stores.insert(B, storage(1, true));
        let worn = crate::storage::keys::key("minecraft:iron_pickaxe", 9);
        assert_eq!(stocks.room_for(B, &worn), 27);
    }

    #[test]
    fn a_named_sword_first_in_the_chest_does_not_hold_up_the_rest() {
        // A stack the storage cannot keep is not planned into it, so what follows still goes.
        let mut stocks = Stocks::default();
        let named = crate::storage::keys::stack_key("minecraft:diamond_sword", 0, true);
        let mut c = filled(chest(2), &named, &[1, 0]);
        c.slots[1].item = Some(RAW.into());
        c.slots[1].count = 5;
        stocks.blocks.insert(A, c);
        stocks.stores.insert(B, storage(1, true));
        let joints = [joint(A, Mode::From), joint(B, Mode::Into)];
        let mut state = NetState::default();
        let r = tick(&joints, 2000.0, 20.0, &mut state, &mut stocks);
        let moved = totals(&r.transfers);
        assert_eq!(moved.get(&(A, B, RAW.into())), Some(&5));
        assert!(!moved.keys().any(|(_, _, item)| *item == named));
    }
}
