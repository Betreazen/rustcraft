//! Slice 1: housings and nodes; what a housing shows (N3, M3) and what its block entity keeps
//! through a save. Round 31: every housing has all three channels built in and a tier of its own;
//! a housing saved with channels set by hand gets them built in.

use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::{Face, Pos};
use crate::housing::Diagnosis;
use crate::net::items::Mode;
use crate::net::migrate::OldSlots;
use crate::net::settings::JointSetting;
use crate::net::topology::{Channel, ChannelKind, ENERGY_SLOT, FLUID_SLOT, ITEM_SLOT};
use crate::tuning::Tuning;

pub const ALL: [Build; 5] = [
    served_line,
    too_far,
    no_node,
    housing_survives_a_save,
    old_housings_get_their_channels_built_in,
];

pub const HOUSING: &str = "rustcraft:housing";
pub const NODE: &str = "rustcraft:node";

/// A node at `origin` (of `node_tier`) and `len` housings of `housing_tier` after it along x.
pub fn line(
    b: &mut dyn Bench,
    origin: Pos,
    node_tier: u8,
    len: i32,
    housing_tier: u8,
) -> Result<(), String> {
    place_node(b, origin, node_tier)?;
    for x in 1..=len {
        place_housing(b, at(origin, [x, 0, 0]), housing_tier)?;
    }
    Ok(())
}

pub fn place_node(b: &mut dyn Bench, pos: Pos, tier: u8) -> Result<(), String> {
    b.place(pos, NODE, &[("tier", &tier.to_string())])
}

/// A housing of `tier` at `pos`.
pub fn place_housing(b: &mut dyn Bench, pos: Pos, tier: u8) -> Result<(), String> {
    b.place(pos, HOUSING, &[("tier", &tier.to_string())])
}

fn diagnosis(b: &dyn Bench, pos: Pos) -> Result<Diagnosis, String> {
    b.housing(pos)
        .ok_or_else(|| format!("no housing known at {pos:?}"))
}

/// N3, M3, M10: housings within the node's range work; their nets join the line's housings and
/// carry at its weakest housing's tier; a node of tier 2 serves its own range.
fn served_line(o: Pos, t: &Tuning) -> Scenario {
    let second = at(o, [0, 0, 4]);
    let (limit1, limit2) = (t.range_of(1), t.range_of(2));
    Scenario::new("magistral_served_line", plot_area(o))
        .then("build two lines of five, the first of tier 2 with one of tier 1", move |b, _| {
            line(b, o, 1, 5, 2)?;
            place_housing(b, at(o, [5, 0, 0]), 1)?;
            line(b, second, 2, 5, 1)
        })
        .check("the tier-1 line works, held to its tier-1 housing", move |b, _| {
            same(
                "diagnosis",
                diagnosis(b, at(o, [3, 0, 0]))?,
                Diagnosis::Works {
                    distance: 3,
                    limit: limit1,
                    line_len: 5,
                    tier: 2,
                    net_len: 5,
                    net_tier: 1,
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
        .then("build a line one past the range", move |b, _| line(b, o, 1, len, 1))
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
                b.place(at(o, [x, 0, 0]), HOUSING, &[])?;
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

/// M7, round 31: a housing's tier and its joint settings come back the same from the binary NBT a
/// save writes, and so do the level's storage and monorail records.
fn housing_survives_a_save(o: Pos, _: &Tuning) -> Scenario {
    let h = at(o, [1, 0, 0]);
    Scenario::new("magistral_housing_survives_a_save", plot_area(o))
        .then("build a tier-3 housing with a joint on its fluid channel", move |b, _| {
            line(b, o, 4, 1, 3)?;
            b.place(at(h, [0, 0, -1]), "minecraft:chest", &[])?;
            let mut setting = JointSetting::of_mode(Mode::From);
            setting.priority = 3;
            setting.filter.allow_only = true;
            setting.filter.items.insert("minecraft:cobblestone".into());
            b.set_joint(h, Face::ZNeg, FLUID_SLOT, setting)
        })
        .check(
            "the block entity and the records come back the same",
            move |b, _| b.round_trip(h),
        )
        .check("the housing is of tier 3", move |b, _| match diagnosis(b, h)? {
            Diagnosis::Works { tier: 3, .. } => Ok(()),
            d => Err(format!("{d:?}, expected a working tier-3 housing")),
        })
}

/// Round 31, the owner's world in small: a line of 0.19 housings with energy in slot 1 all along,
/// items in slot 2 on two branches split by a housing without them, a lone item channel in slot 3
/// of another, and a fluid channel of tier 3 in slot 4 of the last. Updated, every housing gets
/// all three channels at the highest tier it had, its joints' settings move with their kind, and
/// the two item branches are one net: the chest at one end fills the chest at the other.
fn old_housings_get_their_channels_built_in(o: Pos, _: &Tuning) -> Scenario {
    const LEN: i32 = 9;
    const COBBLE: &str = "minecraft:cobblestone";
    let ch = |kind, tier| Some(Channel { kind, tier });
    let energy = ch(ChannelKind::Energy, 1);
    let item = ch(ChannelKind::Item, 1);
    let lone = ch(ChannelKind::Item, 2);
    let fluid = ch(ChannelKind::Fluid, 3);
    let old = move |x: i32| -> OldSlots {
        match x {
            // Two item channels: the branch's in slot 2 and a lone one in slot 4.
            3 => [energy, item, None, item],
            4 => [energy, None, None, None],
            1..=7 => [energy, item, None, None],
            8 => [energy, None, lone, None],
            _ => [energy, None, None, fluid],
        }
    };
    let h = move |x: i32| at(o, [x, 0, 0]);
    let (give, take) = (at(o, [1, 0, -1]), at(o, [7, 0, -1]));
    Scenario::new("magistral_old_housings_get_their_channels_built_in", plot_area(o))
        .then("a node and nine housings saved by 0.19, a chest at each end", move |b, _| {
            let set = JointSetting::of_mode;
            place_node(b, o, 1)?;
            for x in 1..=LEN {
                b.place(h(x), HOUSING, &[])?;
            }
            b.place(give, "minecraft:chest", &[])?;
            b.place(take, "minecraft:chest", &[])?;
            for x in 1..=LEN {
                let joints: Vec<(Face, usize, JointSetting)> = match x {
                    1 => vec![(Face::ZNeg, 1, set(Mode::From)), (Face::ZPos, 0, set(Mode::Off))],
                    3 => vec![
                        (Face::ZNeg, 1, set(Mode::Into)),
                        (Face::ZNeg, 3, set(Mode::From)),
                        (Face::ZPos, 3, set(Mode::Off)),
                    ],
                    7 => vec![(Face::ZNeg, 1, set(Mode::Into))],
                    8 => vec![(Face::YPos, 2, set(Mode::From))],
                    _ => Vec::new(),
                };
                b.set_old_housing(h(x), old(x), &joints)?;
            }
            super::transport::fill(b, give, COBBLE, 16)?;
            b.refresh();
            Ok(())
        })
        .until("each housing took the tier of its best channel", 100, move |b, _| {
            let tiers: Vec<Option<String>> = (1..=LEN).map(|x| b.property(h(x), "tier")).collect();
            let want: Vec<Option<String>> =
                [1, 1, 1, 1, 1, 1, 1, 2, 3].iter().map(|t| Some(t.to_string())).collect();
            Ok(tiers == want)
        })
        .check("all nine are one net of each kind, held to tier 1", move |b, _| {
            match diagnosis(b, h(5))? {
                Diagnosis::Works { net_len: 9, net_tier: 1, tier: 1, .. } => Ok(()),
                d => Err(format!("{d:?}, expected a net of 9 at tier 1")),
            }
        })
        .check("the joints' settings moved with their kind", move |b, _| {
            let mode = |x: i32, face: Face, slot: usize| {
                b.housing_joints(h(x)).and_then(|j| j.get(face, slot).map(|s| s.mode))
            };
            same("items of housing 1", mode(1, Face::ZNeg, ITEM_SLOT), Some(Mode::From))?;
            same("energy of housing 1", mode(1, Face::ZPos, ENERGY_SLOT), Some(Mode::Off))?;
            same("old slot 2 gone from housing 1", mode(1, Face::ZNeg, FLUID_SLOT), None)?;
            // Two item channels: the branch's setting wins, the lone one's other face stays.
            same("items of housing 3", mode(3, Face::ZNeg, ITEM_SLOT), Some(Mode::Into))?;
            same("housing 3's lone channel", mode(3, Face::ZPos, ITEM_SLOT), Some(Mode::Off))?;
            same("the lone channel of housing 8", mode(8, Face::YPos, ITEM_SLOT), Some(Mode::From))
        })
        .until("the chest at one branch fills the chest at the other", 600, move |b, _| {
            Ok(count_of(&b.items(take), COBBLE) >= 8)
        })
}
