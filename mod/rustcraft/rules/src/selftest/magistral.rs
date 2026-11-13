//! Slice 1: housings, nodes and channels; what a housing shows (N3, M3) and what its block entity
//! keeps through a save.

use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::geom::{Face, Pos};
use crate::housing::{Diagnosis, SlotState};
use crate::net::items::Mode;
use crate::net::settings::JointSetting;
use crate::net::topology::{Channel, ChannelKind, SLOTS};
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [served_line, too_far, no_node, slots_survive_a_save];

pub const HOUSING: &str = "rustcraft:housing";
pub const NODE: &str = "rustcraft:node";

pub fn channel(kind: ChannelKind, tier: u8) -> Option<Channel> {
    Some(Channel { kind, tier })
}

/// A node at `origin` (of `tier`) and `len` housings after it along x, each with `slots`.
pub fn line(
    b: &mut dyn Bench,
    origin: Pos,
    tier: u8,
    len: i32,
    slots: [Option<Channel>; SLOTS],
) -> Result<(), String> {
    place_node(b, origin, tier)?;
    for x in 1..=len {
        let p = at(origin, [x, 0, 0]);
        b.place(p, HOUSING, &[])?;
        b.set_channels(p, slots)?;
    }
    Ok(())
}

pub fn place_node(b: &mut dyn Bench, pos: Pos, tier: u8) -> Result<(), String> {
    b.place(pos, NODE, &[("tier", &tier.to_string())])
}

fn diagnosis(b: &dyn Bench, pos: Pos) -> Result<Diagnosis, String> {
    b.housing(pos)
        .ok_or_else(|| format!("no housing known at {pos:?}"))
}

/// N3, M3: housings within the node's range work; each slot names its net's length; a node of
/// tier 2 serves its own range.
fn served_line(o: Pos, t: &Tuning) -> Scenario {
    let item = channel(ChannelKind::Item, 1);
    let energy = channel(ChannelKind::Energy, 1);
    let slots = [item, None, energy, None];
    let second = at(o, [0, 0, 4]);
    let (limit1, limit2) = (t.range_of(1), t.range_of(2));
    let net = |kind, tier| SlotState::Channel {
        kind,
        tier,
        net_len: 5,
    };
    Scenario::new("magistral_served_line", plot_area(o))
        .then("build two lines of five", move |b, _| {
            line(b, o, 1, 5, slots)?;
            line(b, second, 2, 5, slots)
        })
        .check("the tier-1 line works", move |b, _| {
            same(
                "diagnosis",
                diagnosis(b, at(o, [3, 0, 0]))?,
                Diagnosis::Works {
                    distance: 3,
                    limit: limit1,
                    line_len: 5,
                    slots: [
                        net(ChannelKind::Item, 1),
                        SlotState::Empty,
                        net(ChannelKind::Energy, 1),
                        SlotState::Empty,
                    ],
                },
            )
        })
        .check(
            "the tier-2 node serves its own range",
            move |b, _| match diagnosis(b, at(second, [5, 0, 0]))? {
                Diagnosis::Works {
                    distance: 5, limit, ..
                } if limit == limit2 => Ok(()),
                d => Err(format!("{d:?}, expected works at 5 of {limit2}")),
            },
        )
}

/// N3: the housing one block past a tier-1 node's range is «Дальше предела узла».
fn too_far(o: Pos, t: &Tuning) -> Scenario {
    let limit = t.range_of(1);
    let len = limit as i32 + 1;
    Scenario::new("magistral_too_far", plot_area(o))
        .then("build a line one past the range", move |b, _| {
            line(
                b,
                o,
                1,
                len,
                [channel(ChannelKind::Item, 1), None, None, None],
            )
        })
        .check("the last housing is too far", move |b, _| {
            same(
                "diagnosis",
                diagnosis(b, at(o, [len, 0, 0]))?,
                Diagnosis::TooFar {
                    distance: limit + 1,
                    limit,
                },
            )
        })
        .check("the one before still works", move |b, _| {
            match diagnosis(b, at(o, [len - 1, 0, 0]))? {
                Diagnosis::Works { distance, .. } if distance == limit => Ok(()),
                d => Err(format!("{d:?}, expected works at {limit}")),
            }
        })
}

/// N3.a: housings with no node on their line.
fn no_node(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("magistral_no_node", plot_area(o))
        .then("build housings without a node", move |b, _| {
            for x in 0..3 {
                let p = at(o, [x, 0, 0]);
                b.place(p, HOUSING, &[])?;
                b.set_channels(p, [channel(ChannelKind::Item, 1), None, None, None])?;
            }
            Ok(())
        })
        .check("they say there is no node", move |b, _| {
            same(
                "diagnosis",
                diagnosis(b, at(o, [1, 0, 0]))?,
                Diagnosis::NoNode,
            )
        })
}

/// M1, M7: a housing's slots and its joint settings come back the same from the binary NBT a save
/// writes, and so do the level's storage and monorail records.
fn slots_survive_a_save(o: Pos, _: &Tuning) -> Scenario {
    let slots = [
        channel(ChannelKind::Fluid, 3),
        None,
        channel(ChannelKind::Item, 2),
        channel(ChannelKind::Energy, 4),
    ];
    let h = at(o, [1, 0, 0]);
    Scenario::new("magistral_slots_survive_a_save", plot_area(o))
        .then(
            "build a housing with three channels and a joint",
            move |b, _| {
                line(b, o, 4, 1, slots)?;
                b.place(at(h, [0, 0, -1]), "minecraft:chest", &[])?;
                let mut setting = JointSetting::of_mode(Mode::From);
                setting.priority = 3;
                setting.filter.allow_only = true;
                setting.filter.items.insert("minecraft:cobblestone".into());
                b.set_joint(h, Face::ZNeg, 2, setting)
            },
        )
        .check(
            "the block entity and the records come back the same",
            move |b, _| b.round_trip(h),
        )
        .check(
            "the slots are read back as set",
            move |b, _| match diagnosis(b, h)? {
                Diagnosis::Works { slots: shown, .. } => {
                    let kinds: Vec<Option<(ChannelKind, u8)>> = shown
                        .iter()
                        .map(|s| match s {
                            SlotState::Channel { kind, tier, .. } => Some((*kind, *tier)),
                            SlotState::Empty => None,
                        })
                        .collect();
                    let set: Vec<Option<(ChannelKind, u8)>> =
                        slots.iter().map(|c| c.map(|c| (c.kind, c.tier))).collect();
                    same("slots", kinds, set)
                }
                d => Err(format!("{d:?}, expected a working housing")),
            },
        )
}
