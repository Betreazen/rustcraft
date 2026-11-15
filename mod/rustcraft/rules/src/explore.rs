//! Slice 13: the cartographer and the scout compass (spec 22, EX1; guide `tool_map`,
//! `scout_compass`). A worker with the map tool marks the chunks along its rails — the rail's
//! chunk and one on each side, two with a radius module — the biomes it rides through and the
//! structures near its way. Each chunk, biome and structure pays its owner explorer's experience
//! once (EX1: `chunk`, `biome`, `structure` reports, one time an owner; a structure counts by its
//! id, as Path-of-craft-tree counts a player's). The first place each biome was found and the
//! places of the structures found are kept for the scout compass, which names the nearest one its
//! holder has not been in yet.
//!
//! Structures: the glue asks the engine which structure starts' boxes reach a few cells round each
//! rail cell (`structure_samples`), a grid over the band at three heights, a budget of them a rail
//! cell, so successive cells go round the whole grid.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A chunk of a dimension.
pub type Chunk = (String, i32, i32);

/// The step of the grid of cells the cartographer samples for structures, in blocks.
pub const SAMPLE_STEP: i32 = 8;
/// The heights round the rail it samples.
pub const SAMPLE_DY: [i32; 3] = [-8, 0, 8];
/// Cells sampled for structures a rail cell.
pub const SAMPLE_BUDGET: usize = 16;
/// Two places of one structure id nearer than this (flat, each axis) are taken for one structure.
pub const SITE_APART: i32 = 96;
/// Structure places kept a dimension, for the compass.
pub const SITES_MAX: usize = 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Atlas {
    /// What each owner's cartographers marked (EX1: once an owner).
    pub chunks: BTreeMap<u128, BTreeSet<Chunk>>,
    pub biomes: BTreeMap<u128, BTreeSet<String>>,
    /// Where each biome was first found, by dimension: (biome, block).
    pub found: BTreeMap<String, Vec<(String, [i32; 3])>>,
    /// The biomes each player has been in.
    pub visited: BTreeMap<u128, BTreeSet<String>>,
    /// The structure ids each owner's cartographers found (EX1: once an owner). An atlas of 0.13
    /// has none.
    #[serde(default)]
    pub structures: BTreeMap<u128, BTreeSet<String>>,
    /// Where structures were found, by dimension: (id, block), one place a structure.
    #[serde(default)]
    pub sites: BTreeMap<String, Vec<(String, [i32; 3])>>,
    /// The structure ids each player has been in.
    #[serde(default)]
    pub been: BTreeMap<u128, BTreeSet<String>>,
}

/// What the compass points to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Biome(String),
    Structure(String),
}

/// The compass's target: the place, its block and how far it is (flat).
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub place: Place,
    pub at: [i32; 3],
    pub distance: f64,
}

/// The chunks of the band round `cell`: its chunk and `side` chunks each way.
pub fn band(cell: [i32; 3], side: i32) -> Vec<(i32, i32)> {
    let (cx, cz) = (cell[0] >> 4, cell[2] >> 4);
    (-side..=side).flat_map(|dx| (-side..=side).map(move |dz| (cx + dx, cz + dz))).collect()
}

/// The cells round rail `cell` asked for structures: of the grid of SAMPLE_STEP over `side`
/// chunks each way at the SAMPLE_DY heights, `budget` cells from the `turn`th on (wrapping round).
pub fn structure_samples(cell: [i32; 3], side: i32, turn: usize, budget: usize) -> Vec<[i32; 3]> {
    let reach = side.max(0) * 16;
    let steps: Vec<i32> = (-reach..=reach).step_by(SAMPLE_STEP as usize).collect();
    let grid: Vec<[i32; 3]> = steps
        .iter()
        .flat_map(|&dx| steps.iter().map(move |&dz| (dx, dz)))
        .flat_map(|(dx, dz)| SAMPLE_DY.iter().map(move |&dy| [cell[0] + dx, cell[1] + dy, cell[2] + dz]))
        .collect();
    let n = budget.min(grid.len());
    (0..n).map(|i| grid[(turn + i) % grid.len()]).collect()
}

fn flat_distance(p: [i32; 3], at: [f64; 3]) -> f64 {
    ((f64::from(p[0]) - at[0]).powi(2) + (f64::from(p[2]) - at[2]).powi(2)).sqrt()
}

impl Atlas {
    /// Marks a chunk for `owner`: true when it is new to them.
    pub fn mark_chunk(&mut self, owner: u128, chunk: Chunk) -> bool {
        self.chunks.entry(owner).or_default().insert(chunk)
    }

    /// Marks a biome found at `at` for `owner`: true when it is new to them. The first place a
    /// biome is found anywhere is kept for the compass.
    pub fn mark_biome(&mut self, owner: u128, dimension: &str, biome: &str, at: [i32; 3]) -> bool {
        let list = self.found.entry(dimension.to_owned()).or_default();
        if !list.iter().any(|(b, _)| b == biome) {
            list.push((biome.to_owned(), at));
        }
        self.biomes.entry(owner).or_default().insert(biome.to_owned())
    }

    /// Marks structure `id` found at `at` for `owner`: true when the id is new to them. The place
    /// is kept for the compass unless one of the same id lies within SITE_APART (the same
    /// structure) or the dimension keeps SITES_MAX already.
    pub fn mark_structure(&mut self, owner: u128, dimension: &str, id: &str, at: [i32; 3]) -> bool {
        let list = self.sites.entry(dimension.to_owned()).or_default();
        let near = |p: &[i32; 3]| (p[0] - at[0]).abs() < SITE_APART && (p[2] - at[2]).abs() < SITE_APART;
        if list.len() < SITES_MAX && !list.iter().any(|(s, p)| s == id && near(p)) {
            list.push((id.to_owned(), at));
        }
        self.structures.entry(owner).or_default().insert(id.to_owned())
    }

    pub fn visit(&mut self, player: u128, biome: &str) {
        self.visited.entry(player).or_default().insert(biome.to_owned());
    }

    /// `player` is in structure `id`: true when that is new.
    pub fn enter(&mut self, player: u128, id: &str) -> bool {
        self.been.entry(player).or_default().insert(id.to_owned())
    }

    /// The scout compass's target for `player` at `at`: the nearest found biome or structure of
    /// the dimension they have not been in yet.
    pub fn target(&self, player: u128, dimension: &str, at: [f64; 3]) -> Option<Target> {
        let biomes = self.visited.get(&player);
        let structures = self.been.get(&player);
        let new_biomes = self
            .found
            .get(dimension)
            .into_iter()
            .flatten()
            .filter(|(b, _)| biomes.is_none_or(|s| !s.contains(b)))
            .map(|(b, p)| (Place::Biome(b.clone()), *p));
        let new_structures = self
            .sites
            .get(dimension)
            .into_iter()
            .flatten()
            .filter(|(id, _)| structures.is_none_or(|s| !s.contains(id)))
            .map(|(id, p)| (Place::Structure(id.clone()), *p));
        new_biomes
            .chain(new_structures)
            .map(|(place, p)| Target { place, at: p, distance: flat_distance(p, at) })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
}

/// The compass's word for the way from `at` to `to`: one of eight directions, `north` being -z.
pub fn direction(at: [f64; 3], to: [i32; 3]) -> &'static str {
    let (dx, dz) = (f64::from(to[0]) + 0.5 - at[0], f64::from(to[2]) + 0.5 - at[2]);
    let angle = dx.atan2(-dz).to_degrees().rem_euclid(360.0);
    const WAYS: [&str; 8] = ["north", "north_east", "east", "south_east", "south", "south_west", "west", "north_west"];
    WAYS[((angle + 22.5) / 45.0) as usize % 8]
}

/// A structure id as words, for a client without the content mod's name for it:
/// `minecraft:village_plains` → `village plains`.
pub fn structure_words(id: &str) -> String {
    id.rsplit(':').next().unwrap_or(id).replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVER: &str = "minecraft:overworld";

    #[test]
    fn each_place_pays_once_an_owner() {
        // EX1.
        let mut a = Atlas::default();
        let c = (OVER.to_owned(), 1, 2);
        assert!(a.mark_chunk(7, c.clone()));
        assert!(!a.mark_chunk(7, c.clone()));
        assert!(a.mark_chunk(8, c), "another owner's cartographer");
        assert!(a.mark_biome(7, OVER, "minecraft:desert", [100, 64, 0]));
        assert!(!a.mark_biome(7, OVER, "minecraft:desert", [200, 64, 0]));
        assert_eq!(a.found[OVER], vec![("minecraft:desert".to_owned(), [100, 64, 0])]);
        assert_eq!(band([17, 64, -1], 1).len(), 9);
        assert!(band([17, 64, -1], 1).contains(&(1, -1)));
    }

    #[test]
    fn a_structure_pays_once_an_owner_by_its_id() {
        // EX1: Path-of-craft-tree counts `structure:<id>` once an owner, as for a player.
        let mut a = Atlas::default();
        assert!(a.mark_structure(7, OVER, "minecraft:village_plains", [100, 70, 0]));
        assert!(!a.mark_structure(7, OVER, "minecraft:village_plains", [110, 70, 5]), "the same village");
        assert!(!a.mark_structure(7, OVER, "minecraft:village_plains", [2000, 70, 0]), "another village");
        assert!(a.mark_structure(8, OVER, "minecraft:village_plains", [100, 70, 0]), "another owner");
        assert!(a.mark_structure(7, OVER, "minecraft:desert_pyramid", [300, 64, 0]));
        let sites = &a.sites[OVER];
        assert_eq!(sites.len(), 3, "{sites:?}");
        assert!(sites.contains(&("minecraft:village_plains".to_owned(), [2000, 70, 0])));
    }

    #[test]
    fn the_structure_places_are_bounded() {
        let mut a = Atlas::default();
        for i in 0..(SITES_MAX as i32 + 10) {
            a.mark_structure(1, OVER, "minecraft:ruined_portal", [i * SITE_APART * 2, 64, 0]);
        }
        assert_eq!(a.sites[OVER].len(), SITES_MAX);
    }

    #[test]
    fn an_atlas_of_slice_13_reads_without_structures() {
        let old = r#"{"chunks":{},"biomes":{"7":["minecraft:desert"]},"found":{},"visited":{}}"#;
        let a: Atlas = serde_json::from_str(old).unwrap();
        assert!(a.structures.is_empty() && a.sites.is_empty() && a.been.is_empty());
        assert!(a.biomes[&7].contains("minecraft:desert"));
        let back: Atlas = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn the_compass_points_to_the_nearest_unvisited_biome() {
        let mut a = Atlas::default();
        a.mark_biome(1, OVER, "minecraft:desert", [100, 64, 0]);
        a.mark_biome(1, OVER, "minecraft:jungle", [0, 64, -30]);
        let t = a.target(5, OVER, [0.0, 64.0, 0.0]).unwrap();
        assert_eq!((t.place, t.distance.round()), (Place::Biome("minecraft:jungle".into()), 30.0));
        a.visit(5, "minecraft:jungle");
        assert_eq!(a.target(5, OVER, [0.0, 64.0, 0.0]).unwrap().place, Place::Biome("minecraft:desert".into()));
        assert_eq!(direction([0.0, 64.0, 0.0], [0, 64, -30]), "north");
        assert_eq!(direction([0.0, 64.0, 0.0], [100, 64, 0]), "east");
    }

    #[test]
    fn the_compass_points_to_a_nearer_structure_the_player_was_not_in() {
        // Spec 22: «указывает игроку на ближайшую найденную картографом постройку или биом, где
        // игрок ещё не был».
        let mut a = Atlas::default();
        a.mark_biome(1, OVER, "minecraft:desert", [100, 64, 0]);
        a.mark_structure(1, OVER, "minecraft:desert_pyramid", [40, 64, 0]);
        a.mark_structure(1, "minecraft:the_nether", "minecraft:fortress", [1, 64, 0]);
        let t = a.target(5, OVER, [0.0, 64.0, 0.0]).unwrap();
        assert_eq!((t.place, t.at), (Place::Structure("minecraft:desert_pyramid".into()), [40, 64, 0]));
        assert!(a.enter(5, "minecraft:desert_pyramid"));
        assert!(!a.enter(5, "minecraft:desert_pyramid"));
        assert_eq!(a.target(5, OVER, [0.0, 64.0, 0.0]).unwrap().place, Place::Biome("minecraft:desert".into()));
        a.visit(5, "minecraft:desert");
        assert_eq!(a.target(5, OVER, [0.0, 64.0, 0.0]), None);
        assert_eq!(structure_words("minecraft:village_plains"), "village plains");
    }

    #[test]
    fn the_samples_go_round_the_band_within_the_budget() {
        let cell = [100, 64, 100];
        let one = structure_samples(cell, 1, 0, SAMPLE_BUDGET);
        assert_eq!(one.len(), SAMPLE_BUDGET);
        // Side 1: offsets -16..=16 by 8 (5 × 5 columns) at three heights.
        let all: BTreeSet<[i32; 3]> =
            (0..10).flat_map(|k| structure_samples(cell, 1, k * SAMPLE_BUDGET, SAMPLE_BUDGET)).collect();
        assert_eq!(all.len(), 5 * 5 * SAMPLE_DY.len());
        assert!(all.contains(&[84, 56, 116]) && all.contains(&[100, 64, 100]));
        assert!(all.iter().all(|p| (p[0] - 100).abs() <= 16 && (p[2] - 100).abs() <= 16));
        assert_eq!(structure_samples(cell, 0, 0, 100).len(), SAMPLE_DY.len(), "side 0: the cell's column");
    }
}
