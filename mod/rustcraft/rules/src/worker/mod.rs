//! The worker cart (spec 4.7, W1–W7; guide `chassis`, `tool_saw`, `tool_reaper`, `dock_unload`,
//! `dock_charge`, `upgrades`). A worker is a chassis on the monorail with one tool, upgrades, a
//! hold and a battery. It rides its own stretch of rail on its own (W2, W4): straight on where it
//! can, back at a dead end, round a loop; its tool works the cells along the way. With a full
//! hold it rides to the nearest unload dock on its rails (W4.a), with a battery under the
//! threshold to the nearest charge dock (W4.b); with no such dock on its rails it stops and says
//! so (W4.c). These rules move it and choose; the bundle breaks blocks, moves items and energy and
//! draws it.
//!
//! In this slice a worker rides floor rails only: a wall or ceiling rail ends its way.

pub mod hold;
pub mod plants;
pub mod trees;

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

/// A worker's tool (spec 4.7 table; slice 7 has the two of slice 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tool {
    Saw,
    Reaper,
}

impl Tool {
    pub const ALL: [Tool; 2] = [Tool::Saw, Tool::Reaper];

    pub fn item(self) -> &'static str {
        match self {
            Tool::Saw => "rustcraft:tool_saw",
            Tool::Reaper => "rustcraft:tool_reaper",
        }
    }

    pub fn of(item: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.item() == item)
    }

    /// The tool word of Path-of-craft-tree's `break` report (contract, «break»).
    pub fn report_word(self) -> &'static str {
        match self {
            Tool::Saw => "axe",
            Tool::Reaper => "hoe",
        }
    }

    /// What the unload dock leaves in the hold for this tool: saplings, seeds.
    pub fn keeps(self, item: &str) -> bool {
        match self {
            Tool::Saw => trees::is_sapling(item),
            Tool::Reaper => plants::is_seed(item),
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
}

/// Whether `p` is a floor rail: the rails a worker rides in this slice.
fn floor(net: &Network, p: Pos) -> bool {
    net.rails.get(&p).is_some_and(|r| r.mount == Face::YNeg)
}

/// The floor rails a worker in `p` can ride to.
pub fn ways(net: &Network, p: Pos) -> Vec<Pos> {
    neighbours(net, p)
        .into_iter()
        .filter(|&n| floor(net, n))
        .collect()
}

/// The floor rail cells beside each dock of `docks`, by a face: where a worker stops for it.
pub fn dock_cells(net: &Network, docks: &BTreeSet<Pos>) -> BTreeMap<Pos, Pos> {
    let mut out = BTreeMap::new();
    for &d in docks {
        for f in Face::ALL {
            let r = step(d, f);
            if floor(net, r) {
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
        }
    }

    fn count(&self, kind: Module) -> u32 {
        modules::count(&self.upgrades, kind)
    }

    /// The battery's size: battery upgrades multiply it in full (A9).
    pub fn capacity(&self, t: &Tuning) -> f64 {
        t.worker_battery
            * t.worker_battery_upgrade
                .powi(self.count(Module::Battery) as i32)
    }

    /// The hold's size with its storage modules.
    pub fn hold_size(&self, t: &Tuning) -> usize {
        (t.worker_hold + t.worker_hold_per_storage * self.count(Module::Storage) as usize)
            .min(t.worker_hold_max)
    }

    /// H4: cells aside from the way and blocks up from it.
    pub fn zone(&self, t: &Tuning) -> (i32, i32) {
        let i = usize::from(self.count(Module::Radius) > 0);
        (t.tool_side[i], t.tool_height[i])
    }

    /// Every cell of its tool's zone round rail cell `cell`, bottom up.
    pub fn zone_cells(&self, cell: Pos, t: &Tuning) -> Vec<Pos> {
        let (side, height) = self.zone(t);
        let mut out = Vec::new();
        for dy in 0..height {
            for dx in -side..=side {
                for dz in -side..=side {
                    out.push([cell[0] + dx, cell[1] + dy, cell[2] + dz]);
                }
            }
        }
        out
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
            let d = [self.cell[0] - f[0], self.cell[2] - f[2]];
            if let Some(&w) = forward
                .iter()
                .find(|w| [w[0] - self.cell[0], w[2] - self.cell[2]] == d)
            {
                return Some(w);
            }
        }
        forward.first().copied()
    }

    /// One tick of riding.
    pub fn tick(&mut self, track: &Track<'_>, t: &Tuning) -> Step {
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
                Errand::Unload | Errand::Charge => {
                    let unload = self.errand == Errand::Unload;
                    let docks =
                        dock_cells(track.net, if unload { track.unload } else { track.charge });
                    if let Some(&dock) = docks.get(&self.cell) {
                        return Step::AtDock(dock);
                    }
                    let Some(path) = way_to(track.net, self.cell, &docks) else {
                        self.note = if unload {
                            Note::NoUnloadDock
                        } else {
                            Note::NoChargeDock
                        };
                        return Step::Stopped;
                    };
                    self.to = path.first().copied();
                    self.note = if unload {
                        Note::ToUnload
                    } else {
                        Note::ToCharge
                    };
                }
            }
        }
        let Some(to) = self.to else {
            return Step::Stopped;
        };
        if !floor(track.net, to) {
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
        self.progress += t.worker_speed / t.ticks_per_second;
        if self.progress < 1.0 - 1e-9 {
            return Step::Moved;
        }
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
        };
        let mut w = worker(&t);
        let per_cell = (t.ticks_per_second / t.worker_speed).ceil() as usize;
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
    fn a_full_hold_rides_to_the_unload_dock() {
        let t = Tuning::default();
        let net = line(6);
        let unload = BTreeSet::from([[5, 0, 1]]);
        let none = BTreeSet::new();
        let track = Track {
            net: &net,
            unload: &unload,
            charge: &none,
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
    fn a_fleet_survives_its_json() {
        let t = Tuning::default();
        let mut f = Fleet::default();
        let id = f.add(0xabc, [1, 2, 3], &t);
        f.workers[0].hold.add("minecraft:wheat", 3, 64);
        f.workers[0].tool = Some(Tool::Reaper);
        let back = Fleet::decode(&f.encode()).unwrap();
        assert_eq!(back, f);
        assert_eq!(back.workers[0].id, id);
    }
}
