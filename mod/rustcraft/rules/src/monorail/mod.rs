//! The monorail as a graph (spec 4.6). Rails sit on any face of a block (T1) and join their
//! neighbours by face or, on floors and ceilings, by a climb of one block up or down (T2).
//! Stations of one connected network see each other (T6). A trip takes the shortest path (T8),
//! costs energy by length (T8: 1 E per 4 blocks), and a station names the shortfall (T8.a) or
//! where the path breaks or has no room for the cabin (T8.b). The tier of each rail sets the speed
//! on it (T4).
//!
//! Positions are the game's: y is up. A rail's `mount` is the side of its cell the block it hangs
//! on is behind: `YNeg` for a floor rail, `YPos` for a ceiling rail, a side for a wall rail.

pub mod capsule;
pub mod route;
pub mod shape;
pub mod window;
pub mod wire;

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use crate::geom::{Face, Pos, step};

/// A rail piece: its tier and the side of its cell the block it hangs on is behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rail {
    pub tier: u8,
    pub mount: Face,
}

impl Rail {
    /// A floor or ceiling rail: the only ones that climb.
    pub fn climbs(self) -> bool {
        matches!(self.mount, Face::YNeg | Face::YPos)
    }
}

/// A station block: its name and the energy in its buffer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Station {
    pub name: String,
    pub energy: f64,
}

/// The rails and stations of a level.
#[derive(Clone, Debug, Default)]
pub struct Network {
    pub rails: HashMap<Pos, Rail>,
    /// Station blocks by position.
    pub stations: BTreeMap<Pos, Station>,
}

/// The numbers of the monorail.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailSpec {
    /// T8: blocks of path per 1 E of trip cost.
    pub blocks_per_energy: f64,
    /// T4: blocks a second on a rail of each tier (guide `monorail`: 8 / 14 / 22 / 32).
    pub speed: [f64; 4],
}

impl Default for RailSpec {
    fn default() -> Self {
        RailSpec {
            blocks_per_energy: 4.0,
            speed: [8.0, 14.0, 22.0, 32.0],
        }
    }
}

impl RailSpec {
    /// Blocks a second on a rail of `tier` (1–4; others are held to that range).
    pub fn speed_of(&self, tier: u8) -> f64 {
        self.speed[usize::from(tier.clamp(1, 4)) - 1]
    }
}

/// A planned trip.
#[derive(Clone, Debug, PartialEq)]
pub struct Trip {
    /// The rail cells the capsule passes, from the start station's rail to the destination's.
    pub path: Vec<Pos>,
    /// Blocks of path: steps from rail to rail.
    pub length: u32,
    /// E the start station pays.
    pub cost: f64,
}

/// Why a trip cannot start.
#[derive(Clone, Debug, PartialEq)]
pub enum TripError {
    /// No station block at that position.
    UnknownStation(Pos),
    /// The station touches no rail.
    NoRail(Pos),
    /// T8.a: the start station lacks this much energy.
    NotEnoughEnergy { short: f64 },
    /// T8.b: the destination is not on this network; the path ends nearest to it here.
    Broken { ends_at: Pos },
    /// T8.b: the cabin has no room at this rail: the block it would pass through is solid.
    NoRoom { at: Pos },
}

/// Whether `a` and `b`, both rails of `net`, join: by a face, or by a climb (one block across and
/// one up or down) between two floor rails or two ceiling rails.
fn joined(net: &Network, a: Pos, b: Pos) -> bool {
    let (Some(ra), Some(rb)) = (net.rails.get(&a), net.rails.get(&b)) else {
        return false;
    };
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let manhattan = d[0].abs() + d[1].abs() + d[2].abs();
    if manhattan == 1 {
        return true;
    }
    // A climb: one step across, one up or down, the same floor or ceiling mount.
    let across = d[0].abs() + d[2].abs() == 1;
    across && d[1].abs() == 1 && ra.climbs() && ra.mount == rb.mount
}

/// The rail cells joined to `pos`: by a face, or one block up or down with one block across.
pub fn neighbours(net: &Network, pos: Pos) -> Vec<Pos> {
    let mut out = Vec::new();
    for face in Face::ALL {
        let p = step(pos, face);
        if net.rails.contains_key(&p) {
            out.push(p);
        }
    }
    if net.rails.get(&pos).is_some_and(|r| r.climbs()) {
        for face in [Face::XNeg, Face::XPos, Face::ZNeg, Face::ZPos] {
            let across = step(pos, face);
            for dy in [-1, 1] {
                let climb = [across[0], across[1] + dy, across[2]];
                if joined(net, pos, climb) {
                    out.push(climb);
                }
            }
        }
    }
    out
}

/// The rail cells reachable from `start`.
pub fn component(net: &Network, start: Pos) -> BTreeSet<Pos> {
    let mut seen = BTreeSet::from([start]);
    let mut stack = vec![start];
    while let Some(p) = stack.pop() {
        for n in neighbours(net, p) {
            if seen.insert(n) {
                stack.push(n);
            }
        }
    }
    seen
}

/// The rail a station block stands at: the first rail touching it by a face, the sides first
/// (a station stands beside its track), then below and above.
pub fn station_rail(net: &Network, station: Pos) -> Option<Pos> {
    const ORDER: [Face; 6] = [
        Face::XNeg,
        Face::XPos,
        Face::ZNeg,
        Face::ZPos,
        Face::YNeg,
        Face::YPos,
    ];
    ORDER
        .into_iter()
        .map(|f| step(station, f))
        .find(|p| net.rails.contains_key(p))
}

/// T6: the stations on the same network as the station at `station`, itself excluded, by name
/// (then position).
pub fn visible_stations(net: &Network, station: Pos) -> Vec<Pos> {
    let Some(rail) = station_rail(net, station) else {
        return Vec::new();
    };
    let reach = component(net, rail);
    let mut out: Vec<Pos> = net
        .stations
        .keys()
        .copied()
        .filter(|&s| s != station)
        .filter(|&s| station_rail(net, s).is_some_and(|r| reach.contains(&r)))
        .collect();
    out.sort_by(|a, b| {
        net.stations[a]
            .name
            .cmp(&net.stations[b].name)
            .then(a.cmp(b))
    });
    out
}

/// The shortest path of rail cells from `from` to `to`, by face and climb steps; ties go the way
/// the neighbours come first, so the result is stable. Err with the reached cell nearest to `to`
/// when it cannot be reached.
pub fn shortest_path(net: &Network, from: Pos, to: Pos) -> Result<Vec<Pos>, Pos> {
    let mut came_from: HashMap<Pos, Pos> = HashMap::from([(from, from)]);
    let mut queue = VecDeque::from([from]);
    while let Some(p) = queue.pop_front() {
        if p == to {
            let mut path = vec![to];
            let mut at = to;
            while at != from {
                at = came_from[&at];
                path.push(at);
            }
            path.reverse();
            return Ok(path);
        }
        for n in neighbours(net, p) {
            if let std::collections::hash_map::Entry::Vacant(e) = came_from.entry(n) {
                e.insert(p);
                queue.push_back(n);
            }
        }
    }
    let squared = |p: &Pos| (0..3).map(|a| i64::from(p[a] - to[a]).pow(2)).sum::<i64>();
    Err(*came_from
        .keys()
        .min_by_key(|p| (squared(p), **p))
        .expect("the start is always reached"))
}

/// T8: plans a trip from the station at `from` to the station at `to`. `free` says whether a block
/// lets the cabin through (capsule::room_cells names the blocks it needs).
pub fn plan_trip(
    net: &Network,
    spec: &RailSpec,
    from: Pos,
    to: Pos,
    free: impl Fn(Pos) -> bool,
) -> Result<Trip, TripError> {
    let start = net
        .stations
        .get(&from)
        .ok_or(TripError::UnknownStation(from))?;
    if !net.stations.contains_key(&to) {
        return Err(TripError::UnknownStation(to));
    }
    let start_rail = station_rail(net, from).ok_or(TripError::NoRail(from))?;
    let goal = station_rail(net, to).ok_or(TripError::NoRail(to))?;
    let path =
        shortest_path(net, start_rail, goal).map_err(|ends_at| TripError::Broken { ends_at })?;
    for &cell in &path {
        let mount = net.rails[&cell].mount;
        if let Some(at) = capsule::room_cells(cell, mount)
            .into_iter()
            .find(|&p| !net.rails.contains_key(&p) && !free(p))
        {
            return Err(TripError::NoRoom { at });
        }
    }
    let length = (path.len() - 1) as u32;
    let cost = f64::from(length) / spec.blocks_per_energy;
    if start.energy + 1e-9 < cost {
        return Err(TripError::NotEnoughEnergy {
            short: cost - start.energy,
        });
    }
    Ok(Trip { path, length, cost })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn rail(net: &mut Network, pos: Pos, mount: Face) {
        net.rails.insert(pos, Rail { tier: 1, mount });
    }

    pub fn floor_rail(net: &mut Network, pos: Pos) {
        rail(net, pos, Face::YNeg);
    }

    pub fn station(net: &mut Network, name: &str, at: Pos, energy: f64) {
        net.stations.insert(
            at,
            Station {
                name: name.into(),
                energy,
            },
        );
    }

    pub const HOME: Pos = [0, 0, -1];
    pub const MINE: Pos = [100, 0, -1];
    pub const FARM: Pos = [49, 0, 50];

    /// Example of T8: 100 blocks from «Дом» to «Шахта», at the 50th a branch of 50 to «Ферма».
    /// Each station stands beside the end of its track.
    pub fn fork() -> Network {
        let mut net = Network::default();
        for x in 0..=100 {
            floor_rail(&mut net, [x, 0, 0]);
        }
        for z in 1..=50 {
            floor_rail(&mut net, [50, 0, z]);
        }
        station(&mut net, "Дом", HOME, 1000.0);
        station(&mut net, "Шахта", MINE, 1000.0);
        station(&mut net, "Ферма", FARM, 1000.0);
        net
    }

    fn open(_: Pos) -> bool {
        true
    }

    #[test]
    fn the_capsule_takes_the_branch_to_its_station_and_pays_by_length() {
        // Rule T8, example «развилка».
        let trip = plan_trip(&fork(), &RailSpec::default(), HOME, FARM, open).unwrap();
        assert_eq!(trip.length, 100);
        assert_eq!(trip.cost, 25.0);
        assert_eq!(trip.path.first(), Some(&[0, 0, 0]));
        assert_eq!(trip.path.last(), Some(&[50, 0, 50]));
        // Straight along x to the fork, then along the branch.
        assert!(trip.path.contains(&[50, 0, 0]));
        assert!(!trip.path.contains(&[51, 0, 0]));
    }

    #[test]
    fn stations_of_one_network_see_each_other_and_no_others() {
        // Rule T6.
        let mut net = fork();
        floor_rail(&mut net, [0, 0, 200]);
        station(&mut net, "Остров", [0, 0, 199], 0.0);
        assert_eq!(visible_stations(&net, HOME), vec![FARM, MINE]);
        assert!(visible_stations(&net, [0, 0, 199]).is_empty());
    }

    #[test]
    fn a_station_without_enough_energy_names_the_shortfall() {
        // Rule T8.a.
        let mut net = fork();
        net.stations.get_mut(&HOME).unwrap().energy = 10.0;
        assert_eq!(
            plan_trip(&net, &RailSpec::default(), HOME, FARM, open),
            Err(TripError::NotEnoughEnergy { short: 15.0 })
        );
    }

    #[test]
    fn a_broken_path_names_where_it_ends() {
        // Rule T8.b: a rail missing at x=70 cuts «Шахта» off; the path ends at x=69.
        let mut net = fork();
        net.rails.remove(&[70, 0, 0]);
        assert_eq!(
            plan_trip(&net, &RailSpec::default(), HOME, MINE, open),
            Err(TripError::Broken {
                ends_at: [69, 0, 0]
            })
        );
    }

    #[test]
    fn a_solid_block_over_the_track_leaves_no_room_for_the_cabin() {
        // Rule T8.b: a floor rail needs the block above it free.
        let net = fork();
        let blocked = |p: Pos| p != [20, 1, 0];
        assert_eq!(
            plan_trip(&net, &RailSpec::default(), HOME, MINE, blocked),
            Err(TripError::NoRoom { at: [20, 1, 0] })
        );
    }

    #[test]
    fn a_station_away_from_any_rail_has_no_trip() {
        let mut net = fork();
        station(&mut net, "Пустырь", [0, 5, 5], 100.0);
        assert_eq!(
            plan_trip(&net, &RailSpec::default(), [0, 5, 5], HOME, open),
            Err(TripError::NoRail([0, 5, 5]))
        );
        assert!(visible_stations(&net, [0, 5, 5]).is_empty());
    }

    #[test]
    fn rails_join_up_a_climb_and_up_a_wall() {
        // Rule T2: a climb (one up, one across) and a vertical run up a wall join the network.
        let mut net = Network::default();
        floor_rail(&mut net, [0, 0, 0]);
        floor_rail(&mut net, [1, 1, 0]); // climb
        floor_rail(&mut net, [2, 1, 0]);
        for y in 2..6 {
            rail(&mut net, [2, y, 0], Face::XPos); // up a wall on the east
        }
        station(&mut net, "Низ", [-1, 0, 0], 100.0);
        station(&mut net, "Верх", [2, 5, 1], 100.0);
        let trip = plan_trip(&net, &RailSpec::default(), [-1, 0, 0], [2, 5, 1], open).unwrap();
        assert_eq!(trip.length, 6);
        assert_eq!(trip.path[1], [1, 1, 0]);
    }

    #[test]
    fn a_climb_joins_floors_to_floors_and_not_a_floor_to_a_wall() {
        let mut net = Network::default();
        floor_rail(&mut net, [0, 0, 0]);
        rail(&mut net, [1, 1, 0], Face::ZNeg);
        assert!(neighbours(&net, [0, 0, 0]).is_empty());
        rail(&mut net, [1, 1, 0], Face::YPos);
        assert!(neighbours(&net, [0, 0, 0]).is_empty());
        rail(&mut net, [0, 0, 0], Face::YPos);
        assert_eq!(neighbours(&net, [0, 0, 0]), vec![[1, 1, 0]]);
    }

    #[test]
    fn the_guide_gives_each_tier_its_speed() {
        // Guide `monorail`: 8 / 14 / 22 / 32 blocks a second (T4).
        let spec = RailSpec::default();
        assert_eq!(
            [1, 2, 3, 4].map(|t| spec.speed_of(t)),
            [8.0, 14.0, 22.0, 32.0]
        );
        assert_eq!(spec.speed_of(9), 32.0);
    }
}
