//! Slice 6: the monorail without a rider. Rails take the shape their neighbours give them (T2,
//! spec 18.6); stations of one network see each other (T6); a trip is refused for want of energy,
//! for a broken way or for no room (T8.a, T8.b), and a trip that starts is paid for (T8). The ride
//! itself needs a player and stays with the bot check.

use std::collections::BTreeSet;

use super::energy::STATION;
use super::{Bench, Build, Scenario, at, expect, near, plot_area, same};
use crate::geom::{Face, Pos};
use crate::monorail::shape::{self, Shape};
use crate::monorail::{Network, Rail, TripError, visible_stations};
use crate::tuning::Tuning;

pub const ALL: [Build; 6] = [
    shapes,
    stations_see_each_other,
    refused_without_energy,
    refused_on_a_broken_way,
    refused_without_room,
    a_trip_is_paid,
];

const MONORAIL: &str = "rustcraft:monorail";
/// Rails of the straight track: x 0..=TRACK, one block over the plot's origin.
const TRACK: i32 = 8;
/// The energy a station is given when energy is not what a scenario checks.
const PLENTY: f64 = 100.0;

fn rail(o: Pos, x: i32, z: i32) -> Pos {
    at(o, [x, 1, z])
}

fn place_rail(b: &mut dyn Bench, p: Pos) -> Result<(), String> {
    b.place(
        p,
        MONORAIL,
        &[(shape::MOUNT, shape::mount_value(Face::YNeg))],
    )
}

/// The two stations of the straight track, beside its ends.
fn ends(o: Pos) -> (Pos, Pos) {
    (rail(o, 0, -1), rail(o, TRACK, -1))
}

/// A straight track of floor rails with a station beside each end, leaving out `gap`.
fn track(b: &mut dyn Bench, o: Pos, gap: Option<i32>) -> Result<(), String> {
    for x in (0..=TRACK).filter(|x| Some(*x) != gap) {
        place_rail(b, rail(o, x, 0))?;
    }
    let (from, to) = ends(o);
    b.place(from, STATION, &[])?;
    b.place(to, STATION, &[])
}

fn shapes(o: Pos, _: &Tuning) -> Scenario {
    // A straight run, a corner, a climb, and a station the first rail turns into.
    let cells: Vec<Pos> = (0..=4)
        .map(|x| rail(o, x, 0))
        .chain([rail(o, 4, 1), rail(o, 4, 2), at(o, [4, 2, 3])])
        .collect();
    let station = rail(o, 0, -1);
    let built = cells.clone();
    Scenario::new("monorail_rails_take_their_shapes", plot_area(o))
        .then(
            "build a track with a corner, a climb and a station",
            move |b, _| {
                for &p in &built {
                    place_rail(b, p)?;
                }
                b.place(station, STATION, &[])
            },
        )
        .check("each rail shows the rules' shape", move |b, _| {
            let mut net = Network::default();
            for &p in &cells {
                net.rails.insert(
                    p,
                    Rail {
                        tier: 1,
                        mount: Face::YNeg,
                    },
                );
            }
            let stations = BTreeSet::from([station]);
            same(
                "the middle of the straight run",
                shape::shape_of(&net, rail(o, 2, 0), &stations),
                Shape::Halves(0b0101),
            )?;
            for &p in &cells {
                let wanted = shape::shape_of(&net, p, &stations).value();
                same(
                    &format!("shape at {p:?}"),
                    b.property(p, shape::PROPERTY),
                    Some(wanted),
                )?;
            }
            Ok(())
        })
}

fn stations_see_each_other(o: Pos, _: &Tuning) -> Scenario {
    let (from, to) = ends(o);
    let apart = rail(o, 0, 3);
    Scenario::new("monorail_stations_see_each_other", plot_area(o))
        .then(
            "build a track with two stations and a station apart",
            move |b, _| {
                track(b, o, None)?;
                for x in 0..=2 {
                    place_rail(b, rail(o, x, 4))?;
                }
                b.place(apart, STATION, &[])
            },
        )
        .check(
            "the stations of one network see each other only",
            move |b, _| {
                let net = b.network();
                same(
                    "seen from the first",
                    visible_stations(&net, from),
                    vec![to],
                )?;
                same(
                    "seen from the one apart",
                    visible_stations(&net, apart),
                    vec![],
                )
            },
        )
        .check(
            "the network's record comes back the same from a save",
            move |b, _| b.round_trip(from),
        )
}

/// A scenario that asks for a trip between the track's stations and checks the answer.
fn trip(
    o: Pos,
    name: &'static str,
    gap: Option<i32>,
    energy: f64,
    extra: Option<Pos>,
    check: impl Fn(&mut dyn Bench, Result<crate::monorail::Trip, TripError>) -> Result<(), String>
    + Send
    + 'static,
) -> Scenario {
    let (from, to) = ends(o);
    Scenario::new(name, plot_area(o))
        .then("build the track", move |b, _| {
            track(b, o, gap)?;
            if let Some(p) = extra {
                b.place(p, "minecraft:stone", &[])?;
            }
            b.set_station_energy(from, energy)
        })
        .check("the trip is answered as the rules say", move |b, _| {
            let answer = b.plan_trip(from, to, true);
            check(b, answer)
        })
}

fn refused_without_energy(o: Pos, t: &Tuning) -> Scenario {
    let cost = expect::trip_cost(t, TRACK as u32);
    trip(
        o,
        "monorail_trip_refused_without_energy",
        None,
        0.0,
        None,
        move |_, answer| {
            same(
                "answer",
                answer.map(|t| t.length),
                Err(TripError::NotEnoughEnergy { short: cost }),
            )
        },
    )
}

fn refused_on_a_broken_way(o: Pos, _: &Tuning) -> Scenario {
    let gap = TRACK / 2;
    let ends_at = rail(o, gap - 1, 0);
    trip(
        o,
        "monorail_trip_refused_on_a_broken_way",
        Some(gap),
        PLENTY,
        None,
        move |_, answer| {
            same(
                "answer",
                answer.map(|t| t.length),
                Err(TripError::Broken { ends_at }),
            )
        },
    )
}

fn refused_without_room(o: Pos, _: &Tuning) -> Scenario {
    let block = at(o, [TRACK / 2, 2, 0]);
    trip(
        o,
        "monorail_trip_refused_without_room",
        None,
        PLENTY,
        Some(block),
        move |_, answer| {
            same(
                "answer",
                answer.map(|t| t.length),
                Err(TripError::NoRoom { at: block }),
            )
        },
    )
}

fn a_trip_is_paid(o: Pos, t: &Tuning) -> Scenario {
    let cost = expect::trip_cost(t, TRACK as u32);
    let (from, _) = ends(o);
    trip(
        o,
        "monorail_trip_pays_its_energy",
        None,
        PLENTY,
        None,
        move |b, answer| {
            let trip = answer.map_err(|e| format!("refused: {e:?}"))?;
            same("length", trip.length, TRACK as u32)?;
            near("cost", trip.cost, cost, 1e-9)?;
            let left = b.station_energy(from).ok_or("no station")?;
            near("energy left", left, PLENTY - cost, 1e-6)
        },
    )
}
