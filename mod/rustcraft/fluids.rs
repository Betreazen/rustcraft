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
use crate::game::{
    Game,
    rustcraft_content::{self as content, Machine},
    rustcraft_machines::{read_f64, read_int, write_f64, write_int},
    rustcraft_power::{self as power, Sink},
    rustcraft_server::{LevelNet, line},
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
        processor::Shown,
        pump::{Pump, Source, Under, WATER},
    },
    net::{
        fluids::{self as fluid_net, BUCKET, FluidInventories, FluidKey, FluidNetState},
        items::{Joint, Mode},
        joints,
        tanks::{self, TankBlock, TankGroup},
        topology::ChannelKind,
    },
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
const PROGRESS_KEY: &str = "progress";
const HELD_FLUID_KEY: &str = "held_fluid";
const HELD_KEY: &str = "held";
/// A tank joint takes by default, a pump's gives.
const TANK_DEFAULT: Mode = Mode::Into;
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

/// The default direction of a fluid joint on `block`: a tank takes, a pump gives, a lava generator
/// and a crystal (its structure's solution) take.
pub(super) fn default_mode(block: &str) -> Option<Mode> {
    if block == content::TANK || block == content::LAVA_GENERATOR || block == content::CRYSTAL {
        Some(TANK_DEFAULT)
    } else if block == content::PUMP {
        Some(PUMP_DEFAULT)
    } else {
        None
    }
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
    for net in fluid_nets.clone() {
        for &housing in &net.housings {
            for face in Face::ALL {
                let pos = step(housing, face);
                if let Some(mode) = default_mode(&game.native_block(pos)) {
                    blocks.insert(pos, mode);
                }
            }
        }
    }
    let nets: Vec<FluidNet> = fluid_nets
        .filter_map(|net| {
            let joints = joints::item_joints(net, &level.layout, &blocks, &level.joints);
            let first = *net.housings.first()?;
            (!joints.is_empty()).then(|| FluidNet {
                key: (net.slot, first),
                joints,
                buckets_per_second: tuning.fluids_of(net.min_tier),
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
    let f = &mut level.fluids;
    f.joint_blocks = joint_blocks;
    f.states.retain(|key, _| nets.iter().any(|n| n.key == *key));
    f.nets = nets;
    f.tanks = tanks;
}

// ---- The tanks and the pumps as the fluid tick sees them ----

fn read_tank(game: &Game<'_>, pos: Pos) -> Option<TankBlock> {
    let entity = game.native_mod_block_entity(pos)?;
    let amount = read_int(&entity.nbt, AMOUNT_KEY).map_or(0, |a| a.max(0) as u64);
    let fluid = match entity.nbt.get(&JavaString::from(FLUID_KEY)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    };
    Some(TankBlock { fluid, amount })
}

pub(super) fn write_tank(nbt: &mut Compound, block: &TankBlock) {
    nbt.insert(
        JavaString::from(FLUID_KEY),
        Tag::String(JavaString::from(block.fluid.as_deref().unwrap_or(""))),
    );
    write_int(nbt, AMOUNT_KEY, block.amount.min(i32::MAX as u64) as i32);
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
}

impl FluidInventories for Inventories<'_> {
    fn offered(&self, block: Pos) -> Vec<(FluidKey, u64)> {
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
        if let Some(s) = self.sinks.get(&block) {
            return s.room_for(fluid);
        }
        self.by_anchor
            .get(&block)
            .map_or(0, |&g| self.groups[g].room_for(fluid))
    }

    fn take(&mut self, block: Pos, fluid: &str, amount: u64) -> u64 {
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
        if let Some(s) = self.sinks.get_mut(&block) {
            let n = s.room_for(fluid).min(amount);
            s.held += n;
            return n;
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

/// One move of the level's fluids: pumps into the tanks they touch, then every fluid net.
pub(super) fn tick(game: &mut Game<'_>, level: &mut LevelNet, tuning: &Tuning) {
    let now = level.ticks;
    let pumps_at: Vec<Pos> = level
        .machines
        .iter()
        .filter(|(_, m)| **m == Machine::Pump)
        .map(|(p, _)| *p)
        .collect();
    let f = &mut level.fluids;
    if f.tanks.is_empty() && pumps_at.is_empty() {
        return;
    }
    let per_block = tuning.tank_buckets * BUCKET;
    let mut groups = read_groups(game, &f.tanks, per_block);
    let before = groups.clone();
    let mut pumps: BTreeMap<Pos, Option<(FluidKey, u64)>> = pumps_at
        .iter()
        .filter_map(|&p| {
            let entity = game.native_mod_block_entity(p)?;
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
    let moves_per_second = tuning.ticks_per_second / EVERY as f64;
    let mut inv = Inventories {
        groups: &mut groups,
        by_anchor,
        pumps: &mut pumps,
        flow: &mut f.flow,
        sinks: &mut sinks,
    };
    f.no_lane.retain(|_, (_, until)| *until > now);
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
    write_back(game, &before, &groups, per_block);
    power::write_sinks(game, &sinks_before, &sinks);
    for (pos, held) in &pumps {
        if pumps_before.get(pos) != Some(held) {
            game.native_with_mod_block_entity(*pos, |e| write_held(&mut e.nbt, held));
        }
    }
    if now.is_multiple_of(WINDOW_EVERY) {
        show_windows(game, level, &groups);
    }
}

/// Writes each tank block whose share changed.
fn write_back(game: &mut Game<'_>, before: &[TankGroup], after: &[TankGroup], per_block: u64) {
    for (b, a) in before.iter().zip(after) {
        if b.amount == a.amount && b.fluid == a.fluid {
            continue;
        }
        let old: HashMap<Pos, TankBlock> = b.shares(per_block).into_iter().collect();
        for (pos, share) in a.shares(per_block) {
            if old.get(&pos) != Some(&share) {
                game.native_with_mod_block_entity(pos, |e| write_tank(&mut e.nbt, &share));
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
            amount: g.amount,
            capacity: g.capacity,
            blocks: g.blocks.len() as u32,
            coming,
            going,
            note,
        });
        for &pos in &g.blocks {
            let shown = game.native_mod_block_entity(pos).map(|e| e.data.clone());
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
    let entity = game.native_mod_block_entity(pos)?;
    let pump = Pump {
        progress: read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0),
        held: read_held(&entity.nbt),
    };
    let under = under(game, pos);
    Some(PumpHeld {
        request: pump.request(&under, tuning.pump_energy),
        pump,
        under,
        data: entity.data.clone(),
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
    } = held;
    let before = pump.clone();
    let tick = pump.tick(&under, share, tuning.pump_ticks);
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
        asked: tuning.pump_energy,
        shown,
        no_lane,
    });
    if pump == before && shown_data == data {
        return Some(shown);
    }
    game.native_with_mod_block_entity(pos, move |e| {
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
        log::warn!(
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

/// Buckets as the guide writes them: «20», «12,8».
pub(super) fn bucket_text(mb: u64) -> String {
    let tenths = (mb + 50) / 100;
    if tenths % 10 == 0 {
        (tenths / 10).to_string()
    } else {
        format!("{},{}", tenths / 10, tenths % 10)
    }
}
