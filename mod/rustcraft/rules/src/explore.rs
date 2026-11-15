//! Slice 13: the cartographer and the scout compass (spec 22, EX1; guide `tool_map`,
//! `scout_compass`). A worker with the map tool marks the chunks along its rails — the rail's
//! chunk and one on each side, two with a radius module — and the biomes it rides through. Each
//! chunk and biome pays its owner explorer's experience once (EX1: `chunk`, `biome` reports, one
//! time an owner). The first place each biome was found is kept for the scout compass, which
//! names the nearest one its holder has not been in yet. Structures are not found: the engine
//! gives a mod no structure lookup.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A chunk of a dimension.
pub type Chunk = (String, i32, i32);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Atlas {
    /// What each owner's cartographers marked (EX1: once an owner).
    pub chunks: BTreeMap<u128, BTreeSet<Chunk>>,
    pub biomes: BTreeMap<u128, BTreeSet<String>>,
    /// Where each biome was first found, by dimension: (biome, block).
    pub found: BTreeMap<String, Vec<(String, [i32; 3])>>,
    /// The biomes each player has been in.
    pub visited: BTreeMap<u128, BTreeSet<String>>,
}

/// The chunks of the band round `cell`: its chunk and `side` chunks each way.
pub fn band(cell: [i32; 3], side: i32) -> Vec<(i32, i32)> {
    let (cx, cz) = (cell[0] >> 4, cell[2] >> 4);
    (-side..=side).flat_map(|dx| (-side..=side).map(move |dz| (cx + dx, cz + dz))).collect()
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

    pub fn visit(&mut self, player: u128, biome: &str) {
        self.visited.entry(player).or_default().insert(biome.to_owned());
    }

    /// The scout compass's target for `player` at `at`: the nearest found biome of the dimension
    /// they have not been in yet.
    pub fn target(&self, player: u128, dimension: &str, at: [f64; 3]) -> Option<(String, [i32; 3], f64)> {
        let seen = self.visited.get(&player);
        self.found
            .get(dimension)?
            .iter()
            .filter(|(b, _)| seen.is_none_or(|s| !s.contains(b)))
            .map(|(b, p)| {
                let d = ((f64::from(p[0]) - at[0]).powi(2) + (f64::from(p[2]) - at[2]).powi(2)).sqrt();
                (b.clone(), *p, d)
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
    }
}

/// The compass's word for the way from `at` to `to`: one of eight directions, `north` being -z.
pub fn direction(at: [f64; 3], to: [i32; 3]) -> &'static str {
    let (dx, dz) = (f64::from(to[0]) + 0.5 - at[0], f64::from(to[2]) + 0.5 - at[2]);
    let angle = dx.atan2(-dz).to_degrees().rem_euclid(360.0);
    const WAYS: [&str; 8] = ["north", "north_east", "east", "south_east", "south", "south_west", "west", "north_west"];
    WAYS[((angle + 22.5) / 45.0) as usize % 8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_place_pays_once_an_owner() {
        // EX1.
        let mut a = Atlas::default();
        let c = ("minecraft:overworld".to_owned(), 1, 2);
        assert!(a.mark_chunk(7, c.clone()));
        assert!(!a.mark_chunk(7, c.clone()));
        assert!(a.mark_chunk(8, c), "another owner's cartographer");
        assert!(a.mark_biome(7, "minecraft:overworld", "minecraft:desert", [100, 64, 0]));
        assert!(!a.mark_biome(7, "minecraft:overworld", "minecraft:desert", [200, 64, 0]));
        assert_eq!(a.found["minecraft:overworld"], vec![("minecraft:desert".to_owned(), [100, 64, 0])]);
        assert_eq!(band([17, 64, -1], 1).len(), 9);
        assert!(band([17, 64, -1], 1).contains(&(1, -1)));
    }

    #[test]
    fn the_compass_points_to_the_nearest_unvisited_biome() {
        let mut a = Atlas::default();
        a.mark_biome(1, "minecraft:overworld", "minecraft:desert", [100, 64, 0]);
        a.mark_biome(1, "minecraft:overworld", "minecraft:jungle", [0, 64, -30]);
        let (b, _, d) = a.target(5, "minecraft:overworld", [0.0, 64.0, 0.0]).unwrap();
        assert_eq!((b.as_str(), d.round()), ("minecraft:jungle", 30.0));
        a.visit(5, "minecraft:jungle");
        assert_eq!(a.target(5, "minecraft:overworld", [0.0, 64.0, 0.0]).unwrap().0, "minecraft:desert");
        assert_eq!(direction([0.0, 64.0, 0.0], [0, 64, -30]), "north");
        assert_eq!(direction([0.0, 64.0, 0.0], [100, 64, 0]), "east");
    }
}
