//! What a housing shows when a player looks at it (N3, which replaces M9). Game-free: the bundle
//! turns a `Diagnosis` into text. Round 31: the three channels are built into every housing (see
//! `net::topology`); a housing saved with channels set by hand is read by `net::migrate`.

use crate::geom::Pos;
use crate::net::topology::{ChannelKind, Layout, Reach, Topology};
use crate::tiers::{self, Family};

/// The housing block.
pub const BLOCK: &str = "rustcraft:housing";
/// Round 31: the housing in tiers 1–4, as the machines (U1): `rustcraft:housing` and
/// `rustcraft:housing_2..4`, each the same block with its `tier`. Not in `tiers::FAMILIES`: its
/// placement and replacement are its own (`rustcraft_pipe18`), since a housing placed must keep the
/// sides the game joins it by.
pub const TIERS: Family = Family::new(BLOCK, 1);

/// The item of a housing of `tier` (brought into 1–4).
pub fn item_of_tier(tier: u8) -> String {
    let tier = tier.clamp(TIERS.first, TIERS.last);
    TIERS.item(tier).unwrap_or_else(|| BLOCK.to_owned())
}

/// U2 for housings: a housing of tier `held` clicked on one of tier `placed` takes its place only
/// when it is higher; the answer is the tier that comes back to the hand.
pub fn swap(placed: u8, held: u8) -> Option<u8> {
    tiers::upgrade(placed, held).then_some(placed)
}

/// What a housing shows when a player looks at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnosis {
    /// N3.a: «Нет узла магистрали».
    NoNode,
    /// N3: «Дальше предела узла», `distance` blocks of pipe from the best node, which serves `limit`.
    TooFar { distance: u32, limit: u32 },
    /// The housing works: `distance` blocks of pipe from its node, which serves `limit`; its line
    /// has `line_len` housings. It is of `tier`; its channels' nets join `net_len` housings and
    /// carry at `net_tier`, the tier of their weakest housing (M10).
    Works {
        distance: u32,
        limit: u32,
        line_len: usize,
        tier: u8,
        net_len: usize,
        net_tier: u8,
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
            // Every channel of a housing is on a net of the same housings (round 31).
            let net = topology.net_of_kind(pos, ChannelKind::Item);
            Diagnosis::Works {
                distance,
                limit,
                line_len,
                tier: housing.tier,
                net_len: net.map_or(1, |n| n.housings.len()),
                net_tier: net.map_or(housing.tier, |n| n.min_tier),
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
    use crate::net::topology::{Housing, Node, analyse};
    use crate::tuning::Tuning;

    #[test]
    fn diagnoses_follow_the_node_rules() {
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        for x in 1..=33 {
            // Tier 2 but for the tenth housing: it holds the net back.
            let tier = if x == 10 { 1 } else { 2 };
            layout.housings.insert([x, 0, 0], Housing::new(tier));
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
        // The net stops at the 32nd housing: the 33rd is beyond range (N3).
        assert_eq!(
            diagnose(&topology, &layout, [2, 0, 0]),
            Some(Diagnosis::Works {
                distance: 2,
                limit: 32,
                line_len: 33,
                tier: 2,
                net_len: 32,
                net_tier: 1,
            })
        );
    }

    #[test]
    fn each_housing_item_places_its_own_tier() {
        assert_eq!(TIERS.tier_of("rustcraft:housing"), Some(1));
        assert_eq!(TIERS.tier_of("rustcraft:housing_4"), Some(4));
        assert_eq!(TIERS.tier_of("rustcraft:node_2"), None);
        assert!(tiers::family_of_block(BLOCK).is_none());
        for tier in 1..=4 {
            assert_eq!(TIERS.tier_of(&item_of_tier(tier)), Some(tier));
        }
        assert_eq!(item_of_tier(0), "rustcraft:housing");
        assert_eq!(item_of_tier(9), "rustcraft:housing_4");
        assert_eq!((swap(1, 3), swap(2, 2), swap(3, 2)), (Some(1), None, None));
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
