// Item transport along item channel nets (M6–M8, M16, M17). After each rescan the joints of
// every item net are built from the topology, the blocks around it and the settings each housing
// keeps for its faces and slots (rustcraft_rules::net::joints::item_joints, M7): a working
// housing's face that touches a block with slots is a joint, with that face's and slot's
// direction, filter and priority, or the block's default. Every EVERY ticks each net reads its
// joints' slots into a snapshot (rustcraft_rules::net::stock), runs
// rustcraft_rules::net::items::tick on it and makes the moves it decided with the game's own
// mover, `native_move_items`, which moves one item at a time and leaves in its slot what the
// target refuses (SRC/crates/horizon-client/src/game/machines/native_items.rs:171-232): no item
// is lost.
//
// Defaults: a machine works on its joints both ways (M17), a vanilla container takes («в
// блок»). Slice 2 set one mode per block with the wrench: the furnace kept it in its block
// entity (`joint`), a vanilla container in a level attachment (`AttachmentType` with
// `Holder::Level`, SRC/docs/native-mods.md:4970-5025; written to the world's
// dimensions/<ns>/<dim>/data/horizon/attachments.dat). Slice 3 reads those as the default of
// that block's joints and writes them no more: the joint window (rustcraft_window) keeps every
// setting in the housing, per face and per slot.
//
// A storage (rustcraft_storage) is a joint too, on any of its cells or a terminal that touches
// it, standing at its first cell so that the net counts it once. It is no container: the net
// plans on its counts (rustcraft_rules::net::stock::NetStore) and the moves to and from it go
// through rustcraft_storage, which takes a container's stacks out with `native_extract_item` only
// as far as the storage has room, and gives with `native_insert_item` (native_hooks.rs:
// 11166-11177), keeping what the target refuses. By default it takes («в блок», M16).
use crate::game::{
    Game,
    rustcraft_content::{self as content, Machine},
    rustcraft_machines::slot_takes,
    rustcraft_server::{self as server, LevelNet},
    rustcraft_storage as storage, rustcraft_window as window,
};
use horizon_client::{
    mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Holder, Host},
    text::Text,
};
use horizon_core::geometry::Direction;
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    net::{
        items::{self, Joint, JointNote, Mode, NetState, Transfer},
        joints,
        stock::{self, Stock, StockSlot, Stocks},
        topology::ChannelKind,
    },
    storage::{keys, terminal},
    tuning::Tuning,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Item nets move every EVERY level ticks, with the per-second budget spread over the moves.
pub const EVERY: u64 = 4;
/// How long a receiver stays «Приёмник полон» after the last move that found it full, in ticks.
const NOTE_HOLD: u64 = 40;
/// Slice 2's furnace kept its joint mode under this key, as `Mode::code`.
const LEGACY_JOINT_KEY: &str = "joint";
/// M17: a machine takes its input and gives its products through one joint.
const MACHINE_DEFAULT: Mode = Mode::Both;
/// A chest beside the magistral is filled until its joint says otherwise.
const CONTAINER_DEFAULT: Mode = Mode::Into;
/// A storage takes what the net brings (M16: «ненужное уходит туда»).
const STORAGE_DEFAULT: Mode = Mode::Into;

/// Slice 2's joint modes of the level's vanilla containers, saved with the level: read as those
/// blocks' defaults, forgotten when a block breaks.
static JOINT_MODES: AttachmentType<BTreeMap<Pos, Mode>> = AttachmentType {
    namespace: "rustcraft",
    name: "joint_modes",
    default: BTreeMap::new,
    codec: Some(AttachmentCodec {
        encode: encode_modes,
        decode: decode_modes,
    }),
    copy_on_death: false,
    sync: false,
};
const MODES_KEY: &str = "modes";

struct ItemNet {
    /// The net's slot and its first housing: what keeps its throughput and turns over rescans.
    key: (usize, Pos),
    joints: Vec<Joint>,
    per_second: f64,
}

/// One level's item nets and their joints.
#[derive(Default)]
pub struct Transport {
    nets: Vec<ItemNet>,
    states: HashMap<(usize, Pos), NetState>,
    /// Every block behind a joint of some item net, with the directions of its joints.
    pub joint_blocks: BTreeMap<Pos, Vec<Mode>>,
    /// Receivers a move found full, until which level tick.
    full_until: HashMap<Pos, u64>,
}

impl Transport {
    /// Whether the receiver at `pos` was full on a recent move.
    pub fn is_full(&self, pos: Pos, now: u64) -> bool {
        self.full_until.get(&pos).is_some_and(|&until| until > now)
    }
}

/// The side of the block that `face` names (`ModContainer.faces` and the item helpers take a
/// `Direction`: down, up, north, south, west, east; north is z−, west is x−).
pub(super) fn direction(face: Face) -> Direction {
    match face {
        Face::XNeg => Direction::West,
        Face::XPos => Direction::East,
        Face::YNeg => Direction::Down,
        Face::YPos => Direction::Up,
        Face::ZNeg => Direction::North,
        Face::ZPos => Direction::South,
    }
}

/// The side of a joint's block that looks at its housing.
fn block_side(joint: &Joint) -> Direction {
    direction(joint.face.opposite())
}

/// Builds the item nets' joints again from the level's topology, the blocks around it and the
/// housings' settings.
pub(super) fn rebuild(game: &mut Game<'_>, dimension: &str, level: &mut LevelNet, tuning: &Tuning) {
    let saved = load_modes(game);
    let mut inventories: BTreeMap<Pos, Mode> = BTreeMap::new();
    let mut checked: BTreeSet<Pos> = BTreeSet::new();
    let item_nets = || {
        level
            .topology
            .nets
            .iter()
            .filter(|n| n.kind == ChannelKind::Item)
    };
    for net in item_nets() {
        for &housing in &net.housings {
            for face in Face::ALL {
                let block = step(housing, face);
                let pipe = level.layout.housings.contains_key(&block)
                    || level.layout.nodes.contains_key(&block);
                if pipe || !checked.insert(block) {
                    continue;
                }
                let side = direction(face.opposite());
                if let Some(mode) = default_mode(game, dimension, block, side, &saved) {
                    inventories.insert(block, mode);
                }
            }
        }
    }
    let nets: Vec<ItemNet> = item_nets()
        .filter_map(|net| {
            let joints: Vec<Joint> =
                joints::item_joints(net, &level.layout, &inventories, &level.joints)
                    .into_iter()
                    .filter_map(|mut j| {
                        // A storage's cells and terminals are one joint, at its first cell; a
                        // terminal that touches no cell is none.
                        if storage::is_storage_block(&game.native_block(j.block)) {
                            j.block = storage::anchor_of(dimension, j.block)?;
                        }
                        Some(j)
                    })
                    .collect();
            let first = *net.housings.first()?;
            (!joints.is_empty()).then(|| ItemNet {
                key: (net.slot, first),
                joints,
                per_second: tuning.items_of(net.min_tier),
            })
        })
        .collect();
    let t = &mut level.transport;
    let mut joint_blocks: BTreeMap<Pos, Vec<Mode>> = BTreeMap::new();
    for j in nets.iter().flat_map(|n| n.joints.iter()) {
        let modes = joint_blocks.entry(j.block).or_default();
        if !modes.contains(&j.mode) {
            modes.push(j.mode);
        }
    }
    for modes in joint_blocks.values_mut() {
        modes.sort();
    }
    t.joint_blocks = joint_blocks;
    t.states.retain(|key, _| nets.iter().any(|n| n.key == *key));
    t.nets = nets;
}

/// The default direction of the joints at `pos` if it has slots a joint can reach: a machine's
/// slice-2 mode or both ways, a vanilla container's slice-2 mode or «в блок».
pub(super) fn default_mode(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    side: Direction,
    saved: &BTreeMap<Pos, Mode>,
) -> Option<Mode> {
    let block = game.native_block(pos);
    if let Some(machine) = Machine::of(&block) {
        // The pump has no slot a net reaches.
        return machine
            .has_item_slots()
            .then(|| legacy_machine_mode(game, pos).unwrap_or(MACHINE_DEFAULT));
    }
    if storage::is_storage_block(&block) {
        return Some(STORAGE_DEFAULT);
    }
    if block.starts_with("rustcraft:") {
        return None;
    }
    // Slots through that side: a chest, a barrel, a hopper, a vanilla furnace (native_items.rs:110-136).
    game.native_container_slots(dimension, pos, side)?;
    Some(saved.get(&pos).copied().unwrap_or(CONTAINER_DEFAULT))
}

/// The slice-2 modes of the level's vanilla containers.
pub(super) fn legacy_modes(game: &mut Game<'_>) -> BTreeMap<Pos, Mode> {
    load_modes(game)
}

/// One move of every item net of the level in place.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &mut LevelNet, tuning: &Tuning) {
    let now = level.ticks;
    let moves_per_second = tuning.ticks_per_second / EVERY as f64;
    let LevelNet {
        transport,
        machines,
        ..
    } = level;
    let Transport {
        nets,
        states,
        full_until,
        ..
    } = transport;
    full_until.retain(|_, until| *until > now);
    for net in nets.iter() {
        if net.joints.len() < 2 {
            continue;
        }
        let mut stocks = snapshot(game, dimension, &net.joints, machines, tuning);
        let stores: BTreeSet<Pos> = stocks.stores.keys().copied().collect();
        let state = states.entry(net.key).or_default();
        let result = items::tick(
            &net.joints,
            net.per_second,
            moves_per_second,
            state,
            &mut stocks,
        );
        apply(game, dimension, &net.joints, &stores, &result.transfers);
        for (j, note) in result.notes {
            if note == JointNote::ReceiverFull {
                full_until.insert(net.joints[j].block, now + NOTE_HOLD);
            }
        }
    }
}

/// One slot a joint reaches: its number, what it holds, whether the net may take it.
type ReadSlot = (usize, Option<(String, u32)>, bool);

/// The slots behind the joints as the game holds them now. A machine takes only into its inputs
/// and into each only what that slot takes (M17, rustcraft_machines::slot_takes: the furnace
/// what it smelts, a fuel slot fuel, the autocrafter its pages' ingredients, one kind a slot); a
/// vanilla container is taken to take anything in any slot, and the mover refuses what it
/// doesn't (a vanilla furnace's fuel slot).
fn snapshot(
    game: &Game<'_>,
    dimension: &str,
    joints: &[Joint],
    machines: &BTreeMap<Pos, Machine>,
    tuning: &Tuning,
) -> Stocks {
    let mut read: BTreeMap<Pos, Vec<ReadSlot>> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stocks = Stocks::default();
    for joint in joints {
        if read.contains_key(&joint.block) || stocks.stores.contains_key(&joint.block) {
            continue;
        }
        if let Some(store) = storage::net_store(dimension, joint.block) {
            seen.extend(store.store.items.keys().cloned());
            stocks.stores.insert(joint.block, store);
            continue;
        }
        let Some(slots) = game.native_container_slots(dimension, joint.block, block_side(joint))
        else {
            continue;
        };
        let slots: Vec<ReadSlot> = slots
            .into_iter()
            .map(|s| {
                // Keyed as the storage keys stacks: a worn one apart, one with components marked.
                let held = s.stack.map(|st| {
                    let marked = st.extra.is_some() || !terminal::addressable(st.damage);
                    (keys::stack_key(&st.item, st.damage, marked), st.count)
                });
                if let Some((item, _)) = &held {
                    seen.insert(item.clone());
                }
                (s.slot, held, s.can_take)
            })
            .collect();
        read.insert(joint.block, slots);
    }
    for (pos, slots) in read {
        let machine = machines.get(&pos).copied();
        let entity = machine.and_then(|_| game.native_mod_block_entity(pos));
        let slots = slots
            .into_iter()
            .map(|(slot, held, can_take)| {
                let input = machine.is_none_or(|m| m.inputs().contains(&slot));
                let only = entity.as_ref().filter(|_| input).map(|entity| {
                    seen.iter()
                        .filter(|item| slot_takes(entity, slot, keys::item_of(item), tuning))
                        .cloned()
                        .collect()
                });
                let (item, count) = held.map_or((None, 0), |(i, n)| (Some(i), n));
                StockSlot {
                    item,
                    count,
                    can_take,
                    can_put: input,
                    only,
                }
            })
            .collect();
        stocks.blocks.insert(
            pos,
            Stock {
                slots,
                accepts: None,
            },
        );
    }
    for item in seen {
        // A storage's key names a worn item after its id.
        let stack = game
            .items()
            .get(keys::item_of(&item))
            .map_or(stock::DEFAULT_STACK, |info| info.max_stack);
        stocks.max_stack.insert(item, stack);
    }
    stocks
}

/// Makes the moves the net decided, through the sides the joints touch; a move to or from a
/// storage (at its anchor, one of `stores`) goes through rustcraft_storage.
fn apply(
    game: &mut Game<'_>,
    dimension: &str,
    joints: &[Joint],
    stores: &BTreeSet<Pos>,
    transfers: &[Transfer],
) {
    let side_of = |block: Pos| joints.iter().find(|j| j.block == block).map(block_side);
    for ((from, to, item), count) in stock::totals(transfers) {
        let (Some(from_side), Some(to_side)) = (side_of(from), side_of(to)) else {
            continue;
        };
        let moved = match (stores.contains(&from), stores.contains(&to)) {
            (true, true) => storage::between_storages(game, dimension, from, to, &item, count),
            (true, false) => {
                storage::out_of_storage(game, dimension, from, to, to_side, &item, count)
            }
            (false, true) => {
                storage::into_storage(game, dimension, from, from_side, to, &item, count)
            }
            (false, false) => game.native_move_items(
                dimension,
                from,
                from_side,
                to,
                to_side,
                Some(keys::item_of(&item)),
                count,
            ),
        };
        if moved < count {
            log::debug!("rustcraft: {moved} of {count} {item} moved from {from:?} to {to:?}");
        }
    }
}

// ---- The wrench ----

/// `native_right_click_block` (native_hooks.rs:8300-8314, before the block's own use and the
/// item's, SRC/crates/horizon-client/src/game.rs:6735-6750): the wrench, not sneaking, on a
/// housing opens its joint window on the face clicked (or its first joint); on a machine or a
/// container beside the magistral it opens the window of the first housing with a channel that
/// touches it, on the face that looks at it. The click does nothing else (a machine's window and
/// a chest's lid stay shut). With sneaking the wrench takes a channel out of a housing instead
/// (rustcraft_server::on_use_item_on).
pub(super) fn on_right_click_block(
    game: &mut Game<'_>,
    pos: Pos,
    face: [i32; 3],
    canceled: &mut bool,
) {
    if *canceled {
        return;
    }
    // The acting player's main hand (native_server.rs:1031-1037).
    let Some((held, _, _)) = game.native_held() else {
        return;
    };
    if held != content::WRENCH {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    if player.sneaking {
        return;
    }
    let target = if game.native_block(pos) == content::HOUSING {
        Some((pos, Face::from_normal(face)))
    } else {
        server::with_level(&player.dimension, |level| {
            Face::ALL.into_iter().find_map(|f| {
                let housing = step(pos, f);
                let held = level.layout.housings.get(&housing)?;
                held.slots
                    .iter()
                    .any(Option::is_some)
                    .then_some((housing, Some(f.opposite())))
            })
        })
        .flatten()
    };
    let Some((housing, preferred)) = target else {
        return;
    };
    *canceled = true;
    window::request_open(player.uuid, &player.dimension, housing, preferred);
}

/// A vanilla container broken: its slice-2 mode goes with it.
pub(super) fn forget(game: &mut Game<'_>, pos: Pos) {
    let mut saved = load_modes(game);
    if saved.remove(&pos).is_some() {
        save_modes(game, saved);
    }
}

/// The slice-2 mode a machine kept in its block entity, if any.
fn legacy_machine_mode(game: &Game<'_>, pos: Pos) -> Option<Mode> {
    let entity = game.native_mod_block_entity(pos)?;
    match entity.nbt.get(&JavaString::from(LEGACY_JOINT_KEY)) {
        Some(Tag::Byte(code)) => Mode::from_code(*code as u8),
        _ => None,
    }
}

fn load_modes(game: &mut Game<'_>) -> BTreeMap<Pos, Mode> {
    // `Game` is not an `AttachmentHost`; `dyn Host` is (SRC/docs/native-mods.md:4842-4866).
    let host: &mut dyn Host = game;
    JOINT_MODES.get(host, Holder::Level).unwrap_or_else(|e| {
        log::warn!("rustcraft: joint modes not read: {e:?}");
        BTreeMap::new()
    })
}

fn save_modes(game: &mut Game<'_>, modes: BTreeMap<Pos, Mode>) {
    let host: &mut dyn Host = game;
    if let Err(e) = JOINT_MODES.set(host, Holder::Level, modes) {
        log::warn!("rustcraft: joint modes not saved: {e:?}");
    }
}

/// The modes as one int array: x, y, z, `Mode::code` for each block.
fn encode_modes(modes: &BTreeMap<Pos, Mode>) -> Result<Compound, AttachmentError> {
    let flat: Vec<i32> = modes
        .iter()
        .flat_map(|(p, m)| [p[0], p[1], p[2], i32::from(m.code())])
        .collect();
    Ok(Compound::from([(
        JavaString::from(MODES_KEY),
        Tag::IntArray(flat),
    )]))
}

/// What `encode_modes` wrote; a block with a mode it does not know keeps its default.
fn decode_modes(data: &Compound) -> Result<BTreeMap<Pos, Mode>, AttachmentError> {
    let Some(Tag::IntArray(flat)) = data.get(&JavaString::from(MODES_KEY)) else {
        return Ok(BTreeMap::new());
    };
    Ok(flat
        .chunks_exact(4)
        .filter_map(|c| {
            let mode = Mode::from_code(u8::try_from(c[3]).ok()?)?;
            Some(([c[0], c[1], c[2]], mode))
        })
        .collect())
}

// ---- Text ----

pub(super) fn mode_name(mode: Mode) -> Text {
    match mode {
        Mode::Into => server::line("rustcraft.mode.into", "into the block", &[]),
        Mode::From => server::line("rustcraft.mode.from", "out of the block", &[]),
        Mode::Both => server::line("rustcraft.mode.both", "both ways", &[]),
        Mode::Off => server::line("rustcraft.mode.off", "off", &[]),
    }
}

/// «Стык: из блока», or «Стык: в блок, из блока» for a block whose joints differ.
pub(super) fn joint_text(modes: &[Mode]) -> Text {
    let mut names = Text::literal("");
    for (i, &mode) in modes.iter().enumerate() {
        if i > 0 {
            names.push(Text::literal(", "));
        }
        names.push(mode_name(mode));
    }
    server::line("rustcraft.joint.mode", "Joint: %s", &[names])
}
