//! Round 31: the connectors («присоски») of the machines. A machine a housing of the magistral
//! touches shows a connector on that side: a bolted socket round the end of the housing's arm,
//! the part `link_<side>` of its model (assets/models/kit.py `links`). The block keeps one boolean
//! state property per side, [`FACES`], and its blockstate shows `<model>__link_<side>` for each
//! side that is `true`; the server sets them from the housings around it
//! (rustcraft_connectors).
//!
//! Five sides in the state, not six: a sixth boolean would double the states again. Every machine
//! copies the furnace (`facing` 4 × `lit` 2), many have a `tier` (4); 32 combinations of five
//! sides keep the registry under 2^16 states, where chunks keep vanilla's 16-bit palette
//! (SRC/docs/modpacks.md, «Limits»), and 64 would not ([`tests`]).
//!
//! Round 32: the sixth, the bottom, goes through the block entity instead. The server writes
//! whether a housing stands right below into the block's entity ([`BOTTOM_KEY`]) and tells
//! clients when that changes; the client draws [`bottom_model`] (`<model>__link_down`, part
//! `link_z-` of kit.py `links`) at the block's place, unturned, and keeps the drawing until the
//! entity changes. Every block of [`BLOCKS`] has a block entity.
//!
//! Housings only: a node is pipe and joins no machine (net::joints), so it shows no connector,
//! as the crystal's, the panel's and the windmill's connectors of slice 18 (rustcraft_power).

use crate::geom::{Face, Pos, step};

/// The sides a machine shows a connector on: the state property and the face it names.
pub const FACES: [(&str, Face); 5] = [
    ("north", Face::ZNeg),
    ("east", Face::XPos),
    ("south", Face::ZPos),
    ("west", Face::XNeg),
    ("up", Face::YPos),
];

/// The blocks with connectors: every machine a housing joins on any of several sides, the tank,
/// the storage's cell and its terminal.
pub const BLOCKS: [&str; 40] = [
    "rustcraft:alloy",
    "rustcraft:anchor",
    "rustcraft:apiary",
    "rustcraft:archeo",
    "rustcraft:battery",
    "rustcraft:breaker",
    "rustcraft:brewer",
    "rustcraft:builder",
    "rustcraft:butcher",
    "rustcraft:cell",
    "rustcraft:charger",
    "rustcraft:crafter",
    "rustcraft:crusher",
    "rustcraft:disintegrator",
    "rustcraft:dock_charge",
    "rustcraft:dock_load",
    "rustcraft:dock_unload",
    "rustcraft:enchanter",
    "rustcraft:feeder",
    "rustcraft:fishery",
    "rustcraft:furnace",
    "rustcraft:gen_fuel",
    "rustcraft:gen_lava",
    "rustcraft:kitchen",
    "rustcraft:miner",
    "rustcraft:mixer",
    "rustcraft:placer",
    "rustcraft:portal_dock",
    "rustcraft:projector",
    "rustcraft:pump",
    "rustcraft:repair",
    "rustcraft:shearer",
    "rustcraft:station",
    "rustcraft:tank",
    "rustcraft:terminal",
    "rustcraft:trash",
    "rustcraft:turbine",
    "rustcraft:turret",
    "rustcraft:vein",
    "rustcraft:xp_tank",
];

/// The bottom side, drawn by the block entity: the face, and the key of the block entity's
/// compound that holds 1 while a housing stands below.
pub const BOTTOM: Face = Face::YNeg;
pub const BOTTOM_KEY: &str = "link_dn";

/// The bottom connector's model of each of [`BLOCKS`], in its order: the model the blockstate
/// names for the other sides, `__link_down` (the portal dock's model is `pdock`).
pub const BOTTOM_MODELS: [&str; 40] = [
    "rustcraft:block/alloy__link_down",
    "rustcraft:block/anchor__link_down",
    "rustcraft:block/apiary__link_down",
    "rustcraft:block/archeo__link_down",
    "rustcraft:block/battery__link_down",
    "rustcraft:block/breaker__link_down",
    "rustcraft:block/brewer__link_down",
    "rustcraft:block/builder__link_down",
    "rustcraft:block/butcher__link_down",
    "rustcraft:block/cell__link_down",
    "rustcraft:block/charger__link_down",
    "rustcraft:block/crafter__link_down",
    "rustcraft:block/crusher__link_down",
    "rustcraft:block/disintegrator__link_down",
    "rustcraft:block/dock_charge__link_down",
    "rustcraft:block/dock_load__link_down",
    "rustcraft:block/dock_unload__link_down",
    "rustcraft:block/enchanter__link_down",
    "rustcraft:block/feeder__link_down",
    "rustcraft:block/fishery__link_down",
    "rustcraft:block/furnace__link_down",
    "rustcraft:block/gen_fuel__link_down",
    "rustcraft:block/gen_lava__link_down",
    "rustcraft:block/kitchen__link_down",
    "rustcraft:block/miner__link_down",
    "rustcraft:block/mixer__link_down",
    "rustcraft:block/placer__link_down",
    "rustcraft:block/pdock__link_down",
    "rustcraft:block/projector__link_down",
    "rustcraft:block/pump__link_down",
    "rustcraft:block/repair__link_down",
    "rustcraft:block/shearer__link_down",
    "rustcraft:block/station__link_down",
    "rustcraft:block/tank__link_down",
    "rustcraft:block/terminal__link_down",
    "rustcraft:block/trash__link_down",
    "rustcraft:block/turbine__link_down",
    "rustcraft:block/turret__link_down",
    "rustcraft:block/vein__link_down",
    "rustcraft:block/xp_tank__link_down",
];

/// The model of `block`'s bottom connector, none for a block without connectors.
pub fn bottom_model(block: &str) -> Option<&'static str> {
    BLOCKS.iter().position(|b| *b == block).map(|i| BOTTOM_MODELS[i])
}

/// The cell under a block at `pos`: its bottom shows a connector when a housing stands there.
pub fn below(pos: Pos) -> Pos {
    step(pos, BOTTOM)
}

/// The blocks that join the magistral but take no connector of this kind, and why: they meet a
/// housing on one side only or already draw their own (slice 18).
pub const WITHOUT: [(&str, &str); 4] = [
    ("rustcraft:solar", "a thin panel on a post; its block entity draws the slice-18 connectors"),
    ("rustcraft:windmill", "a mast on a small base; its block entity draws the slice-18 connectors"),
    ("rustcraft:crystal", "its block entity draws it and its slice-18 connectors"),
    ("rustcraft:charge_plate", "a plate laid over a housing: it meets the magistral from below only"),
];

/// Whether `block` shows connectors through its state.
pub fn has(block: &str) -> bool {
    BLOCKS.contains(&block)
}

/// Each of [`FACES`] of a block at `pos` with the cell beside it on that side: the side shows a
/// connector when a housing stands there.
pub fn sides(pos: Pos) -> [(&'static str, Pos); 5] {
    FACES.map(|(name, face)| (name, step(pos, face)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/assets/rustcraft");
    /// Block states of vanilla 26.3 (SRC/docs/modpacks.md, «Limits»).
    const VANILLA: usize = 35_723;
    /// The rest of RustCraft (housing with its tiers, node, monorail's 480, the sources), with
    /// room for this slice's other streams.
    const RUSTCRAFT_REST: usize = 2_500;
    /// The other mods of the pack: SoulsCraft declares 378, Path-of-craft-tree and LetsCam none.
    const OTHER_MODS: usize = 1_000;

    fn blockstate(block: &str) -> Option<Value> {
        let name = block.strip_prefix("rustcraft:")?;
        let text = std::fs::read_to_string(format!("{ASSETS}/blockstates/{name}.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// The sides whose connector model the blockstate shows when the side's property is true.
    fn linked_sides(state: &Value) -> Vec<String> {
        let parts = state["multipart"].as_array().cloned().unwrap_or_default();
        FACES
            .iter()
            .filter(|(side, _)| {
                parts.iter().any(|p| {
                    p["when"][*side] == "true"
                        && p["apply"]["model"].as_str().is_some_and(|m| m.ends_with(&format!("__link_{side}")))
                })
            })
            .map(|(side, _)| (*side).to_owned())
            .collect()
    }

    #[test]
    fn every_block_with_connectors_shows_one_on_each_side_and_no_other_does() {
        for block in BLOCKS {
            let state = blockstate(block).unwrap_or_else(|| panic!("no blockstate for {block}"));
            assert_eq!(linked_sides(&state).len(), FACES.len(), "{block} lacks a connector side");
        }
        for entry in std::fs::read_dir(format!("{ASSETS}/blockstates")).unwrap().filter_map(|e| e.ok()) {
            let id = format!("rustcraft:{}", entry.file_name().to_string_lossy().trim_end_matches(".json"));
            let state: Value = serde_json::from_str(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
            if !has(&id) && id != "rustcraft:housing" {
                assert!(linked_sides(&state).is_empty(), "{id} shows connectors through its state");
            }
        }
    }

    #[test]
    fn the_connector_models_exist() {
        for block in BLOCKS {
            let state = blockstate(block).unwrap();
            for part in state["multipart"].as_array().unwrap() {
                let model = part["apply"]["model"].as_str().unwrap();
                let path = model.strip_prefix("rustcraft:block/").unwrap();
                assert!(
                    std::path::Path::new(&format!("{ASSETS}/models/block/{path}.json")).exists(),
                    "{block} names {model}, which is not exported"
                );
            }
        }
    }

    #[test]
    fn the_lists_do_not_overlap() {
        for (block, _) in WITHOUT {
            assert!(!has(block), "{block} is on both lists");
        }
    }

    /// The states the connected blocks take: the furnace's `facing` and `lit` (the cell copies the
    /// iron block: one state), their `tier` if they have one, times 2 for each side.
    fn connected_states() -> usize {
        BLOCKS
            .iter()
            .map(|block| {
                let template = if *block == "rustcraft:cell" { 1 } else { 8 };
                let tiers = if crate::tiers::family_of_block(block).is_some() { 4 } else { 1 };
                template * tiers << FACES.len()
            })
            .sum()
    }

    #[test]
    fn the_registry_keeps_the_16_bit_palette() {
        let total = VANILLA + connected_states() + RUSTCRAFT_REST + OTHER_MODS;
        assert!(total <= 1 << 16, "{total} block states: chunks would need a 17-bit palette");
        // A sixth side would not fit: the reason the bottom's connector is the block entity's.
        assert!(VANILLA + 2 * connected_states() > 1 << 16);
    }

    #[test]
    fn a_housing_beside_sets_its_side() {
        let housings = [[1, 0, 0], [0, 1, 0], [0, -1, 0]];
        let got = sides([0, 0, 0]).map(|(side, next)| (side, housings.contains(&next)));
        assert_eq!(
            got,
            [("north", false), ("east", true), ("south", false), ("west", false), ("up", true)],
            "east and up; the housing below is the block entity's"
        );
        assert_eq!(below([0, 0, 0]), [0, -1, 0]);
    }

    /// Each bottom model is the blockstate's connector model of the same block, `down` for
    /// `north`, and is exported; no block shows it through its state.
    #[test]
    fn every_block_with_connectors_has_its_bottom_model() {
        for (block, model) in BLOCKS.iter().zip(BOTTOM_MODELS) {
            assert_eq!(bottom_model(block), Some(model));
            let state = blockstate(block).unwrap();
            let parts = state["multipart"].as_array().unwrap();
            let north = parts
                .iter()
                .find(|p| p["when"]["north"] == "true")
                .and_then(|p| p["apply"]["model"].as_str())
                .unwrap_or_else(|| panic!("{block} has no north connector"));
            assert_eq!(north.replace("__link_north", "__link_down"), model, "{block}");
            let path = model.strip_prefix("rustcraft:block/").unwrap();
            assert!(
                std::path::Path::new(&format!("{ASSETS}/models/block/{path}.json")).exists(),
                "{model} is not exported"
            );
            assert!(parts.iter().all(|p| p["apply"]["model"] != model), "{block} shows {model} through its state");
        }
        for (block, _) in WITHOUT {
            assert_eq!(bottom_model(block), None);
        }
    }
}
