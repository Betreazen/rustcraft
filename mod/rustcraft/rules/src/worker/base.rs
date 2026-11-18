//! The worker's base (0.19.1, the owner: «каждому работнику нужен блок зарядки и блок разгрузки, а
//! грузовому ещё блок погрузки; без них работник стоит. Бур всегда уходит от зарядки и
//! разгрузчика»). Every stretch of connected rail is one part; a part has a dock of a kind when one
//! stands beside one of its cells (`dock_cells`). A worker on a part without an unload dock, a
//! charge dock or (a cargo worker) a load dock stands and says which it lacks, unload first.
//!
//! The bore bores only at the end of its part farthest by rail from the nearest dock, away from
//! the docks: on a line with its docks at one end, past the other end; on a single rail cell, away
//! from the side its docks stand on. A part with no end (a loop) gives it nowhere to bore.

use super::{Note, dock_cells, ways};
use crate::geom::Pos;
use crate::monorail::Network;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// Where the bore of a part bores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoreWay {
    /// At rail cell `cell`, towards `heading` (a unit step along x or z).
    End { cell: Pos, heading: [i32; 2] },
    /// No end it may bore from: a loop, or ends that only climb.
    NoEnd,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Part {
    unload: bool,
    charge: bool,
    load: bool,
    bore: BoreWay,
}

/// The parts of a level's rails and the docks each has, worked out once a tick for all workers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bases {
    part_of: HashMap<Pos, usize>,
    parts: Vec<Part>,
}

impl Bases {
    pub fn of(
        net: &Network,
        unload: &BTreeSet<Pos>,
        charge: &BTreeSet<Pos>,
        load: &BTreeSet<Pos>,
    ) -> Bases {
        let raw: Vec<Pos> = unload.iter().chain(charge).chain(load).copied().collect();
        let (unload, charge, load) = (
            dock_cells(net, unload),
            dock_cells(net, charge),
            dock_cells(net, load),
        );
        let at_docks: BTreeSet<Pos> = unload.keys().chain(charge.keys()).chain(load.keys()).copied().collect();
        let mut bases = Bases::default();
        for &start in net.rails.keys() {
            if bases.part_of.contains_key(&start) {
                continue;
            }
            let id = bases.parts.len();
            let mut cells = vec![start];
            bases.part_of.insert(start, id);
            let mut i = 0;
            while i < cells.len() {
                for n in ways(net, cells[i]) {
                    if let std::collections::hash_map::Entry::Vacant(e) = bases.part_of.entry(n) {
                        e.insert(id);
                        cells.push(n);
                    }
                }
                i += 1;
            }
            let has = |docks: &BTreeMap<Pos, Pos>| cells.iter().any(|c| docks.contains_key(c));
            bases.parts.push(Part {
                unload: has(&unload),
                charge: has(&charge),
                load: has(&load),
                bore: bore_way(net, &cells, &at_docks, &raw),
            });
        }
        bases
    }

    /// For the riding tests: every part of `net` has every dock, and no end to bore from.
    #[cfg(test)]
    pub fn complete(net: &Network) -> Bases {
        let mut bases = Bases::of(net, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new());
        for part in &mut bases.parts {
            *part = Part { unload: true, charge: true, load: true, bore: BoreWay::NoEnd };
        }
        bases
    }

    fn part(&self, cell: Pos) -> Option<&Part> {
        self.part_of.get(&cell).map(|&i| &self.parts[i])
    }

    /// What the worker on rail cell `cell` lacks of its base, unload dock first; a cargo worker
    /// also needs a load dock. None on a part that has them, or off the rails.
    pub fn lacks(&self, cell: Pos, cargo: bool) -> Option<Note> {
        let part = self.part(cell)?;
        if !part.unload {
            Some(Note::NoUnloadDock)
        } else if !part.charge {
            Some(Note::NoChargeDock)
        } else if cargo && !part.load {
            Some(Note::NoLoadDock)
        } else {
            None
        }
    }

    /// Where a bore on rail cell `cell` bores.
    pub fn bore_way(&self, cell: Pos) -> Option<BoreWay> {
        self.part(cell).map(|p| p.bore)
    }
}

/// The unit step along x or z from `from` to `to`, if they are a step apart across.
fn across(from: Pos, to: Pos) -> Option<[i32; 2]> {
    let d = [to[0] - from[0], to[2] - from[2]];
    (d[0].abs() + d[1].abs() == 1).then_some(d)
}

/// Where the part of `cells` bores: `at_docks` are the rail cells beside a dock, `docks` the
/// docks themselves.
fn bore_way(net: &Network, cells: &[Pos], at_docks: &BTreeSet<Pos>, docks: &[Pos]) -> BoreWay {
    if let [only] = cells {
        // A single rail cell: away from the side its docks stand on.
        let mut sum = [0, 0];
        for d in docks {
            let off = [d[0] - only[0], d[1] - only[1], d[2] - only[2]];
            if off.iter().map(|c| c.abs()).sum::<i32>() == 1 {
                sum[0] += off[0];
                sum[1] += off[2];
            }
        }
        let heading = if sum[0].abs() > sum[1].abs() {
            [-sum[0].signum(), 0]
        } else if sum[1] != 0 {
            [0, -sum[1].signum()]
        } else {
            return BoreWay::NoEnd;
        };
        return BoreWay::End { cell: *only, heading };
    }
    // The ends of the part, each with the step past it.
    let ends: Vec<(Pos, [i32; 2])> = cells
        .iter()
        .filter_map(|&c| match ways(net, c).as_slice() {
            [n] => across(*n, c).map(|h| (c, h)),
            _ => None,
        })
        .collect();
    if ends.is_empty() {
        return BoreWay::NoEnd;
    }
    // How far by rail each cell is from the nearest dock.
    let mut far: HashMap<Pos, u32> = HashMap::new();
    let mut queue: VecDeque<Pos> = VecDeque::new();
    for c in cells.iter().filter(|c| at_docks.contains(*c)) {
        far.insert(*c, 0);
        queue.push_back(*c);
    }
    while let Some(p) = queue.pop_front() {
        let d = far[&p];
        for n in ways(net, p) {
            if let std::collections::hash_map::Entry::Vacant(e) = far.entry(n) {
                e.insert(d + 1);
                queue.push_back(n);
            }
        }
    }
    ends.into_iter()
        .max_by(|a, b| {
            let fa = far.get(&a.0).copied().unwrap_or(0);
            let fb = far.get(&b.0).copied().unwrap_or(0);
            fa.cmp(&fb).then(b.0.cmp(&a.0))
        })
        .map_or(BoreWay::NoEnd, |(cell, heading)| BoreWay::End { cell, heading })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Face;
    use crate::monorail::Rail;

    fn rails(cells: &[Pos]) -> Network {
        let mut net = Network::default();
        for &c in cells {
            net.rails.insert(c, Rail { tier: 1, mount: Face::YNeg });
        }
        net
    }

    fn line(n: i32) -> Vec<Pos> {
        (0..n).map(|x| [x, 0, 0]).collect()
    }

    #[test]
    fn a_worker_needs_an_unload_dock_then_a_charge_dock() {
        let net = rails(&line(5));
        let none = BTreeSet::new();
        let unload = BTreeSet::from([[0, 0, 1]]);
        let charge = BTreeSet::from([[4, 0, -1]]);
        let load = BTreeSet::from([[2, 0, 1]]);
        let lacks = |u: &BTreeSet<Pos>, c: &BTreeSet<Pos>, l: &BTreeSet<Pos>, cargo: bool| {
            Bases::of(&net, u, c, l).lacks([2, 0, 0], cargo)
        };
        assert_eq!(lacks(&none, &none, &none, false), Some(Note::NoUnloadDock), "unload first");
        assert_eq!(lacks(&none, &charge, &none, false), Some(Note::NoUnloadDock));
        assert_eq!(lacks(&unload, &none, &none, false), Some(Note::NoChargeDock));
        assert_eq!(lacks(&unload, &charge, &none, false), None);
        assert_eq!(lacks(&unload, &charge, &none, true), Some(Note::NoLoadDock), "the cargo worker");
        assert_eq!(lacks(&unload, &charge, &load, true), None);
        // A dock beside another stretch of rail is no dock of this one.
        let mut cells = line(5);
        cells.push([10, 0, 0]);
        let net = rails(&cells);
        let far = BTreeSet::from([[10, 0, 1]]);
        assert_eq!(Bases::of(&net, &far, &far, &none).lacks([2, 0, 0], false), Some(Note::NoUnloadDock));
        assert_eq!(Bases::of(&net, &far, &far, &none).lacks([10, 0, 0], false), None);
        assert_eq!(Bases::of(&net, &far, &far, &none).lacks([50, 0, 0], false), None, "off the rails");
    }

    #[test]
    fn the_bore_bores_away_from_its_docks() {
        let net = rails(&line(6));
        let none = BTreeSet::new();
        // Docks at the west end: it bores past the east end, east.
        let west = BTreeSet::from([[0, 0, 1]]);
        let west_charge = BTreeSet::from([[0, 0, -1]]);
        let b = Bases::of(&net, &west, &west_charge, &none);
        assert_eq!(b.bore_way([3, 0, 0]), Some(BoreWay::End { cell: [5, 0, 0], heading: [1, 0] }));
        // At the east end: west.
        let east = BTreeSet::from([[5, 0, 1]]);
        let b = Bases::of(&net, &east, &east, &none);
        assert_eq!(b.bore_way([0, 0, 0]), Some(BoreWay::End { cell: [0, 0, 0], heading: [-1, 0] }));
        // In the middle, nearer the east end: towards the west end, the farther.
        let mid = BTreeSet::from([[4, 0, 1]]);
        let b = Bases::of(&net, &mid, &mid, &none);
        assert_eq!(b.bore_way([0, 0, 0]), Some(BoreWay::End { cell: [0, 0, 0], heading: [-1, 0] }));
    }

    #[test]
    fn a_single_rail_bores_away_from_the_side_of_its_docks() {
        let net = rails(&[[0, 0, 0]]);
        let none = BTreeSet::new();
        let north = BTreeSet::from([[0, 0, -1]]);
        let b = Bases::of(&net, &north, &north, &none);
        assert_eq!(b.bore_way([0, 0, 0]), Some(BoreWay::End { cell: [0, 0, 0], heading: [0, 1] }));
        let east_and_under = BTreeSet::from([[1, 0, 0], [0, -1, 0]]);
        let b = Bases::of(&net, &east_and_under, &none, &none);
        assert_eq!(b.bore_way([0, 0, 0]), Some(BoreWay::End { cell: [0, 0, 0], heading: [-1, 0] }));
        let under = BTreeSet::from([[0, -1, 0]]);
        assert_eq!(Bases::of(&net, &under, &under, &none).bore_way([0, 0, 0]), Some(BoreWay::NoEnd));
    }

    #[test]
    fn a_loop_has_no_end_to_bore_from() {
        let mut cells = Vec::new();
        for x in 0..4 {
            cells.push([x, 0, 0]);
            cells.push([x, 0, 3]);
        }
        for z in 1..3 {
            cells.push([0, 0, z]);
            cells.push([3, 0, z]);
        }
        let net = rails(&cells);
        let dock = BTreeSet::from([[1, 0, 1]]);
        let b = Bases::of(&net, &dock, &dock, &BTreeSet::new());
        assert_eq!(b.lacks([2, 0, 3], false), None);
        assert_eq!(b.bore_way([2, 0, 3]), Some(BoreWay::NoEnd));
    }
}
