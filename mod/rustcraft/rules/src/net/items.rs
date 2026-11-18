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
    /// Slice 18 (guide `quickstart` 5: ore → crusher → furnace): a crusher on its default joint.
    /// What it takes goes to it, not to another receiver of its priority that takes it too (the
    /// furnace smelts ore as well), and waits for it while it is full.
    pub refiner: bool,
    /// 0.18.5 (guide `dock_load`: «Загружает грузовую вагонетку предметами из магистрали»): a
    /// storage on its default joint. It takes as before (M16) and gives to load docks only.
    pub store: bool,
    /// 0.18.5: a load dock on its default joint, which the storages of its net fill for the
    /// cargo workers.
    pub loader: bool,
}

/// The inventories behind the joints, as the game sees them this tick.
pub trait Inventories {
    /// What the block offers to the net, in slot order: for a machine only its products.
    fn offered(&self, block: Pos) -> Vec<(ItemKey, u32)>;
    /// How many of `item` the block can take now.
    fn room_for(&self, block: Pos, item: &str) -> u32;
    /// Slice 18: the block is a machine that takes `item` as an ingredient, room or not. Such an
    /// item is no auto container's (guide `ch_item`: what no receiver needs goes to a chest).
    fn needs(&self, _block: Pos, _item: &str) -> bool {
        false
    }
    /// Slice 18: the block is a machine (it takes only its ingredients): its products go to the
    /// nearest auto container (guide `quickstart` 5, «слитками в правом»).
    fn is_machine(&self, _block: Pos) -> bool {
        false
    }
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
    let mut sources = first_per_block(joints, Mode::gives);
    let receivers = first_per_block(joints, Mode::takes);
    // 0.18.5: a storage on its default joint gives to the load docks of its net (`Joint::store`).
    let loaders: BTreeSet<usize> = receivers.iter().copied().filter(|&r| joints[r].loader).collect();
    if !loaders.is_empty() {
        let given: BTreeSet<Pos> = sources.iter().map(|&j| joints[j].block).collect();
        let mut stores = BTreeSet::new();
        sources.extend((0..joints.len()).filter(|&j| {
            joints[j].store
                && joints[j].mode.takes()
                && !given.contains(&joints[j].block)
                && stores.insert(joints[j].block)
        }));
    }
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
        // A storage gives only to the load docks (`Joint::store`), what their room takes.
        let pulled = src.store && !src.mode.gives();
        for (item, count) in inv.offered(src.block) {
            if pulled && available == 0 {
                break;
            }
            if count == 0 || !src.filter.passes(&item) {
                continue;
            }
            let mut left = count;
            // M17: never back into the block it came from.
            let open = |r: usize| {
                joints[r].block != src.block
                    && joints[r].filter.passes(&item)
                    && (!pulled || loaders.contains(&r))
            };
            // Slice 18: an item a machine on the line takes is no auto container's.
            let machine_needs = receivers
                .iter()
                .any(|&r| !joints[r].auto && open(r) && inv.needs(joints[r].block, &item));
            for ((auto, std::cmp::Reverse(priority)), group) in &by_priority {
                if left == 0 || available == 0 {
                    break;
                }
                if *auto && machine_needs {
                    continue;
                }
                let mut candidates: Vec<usize> = group
                    .iter()
                    .copied()
                    .filter(|&r| open(r) && !(src.auto && joints[r].auto))
                    .collect();
                // Slice 18: a crusher of this priority that takes the item goes before the others.
                let refines = |r: usize| joints[r].refiner && inv.needs(joints[r].block, &item);
                if candidates.iter().any(|&r| refines(r)) {
                    candidates.retain(|&r| refines(r));
                }
                if candidates.is_empty() {
                    continue;
                }
                let turn = state.turn.entry(*priority).or_insert(0);
                let mut gave = 0;
                // A machine's products: auto containers nearest it along the line first, equally
                // near in turn; from a chest, auto containers take turns (M8).
                let nearest = *auto && inv.is_machine(src.block);
                for near in nearest_first(joints, &candidates, src.housing, nearest) {
                    gave += share_in_turn(
                        joints,
                        &near,
                        turn,
                        src.block,
                        &item,
                        &mut left,
                        &mut available,
                        inv,
                        &mut result.transfers,
                    );
                }
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

/// Slice 18 (guide `quickstart` 5, «слитками в правом»): with `nearest`, the candidates grouped
/// by how far their housings are from `from` (the source's), nearest first; else one group.
fn nearest_first(joints: &[Joint], candidates: &[usize], from: Pos, nearest: bool) -> Vec<Vec<usize>> {
    if !nearest {
        return vec![candidates.to_vec()];
    }
    let far = |r: usize| -> i32 {
        let h = joints[r].housing;
        (0..3).map(|i| (h[i] - from[i]).abs()).sum()
    };
    let mut by_far: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    for &r in candidates {
        by_far.entry(far(r)).or_default().push(r);
    }
    by_far.into_values().collect()
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
        /// What a machine takes as an ingredient, room or not (`Inventories::needs`).
        need: HashMap<Pos, BTreeSet<ItemKey>>,
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
        /// A machine's input for `item` with `room` left.
        fn machine_takes(&mut self, block: Pos, item: &str, room: u32) {
            self.takes(block, item, room);
            self.need.entry(block).or_default().insert(item.into());
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
        fn needs(&self, block: Pos, item: &str) -> bool {
            self.need.get(&block).is_some_and(|n| n.contains(item))
        }
        fn is_machine(&self, block: Pos) -> bool {
            self.need.contains_key(&block)
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
            refiner: false,
            store: false,
            loader: false,
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

    /// The guide's quickstart line (`quickstart` 3–5, scenes.js `bus`): the left chest, the
    /// crusher, the furnace and the right chest along one line, no joint set.
    const LEFT: Pos = [10, 0, 0];
    const LINE_CRUSHER: Pos = [11, 0, 0];
    const LINE_FURNACE: Pos = [12, 0, 0];
    const RIGHT: Pos = [13, 0, 0];
    const RAW: &str = "minecraft:raw_iron";
    const DUST: &str = "rustcraft:iron_dust";
    const INGOT: &str = "minecraft:iron_ingot";

    fn quickstart_line() -> [Joint; 4] {
        [
            auto_chest(LEFT),
            Joint { refiner: true, ..joint(LINE_CRUSHER, Mode::Both, 0) },
            joint(LINE_FURNACE, Mode::Both, 0),
            auto_chest(RIGHT),
        ]
    }

    #[test]
    fn ore_goes_to_the_crusher_not_the_furnace() {
        // Guide `quickstart` 5: ore from the left chest → crusher; the furnace smelts ore too, but
        // on a line with a crusher it waits for the dust. A full crusher keeps the rest waiting.
        let mut world = World::default();
        world.gives(LEFT, RAW, 6);
        world.machine_takes(LINE_CRUSHER, RAW, 4);
        world.machine_takes(LINE_FURNACE, RAW, 64);
        world.machine_takes(LINE_FURNACE, DUST, 64);
        world.takes(RIGHT, RAW, 64);
        run(&quickstart_line(), &mut world, 2);
        assert_eq!(world.got(LINE_CRUSHER, RAW), 4);
        assert_eq!(world.got(LINE_FURNACE, RAW), 0, "ore skipped the crusher");
        assert_eq!(world.got(RIGHT, RAW), 0, "ore went from chest to chest");
        assert_eq!(world.left(LEFT, RAW), 2);
    }

    #[test]
    fn dust_waits_for_the_furnace_and_never_goes_into_a_chest() {
        // A product a machine on the line takes is no chest's: it waits while the furnace is full.
        let mut world = World::default();
        world.gives(LINE_CRUSHER, DUST, 6);
        world.machine_takes(LINE_FURNACE, DUST, 2);
        world.takes(LEFT, DUST, 64);
        world.takes(RIGHT, DUST, 64);
        let r = run(&quickstart_line(), &mut world, 2);
        assert_eq!(world.got(LINE_FURNACE, DUST), 2);
        assert_eq!(world.got(LEFT, DUST) + world.got(RIGHT, DUST), 0, "dust in a chest");
        assert_eq!(world.left(LINE_CRUSHER, DUST), 4);
        assert!(r[1].notes.contains(&(2, JointNote::ReceiverFull)), "the furnace is full");
    }

    #[test]
    fn a_chests_items_take_turns_between_auto_chests() {
        // M8 holds for what a chest gives: equally prioritised receivers share, near or far.
        let mut world = World::default();
        world.gives(LEFT, INGOT, 6);
        world.takes(LINE_FURNACE, INGOT, 64);
        world.takes(RIGHT, INGOT, 64);
        let line = [
            joint(LEFT, Mode::From, 0),
            auto_chest(LINE_FURNACE),
            auto_chest(RIGHT),
        ];
        run(&line, &mut world, 1);
        assert_eq!(world.got(LINE_FURNACE, INGOT), 3);
        assert_eq!(world.got(RIGHT, INGOT), 3);
    }

    #[test]
    fn ingots_go_to_the_nearest_chest_then_the_next() {
        // «Она сама станет слитками в правом»: the furnace's product goes to the chest nearest along
        // the line; only when that one is full, on to the next.
        let mut world = World::default();
        world.machine_takes(LINE_FURNACE, DUST, 64);
        world.gives(LINE_FURNACE, INGOT, 5);
        world.takes(LEFT, INGOT, 64);
        world.takes(RIGHT, INGOT, 64);
        run(&quickstart_line(), &mut world, 1);
        assert_eq!(world.got(RIGHT, INGOT), 5);
        assert_eq!(world.got(LEFT, INGOT), 0);
        let mut world = World::default();
        world.machine_takes(LINE_FURNACE, DUST, 64);
        world.gives(LINE_FURNACE, INGOT, 5);
        world.takes(LEFT, INGOT, 64);
        world.takes(RIGHT, INGOT, 3);
        run(&quickstart_line(), &mut world, 1);
        assert_eq!(world.got(RIGHT, INGOT), 3);
        assert_eq!(world.got(LEFT, INGOT), 2, "on to the next chest when the nearest is full");
    }

    #[test]
    fn a_set_furnace_joint_still_takes_ore_before_the_crusher() {
        // The crusher goes first only among receivers of its priority: a player's priority wins.
        let mut world = World::default();
        world.gives(LEFT, RAW, 4);
        world.machine_takes(LINE_CRUSHER, RAW, 64);
        world.machine_takes(LINE_FURNACE, RAW, 64);
        let mut line = quickstart_line();
        line[2].priority = 1;
        run(&line, &mut world, 1);
        assert_eq!(world.got(LINE_FURNACE, RAW), 4);
        assert_eq!(world.got(LINE_CRUSHER, RAW), 0);
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

    #[test]
    fn a_storage_fills_the_load_docks_of_its_net_only() {
        // 0.18.5, guide `dock_load`: «Загружает грузовую вагонетку предметами из магистрали». A
        // storage on its default joint gives to a load dock, never to a chest or a furnace.
        const STORE: Pos = [9, 0, 0];
        const DOCK: Pos = [7, 0, 0];
        let mut world = World::default();
        world.gives(STORE, "minecraft:cobblestone", 20);
        world.takes(STORE, "minecraft:cobblestone", 1000);
        world.takes(DOCK, "minecraft:cobblestone", 8);
        world.takes(CHEST, "minecraft:cobblestone", 64);
        world.takes(FURNACE, "minecraft:cobblestone", 64);
        let store = Joint { store: true, ..joint(STORE, Mode::Into, 0) };
        let dock = Joint { loader: true, ..joint(DOCK, Mode::Both, 0) };
        let joints = [store.clone(), dock.clone(), joint(CHEST, Mode::Into, 0), joint(FURNACE, Mode::Both, 0)];
        run(&joints, &mut world, 40);
        assert_eq!(world.got(DOCK, "minecraft:cobblestone"), 8, "the dock's room");
        assert_eq!(world.got(CHEST, "minecraft:cobblestone"), 0);
        assert_eq!(world.got(FURNACE, "minecraft:cobblestone"), 0);
        // A load dock the player set, or a storage the player set «в блок», trade nothing so.
        let mut world = World::default();
        world.gives(STORE, "minecraft:cobblestone", 20);
        world.takes(DOCK, "minecraft:cobblestone", 8);
        let set = [joint(STORE, Mode::Into, 0), dock.clone()];
        run(&set, &mut world, 10);
        assert_eq!(world.got(DOCK, "minecraft:cobblestone"), 0);
        let set = [store, Joint { loader: false, ..dock }];
        run(&set, &mut world, 10);
        assert_eq!(world.got(DOCK, "minecraft:cobblestone"), 0);
    }
}
