//! Energy: one unit (E1), no losses. A circuit is an energy channel net together with what its
//! joints touch, or a source touching machines directly (E10). Every tick a circuit shares what it
//! produces: when production covers demand, every consumer gets all it asks and the surplus
//! charges storage; when it does not, storage covers the gap, and if that is not enough every
//! consumer gets the same share of what it asked and works that much slower (E3). The channel's
//! tier caps how much flows through a net (E2).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::geom::{Face, Pos, step};

/// A source this tick: how many E a second it can give.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Producer {
    pub block: Pos,
    pub output: f64,
}

/// A consumer this tick: how many E a second it asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Consumer {
    pub block: Pos,
    pub request: f64,
}

/// A battery or a battery structure (E11): its charge and how fast it takes and gives, a second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Storage {
    pub block: Pos,
    pub stored: f64,
    pub capacity: f64,
    pub max_in: f64,
    pub max_out: f64,
}

/// What a circuit did in one tick, for the windows (E4, E17) and the machines.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Balance {
    /// E a second the sources could give.
    pub produced: f64,
    /// E a second the consumers asked for.
    pub requested: f64,
    /// E a second each consumer got.
    pub supplied: BTreeMap<Pos, f64>,
    /// The share of their request every consumer got: 1 when there is enough (E3).
    pub share: f64,
}

impl Balance {
    pub fn enough(&self) -> bool {
        self.share >= 1.0 - 1e-9
    }
}

/// One tick of a circuit lasting `dt` seconds. `cap` is the E a second its channel lets through
/// (E2), `None` for a source touching machines directly. Storage is charged or drained in place.
pub fn balance(
    producers: &[Producer],
    consumers: &[Consumer],
    storages: &mut [Storage],
    cap: Option<f64>,
    dt: f64,
) -> Balance {
    let produced: f64 = producers.iter().map(|p| p.output.max(0.0)).sum();
    let requested: f64 = consumers.iter().map(|c| c.request.max(0.0)).sum();
    let cap = cap.unwrap_or(f64::INFINITY);
    // What can reach the consumers at all, through the channel.
    let deliverable = requested.min(cap);
    let from_production = produced.min(deliverable);
    let from_storage = drain(storages, deliverable - from_production, dt);
    let delivered = from_production + from_storage;
    let share = if requested > 0.0 {
        (delivered / requested).min(1.0)
    } else {
        1.0
    };
    // The surplus charges storage, through the channel too.
    let surplus = (produced - from_production).min((cap - delivered).max(0.0));
    charge(storages, surplus, dt);
    Balance {
        produced,
        requested,
        supplied: consumers
            .iter()
            .map(|c| (c.block, c.request.max(0.0) * share))
            .collect(),
        share,
    }
}

/// Takes up to `wanted` E a second out of storage for `dt` seconds; returns the E a second given.
fn drain(storages: &mut [Storage], wanted: f64, dt: f64) -> f64 {
    let mut left = wanted.max(0.0) * dt;
    for s in storages.iter_mut() {
        let e = left.min(s.max_out * dt).min(s.stored);
        s.stored -= e;
        left -= e;
    }
    wanted.max(0.0) - left / dt
}

/// Puts up to `offered` E a second into storage for `dt` seconds.
fn charge(storages: &mut [Storage], offered: f64, dt: f64) {
    let mut left = offered.max(0.0) * dt;
    for s in storages.iter_mut() {
        let e = left.min(s.max_in * dt).min(s.capacity - s.stored).max(0.0);
        s.stored += e;
        left -= e;
    }
}

/// What a source touching machines gives each of them (E10), and what it has left for its net.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirectFeed {
    pub supplied: BTreeMap<Pos, f64>,
    pub source_left: BTreeMap<Pos, f64>,
}

/// E10: sources power the machines that touch them, without a housing. A source shares its output
/// among the touching machines by what they ask; a machine never passes energy on, so a machine
/// that touches only another machine gets nothing.
pub fn direct_feed(producers: &[Producer], consumers: &[Consumer]) -> DirectFeed {
    let mut wanting: HashMap<Pos, f64> = consumers
        .iter()
        .map(|c| (c.block, c.request.max(0.0)))
        .collect();
    let mut feed = DirectFeed::default();
    for p in producers {
        let touching: Vec<Pos> = Face::ALL
            .into_iter()
            .map(|f| step(p.block, f))
            .filter(|b| wanting.get(b).is_some_and(|w| *w > 0.0))
            .collect();
        let asked: f64 = touching.iter().map(|b| wanting[b]).sum();
        let output = p.output.max(0.0);
        let given_total = asked.min(output);
        for b in touching {
            let give = if asked > 0.0 {
                wanting[&b] / asked * given_total
            } else {
                0.0
            };
            *feed.supplied.entry(b).or_insert(0.0) += give;
            *wanting.get_mut(&b).expect("touching consumers are wanting") -= give;
        }
        feed.source_left.insert(p.block, output - given_total);
    }
    feed
}

/// An energy channel net as one circuit: the sources and machines touching its housings, and the
/// E a second its weakest channel lets through (E2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Circuit {
    pub members: std::collections::BTreeSet<Pos>,
    pub cap: f64,
}

/// The E a second each consumer gets this tick: first from sources touching it (E10), then from
/// each energy net it is on, sharing what the net's sources have left (E3). A source or a
/// machine on two nets is asked by each in turn, so nothing is counted twice. Consumers that get
/// nothing are left out.
pub fn supply(
    producers: &[Producer],
    consumers: &[Consumer],
    circuits: &[Circuit],
    dt: f64,
) -> BTreeMap<Pos, f64> {
    supply_report(producers, consumers, circuits, dt).got
}

/// One tick's sharing: what each consumer got and what each source gave, E a second.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Supply {
    /// Consumers that got something.
    pub got: BTreeMap<Pos, f64>,
    /// Every source, with what it gave (0 when nobody took from it).
    pub used: BTreeMap<Pos, f64>,
}

/// `supply`, with what each source gave: a fuel generator burns only for what it gave.
pub fn supply_report(
    producers: &[Producer],
    consumers: &[Consumer],
    circuits: &[Circuit],
    dt: f64,
) -> Supply {
    let (got, left) = share_out(producers, consumers, circuits, dt);
    let used = producers
        .iter()
        .map(|p| {
            let rest = left.get(&p.block).copied().unwrap_or(0.0);
            (p.block, (p.output.max(0.0) - rest).max(0.0))
        })
        .collect();
    Supply { got, used }
}

/// The blocks whose energy a source's window sums up (E17): the source, the consumers touching
/// it and every member of the circuits it is on.
pub fn group_of(source: Pos, consumers: &[Consumer], circuits: &[Circuit]) -> BTreeSet<Pos> {
    let mut members: BTreeSet<Pos> = BTreeSet::from([source]);
    members.extend(
        consumers
            .iter()
            .map(|c| c.block)
            .filter(|b| Face::ALL.into_iter().any(|f| step(source, f) == *b)),
    );
    for c in circuits.iter().filter(|c| c.members.contains(&source)) {
        members.extend(c.members.iter().copied());
    }
    members
}

/// E17: what a group of blocks makes, asks for and gets, E a second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GroupBalance {
    /// What its sources could give.
    pub produced: f64,
    /// What its consumers ask for.
    pub requested: f64,
    /// What its consumers got.
    pub supplied: f64,
}

impl GroupBalance {
    pub fn of(
        members: &BTreeSet<Pos>,
        producers: &[Producer],
        consumers: &[Consumer],
        got: &BTreeMap<Pos, f64>,
    ) -> GroupBalance {
        let mine = |b: &Pos| members.contains(b);
        GroupBalance {
            produced: producers
                .iter()
                .filter(|p| mine(&p.block))
                .map(|p| p.output.max(0.0))
                .sum(),
            requested: consumers
                .iter()
                .filter(|c| mine(&c.block))
                .map(|c| c.request.max(0.0))
                .sum(),
            supplied: got.iter().filter(|(b, _)| mine(b)).map(|(_, e)| e).sum(),
        }
    }

    /// Whether every consumer gets all it asks for (E3).
    pub fn enough(&self) -> bool {
        self.supplied >= self.requested - 1e-6
    }

    /// How much slower the machines work, in percent: 0 when there is enough.
    pub fn slowdown_percent(&self) -> u32 {
        if self.enough() || self.requested <= 0.0 {
            0
        } else {
            (((1.0 - self.supplied / self.requested) * 100.0).round() as u32).clamp(1, 100)
        }
    }
}

/// A battery block this tick (E11): what it may give and what it may take, E a second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Store {
    pub block: Pos,
    pub give: f64,
    pub take: f64,
}

/// One tick's sharing with batteries: what each consumer got, what each source gave (to machines
/// and to batteries), and what each battery block gave and took, E a second.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shared {
    pub got: BTreeMap<Pos, f64>,
    pub used: BTreeMap<Pos, f64>,
    pub given: BTreeMap<Pos, f64>,
    pub taken: BTreeMap<Pos, f64>,
}

/// `supply_report` with batteries (E11, «накопитель»): the sources feed the machines first (E10,
/// E3); the batteries then cover what the machines still lack, by contact and through the nets
/// they are on; what the sources have left charges the batteries. A battery never charges
/// another one.
pub fn supply_with_storage(
    producers: &[Producer],
    consumers: &[Consumer],
    stores: &[Store],
    circuits: &[Circuit],
    dt: f64,
) -> Shared {
    let first = supply_report(producers, consumers, circuits, dt);
    let lacking: Vec<Consumer> = consumers
        .iter()
        .map(|c| Consumer {
            block: c.block,
            request: c.request.max(0.0) - first.got.get(&c.block).copied().unwrap_or(0.0),
        })
        .filter(|c| c.request > 1e-9)
        .collect();
    let givers: Vec<Producer> = stores
        .iter()
        .filter(|s| s.give > 0.0)
        .map(|s| Producer {
            block: s.block,
            output: s.give,
        })
        .collect();
    let second = supply_report(&givers, &lacking, circuits, dt);
    let left: Vec<Producer> = producers
        .iter()
        .map(|p| Producer {
            block: p.block,
            output: p.output.max(0.0) - first.used.get(&p.block).copied().unwrap_or(0.0),
        })
        .filter(|p| p.output > 1e-9)
        .collect();
    let takers: Vec<Consumer> = stores
        .iter()
        .filter(|s| s.take > 0.0)
        .map(|s| Consumer {
            block: s.block,
            request: s.take,
        })
        .collect();
    let third = supply_report(&left, &takers, circuits, dt);
    let mut got = first.got;
    for (b, e) in second.got {
        *got.entry(b).or_insert(0.0) += e;
    }
    let mut used = first.used;
    for (b, e) in third.used {
        *used.entry(b).or_insert(0.0) += e;
    }
    Shared {
        got,
        used,
        given: second.used.into_iter().filter(|(_, e)| *e > 0.0).collect(),
        taken: third.got,
    }
}

/// E18, E20, E11, E7: a structure joins a net through any of its blocks, so a net that reaches
/// one block of a group reaches them all.
pub fn join_groups(circuits: &mut [Circuit], groups: &[BTreeSet<Pos>]) {
    for c in circuits.iter_mut() {
        for g in groups {
            if g.iter().any(|b| c.members.contains(b)) {
                c.members.extend(g.iter().copied());
            }
        }
    }
}

/// What `supply_report` shares out: each consumer's energy and each source's rest.
fn share_out(
    producers: &[Producer],
    consumers: &[Consumer],
    circuits: &[Circuit],
    dt: f64,
) -> (BTreeMap<Pos, f64>, BTreeMap<Pos, f64>) {
    let feed = direct_feed(producers, consumers);
    let mut got = feed.supplied;
    let mut left: BTreeMap<Pos, f64> = producers
        .iter()
        .map(|p| {
            (
                p.block,
                feed.source_left.get(&p.block).copied().unwrap_or(0.0),
            )
        })
        .collect();
    let mut wanting: BTreeMap<Pos, f64> = consumers
        .iter()
        .map(|c| {
            (
                c.block,
                (c.request.max(0.0) - got.get(&c.block).copied().unwrap_or(0.0)).max(0.0),
            )
        })
        .collect();
    for circuit in circuits {
        let ps: Vec<Producer> = left
            .iter()
            .filter(|(b, e)| circuit.members.contains(*b) && **e > 0.0)
            .map(|(&block, &output)| Producer { block, output })
            .collect();
        let cs: Vec<Consumer> = wanting
            .iter()
            .filter(|(b, w)| circuit.members.contains(*b) && **w > 0.0)
            .map(|(&block, &request)| Consumer { block, request })
            .collect();
        if ps.is_empty() || cs.is_empty() {
            continue;
        }
        let b = balance(&ps, &cs, &mut [], Some(circuit.cap), dt);
        let used: f64 = b.supplied.values().sum();
        for (block, e) in b.supplied {
            *got.entry(block).or_insert(0.0) += e;
            if let Some(w) = wanting.get_mut(&block) {
                *w = (*w - e).max(0.0);
            }
        }
        // The sources give what was used in proportion to what each had.
        let fraction_left = if b.produced > 0.0 {
            1.0 - (used / b.produced).min(1.0)
        } else {
            1.0
        };
        for p in &ps {
            left.insert(p.block, p.output * fraction_left);
        }
    }
    got.retain(|_, e| *e > 0.0);
    (got, left)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn producer(x: i32, output: f64) -> Producer {
        Producer {
            block: [x, 0, 0],
            output,
        }
    }

    fn consumer(x: i32, request: f64) -> Consumer {
        Consumer {
            block: [x, 0, 0],
            request,
        }
    }

    fn battery(x: i32, stored: f64) -> Storage {
        Storage {
            block: [x, 0, 0],
            stored,
            capacity: 40_000.0,
            max_in: 32.0,
            max_out: 32.0,
        }
    }

    #[test]
    fn enough_energy_feeds_everyone_and_charges_storage() {
        let mut storages = [battery(9, 100.0)];
        let b = balance(
            &[producer(0, 16.0)],
            &[consumer(1, 8.0)],
            &mut storages,
            None,
            DT,
        );
        assert!(b.enough());
        assert!(close(b.supplied[&[1, 0, 0]], 8.0));
        assert!(close(storages[0].stored, 108.0));
    }

    #[test]
    fn a_shortage_slows_every_consumer_by_the_same_share() {
        // Rule E3: two furnaces asking 8 each from a 4 E/s crystal get a quarter each.
        let b = balance(
            &[producer(0, 4.0)],
            &[consumer(1, 8.0), consumer(2, 8.0)],
            &mut [],
            None,
            DT,
        );
        assert!(!b.enough());
        assert!(close(b.share, 0.25));
        assert!(close(b.supplied[&[1, 0, 0]], 2.0));
        assert!(close(b.supplied[&[2, 0, 0]], 2.0));
    }

    #[test]
    fn storage_covers_a_shortage_while_it_lasts() {
        let mut storages = [battery(9, 1000.0)];
        let b = balance(
            &[producer(0, 4.0)],
            &[consumer(1, 8.0)],
            &mut storages,
            None,
            DT,
        );
        assert!(b.enough());
        assert!(close(storages[0].stored, 996.0));
        // An empty battery covers nothing.
        let mut empty = [battery(9, 0.0)];
        let b = balance(
            &[producer(0, 4.0)],
            &[consumer(1, 8.0)],
            &mut empty,
            None,
            DT,
        );
        assert!(close(b.share, 0.5));
    }

    #[test]
    fn storage_gives_no_faster_than_its_rate() {
        let mut storages = [battery(9, 1000.0)];
        let b = balance(&[], &[consumer(1, 64.0)], &mut storages, None, DT);
        assert!(close(b.supplied[&[1, 0, 0]], 32.0));
        assert!(close(storages[0].stored, 968.0));
    }

    #[test]
    fn the_channel_caps_the_flow() {
        // Rule E2: a tier-1 energy channel passes 32 E/s.
        let b = balance(
            &[producer(0, 40.0)],
            &[consumer(1, 40.0)],
            &mut [],
            Some(32.0),
            DT,
        );
        assert!(close(b.supplied[&[1, 0, 0]], 32.0));
        assert!(close(b.share, 0.8));
    }

    #[test]
    fn storage_does_not_overfill() {
        let mut storages = [Storage {
            capacity: 105.0,
            ..battery(9, 100.0)
        }];
        balance(&[producer(0, 16.0)], &[], &mut storages, None, DT);
        assert!(close(storages[0].stored, 105.0));
    }

    #[test]
    fn a_source_powers_a_touching_machine_but_a_machine_passes_nothing_on() {
        // Rule E10: crystal at x=0, furnace at x=1 touches it, furnace at x=2 touches only x=1.
        let feed = direct_feed(&[producer(0, 4.0)], &[consumer(1, 8.0), consumer(2, 8.0)]);
        assert!(close(feed.supplied[&[1, 0, 0]], 4.0));
        assert!(feed.supplied.get(&[2, 0, 0]).is_none_or(|e| close(*e, 0.0)));
        assert!(close(feed.source_left[&[0, 0, 0]], 0.0));
    }

    fn circuit(members: &[i32], cap: f64) -> Circuit {
        Circuit {
            members: members.iter().map(|&x| [x, 0, 0]).collect(),
            cap,
        }
    }

    #[test]
    fn a_crystal_powers_the_furnace_it_touches_and_not_the_one_behind() {
        // In-game check (b), E10: crystal x=0, furnace x=1 beside it, furnace x=2 beside that.
        let got = supply(
            &[producer(0, 4.0)],
            &[consumer(1, 8.0), consumer(2, 8.0)],
            &[],
            DT,
        );
        assert!(close(got[&[1, 0, 0]], 4.0));
        assert!(!got.contains_key(&[2, 0, 0]));
    }

    #[test]
    fn two_furnaces_on_one_crystal_through_a_channel_get_half_its_output_each() {
        // In-game check (c), E3: a 4 E/s crystal and two 8 E/s furnaces on one energy net get
        // 2 E/s each, a quarter of what they ask: half the speed of one furnace on that crystal.
        let net = circuit(&[0, 5, 9], 32.0);
        let got = supply(
            &[producer(0, 4.0)],
            &[consumer(5, 8.0), consumer(9, 8.0)],
            &[net],
            DT,
        );
        assert!(close(got[&[5, 0, 0]], 2.0));
        assert!(close(got[&[9, 0, 0]], 2.0));
    }

    #[test]
    fn what_a_source_gives_by_contact_is_not_given_again_through_the_net() {
        // A 16 E/s source touches furnace x=1 (8 E/s) and is on a net with furnace x=7.
        let net = circuit(&[0, 7], 32.0);
        let got = supply(
            &[producer(0, 16.0)],
            &[consumer(1, 8.0), consumer(7, 16.0)],
            &[net],
            DT,
        );
        assert!(close(got[&[1, 0, 0]], 8.0));
        assert!(close(got[&[7, 0, 0]], 8.0));
    }

    #[test]
    fn a_machine_on_two_nets_is_not_fed_twice() {
        let a = circuit(&[0, 5], 32.0);
        let b = circuit(&[9, 5], 32.0);
        let got = supply(
            &[producer(0, 6.0), producer(9, 6.0)],
            &[consumer(5, 8.0)],
            &[a, b],
            DT,
        );
        assert!(close(got[&[5, 0, 0]], 8.0));
    }

    #[test]
    fn the_channel_tier_caps_a_circuit() {
        // E2: a 4 E/s cap between a 40 E/s source and a 40 E/s machine.
        let got = supply(
            &[producer(0, 40.0)],
            &[consumer(5, 40.0)],
            &[circuit(&[0, 5], 4.0)],
            DT,
        );
        assert!(close(got[&[5, 0, 0]], 4.0));
    }

    #[test]
    fn the_report_says_what_each_source_gave() {
        // A 16 E/s generator touching one 8 E/s furnace and a crystal on a net with a furnace
        // asking 2: the generator gives 8, the crystal 2.
        let net = circuit(&[5, 7], 32.0);
        let report = supply_report(
            &[producer(0, 16.0), producer(5, 4.0)],
            &[consumer(1, 8.0), consumer(7, 2.0)],
            &[net],
            DT,
        );
        assert!(close(report.used[&[0, 0, 0]], 8.0));
        assert!(close(report.used[&[5, 0, 0]], 2.0));
        assert!(close(report.got[&[1, 0, 0]], 8.0));
        // A source nobody takes from gave nothing.
        let idle = supply_report(&[producer(0, 16.0)], &[], &[], DT);
        assert!(close(idle.used[&[0, 0, 0]], 0.0));
    }

    #[test]
    fn a_generators_window_sums_up_its_circuit() {
        // E17: generator x=0 touches furnace x=1 and is on a net with furnaces x=5 and x=9.
        let net = circuit(&[0, 5, 9], 32.0);
        let producers = [producer(0, 16.0)];
        let consumers = [
            consumer(1, 8.0),
            consumer(5, 8.0),
            consumer(9, 8.0),
            consumer(20, 8.0),
        ];
        let members = group_of([0, 0, 0], &consumers, std::slice::from_ref(&net));
        assert_eq!(members.len(), 4);
        assert!(!members.contains(&[20, 0, 0]));
        let got = supply(&producers, &consumers, std::slice::from_ref(&net), DT);
        let b = GroupBalance::of(&members, &producers, &consumers, &got);
        assert!(close(b.produced, 16.0));
        assert!(close(b.requested, 24.0));
        assert!(close(b.supplied, 16.0));
        assert!(!b.enough());
        assert_eq!(b.slowdown_percent(), 33);
        let full = GroupBalance {
            produced: 16.0,
            requested: 8.0,
            supplied: 8.0,
        };
        assert!(full.enough());
        assert_eq!(full.slowdown_percent(), 0);
    }

    fn store(x: i32, give: f64, take: f64) -> Store {
        Store {
            block: [x, 0, 0],
            give,
            take,
        }
    }

    #[test]
    fn a_battery_covers_what_the_sources_lack_and_takes_their_rest() {
        // A battery x=1 touches a furnace x=2 asking 8, alone: it gives 8.
        let s = supply_with_storage(&[], &[consumer(2, 8.0)], &[store(1, 32.0, 32.0)], &[], DT);
        assert!(close(s.got[&[2, 0, 0]], 8.0));
        assert!(close(s.given[&[1, 0, 0]], 8.0));
        assert!(s.taken.is_empty());
        // A crystal x=0 touching the battery and nothing else charges it with all its 4.
        let s = supply_with_storage(&[producer(0, 4.0)], &[], &[store(1, 32.0, 32.0)], &[], DT);
        assert!(close(s.taken[&[1, 0, 0]], 4.0));
        assert!(close(s.used[&[0, 0, 0]], 4.0));
        // A battery gives no faster than its rate: 48 asked, 32 given.
        let s = supply_with_storage(
            &[],
            &[consumer(2, 24.0), consumer(9, 24.0)],
            &[store(1, 32.0, 32.0)],
            &[circuit(&[1, 2, 9], 512.0)],
            DT,
        );
        let total: f64 = s.got.values().sum();
        assert!(close(total, 32.0));
    }

    #[test]
    fn a_net_that_reaches_one_block_of_a_structure_reaches_all() {
        let mut nets = [circuit(&[0, 5], 32.0)];
        let group: BTreeSet<Pos> = [[5, 0, 0], [6, 0, 0], [7, 0, 0]].into();
        join_groups(&mut nets, &[group]);
        assert!(nets[0].members.contains(&[7, 0, 0]));
        let mut apart = [circuit(&[0, 1], 32.0)];
        join_groups(&mut apart, &[[[9, 0, 0]].into()]);
        assert!(!apart[0].members.contains(&[9, 0, 0]));
    }

    #[test]
    fn a_source_shares_among_touching_machines_and_keeps_the_rest() {
        // A 16 E/s generator between two furnaces asking 4 each: both full, 8 left for its net.
        let feed = direct_feed(&[producer(0, 16.0)], &[consumer(-1, 4.0), consumer(1, 4.0)]);
        assert!(close(feed.supplied[&[-1, 0, 0]], 4.0));
        assert!(close(feed.supplied[&[1, 0, 0]], 4.0));
        assert!(close(feed.source_left[&[0, 0, 0]], 8.0));
    }
}
