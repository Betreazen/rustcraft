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

/// What a click with `channel` does to a housing (M2, U4): a channel of a higher tier takes the
/// place of the first channel of its kind with a lower tier, which comes back (the slot keeps its
/// joint settings, U3); otherwise it goes into the first free slot (M2). The housing after it,
/// the slot, and the channel that came out; none when all four slots are taken by others.
pub fn put(housing: Housing, channel: Channel) -> Option<(Housing, usize, Option<Channel>)> {
    let lower = housing
        .slots
        .iter()
        .position(|s| s.is_some_and(|c| c.kind == channel.kind && c.tier < channel.tier));
    if let Some(slot) = lower {
        let mut next = housing;
        let old = next.slots[slot].replace(channel);
        return Some((next, slot, old));
    }
    insert(housing, channel).map(|(h, slot)| (h, slot, None))
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

// ---- Slice 18: the facade's whole block (F4) ----

/// The chorus plant's side properties, in the order `covered_shapes` writes a state's values.
pub const SIDES: [&str; 6] = ["north", "east", "south", "west", "up", "down"];
/// The housing's state property that says a facade hides it (guide `facade`, F4).
pub const COVERED: &str = "covered";

/// A box in blocks: min x, y, z, max x, y, z.
pub type Box6 = [f64; 6];

/// The boxes `PipeBlock` builds for a pipe `thickness` pixels across with the sides `joined`
/// (in `SIDES` order): a core and an arm to each joined side.
pub fn pipe_boxes(thickness: f64, joined: [bool; 6]) -> Vec<Box6> {
    let lo = (16.0 - thickness) / 32.0;
    let hi = 1.0 - lo;
    let mut out = vec![[lo, lo, lo, hi, hi, hi]];
    for (i, side) in SIDES.iter().enumerate() {
        if !joined[i] {
            continue;
        }
        out.push(match *side {
            "north" => [lo, lo, 0.0, hi, hi, lo],
            "south" => [lo, lo, hi, hi, hi, 1.0],
            "west" => [0.0, lo, lo, lo, hi, hi],
            "east" => [hi, lo, lo, 1.0, hi, hi],
            "up" => [lo, hi, lo, hi, 1.0, hi],
            _ => [lo, 0.0, lo, hi, lo, hi],
        });
    }
    out
}

/// F4: the housing's shapes by state, keyed as the content mod's `shapes` table keys them
/// (`covered` then `SIDES`, values joined by commas): a hidden housing is a whole block, an open
/// one the pipe `thickness` pixels across.
pub fn covered_shapes(thickness: f64) -> Vec<(String, Vec<Box6>)> {
    let mut out = Vec::new();
    for covered in [false, true] {
        for bits in 0..64u8 {
            let joined: [bool; 6] = std::array::from_fn(|i| bits & (1 << i) != 0);
            let mut key = covered.to_string();
            for j in joined {
                key.push(',');
                key.push_str(if j { "true" } else { "false" });
            }
            let boxes = if covered {
                vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]
            } else {
                pipe_boxes(thickness, joined)
            };
            out.push((key, boxes));
        }
    }
    out
}

// ---- Slice 18: what gives a facade its look (F2) ----

/// Parts of a block id that name a block that is not whole: a slab, stairs, a torch, a fence and
/// the like. The engine gives a mod no item's shape before it is placed (only
/// `native_block_collision` of a placed block, SRC native_hooks.rs:11143), so the name decides.
const NOT_WHOLE: [&str; 40] = [
    "slab", "stairs", "torch", "fence", "wall", "door", "pane", "carpet", "button",
    "pressure_plate", "rail", "sign", "banner", "flower", "sapling", "ladder", "lever",
    "lantern", "chain", "bars", "candle", "pot", "head", "skull", "bed", "cake",
    "anvil", "bell", "trapdoor", "vine", "grass", "fern", "bush", "coral", "mushroom",
    "amethyst_bud", "cluster", "snow", "dripleaf", "rod",
];

/// F2: whether a click with the block `block` gives a facade its look: a whole block only.
pub fn whole_block(block: &str) -> bool {
    let name = block.rsplit(':').next().unwrap_or(block);
    let wall_block = name.ends_with("_wall") || name == "wall";
    let exceptions = ["mushroom_block", "snow_block", "chain_command_block", "stem"];
    if exceptions.iter().any(|e| name.ends_with(e)) {
        return true;
    }
    !wall_block && !NOT_WHOLE.iter().any(|p| name.contains(p))
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
    fn a_higher_tier_channel_takes_the_place_of_a_lower_one() {
        // U4: a tier-2 item channel replaces the tier-1 one in its slot; the old one comes back.
        let h = Housing {
            slots: [
                Some(channel(ChannelKind::Energy)),
                Some(channel(ChannelKind::Item)),
                None,
                None,
            ],
        };
        let two = Channel {
            kind: ChannelKind::Item,
            tier: 2,
        };
        let (h, slot, old) = put(h, two).unwrap();
        assert_eq!((slot, old), (1, Some(channel(ChannelKind::Item))));
        assert_eq!(h.slots[1], Some(two));
        // The same tier again, or a lower one, goes into a free slot (M2).
        let (h, slot, old) = put(h, two).unwrap();
        assert_eq!((slot, old), (2, None));
        let (h, slot, old) = put(h, channel(ChannelKind::Item)).unwrap();
        assert_eq!((slot, old), (3, None));
        // All four taken, nothing lower of its kind: no room.
        assert_eq!(put(h, channel(ChannelKind::Fluid)), None);
        let three = Channel {
            kind: ChannelKind::Energy,
            tier: 3,
        };
        let (_, slot, old) = put(h, three).unwrap();
        assert_eq!((slot, old), (0, Some(channel(ChannelKind::Energy))));
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

    #[test]
    fn an_open_housing_is_a_core_and_its_arms() {
        let boxes = pipe_boxes(6.5, [true, false, false, false, false, true]);
        assert_eq!(boxes.len(), 3);
        let lo = 4.75 / 16.0;
        assert_eq!(boxes[0], [lo, lo, lo, 1.0 - lo, 1.0 - lo, 1.0 - lo]);
        assert_eq!(boxes[1][2], 0.0, "north arm reaches z 0");
        assert_eq!(boxes[2][1], 0.0, "down arm reaches y 0");
    }

    #[test]
    fn a_hidden_housing_is_a_whole_block_in_every_state() {
        // F4: «скрытый корпус держит форму целого блока».
        let shapes = covered_shapes(6.5);
        assert_eq!(shapes.len(), 128);
        let keys: std::collections::BTreeSet<_> = shapes.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(keys.len(), 128);
        for (key, boxes) in &shapes {
            assert_eq!(key.split(',').count(), 7);
            if key.starts_with("true") {
                assert_eq!(boxes, &vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]);
            } else {
                assert!(boxes.len() >= 1 && boxes[0][0] > 0.0);
            }
        }
    }

    #[test]
    fn only_a_whole_block_gives_a_facade_its_look() {
        for b in ["minecraft:bricks", "minecraft:oak_planks", "minecraft:stone", "minecraft:red_mushroom_block", "minecraft:snow_block", "minecraft:mossy_stone_bricks"] {
            assert!(whole_block(b), "{b}");
        }
        for b in ["minecraft:oak_slab", "minecraft:torch", "minecraft:stone_brick_stairs", "minecraft:cobblestone_wall", "minecraft:oak_fence", "minecraft:glass_pane"] {
            assert!(!whole_block(b), "{b}");
        }
    }
}
