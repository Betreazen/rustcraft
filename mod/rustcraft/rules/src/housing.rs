//! What a housing holds and how a player changes it (M1, M2), and the diagnosis a housing shows
//! (N3, which replaces M9). Game-free: the bundle keeps `encode`'s bytes in the housing's block
//! entity and turns a `Diagnosis` into text.

use crate::geom::Pos;
use crate::net::topology::{Channel, ChannelKind, Housing, Layout, Reach, SLOTS, Topology};

/// Byte of an empty slot in `encode`.
const EMPTY: u8 = 0;
/// A slot's byte is `kind * KIND_STEP + tier`; tiers 1 to 15 fit below the step.
const KIND_STEP: u8 = 16;
const MAX_TIER: u8 = KIND_STEP - 1;

fn kind_code(kind: ChannelKind) -> u8 {
    match kind {
        ChannelKind::Item => 1,
        ChannelKind::Fluid => 2,
        ChannelKind::Energy => 3,
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

/// The four slots as four bytes, the form the housing saves.
pub fn encode(housing: &Housing) -> [u8; SLOTS] {
    housing.slots.map(|slot| match slot {
        None => EMPTY,
        Some(c) => kind_code(c.kind) * KIND_STEP + c.tier.clamp(1, MAX_TIER),
    })
}

/// The housing `encode` wrote. A byte it does not know reads as an empty slot, and missing
/// bytes too, so a damaged save loses a channel instead of the housing.
pub fn decode(bytes: &[u8]) -> Housing {
    let mut housing = Housing::default();
    for (slot, &byte) in housing.slots.iter_mut().zip(bytes) {
        let tier = byte % KIND_STEP;
        *slot = kind_of(byte / KIND_STEP)
            .filter(|_| tier >= 1)
            .map(|kind| Channel { kind, tier });
    }
    housing
}

/// M2: the housing with `channel` in its first free slot, and that slot; none when all four
/// are taken.
pub fn insert(housing: Housing, channel: Channel) -> Option<(Housing, usize)> {
    let slot = housing.slots.iter().position(Option::is_none)?;
    let mut next = housing;
    next.slots[slot] = Some(channel);
    Some((next, slot))
}

/// The wrench: the housing without its last channel (the highest slot that holds one), that
/// slot and the channel; none when the housing is empty.
pub fn remove_last(housing: Housing) -> Option<(Housing, usize, Channel)> {
    let slot = housing.slots.iter().rposition(Option::is_some)?;
    let channel = housing.slots[slot]?;
    let mut next = housing;
    next.slots[slot] = None;
    Some((next, slot, channel))
}

/// What one slot of a working housing does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotState {
    Empty,
    /// A channel and the number of housings its channel net joins (M3), this one included.
    Channel {
        kind: ChannelKind,
        tier: u8,
        net_len: usize,
    },
}

/// What a housing shows when a player looks at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnosis {
    /// N3.a: «Нет узла магистрали».
    NoNode,
    /// N3: «Дальше предела узла», `distance` blocks of pipe from the best node, which serves `limit`.
    TooFar { distance: u32, limit: u32 },
    /// The housing works: `distance` blocks of pipe from its node, which serves `limit`; its line
    /// has `line_len` housings.
    Works {
        distance: u32,
        limit: u32,
        line_len: usize,
        slots: [SlotState; SLOTS],
    },
}

/// The diagnosis of the housing at `pos`, from the topology `analyse` made of `layout`; none
/// when `layout` has no housing there.
pub fn diagnose(topology: &Topology, layout: &Layout, pos: Pos) -> Option<Diagnosis> {
    let housing = layout.housings.get(&pos)?;
    let reach = topology.reach.get(&pos).copied().unwrap_or(Reach::NoNode);
    Some(match reach {
        Reach::NoNode => Diagnosis::NoNode,
        Reach::TooFar { distance, limit } => Diagnosis::TooFar { distance, limit },
        Reach::Served {
            distance, limit, ..
        } => {
            let line_len = topology
                .line_of(pos)
                .map_or(1, |line| topology.lines[line].len());
            let slots = std::array::from_fn(|slot| match housing.slots[slot] {
                None => SlotState::Empty,
                Some(c) => SlotState::Channel {
                    kind: c.kind,
                    tier: c.tier,
                    net_len: topology
                        .net_of(pos, slot)
                        .map_or(1, |net| net.housings.len()),
                },
            });
            Diagnosis::Works {
                distance,
                limit,
                line_len,
                slots,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::topology::{Node, analyse};
    use crate::tuning::Tuning;

    fn channel(kind: ChannelKind) -> Channel {
        Channel { kind, tier: 1 }
    }

    #[test]
    fn slots_survive_a_save() {
        let housing = Housing {
            slots: [
                Some(channel(ChannelKind::Item)),
                None,
                Some(Channel {
                    kind: ChannelKind::Energy,
                    tier: 3,
                }),
                Some(channel(ChannelKind::Fluid)),
            ],
        };
        assert_eq!(decode(&encode(&housing)), housing);
    }

    #[test]
    fn a_damaged_save_reads_as_empty_slots() {
        assert_eq!(decode(&[]), Housing::default());
        // Unknown kind 9, a tier of 0, then one good item channel; the fifth byte is ignored.
        let housing = decode(&[9 * KIND_STEP + 1, KIND_STEP, KIND_STEP + 2, 0, 17]);
        assert_eq!(housing.slots[0], None);
        assert_eq!(housing.slots[1], None);
        assert_eq!(
            housing.slots[2],
            Some(Channel {
                kind: ChannelKind::Item,
                tier: 2
            })
        );
    }

    #[test]
    fn a_channel_takes_the_first_free_slot() {
        // M2: any mix of kinds, in click order.
        let mut housing = Housing::default();
        let kinds = [
            ChannelKind::Item,
            ChannelKind::Item,
            ChannelKind::Fluid,
            ChannelKind::Energy,
        ];
        for (expected, kind) in kinds.into_iter().enumerate() {
            let (next, slot) = insert(housing, channel(kind)).unwrap();
            assert_eq!(slot, expected);
            housing = next;
        }
        assert_eq!(insert(housing, channel(ChannelKind::Item)), None);
    }

    #[test]
    fn a_freed_slot_is_filled_first() {
        let housing = Housing {
            slots: [
                Some(channel(ChannelKind::Item)),
                None,
                Some(channel(ChannelKind::Fluid)),
                None,
            ],
        };
        assert_eq!(insert(housing, channel(ChannelKind::Energy)).unwrap().1, 1);
    }

    #[test]
    fn the_wrench_takes_the_last_channel() {
        let housing = Housing {
            slots: [
                Some(channel(ChannelKind::Item)),
                None,
                Some(channel(ChannelKind::Fluid)),
                None,
            ],
        };
        let (next, slot, taken) = remove_last(housing).unwrap();
        assert_eq!((slot, taken.kind), (2, ChannelKind::Fluid));
        let (next, slot, _) = remove_last(next).unwrap();
        assert_eq!(slot, 0);
        assert_eq!(remove_last(next), None);
    }

    #[test]
    fn diagnoses_follow_the_node_rules() {
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        for x in 1..=33 {
            layout.housings.insert(
                [x, 0, 0],
                Housing {
                    slots: [Some(channel(ChannelKind::Item)), None, None, None],
                },
            );
        }
        // A lone housing far away: no node on its line.
        layout.housings.insert([0, 10, 0], Housing::default());
        let topology = analyse(&layout, &Tuning::default());
        assert_eq!(
            diagnose(&topology, &layout, [33, 0, 0]),
            Some(Diagnosis::TooFar {
                distance: 33,
                limit: 32
            })
        );
        assert_eq!(
            diagnose(&topology, &layout, [0, 10, 0]),
            Some(Diagnosis::NoNode)
        );
        assert_eq!(diagnose(&topology, &layout, [5, 5, 5]), None);
        let Some(Diagnosis::Works {
            distance,
            limit,
            line_len,
            slots,
        }) = diagnose(&topology, &layout, [2, 0, 0])
        else {
            panic!("the housing two blocks from the node works");
        };
        assert_eq!((distance, limit, line_len), (2, 32, 33));
        // The item net stops at the 32nd housing: the 33rd is beyond range (N3).
        assert_eq!(
            slots[0],
            SlotState::Channel {
                kind: ChannelKind::Item,
                tier: 1,
                net_len: 32
            }
        );
        assert_eq!(slots[1], SlotState::Empty);
    }
}
