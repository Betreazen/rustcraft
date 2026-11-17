//! Slice 18: the scout compass's two targets (guide `scout_compass`: «Правый клик переключает
//! цель: постройки или биомы»). Each player's compass looks for one kind at a time; the atlas and
//! what each player has been in are rustcraft_rules::explore's.

use crate::explore::{Atlas, Place, Target};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Structures the cartographers found.
    #[default]
    Structures,
    Biomes,
}

impl Mode {
    pub fn toggled(self) -> Mode {
        match self {
            Mode::Structures => Mode::Biomes,
            Mode::Biomes => Mode::Structures,
        }
    }

    pub fn takes(self, place: &Place) -> bool {
        matches!((self, place), (Mode::Structures, Place::Structure(_)) | (Mode::Biomes, Place::Biome(_)))
    }
}

/// The nearest place of `mode` in `dimension` that `player` at `at` has not been in.
pub fn target(atlas: &Atlas, mode: Mode, player: u128, dimension: &str, at: [f64; 3]) -> Option<Target> {
    let places: Vec<(Place, [i32; 3])> = match mode {
        Mode::Biomes => {
            let seen = atlas.visited.get(&player);
            atlas
                .found
                .get(dimension)
                .into_iter()
                .flatten()
                .filter(|(b, _)| seen.is_none_or(|s| !s.contains(b)))
                .map(|(b, p)| (Place::Biome(b.clone()), *p))
                .collect()
        }
        Mode::Structures => {
            let been = atlas.been.get(&player);
            atlas
                .sites
                .get(dimension)
                .into_iter()
                .flatten()
                .filter(|(id, _)| been.is_none_or(|s| !s.contains(id)))
                .map(|(id, p)| (Place::Structure(id.clone()), *p))
                .collect()
        }
    };
    places
        .into_iter()
        .map(|(place, p)| {
            let distance = (f64::from(p[0]) - at[0]).hypot(f64::from(p[2]) - at[2]);
            Target { place, at: p, distance }
        })
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compass_looks_for_one_kind_at_a_time() {
        let mut atlas = Atlas::default();
        let me = 7;
        atlas.mark_biome(me, "minecraft:overworld", "minecraft:desert", [10, 64, 0]);
        atlas.mark_structure(me, "minecraft:overworld", "minecraft:village_plains", [40, 64, 0]);
        let at = [0.0, 64.0, 0.0];
        let s = target(&atlas, Mode::Structures, me, "minecraft:overworld", at).expect("a village");
        assert_eq!(s.place, Place::Structure("minecraft:village_plains".to_owned()));
        let b = target(&atlas, Mode::Biomes, me, "minecraft:overworld", at).expect("a desert");
        assert_eq!(b.place, Place::Biome("minecraft:desert".to_owned()));
        atlas.visit(me, "minecraft:desert");
        assert_eq!(target(&atlas, Mode::Biomes, me, "minecraft:overworld", at), None);
        assert_eq!(Mode::Structures.toggled(), Mode::Biomes);
        assert!(Mode::Biomes.takes(&Place::Biome("x".to_owned())));
        assert!(!Mode::Biomes.takes(&Place::Structure("x".to_owned())));
    }
}
