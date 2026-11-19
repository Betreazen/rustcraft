// Tanks, pumps and the fluid nets on the server (spec 4.3, M13, round 15).
//
// A tank `rustcraft:tank` keeps its share of fluid in its block entity's saved compound (`fluid`,
// `amount` in millibuckets); tanks that touch are one tank (rustcraft_rules::net::tanks), its
// fluid laid out again from the bottom up after each change, and its window shows the whole
// group. A pump `rustcraft:pump` is a machine (rustcraft_machines runs it on energy, 4 E/s): it
// takes the source block under it a bucket at a time (rustcraft_rules::machines::pump), keeps
// that bucket until it has given it away, and gives it to a tank it touches or, as a joint «из
// блока», to a fluid channel. Water stays where the game would fill it again; lava and lone water
// are taken out of the world (`native_set_block` to air, native_hooks.rs:10444-10470).
//
// The fluid nets work as the item nets: after each rescan a fluid net's joints are the faces of
// its housings that touch a tank or a pump, with the joint window's direction for that face and
// slot or the block's default (a tank takes, a pump gives); every EVERY ticks
// rustcraft_rules::net::fluids::tick moves fluid between them, each fluid in its own lane at the
// channel's rate, and as many fluids at once as the channel's tier carries (M13). A fluid with no
// lane left is named on its source: «В канале нет места для новой жидкости». The tanks' own fluid
// helpers (`native_block_entity_fluids`) are not used: the mod is the only one that fills these
// tanks, and a group shares one level over many blocks, which the engine's per-block tanks do not.
//
// Round 31 (spec 29, FL1–FL8): one channel carries any number of fluids (the tuning sets no cap),
// and the nets are smart: each block behind a joint has a role (rustcraft_rules::net::fluids::
// Endpoint). Tanks and the storage are stores, on their default joint «both ways»: they take what
// sources give beyond what consumers want, and give to consumers only (a mixer's water, a crystal
// structure's solution, a lava generator's lava), never to one another. The storage is one joint
// at its anchor for all its cells and terminals, as on the item nets; its fluids live in its
// record (rustcraft_storage), read before each move and the change written back after. After each
// move the fluids of the tanks on each storage's nets go to rustcraft_storage, which counts them
// as its stock (FL5). An order's water, the kitchen's milk and the shearer's milk reach the
// storage's cells as they reach tanks (FL6, FL7).
use crate::game::{
    Game,
    rustcraft_content::{self as content, Machine},
    rustcraft_machines::{self as machines, read_f64, read_int, write_f64, write_int},
    rustcraft_mixer::{self as mixer, Tanks as MixerTanks},
    rustcraft_power::{self as power, Sink},
    rustcraft_server::{LevelNet, line},
    rustcraft_storage as storage,
};
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    machines::{
        fluid_windows::{
            FluidShown,
            pump::{self as pump_window, PumpPanel},
            tank::{self as tank_window, Note, TankPanel},
        },
        catalyst::{Catalyst, Tank as CatalystTank},
        processor::{Shown, WorkMode},
        pump::{self, Pump, Source, Under, WATER},
    },
    net::{
        fluids::{self as fluid_net, BUCKET, Endpoint, FluidInventories, FluidKey, FluidNetState, TankPin},
        items::{Joint, Mode},
        joints,
        tanks::{self, TankBlock, TankGroup},
        topology::ChannelKind,
    },
    storage::Store,
    tuning::Tuning,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Fluid nets move every EVERY level ticks, as item nets do.
pub const EVERY: u64 = 4;
/// How long a source stays «В канале нет места для новой жидкости» after the last move that
/// found no lane, in ticks.
const NOTE_HOLD: u64 = 40;
/// The tank windows' numbers are written this often, in ticks.
const WINDOW_EVERY: u64 = 20;
/// The flow «Приходит / Уходит» is counted over this many ticks: a pump gives a bucket every
/// 10 seconds, so a shorter count would show it as nothing most of the time.
const RATE_EVERY: u64 = 200;
const FLUID_KEY: &str = "fluid";
const AMOUNT_KEY: &str = "amount";
/// 0.21 (spec 42, FT1): the fluid the tank's group is kept for; empty while free.
const PIN_KEY: &str = "pin";
const PROGRESS_KEY: &str = "progress";
const HELD_FLUID_KEY: &str = "held_fluid";
const HELD_KEY: &str = "held";
/// A consumer's joint takes by default, a pump's gives.
const TANK_DEFAULT: Mode = Mode::Into;
/// Round 31 (FL3): a store's joint (a tank, the storage) works both ways by its role.
const STORE_DEFAULT: Mode = Mode::Both;
const PUMP_DEFAULT: Mode = Mode::From;
const AIR: &str = "minecraft:air";
/// Blocks that hold no water up under a source.
const LOOSE: [&str; 5] = [
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:void_air",
    "minecraft:lava",
    "minecraft:bubble_column",
];

struct FluidNet {
    key: (usize, Pos),
    joints: Vec<Joint>,
    buckets_per_second: f64,
    kinds: usize,
}

/// One level's tanks and fluid nets.
#[derive(Default)]
pub struct Fluids {
    tanks: BTreeSet<Pos>,
    nets: Vec<FluidNet>,
    states: HashMap<(usize, Pos), FluidNetState>,
    /// Every block behind a fluid joint, with the directions of its joints.
    pub joint_blocks: BTreeMap<Pos, Vec<Mode>>,
    /// Sources whose fluid found no lane, the fluid and until which tick.
    no_lane: HashMap<Pos, (FluidKey, u64)>,
    /// Millibuckets into and out of each tank group (by its anchor) since the window last showed.
    flow: HashMap<Pos, (u64, u64)>,
    /// What each tank group's window showed last, by its anchor: buckets a second in and out.
    rates: HashMap<Pos, (f64, f64)>,
    /// Round 31 (FL4): the anchors of the storages that are joints of a fluid net.
    pub stores: BTreeSet<Pos>,
    /// 0.21 (round 32, SP8): the trashes on a fluid net where a storage limits a fluid: they take
    /// its surplus with no filter.
    pub surplus_sinks: BTreeSet<Pos>,
}

impl Fluids {
    /// Whether the fluid of the source at `pos` (a pump or a tank's block) found no lane lately.
    pub fn no_lane(&self, pos: Pos, now: u64) -> Option<&str> {
        self.no_lane
            .get(&pos)
            .filter(|(_, until)| *until > now)
            .map(|(f, _)| f.as_str())
    }
}

/// The default direction of a fluid joint on `block`: a tank and the storage both ways by their
/// role (round 31), a pump gives, a lava generator and a crystal (its structure's solution) take,
/// a mixer takes water and gives its product.
pub(super) fn default_mode(block: &str) -> Option<Mode> {
    // Round 31 (FL3): tanks and the storage are stores, both ways by their role.
    if block == content::TANK || storage::is_storage_block(block) {
        Some(STORE_DEFAULT)
    } else if block == content::LAVA_GENERATOR || block == content::CRYSTAL {
        Some(TANK_DEFAULT)
    } else if block == content::PUMP {
        Some(PUMP_DEFAULT)
    } else if machines::takes_catalyst(block) {
        // Slice 11: a furnace or a crusher takes its catalyst (K1); slice 18 the smelter and the
        // autocrafter too (the pump, above, gives).
        Some(TANK_DEFAULT)
    } else if block == content::MIXER {
        Some(Mode::Both)
    } else if block == TRASH {
        // Slice 18 (guide `trash`): the trash destroys fluids its joint's filter lets in.
        Some(TANK_DEFAULT)
    } else {
        // Wave 2 (w2-craft): the kitchen and the shearer, at the stream-5 block below.
        craft_default_mode(block)
    }
}

/// Slice 18: the trash block, a fluid sink of the fluid nets.
const TRASH: &str = "rustcraft:trash";

/// Slice 18: how much fluid (mB) each trash of the level may destroy this pass and what a bucket
/// costs it: its tier's rate a second with its speed modules and work mode, in buckets, over the
/// pass, as far as its buffer pays 1 E a bucket (0.6 with an economy module; guide `trash`,
/// rustcraft_rules::machines::tiered::trash_rate).
fn trash_room(game: &Game<'_>, level: &LevelNet, t: &Tuning) -> BTreeMap<Pos, (u64, f64)> {
    level
        .machines
        .iter()
        .filter(|(_, m)| **m == Machine::Util(rustcraft_rules::utility::Util::Trash))
        .map(|(&p, _)| {
            let tier = crate::game::rustcraft_tiers::tier_at(game, p);
            let mode = machines::entity(game, p).map_or(Default::default(), |e| machines::read_work(&e.nbt));
            let slots = crate::game::rustcraft_base::modules_at(game, p);
            let (rate, per) = rustcraft_rules::machines::tiered::trash_rate(tier, &slots, mode, t);
            let pass = (rate * (BUCKET * EVERY) as f64 / 20.0) as u64;
            let paid = (machines::buffer_energy(game, p).max(0.0) / per.max(1e-6) * BUCKET as f64) as u64;
            (p, (pass.min(paid), per))
        })
        .collect()
}

/// Builds the fluid nets' joints again after a rescan.
pub(super) fn rebuild(
    game: &Game<'_>,
    level: &mut LevelNet,
    tanks: BTreeSet<Pos>,
    tuning: &Tuning,
) {
    let fluid_nets = level
        .topology
        .nets
        .iter()
        .filter(|n| n.kind == ChannelKind::Fluid);
    let mut blocks: BTreeMap<Pos, Mode> = BTreeMap::new();
    let mut storage_blocks: BTreeSet<Pos> = BTreeSet::new();
    for net in fluid_nets.clone() {
        for &housing in &net.housings {
            for face in Face::ALL {
                let pos = step(housing, face);
                let block = game.native_block(pos);
                if let Some(mode) = default_mode(&block) {
                    blocks.insert(pos, mode);
                    if storage::is_storage_block(&block) {
                        storage_blocks.insert(pos);
                    }
                }
            }
        }
    }
    let dimension = game.native_dimension().to_owned();
    // Slice 18 (cross, guide `mod_line`): a line-boost module in a node raises the fluid
    // channels it serves too, the net held to its weakest section, as the item nets.
    let boosts = crate::game::rustcraft_node_window::node_modules(game, level.layout.nodes.keys().copied());
    let nets: Vec<FluidNet> = fluid_nets
        .filter_map(|net| {
            // Round 31 (FL4): a storage's cells and terminals are one joint, at its anchor; a
            // terminal that touches no cell is none.
            let joints: Vec<Joint> = joints::item_joints(net, &level.layout, &blocks, &level.joints)
                .into_iter()
                .filter_map(|mut j| {
                    if storage_blocks.contains(&j.block) {
                        j.block = storage::anchor_of(&dimension, j.block)?;
                    }
                    Some(j)
                })
                .collect();
            let first = *net.housings.first()?;
            (!joints.is_empty()).then(|| FluidNet {
                key: (net.slot, first),
                joints,
                buckets_per_second: tuning.fluids_of(net.min_tier)
                    * rustcraft_rules::net::node_window::net_boost(&net.housings, &level.topology.reach, &boosts),
                kinds: tuning.kinds_of(net.min_tier),
            })
        })
        .collect();
    let mut joint_blocks: BTreeMap<Pos, Vec<Mode>> = BTreeMap::new();
    for j in nets.iter().flat_map(|n| n.joints.iter()) {
        let modes = joint_blocks.entry(j.block).or_default();
        if !modes.contains(&j.mode) {
            modes.push(j.mode);
        }
    }
    let anchors: BTreeSet<Pos> = storage_blocks
        .iter()
        .filter_map(|&p| storage::anchor_of(&dimension, p))
        .collect();
    let f = &mut level.fluids;
    f.stores = nets
        .iter()
        .flat_map(|n| n.joints.iter())
        .filter(|j| anchors.contains(&j.block))
        .map(|j| j.block)
        .collect();
    storage::clear_line_fluids_but(&dimension, &f.stores);
    f.joint_blocks = joint_blocks;
    f.states.retain(|key, _| nets.iter().any(|n| n.key == *key));
    f.nets = nets;
    f.tanks = tanks;
}

// ---- The tanks and the pumps as the fluid tick sees them ----

fn read_string(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn read_tank(game: &Game<'_>, pos: Pos) -> Option<TankBlock> {
    let entity = crate::game::rustcraft_machines::entity(game, pos)?;
    let amount = read_int(&entity.nbt, AMOUNT_KEY).map_or(0, |a| a.max(0) as u64);
    let fluid = read_string(&entity.nbt, FLUID_KEY);
    // 0.21 (FT6): a tank from before 0.21 has no pin; rustcraft_rules::net::tanks keeps it for
    // what it holds.
    let pin = read_string(&entity.nbt, PIN_KEY);
    Some(TankBlock { fluid, amount, pin })
}

pub(super) fn write_tank(nbt: &mut Compound, block: &TankBlock) {
    nbt.insert(
        JavaString::from(FLUID_KEY),
        Tag::String(JavaString::from(block.fluid.as_deref().unwrap_or(""))),
    );
    write_int(nbt, AMOUNT_KEY, block.amount.min(i32::MAX as u64) as i32);
    nbt.insert(
        JavaString::from(PIN_KEY),
        Tag::String(JavaString::from(block.pin.as_deref().unwrap_or(""))),
    );
}

/// The self-test's tank: `amount` of `fluid` in the block at `nbt`, kept for that fluid while it
/// holds some; emptied, it keeps the pin it had (FT1).
pub(super) fn set_tank_contents(nbt: &mut Compound, fluid: &str, amount: u64) {
    let pin = if amount > 0 { Some(fluid.to_owned()) } else { read_string(nbt, PIN_KEY) };
    let block = TankBlock { fluid: Some(fluid.to_owned()).filter(|_| amount > 0), amount, pin };
    write_tank(nbt, &block);
}

fn read_held(nbt: &Compound) -> Option<(FluidKey, u64)> {
    let amount = read_int(nbt, HELD_KEY).filter(|&a| a > 0)? as u64;
    match nbt.get(&JavaString::from(HELD_FLUID_KEY)) {
        Some(Tag::String(s)) => Some((s.to_string_lossy(), amount)),
        _ => None,
    }
}

fn write_held(nbt: &mut Compound, held: &Option<(FluidKey, u64)>) {
    let (fluid, amount) = held.as_ref().map_or(("", 0), |(f, a)| (f.as_str(), *a));
    nbt.insert(
        JavaString::from(HELD_FLUID_KEY),
        Tag::String(JavaString::from(fluid)),
    );
    write_int(nbt, HELD_KEY, amount.min(i32::MAX as u64) as i32);
}

/// The level's tanks as groups, from their block entities now.
fn read_groups(game: &Game<'_>, tanks: &BTreeSet<Pos>, per_block: u64) -> Vec<TankGroup> {
    let blocks: BTreeMap<Pos, TankBlock> = tanks
        .iter()
        .filter_map(|&p| Some((p, read_tank(game, p)?)))
        .collect();
    tanks::groups(&blocks, per_block)
}

/// The fluid helpers' view of the tank groups and the pumps' held buckets, for one tick.
struct Inventories<'a> {
    groups: &'a mut [TankGroup],
    /// A tank group's index by its anchor.
    by_anchor: HashMap<Pos, usize>,
    pumps: &'a mut BTreeMap<Pos, Option<(FluidKey, u64)>>,
    flow: &'a mut HashMap<Pos, (u64, u64)>,
    /// Slice 8: lava generators and crystal structures, by their key block.
    sinks: &'a mut BTreeMap<Pos, Sink>,
    /// Slice 10: the mixers' tanks, and the size of each.
    mixers: &'a mut BTreeMap<Pos, MixerTanks>,
    mixer_tank: u64,
    /// Slice 11: the furnaces' and crushers' catalyst tanks.
    catalysts: &'a mut BTreeMap<Pos, CatalystTank>,
    tuning: &'a Tuning,
    /// Slice 18: what each trash may still destroy this pass, mB, and what it destroyed.
    trash: BTreeMap<Pos, u64>,
    destroyed: BTreeMap<Pos, (u64, String)>,
    /// Round 31 (FL4): the storages on the nets by anchor, each its id and its fluid part, and
    /// what a cell holds of each fluid, mB.
    storages: &'a mut BTreeMap<Pos, (u32, Store)>,
    cell_mb: u64,
}

impl FluidInventories for Inventories<'_> {
    fn pin(&self, block: Pos) -> TankPin {
        match self.by_anchor.get(&block).map(|&g| &self.groups[g].pin) {
            Some(Some(f)) => TankPin::For(f.clone()),
            Some(None) => TankPin::Free,
            None => TankPin::NotATank,
        }
    }

    fn endpoint(&self, block: Pos) -> Endpoint {
        if self.storages.contains_key(&block) || self.by_anchor.contains_key(&block) {
            Endpoint::Store
        } else if self.mixers.contains_key(&block) || self.sinks.contains_key(&block) {
            Endpoint::Consumer
        } else if self.trash.contains_key(&block) {
            // 0.21 (round 32, SP8): it takes a storage's surplus with no filter.
            Endpoint::Trash
        } else {
            Endpoint::Plain
        }
    }

    fn surplus(&self, block: Pos, fluid: &str) -> u64 {
        self.storages.get(&block).map_or(0, |(_, s)| s.fluid_surplus(fluid))
    }

    fn at_limit(&self, block: Pos, fluid: &str) -> bool {
        self.storages.get(&block).is_some_and(|(_, s)| s.fluid_at_limit(fluid))
    }

    fn offered(&self, block: Pos) -> Vec<(FluidKey, u64)> {
        if let Some((_, store)) = self.storages.get(&block) {
            return store.fluids.iter().map(|(f, n)| (f.clone(), *n)).collect();
        }
        if let Some(m) = self.mixers.get(&block) {
            return m.product.iter().cloned().collect();
        }
        if let Some(&g) = self.by_anchor.get(&block) {
            let g = &self.groups[g];
            return g.fluid.iter().map(|f| (f.clone(), g.amount)).collect();
        }
        self.pumps
            .get(&block)
            .cloned()
            .flatten()
            .into_iter()
            .collect()
    }

    fn room_for(&self, block: Pos, fluid: &str) -> u64 {
        if let Some((_, store)) = self.storages.get(&block) {
            return store.fluid_room(fluid, self.cell_mb);
        }
        if let Some(room) = self.trash.get(&block) {
            return *room;
        }
        if let Some(c) = self.catalysts.get(&block) {
            return c.room_for(fluid, self.tuning);
        }
        if let Some(s) = self.sinks.get(&block) {
            return s.room_for(fluid);
        }
        if let Some(m) = self.mixers.get(&block) {
            return if fluid == WATER {
                self.mixer_tank.saturating_sub(m.water)
            } else {
                0
            };
        }
        self.by_anchor
            .get(&block)
            .map_or(0, |&g| self.groups[g].room_for(fluid))
    }

    fn take(&mut self, block: Pos, fluid: &str, amount: u64) -> u64 {
        if let Some((_, store)) = self.storages.get_mut(&block) {
            return store.take_fluid(fluid, amount);
        }
        if let Some(m) = self.mixers.get_mut(&block) {
            return take_product(m, fluid, amount);
        }
        if let Some(&g) = self.by_anchor.get(&block) {
            let n = self.groups[g].take(fluid, amount);
            self.flow.entry(block).or_default().1 += n;
            return n;
        }
        let Some(held) = self.pumps.get_mut(&block) else {
            return 0;
        };
        let n = match held.as_mut() {
            Some((f, have)) if f.as_str() == fluid => {
                let n = (*have).min(amount);
                *have -= n;
                n
            }
            _ => return 0,
        };
        if held.as_ref().is_some_and(|(_, left)| *left == 0) {
            *held = None;
        }
        n
    }

    fn put(&mut self, block: Pos, fluid: &str, amount: u64) -> u64 {
        if let Some((_, store)) = self.storages.get_mut(&block) {
            return store.put_fluid(fluid, amount, self.cell_mb);
        }
        if let Some(room) = self.trash.get_mut(&block) {
            let n = (*room).min(amount);
            *room -= n;
            let d = self.destroyed.entry(block).or_insert_with(|| (0, fluid.to_owned()));
            d.0 += n;
            d.1 = fluid.to_owned();
            return n;
        }
        // Slice 18 (cross): only a catalyst goes into a machine's catalyst tank, so a pump (which
        // has one now) still takes its own refused bucket back below.
        if let Some(c) = self.catalysts.get_mut(&block)
            && Catalyst::of(fluid).is_some()
        {
            return c.put(fluid, amount, self.tuning);
        }
        if let Some(s) = self.sinks.get_mut(&block) {
            let n = s.room_for(fluid).min(amount);
            s.held += n;
            return n;
        }
        if let Some(m) = self.mixers.get_mut(&block) {
            if fluid == WATER {
                let n = self.mixer_tank.saturating_sub(m.water).min(amount);
                m.water += n;
                return n;
            }
            // Only its own product goes back into it (a move the target refused).
            return match m.product.as_mut() {
                Some((f, have)) if f.as_str() == fluid => {
                    *have += amount;
                    amount
                }
                None => {
                    m.product = Some((fluid.to_owned(), amount));
                    amount
                }
                _ => 0,
            };
        }
        if let Some(&g) = self.by_anchor.get(&block) {
            let n = self.groups[g].put(fluid, amount);
            self.flow.entry(block).or_default().0 += n;
            return n;
        }
        // Only a pump's own bucket goes back into it (a move the target refused).
        match self.pumps.get_mut(&block) {
            Some(held @ None) => {
                *held = Some((fluid.to_owned(), amount));
                amount
            }
            Some(Some((f, have))) if f.as_str() == fluid => {
                *have += amount;
                amount
            }
            _ => 0,
        }
    }
}

/// Up to `amount` of a mixer's product `fluid` taken out of it.
fn take_product(m: &mut MixerTanks, fluid: &str, amount: u64) -> u64 {
    let Some((f, have)) = m.product.as_mut() else {
        return 0;
    };
    if f.as_str() != fluid {
        return 0;
    }
    let n = (*have).min(amount);
    *have -= n;
    if *have == 0 {
        m.product = None;
    }
    n
}

/// Slice 10: each mixer drinks water from a water tank it touches and gives its product to the
/// tank on top of it.
fn mixers_and_tanks(
    mixers: &mut BTreeMap<Pos, MixerTanks>,
    groups: &mut [TankGroup],
    group_of: &HashMap<Pos, usize>,
    flow: &mut HashMap<Pos, (u64, u64)>,
    size: u64,
) {
    for (&pos, m) in mixers.iter_mut() {
        for face in Face::ALL {
            let Some(&g) = group_of.get(&step(pos, face)) else {
                continue;
            };
            let room = size.saturating_sub(m.water);
            if room > 0 && groups[g].fluid.as_deref() == Some(WATER) {
                let n = groups[g].take(WATER, room);
                flow.entry(groups[g].anchor()).or_default().1 += n;
                m.water += n;
            }
        }
        let Some(&g) = group_of.get(&step(pos, Face::YPos)) else {
            continue;
        };
        if let Some((fluid, amount)) = m.product.clone() {
            let n = groups[g].put(&fluid, amount);
            flow.entry(groups[g].anchor()).or_default().0 += n;
            take_product(m, &fluid, n);
        }
    }
}

/// Slice 11: each furnace and crusher takes a catalyst from a tank it touches (K1), one catalyst at
/// a time (K2).
fn catalysts_from_tanks(
    catalysts: &mut BTreeMap<Pos, CatalystTank>,
    groups: &mut [TankGroup],
    group_of: &HashMap<Pos, usize>,
    flow: &mut HashMap<Pos, (u64, u64)>,
    tuning: &Tuning,
) {
    for (&pos, c) in catalysts.iter_mut() {
        for face in Face::ALL {
            let Some(&g) = group_of.get(&step(pos, face)) else {
                continue;
            };
            let Some(fluid) = groups[g].fluid.clone().filter(|f| Catalyst::of(f).is_some()) else {
                continue;
            };
            let room = c.room_for(&fluid, tuning);
            if room == 0 {
                continue;
            }
            let n = groups[g].take(&fluid, room);
            flow.entry(groups[g].anchor()).or_default().1 += n;
            c.put(&fluid, n, tuning);
        }
    }
}

/// One move of the level's fluids: pumps into the tanks they touch, then every fluid net.
pub(super) fn tick(game: &mut Game<'_>, level: &mut LevelNet, tuning: &Tuning) {
    let now = level.ticks;
    // Slice 18: the trashes' room this pass, read before the nets borrow the level.
    let trash_rooms = trash_room(game, level, tuning);
    let trash: BTreeMap<Pos, u64> = trash_rooms.iter().map(|(p, (room, _))| (*p, *room)).collect();
    let pumps_at: Vec<Pos> = level
        .machines
        .iter()
        .filter(|(_, m)| **m == Machine::Pump)
        .map(|(p, _)| *p)
        .collect();
    let mixers_at: Vec<Pos> = level
        .machines
        .iter()
        .filter(|(_, m)| **m == Machine::Mixer)
        .map(|(p, _)| *p)
        .collect();
    // Slice 11: the machines that take a catalyst.
    let catalysts_at: Vec<Pos> = level
        .machines
        .iter()
        // Slice 18 (cross): every machine that takes one (rustcraft_machines::takes_catalyst).
        .filter(|(_, m)| machines::takes_catalyst(m.block()))
        .map(|(p, _)| *p)
        .collect();
    let dimension = game.native_dimension().to_owned();
    let f = &mut level.fluids;
    if f.tanks.is_empty() && pumps_at.is_empty() && mixers_at.is_empty() && f.stores.is_empty() {
        return;
    }
    // Round 31 (FL4): the storages' fluids as they are now.
    let mut storages: BTreeMap<Pos, (u32, Store)> = f
        .stores
        .iter()
        .filter_map(|&anchor| Some((anchor, storage::fluid_store(&dimension, anchor)?)))
        .collect();
    let storages_before = storages.clone();
    let per_block = tuning.tank_buckets * BUCKET;
    let mut groups = read_groups(game, &f.tanks, per_block);
    let before = groups.clone();
    let mut pumps: BTreeMap<Pos, Option<(FluidKey, u64)>> = pumps_at
        .iter()
        .filter_map(|&p| {
            let entity = crate::game::rustcraft_machines::entity(game, p)?;
            Some((p, read_held(&entity.nbt)))
        })
        .collect();
    let pumps_before = pumps.clone();
    let group_of: HashMap<Pos, usize> = groups
        .iter()
        .enumerate()
        .flat_map(|(i, g)| g.blocks.iter().map(move |&b| (b, i)))
        .collect();
    let by_anchor: HashMap<Pos, usize> = groups
        .iter()
        .enumerate()
        .map(|(i, g)| (g.anchor(), i))
        .collect();
    // Slice 8: lava generators and crystal structures drink from the tanks they touch.
    let sinks_before = power::sinks(game, &level.sources, tuning);
    let mut sinks: BTreeMap<Pos, Sink> = sinks_before.iter().map(|s| (s.key, s.clone())).collect();
    let sink_of: HashMap<Pos, Pos> = sinks
        .values()
        .flat_map(|s| s.blocks.iter().map(move |&b| (b, s.key)))
        .collect();
    for s in sinks.values_mut() {
        for &b in &s.blocks.clone() {
            for face in Face::ALL {
                let Some(&g) = group_of.get(&step(b, face)) else {
                    continue;
                };
                let room = s.room_for(groups[g].fluid.as_deref().unwrap_or(""));
                if room == 0 {
                    continue;
                }
                let fluid = s.fluid;
                let n = groups[g].take(fluid, room);
                f.flow.entry(groups[g].anchor()).or_default().1 += n;
                s.held += n;
            }
        }
    }
    // A pump gives its bucket straight to a tank it touches.
    for (&pump, held) in pumps.iter_mut() {
        for face in Face::ALL {
            let Some(&g) = group_of.get(&step(pump, face)) else {
                continue;
            };
            let Some((fluid, amount)) = held.clone() else {
                break;
            };
            let n = groups[g].put(&fluid, amount);
            f.flow.entry(groups[g].anchor()).or_default().0 += n;
            *held = (amount > n).then(|| (fluid, amount - n));
        }
    }
    let mixer_tank = mixer::tank_size(tuning);
    let mut mixers: BTreeMap<Pos, MixerTanks> = mixers_at
        .iter()
        .filter_map(|&p| Some((p, mixer::read_tanks(game, p)?)))
        .collect();
    let mixers_before = mixers.clone();
    mixers_and_tanks(&mut mixers, &mut groups, &group_of, &mut f.flow, mixer_tank);
    let mut catalysts: BTreeMap<Pos, CatalystTank> = catalysts_at
        .iter()
        .filter_map(|&p| Some((p, machines::catalyst_at(game, p)?)))
        .collect();
    let catalysts_before = catalysts.clone();
    catalysts_from_tanks(&mut catalysts, &mut groups, &group_of, &mut f.flow, tuning);
    let moves_per_second = tuning.ticks_per_second / EVERY as f64;
    let mut inv = Inventories {
        groups: &mut groups,
        by_anchor,
        pumps: &mut pumps,
        flow: &mut f.flow,
        sinks: &mut sinks,
        mixers: &mut mixers,
        mixer_tank,
        catalysts: &mut catalysts,
        tuning,
        trash,
        destroyed: BTreeMap::new(),
        storages: &mut storages,
        cell_mb: tuning.storage_cell_buckets * BUCKET,
    };
    f.no_lane.retain(|_, (_, until)| *until > now);
    let mut surplus_sinks = BTreeSet::new();
    for net in &f.nets {
        // A joint on any block of a tank group is a joint on the whole group.
        let joints: Vec<Joint> = net
            .joints
            .iter()
            .map(|j| {
                let mut j = j.clone();
                if let Some(&g) = group_of.get(&j.block) {
                    j.block = inv.groups[g].anchor();
                }
                if let Some(&key) = sink_of.get(&j.block) {
                    j.block = key;
                }
                j
            })
            .collect();
        if joints.len() < 2 {
            continue;
        }
        // 0.21 (SP8): a storage of the net limits a fluid: its trashes take the surplus.
        if joints.iter().any(|j| inv.storages.get(&j.block).is_some_and(|(_, s)| !s.limits.is_empty())) {
            surplus_sinks.extend(joints.iter().map(|j| j.block).filter(|b| inv.trash.contains_key(b)));
        }
        let state = f.states.entry(net.key).or_default();
        let result = fluid_net::tick(
            &joints,
            net.buckets_per_second,
            net.kinds,
            moves_per_second,
            state,
            &mut inv,
        );
        for n in result.no_room {
            f.no_lane
                .insert(joints[n.joint].block, (n.fluid, now + NOTE_HOLD));
        }
    }
    f.surplus_sinks = surplus_sinks;
    // Slice 18: each trash pays 1 E a bucket it destroyed.
    let destroyed = std::mem::take(&mut inv.destroyed);
    for (pos, (mb, fluid)) in destroyed {
        let dimension = game.native_dimension().to_owned();
        crate::game::rustcraft_utility::note_fluid_burnt(&dimension, pos, &fluid, mb);
        let per = trash_rooms.get(&pos).map_or(1.0, |(_, per)| *per);
        let e = machines::buffer_energy(game, pos);
        machines::set_buffer_energy(game, pos, e - mb as f64 / BUCKET as f64 * per);
    }
    for (anchor, (id, store)) in &storages {
        if let Some((_, was)) = storages_before.get(anchor) {
            storage::apply_fluids(&dimension, *id, was, store);
        }
    }
    hand_over_line_fluids(&dimension, &level.fluids, &groups, &group_of);
    write_back(game, &before, &groups, per_block);
    power::write_sinks(game, &sinks_before, &sinks);
    for (pos, m) in &mixers {
        if mixers_before.get(pos) != Some(m) {
            mixer::write_tanks(game, *pos, m);
        }
    }
    for (pos, c) in &catalysts {
        if catalysts_before.get(pos) != Some(c) {
            machines::set_catalyst(game, *pos, c);
        }
    }
    for (pos, held) in &pumps {
        if pumps_before.get(pos) != Some(held) {
            game.native_with_mod_block_entity(*pos, |e| write_held(&mut e.nbt, held));
        }
    }
    if now.is_multiple_of(WINDOW_EVERY) {
        show_windows(game, level, &groups);
    }
}

/// Round 31 (FL5): each storage on a fluid net learns what the tanks on its nets hold: every tank
/// group behind a joint that gives (by default, or set «из блока») on a net the storage is a joint
/// of, each group once.
fn hand_over_line_fluids(
    dimension: &str,
    f: &Fluids,
    groups: &[TankGroup],
    group_of: &HashMap<Pos, usize>,
) {
    for &anchor in &f.stores {
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        for net in &f.nets {
            if !net.joints.iter().any(|j| j.block == anchor && j.mode != Mode::Off) {
                continue;
            }
            seen.extend(
                net.joints
                    .iter()
                    .filter(|j| j.mode.gives())
                    .filter_map(|j| group_of.get(&j.block).copied()),
            );
        }
        let mut fluids: BTreeMap<String, u64> = BTreeMap::new();
        for g in seen.into_iter().map(|i| &groups[i]) {
            if let Some(fluid) = g.fluid.as_ref().filter(|_| g.amount > 0) {
                *fluids.entry(fluid.clone()).or_insert(0) += g.amount;
            }
        }
        storage::set_line_fluids(dimension, anchor, fluids);
    }
}

/// Writes each tank block whose share changed.
fn write_back(game: &mut Game<'_>, before: &[TankGroup], after: &[TankGroup], per_block: u64) {
    for (b, a) in before.iter().zip(after) {
        if b.amount == a.amount && b.fluid == a.fluid && b.pin == a.pin {
            continue;
        }
        let old: HashMap<Pos, TankBlock> = b.shares(per_block).into_iter().collect();
        for (pos, share) in a.shares(per_block) {
            if old.get(&pos) != Some(&share) {
                game.native_with_mod_block_entity(pos, |e| write_tank(&mut e.nbt, &share));
                // Slice 12: the fluid's height and colour on the tank's model.
                let level = share.amount as f64 / per_block.max(1) as f64;
                let colour = share.fluid.as_deref().map_or(0, rustcraft_rules::anim::fluid_tint);
                crate::game::rustcraft_anim::show_level_cached(game, pos, level, colour);
            }
        }
    }
}

/// Every WINDOW_EVERY ticks: each tank's window data, the whole group's numbers.
fn show_windows(game: &mut Game<'_>, level: &mut LevelNet, groups: &[TankGroup]) {
    let now = level.ticks;
    let f = &mut level.fluids;
    if now.is_multiple_of(RATE_EVERY) {
        let seconds = RATE_EVERY as f64 / 20.0;
        let flow = std::mem::take(&mut f.flow);
        f.rates = flow
            .into_iter()
            .map(|(anchor, (into, out))| {
                let per = |mb: u64| mb as f64 / BUCKET as f64 / seconds;
                (anchor, (per(into), per(out)))
            })
            .collect();
    }
    for g in groups {
        let (coming, going) = f.rates.get(&g.anchor()).copied().unwrap_or((0.0, 0.0));
        let note = if f.no_lane(g.anchor(), now).is_some() {
            Note::NoLane
        } else if g.mixed {
            Note::Mixed
        } else {
            Note::None
        };
        let data = tank_window::encode(&TankPanel {
            fluid: FluidShown::of(g.fluid.as_deref()),
            pin: FluidShown::of(g.pin.as_deref()),
            amount: g.amount,
            capacity: g.capacity,
            blocks: g.blocks.len() as u32,
            coming,
            going,
            note,
        });
        for &pos in &g.blocks {
            let shown = crate::game::rustcraft_machines::entity(game, pos).map(|e| e.data.clone());
            if shown.as_ref() != Some(&data) {
                game.native_with_mod_block_entity(pos, |e| e.data = data.clone());
            }
        }
    }
}

// ---- The pump, run by rustcraft_machines on its share of energy ----

/// A pump as read this tick.
pub struct PumpHeld {
    pump: Pump,
    under: Under,
    pub request: f64,
    data: Vec<i32>,
    /// Slice 18: its modules and tier, and the ticks and E/s a bucket takes on them.
    fitted: machines::Fitted,
    ticks: f64,
    energy: f64,
    mode: WorkMode,
    catalyst: CatalystTank,
}

/// A8, A7: a higher tier and speed modules pump faster; economy modules cost less. Owner
/// 2026-11-17: its work mode too (A1, `pump::numbers`).
fn pump_numbers(fitted: &machines::Fitted, mode: WorkMode, catalyst: &CatalystTank, tuning: &Tuning) -> (f64, f64) {
    let tier = fitted.speed(Machine::Pump, tuning);
    let (work, draw) = rustcraft_rules::modules::factors(&fitted.modules, tuning);
    // Slice 18 (cross, guide `fluids`): the accelerator and the economizer act on it too.
    let (cat_work, cat_draw) = machines::catalyst_factors(content::PUMP, catalyst, tuning);
    let modules = (work * cat_work, draw * cat_draw);
    pump::numbers(tuning.pump_ticks, tuning.pump_energy, tier, modules, mode)
}

/// What is under the pump at `pos`.
fn under(game: &Game<'_>, pos: Pos) -> Under {
    let below = [pos[0], pos[1] - 1, pos[2]];
    let is_source = |p: Pos, block: &str| {
        game.native_block(p) == block
            && game.native_block_property(p, "level").as_deref() == Some("0")
    };
    let source = if is_source(below, WATER) {
        Some(Source::Water)
    } else if is_source(below, Source::Lava.fluid()) {
        Some(Source::Lava)
    } else {
        None
    };
    let water_beside = [Face::XNeg, Face::XPos, Face::ZNeg, Face::ZPos]
        .into_iter()
        .filter(|&f| is_source(step(below, f), WATER))
        .count() as u8;
    let under_source = [below[0], below[1] - 1, below[2]];
    let block = game.native_block(under_source);
    let held_up = if block == WATER {
        is_source(under_source, WATER)
    } else {
        !LOOSE.contains(&block.as_str())
    };
    Under {
        source,
        water_beside,
        held_up,
    }
}

pub(super) fn read_pump(game: &Game<'_>, pos: Pos, tuning: &Tuning) -> Option<PumpHeld> {
    let entity = crate::game::rustcraft_machines::entity(game, pos)?;
    let pump = Pump {
        progress: read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0),
        held: read_held(&entity.nbt),
    };
    let under = under(game, pos);
    let fitted = machines::fitted(game, pos, entity);
    let mode = machines::read_work(&entity.nbt);
    let catalyst = machines::read_catalyst(&entity.nbt);
    let (ticks, energy) = pump_numbers(&fitted, mode, &catalyst, tuning);
    Some(PumpHeld {
        request: pump.request(&under, energy),
        pump,
        under,
        data: entity.data.clone(),
        fitted,
        ticks,
        energy,
        mode,
        catalyst,
    })
}

/// Runs the pump on `share` of its request: a bucket pumped takes its source out of the world
/// unless the game fills it again. What it shows.
pub(super) fn run_pump(
    game: &mut Game<'_>,
    pos: Pos,
    held: PumpHeld,
    share: f64,
    no_lane: bool,
    tuning: &Tuning,
) -> Option<Shown> {
    let PumpHeld {
        mut pump,
        under,
        request,
        data: shown_data,
        fitted,
        ticks,
        energy,
        mode,
        mut catalyst,
    } = held;
    let before = pump.clone();
    let tick = pump.tick(&under, share, ticks);
    let catalyst_before = catalyst.clone();
    if tick.state == rustcraft_rules::machines::processor::State::Working {
        catalyst.spend(1.0 / tuning.ticks_per_second, share, tuning);
    }
    let shown = Pump::shown(tick, share);
    if let Some(source) = tick.pumped
        && !under.endless()
    {
        take_source(game, pos, source);
    }
    let data = pump_window::encode(&PumpPanel {
        under: under.source,
        endless: under.endless(),
        progress: pump.progress,
        got: if request > 0.0 { share * request } else { 0.0 },
        asked: energy,
        shown,
        no_lane,
    });
    let mut data = data;
    data.extend(fitted.extras(game, &[], 0, &None));
    data.push(ticks.round() as i32);
    data.push(pump_window::mode_slot(mode));
    if pump == before && shown_data == data && fitted.tier_saved() && catalyst == catalyst_before {
        return Some(shown);
    }
    game.native_with_mod_block_entity(pos, move |e| {
        fitted.save_tier(&mut e.nbt);
        if catalyst != catalyst_before {
            machines::write_catalyst(&mut e.nbt, &catalyst);
        }
        if pump.progress != before.progress {
            write_f64(&mut e.nbt, PROGRESS_KEY, pump.progress);
        }
        if pump.held != before.held {
            write_held(&mut e.nbt, &pump.held);
        }
        if e.data != data {
            e.data = data;
        }
        shown
    })
}

/// The source under the pump at `pos` taken out of the world.
fn take_source(game: &mut Game<'_>, pos: Pos, source: Source) {
    let below = [pos[0], pos[1] - 1, pos[2]];
    let dimension = game.native_dimension();
    let Some(air) = game.native_default_state(AIR) else {
        return;
    };
    // Flags 3: neighbours hear it and the players see it, as `Level.setBlock` with UPDATE_ALL.
    if !game.native_set_block(dimension, below, air, 3) {
        crate::game::rustcraft_debug::journal_warn!(
            "rustcraft: the {} source under the pump at {pos:?} stayed",
            source.fluid()
        );
    }
}

// ---- What a look shows ----

/// A fluid's name in the player's language.
pub(super) fn fluid_name(fluid: &str) -> Text {
    match fluid {
        WATER => line("rustcraft.fluid.water", "water", &[]),
        "minecraft:lava" => line("rustcraft.fluid.lava", "lava", &[]),
        "rustcraft:solution" => line("rustcraft.fluid.solution", "nutrient solution", &[]),
        "rustcraft:accelerator" => line("rustcraft.fluid.accelerator", "accelerator", &[]),
        "rustcraft:economizer" => line("rustcraft.fluid.economizer", "economizer", &[]),
        "rustcraft:enricher" => line("rustcraft.fluid.enricher", "enricher", &[]),
        other => Text::literal(other.to_owned()),
    }
}

/// «В канале нет места для новой жидкости: лава» for a source whose fluid found no lane.
pub(super) fn no_lane_text(fluid: &str) -> Text {
    line(
        "rustcraft.fluid.no_lane",
        "No room in the channel for another fluid: %s",
        &[fluid_name(fluid)],
    )
}

/// The tank group the tank block at `pos` belongs to, as the fluid tick reads it.
pub(super) fn group_at(
    game: &Game<'_>,
    level: &LevelNet,
    pos: Pos,
    tuning: &Tuning,
) -> Option<TankGroup> {
    let per_block = tuning.tank_buckets * BUCKET;
    read_groups(game, &level.fluids.tanks, per_block)
        .into_iter()
        .find(|g| g.blocks.contains(&pos))
}

/// Round 32 (spec 33): what the fluid nets over `housings` reach, for the terminal's «Сеть» page
/// (rustcraft_terminal_net): each tank group behind one of their joints, once, and every other
/// block behind one; each with whether its joints there take into it and give from it.
#[derive(Default)]
pub(super) struct Lane {
    pub tanks: Vec<(TankGroup, (bool, bool))>,
    pub ends: Vec<(Pos, (bool, bool))>,
}

/// The fluid nets whose housings meet `housings`, as the last rescan built their joints, with the
/// tank groups as their block entities hold them now.
pub(super) fn lane_of(game: &Game<'_>, level: &LevelNet, housings: &BTreeSet<Pos>, tuning: &Tuning) -> Lane {
    let firsts: BTreeSet<Pos> = level
        .topology
        .nets
        .iter()
        .filter(|n| n.kind == ChannelKind::Fluid && !n.housings.is_disjoint(housings))
        .filter_map(|n| n.housings.first().copied())
        .collect();
    let mut modes: BTreeMap<Pos, (bool, bool)> = BTreeMap::new();
    for net in level.fluids.nets.iter().filter(|n| firsts.contains(&n.key.1)) {
        for j in &net.joints {
            let m = modes.entry(j.block).or_default();
            m.0 |= j.mode.takes();
            m.1 |= j.mode.gives();
        }
    }
    if modes.is_empty() {
        return Lane::default();
    }
    let mut tanks = Vec::new();
    let mut in_tanks: BTreeSet<Pos> = BTreeSet::new();
    for g in read_groups(game, &level.fluids.tanks, tuning.tank_buckets * BUCKET) {
        let Some(m) = g
            .blocks
            .iter()
            .filter_map(|b| modes.get(b).copied())
            .reduce(|a, b| (a.0 | b.0, a.1 | b.1))
        else {
            continue;
        };
        in_tanks.extend(g.blocks.iter().copied());
        tanks.push((g, m));
    }
    let ends = modes.into_iter().filter(|(p, _)| !in_tanks.contains(p)).collect();
    Lane { tanks, ends }
}

/// Round 31 (FL5, FL6; slice 15, O12): the fluid stock (mB by fluid) an order of the storage
/// `owner` (its id and anchor) can draw on, with the mixers `mixers` on its line: what the
/// storage's cells hold; the tank groups `stock_tanks_near` finds, each once; and the water in the
/// mixers' own tanks.
pub(super) fn line_stock(
    game: &Game<'_>,
    level: &LevelNet,
    owner: Option<(u32, Pos)>,
    mixers: &[Pos],
    tuning: &Tuning,
) -> BTreeMap<String, u64> {
    let dimension = game.native_dimension().to_owned();
    let mut stock = owner.map_or_else(BTreeMap::new, |(id, _)| storage::own_fluids(&dimension, id));
    let near = stock_tanks_near(level, owner.map(|(_, anchor)| anchor), mixers);
    let per_block = tuning.tank_buckets * BUCKET;
    for g in read_groups(game, &level.fluids.tanks, per_block) {
        if g.amount == 0 || !g.blocks.iter().any(|b| near.contains(b)) {
            continue;
        }
        if let Some(fluid) = g.fluid {
            *stock.entry(fluid).or_insert(0) += g.amount;
        }
    }
    let water: u64 = mixers
        .iter()
        .filter_map(|&m| mixer::read_tanks(game, m))
        .map(|t| t.water)
        .sum();
    if water > 0 {
        *stock.entry(WATER.to_owned()).or_insert(0) += water;
    }
    stock
}

/// The blocks whose tank groups an order's fluid comes from: those touching one of `mixers`, and
/// those behind a joint that gives (by default or «из блока») on a fluid net a mixer takes from or
/// the storage at `owner` is a joint of (FL5).
fn stock_tanks_near(level: &LevelNet, owner: Option<Pos>, mixers: &[Pos]) -> BTreeSet<Pos> {
    let mut near: BTreeSet<Pos> = mixers
        .iter()
        .flat_map(|&m| Face::ALL.into_iter().map(move |f| step(m, f)))
        .collect();
    for net in &level.fluids.nets {
        let mixer_takes = net
            .joints
            .iter()
            .any(|j| mixers.contains(&j.block) && j.mode.takes());
        let storage_on = owner
            .is_some_and(|a| net.joints.iter().any(|j| j.block == a && j.mode != Mode::Off));
        if mixer_takes || storage_on {
            near.extend(net.joints.iter().filter(|j| j.mode.gives()).map(|j| j.block));
        }
    }
    near
}

// ---- Slice 18 (stream 5): the kitchen and the shearer use the tank they touch ----
//
// Wave 2 (guide `kitchen`, `shearer`: «из бака жидкостным каналом», «в бак жидкостным каналом»):
// and the tanks of a fluid net they are joints of. The kitchen's joint takes by default, the
// shearer's gives (`craft_default_mode`); the kitchen draws from a tank whose joint gives (From or
// Both), the shearer fills one whose joint takes (Into, the tank's default, or Both), as a mixer's
// order brings its water (`bring_water`). Their joints carry none of the net's own flow: the
// net's inventories give them no room and nothing offered.

/// Wave 2: the default fluid joint of the kitchen (takes milk) and the shearer (gives it).
fn craft_default_mode(block: &str) -> Option<Mode> {
    use rustcraft_rules::utility::Util;
    if block == Util::Kitchen.spec().block {
        Some(Mode::Into)
    } else if block == Util::Shearer.spec().block {
        Some(Mode::From)
    } else if block == Util::PortalDock.spec().block {
        // Wave 2 (guide `portal_dock`): it takes fluid in and gives what came from its pair.
        Some(Mode::Both)
    } else {
        None
    }
}

// ---- Wave 2: the portal dock's fluid (guide `portal_dock`: «предметы и жидкости», 16 вёдер) ----
//
// A dock sends the fluid of the tanks behind a joint that gives (From) on a fluid net it is a
// joint of, and unloads what came from its pair into a tank it touches or one behind a joint that
// takes (Into, Both) on such a net. A tank it only touches is not taken from, so two docks never
// hand one tank's fluid back and forth.

/// The blocks the dock at `pos` takes fluid from (`sink` false) or gives it to.
fn dock_near(level: &LevelNet, pos: Pos, sink: bool) -> BTreeSet<Pos> {
    let mut near: BTreeSet<Pos> = if sink {
        Face::ALL.into_iter().map(|f| step(pos, f)).collect()
    } else {
        BTreeSet::new()
    };
    for net in &level.fluids.nets {
        if !net.joints.iter().any(|j| j.block == pos && j.mode != Mode::Off) {
            continue;
        }
        near.extend(
            net.joints
                .iter()
                .filter(|j| j.block != pos)
                .filter(|j| if sink { matches!(j.mode, Mode::Into | Mode::Both) } else { j.mode == Mode::From })
                .map(|j| j.block),
        );
    }
    near
}

/// Wave 2: the fluids (mB) the dock at `pos` may send now.
pub(super) fn dock_offer(game: &Game<'_>, level: &LevelNet, pos: Pos, tuning: &Tuning) -> Vec<(String, u64)> {
    let near = dock_near(level, pos, false);
    if near.is_empty() {
        return Vec::new();
    }
    read_groups(game, &level.fluids.tanks, tuning.tank_buckets * BUCKET)
        .into_iter()
        .filter(|g| g.blocks.iter().any(|b| near.contains(b)))
        .filter_map(|g| Some((g.fluid.clone()?, g.amount)).filter(|(_, n)| *n > 0))
        .collect()
}

/// Wave 2: the dock at `pos` takes up to `amount` of `fluid` from the tanks it sends from (or
/// puts it into those it unloads to, `sink`): how much.
pub(super) fn dock_move(game: &mut Game<'_>, level: &LevelNet, pos: Pos, fluid: &str, amount: u64, sink: bool, tuning: &Tuning) -> u64 {
    let near = dock_near(level, pos, sink);
    let per_block = tuning.tank_buckets * BUCKET;
    let before = read_groups(game, &level.fluids.tanks, per_block);
    let mut after = before.clone();
    let mut done = 0;
    let reach = |g: &TankGroup| g.blocks.iter().any(|b| near.contains(b));
    // 0.21 (FT3): unloading, the tanks kept for the fluid first, a free one only without them.
    let order: Vec<usize> = if sink {
        tanks::fill_order(&after, reach, fluid)
    } else {
        (0..after.len()).filter(|&i| reach(&after[i])).collect()
    };
    for i in order {
        if done >= amount {
            break;
        }
        let g = &mut after[i];
        done += if sink { g.put(fluid, amount - done) } else { g.take(fluid, amount - done) };
    }
    if done > 0 {
        write_back(game, &before, &after, per_block);
    }
    done
}

/// Wave 2: the blocks a craft machine at `pos` reaches on the fluid nets it is a joint of: to take
/// (`fill` false) the joints that give, to fill the joints that take.
fn net_reach(level: &LevelNet, pos: Pos, fill: bool) -> BTreeSet<Pos> {
    let (mine, theirs) = if fill { (Mode::Into, Mode::From) } else { (Mode::From, Mode::Into) };
    let mut out = BTreeSet::new();
    for net in &level.fluids.nets {
        if net.joints.iter().any(|j| j.block == pos && j.mode != mine && j.mode != Mode::Off) {
            out.extend(
                net.joints
                    .iter()
                    .filter(|j| j.block != pos && j.mode != theirs && j.mode != Mode::Off)
                    .map(|j| j.block),
            );
        }
    }
    out
}

/// The tank group touching `pos`, or on a fluid net it is a joint of (wave 2), that holds `fluid`
/// or is empty (for `fill`), and its index.
fn touching_group(game: &Game<'_>, level: &LevelNet, pos: Pos, fluid: &str, fill: bool, tuning: &Tuning) -> Option<(Vec<TankGroup>, usize)> {
    let per_block = tuning.tank_buckets * BUCKET;
    let groups = read_groups(game, &level.fluids.tanks, per_block);
    let mut near: BTreeSet<Pos> = Face::ALL.into_iter().map(|f| step(pos, f)).collect();
    near.extend(net_reach(level, pos, fill));
    let reach = |g: &TankGroup| g.blocks.iter().any(|b| near.contains(b));
    let i = if fill {
        // 0.21 (FT3): the tank kept for the fluid with room first; a free one only without one.
        let order = tanks::fill_order(&groups, reach, fluid);
        order.iter().copied().find(|&i| groups[i].room_for(fluid) > 0).or(order.first().copied())?
    } else {
        groups.iter().position(|g| reach(g) && g.fluid.as_deref() == Some(fluid))?
    };
    Some((groups, i))
}

/// Round 31 (FL7): the storages on the fluid nets `pos` is a joint of that reach it: to take
/// (`fill` false) those whose joint gives, to fill those whose joint takes; by anchor.
fn net_storages(level: &LevelNet, pos: Pos, fill: bool) -> Vec<Pos> {
    net_reach(level, pos, fill)
        .into_iter()
        .filter(|b| level.fluids.stores.contains(b))
        .collect()
}

/// How much `fluid` (mB) the tanks touching `pos` hold, and their room for it; round 31 (FL7):
/// with the storages on its fluid line.
pub(super) fn touching_amount(game: &Game<'_>, level: &LevelNet, pos: Pos, fluid: &str, tuning: &Tuning) -> (u64, u64) {
    let held = touching_group(game, level, pos, fluid, false, tuning).map_or(0, |(g, i)| g[i].amount);
    let room = touching_group(game, level, pos, fluid, true, tuning).map_or(0, |(g, i)| g[i].room_for(fluid));
    let dimension = game.native_dimension();
    let cell_mb = tuning.storage_cell_buckets * BUCKET;
    let stored: u64 = net_storages(level, pos, false)
        .into_iter()
        .filter_map(|a| storage::fluid_store(dimension, a))
        .map(|(_, s)| s.fluid(fluid))
        .sum();
    let store_room = net_storages(level, pos, true)
        .into_iter()
        .filter_map(|a| storage::fluid_store(dimension, a))
        .fold(0u64, |n, (_, s)| n.saturating_add(s.fluid_room(fluid, cell_mb)));
    // A storage that destroys its surplus has room without end; the windows add these up.
    (held + stored, room.saturating_add(store_room).min(u64::from(u32::MAX)))
}

/// Takes up to `amount` of `fluid` from a tank touching `pos`, then (round 31, FL7) from the
/// storages on its fluid line: how much it took.
pub(super) fn take_touching(game: &mut Game<'_>, level: &LevelNet, pos: Pos, fluid: &str, amount: u64, tuning: &Tuning) -> u64 {
    let mut took = 0;
    if let Some((before, i)) = touching_group(game, level, pos, fluid, false, tuning) {
        let mut after = before.clone();
        took = after[i].take(fluid, amount);
        write_back(game, &before, &after, tuning.tank_buckets * BUCKET);
    }
    let dimension = game.native_dimension().to_owned();
    for anchor in net_storages(level, pos, false) {
        if took >= amount {
            break;
        }
        if let Some((id, _)) = storage::fluid_store(&dimension, anchor) {
            took += storage::take_fluid(&dimension, id, fluid, amount - took);
        }
    }
    took
}

/// Puts up to `amount` of `fluid` into a tank touching `pos`, then (round 31, FL7) into the
/// storages on its fluid line: how much went in.
pub(super) fn put_touching(game: &mut Game<'_>, level: &LevelNet, pos: Pos, fluid: &str, amount: u64, tuning: &Tuning) -> u64 {
    let mut went = 0;
    if let Some((before, i)) = touching_group(game, level, pos, fluid, true, tuning) {
        let mut after = before.clone();
        went = after[i].put(fluid, amount);
        write_back(game, &before, &after, tuning.tank_buckets * BUCKET);
    }
    let dimension = game.native_dimension().to_owned();
    let cell_mb = tuning.storage_cell_buckets * BUCKET;
    for anchor in net_storages(level, pos, true) {
        if went >= amount {
            break;
        }
        if let Some((id, _)) = storage::fluid_store(&dimension, anchor) {
            went += storage::put_fluid(&dimension, id, fluid, amount - went, cell_mb);
        }
    }
    went
}

/// Slice 18 (stream 5, O12 handed over from stream 1): «заказ приносит свою воду»: an order's
/// mixer step takes its water into the mixer's own tank, as far as it has room. Round 31 (FL6):
/// from the cells of the order's storage `owner` (its id and anchor) first, straight in as items
/// go where no net joins the two, then from the tanks `line_stock` counts. How much came.
pub(super) fn bring_water(
    game: &mut Game<'_>,
    level: &LevelNet,
    mixer_at: Pos,
    amount: u64,
    owner: Option<(u32, Pos)>,
    tuning: &Tuning,
) -> u64 {
    let Some(mut tanks) = mixer::read_tanks(game, mixer_at) else {
        return 0;
    };
    let room = mixer::tank_size(tuning).saturating_sub(tanks.water).min(amount);
    if room == 0 {
        return 0;
    }
    let dimension = game.native_dimension().to_owned();
    let mut got = owner.map_or(0, |(id, _)| storage::take_fluid(&dimension, id, WATER, room));
    if got < room {
        let near = stock_tanks_near(level, owner.map(|(_, anchor)| anchor), &[mixer_at]);
        let per_block = tuning.tank_buckets * BUCKET;
        let before = read_groups(game, &level.fluids.tanks, per_block);
        let mut after = before.clone();
        for g in after.iter_mut() {
            if got >= room {
                break;
            }
            if g.fluid.as_deref() == Some(WATER) && g.blocks.iter().any(|b| near.contains(b)) {
                got += g.take(WATER, room - got);
            }
        }
        write_back(game, &before, &after, per_block);
    }
    tanks.water += got;
    mixer::write_tanks(game, mixer_at, &tanks);
    got
}

/// A tank's diagnosis: «Бак: вода, 20 из 32 вёдер», its joints and its note.
pub(super) fn tank_text(game: &Game<'_>, level: &LevelNet, pos: Pos, tuning: &Tuning) -> Text {
    let Some(g) = group_at(game, level, pos, tuning) else {
        return line("rustcraft.tank.empty", "Tank: empty", &[]);
    };
    let buckets = |mb: u64| Text::literal(bucket_text(mb));
    let mut text = match &g.fluid {
        Some(f) => line(
            "rustcraft.tank.holds",
            "Tank: %s, %s of %s buckets",
            &[fluid_name(f), buckets(g.amount), buckets(g.capacity)],
        ),
        None => line(
            "rustcraft.tank.empty_of",
            "Tank: empty, %s buckets",
            &[buckets(g.capacity)],
        ),
    };
    // 0.21 (FT5): what the empty tank is kept for, or that it is free.
    if g.fluid.is_none() {
        text.push(Text::literal(" · "));
        text.push(match &g.pin {
            Some(f) => line(
                "rustcraft.tank.kept_for",
                "for %s (sneak and click it with the wrench to free it)",
                &[fluid_name(f)],
            ),
            None => line("rustcraft.tank.free", "free: takes the next new fluid", &[]),
        });
    }
    if let Some(fluid) = level.fluids.no_lane(g.anchor(), level.ticks) {
        text.push(Text::literal(" · "));
        text.push(no_lane_text(fluid));
    } else if g.mixed {
        text.push(Text::literal(" · "));
        text.push(line(
            "rustcraft.tank.mixed",
            "a tank with another fluid touches it",
            &[],
        ));
    }
    text
}

/// 0.21 (spec 42, FT4): the sneaking wrench on a tank frees its empty group for any fluid; a tank
/// that holds a fluid stays kept for it. Tells `player` what happened.
pub(super) fn wrench_frees_tank(game: &mut Game<'_>, player: u128, dimension: &str, pos: Pos) {
    let tuning = crate::game::rustcraft_server::tuning();
    let group = crate::game::rustcraft_server::with_level(dimension, |level| {
        group_at(game, level, pos, tuning)
    })
    .flatten();
    let text = match group {
        None => line("rustcraft.tank.no_group", "The tank is not ready yet", &[]),
        Some(g) => {
            let mut after = g.clone();
            if after.free() {
                write_back(game, &[g], &[after], tuning.tank_buckets * BUCKET);
                line("rustcraft.tank.freed", "The tank is free: it takes the next new fluid", &[])
            } else if let Some(f) = &g.fluid {
                line("rustcraft.tank.free_not_empty", "Empty the tank first: it holds %s", &[fluid_name(f)])
            } else {
                line("rustcraft.tank.free_already", "The tank is free already", &[])
            }
        }
    };
    crate::game::rustcraft_server::tell(game, player, text);
}

/// Buckets as the guide writes them: «20», «12,8».
pub(super) fn bucket_text(mb: u64) -> String {
    let tenths = (mb + 50) / 100;
    if tenths % 10 == 0 {
        (tenths / 10).to_string()
    } else {
        format!("{},{}", tenths / 10, tenths % 10)
    }
}

// ---- Slice 18, stream 3: a bucket on a tank (guide `tank`) ----

/// A bucket `held` used on the tank at `pos`: the tank group changes and the bucket the player
/// holds after it; why not, else (a translation key's tail and its fallback).
pub(super) fn bucket_on_tank(
    game: &mut Game<'_>,
    dimension: &str,
    pos: Pos,
    held: &str,
) -> Result<&'static str, (&'static str, &'static str)> {
    use rustcraft_rules::machines::bucket::{self, Use};
    let tuning = crate::game::rustcraft_server::tuning();
    let per_block = tuning.tank_buckets * BUCKET;
    let group = crate::game::rustcraft_server::with_level(dimension, |level| {
        group_at(game, level, pos, tuning)
    })
    .flatten()
    .ok_or(("tank.no_group", "The tank is not ready yet"))?;
    let mut after = group.clone();
    let result = match bucket::use_on(held, &group) {
        Use::Pour { fluid } => {
            after.put(fluid, BUCKET);
            bucket::EMPTY
        }
        Use::Fill { fluid, filled } => {
            after.take(fluid, BUCKET);
            filled
        }
        Use::NoRoom => return Err(("tank.bucket_no_room", "No room in the tank for a bucket of it")),
        Use::Empty => return Err(("tank.bucket_empty", "Less than a bucket in the tank")),
        Use::NoBucket(_) => {
            return Err(("tank.bucket_no_bucket", "No bucket takes it: it lives in tanks and channels"));
        }
        Use::Other => return Err(("", "")),
    };
    write_back(game, &[group], &[after], per_block);
    Ok(result)
}

/// `native_right_click_block`: a water, lava or empty bucket on a tank pours or fills, before
/// the tank's window opens.
pub(super) fn on_right_click_block(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    if *canceled || game.native_block(pos) != content::TANK {
        return;
    }
    let Some(player) = crate::game::rustcraft_server::acting(game) else {
        return;
    };
    let Some((held, _, true)) = game.native_held() else {
        return;
    };
    if rustcraft_rules::machines::bucket::use_on(&held, &Default::default())
        == rustcraft_rules::machines::bucket::Use::Other
    {
        return;
    }
    *canceled = true;
    match bucket_on_tank(game, &player.dimension, pos, &held) {
        Ok(result) => {
            if player.game_mode != 1 {
                game.native_take_items(&held, 1);
                if game.native_give_items(result, 1) == 0 {
                    crate::game::rustcraft_debug::journal_warn!("rustcraft: no room for {result} from the tank at {pos:?}");
                }
            }
        }
        Err((key, fallback)) if !key.is_empty() => {
            crate::game::rustcraft_server::tell(
                game,
                player.uuid,
                line(&format!("rustcraft.{key}"), fallback, &[]),
            );
        }
        Err(_) => {}
    }
}
