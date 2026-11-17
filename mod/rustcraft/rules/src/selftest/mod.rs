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
pub mod craft18;
pub mod energy;
pub mod expect;
pub mod fluids;
pub mod handbook;
pub mod magistral;
pub mod miners;
// Slice 18, the module stream.
pub mod modules18;
pub mod monorail;
pub mod orders2;
// s18/pipe
pub mod pipe18;
pub mod rest;
pub mod run;
pub mod s18_energy;
pub mod slice10;
pub mod slice11;
pub mod slice12;
pub mod slice13;
pub mod slice14;
pub mod slice15;
pub mod slice18_machine;
// Slice 18, wave 2: machines and modules.
pub mod w2_machine;
// Slice 18, cross-stream agent.
pub mod s18_cross;
pub mod s18_last;
pub mod explore2;
pub mod sources;
pub mod storage;
pub mod traversal;
pub mod transport;
pub mod turret;
pub mod windows;
pub mod workers;
// Slice 18, stream 6 (rail and workers).
pub mod s18_rail;
// Slice 18, base and gear (stream 7).
pub mod gear18;
// Slice 18, last leftovers: the backpack.
pub mod s18_last_pack;
pub mod s18_last_orders;
// Slice 18, wave 2: rail, workers, base and gear.
pub mod s18_w2_rail_gear;
// Slice 18, wave 2: the magistral and the energy.
pub mod w2_pipe_energy;

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
    // s18/pipe
    /// Stream 1's actions on the block at `pos` (rustcraft_pipe18::bench), its answer.
    fn pipe(&mut self, action: &str, pos: Pos) -> Result<String, String>;

    // ---- Slice 9 ----

    /// What the window of the slice-7 or slice-8 block at `pos` shows: the sheet the server
    /// sends a player who has it open.
    fn sheet(&self, pos: Pos) -> Option<crate::machines::sheet::Sheet>;
    /// `press` that also says what is on the cursor after it.
    fn press_cursor(
        &mut self,
        pos: Pos,
        button: i32,
        carried: Option<(&str, u32)>,
    ) -> Result<Option<(String, u32)>, String>;

    // ---- Slice 10 ----

    /// The mixer at `pos`: its tanks and what it showed on its last tick.
    fn mixer(&self, pos: Pos) -> Option<crate::machines::mixer::MixerView>;
    /// An item entity of `count` `item` lying at `at`.
    fn drop_item(&mut self, at: [f64; 3], item: &str, count: u32) -> Result<(), String>;
    /// The item entities lying in the box `min`..`max`: item and count.
    fn lying(&self, min: [f64; 3], max: [f64; 3]) -> Vec<(String, u32)>;

    // ---- Slice 11 ----

    /// The catalyst tank of the furnace or crusher at `pos`.
    fn catalyst(&self, pos: Pos) -> Option<crate::machines::catalyst::Tank>;
    /// What the butcher at `pos` showed on its last strike.
    fn butcher(&self, pos: Pos) -> Option<crate::butcher::Shown>;

    // Slice 12.
    /// What clients were told of the block's moving parts.
    fn anim(&self, pos: Pos) -> Option<crate::anim::Shown>;

    // Slice 13.
    /// Puts a gear piece with a number of its own and `charge` E into slot `slot` at `pos`.
    fn put_gear(&mut self, pos: Pos, slot: usize, item: &str, charge: f64) -> Result<(), String>;
    /// The charge of the gear piece in slot `slot` at `pos`.
    fn gear_charge(&self, pos: Pos, slot: usize) -> Option<f64>;

    // Slice 14.
    /// What the crafting grid `grid` makes when every gear piece in it has a number of its own and
    /// `charge` E: the result's item and its charge (None when it has no number).
    fn craft_gear(&mut self, grid: &[Option<&str>; 9], charge: f64) -> Option<(String, Option<f64>)>;
    // Slice 15.
    /// An order for `count` of `item` at the storage the cell or terminal at `pos` belongs to, as
    /// a terminal's order button gives it (without the button's check of the book): Err names
    /// why it was refused (no smart line yet, a full queue).
    fn order(&mut self, pos: Pos, item: &str, count: u32) -> Result<(), String>;
    /// The queue of the storage the cell or terminal at `pos` belongs to.
    fn orders(&self, pos: Pos) -> Vec<crate::orders::queue::Order>;
    /// «Держать N» of `item` at the storage the cell or terminal at `pos` belongs to.
    fn set_keep(&mut self, pos: Pos, item: &str, n: u64) -> Result<(), String>;
    /// O7: the order `order` of the storage the cell or terminal at `pos` belongs to cancelled.
    fn cancel_order(&mut self, pos: Pos, order: u16) -> Result<(), String>;

    // ST4, ST5.
    /// `put` of a block the living vein grew, as a worker's hold gives it out (custom data
    /// `rustcraft:grown`).
    fn put_grown(&mut self, pos: Pos, slot: usize, item: &str, count: u32) -> Result<(), String>;
    /// A restart of the server for the orders of the storage the cell or terminal at `pos`
    /// belongs to: the level's storage record (its orders and jobs with it) through the binary NBT
    /// a save writes and back, then the level's orders, jobs and temporary pages forgotten and
    /// read again from what came back.
    fn restart_orders(&mut self, pos: Pos) -> Result<(), String>;
    // The zipline's ropes and the harpoon on mobs (12.5 Z2, G5).
    /// Z2: a rope `owner` ties between anchors at `a` and `b` as a zipline's use ties it (the
    /// anchors set, the rope saved, the owner's oldest beyond the limit untied), with no player and
    /// no charge: its id.
    fn tie_rope(&mut self, owner: u128, a: Pos, b: Pos) -> Result<u64, String>;
    /// The saved ropes.
    fn ropes(&self) -> Vec<crate::traversal::Rope>;
    /// G5 on mobs: the harpoon of a player with feet at `from` hooks the mob `id`.
    fn harpoon_mob(&mut self, from: [f64; 3], id: u32) -> Result<(), String>;
    // The handbook (H2).
    /// What the server's grant does for the player `player` carrying `carried`: true when they
    /// get the handbook now (noted as given; no stack moves, the player is a pretend one).
    fn handbook_offer(&mut self, player: u128, carried: &[&str]) -> bool;
    /// The player forgotten by the grant, as if they never had the handbook.
    fn handbook_forget(&mut self, player: u128);
    /// Whether the game's item registry has `item`.
    fn item_known(&self, item: &str) -> bool;
    /// The 3 × 3 grid of the first crafting recipe of `item`, as the handbook's page reads it.
    fn recipe_grid(&self, item: &str) -> Option<[Option<String>; 9]>;
    /// The recipes the handbook's page `page` (an index into PAGES) shows, as its window reads
    /// them from the game (slice 18: machines' and transmuting recipes too).
    fn handbook_recipes(&self, page: usize) -> Vec<crate::handbook::recipes::Shown>;
    // Spec 26: the wrench lifts suspicious blocks.
    /// The loot table of the suspicious block at `pos`, as a structure leaves it in its block
    /// entity.
    fn set_brush_loot(&mut self, pos: Pos, table: &str) -> Result<(), String>;
    /// A sneaking player's wrench on the suspicious block at `pos`, the item it gives put into
    /// slot `slot` of the block entity at `into` instead of the player's inventory: what the
    /// item's marks say.
    fn wrench_lift(
        &mut self,
        pos: Pos,
        into: Pos,
        slot: usize,
    ) -> Result<crate::utility::found::Lift, String>;

    // ---- Slice 18, energy stream ----

    /// Breaks the block at `pos` as `tool` would (none: by hand), the drops rolled by its loot
    /// table and handed back instead of dropped.
    fn break_block(&mut self, pos: Pos, tool: Option<&str>) -> Result<Vec<(String, u32)>, String>;
    /// The seed at `pos` (CR2): its growth and the solution it holds.
    fn seed(&self, pos: Pos) -> Option<crate::grow::Seed>;
    /// Sets how far the seed at `pos` has grown.
    fn set_seed(&mut self, pos: Pos, progress: f64) -> Result<(), String>;
    /// How clients are told to draw the crystal at `pos`: a merged structure's box (its anchor),
    /// hidden (another block of it), and lit with solution.
    fn crystal_drawn(&self, pos: Pos) -> Option<(Option<[i32; 3]>, bool, bool)>;
    // ---- Slice 18, stream 6 (rail and workers) ----
    /// W3: worker `id` taken away as its one chassis item and that item placed on rail `cell`;
    /// the new worker's id.
    fn worker_as_item(&mut self, id: u64, cell: Pos) -> Result<u64, String>;
    /// U4: chassis `item` clicked on worker `id`; the old chassis's item it gives back.
    fn swap_chassis(&mut self, id: u64, item: &str) -> Result<String, String>;
    /// W8: whether `owner` may place one more worker under a server limit of `limit`.
    fn worker_room(&self, owner: u128, limit: u32) -> bool;
    /// H6: worker `id`'s filter set to `items`.
    fn set_worker_filter(&mut self, id: u64, items: &[&str]) -> Result<(), String>;
    // ---- Slice 18, base and gear (stream 7) ----

    /// `put_gear` of a piece holding `held` (its charge, modules, levels).
    /// (Defaults keep other benches compiling; the game's bench answers.)
    fn put_gear_held(&mut self, _pos: Pos, _slot: usize, _item: &str, _held: crate::gear::Held) -> Result<(), String> {
        Err("not on this bench".to_owned())
    }
    /// The held data of the gear piece in slot `slot` at `pos`.
    fn gear_held(&self, _pos: Pos, _slot: usize) -> Option<crate::gear::Held> {
        None
    }
    /// The aim a turret's block entity tells clients: (yaw, pitch) in degrees, None before any.
    fn turret_aim(&self, _pos: Pos) -> Option<(f32, f32)> {
        None
    }
    /// What the anvil makes of a piece `piece` (with no enchantment yet) and `books` Capacity
    /// books of level `level` one after another: the piece's Capacity level after each.
    fn anvil_capacity(&mut self, _piece: &str, _level: u8, _books: usize) -> Vec<Option<u8>> {
        Vec::new()
    }
    /// Rope `id` as if tied `ms` earlier.
    fn age_rope(&mut self, _id: u64, _ms: u64) -> Result<(), String> {
        Err("not on this bench".to_owned())
    }
    // ---- Slice 18, stream 3 (machines) ----
    /// A right click with `item` on the tiered block at `pos` (U2, rustcraft_tiers): the old
    /// tier's item it gives back, or why nothing was replaced.
    fn swap_tier(&mut self, pos: Pos, item: &str) -> Result<String, String> {
        let _ = (pos, item);
        Err("no tiers on this bench".into())
    }
    /// A bucket `held` used on the tank at `pos` (guide `tank`): the bucket it leaves in the hand.
    fn use_bucket(&mut self, pos: Pos, held: &str) -> Result<String, String> {
        let _ = (pos, held);
        Err("no buckets on this bench".into())
    }
    // ---- Slice 18, stream 5 (craft and storage) ----

    /// An item written as SNBT (`{id:..,count:..,components:{..}}`) put into the container at
    /// `pos` as a hopper would: how many went in.
    fn insert_snbt(&mut self, pos: Pos, snbt: &str) -> Result<u32, String>;
    /// The stacks the storage the cell or terminal at `pos` belongs to holds, each as the stack
    /// its key makes: `item xCOUNT`, then its own name in quotes and its (stored) enchantments
    /// `id level`, space-separated.
    fn stored_stacks(&self, pos: Pos) -> Vec<String>;
    /// The translation key of what a player looking at the cell or terminal at `pos` reads.
    fn storage_diagnosis(&self, pos: Pos) -> Option<String>;
    /// The int array under `key` in the saved compound of the mod block entity at `pos`.
    fn int_array(&self, pos: Pos, key: &str) -> Vec<i32>;
    /// The long (or int) under `key` in the saved compound of the mod block entity at `pos`.
    fn long(&self, pos: Pos, key: &str) -> i64;
    // Slice 18, cross-stream agent.
    /// The data slots the mod block entity at `pos` sends its window.
    fn menu_data(&self, _pos: Pos) -> Vec<i32> {
        Vec::new()
    }
    // Slice 18, last leftovers: the backpack's store.
    /// The backpack's store without a player: a deposit past 99 with a stack module, the module
    /// kept while a slot is past a plain stack, the store saved and read back, a slice-10 file
    /// read, the window's message (rustcraft_backpack::self_test_store).
    fn backpack_store(&mut self) -> Result<(), String> {
        Err("no backpack store on this bench".into())
    }
    // ---- Slice 18, wave 2: rail, workers, base and gear ----
    /// A right click with rail item `item` on the rail at `pos`: the old tier's item it gives
    /// back, or why nothing changed (rustcraft_monorail::swap_rail_in_place).
    fn swap_rail(&mut self, pos: Pos, item: &str) -> Result<String, String> {
        let _ = (pos, item);
        Err("no rails on this bench".into())
    }
    /// A press in worker `id`'s chassis window (rustcraft_workers::press_for_selftest).
    fn chassis_press(&mut self, id: u64, press: crate::worker::screen::Press) -> Result<(), String> {
        let _ = (id, press);
        Err("no chassis windows on this bench".into())
    }
    /// Worker `id`'s sockets hold `modules`.
    fn set_worker_modules(&mut self, id: u64, modules: &[&str]) -> Result<(), String> {
        let _ = (id, modules);
        Err("no workers on this bench".into())
    }
}

/// A mob as the self-test reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct MobView {
    pub kind: String,
    /// Its feet.
    pub pos: [f64; 3],
    pub health: f32,
    pub effects: Vec<(String, u32, u8)>,
    /// Slice 18: the share of its speed a turret's slowness takes now (its movement speed
    /// modifier `rustcraft:turret_slow`), if it is slowed.
    pub slow: Option<f64>,
    /// Ticks left of that slowness.
    pub slow_ticks: Option<u32>,
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
    all.extend(windows::ALL);
    all.extend(slice10::ALL);
    all.extend(slice11::ALL);
    all.extend(slice12::ALL);
    all.extend(slice13::ALL);
    all.extend(slice14::ALL);
    all.extend(slice15::ALL);
    all.extend(explore2::ALL);
    all.extend(orders2::ALL);
    all.extend(traversal::ALL);
    all.extend(handbook::ALL);
    all.extend(rest::ALL);
    // Slice 18, energy stream.
    all.extend(s18_energy::ALL);
    // Slice 18, stream 6 (rail and workers).
    all.extend(s18_rail::ALL);
    // Slice 18, base and gear (stream 7).
    all.extend(gear18::ALL);
    // Slice 18, stream 3.
    all.extend(slice18_machine::ALL);
    // Slice 18, the module stream.
    all.extend(modules18::ALL);
    all.extend(craft18::ALL);
    // s18/pipe
    all.extend(pipe18::ALL);
    // Slice 18, cross-stream agent.
    all.extend(s18_cross::ALL);
    all.extend(s18_last::ALL);
    // Slice 18, last leftovers: the backpack.
    all.extend(s18_last_pack::ALL);
    all.extend(s18_last_orders::ALL);
    // Slice 18, wave 2: rail, workers, base and gear.
    all.extend(s18_w2_rail_gear::ALL);
    all.extend(w2_pipe_energy::ALL);
    // Slice 18, wave 2: machines and modules.
    all.extend(w2_machine::ALL);
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
