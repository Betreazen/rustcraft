//! The in-world self-test of the joint bench (horizon-joint-dev, «Быстрые проверки»): scenarios
//! that build real blocks in the mod's part of a fresh world (x 0..1000, z 0..1000, y 100 and up),
//! let the server tick them and check what the mod did from block entities, containers and the
//! level's state. The bundle gives the scenarios the world through `Bench`; the scenarios, the
//! order they run in and the expected numbers live here, the expected numbers worked out from
//! `Tuning` and the rules themselves, so nothing is written down twice.
//!
//! A scenario is a list of stages: an action, a wait of some ticks, or a check polled each tick
//! until it holds or its limit runs out. Every scenario has its own plot; `run::Run` holds their
//! chunks, clears the plots, runs all scenarios side by side from the level's tick (so it works
//! under `/tick sprint`), clears each plot again when its scenario ends and reports
//! `{"mod","done","passed","failed","timings"}`.

pub mod anchor;
pub mod energy;
pub mod expect;
pub mod fluids;
pub mod magistral;
pub mod miners;
pub mod monorail;
pub mod run;
pub mod sources;
pub mod storage;
pub mod transport;
pub mod turret;
pub mod workers;

use std::collections::BTreeMap;

use crate::geom::Pos;
use crate::housing::Diagnosis;
use crate::machines::alloy::AlloyRecipe;
use crate::machines::processor::Shown;
use crate::monorail::{Network, Trip, TripError};
use crate::net::settings::JointSetting;
use crate::net::tanks::TankGroup;
use crate::net::topology::Channel;
use crate::storage::terminal::LimitChange;
use crate::storage::{Shape, Store};
use crate::tuning::Tuning;

/// What one slot holds: an item and a count, or nothing.
pub type Slot = Option<(String, u32)>;

/// A box of blocks, both corners included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub min: Pos,
    pub max: Pos,
}

impl Area {
    /// The chunks the box touches, as `[x, z]` chunk coordinates.
    pub fn chunks(&self) -> Vec<[i32; 2]> {
        let mut out = Vec::new();
        for cx in (self.min[0] >> 4)..=(self.max[0] >> 4) {
            for cz in (self.min[2] >> 4)..=(self.max[2] >> 4) {
                out.push([cx, cz]);
            }
        }
        out
    }
}

/// A storage as the server holds it: its record and the shape of its cells.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageView {
    pub cells: usize,
    pub shape: Shape,
    pub store: Store,
}

/// The world as the scenarios see it. Every position is in the level the test runs in.
pub trait Bench {
    // ---- The plots ----

    /// Holds or lets go of the chunks of `area`, so they load and tick with nobody near.
    fn hold(&mut self, area: &Area, hold: bool);
    /// Whether every chunk of `area` is loaded.
    fn loaded(&self, area: &Area) -> bool;
    /// Every block of `area` turned to air, block entities and fluids with them.
    fn clear(&mut self, area: &Area) -> Result<(), String>;
    /// The magistral, the machines, the storages and the monorail read again before the next
    /// tick (what a player's placing or breaking marks).
    fn refresh(&mut self);
    /// Milliseconds since some fixed moment.
    fn now_ms(&self) -> u128;

    // ---- Building ----

    /// Places `block` with its default state, then sets each of `props` on it, as the game's
    /// own `Level.setBlock` does.
    fn place(&mut self, pos: Pos, block: &str, props: &[(&str, &str)]) -> Result<(), String>;
    /// Puts `count` of `item` into slot `slot` of the mod block entity at `pos`.
    fn put(&mut self, pos: Pos, slot: usize, item: &str, count: u32) -> Result<(), String>;
    /// Puts `count` of `item` into the vanilla container at `pos` from above, as a hopper would;
    /// how many went in.
    fn insert(&mut self, pos: Pos, item: &str, count: u32) -> Result<u32, String>;
    /// The housing at `pos` with these channels, written as its block entity keeps them.
    fn set_channels(&mut self, pos: Pos, slots: [Option<Channel>; 4]) -> Result<(), String>;
    /// One joint setting of the housing at `pos`, written as the joint window writes it.
    fn set_joint(
        &mut self,
        pos: Pos,
        face: crate::geom::Face,
        slot: usize,
        setting: JointSetting,
    ) -> Result<(), String>;
    /// A press on the window of the machine at `pos`, with `carried` on the cursor
    /// (`native_menu_button_v2`, then `native_menu_button`).
    fn press(&mut self, pos: Pos, button: i32, carried: Option<(&str, u32)>) -> Result<(), String>;
    /// The tank block at `pos` holding `amount` mB of `fluid`.
    fn set_tank(&mut self, pos: Pos, fluid: &str, amount: u64) -> Result<(), String>;
    /// The station at `pos` with `energy` E in its buffer.
    fn set_station_energy(&mut self, pos: Pos, energy: f64) -> Result<(), String>;
    /// A limit button of a terminal of the storage that `pos` (a cell or a terminal) belongs to.
    fn change_limit(&mut self, pos: Pos, item: &str, change: LimitChange) -> Result<(), String>;

    // ---- Reading ----

    fn tuning(&self) -> &Tuning;
    fn block(&self, pos: Pos) -> String;
    fn property(&self, pos: Pos, key: &str) -> Option<String>;
    /// The slots of the container at `pos`: a mod block entity's own, or a vanilla container's.
    fn items(&self, pos: Pos) -> Vec<Slot>;
    /// The slots of the machine at `pos` the net and hoppers put into, in its own order.
    fn inputs(&self, pos: Pos) -> Vec<usize>;
    /// The housing's diagnosis as the server worked it out.
    fn housing(&self, pos: Pos) -> Option<Diagnosis>;
    /// What the machine at `pos` showed on its last tick.
    fn shown(&self, pos: Pos) -> Option<Shown>;
    /// Whether an item net found the receiver at `pos` full lately («Приёмник полон»).
    fn receiver_full(&self, pos: Pos) -> bool;
    fn storage(&self, pos: Pos) -> Option<StorageView>;
    /// The tank group the tank block at `pos` belongs to.
    fn tank(&self, pos: Pos) -> Option<TankGroup>;
    /// The fluid the source at `pos` found no lane for lately.
    fn no_lane(&self, pos: Pos) -> Option<String>;
    fn station_energy(&self, pos: Pos) -> Option<f64>;
    /// The level's monorail network as the server keeps it.
    fn network(&self) -> Network;
    /// Plans a trip as «Ехать» does; with `pay` the start station also pays for it.
    fn plan_trip(&mut self, from: Pos, to: Pos, pay: bool) -> Result<Trip, TripError>;
    /// Ticks the base game's furnace takes to smelt `item`.
    fn cook_ticks(&self, item: &str) -> Option<f64>;
    /// The crusher's recipe for `item`: its result, count and ticks.
    fn crush(&self, item: &str) -> Option<(String, u32, f64)>;
    fn alloy_recipes(&self) -> Vec<AlloyRecipe>;
    /// What a 3×3 crafting grid makes: the result and its count.
    fn craft(&self, grid: &[Option<&str>; 9]) -> Option<(String, u32)>;
    /// The block entity at `pos` (a housing) and the level's records (storages, monorail) through
    /// the binary NBT a save writes and back: what a restart reads. Err names what came back
    /// different.
    fn round_trip(&self, pos: Pos) -> Result<(), String>;

    // ---- Slice 7 ----

    /// A mob of `kind` with no AI at `at`, with `health` if given; its id.
    fn spawn_mob(&mut self, kind: &str, at: [f64; 3], health: Option<f32>) -> Result<u32, String>;
    /// A living mob: its health and its effects (id, ticks left, amplifier).
    fn mob(&self, id: u32) -> Option<MobView>;
    /// The modules in the slots of a turret or an anchor at `pos` (item ids).
    fn set_modules(&mut self, pos: Pos, modules: &[&str]) -> Result<(), String>;
    /// The block at `pos` is `owner`'s, placed at `placed` (a larger one is newer).
    fn set_owner(&mut self, pos: Pos, owner: u128, placed: u64) -> Result<(), String>;
    /// The energy buffer of a turret or a charge dock.
    fn set_buffer(&mut self, pos: Pos, energy: f64) -> Result<(), String>;
    fn buffer(&self, pos: Pos) -> f64;
    /// What the turret showed on its last tick.
    fn turret(&self, pos: Pos) -> Option<crate::turret::Shown>;
    /// Whether the mob bleeds: SoulsCraft's effect on it, or the turret's own timer.
    fn bleeding(&self, id: u32) -> bool;
    /// Whether SoulsCraft's mobs.json was found where Tuning says.
    fn souls_found(&self) -> bool;
    /// A worker placed on rail `cell`; its id.
    fn spawn_worker(
        &mut self,
        cell: Pos,
        owner: u128,
        tool: Option<crate::worker::Tool>,
        hold: &[(&str, u32)],
        charge: Option<f64>,
    ) -> Result<u64, String>;
    fn worker(&self, id: u64) -> Option<crate::worker::Worker>;
    /// The report lines sent to Path-of-craft-tree lately (`unsent …` without it).
    fn reports(&self) -> Vec<String>;
    /// Players taken as on the server by the anchors, the self-test's pretend owners.
    fn pretend_online(&mut self, owner: u128, online: bool);
    fn anchor(&self, pos: Pos) -> Option<crate::anchor::State>;
    /// The chunks the anchors hold now.
    fn held_chunks(&self) -> Vec<[i32; 2]>;
    /// The workers' record through its saved form and back.
    fn workers_round_trip(&self) -> Result<(), String>;

    // ---- Slice 8 ----

    /// What a crystal, a panel, a windmill, a lava generator or a battery showed on its last tick.
    fn power(&self, pos: Pos) -> Option<crate::power::PowerView>;
    /// The time of day (ticks) and the rain level now.
    fn sky(&self) -> (i64, f32);
    /// The block and sky light at `pos`.
    fn light(&self, pos: Pos) -> (u8, u8);
    /// The lowest block y of the level.
    fn min_y(&self) -> i32;
    /// What a miner, a placer or a breaker showed on its last tick.
    fn mining(&self, pos: Pos) -> Option<crate::mining::Shown>;
    /// The ores switched off in a miner's window (D5).
    fn set_ores_off(&mut self, pos: Pos, off: u16) -> Result<(), String>;
    /// The `role` a turret's block entity says (`attack` or `support`).
    fn role(&self, pos: Pos) -> Option<String>;
    /// The console's `rustcraft_testkit` with these words after it; its answer.
    fn testkit(&mut self, args: &str) -> String;
}

/// A mob as the self-test reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct MobView {
    pub kind: String,
    pub health: f32,
    pub effects: Vec<(String, u32, u8)>,
}

impl MobView {
    /// The effect `id`, if it has it: ticks left and amplifier.
    pub fn effect(&self, id: &str) -> Option<(u32, u8)> {
        self.effects.iter().find(|e| e.0 == id).map(|e| (e.1, e.2))
    }
}

/// What a scenario's stages share: how long it has run, and numbers one stage notes for another.
#[derive(Clone, Debug, Default)]
pub struct Memo {
    /// Level ticks since the scenario started.
    pub elapsed: u64,
    pub marks: BTreeMap<&'static str, f64>,
}

impl Memo {
    pub fn mark(&mut self, key: &'static str, value: f64) {
        self.marks.insert(key, value);
    }

    pub fn get(&self, key: &str) -> f64 {
        self.marks.get(key).copied().unwrap_or(0.0)
    }
}

pub type Act = Box<dyn FnMut(&mut dyn Bench, &mut Memo) -> Result<(), String> + Send>;
pub type Probe = Box<dyn FnMut(&mut dyn Bench, &mut Memo) -> Result<bool, String> + Send>;

/// One step of a scenario.
pub enum Stage {
    /// Done once; the world is read again before the next stage.
    Do { what: &'static str, act: Act },
    /// That many level ticks pass.
    Wait(u64),
    /// Asked every tick until it says true; a fail after `limit` ticks.
    Until {
        what: &'static str,
        limit: u64,
        probe: Probe,
    },
}

/// A named check with its own plot.
pub struct Scenario {
    pub name: &'static str,
    pub area: Area,
    pub stages: Vec<Stage>,
}

impl Scenario {
    pub fn new(name: &'static str, area: Area) -> Self {
        Scenario {
            name,
            area,
            stages: Vec::new(),
        }
    }

    pub fn then(
        mut self,
        what: &'static str,
        act: impl FnMut(&mut dyn Bench, &mut Memo) -> Result<(), String> + Send + 'static,
    ) -> Self {
        self.stages.push(Stage::Do {
            what,
            act: Box::new(act),
        });
        self
    }

    pub fn wait(mut self, ticks: u64) -> Self {
        self.stages.push(Stage::Wait(ticks));
        self
    }

    pub fn until(
        mut self,
        what: &'static str,
        limit: u64,
        probe: impl FnMut(&mut dyn Bench, &mut Memo) -> Result<bool, String> + Send + 'static,
    ) -> Self {
        self.stages.push(Stage::Until {
            what,
            limit,
            probe: Box::new(probe),
        });
        self
    }

    /// A check made once: a fail at once when it does not hold.
    pub fn check(
        self,
        what: &'static str,
        mut test: impl FnMut(&mut dyn Bench, &mut Memo) -> Result<(), String> + Send + 'static,
    ) -> Self {
        self.until(what, 0, move |b, m| test(b, m).map(|()| true))
    }
}

/// Where the plots lie: a grid in the mod's part of the bench world, one plot a scenario.
pub const PLOT_ORIGIN: Pos = [16, 100, 16];
/// A plot's size along x and z; scenarios keep inside it.
pub const PLOT_SIZE: [i32; 2] = [48, 16];
pub const PLOT_HEIGHT: i32 = 12;
/// Plots a row of the grid.
pub const PLOTS_A_ROW: usize = 6;

/// The corner of plot `index`; a scenario builds from there (a margin of 2 blocks inside it).
pub fn plot_origin(index: usize) -> Pos {
    let col = (index % PLOTS_A_ROW) as i32;
    let row = (index / PLOTS_A_ROW) as i32;
    [
        PLOT_ORIGIN[0] + col * PLOT_SIZE[0] + 2,
        PLOT_ORIGIN[1] + 2,
        PLOT_ORIGIN[2] + row * PLOT_SIZE[1] + 4,
    ]
}

/// The whole plot of a scenario built from `origin`.
pub fn plot_area(origin: Pos) -> Area {
    let min = [origin[0] - 2, origin[1] - 2, origin[2] - 4];
    Area {
        min,
        max: [
            min[0] + PLOT_SIZE[0] - 1,
            min[1] + PLOT_HEIGHT - 1,
            min[2] + PLOT_SIZE[1] - 1,
        ],
    }
}

/// `origin` moved by `d`.
pub fn at(origin: Pos, d: [i32; 3]) -> Pos {
    [origin[0] + d[0], origin[1] + d[1], origin[2] + d[2]]
}

/// A builder of one scenario from its plot's origin.
pub type Build = fn(Pos, &Tuning) -> Scenario;

/// Every scenario, in the order the plots are handed out.
pub fn catalog() -> Vec<Build> {
    let mut all: Vec<Build> = Vec::new();
    all.extend(magistral::ALL);
    all.extend(transport::ALL);
    all.extend(energy::ALL);
    all.extend(storage::ALL);
    all.extend(fluids::ALL);
    all.extend(monorail::ALL);
    all.extend(workers::ALL);
    all.extend(turret::ALL);
    all.extend(anchor::ALL);
    all.extend(sources::ALL);
    all.extend(miners::ALL);
    all
}

/// The scenarios whose name contains `filter` (all for an empty one), each on its own plot.
pub fn scenarios(filter: &str, tuning: &Tuning) -> Vec<Scenario> {
    catalog()
        .into_iter()
        .enumerate()
        .map(|(i, build)| build(plot_origin(i), tuning))
        .filter(|s| s.name.contains(filter))
        .collect()
}

// ---- Small helpers the scenarios share ----

/// How many of `item` the slots hold.
pub fn count_of(slots: &[Slot], item: &str) -> u32 {
    slots
        .iter()
        .flatten()
        .filter(|(i, _)| i == item)
        .map(|(_, n)| n)
        .sum()
}

/// How many items the slots hold, of any kind.
pub fn total(slots: &[Slot]) -> u32 {
    slots.iter().flatten().map(|(_, n)| n).sum()
}

/// A fail unless `value` is within `tolerance` of `wanted`.
pub fn near(what: &str, value: f64, wanted: f64, tolerance: f64) -> Result<(), String> {
    if (value - wanted).abs() <= tolerance {
        Ok(())
    } else {
        Err(format!("{what} {value}, expected {wanted} ± {tolerance}"))
    }
}

/// A fail unless `got == wanted`.
pub fn same<T: PartialEq + std::fmt::Debug>(what: &str, got: T, wanted: T) -> Result<(), String> {
    if got == wanted {
        Ok(())
    } else {
        Err(format!("{what} {got:?}, expected {wanted:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_plot_lies_in_the_mods_part_of_the_world_and_apart() {
        // horizon-joint-dev README: RustCraft x 0..1000, z 0..1000, y 100 and up.
        let tuning = Tuning::default();
        let all = scenarios("", &tuning);
        assert!(all.len() >= 25, "{}", all.len());
        for (i, s) in all.iter().enumerate() {
            let a = s.area;
            assert!(a.min[0] >= 0 && a.max[0] < 1000, "{}", s.name);
            assert!(a.min[2] >= 0 && a.max[2] < 1000, "{}", s.name);
            assert!(a.min[1] >= 100, "{}", s.name);
            for t in &all[i + 1..] {
                let apart = (0..3).any(|k| a.max[k] < t.area.min[k] || t.area.max[k] < a.min[k]);
                assert!(apart, "{} and {} overlap", s.name, t.name);
            }
        }
    }

    #[test]
    fn names_are_unique_and_the_filter_picks_by_name() {
        let tuning = Tuning::default();
        let all = scenarios("", &tuning);
        let mut names: Vec<&str> = all.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len());
        let some = scenarios("storage", &tuning);
        assert!(!some.is_empty());
        assert!(some.iter().all(|s| s.name.contains("storage")));
    }

    #[test]
    fn an_area_lists_the_chunks_it_touches() {
        let a = Area {
            min: [14, 100, 0],
            max: [17, 105, 15],
        };
        assert_eq!(a.chunks(), vec![[0, 0], [1, 0]]);
    }
}
