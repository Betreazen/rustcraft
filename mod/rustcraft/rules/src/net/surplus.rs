//! 0.21 (round 32, spec 32): a storage's surplus leaves by itself over its item net. The owner:
//! «если я выставляю лимиты на какие-то предметы, чтобы они в первую очередь автоматически шли в
//! дезинтегратор, если он есть, а потом в утилизатор».
//!
//! A storage's limit on an item is how much of it the storage keeps (`storage::surplus`). Its
//! surplus is what it holds over the limit (SP1) and what arrives that no receiver of the net
//! keeps while a storage of the net holds its limit (SP2). The surplus goes to a sink of the net
//! (SP3): to the disintegrators that take it apart, else to the trashes. A sink takes the surplus
//! with no joint filter; its allow list only adds what it takes beyond the surplus, a block list
//! still keeps out what it names (SP4). With no sink for an item it stays as before: the storage
//! takes no more, or destroys it with «уничтожать излишек» (S5). The moves count against the
//! net's throughput (SP6).

use std::collections::BTreeSet;

use crate::geom::Pos;
use crate::net::items::{Filter, Inventories, Joint, Mode, NetState, Transfer, record, share_in_turn};

/// What a block does with a storage's surplus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sink {
    /// It takes apart what it can (`Inventories::takes_surplus`): that surplus goes to it alone
    /// and waits for it while it is busy.
    Disintegrator,
    /// It burns the rest, 1 E an item.
    Trash,
}

/// The turn key of the sinks (`NetState::turn`).
const SINK_TURN: i32 = i32::MIN + 1;

/// SP4: a block list on a sink's joint still keeps out what it names; an allow list does not
/// limit the surplus.
fn refused(filter: &Filter, item: &str) -> bool {
    !filter.allow_only && filter.items.contains(item)
}

/// SP3: the sinks among `receivers` that take `item` from `from` as surplus: the disintegrators
/// that take it apart, or the trashes when no disintegrator does.
pub fn sinks_for(
    joints: &[Joint],
    receivers: &[usize],
    inv: &dyn Inventories,
    from: Pos,
    item: &str,
) -> Vec<usize> {
    let of = |kind: Sink| -> Vec<usize> {
        receivers
            .iter()
            .copied()
            .filter(|&r| {
                let j = &joints[r];
                j.block != from
                    && inv.sink(j.block) == Some(kind)
                    && !refused(&j.filter, item)
                    && inv.takes_surplus(j.block, item)
            })
            .collect()
    };
    let breakers = of(Sink::Disintegrator);
    if breakers.is_empty() { of(Sink::Trash) } else { breakers }
}

/// Up to `count` of `item` from `from` to its sinks in turn, as far as they have room and the
/// net throughput `available`: how many went.
#[allow(clippy::too_many_arguments)]
fn to_sinks(
    joints: &[Joint],
    receivers: &[usize],
    state: &mut NetState,
    inv: &mut dyn Inventories,
    from: Pos,
    item: &str,
    count: u32,
    available: &mut u32,
    transfers: &mut Vec<Transfer>,
) -> u32 {
    let sinks = sinks_for(joints, receivers, &*inv, from, item);
    if sinks.is_empty() || count == 0 {
        return 0;
    }
    let turn = state.turn.entry(SINK_TURN).or_insert(0);
    let mut left = count;
    share_in_turn(
        joints,
        &sinks,
        turn,
        from,
        item,
        &mut left,
        available,
        inv,
        transfers,
        surplus_room,
    )
}

fn surplus_room(inv: &dyn Inventories, block: Pos, item: &str) -> u32 {
    inv.surplus_room(block, item)
}

/// SP1: each storage of the net, on a joint not switched off and with no orders (they may need
/// what it holds), gives what it holds over its limits to its sinks. How many moved.
pub fn stock_out(
    joints: &[Joint],
    receivers: &[usize],
    state: &mut NetState,
    inv: &mut dyn Inventories,
    available: &mut u32,
    transfers: &mut Vec<Transfer>,
) -> u32 {
    let mut seen = BTreeSet::new();
    let stores: Vec<usize> = (0..joints.len())
        .filter(|&j| {
            let b = joints[j].block;
            joints[j].mode != Mode::Off && inv.is_store(b) && !inv.busy(b) && seen.insert(b)
        })
        .collect();
    let mut moved = 0;
    for s in stores {
        let store = &joints[s];
        for (item, over) in inv.surplus(store.block) {
            if *available == 0 {
                return moved;
            }
            if store.filter.passes(&item) {
                moved += to_sinks(joints, receivers, state, inv, store.block, &item, over, available, transfers);
            }
        }
    }
    moved
}

/// SP2: `left` of `item` from `from` (no storage) that no receiver took, while the storages
/// `limited` of the net hold their limit of it, go to its sinks; what they do not take goes into
/// a storage of them that destroys the surplus (S5, as before). How many moved.
#[allow(clippy::too_many_arguments)]
pub fn arriving(
    joints: &[Joint],
    receivers: &[usize],
    limited: &[usize],
    state: &mut NetState,
    inv: &mut dyn Inventories,
    from: Pos,
    item: &str,
    left: u32,
    available: &mut u32,
    transfers: &mut Vec<Transfer>,
) -> u32 {
    if limited.is_empty() {
        return 0;
    }
    let mut gave = to_sinks(joints, receivers, state, inv, from, item, left, available, transfers);
    for &r in limited {
        let block = joints[r].block;
        if !inv.destroys(block, item) {
            continue;
        }
        while gave < left && *available > 0 {
            if inv.take(from, item, 1) == 0 {
                return gave;
            }
            if inv.put(block, item, 1) == 0 {
                inv.put(from, item, 1);
                break;
            }
            record(transfers, from, block, item);
            gave += 1;
            *available -= 1;
        }
    }
    gave
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Face;
    use crate::net::items::{JointNote, tick};
    use crate::net::stock::{NetStore, SinkStock, Stock, StockSlot, Stocks, totals};
    use crate::storage::{Limit, Store};
    use std::collections::BTreeMap;

    const CHEST: Pos = [0, 0, 0];
    const STORE: Pos = [1, 0, 0];
    const DIS: Pos = [2, 0, 0];
    const TRASH: Pos = [3, 0, 0];
    const STORE2: Pos = [4, 0, 0];

    const PICK: &str = "minecraft:iron_pickaxe";
    const INGOT: &str = "minecraft:iron_ingot";
    const STICK: &str = "minecraft:stick";
    const COBBLE: &str = "minecraft:cobblestone";
    const SEEDS: &str = "minecraft:wheat_seeds";

    fn joint(block: Pos, mode: Mode) -> Joint {
        Joint {
            housing: [block[0], block[1] + 1, block[2]],
            face: Face::YNeg,
            block,
            mode,
            filter: Filter::default(),
            priority: 0,
            auto: false,
            refiner: false,
            store: false,
            loader: false,
        }
    }

    /// The trash's joint as it is placed (rustcraft_rules::machines::tiered::trash_joint).
    fn trash_joint() -> Joint {
        let s = crate::machines::tiered::trash_joint();
        Joint { filter: s.filter, priority: s.priority, ..joint(TRASH, s.mode) }
    }

    /// A storage on its default joint.
    fn store_joint(block: Pos) -> Joint {
        Joint { store: true, ..joint(block, Mode::Into) }
    }

    fn chest_with(items: &[(&str, u32)]) -> Stock {
        let mut slots = vec![StockSlot { can_take: true, can_put: true, ..StockSlot::default() }; 9];
        for (slot, (item, n)) in slots.iter_mut().zip(items) {
            slot.item = Some((*item).to_owned());
            slot.count = *n;
        }
        Stock { slots, accepts: None }
    }

    /// A machine's inputs that take nothing from the net's own flow (no filter names anything).
    fn inputs(n: usize, only: &[&str]) -> Stock {
        let slot = StockSlot {
            can_put: true,
            only: Some(only.iter().map(|i| (*i).to_owned()).collect()),
            ..StockSlot::default()
        };
        Stock { slots: vec![slot; n], accepts: None }
    }

    fn sink(kind: Sink, takes: &[&str]) -> SinkStock {
        SinkStock { kind, takes: takes.iter().map(|i| (*i).to_owned()).collect() }
    }

    fn store(limits: &[(&str, u64, bool)], items: &[(&str, u64)]) -> NetStore {
        let mut s = Store { cells: 4, ..Store::default() };
        for (item, max, destroy) in limits {
            s.limits.insert((*item).into(), Limit { max: *max, destroy_surplus: *destroy });
        }
        for (item, n) in items {
            s.restore(item, *n);
        }
        NetStore { store: s, works: true }
    }

    /// A storage, a disintegrator that takes pickaxes apart and a trash on one net, a chest too.
    fn line(limits: &[(&str, u64, bool)], held: &[(&str, u64)], chest: &[(&str, u32)]) -> Stocks {
        let mut s = Stocks::default();
        s.max_stack.insert(PICK.into(), 1);
        s.stores.insert(STORE, store(limits, held));
        s.blocks.insert(CHEST, chest_with(chest));
        s.blocks.insert(DIS, inputs(1, &[]));
        s.blocks.insert(TRASH, inputs(9, &[]));
        let limited: Vec<&str> = limits.iter().map(|(i, _, _)| *i).collect();
        s.sinks.insert(DIS, sink(Sink::Disintegrator, &[PICK]));
        s.sinks.insert(TRASH, sink(Sink::Trash, &limited));
        s
    }

    fn joints() -> Vec<Joint> {
        vec![joint(CHEST, Mode::From), store_joint(STORE), joint(DIS, Mode::Both), trash_joint()]
    }

    fn run(joints: &[Joint], stocks: &mut Stocks, per_second: f64, ticks: u32) -> BTreeMap<(Pos, Pos, String), u32> {
        let mut state = NetState::default();
        let mut all = BTreeMap::new();
        for _ in 0..ticks {
            let r = tick(joints, per_second, 20.0, &mut state, stocks);
            for (k, n) in totals(&r.transfers) {
                *all.entry(k).or_insert(0) += n;
            }
        }
        all
    }

    fn held(s: &Stocks, block: Pos, item: &str) -> u32 {
        s.blocks[&block].slots.iter().filter(|x| x.item.as_deref() == Some(item)).map(|x| x.count).sum()
    }

    #[test]
    fn a_lowered_limit_sends_the_stock_over_it_to_the_disintegrator_first() {
        // SP1, SP3: three pickaxes over the limit; the disintegrator takes them apart, one at a
        // time (its input holds one), none goes to the trash.
        let mut s = line(&[(PICK, 2, false)], &[(PICK, 5)], &[]);
        let moved = run(&joints(), &mut s, 2000.0, 1);
        assert_eq!(moved.get(&(STORE, DIS, PICK.into())), Some(&1));
        assert!(!moved.keys().any(|(_, to, _)| *to == TRASH), "{moved:?}");
        assert_eq!(s.stores[&STORE].store.count(PICK), 4);
        // The disintegrator took it apart: the rest follows, two in all, never below the limit.
        for _ in 0..4 {
            s.blocks.insert(DIS, inputs(1, &[]));
            run(&joints(), &mut s, 2000.0, 1);
        }
        assert_eq!(s.stores[&STORE].store.count(PICK), 2);
    }

    #[test]
    fn what_the_disintegrator_does_not_take_apart_goes_to_the_trash() {
        // SP3: ingots and sticks over their limits (the disintegrator gave them back) burn.
        let mut s = line(&[(PICK, 2, false), (INGOT, 10, false), (STICK, 0, false)], &[(INGOT, 13), (STICK, 2)], &[]);
        let moved = run(&joints(), &mut s, 2000.0, 1);
        assert_eq!(moved.get(&(STORE, TRASH, INGOT.into())), Some(&3));
        assert_eq!(moved.get(&(STORE, TRASH, STICK.into())), Some(&2));
        assert!(!moved.keys().any(|(_, to, _)| *to == DIS));
        assert_eq!(s.stores[&STORE].store.count(INGOT), 10);
    }

    #[test]
    fn what_arrives_over_the_limit_goes_to_the_sinks_and_the_rest_stays() {
        // SP2: the storage keeps 4 cobblestone; the chest's other 6 burn. Cobblestone it does not
        // limit stays with the storage, seeds nobody limits stay in the chest's net as before.
        let mut s = line(&[(COBBLE, 4, false)], &[], &[(COBBLE, 10)]);
        let moved = run(&joints(), &mut s, 2000.0, 1);
        assert_eq!(moved.get(&(CHEST, STORE, COBBLE.into())), Some(&4));
        assert_eq!(moved.get(&(CHEST, TRASH, COBBLE.into())), Some(&6));
        assert_eq!(held(&s, CHEST, COBBLE), 0);
    }

    #[test]
    fn a_trash_with_no_filter_takes_nothing_that_is_no_surplus() {
        // SP4: under the limit, and an item no storage limits, never reach the trash.
        let mut s = line(&[(COBBLE, 64, false)], &[], &[(COBBLE, 10), (SEEDS, 5)]);
        s.stores.get_mut(&STORE).unwrap().store.cells = 0;
        let moved = run(&joints(), &mut s, 2000.0, 3);
        assert!(!moved.keys().any(|(_, to, _)| *to == TRASH), "{moved:?}");
        assert_eq!(held(&s, CHEST, COBBLE), 10, "a full storage under its limit keeps them waiting");
        assert_eq!(held(&s, CHEST, SEEDS), 5);
    }

    #[test]
    fn with_no_sink_nothing_changes() {
        // Without a disintegrator or a trash: the storage takes no more, the rest waits (S5).
        let mut s = line(&[(COBBLE, 4, false), (PICK, 1, false)], &[(PICK, 3)], &[(COBBLE, 10)]);
        let plain = [joint(CHEST, Mode::From), store_joint(STORE)];
        let moved = run(&plain, &mut s, 2000.0, 2);
        assert_eq!(moved.get(&(CHEST, STORE, COBBLE.into())), Some(&4));
        assert_eq!(moved.len(), 1, "{moved:?}");
        assert_eq!(held(&s, CHEST, COBBLE), 6);
        assert_eq!(s.stores[&STORE].store.count(PICK), 3, "the stock over the limit stays");
    }

    #[test]
    fn destroying_the_surplus_comes_after_the_sinks() {
        // S5 with SP2: with «уничтожать излишек» the sinks take first; with no room in them the
        // storage destroys the rest, so the line never clogs. With no sink it destroys all.
        let mut s = line(&[(COBBLE, 4, true)], &[], &[(COBBLE, 30)]);
        s.blocks.insert(TRASH, inputs(1, &[]));
        let moved = run(&joints(), &mut s, 2000.0, 1);
        assert_eq!(moved.get(&(CHEST, TRASH, COBBLE.into())), Some(&26));
        let mut s = line(&[(COBBLE, 4, true)], &[], &[(COBBLE, 100)]);
        s.blocks.insert(TRASH, inputs(1, &[]));
        let moved = run(&joints(), &mut s, 2000.0, 1);
        assert_eq!(moved.get(&(CHEST, TRASH, COBBLE.into())), Some(&64));
        assert_eq!(moved.get(&(CHEST, STORE, COBBLE.into())), Some(&36), "4 kept, 32 destroyed");
        assert_eq!(s.stores[&STORE].store.count(COBBLE), 4);
        assert_eq!(held(&s, CHEST, COBBLE), 0);
        // No sink: the storage destroys what is over, as before, and says nothing is full.
        let mut s = line(&[(COBBLE, 4, true)], &[], &[(COBBLE, 30)]);
        let plain = [joint(CHEST, Mode::From), store_joint(STORE)];
        let mut state = NetState::default();
        let r = tick(&plain, 2000.0, 20.0, &mut state, &mut s);
        assert_eq!(totals(&r.transfers).get(&(CHEST, STORE, COBBLE.into())), Some(&30));
        assert!(r.notes.iter().all(|(_, n)| *n != JointNote::ReceiverFull), "{:?}", r.notes);
    }

    #[test]
    fn a_disintegrator_busy_with_one_keeps_the_rest_waiting_not_burning() {
        // SP3: what the disintegrator takes apart never goes to the trash, even while it is full.
        let mut s = line(&[(PICK, 0, false)], &[(PICK, 4)], &[]);
        s.blocks.get_mut(&DIS).unwrap().slots[0].item = Some(PICK.into());
        s.blocks.get_mut(&DIS).unwrap().slots[0].count = 1;
        let moved = run(&joints(), &mut s, 2000.0, 3);
        assert!(moved.is_empty(), "{moved:?}");
        assert_eq!(s.stores[&STORE].store.count(PICK), 4);
    }

    #[test]
    fn a_charged_item_the_disintegrator_refuses_burns_instead() {
        // The disintegrator's set leaves out what it refuses now (a charged item): the trash.
        let mut s = line(&[(PICK, 0, false)], &[(PICK, 1)], &[]);
        s.sinks.insert(DIS, sink(Sink::Disintegrator, &[]));
        s.sinks.insert(TRASH, sink(Sink::Trash, &[PICK]));
        let moved = run(&joints(), &mut s, 2000.0, 1);
        assert_eq!(moved.get(&(STORE, TRASH, PICK.into())), Some(&1));
    }

    #[test]
    fn surplus_counts_against_the_throughput_and_waits_while_the_storage_has_orders() {
        // SP6: a tier-1 net carries 4 a second; SP1: a storage with orders keeps its stock.
        let mut s = line(&[(COBBLE, 0, false)], &[(COBBLE, 64)], &[]);
        let moved = run(&joints(), &mut s, 4.0, 20);
        assert_eq!(moved.get(&(STORE, TRASH, COBBLE.into())), Some(&4));
        let mut s = line(&[(COBBLE, 0, false)], &[(COBBLE, 64)], &[]);
        s.busy.insert(STORE);
        assert!(run(&joints(), &mut s, 2000.0, 2).is_empty());
    }

    #[test]
    fn a_block_list_on_the_sink_still_keeps_out_what_it_names() {
        // SP4: «всё, кроме булыжника» on the trash: the surplus cobblestone stays.
        let mut s = line(&[(COBBLE, 0, false)], &[(COBBLE, 5)], &[]);
        let mut j = joints();
        j[3].filter = Filter { allow_only: false, items: [COBBLE.to_owned()].into() };
        assert!(run(&j, &mut s, 2000.0, 2).is_empty());
        // An allow list naming something else does not keep the surplus out.
        j[3].filter = Filter { allow_only: true, items: [SEEDS.to_owned()].into() };
        let moved = run(&j, &mut s, 2000.0, 1);
        assert_eq!(moved.get(&(STORE, TRASH, COBBLE.into())), Some(&5));
    }

    #[test]
    fn a_second_storage_that_keeps_the_item_takes_it_before_the_trash() {
        // SP2: what another storage of the net keeps is no surplus.
        let mut s = line(&[(COBBLE, 4, false)], &[(COBBLE, 4)], &[(COBBLE, 10)]);
        s.stores.insert(STORE2, store(&[], &[]));
        let mut j = joints();
        j.push(store_joint(STORE2));
        let moved = run(&j, &mut s, 2000.0, 1);
        assert_eq!(moved.get(&(CHEST, STORE2, COBBLE.into())), Some(&10));
        assert!(!moved.keys().any(|(_, to, _)| *to == TRASH), "{moved:?}");
    }

    #[test]
    fn a_storage_giving_to_the_net_gives_no_surplus_of_another() {
        // A storage set «из блока» gives its own: it is no arriving surplus of the full one.
        let mut s = line(&[(COBBLE, 4, false)], &[(COBBLE, 4)], &[]);
        s.stores.insert(STORE2, store(&[], &[(COBBLE, 10)]));
        let mut j = joints();
        j.push(joint(STORE2, Mode::From));
        let moved = run(&j, &mut s, 2000.0, 2);
        assert!(!moved.keys().any(|(_, to, _)| *to == TRASH), "{moved:?}");
        assert_eq!(s.stores[&STORE2].store.count(COBBLE), 10);
    }

    #[test]
    fn a_sink_switched_off_takes_nothing() {
        let mut s = line(&[(COBBLE, 0, false)], &[(COBBLE, 5)], &[]);
        let mut j = joints();
        j[3].mode = Mode::Off;
        assert!(run(&j, &mut s, 2000.0, 2).is_empty());
    }
}
