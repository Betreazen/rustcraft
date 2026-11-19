//! Round 31: housings saved before the channels were built in (0.19 and older).
//!
//! Such a housing keeps the channels a player set into its four slots (four bytes, `kind * 16 +
//! tier` each, 0 for an empty slot) and its joints' settings per face and per old slot. On update
//! every housing gets all its channels (`net::topology`): its tier is the highest tier among the
//! channels it had (1 with none), and its joints' settings move by channel kind to that kind's
//! fixed slot. Where it had two channels of one kind, the settings of the one in the bigger net
//! win; a face only the other one had a setting for keeps that setting, so nothing set is lost.
//! The bundle reads the old save, asks `plan` and `remap`, and writes the housing back.
//!
//! 0.21 (XC9): a housing saved by 0.20 keeps its settings of slots 0–2 as they are, and the
//! experience slot it never had is empty, so its joints work in their default. Before 0.21 there
//! was no experience channel, so no old slot names one and `plan` gives the experience slot no
//! sources.

use std::collections::{BTreeMap, BTreeSet};

use crate::geom::Pos;
use crate::net::settings::Record;
use crate::net::topology::{Channel, ChannelKind, Layout, MAX_TIER, SLOTS, links};
use crate::tuning::Tuning;

/// Slots a housing had before round 31.
pub const OLD_SLOTS: usize = 4;

/// The channels a housing saved before round 31 held, slot by slot.
pub type OldSlots = [Option<Channel>; OLD_SLOTS];

/// Byte of an empty slot in the old save.
const EMPTY: u8 = 0;
/// An old slot's byte is `kind * KIND_STEP + tier`; tiers 1 to 15 fit below the step.
const KIND_STEP: u8 = 16;

/// The kind's code in a 0.19 save; none for experience, which 0.19 had no channel of.
fn kind_code(kind: ChannelKind) -> Option<u8> {
    match kind {
        ChannelKind::Item => Some(1),
        ChannelKind::Fluid => Some(2),
        ChannelKind::Energy => Some(3),
        ChannelKind::Xp => None,
    }
}

fn kind_of(code: u8) -> Option<ChannelKind> {
    match code {
        1 => Some(ChannelKind::Item),
        2 => Some(ChannelKind::Fluid),
        3 => Some(ChannelKind::Energy),
        _ => None,
    }
}

/// The old slots of a saved housing. A byte it does not know reads as an empty slot, and missing
/// bytes too, so a damaged save loses a channel instead of the housing.
pub fn decode(bytes: &[u8]) -> OldSlots {
    let mut slots = [None; OLD_SLOTS];
    for (slot, &byte) in slots.iter_mut().zip(bytes) {
        let tier = byte % KIND_STEP;
        *slot = kind_of(byte / KIND_STEP)
            .filter(|_| tier >= 1)
            .map(|kind| Channel { kind, tier });
    }
    slots
}

/// The old save of `slots`: what 0.19 wrote, for the self-test that builds an old housing. An
/// experience channel, which 0.19 could not hold, is written as an empty slot.
pub fn encode(slots: &OldSlots) -> [u8; OLD_SLOTS] {
    slots.map(|slot| {
        slot.and_then(|c| kind_code(c.kind).map(|code| code * KIND_STEP + c.tier.clamp(1, KIND_STEP - 1)))
            .unwrap_or(EMPTY)
    })
}

/// The tier a housing gets: the highest tier among its old channels, 1 with none.
pub fn tier_of(slots: &OldSlots) -> u8 {
    slots
        .iter()
        .flatten()
        .map(|c| c.tier)
        .max()
        .unwrap_or(1)
        .clamp(1, MAX_TIER)
}

/// What an old housing becomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Its tier (`tier_of`).
    pub tier: u8,
    /// For each slot of the new housing, the old slots that held a channel of that slot's kind,
    /// the one in the bigger old net first (the lower slot first among equals).
    pub sources: [Vec<usize>; SLOTS],
}

/// The plan of every housing of `old`. Their places are housings of `layout`, whose nodes join
/// them as `topology::analyse` does. An old net is the housings of `old` joined to each other that
/// hold a channel of the same kind in the same slot; a node's range does not count, so a line out
/// of reach keeps its settings too.
pub fn plan(
    old: &BTreeMap<Pos, OldSlots>,
    layout: &Layout,
    tuning: &Tuning,
) -> BTreeMap<Pos, Plan> {
    let size = old_net_sizes(old, layout, tuning);
    old.iter()
        .map(|(&pos, slots)| {
            let sources = ChannelKind::ALL.map(|kind| {
                let mut from: Vec<usize> = (0..OLD_SLOTS)
                    .filter(|&s| slots[s].is_some_and(|c| c.kind == kind))
                    .collect();
                from.sort_by_key(|&s| {
                    let len = size.get(&(pos, s)).copied().unwrap_or(1);
                    (std::cmp::Reverse(len), s)
                });
                from
            });
            let plan = Plan {
                tier: tier_of(slots),
                sources,
            };
            (pos, plan)
        })
        .collect()
}

/// The number of housings in the old net of each housing's channel, by housing and old slot.
fn old_net_sizes(
    old: &BTreeMap<Pos, OldSlots>,
    layout: &Layout,
    tuning: &Tuning,
) -> BTreeMap<(Pos, usize), usize> {
    let mut size = BTreeMap::new();
    for slot in 0..OLD_SLOTS {
        let mut seen = BTreeSet::new();
        for (&start, slots) in old {
            let Some(channel) = slots[slot] else {
                continue;
            };
            if !seen.insert(start) {
                continue;
            }
            let mut net = vec![start];
            let mut stack = vec![start];
            while let Some(h) = stack.pop() {
                for (next, _) in links(layout, tuning, h) {
                    let same = old
                        .get(&next)
                        .and_then(|s| s[slot])
                        .is_some_and(|c| c.kind == channel.kind);
                    if same && seen.insert(next) {
                        net.push(next);
                        stack.push(next);
                    }
                }
            }
            for h in &net {
                size.insert((*h, slot), net.len());
            }
        }
    }
    size
}

/// The joints' records of a housing after `plan`: each moved to the slot of its channel's kind.
/// Where two old slots of one kind have a record for the same face, the one `plan` puts first is
/// kept; a record of an old slot that held no channel is dropped (the old window set none).
pub fn remap(records: &[Record], plan: &Plan) -> Vec<Record> {
    let mut out: BTreeMap<(u8, usize), Record> = BTreeMap::new();
    for (slot, from) in plan.sources.iter().enumerate() {
        for &old_slot in from {
            for r in records.iter().filter(|r| usize::from(r.slot) == old_slot) {
                out.entry((r.face, slot)).or_insert_with(|| Record {
                    slot: slot as u8,
                    ..r.clone()
                });
            }
        }
    }
    out.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Face;
    use crate::net::items::Mode;
    use crate::net::settings::{HousingJoints, JointSetting};
    use crate::net::topology::{
        ENERGY_SLOT, FLUID_SLOT, Housing, ITEM_SLOT, Node, analyse,
    };

    fn ch(kind: ChannelKind, tier: u8) -> Option<Channel> {
        Some(Channel { kind, tier })
    }

    fn record(face: Face, slot: u8, mode: Mode) -> Record {
        HousingJoints::default()
            .with(face, 0, JointSetting::of_mode(mode))
            .to_records()
            .into_iter()
            .map(|r| Record { slot, ..r })
            .next()
            .unwrap()
    }

    /// `layout` with a housing at every place of `old` (they keep their new tier).
    fn layout_of(old: &BTreeMap<Pos, OldSlots>, nodes: &[(Pos, u8)]) -> Layout {
        let mut layout = Layout::default();
        for (pos, slots) in old {
            layout.housings.insert(*pos, Housing::new(tier_of(slots)));
        }
        for (pos, tier) in nodes {
            layout.nodes.insert(*pos, Node { tier: *tier });
        }
        layout
    }

    #[test]
    fn old_slots_survive_their_save() {
        let slots = [
            ch(ChannelKind::Item, 1),
            None,
            ch(ChannelKind::Energy, 3),
            ch(ChannelKind::Fluid, 1),
        ];
        assert_eq!(decode(&encode(&slots)), slots);
    }

    #[test]
    fn a_damaged_old_save_reads_as_empty_slots() {
        assert_eq!(decode(&[]), [None; OLD_SLOTS]);
        // Unknown kind 9, a tier of 0, then one good item channel; the fifth byte is ignored.
        let slots = decode(&[9 * KIND_STEP + 1, KIND_STEP, KIND_STEP + 2, 0, 17]);
        assert_eq!(slots[..2], [None, None]);
        assert_eq!(slots[2], ch(ChannelKind::Item, 2));
    }

    #[test]
    fn the_tier_is_the_highest_old_channel_or_one() {
        assert_eq!(tier_of(&[None; OLD_SLOTS]), 1);
        let slots = [ch(ChannelKind::Item, 2), None, ch(ChannelKind::Energy, 3), None];
        assert_eq!(tier_of(&slots), 3);
        assert_eq!(tier_of(&[ch(ChannelKind::Fluid, 9), None, None, None]), MAX_TIER);
    }

    #[test]
    fn the_owners_line_becomes_one_net_of_each_kind() {
        // The owner's world (round 31): 134 housings in a line with energy in slot 1; items in
        // slot 2 on two branches of 35 and 25 housings, split by one housing with no item
        // channel; one housing with a lone item channel in slot 3; a tier-4 hub at the start.
        let energy = ch(ChannelKind::Energy, 1);
        let item = ch(ChannelKind::Item, 1);
        let mut old: BTreeMap<Pos, OldSlots> = BTreeMap::new();
        for x in 1..=134 {
            let items = (1..=35).contains(&x) || (37..=61).contains(&x);
            let slots = match x {
                90 => [energy, None, item, None],
                _ if items => [energy, item, None, None],
                _ => [energy, None, None, None],
            };
            old.insert([x, 64, 82], slots);
        }
        let layout = layout_of(&old, &[([0, 64, 82], 4)]);
        let plans = plan(&old, &layout, &Tuning::default());
        assert!(plans.values().all(|p| p.tier == 1));
        assert_eq!(plans[&[5, 64, 82]].sources, [vec![1], vec![], vec![0], vec![]]);
        assert_eq!(plans[&[36, 64, 82]].sources, [vec![], vec![], vec![0], vec![]]);
        assert_eq!(plans[&[90, 64, 82]].sources, [vec![2], vec![], vec![0], vec![]]);
        // Built in, the channels join: the fishing station's catch at one end reaches the storage
        // at the other on one item net.
        let topology = analyse(&layout, &Tuning::default());
        let items = topology.net_of_kind([1, 64, 82], ChannelKind::Item).unwrap();
        assert_eq!(items.housings.len(), 134);
        assert!(items.housings.contains(&[134, 64, 82]));
        // A joint's settings move with their kind: energy from slot 1 to its own, items from 2.
        let records = [
            record(Face::ZNeg, 0, Mode::Into),
            record(Face::ZNeg, 1, Mode::From),
        ];
        let moved = HousingJoints::from_records(&remap(&records, &plans[&[5, 64, 82]]));
        assert_eq!(moved.get(Face::ZNeg, ENERGY_SLOT).map(|s| s.mode), Some(Mode::Into));
        assert_eq!(moved.get(Face::ZNeg, ITEM_SLOT).map(|s| s.mode), Some(Mode::From));
        assert_eq!(moved.get(Face::ZNeg, FLUID_SLOT), None);
        // The lone item channel's settings, from slot 3.
        let lone = remap(&[record(Face::YPos, 2, Mode::From)], &plans[&[90, 64, 82]]);
        assert_eq!(lone.len(), 1);
        assert_eq!((lone[0].slot, lone[0].face), (ITEM_SLOT as u8, Face::YPos.index()));
    }

    #[test]
    fn two_channels_of_a_kind_keep_the_bigger_nets_settings() {
        // Items in slot 1 along four housings and a lone one in slot 3 of the second; fluid in
        // slot 2 of the second only.
        let mut old: BTreeMap<Pos, OldSlots> = BTreeMap::new();
        for x in 1..=4 {
            old.insert([x, 0, 0], [None, ch(ChannelKind::Item, 1), None, None]);
        }
        old.insert(
            [2, 0, 0],
            [None, ch(ChannelKind::Item, 1), ch(ChannelKind::Fluid, 2), ch(ChannelKind::Item, 2)],
        );
        let layout = layout_of(&old, &[([0, 0, 0], 1)]);
        let p = &plan(&old, &layout, &Tuning::default())[&[2, 0, 0]];
        assert_eq!(p.tier, 2);
        assert_eq!(p.sources, [vec![1, 3], vec![2], vec![], vec![]]);
        let records = [
            // Both item channels on the north face: the bigger net's wins.
            record(Face::ZNeg, 1, Mode::From),
            record(Face::ZNeg, 3, Mode::Into),
            // Only the lone one on the south face: kept.
            record(Face::ZPos, 3, Mode::Off),
            record(Face::ZPos, 2, Mode::Into),
            // A record of the empty slot: no channel to move with.
            record(Face::XPos, 0, Mode::From),
        ];
        let moved = HousingJoints::from_records(&remap(&records, p));
        assert_eq!(moved.settings.len(), 3);
        assert_eq!(moved.get(Face::ZNeg, ITEM_SLOT).map(|s| s.mode), Some(Mode::From));
        assert_eq!(moved.get(Face::ZPos, ITEM_SLOT).map(|s| s.mode), Some(Mode::Off));
        assert_eq!(moved.get(Face::ZPos, FLUID_SLOT).map(|s| s.mode), Some(Mode::Into));
    }

    #[test]
    fn a_housing_saved_with_three_slots_keeps_them_and_its_experience_joints_default() {
        // 0.21 (XC9): what 0.20 saved, settings of slots 0–2, reads back as it was; slot 3 has
        // none, so every experience joint works in the block's default.
        use crate::net::topology::XP_SLOT;
        let saved = HousingJoints::default()
            .with(Face::ZNeg, ITEM_SLOT, JointSetting::of_mode(Mode::From))
            .with(Face::ZNeg, FLUID_SLOT, JointSetting::of_mode(Mode::Into))
            .with(Face::XPos, ENERGY_SLOT, JointSetting::of_mode(Mode::Off));
        let records = saved.to_records();
        assert!(records.iter().all(|r| usize::from(r.slot) < XP_SLOT));
        let read = HousingJoints::from_records(&records);
        assert_eq!(read, saved);
        assert!(Face::ALL.iter().all(|&f| read.get(f, XP_SLOT).is_none()));
        assert_eq!(
            read.resolved(Face::ZNeg, XP_SLOT, crate::net::xp::XP_DEFAULT).mode,
            Mode::Both
        );
    }

    #[test]
    fn an_old_fourth_slot_moves_by_its_kind_not_into_experience() {
        // A 0.19 housing with energy in its fourth slot: its settings go to the energy slot, and
        // the experience slot (the fourth since 0.21) gets nothing of it.
        use crate::net::topology::XP_SLOT;
        let mut old = BTreeMap::new();
        old.insert([1, 0, 0], [None, None, None, ch(ChannelKind::Energy, 2)]);
        let layout = layout_of(&old, &[([0, 0, 0], 1)]);
        let p = &plan(&old, &layout, &Tuning::default())[&[1, 0, 0]];
        assert_eq!(p.sources, [vec![], vec![], vec![3], vec![]]);
        let moved = HousingJoints::from_records(&remap(&[record(Face::YPos, 3, Mode::From)], p));
        assert_eq!(moved.get(Face::YPos, ENERGY_SLOT).map(|s| s.mode), Some(Mode::From));
        assert_eq!(moved.get(Face::YPos, XP_SLOT), None);
        // 0.19 had no experience channel: one written is an empty slot.
        assert_eq!(encode(&[ch(ChannelKind::Xp, 2), None, None, None]), [EMPTY; OLD_SLOTS]);
    }

    #[test]
    fn a_housing_without_channels_is_tier_one_and_drops_nothing_it_had() {
        let mut old = BTreeMap::new();
        old.insert([0, 0, 0], [None; OLD_SLOTS]);
        let layout = layout_of(&old, &[]);
        let p = &plan(&old, &layout, &Tuning::default())[&[0, 0, 0]];
        assert_eq!(p.tier, 1);
        assert!(p.sources.iter().all(Vec::is_empty));
        assert!(remap(&[], p).is_empty());
    }
}
