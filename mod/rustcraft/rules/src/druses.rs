//! Druses in chunks generated before the mod had them (guide `crystal`, E5: «друзы на полу
//! пещер»). The content mod's worldgen (`worldgen/placed_feature/druse.json`) places them only in
//! chunks generated from 0.18.0 on; a world older than that has none where it was explored. The
//! server gives such a chunk the same placement once, the first time it loads after the update:
//! 10 tries a chunk, x and z anywhere in it, y uniform from 8 above the bottom to 48, a scan of up
//! to 12 cells down through air to a solid block, the druse on top of it. Unlike worldgen, the
//! floor must be natural rock: an old chunk may hold a player's underground base, and a druse
//! must not grow on its floor.

use crate::geom::Pos;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Tries a chunk (`minecraft:count` 10).
pub const TRIES: u32 = 10;
/// The highest start (`height_range`, `max_inclusive` 48).
pub const MAX_Y: i32 = 48;
/// The lowest start above the world's bottom (`min_inclusive`, `above_bottom` 8).
pub const ABOVE_BOTTOM: i32 = 8;
/// Cells the scan goes down (`environment_scan`, `max_steps` 12).
pub const SCAN: i32 = 12;

/// Floors a retrofitted druse may stand on: rock as the world generates it.
const NATURAL_FLOORS: [&str; 12] = [
    "minecraft:stone",
    "minecraft:deepslate",
    "minecraft:tuff",
    "minecraft:granite",
    "minecraft:diorite",
    "minecraft:andesite",
    "minecraft:calcite",
    "minecraft:dripstone_block",
    "minecraft:smooth_basalt",
    "minecraft:gravel",
    "minecraft:dirt",
    "minecraft:clay",
];

/// What the scan finds in a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Air of any kind: the scan goes on down.
    Air,
    /// A natural floor: the druse goes on top of it.
    Floor,
    /// Anything else (a player's block, water, a plant): the try fails.
    Other,
}

/// What `block` is to the scan.
pub fn cell_of(block: &str) -> Cell {
    match block {
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => Cell::Air,
        b if NATURAL_FLOORS.contains(&b) || b.ends_with("_ore") => Cell::Floor,
        _ => Cell::Other,
    }
}

/// The starts of chunk `chunk`'s `tries` tries in a world whose bottom is `min_y`: the same for
/// the same chunk every time, so a retry after a restart picks the same cells.
pub fn starts(chunk: [i32; 2], min_y: i32, tries: u32) -> Vec<Pos> {
    let low = min_y + ABOVE_BOTTOM;
    if low > MAX_Y {
        return Vec::new();
    }
    let span = (MAX_Y - low + 1) as u64;
    let mut rng = SplitMix(seed_of(chunk));
    (0..tries)
        .map(|_| {
            let x = (rng.next() % 16) as i32;
            let z = (rng.next() % 16) as i32;
            let y = low + (rng.next() % span) as i32;
            [chunk[0] * 16 + x, y, chunk[1] * 16 + z]
        })
        .collect()
}

/// Where a try from `start` puts its druse: down through air to a natural floor, at most `SCAN`
/// cells, and the druse in the air cell above that floor. None when the start is not air, the
/// scan meets anything else, or it runs out.
pub fn place_from(start: Pos, cell: impl Fn(Pos) -> Cell) -> Option<Pos> {
    let mut pos = start;
    for _ in 0..SCAN {
        match cell(pos) {
            Cell::Floor => return (pos != start).then_some([pos[0], pos[1] + 1, pos[2]]),
            Cell::Air => pos[1] -= 1,
            Cell::Other => return None,
        }
    }
    None
}

/// The chunks each level has had its pass, as the world's file keeps them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {
    levels: BTreeMap<String, BTreeSet<(i32, i32)>>,
}

impl Done {
    /// Whether chunk `chunk` of `level` has had its pass (or was generated with druses).
    pub fn has(&self, level: &str, chunk: [i32; 2]) -> bool {
        self.levels
            .get(level)
            .is_some_and(|s| s.contains(&(chunk[0], chunk[1])))
    }

    /// Notes chunk `chunk` of `level` done; true when it was not yet.
    pub fn mark(&mut self, level: &str, chunk: [i32; 2]) -> bool {
        self.levels
            .entry(level.to_owned())
            .or_default()
            .insert((chunk[0], chunk[1]))
    }
}

fn seed_of(chunk: [i32; 2]) -> u64 {
    // Any fixed mix of the coordinates; the constant only keeps chunk (0, 0) from seeding zero.
    let x = u64::from(chunk[0] as u32);
    let z = u64::from(chunk[1] as u32);
    (x << 32 | z) ^ 0x5DEE_CE66_D1CE_D11E
}

/// SplitMix64: small, fast and good enough to scatter ten tries over a chunk.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_lie_in_the_chunk_and_the_height_range_and_repeat() {
        let a = starts([3, -2], -64, TRIES);
        assert_eq!(a.len(), TRIES as usize);
        for p in &a {
            assert!((48..64).contains(&p[0]), "{p:?}");
            assert!((-32..-16).contains(&p[2]), "{p:?}");
            assert!((-56..=MAX_Y).contains(&p[1]), "{p:?}");
        }
        assert_eq!(a, starts([3, -2], -64, TRIES));
        assert_ne!(a, starts([4, -2], -64, TRIES));
        assert!(starts([0, 0], 60, TRIES).is_empty());
    }

    #[test]
    fn a_druse_lands_on_a_natural_floor_below_air() {
        // Air from y 30 down to 21, stone at 20: a start at 25 puts the druse at 21.
        let cell = |p: Pos| if p[1] <= 20 { Cell::Floor } else { Cell::Air };
        assert_eq!(place_from([5, 25, 7], cell), Some([5, 21, 7]));
        // Starting in rock: no druse inside it.
        assert_eq!(place_from([5, 20, 7], cell), None);
        // Floor further than 12 cells down: none.
        assert_eq!(place_from([5, 40, 7], cell), None);
    }

    #[test]
    fn a_player_floor_or_water_stops_the_try() {
        assert_eq!(cell_of("minecraft:oak_planks"), Cell::Other);
        assert_eq!(cell_of("minecraft:cobblestone"), Cell::Other);
        assert_eq!(cell_of("minecraft:water"), Cell::Other);
        assert_eq!(cell_of("minecraft:deepslate_iron_ore"), Cell::Floor);
        assert_eq!(cell_of("minecraft:cave_air"), Cell::Air);
        let cell = |p: Pos| if p[1] <= 20 { Cell::Other } else { Cell::Air };
        assert_eq!(place_from([5, 25, 7], cell), None);
    }

    #[test]
    fn a_chunk_is_done_once_per_level() {
        let mut d = Done::default();
        assert!(!d.has("minecraft:overworld", [1, 2]));
        assert!(d.mark("minecraft:overworld", [1, 2]));
        assert!(!d.mark("minecraft:overworld", [1, 2]));
        assert!(d.has("minecraft:overworld", [1, 2]));
        assert!(!d.has("minecraft:the_nether", [1, 2]));
        let back: Done = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }
}
