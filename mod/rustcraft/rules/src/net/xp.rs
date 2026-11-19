//! 0.21 (spec 44, XC1–XC10; owner 2026-11-19: «можно четвертым каналом просто в модельку корпуса
//! добавить зеленый канал. Ну, типа опыт»): the experience channel, the fourth built-in channel of
//! every housing (`topology::XP_SLOT`).
//!
//! It joins three kinds of block (`XpPort`): a block that makes experience (the butcher's kills), a
//! condenser that keeps it, and the enchanting machine that spends it. A block is on an experience
//! net through a joint, a face of one of the net's housings that touches it; the joint's direction
//! (kept per face and slot as every channel's, M7; the default «по умолчанию» both ways) says
//! whether the block may give into the net and take from it.
//!
//! Experience made goes first into a condenser touching the block that made it (K6), then over
//! the net into the condensers that take, the nearest by pipe first, each as far as its room. The
//! enchanting machine spends from a condenser touching it first, then from the condensers on its
//! net that give, the nearest first. What rides a net in a second is limited by its weakest
//! housing (`Tuning::xp_of`, M10); a condenser touching the block is no ride.
//!
//! These functions only pair the net with the blocks round it and plan the moves; the bundle
//! reads and writes the condensers' points.

use std::collections::{BTreeMap, BTreeSet};

use crate::geom::{Face, Pos, step};
use crate::net::items::Mode;
use crate::net::settings::HousingJoints;
use crate::net::topology::{ChannelKind, ChannelNet, Layout, Topology, pipe_length};
use crate::tuning::Tuning;

/// The direction of an experience joint the player has not set: both ways, so a condenser on the
/// line both keeps and lends and a butcher's kills ride without a click (XC3).
pub const XP_DEFAULT: Mode = Mode::Both;

/// The blocks the experience channel joins.
pub const CONDENSER: &str = "rustcraft:xp_tank";
pub const ENCHANTER: &str = "rustcraft:enchanter";

/// What a block does on an experience net (XC2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum XpPort {
    /// It makes experience and gives it (the butcher).
    Source,
    /// It keeps experience: it takes and gives (the condenser).
    Store,
    /// It spends experience: it takes (the enchanting machine).
    Consumer,
}

impl XpPort {
    /// The port of a block, by its id; none for a block the channel does not join.
    pub fn of(block: &str) -> Option<XpPort> {
        match block {
            crate::butcher::BLOCK => Some(XpPort::Source),
            CONDENSER => Some(XpPort::Store),
            ENCHANTER => Some(XpPort::Consumer),
            _ => None,
        }
    }

    fn may_give(self) -> bool {
        matches!(self, XpPort::Source | XpPort::Store)
    }

    fn may_take(self) -> bool {
        matches!(self, XpPort::Store | XpPort::Consumer)
    }
}

/// What a block may do on one experience net through its joints there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct End {
    /// It may give into the net: a joint of the net on it gives (from the block, or both ways).
    pub gives: bool,
    /// It may take from the net: a joint of the net on it takes (into the block, or both ways).
    pub takes: bool,
}

/// The direction of the experience joint of `housing` on `face`: its setting or XP_DEFAULT.
fn mode_of(settings: &BTreeMap<Pos, HousingJoints>, housing: Pos, face: Face, slot: usize) -> Mode {
    settings
        .get(&housing)
        .and_then(|h| h.get(face, slot))
        .map_or(XP_DEFAULT, |s| s.mode)
}

/// The blocks of `ports` on experience net `net` and what each may do there: a block with several
/// joints on the net may give if one of them gives, and take if one takes. A block whose joints
/// are all off, or that its port lets do nothing through them, is left out.
pub fn ends(
    net: &ChannelNet,
    layout: &Layout,
    ports: &BTreeMap<Pos, XpPort>,
    settings: &BTreeMap<Pos, HousingJoints>,
) -> BTreeMap<Pos, End> {
    let mut out: BTreeMap<Pos, End> = BTreeMap::new();
    for &housing in &net.housings {
        for face in Face::ALL {
            let block = step(housing, face);
            if layout.housings.contains_key(&block) || layout.nodes.contains_key(&block) {
                continue;
            }
            let Some(port) = ports.get(&block) else {
                continue;
            };
            let mode = mode_of(settings, housing, face, net.slot);
            let end = End {
                gives: port.may_give() && mode.gives(),
                takes: port.may_take() && mode.takes(),
            };
            if end.gives || end.takes {
                let e = out.entry(block).or_default();
                e.gives |= end.gives;
                e.takes |= end.takes;
            }
        }
    }
    out
}

/// The housings of `net` that touch `block`.
fn touching(net: &ChannelNet, block: Pos) -> Vec<Pos> {
    Face::ALL
        .map(|f| step(block, f))
        .into_iter()
        .filter(|h| net.housings.contains(h))
        .collect()
}

/// A condenser a transfer may use: where it is, how many points it has room for (filling) or holds
/// (drawing), and the net it rides (none for one touching the block: no ride, XC5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Store {
    pub pos: Pos,
    pub points: u32,
    pub net: Option<NetKey>,
}

/// An experience net by its first housing, which names it while the layout stands.
pub type NetKey = Pos;

/// The key of `net`.
pub fn key_of(net: &ChannelNet) -> NetKey {
    net.housings.first().copied().unwrap_or_default()
}

/// The points each net may still carry this second.
pub type Budget = BTreeMap<NetKey, u32>;

/// The condensers a block at `at` reaches: the ones touching it first (K6), then the ones on the
/// experience nets it is on, nearest by pipe first (then by place), each net's in turn. `wanted`
/// picks what the block and the condenser must be able to do on the net (`Fill`: the block gives
/// and the condenser takes; `Draw`: the condenser gives and the block takes). `points` says how many
/// points a condenser has room for or holds; one with none is left out.
pub fn reach(
    world: &World<'_>,
    at: Pos,
    wanted: Way,
    points: &dyn Fn(Pos) -> u32,
) -> Vec<Store> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for face in Face::ALL {
        let p = step(at, face);
        if world.ports.get(&p) == Some(&XpPort::Store) && seen.insert(p) {
            out.push(Store { pos: p, points: points(p), net: None });
        }
    }
    for net in world
        .topology
        .nets
        .iter()
        .filter(|n| n.kind == ChannelKind::Xp && !touching(n, at).is_empty())
    {
        let ends = ends(net, world.layout, world.ports, world.settings);
        let me = ends.get(&at).copied().unwrap_or_default();
        let (mine, theirs): (bool, fn(&End) -> bool) = match wanted {
            Way::Fill => (me.gives, |e| e.takes),
            Way::Draw => (me.takes, |e| e.gives),
        };
        if !mine {
            continue;
        }
        let from = touching(net, at);
        let mut found: Vec<(u32, Pos)> = ends
            .iter()
            .filter(|(p, e)| **p != at && world.ports.get(*p) == Some(&XpPort::Store) && theirs(e))
            .filter(|(p, _)| !seen.contains(*p))
            .filter_map(|(&p, _)| {
                let to = touching(net, p);
                pipe_length(world.layout, world.tuning, &net.housings, &from, &to).map(|d| (d, p))
            })
            .collect();
        found.sort_unstable();
        for (_, p) in found {
            seen.insert(p);
            out.push(Store { pos: p, points: points(p), net: Some(key_of(net)) });
        }
    }
    out.retain(|s| s.points > 0);
    out
}

/// XC10: how many blocks of each port share an experience net with the block at `at`, through
/// joints that work for both; for the condenser's window. A block on several of its nets counts
/// once.
pub fn partners(world: &World<'_>, at: Pos) -> BTreeMap<XpPort, u32> {
    let mut found: BTreeMap<Pos, XpPort> = BTreeMap::new();
    for net in world
        .topology
        .nets
        .iter()
        .filter(|n| n.kind == ChannelKind::Xp && !touching(n, at).is_empty())
    {
        let ends = ends(net, world.layout, world.ports, world.settings);
        let Some(me) = ends.get(&at).copied() else {
            continue;
        };
        for (p, e) in &ends {
            let joins = (me.gives && e.takes) || (me.takes && e.gives);
            if *p != at && joins {
                if let Some(port) = world.ports.get(p) {
                    found.insert(*p, *port);
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    for port in found.into_values() {
        *out.entry(port).or_default() += 1;
    }
    out
}

/// Which way a transfer goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// From the block into condensers (a kill's experience).
    Fill,
    /// From condensers into the block (an enchantment).
    Draw,
}

/// The level round a block: its magistral, the joints' settings and the experience ports.
pub struct World<'a> {
    pub topology: &'a Topology,
    pub layout: &'a Layout,
    pub tuning: &'a Tuning,
    pub settings: &'a BTreeMap<Pos, HousingJoints>,
    pub ports: &'a BTreeMap<Pos, XpPort>,
}

/// The points each net may carry in a second: its weakest housing's (XC6).
pub fn budget(topology: &Topology, tuning: &Tuning) -> Budget {
    topology
        .nets
        .iter()
        .filter(|n| n.kind == ChannelKind::Xp)
        .map(|n| (key_of(n), tuning.xp_of(n.min_tier).max(0.0) as u32))
        .collect()
}

/// A planned transfer: points into (filling) or out of (drawing) each condenser.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Moves {
    pub moves: Vec<(Pos, u32)>,
    /// What rode each net, to take off its budget.
    pub carried: BTreeMap<NetKey, u32>,
}

impl Moves {
    pub fn total(&self) -> u32 {
        self.moves.iter().map(|m| m.1).sum()
    }
}

/// XC5: `points` from a block into `stores` in their order, each as far as its room, a net's as
/// far as its budget left; what finds no room is lost.
pub fn fill(points: u32, stores: &[Store], budget: &Budget) -> Moves {
    let mut left = points;
    let mut out = Moves::default();
    for s in stores {
        if left == 0 {
            break;
        }
        let cap = match s.net {
            None => u32::MAX,
            Some(k) => budget.get(&k).copied().unwrap_or(0).saturating_sub(out.carried.get(&k).copied().unwrap_or(0)),
        };
        let n = left.min(s.points).min(cap);
        if n == 0 {
            continue;
        }
        left -= n;
        out.moves.push((s.pos, n));
        if let Some(k) = s.net {
            *out.carried.entry(k).or_default() += n;
        }
    }
    out
}

/// Why a draw could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Short {
    /// No condenser touches the block or gives on its net.
    NoStore,
    /// The condensers it reaches hold less than `need`.
    NotEnough,
    /// They hold enough, but the nets cannot carry it this second.
    Slow,
}

/// XC7: `points` for a block out of `stores` in their order, each as far as it holds; all or
/// none. The enchanting machine also needs the condensers to hold `need` in all (the condenser
/// must hold as many levels as the enchantment's, spec 22).
pub fn draw(points: u32, need: u32, stores: &[Store], budget: &Budget) -> Result<Moves, Short> {
    if stores.is_empty() {
        return Err(Short::NoStore);
    }
    let held: u64 = stores.iter().map(|s| u64::from(s.points)).sum();
    if held < u64::from(need.max(points)) {
        return Err(Short::NotEnough);
    }
    let moves = fill(points, stores, budget);
    if moves.total() < points {
        return Err(Short::Slow);
    }
    Ok(moves)
}

/// Takes what `moves` carried off `budget`.
pub fn spend(budget: &mut Budget, moves: &Moves) {
    for (k, n) in &moves.carried {
        if let Some(b) = budget.get_mut(k) {
            *b = b.saturating_sub(*n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::settings::JointSetting;
    use crate::net::topology::{Housing, Node, XP_SLOT, analyse};

    const BUTCHER: Pos = [1, 1, 0];

    /// A tier-1 node at the origin and housings along x from 1 to `len`, of `tier`.
    fn line(len: i32, tier: u8) -> Layout {
        let mut l = Layout::default();
        l.nodes.insert([0, 0, 0], Node { tier: 1 });
        for x in 1..=len {
            l.housings.insert([x, 0, 0], Housing::new(tier));
        }
        l
    }

    fn ports(list: &[(Pos, XpPort)]) -> BTreeMap<Pos, XpPort> {
        list.iter().copied().collect()
    }

    #[test]
    fn the_ports_are_the_butcher_the_condenser_and_the_enchanter() {
        assert_eq!(XpPort::of("rustcraft:butcher"), Some(XpPort::Source));
        assert_eq!(XpPort::of(crate::utility::Util::XpTank.spec().block), Some(XpPort::Store));
        assert_eq!(XpPort::of(crate::utility::Util::Enchanter.spec().block), Some(XpPort::Consumer));
        assert_eq!(XpPort::of("rustcraft:furnace"), None);
    }

    #[test]
    fn a_kills_experience_rides_the_line_to_the_nearest_condenser_with_room() {
        // XC5: a butcher by housing 1, condensers by housings 4 and 6, another on a second line.
        let mut layout = line(6, 1);
        layout.nodes.insert([0, 0, 5], Node { tier: 1 });
        layout.housings.insert([1, 0, 5], Housing::new(1));
        let topology = analyse(&layout, &Tuning::default());
        let (near, far, other) = ([4, 1, 0], [6, 0, 1], [1, 1, 5]);
        let ports = ports(&[
            (BUTCHER, XpPort::Source),
            (near, XpPort::Store),
            (far, XpPort::Store),
            (other, XpPort::Store),
        ]);
        let settings = BTreeMap::new();
        let tuning = Tuning::default();
        let world = World { topology: &topology, layout: &layout, tuning: &tuning, settings: &settings, ports: &ports };
        let room = |_: Pos| 100;
        let stores = reach(&world, BUTCHER, Way::Fill, &room);
        assert_eq!(stores.iter().map(|s| s.pos).collect::<Vec<_>>(), vec![near, far]);
        assert!(stores.iter().all(|s| s.net.is_some()));
        let budget = budget(&topology, &tuning);
        let moves = fill(2, &stores, &budget);
        assert_eq!(moves.moves, vec![(near, 2)]);
        // The near one full: the far one takes it.
        let full = |p: Pos| if p == near { 0 } else { 100 };
        let stores = reach(&world, BUTCHER, Way::Fill, &full);
        assert_eq!(fill(2, &stores, &budget).moves, vec![(far, 2)]);
        // No room anywhere: the experience is lost.
        assert!(reach(&world, BUTCHER, Way::Fill, &|_| 0).is_empty());
    }

    #[test]
    fn a_condenser_touching_the_butcher_comes_first_and_rides_nothing() {
        // K6 stays: the condenser beside the block, even off the magistral.
        let layout = line(3, 1);
        let topology = analyse(&layout, &Tuning::default());
        let beside = [1, 2, 0];
        let on_line = [3, 1, 0];
        let ports = ports(&[(BUTCHER, XpPort::Source), (beside, XpPort::Store), (on_line, XpPort::Store)]);
        let settings = BTreeMap::new();
        let tuning = Tuning::default();
        let world = World { topology: &topology, layout: &layout, tuning: &tuning, settings: &settings, ports: &ports };
        let stores = reach(&world, BUTCHER, Way::Fill, &|_| 1);
        assert_eq!(stores[0], Store { pos: beside, points: 1, net: None });
        assert_eq!(stores[1].pos, on_line);
        let moves = fill(2, &stores, &BTreeMap::new());
        assert_eq!(moves.moves, vec![(beside, 1)], "the net has no budget: only the touching one");
        assert!(moves.carried.is_empty());
        // Without a magistral at all, the touching condenser still takes it.
        let empty = Topology::default();
        let none = Layout::default();
        let alone = World { topology: &empty, layout: &none, tuning: &tuning, settings: &settings, ports: &ports };
        assert_eq!(reach(&alone, BUTCHER, Way::Fill, &|_| 5).len(), 1);
    }

    #[test]
    fn a_joint_set_off_or_one_way_keeps_a_block_out() {
        // XC3: the butcher's joint «в блок» gives nothing; the condenser's «из блока» takes nothing.
        let layout = line(4, 1);
        let topology = analyse(&layout, &Tuning::default());
        let tank = [4, 1, 0];
        let ports = ports(&[(BUTCHER, XpPort::Source), (tank, XpPort::Store)]);
        let tuning = Tuning::default();
        let set = |pos: Pos, face: Face, mode: Mode| {
            BTreeMap::from([(pos, HousingJoints::default().with(face, XP_SLOT, JointSetting::of_mode(mode)))])
        };
        let reached = |settings: &BTreeMap<Pos, HousingJoints>| {
            let world = World { topology: &topology, layout: &layout, tuning: &tuning, settings, ports: &ports };
            reach(&world, BUTCHER, Way::Fill, &|_| 10).len()
        };
        assert_eq!(reached(&BTreeMap::new()), 1);
        assert_eq!(reached(&set([1, 0, 0], Face::YPos, Mode::Into)), 0);
        assert_eq!(reached(&set([1, 0, 0], Face::YPos, Mode::From)), 1);
        assert_eq!(reached(&set([4, 0, 0], Face::YPos, Mode::From)), 0);
        assert_eq!(reached(&set([4, 0, 0], Face::YPos, Mode::Off)), 0);
        // The item slot's setting is not the experience one.
        let items = BTreeMap::from([(
            [4, 0, 0],
            HousingJoints::default().with(Face::YPos, 0, JointSetting::of_mode(Mode::Off)),
        )]);
        assert_eq!(reached(&items), 1);
    }

    #[test]
    fn the_enchanter_draws_from_the_condensers_on_its_net() {
        // XC7: the enchanting machine by housing 1, condensers of 200 and 1500 points down the line.
        let layout = line(6, 1);
        let topology = analyse(&layout, &Tuning::default());
        let machine = [1, 1, 0];
        let (a, b) = ([3, 1, 0], [6, 1, 0]);
        let ports = ports(&[(machine, XpPort::Consumer), (a, XpPort::Store), (b, XpPort::Store)]);
        let settings = BTreeMap::new();
        let tuning = Tuning::default();
        let world = World { topology: &topology, layout: &layout, tuning: &tuning, settings: &settings, ports: &ports };
        let held = |p: Pos| if p == a { 200 } else { 1500 };
        let stores = reach(&world, machine, Way::Draw, &held);
        assert_eq!(stores.iter().map(|s| s.pos).collect::<Vec<_>>(), vec![a, b]);
        let budget = budget(&topology, &tuning);
        // A level-30 enchantment: 306 points, with 1395 held in all.
        let moves = draw(306, 1395, &stores, &budget).unwrap();
        assert_eq!(moves.moves, vec![(a, 200), (b, 106)]);
        assert_eq!(moves.carried.values().sum::<u32>(), 306);
        assert_eq!(draw(306, 1800, &stores, &budget), Err(Short::NotEnough));
        assert_eq!(draw(306, 1395, &[], &budget), Err(Short::NoStore));
        // The net carried 306 of its 400 this second: the next one waits.
        let mut left = budget.clone();
        spend(&mut left, &moves);
        assert_eq!(left.values().copied().collect::<Vec<_>>(), vec![94]);
        assert_eq!(draw(306, 1395, &stores, &left), Err(Short::Slow));
        // A source on the line gives nothing to the enchanter: it is no condenser.
        let with_butcher = ports_with(&ports, [5, 1, 0], XpPort::Source);
        let world = World { ports: &with_butcher, ..world };
        assert_eq!(reach(&world, machine, Way::Draw, &held).len(), 2);
    }

    #[test]
    fn a_condenser_knows_who_shares_its_channel() {
        let layout = line(6, 1);
        let topology = analyse(&layout, &Tuning::default());
        let tank = [3, 1, 0];
        let ports = ports(&[
            (BUTCHER, XpPort::Source),
            (tank, XpPort::Store),
            ([5, 1, 0], XpPort::Consumer),
            ([6, 1, 0], XpPort::Consumer),
            ([4, 1, 0], XpPort::Store),
        ]);
        let settings = BTreeMap::new();
        let tuning = Tuning::default();
        let world = World { topology: &topology, layout: &layout, tuning: &tuning, settings: &settings, ports: &ports };
        let p = partners(&world, tank);
        assert_eq!(p.get(&XpPort::Source), Some(&1));
        assert_eq!(p.get(&XpPort::Consumer), Some(&2));
        assert_eq!(p.get(&XpPort::Store), Some(&1));
        assert!(partners(&world, [9, 9, 9]).is_empty());
    }

    fn ports_with(p: &BTreeMap<Pos, XpPort>, at: Pos, port: XpPort) -> BTreeMap<Pos, XpPort> {
        let mut out = p.clone();
        out.insert(at, port);
        out
    }

    #[test]
    fn a_stronger_line_carries_more_a_second() {
        // XC6: the weakest housing sets the net's points a second.
        let tuning = Tuning::default();
        for tier in 1..=4u8 {
            let layout = line(3, tier);
            let topology = analyse(&layout, &tuning);
            let b = budget(&topology, &tuning);
            assert_eq!(b.len(), 1, "one experience net");
            assert_eq!(b.values().next().copied(), Some(tuning.xp_of(tier) as u32));
        }
        assert!(tuning.xp_of(1) >= 306.0, "a level-30 enchantment a second on tier 1");
        let stores = [Store { pos: [9, 9, 9], points: 1000, net: Some([1, 0, 0]) }];
        let budget = BTreeMap::from([([1, 0, 0], 50)]);
        assert_eq!(fill(80, &stores, &budget).total(), 50);
    }
}
