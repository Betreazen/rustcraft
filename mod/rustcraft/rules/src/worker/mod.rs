//! The worker cart (spec 4.7, W1–W7; guide `chassis`, `tool_saw`, `tool_reaper`, `dock_unload`,
//! `dock_charge`, `upgrades`). A worker is a chassis on the monorail with one tool, upgrades, a
//! hold and a battery. It rides its own stretch of rail on its own (W2, W4): straight on where it
//! can, back at a dead end, round a loop; its tool works the cells along the way. With a full
//! hold it rides to the nearest unload dock on its rails (W4.a), with a battery under the
//! threshold to the nearest charge dock (W4.b); with no such dock on its rails it stops and says
//! so (W4.c). These rules move it and choose; the bundle breaks blocks, moves items and energy and
//! draws it.
//!
//! Slice 18: a worker rides every rail the capsule rides (W2: «тот же монорельс»): floor, wall,
//! ceiling and the vertical, so a reaper climbs a jungle trunk, hangs over kelp or rides a lift
//! between floors. Its tool's zone grows away from the block its rail hangs on (`zone_cells`).

pub mod chassis;
pub mod digging;
pub mod bore;
pub mod hold;
pub mod plants;
pub mod pivots;
pub mod pose;
pub mod trees;
pub mod wire;

use crate::geom::{Face, Pos, step};
use crate::modules::{self, Module};
use crate::monorail::{Network, neighbours};
use crate::tuning::Tuning;
use hold::Hold;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

pub const CHASSIS: &str = "rustcraft:chassis";
pub const UNLOAD_DOCK: &str = "rustcraft:dock_unload";
pub const CHARGE_DOCK: &str = "rustcraft:dock_charge";
pub const LOAD_DOCK: &str = "rustcraft:dock_load";

/// The world as the tools read it.
pub trait BlockView {
    fn block(&self, pos: Pos) -> String;
    fn prop(&self, pos: Pos, key: &str) -> Option<String>;
    /// `#minecraft:logs`.
    fn is_log(&self, pos: Pos) -> bool;
    /// `#minecraft:leaves`.
    fn is_leaves(&self, pos: Pos) -> bool;
    /// `#minecraft:dirt`: where a sapling grows.
    fn is_dirt(&self, pos: Pos) -> bool;
}

/// A worker's tool (spec 4.7 table): slice 7 has the two of slice 1, slice 10 the other tools of
/// tier 2: the cargo hold (guide `tool_hold`, «грузовой отсек»), the grab (`tool_grab`) and the
/// scoop (`tool_scoop`, spec 22).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tool {
    Saw,
    Reaper,
    Hold,
    Grab,
    Scoop,
    /// Slice 11: the shooter (tier 3), which makes a worker a guard (P2): it rides its ring and
    /// shoots hostile mobs in range, never players, pets or peaceful mobs (P3).
    Shooter,
    /// Slice 13: the cartographer (tier 3): it marks the chunks and biomes along its rails.
    Map,
    /// Slice 14: the bore (tier 3): at the end of its rails it bores the tunnel ahead and lays the
    /// next rail from its hold.
    Bore,
}

impl Tool {
    pub const ALL: [Tool; 8] = [
        Tool::Saw,
        Tool::Reaper,
        Tool::Hold,
        Tool::Grab,
        Tool::Scoop,
        Tool::Shooter,
        Tool::Map,
        Tool::Bore,
    ];

    pub fn item(self) -> &'static str {
        match self {
            Tool::Saw => "rustcraft:tool_saw",
            Tool::Reaper => "rustcraft:tool_reaper",
            Tool::Hold => "rustcraft:tool_hold",
            Tool::Grab => "rustcraft:tool_grab",
            Tool::Scoop => "rustcraft:tool_scoop",
            Tool::Shooter => "rustcraft:tool_shooter",
            Tool::Map => "rustcraft:tool_map",
            Tool::Bore => "rustcraft:tool_bore",
        }
    }

    pub fn of(item: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.item() == item)
    }

    /// The tool word of Path-of-craft-tree's `break` report (contract, «break»).
    /// The cargo hold breaks nothing.
    pub fn report_word(self) -> &'static str {
        match self {
            Tool::Saw => "axe",
            Tool::Reaper => "hoe",
            Tool::Grab | Tool::Bore => "pickaxe",
            Tool::Scoop => "shovel",
            Tool::Hold | Tool::Shooter | Tool::Map => "",
        }
    }

    /// What the unload dock leaves in the hold for this tool: saplings, seeds.
    pub fn keeps(self, item: &str) -> bool {
        match self {
            Tool::Saw => trees::is_sapling(item),
            Tool::Reaper => plants::is_seed(item),
            Tool::Hold | Tool::Grab | Tool::Scoop | Tool::Shooter | Tool::Map => false,
            // The bore keeps its rails and the blocks it fills lava and gaps with.
            Tool::Bore => item == bore::RAIL || bore::is_filler(item),
        }
    }
}

/// What the worker is out to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Errand {
    #[default]
    Work,
    Unload,
    Charge,
    /// Slice 10: a cargo worker on its way to a load dock or loading there.
    Load,
}

/// What the worker shows over itself and in its window (W5, A4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Note {
    #[default]
    Working,
    Unloading,
    Charging,
    ToUnload,
    ToCharge,
    NoTool,
    /// W4.c.
    NoUnloadDock,
    NoChargeDock,
    /// Its battery is empty: it cannot move.
    Flat,
    /// H3.
    NothingToSow,
    /// The unload dock it stands at has no room.
    DockFull,
    /// The charge dock it stands at has no energy.
    DockEmpty,
    /// It stands on a single rail cell or no floor rail.
    NoWay,
    /// Slice 10, the cargo worker: on its way to a load dock, loading, no load dock on its rails,
    /// and an empty load dock with an empty hold.
    ToLoad,
    Loading,
    NoLoadDock,
    NoCargo,
    /// The scoop found nothing to dig along its whole way (guide «Рядом с путём нечего копать»).
    NothingToDig,
    /// Slice 14, the bore: no rails in its hold, and lava ahead with nothing to close it with.
    NoRails,
    LavaAhead,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Worker {
    pub id: u64,
    pub owner: u128,
    /// The rail cell it is in, the one it came from, the one it is riding to and how far.
    pub cell: Pos,
    pub from: Option<Pos>,
    pub to: Option<Pos>,
    pub progress: f64,
    pub tool: Option<Tool>,
    pub upgrades: Vec<Option<Module>>,
    pub hold: Hold,
    pub charge: f64,
    pub errand: Errand,
    /// Ticks its tool still works where it stands.
    pub busy: f64,
    pub note: Note,
    /// Slice 10: rail cells in a row its tool found nothing at (the scoop's diagnosis).
    #[serde(default)]
    pub idle: u32,
    /// Slice 18: its chassis's tier, 1 to 3 (U4: 3 / 5 / 7 sockets).
    #[serde(default = "first_tier")]
    pub tier: u8,
    /// Slice 18: how much of a cell it rode past its last cell this tick (T4: a fast rail takes it
    /// more than a cell a tick); the bundle rides it on while this is left, each cell worked.
    #[serde(default)]
    pub carry: f64,
    /// Slice 18: the block its tool works on now, for the client's pose (not saved).
    #[serde(skip)]
    pub aim: Option<Pos>,
}

fn first_tier() -> u8 {
    1
}

/// What one tick of riding came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Its tool is still at work.
    Busy,
    Moved,
    /// It came into a new cell on its work: the tool looks round there.
    Arrived(Pos),
    /// It stands at a dock of its errand: the dock at that position serves it.
    AtDock(Pos),
    Stopped,
}

/// The rails and the docks a level has.
pub struct Track<'a> {
    pub net: &'a Network,
    pub unload: &'a BTreeSet<Pos>,
    pub charge: &'a BTreeSet<Pos>,
    /// Slice 10: the load docks (W6).
    pub load: &'a BTreeSet<Pos>,
}

/// Whether `p` is a rail: a worker rides every rail (slice 18), floor, wall or ceiling.
fn on_rail(net: &Network, p: Pos) -> bool {
    net.rails.contains_key(&p)
}

/// The side of its cell the block the rail at `p` hangs on is behind (a floor's when no rail).
pub fn mount(net: &Network, p: Pos) -> Face {
    net.rails.get(&p).map_or(Face::YNeg, |r| r.mount)
}

/// The rails a worker in `p` can ride to: every rail joined to it.
pub fn ways(net: &Network, p: Pos) -> Vec<Pos> {
    neighbours(net, p)
}

/// The two unit steps across a mount: the axes a zone spreads along beside its depth.
pub fn across(mount: Face) -> [Pos; 2] {
    match mount {
        Face::XNeg | Face::XPos => [[0, 1, 0], [0, 0, 1]],
        Face::YNeg | Face::YPos => [[1, 0, 0], [0, 0, 1]],
        Face::ZNeg | Face::ZPos => [[1, 0, 0], [0, 1, 0]],
    }
}

/// `p` moved by `k` steps of `d`.
pub fn moved(p: Pos, d: Pos, k: i32) -> Pos {
    [p[0] + d[0] * k, p[1] + d[1] * k, p[2] + d[2] * k]
}

/// The rail cells beside each dock of `docks`, by a face: where a worker stops for it.
pub fn dock_cells(net: &Network, docks: &BTreeSet<Pos>) -> BTreeMap<Pos, Pos> {
    let mut out = BTreeMap::new();
    for &d in docks {
        for f in Face::ALL {
            let r = step(d, f);
            if on_rail(net, r) {
                out.entry(r).or_insert(d);
            }
        }
    }
    out
}

/// The shortest way over floor rails from `from` to any of `targets`: the cells after `from`.
pub fn way_to(net: &Network, from: Pos, targets: &BTreeMap<Pos, Pos>) -> Option<Vec<Pos>> {
    let mut came: HashMap<Pos, Pos> = HashMap::from([(from, from)]);
    let mut queue = VecDeque::from([from]);
    while let Some(p) = queue.pop_front() {
        if targets.contains_key(&p) {
            let mut path = vec![p];
            let mut q = p;
            while came[&q] != q {
                q = came[&q];
                path.push(q);
            }
            path.pop();
            path.reverse();
            return Some(path);
        }
        for n in ways(net, p) {
            if let std::collections::hash_map::Entry::Vacant(e) = came.entry(n) {
                e.insert(p);
                queue.push_back(n);
            }
        }
    }
    None
}

impl Worker {
    pub fn new(id: u64, owner: u128, cell: Pos, tuning: &Tuning) -> Self {
        Worker {
            id,
            owner,
            cell,
            from: None,
            to: None,
            progress: 0.0,
            tool: None,
            upgrades: vec![None; tuning.worker_upgrade_slots],
            hold: Hold::new(tuning.worker_hold),
            charge: tuning.worker_battery,
            errand: Errand::Work,
            busy: 0.0,
            note: Note::NoTool,
            idle: 0,
            tier: 1,
            carry: 0.0,
            aim: None,
        }
    }

    /// Its chassis becomes one of `tier`: as many sockets as that tier has, its upgrades kept in
    /// their sockets (a lower tier's extra sockets are left out: `chassis::swap` never lowers).
    pub fn set_tier(&mut self, tier: u8, _t: &Tuning) {
        self.tier = tier.clamp(1, 3);
        self.upgrades.resize(chassis::sockets(self.tier), None);
    }

    pub fn count(&self, kind: Module) -> u32 {
        modules::count(&self.upgrades, kind)
    }

    /// The battery's size: battery upgrades multiply it in full (A9).
    pub fn capacity(&self, t: &Tuning) -> f64 {
        t.worker_battery
            * t.worker_battery_upgrade
                .powi(self.count(Module::Battery) as i32)
    }

    /// The hold's size with its storage modules; a cargo hold (guide `tool_hold`) makes it big.
    pub fn hold_size(&self, t: &Tuning) -> usize {
        if self.tool == Some(Tool::Hold) {
            return t.cargo_hold.min(t.worker_hold_max);
        }
        (t.worker_hold + t.worker_hold_per_storage * self.count(Module::Storage) as usize)
            .min(t.worker_hold_max)
    }

    /// Slice 11: a guard's range, blocks; a radius upgrade doubles it (A7).
    pub fn guard_range(&self, t: &Tuning) -> f64 {
        t.guard_range * if self.count(Module::Radius) > 0 { 2.0 } else { 1.0 }
    }

    /// H4: cells aside from the way and blocks up from it.
    pub fn zone(&self, t: &Tuning) -> (i32, i32) {
        let i = usize::from(self.count(Module::Radius) > 0);
        (t.tool_side[i], t.tool_height[i])
    }

    /// Every cell of its tool's zone round rail cell `cell` whose rail hangs on `mount`: `height`
    /// layers away from the block it hangs on (up from a floor rail, down from a ceiling rail, out
    /// from a wall), `side` cells across, the layer at the rail first. A reaper on a wall rail
    /// also reaches the wall's own layer beside the block its rail hangs on: cocoa on the trunk's
    /// other faces (guide `tool_reaper`, «по стволу джунглей за какао»).
    pub fn zone_cells(&self, cell: Pos, mount: Face, t: &Tuning) -> Vec<Pos> {
        let (side, height) = self.zone(t);
        let out_dir = mount.opposite().offset();
        let [u, v] = across(mount);
        let wall = !matches!(mount, Face::YNeg | Face::YPos);
        let first = if wall && self.tool == Some(Tool::Reaper) {
            -1
        } else {
            0
        };
        let mut out = Vec::new();
        for depth in first..height {
            for a in -side..=side {
                for b in -side..=side {
                    if depth < 0 && a == 0 && b == 0 {
                        continue;
                    }
                    out.push(moved(moved(moved(cell, out_dir, depth), u, a), v, b));
                }
            }
        }
        out
    }

    /// The corners of its zone round `cell`, both included.
    pub fn zone_box(&self, cell: Pos, mount: Face, t: &Tuning) -> (Pos, Pos) {
        let mut lo = cell;
        let mut hi = cell;
        for p in self.zone_cells(cell, mount, t) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, hi)
    }

    /// Ticks a job of `ticks` takes: speed upgrades make the tool faster (A9: in full).
    pub fn work_ticks(&self, ticks: f64, t: &Tuning) -> f64 {
        ticks / t.speed_work.powi(self.count(Module::WorkSpeed) as i32)
    }

    /// Where it is: between the middles of the floors of its cell and the next.
    pub fn feet(&self) -> [f64; 3] {
        let at = |p: Pos| {
            [
                f64::from(p[0]) + 0.5,
                f64::from(p[1]),
                f64::from(p[2]) + 0.5,
            ]
        };
        let a = at(self.cell);
        let Some(to) = self.to else {
            return a;
        };
        let b = at(to);
        let k = self.progress.clamp(0.0, 1.0);
        [
            a[0] + (b[0] - a[0]) * k,
            a[1] + (b[1] - a[1]) * k,
            a[2] + (b[2] - a[2]) * k,
        ]
    }

    /// Its heading in the base game's yaw (0 south, 90 west), if it has one.
    pub fn yaw(&self) -> Option<f32> {
        let (a, b) = match (self.from, self.to) {
            (_, Some(to)) => (self.cell, to),
            (Some(from), None) => (from, self.cell),
            _ => return None,
        };
        let (dx, dz) = (f64::from(b[0] - a[0]), f64::from(b[2] - a[2]));
        (dx.hypot(dz) > 0.0).then(|| (-dx).atan2(dz).to_degrees() as f32)
    }

    /// The next cell on its work: straight on, else the first other way, else back.
    fn next_on_work(&self, net: &Network) -> Option<Pos> {
        let all = ways(net, self.cell);
        let forward: Vec<Pos> = all
            .iter()
            .copied()
            .filter(|&w| Some(w) != self.from)
            .collect();
        if forward.is_empty() {
            return self.from.filter(|f| all.contains(f));
        }
        if let Some(f) = self.from {
            let d = [self.cell[0] - f[0], self.cell[1] - f[1], self.cell[2] - f[2]];
            if let Some(&w) = forward.iter().find(|w| {
                [w[0] - self.cell[0], w[1] - self.cell[1], w[2] - self.cell[2]] == d
            }) {
                return Some(w);
            }
        }
        forward.first().copied()
    }

    /// One tick of riding.
    pub fn tick(&mut self, track: &Track<'_>, t: &Tuning) -> Step {
        self.carry = 0.0;
        self.advance(track, t, None)
    }

    /// Rides on in the same tick with what it rode past its last cell (`rides_on`).
    pub fn ride_on(&mut self, track: &Track<'_>, t: &Tuning) -> Step {
        let carried = std::mem::take(&mut self.carry);
        self.advance(track, t, Some(carried))
    }

    /// A tick's ride: `gain` of a cell, or its rail's speed for a tick.
    fn advance(&mut self, track: &Track<'_>, t: &Tuning, gain: Option<f64>) -> Step {
        if self.busy > 0.0 {
            self.busy = (self.busy - 1.0).max(0.0);
            return Step::Busy;
        }
        if self.errand == Errand::Work {
            if self.tool.is_none() {
                self.note = Note::NoTool;
                return Step::Stopped;
            }
            if self.hold.is_full() {
                self.errand = Errand::Unload;
            } else if self.charge < t.worker_charge_below * self.capacity(t) {
                self.errand = Errand::Charge;
            } else if self.tool == Some(Tool::Hold) {
                // W6: a cargo worker's work is the ride from the load dock to the unload dock.
                self.errand = Errand::Load;
            }
        }
        if self.to.is_none() {
            match self.errand {
                Errand::Work => {
                    self.to = self.next_on_work(track.net);
                    if self.to.is_none() {
                        self.note = Note::NoWay;
                        return Step::Stopped;
                    }
                    self.note = Note::Working;
                }
                Errand::Unload | Errand::Charge | Errand::Load => {
                    let (docks, none, going) = match self.errand {
                        Errand::Unload => (track.unload, Note::NoUnloadDock, Note::ToUnload),
                        Errand::Charge => (track.charge, Note::NoChargeDock, Note::ToCharge),
                        _ => (track.load, Note::NoLoadDock, Note::ToLoad),
                    };
                    let docks = dock_cells(track.net, docks);
                    if let Some(&dock) = docks.get(&self.cell) {
                        return Step::AtDock(dock);
                    }
                    let Some(path) = way_to(track.net, self.cell, &docks) else {
                        self.note = none;
                        return Step::Stopped;
                    };
                    self.to = path.first().copied();
                    self.note = going;
                }
            }
        }
        let Some(to) = self.to else {
            return Step::Stopped;
        };
        if !on_rail(track.net, to) {
            // The rail ahead is gone: look again from here.
            self.to = None;
            self.progress = 0.0;
            return Step::Stopped;
        }
        if self.charge <= 0.0 {
            self.charge = 0.0;
            self.note = Note::Flat;
            return Step::Stopped;
        }
        // T4: as fast as the rail it rides onto (guide `monorail`: 8 / 14 / 22 / 32 bl/s); what
        // it rode past a cell last tick comes first.
        self.progress += gain.unwrap_or_else(|| self.speed(track.net, to, t) / t.ticks_per_second);
        if self.progress < 1.0 - 1e-9 {
            return Step::Moved;
        }
        self.carry = (self.progress - 1.0).max(0.0);
        self.from = Some(self.cell);
        self.cell = to;
        self.to = None;
        self.progress = 0.0;
        self.charge = (self.charge - t.worker_energy_per_block).max(0.0);
        if self.errand == Errand::Work {
            Step::Arrived(to)
        } else {
            Step::Moved
        }
    }

    /// Blocks a second on the rail at `to`: its tier's speed (T4).
    pub fn speed(&self, net: &Network, to: Pos, t: &Tuning) -> f64 {
        let tier = net.rails.get(&to).map_or(1, |r| r.tier);
        t.rail_spec().speed_of(tier)
    }

    /// Whether it rode past a cell this tick and rides on in the same tick.
    pub fn rides_on(&self) -> bool {
        self.carry > 1e-9 && self.busy <= 0.0
    }

    /// Spends a tool job: `ticks` of work and `energy` E.
    pub fn spend(&mut self, ticks: f64, energy: f64, t: &Tuning) {
        self.busy += self.work_ticks(ticks, t);
        self.charge = (self.charge - energy).max(0.0);
    }

    /// At an unload dock: whether it has nothing more to give (only what its tool keeps).
    pub fn unloaded(&self) -> bool {
        let tool = self.tool;
        self.hold
            .next_to_unload(|i| tool.is_some_and(|t| t.keeps(i)))
            .is_none()
    }

    /// At a load dock: whether it rides on to unload. A full hold does; so does a hold with
    /// something in it when the dock has nothing more to give. An empty hold at an empty dock
    /// waits there for cargo.
    pub fn loaded(&mut self, dock_empty: bool) -> bool {
        if self.hold.is_full() || (dock_empty && self.hold.used() > 0) {
            self.errand = Errand::Unload;
            return true;
        }
        self.note = if dock_empty {
            Note::NoCargo
        } else {
            Note::Loading
        };
        false
    }

    /// Its tool came into a cell and found `done` things to do there: after `idle_after` cells in
    /// a row with nothing, the scoop says so (guide `tool_scoop`).
    pub fn found(&mut self, done: usize, idle_after: u32) {
        if done > 0 {
            self.idle = 0;
            if self.note == Note::NothingToDig {
                self.note = Note::Working;
            }
            return;
        }
        self.idle = self.idle.saturating_add(1);
        if self.idle >= idle_after {
            self.note = Note::NothingToDig;
        }
    }

    /// W3: the worker as its chassis item carries it (custom data `rustcraft:worker`): tool, upgrades,
    /// hold, charge and the rest of what it is, as JSON.
    pub fn packed(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// A packed worker set on rail `cell` for `owner` under `id`: what it carried kept, its way
    /// and errand started afresh. None when `packed` does not read.
    pub fn unpack(packed: &str, id: u64, owner: u128, cell: Pos, t: &Tuning) -> Option<Worker> {
        let old: Worker = serde_json::from_str(packed).ok()?;
        let mut w = Worker::new(id, owner, cell, t);
        w.tool = old.tool;
        w.hold = old.hold;
        w.charge = old.charge;
        w.set_tier(old.tier, t);
        w.upgrades = old.upgrades;
        w.note = if w.tool.is_some() { Note::Working } else { Note::NoTool };
        // Its own upgrades size its hold as before it was packed: nothing is left over.
        let over = w.refit(t);
        debug_assert!(over.is_empty());
        Some(w)
    }

    /// Back to work once the errand is done.
    pub fn back_to_work(&mut self) {
        self.errand = Errand::Work;
        self.note = Note::Working;
    }

    /// Its upgrades and hold changed: the hold takes its new size; what no longer fits comes
    /// back, and the charge keeps within the battery.
    pub fn refit(&mut self, t: &Tuning) -> Vec<hold::Stack> {
        let size = self.hold_size(t);
        let (h, out) = std::mem::take(&mut self.hold).resized(size);
        self.hold = h;
        self.charge = self.charge.min(self.capacity(t));
        out
    }
}

/// The level's workers, saved as one JSON string in a level attachment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fleet {
    pub next_id: u64,
    pub workers: Vec<Worker>,
}

impl Fleet {
    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn decode(s: &str) -> Option<Fleet> {
        serde_json::from_str(s).ok()
    }

    /// A new worker of `owner` on rail `cell`.
    pub fn add(&mut self, owner: u128, cell: Pos, t: &Tuning) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.workers.push(Worker::new(id, owner, cell, t));
        id
    }

    /// W3: a worker taken away as one item set on rail `cell` again for `owner`, with all it
    /// carried; its new id. None when `packed` does not read.
    pub fn put_back(&mut self, owner: u128, cell: Pos, packed: &str, t: &Tuning) -> Option<u64> {
        let id = self.next_id + 1;
        let w = Worker::unpack(packed, id, owner, cell, t)?;
        self.next_id = id;
        self.workers.push(w);
        Some(id)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::monorail::Rail;

    /// Blocks by position for the tools' tests.
    #[derive(Default)]
    pub struct World {
        pub blocks: HashMap<Pos, (String, Vec<(String, String)>)>,
    }

    impl World {
        pub fn set(&mut self, p: Pos, block: &str, props: &[(&str, &str)]) {
            self.blocks.insert(
                p,
                (
                    block.to_owned(),
                    props
                        .iter()
                        .map(|(k, v)| ((*k).into(), (*v).into()))
                        .collect(),
                ),
            );
        }
    }

    impl BlockView for World {
        fn block(&self, p: Pos) -> String {
            self.blocks
                .get(&p)
                .map_or_else(|| "minecraft:air".to_owned(), |b| b.0.clone())
        }
        fn prop(&self, p: Pos, key: &str) -> Option<String> {
            self.blocks
                .get(&p)?
                .1
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        }
        fn is_log(&self, p: Pos) -> bool {
            self.block(p).ends_with("_log")
        }
        fn is_leaves(&self, p: Pos) -> bool {
            self.block(p).ends_with("_leaves")
        }
        fn is_dirt(&self, p: Pos) -> bool {
            matches!(
                self.block(p).as_str(),
                "minecraft:dirt" | "minecraft:grass_block"
            )
        }
    }

    fn line(n: i32) -> Network {
        let mut net = Network::default();
        for x in 0..n {
            net.rails.insert(
                [x, 0, 0],
                Rail {
                    tier: 1,
                    mount: Face::YNeg,
                },
            );
        }
        net
    }

    fn worker(t: &Tuning) -> Worker {
        let mut w = Worker::new(1, 7, [0, 0, 0], t);
        w.tool = Some(Tool::Saw);
        w
    }

    fn ride(w: &mut Worker, track: &Track<'_>, t: &Tuning, ticks: usize) -> Vec<Step> {
        (0..ticks).map(|_| w.tick(track, t)).collect()
    }

    #[test]
    fn a_worker_shuttles_along_its_rails() {
        let t = Tuning::default();
        let net = line(3);
        let none = BTreeSet::new();
        let track = Track {
            net: &net,
            unload: &none,
            charge: &none,
            load: &none,
        };
        let mut w = worker(&t);
        let per_cell = (t.ticks_per_second / t.rail_spec().speed_of(1)).ceil() as usize;
        let mut cells = Vec::new();
        for s in ride(&mut w, &track, &t, per_cell * 5) {
            if let Step::Arrived(p) = s {
                cells.push(p[0]);
            }
        }
        assert_eq!(cells, vec![1, 2, 1, 0, 1]);
        assert!(w.charge < t.worker_battery);
    }

    #[test]
    fn a_worker_rides_as_fast_as_its_rail_tier() {
        // One second on a long line of each tier, riding on while it carries, as the bundle does:
        // the tier's blocks a second, every cell arrived at.
        let t = Tuning::default();
        let none = BTreeSet::new();
        for tier in 1..=4u8 {
            let mut net = line(80);
            for r in net.rails.values_mut() {
                r.tier = tier;
            }
            let track = Track {
                net: &net,
                unload: &none,
                charge: &none,
                load: &none,
            };
            let mut w = worker(&t);
            let mut arrived = 0;
            for _ in 0..t.ticks_per_second as usize {
                let mut step = w.tick(&track, &t);
                loop {
                    if let Step::Arrived(_) = step {
                        arrived += 1;
                    }
                    if !w.rides_on() {
                        break;
                    }
                    step = w.ride_on(&track, &t);
                }
            }
            let speed = t.rail_spec().speed_of(tier);
            assert!(
                (arrived as f64 - speed).abs() <= 1.0,
                "tier {tier}: {arrived} cells in a second, wanted {speed}"
            );
            assert_eq!(w.cell[0], arrived);
        }
    }

    #[test]
    fn a_full_hold_rides_to_the_unload_dock() {
        let t = Tuning::default();
        let net = line(6);
        let unload = BTreeSet::from([[5, 0, 1]]);
        let none = BTreeSet::new();
        let track = Track {
            net: &net,
            unload: &unload,
            charge: &none,
            load: &none,
        };
        let mut w = worker(&t);
        for _ in 0..t.worker_hold {
            w.hold.add("minecraft:oak_log", 64, 64);
            w.hold.add("x", 1, 1);
        }
        assert!(w.hold.is_full());
        let steps = ride(&mut w, &track, &t, 100);
        assert_eq!(w.errand, Errand::Unload);
        assert_eq!(w.cell, [5, 0, 0]);
        assert_eq!(steps.last(), Some(&Step::AtDock([5, 0, 1])));
        assert!(
            !steps.iter().any(|s| matches!(s, Step::Arrived(_))),
            "no work on the errand"
        );
    }

    #[test]
    fn with_no_dock_on_its_rails_it_stops_and_says_so() {
        let t = Tuning::default();
        let net = line(4);
        let far = BTreeSet::from([[50, 0, 1]]);
        let track = Track {
            net: &net,
            unload: &far,
            charge: &far,
            load: &far,
        };
        let mut w = worker(&t);
        w.charge = 0.1 * t.worker_battery;
        assert_eq!(w.tick(&track, &t), Step::Stopped);
        assert_eq!(w.note, Note::NoChargeDock);
    }

    #[test]
    fn upgrades_make_the_battery_and_hold_bigger_and_the_tool_faster() {
        let t = Tuning::default();
        let mut w = worker(&t);
        w.upgrades = vec![
            Some(Module::Battery),
            Some(Module::Storage),
            Some(Module::WorkSpeed),
        ];
        assert_eq!(w.capacity(&t), t.worker_battery * t.worker_battery_upgrade);
        assert!(w.refit(&t).is_empty());
        assert_eq!(
            w.hold.slots.len(),
            t.worker_hold + t.worker_hold_per_storage
        );
        assert_eq!(w.work_ticks(8.0, &t), 8.0 / t.speed_work);
        w.upgrades[0] = Some(Module::Radius);
        assert_eq!(w.zone(&t), (t.tool_side[1], t.tool_height[1]));
    }

    #[test]
    fn a_cargo_worker_loads_then_unloads_and_comes_back() {
        // W6: the cargo hold rides to the load dock, loads, rides to the unload dock.
        let t = Tuning::default();
        let net = line(6);
        let load = BTreeSet::from([[0, 0, 1]]);
        let unload = BTreeSet::from([[5, 0, 1]]);
        let none = BTreeSet::new();
        let track = Track {
            net: &net,
            unload: &unload,
            charge: &none,
            load: &load,
        };
        let mut w = Worker::new(1, 7, [2, 0, 0], &t);
        w.tool = Some(Tool::Hold);
        assert!(w.refit(&t).is_empty());
        assert_eq!(w.hold.slots.len(), t.cargo_hold);
        let steps = ride(&mut w, &track, &t, 60);
        assert_eq!(w.errand, Errand::Load);
        assert_eq!(steps.last(), Some(&Step::AtDock([0, 0, 1])));
        // An empty dock and an empty hold: it waits for cargo.
        assert!(!w.loaded(true));
        assert_eq!(w.note, Note::NoCargo);
        w.hold.add("minecraft:cobblestone", 10, 64);
        assert!(!w.loaded(false));
        assert!(w.loaded(true));
        let steps = ride(&mut w, &track, &t, 100);
        assert_eq!(steps.last(), Some(&Step::AtDock([5, 0, 1])));
        assert!(!w.unloaded());
        w.hold.take("minecraft:cobblestone", 10);
        assert!(w.unloaded());
        w.back_to_work();
        w.tick(&track, &t);
        assert_eq!(w.errand, Errand::Load);
        // No load dock on its rails: it says so.
        let track = Track {
            net: &net,
            unload: &unload,
            charge: &none,
            load: &none,
        };
        let mut w2 = Worker::new(2, 7, [2, 0, 0], &t);
        w2.tool = Some(Tool::Hold);
        assert_eq!(w2.tick(&track, &t), Step::Stopped);
        assert_eq!(w2.note, Note::NoLoadDock);
    }

    #[test]
    fn the_scoop_says_when_it_found_nothing_for_a_while() {
        let t = Tuning::default();
        let mut w = Worker::new(1, 7, [0, 0, 0], &t);
        w.tool = Some(Tool::Scoop);
        for _ in 0..3 {
            w.found(0, 4);
        }
        assert_ne!(w.note, Note::NothingToDig);
        w.found(0, 4);
        assert_eq!(w.note, Note::NothingToDig);
        w.found(2, 4);
        assert_eq!((w.note, w.idle), (Note::Working, 0));
    }

    #[test]
    fn a_fleet_survives_its_json() {
        let t = Tuning::default();
        let mut f = Fleet::default();
        let id = f.add(0xabc, [1, 2, 3], &t);
        f.workers[0].hold.add("minecraft:wheat", 3, 64);
        f.workers[0].tool = Some(Tool::Reaper);
        let back = Fleet::decode(&f.encode()).unwrap();
        assert_eq!(back, f);
        assert_eq!(back.workers[0].id, id);
        // A fleet saved before slice 10 (no `idle`) still reads.
        let old = f.encode().replace(",\"idle\":0", "");
        assert!(!old.contains("idle"));
        assert_eq!(Fleet::decode(&old), Some(f));
    }

    fn rail(net: &mut Network, p: Pos, mount: Face) {
        net.rails.insert(p, Rail { tier: 1, mount });
    }

    #[test]
    fn a_worker_rides_up_a_wall_rail_and_onto_the_ceiling() {
        // A floor rail, a wall rail up three blocks (the wall at x = -1), a ceiling rail on top.
        let t = Tuning::default();
        let mut net = Network::default();
        rail(&mut net, [1, 0, 0], Face::YNeg);
        rail(&mut net, [0, 0, 0], Face::XNeg);
        for y in 1..=3 {
            rail(&mut net, [0, y, 0], Face::XNeg);
        }
        rail(&mut net, [1, 3, 0], Face::YPos);
        let none = BTreeSet::new();
        let track = Track {
            net: &net,
            unload: &none,
            charge: &none,
            load: &none,
        };
        let mut w = Worker::new(1, 7, [1, 0, 0], &t);
        w.tool = Some(Tool::Reaper);
        let mut cells = Vec::new();
        for _ in 0..2000 {
            if let Step::Arrived(p) = w.tick(&track, &t) {
                cells.push(p);
            }
            if cells.len() == 6 {
                break;
            }
        }
        assert_eq!(
            cells,
            vec![[0, 0, 0], [0, 1, 0], [0, 2, 0], [0, 3, 0], [1, 3, 0], [0, 3, 0]],
            "up the wall, onto the ceiling, back at its end"
        );
    }

    #[test]
    fn the_zone_grows_away_from_the_mount() {
        let t = Tuning::default();
        let mut w = Worker::new(1, 7, [0, 0, 0], &t);
        w.tool = Some(Tool::Saw);
        let (side, height) = w.zone(&t);
        // A floor rail: up from the rail, as before slice 18.
        let floor = w.zone_cells([0, 10, 0], Face::YNeg, &t);
        assert_eq!(floor.len() as i32, height * (2 * side + 1).pow(2));
        assert!(floor.iter().all(|p| p[1] >= 10 && p[1] < 10 + height));
        // A ceiling rail: down from it.
        let ceiling = w.zone_box([0, 10, 0], Face::YPos, &t);
        assert_eq!((ceiling.0[1], ceiling.1[1]), (10 - height + 1, 10));
        // A wall rail on a wall to the west: out east, across y and z.
        let wall = w.zone_box([0, 10, 0], Face::XNeg, &t);
        assert_eq!((wall.0[0], wall.1[0]), (0, height - 1));
        assert_eq!((wall.0[1], wall.1[1]), (10 - side, 10 + side));
        // A reaper on a wall rail also reaches the wall's layer, never the block it hangs on.
        w.tool = Some(Tool::Reaper);
        let reach = w.zone_cells([0, 10, 0], Face::XNeg, &t);
        assert!(reach.contains(&[-1, 10, 1]), "a pod on the trunk's side face");
        assert!(!reach.contains(&[-1, 10, 0]), "the trunk itself");
        assert!(!w.zone_cells([0, 10, 0], Face::YNeg, &t).contains(&[0, 9, 1]));
    }

    #[test]
    fn docks_beside_a_wall_rail_serve() {
        let t = Tuning::default();
        let mut net = Network::default();
        for y in 0..4 {
            rail(&mut net, [0, y, 0], Face::XNeg);
        }
        let unload = BTreeSet::from([[1, 3, 0]]);
        let none = BTreeSet::new();
        let track = Track {
            net: &net,
            unload: &unload,
            charge: &none,
            load: &none,
        };
        let mut w = worker(&t);
        w.cell = [0, 0, 0];
        for _ in 0..t.worker_hold {
            w.hold.add("x", 1, 1);
        }
        let steps = ride(&mut w, &track, &t, 400);
        assert_eq!(steps.last(), Some(&Step::AtDock([1, 3, 0])));
        assert_eq!(w.cell, [0, 3, 0]);
    }

    #[test]
    fn a_worker_packs_into_its_chassis_and_back() {
        let t = Tuning::default();
        let mut f = Fleet::default();
        let id = f.add(7, [1, 2, 3], &t);
        let w = &mut f.workers[0];
        w.tool = Some(Tool::Reaper);
        w.upgrades[1] = Some(Module::Storage);
        w.refit(&t);
        w.hold.add("minecraft:wheat_seeds", 40, 64);
        w.charge = 123.0;
        w.errand = Errand::Unload;
        let packed = w.packed();
        f.workers.clear();
        let back = f.put_back(9, [5, 6, 7], &packed, &t).unwrap();
        assert_ne!(back, id);
        let w = &f.workers[0];
        assert_eq!((w.owner, w.cell, w.tool), (9, [5, 6, 7], Some(Tool::Reaper)));
        assert_eq!(w.upgrades[1], Some(Module::Storage));
        assert_eq!(w.hold.count("minecraft:wheat_seeds"), 40);
        assert_eq!(w.hold.slots.len(), t.worker_hold + t.worker_hold_per_storage);
        assert_eq!((w.charge, w.errand), (123.0, Errand::Work));
        assert!(f.put_back(9, [0, 0, 0], "not json", &t).is_none());
    }

    #[test]
    fn a_guard_shoots_twice_as_far_with_a_radius_upgrade() {
        let t = Tuning::default();
        let mut w = Worker::new(1, 7, [0, 0, 0], &t);
        w.tool = Some(Tool::Shooter);
        assert_eq!(Tool::of("rustcraft:tool_shooter"), Some(Tool::Shooter));
        assert_eq!(w.guard_range(&t), t.guard_range);
        w.upgrades[0] = Some(Module::Radius);
        assert_eq!(w.guard_range(&t), 2.0 * t.guard_range);
        assert!(!Tool::Shooter.keeps("minecraft:arrow"));
    }
}
