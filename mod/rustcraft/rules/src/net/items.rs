//! Moving items along one item channel net.
//!
//! Joints are where a housing of the net touches a block with an inventory (M7). Each joint has a
//! mode for this channel, a filter and a priority. Every tick the net moves up to its throughput:
//! from joints that give (a machine's products, a chest) to joints that take, highest priority
//! first and in turn among equal priorities (M8). An item leaves its source only when a receiver
//! has room for it (M6), so nothing gets stuck in the pipe or dropped; an item nobody wants stays
//! where it is (M16). A machine takes and gives through one joint (M17), but what it gives never
//! goes back into itself.

use std::collections::{BTreeMap, BTreeSet};

use crate::geom::{Face, Pos};

/// An item kind as the game names it, with any components that keep stacks apart.
pub type ItemKey = String;

/// Which way items go through a joint, seen from the block behind it (M7, M17).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mode {
    /// The net puts items into the block.
    Into,
    /// The net takes items out of the block.
    From,
    /// Both: a machine takes ingredients and gives products through one joint (M17).
    Both,
    Off,
}

impl Mode {
    /// The order the joint window steps through a joint's modes: the handbook's three (data.js
    /// `Направление`: «в машину», «из машины», «выключен»). `Both` is no step of it: it is a
    /// block's default before its joint is set (data.js: «По умолчанию магистраль сама подаёт
    /// машине то, что она принимает, и забирает готовое»), and the window names it so.
    const CYCLE: [Mode; 3] = [Mode::Into, Mode::From, Mode::Off];
    const ALL: [Mode; 4] = [Mode::Into, Mode::From, Mode::Both, Mode::Off];

    /// The net may put into the block.
    pub fn takes(self) -> bool {
        matches!(self, Mode::Into | Mode::Both)
    }

    /// The net may take out of the block.
    pub fn gives(self) -> bool {
        matches!(self, Mode::From | Mode::Both)
    }

    /// The mode a click sets next: into, from, off, and round again; the default goes to into.
    pub fn next(self) -> Mode {
        if self == Mode::Both {
            return Mode::Into;
        }
        let at = Self::CYCLE.iter().position(|&m| m == self).unwrap_or(0);
        Self::CYCLE[(at + 1) % Self::CYCLE.len()]
    }

    /// The mode before this one in that order (a right click); the default goes to off.
    pub fn previous(self) -> Mode {
        if self == Mode::Both {
            return Mode::Off;
        }
        let at = Self::CYCLE.iter().position(|&m| m == self).unwrap_or(0);
        Self::CYCLE[(at + Self::CYCLE.len() - 1) % Self::CYCLE.len()]
    }

    /// The byte a block saves its joint mode as.
    pub fn code(self) -> u8 {
        match self {
            Mode::Into => 1,
            Mode::From => 2,
            Mode::Both => 3,
            Mode::Off => 4,
        }
    }

    /// The mode `code` wrote; none for a byte it does not know, so a damaged save falls back
    /// to the block's default.
    pub fn from_code(code: u8) -> Option<Mode> {
        Self::ALL.into_iter().find(|m| m.code() == code)
    }
}

/// A white or black list of items on a joint (M7). The default lets everything through.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub allow_only: bool,
    pub items: BTreeSet<ItemKey>,
}

impl Filter {
    pub fn passes(&self, item: &str) -> bool {
        self.items.contains(item) == self.allow_only
    }
}

/// Where a housing of the net touches a block with an inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joint {
    pub housing: Pos,
    /// The face of the housing that looks at the block.
    pub face: Face,
    /// The block behind the joint; two joints on one block are the same inventory.
    pub block: Pos,
    pub mode: Mode,
    pub filter: Filter,
    pub priority: i32,
    /// Slice 18 (guide `quickstart` 5, `housing`): a vanilla container with no setting of its
    /// own. It works both ways but only with the rest of the net: it gives a machine or a storage
    /// what they take, takes a machine's products after every other receiver, and never trades
    /// with another such container.
    pub auto: bool,
}

/// The inventories behind the joints, as the game sees them this tick.
pub trait Inventories {
    /// What the block offers to the net, in slot order: for a machine only its products.
    fn offered(&self, block: Pos) -> Vec<(ItemKey, u32)>;
    /// How many of `item` the block can take now.
    fn room_for(&self, block: Pos, item: &str) -> u32;
    /// Takes up to `count` of `item` out of the block; returns how many it took.
    fn take(&mut self, block: Pos, item: &str, count: u32) -> u32;
    /// Puts up to `count` of `item` into the block; returns how many fitted.
    fn put(&mut self, block: Pos, item: &str, count: u32) -> u32;
}

/// What a net remembers between ticks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetState {
    /// Throughput not yet spent, in items (it grows by a fraction every tick).
    pub budget: f64,
    /// M8: the next receiver in turn for each priority.
    pub turn: BTreeMap<i32, usize>,
    /// The source that goes first this tick, so sources share the budget too.
    pub first_source: usize,
}

/// One move the net made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transfer {
    pub from: Pos,
    pub to: Pos,
    pub item: ItemKey,
    pub count: u32,
}

/// A joint's diagnosis after a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointNote {
    /// «Приёмник полон»: items waited for this receiver but it had no room (M6 example).
    ReceiverFull,
}

/// What one tick of a net did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickResult {
    pub transfers: Vec<Transfer>,
    /// Index into the joints slice, and its note.
    pub notes: Vec<(usize, JointNote)>,
}

/// The turn key of the auto containers' group (`NetState::turn`).
const AUTO_TURN: i32 = i32::MIN;

/// One tick of an item net that carries `per_second` items, with `joints` on it.
pub fn tick(
    joints: &[Joint],
    per_second: f64,
    ticks_per_second: f64,
    state: &mut NetState,
    inv: &mut dyn Inventories,
) -> TickResult {
    let mut result = TickResult::default();
    // Unspent throughput carries over, but no more than a second of it, so an idle net does not
    // burst later.
    state.budget = (state.budget + per_second / ticks_per_second).min(per_second.max(1.0));
    let mut available = (state.budget + 1e-9).floor() as u32;
    let sources = first_per_block(joints, Mode::gives);
    let receivers = first_per_block(joints, Mode::takes);
    if sources.is_empty() || receivers.is_empty() {
        return result;
    }
    // Receivers by priority, highest first; the order inside a priority is the joints' order.
    // Auto containers come after every other receiver (slice 18), with a turn of their own.
    let mut by_priority: BTreeMap<(bool, std::cmp::Reverse<i32>), Vec<usize>> = BTreeMap::new();
    for &r in &receivers {
        let j = &joints[r];
        let priority = if j.auto { AUTO_TURN } else { j.priority };
        by_priority
            .entry((j.auto, std::cmp::Reverse(priority)))
            .or_default()
            .push(r);
    }
    let mut full = BTreeSet::new();
    let mut moved = 0u32;
    let start = state.first_source % sources.len();
    for k in 0..sources.len() {
        let src = &joints[sources[(start + k) % sources.len()]];
        for (item, count) in inv.offered(src.block) {
            if count == 0 || !src.filter.passes(&item) {
                continue;
            }
            let mut left = count;
            for ((_, std::cmp::Reverse(priority)), group) in &by_priority {
                if left == 0 || available == 0 {
                    break;
                }
                // M17: never back into the block it came from.
                let candidates: Vec<usize> = group
                    .iter()
                    .copied()
                    .filter(|&r| {
                        joints[r].block != src.block
                            && joints[r].filter.passes(&item)
                            && !(src.auto && joints[r].auto)
                    })
                    .collect();
                if candidates.is_empty() {
                    continue;
                }
                let turn = state.turn.entry(*priority).or_insert(0);
                let gave = share_in_turn(
                    joints,
                    &candidates,
                    turn,
                    src.block,
                    &item,
                    &mut left,
                    &mut available,
                    inv,
                    &mut result.transfers,
                );
                moved += gave;
                if left > 0 && available > 0 {
                    // Items waited and every receiver of this priority had no room.
                    full.extend(candidates);
                }
            }
        }
    }
    state.first_source = (start + 1) % sources.len();
    state.budget -= f64::from(moved);
    result.notes = full
        .into_iter()
        .map(|j| (j, JointNote::ReceiverFull))
        .collect();
    result
}

/// The first joint on each block whose mode passes `wanted`: two joints on one block are one inventory.
fn first_per_block(joints: &[Joint], wanted: fn(Mode) -> bool) -> Vec<usize> {
    let mut blocks = BTreeSet::new();
    (0..joints.len())
        .filter(|&j| wanted(joints[j].mode) && blocks.insert(joints[j].block))
        .collect()
}

/// M8: hands `item` out one at a time to `candidates` in turn, starting at `turn`, while there is
/// room, items `left` and throughput `available`. Returns how many moved.
#[allow(clippy::too_many_arguments)]
fn share_in_turn(
    joints: &[Joint],
    candidates: &[usize],
    turn: &mut usize,
    from: Pos,
    item: &str,
    left: &mut u32,
    available: &mut u32,
    inv: &mut dyn Inventories,
    transfers: &mut Vec<Transfer>,
) -> u32 {
    let mut gave = 0;
    while *left > 0 && *available > 0 {
        let Some(offset) = (0..candidates.len()).find(|i| {
            inv.room_for(
                joints[candidates[(*turn + i) % candidates.len()]].block,
                item,
            ) > 0
        }) else {
            break;
        };
        let at = (*turn + offset) % candidates.len();
        let to = joints[candidates[at]].block;
        if inv.take(from, item, 1) == 0 {
            *left = 0;
            break;
        }
        if inv.put(to, item, 1) == 0 {
            // The receiver said it had room but took nothing: give the item back.
            inv.put(from, item, 1);
            break;
        }
        record(transfers, from, to, item);
        *left -= 1;
        *available -= 1;
        gave += 1;
        *turn = (at + 1) % candidates.len();
    }
    gave
}

/// Adds one item to the transfer list, merging it into the last move if it is the same.
fn record(transfers: &mut Vec<Transfer>, from: Pos, to: Pos, item: &str) {
    if let Some(t) = transfers
        .iter_mut()
        .find(|t| t.from == from && t.to == to && t.item == item)
    {
        t.count += 1;
    } else {
        transfers.push(Transfer {
            from,
            to,
            item: item.to_string(),
            count: 1,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Chests and machines for the tests: `offer` is what a block gives, `room` how much of each
    /// item it still takes.
    #[derive(Default)]
    struct World {
        offer: HashMap<Pos, Vec<(ItemKey, u32)>>,
        room: HashMap<Pos, HashMap<ItemKey, u32>>,
        got: HashMap<Pos, HashMap<ItemKey, u32>>,
    }

    impl World {
        fn gives(&mut self, block: Pos, item: &str, count: u32) {
            self.offer
                .entry(block)
                .or_default()
                .push((item.into(), count));
        }
        fn takes(&mut self, block: Pos, item: &str, room: u32) {
            self.room
                .entry(block)
                .or_default()
                .insert(item.into(), room);
        }
        fn got(&self, block: Pos, item: &str) -> u32 {
            self.got
                .get(&block)
                .and_then(|m| m.get(item))
                .copied()
                .unwrap_or(0)
        }
        fn left(&self, block: Pos, item: &str) -> u32 {
            self.offer
                .get(&block)
                .map(|v| v.iter().filter(|(i, _)| i == item).map(|(_, c)| c).sum())
                .unwrap_or(0)
        }
    }

    impl Inventories for World {
        fn offered(&self, block: Pos) -> Vec<(ItemKey, u32)> {
            self.offer.get(&block).cloned().unwrap_or_default()
        }
        fn room_for(&self, block: Pos, item: &str) -> u32 {
            self.room
                .get(&block)
                .and_then(|m| m.get(item))
                .copied()
                .unwrap_or(0)
        }
        fn take(&mut self, block: Pos, item: &str, count: u32) -> u32 {
            let mut left = count;
            for (i, c) in self.offer.entry(block).or_default().iter_mut() {
                if i == item && left > 0 {
                    let n = (*c).min(left);
                    *c -= n;
                    left -= n;
                }
            }
            count - left
        }
        fn put(&mut self, block: Pos, item: &str, count: u32) -> u32 {
            let room = self
                .room
                .entry(block)
                .or_default()
                .entry(item.into())
                .or_insert(0);
            let n = (*room).min(count);
            *room -= n;
            *self
                .got
                .entry(block)
                .or_default()
                .entry(item.into())
                .or_insert(0) += n;
            n
        }
    }

    fn joint(block: Pos, mode: Mode, priority: i32) -> Joint {
        Joint {
            housing: [block[0], block[1] + 1, block[2]],
            face: Face::YNeg,
            block,
            mode,
            filter: Filter::default(),
            priority,
            auto: false,
        }
    }

    fn auto_chest(block: Pos) -> Joint {
        Joint {
            auto: true,
            ..joint(block, Mode::Both, 0)
        }
    }

    const FURNACE: Pos = [5, 0, 0];

    #[test]
    fn an_auto_chest_feeds_a_machine_and_takes_its_products() {
        // Guide `quickstart` 5: ore in the left chest reaches the furnace, ingots the right chest.
        let mut world = World::default();
        world.gives(CHEST, "minecraft:raw_iron", 4);
        world.takes(FURNACE, "minecraft:raw_iron", 64);
        world.gives(FURNACE, "minecraft:iron_ingot", 2);
        world.takes(CHEST2, "minecraft:iron_ingot", 64);
        world.takes(CHEST2, "minecraft:raw_iron", 64);
        let joints = [auto_chest(CHEST), joint(FURNACE, Mode::Both, 0), auto_chest(CHEST2)];
        run(&joints, &mut world, 1);
        assert_eq!(world.got(FURNACE, "minecraft:raw_iron"), 4);
        assert_eq!(world.got(CHEST2, "minecraft:iron_ingot"), 2);
        assert_eq!(world.got(CHEST2, "minecraft:raw_iron"), 0, "chest to chest");
    }

    #[test]
    fn auto_chests_never_trade_between_themselves() {
        let mut world = World::default();
        world.gives(CHEST, "minecraft:cobblestone", 10);
        world.takes(CHEST2, "minecraft:cobblestone", 64);
        let joints = [auto_chest(CHEST), auto_chest(CHEST2)];
        let r = run(&joints, &mut world, 2);
        assert!(r.iter().all(|t| t.transfers.is_empty()));
        assert!(r.iter().all(|t| t.notes.is_empty()), "no «Приёмник полон» either");
    }

    #[test]
    fn an_auto_chest_is_the_last_receiver() {
        // A machine product goes to a storage or a set receiver first, to the chest only after.
        let mut world = World::default();
        world.gives(FURNACE, "minecraft:iron_ingot", 3);
        world.takes(CHEST, "minecraft:iron_ingot", 64);
        world.takes(CHEST2, "minecraft:iron_ingot", 2);
        let joints = [
            joint(FURNACE, Mode::Both, 0),
            auto_chest(CHEST),
            joint(CHEST2, Mode::Into, -5),
        ];
        run(&joints, &mut world, 1);
        assert_eq!(world.got(CHEST2, "minecraft:iron_ingot"), 2);
        assert_eq!(world.got(CHEST, "minecraft:iron_ingot"), 1);
    }

    #[test]
    fn a_set_chest_still_feeds_an_auto_chest() {
        let mut world = World::default();
        world.gives(CHEST, "minecraft:cobblestone", 5);
        world.takes(CHEST2, "minecraft:cobblestone", 64);
        let joints = [joint(CHEST, Mode::From, 0), auto_chest(CHEST2)];
        run(&joints, &mut world, 1);
        assert_eq!(world.got(CHEST2, "minecraft:cobblestone"), 5);
    }

    const CRUSHER: Pos = [0, 0, 0];
    const CHEST: Pos = [1, 0, 0];
    const CHEST2: Pos = [2, 0, 0];

    /// Ticks a net that is fast enough to move everything at once.
    fn run(joints: &[Joint], world: &mut World, ticks: u32) -> Vec<TickResult> {
        let mut state = NetState::default();
        (0..ticks)
            .map(|_| tick(joints, 2000.0, 20.0, &mut state, world))
            .collect()
    }

    #[test]
    fn nothing_leaves_a_source_when_the_receiver_is_full() {
        // Rule M6, example «приёмник полон».
        let mut world = World::default();
        world.gives(CRUSHER, "minecraft:raw_iron", 10);
        world.takes(CHEST, "minecraft:raw_iron", 0);
        let joints = [joint(CRUSHER, Mode::From, 0), joint(CHEST, Mode::Into, 0)];
        let r = run(&joints, &mut world, 1);
        assert!(r[0].transfers.is_empty());
        assert_eq!(world.left(CRUSHER, "minecraft:raw_iron"), 10);
        assert_eq!(r[0].notes, vec![(1, JointNote::ReceiverFull)]);
    }

    #[test]
    fn receivers_of_equal_priority_take_turns() {
        // Rule M8.
        let mut world = World::default();
        world.gives(CRUSHER, "rc:dust_iron", 6);
        world.takes(CHEST, "rc:dust_iron", 64);
        world.takes(CHEST2, "rc:dust_iron", 64);
        let joints = [
            joint(CRUSHER, Mode::From, 0),
            joint(CHEST, Mode::Into, 0),
            joint(CHEST2, Mode::Into, 0),
        ];
        run(&joints, &mut world, 1);
        assert_eq!(world.got(CHEST, "rc:dust_iron"), 3);
        assert_eq!(world.got(CHEST2, "rc:dust_iron"), 3);
    }

    #[test]
    fn a_higher_priority_fills_first() {
        // Rule M7: priority.
        let mut world = World::default();
        world.gives(CRUSHER, "rc:dust_iron", 10);
        world.takes(CHEST, "rc:dust_iron", 4);
        world.takes(CHEST2, "rc:dust_iron", 64);
        let joints = [
            joint(CRUSHER, Mode::From, 0),
            joint(CHEST, Mode::Into, 5),
            joint(CHEST2, Mode::Into, 0),
        ];
        run(&joints, &mut world, 1);
        assert_eq!(world.got(CHEST, "rc:dust_iron"), 4);
        assert_eq!(world.got(CHEST2, "rc:dust_iron"), 6);
    }

    #[test]
    fn a_filter_keeps_other_items_out() {
        // Rule M7: white list.
        let mut world = World::default();
        world.gives(CRUSHER, "minecraft:cobblestone", 5);
        world.gives(CRUSHER, "minecraft:raw_iron", 5);
        world.takes(CHEST, "minecraft:cobblestone", 64);
        world.takes(CHEST, "minecraft:raw_iron", 64);
        let mut ore_only = joint(CHEST, Mode::Into, 0);
        ore_only.filter = Filter {
            allow_only: true,
            items: BTreeSet::from(["minecraft:raw_iron".to_string()]),
        };
        run(&[joint(CRUSHER, Mode::From, 0), ore_only], &mut world, 1);
        assert_eq!(world.got(CHEST, "minecraft:raw_iron"), 5);
        assert_eq!(world.got(CHEST, "minecraft:cobblestone"), 0);
        // Rule M16: what nobody wants stays in the source.
        assert_eq!(world.left(CRUSHER, "minecraft:cobblestone"), 5);
    }

    #[test]
    fn a_machine_gives_and_takes_through_one_joint_but_not_to_itself() {
        // Rule M17: the crafter's paper goes to the chest, the chest's cane goes to the crafter.
        let crafter: Pos = [5, 0, 0];
        let mut world = World::default();
        world.gives(crafter, "minecraft:paper", 3);
        world.takes(crafter, "minecraft:paper", 64);
        world.takes(crafter, "minecraft:sugar_cane", 64);
        world.gives(CHEST, "minecraft:sugar_cane", 9);
        world.takes(CHEST, "minecraft:paper", 64);
        let joints = [joint(crafter, Mode::Both, 0), joint(CHEST, Mode::Both, 0)];
        run(&joints, &mut world, 1);
        assert_eq!(world.got(CHEST, "minecraft:paper"), 3);
        assert_eq!(world.got(crafter, "minecraft:paper"), 0);
        assert_eq!(world.got(crafter, "minecraft:sugar_cane"), 9);
    }

    #[test]
    fn the_net_moves_no_more_than_its_throughput() {
        // A tier-1 item channel carries 4 items a second (tier table): 20 ticks move 4.
        let mut world = World::default();
        world.gives(CRUSHER, "rc:dust_iron", 64);
        world.takes(CHEST, "rc:dust_iron", 64);
        let joints = [joint(CRUSHER, Mode::From, 0), joint(CHEST, Mode::Into, 0)];
        let mut state = NetState::default();
        for _ in 0..20 {
            tick(&joints, 4.0, 20.0, &mut state, &mut world);
        }
        assert_eq!(world.got(CHEST, "rc:dust_iron"), 4);
        assert_eq!(world.left(CRUSHER, "rc:dust_iron"), 60);
    }

    #[test]
    fn the_wrench_cycles_the_handbooks_three_directions() {
        // Wave 2 (data.js `Направление`): Into → From → Off → Into; the default is left for into
        // (or, back, for off) and never comes round again.
        let mut mode = Mode::Both;
        let mut seen = vec![mode];
        for _ in 0..4 {
            mode = mode.next();
            seen.push(mode);
        }
        assert_eq!(seen, [Mode::Both, Mode::Into, Mode::From, Mode::Off, Mode::Into]);
        assert_eq!(Mode::Both.previous(), Mode::Off);
        assert_eq!(Mode::Into.previous(), Mode::Off);
    }

    #[test]
    fn a_mode_survives_a_save_and_a_bad_byte_reads_as_none() {
        for mode in [Mode::Into, Mode::From, Mode::Both, Mode::Off] {
            assert_eq!(Mode::from_code(mode.code()), Some(mode));
        }
        assert_eq!(Mode::from_code(0), None);
        assert_eq!(Mode::from_code(9), None);
    }

    #[test]
    fn a_joint_that_is_off_does_nothing() {
        let mut world = World::default();
        world.gives(CRUSHER, "rc:dust_iron", 5);
        world.takes(CHEST, "rc:dust_iron", 64);
        run(
            &[joint(CRUSHER, Mode::Off, 0), joint(CHEST, Mode::Into, 0)],
            &mut world,
            1,
        );
        assert_eq!(world.got(CHEST, "rc:dust_iron"), 0);
    }
}
