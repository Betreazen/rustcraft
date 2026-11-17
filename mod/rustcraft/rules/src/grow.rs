//! Slice 18: the crystal shard, the druses and growing crystals (guide `crystal`, `grow`; spec
//! E5, CR2). A shard clicked onto the top of a tank of nutrient solution stands there as a seed;
//! the seed drinks solution from the tank and grows: a small crystal, then a druse with crystals
//! all round. Broken grown, it gives two to four shards (its loot table); broken before, its one
//! shard back. Druses also lie in caves underground (worldgen in the content mod).

use crate::tuning::Tuning;

/// The crystal shard: the crystal's, the solar panel's and the battery's ingredient.
pub const SHARD: &str = "rustcraft:crystal_shard";
/// A druse in a cave: breaks into two to four shards.
pub const DRUSE: &str = "rustcraft:druse";
/// A shard growing on a tank, with its stage 0..2 in its state.
pub const SEED: &str = "rustcraft:seed";
/// The seed's state property and its values.
pub const STAGE: &str = "stage";
pub const STAGES: [&str; 3] = ["0", "1", "2"];
/// The fluid the seed drinks.
pub const SOLUTION: &str = crate::power::SOLUTION;
const BUCKET: f64 = 1000.0;

/// The stage a seed shows at `progress` (0..1): the small crystal for the first half, the
/// half-grown druse after it, the grown druse at the end.
pub fn stage(progress: f64) -> u8 {
    if progress >= 1.0 - 1e-9 {
        2
    } else if progress >= 0.5 {
        1
    } else {
        0
    }
}

/// Whether a shard clicked on the top face of `block` holding `fluid` becomes a seed there (CR2:
/// only on a tank that holds solution).
pub fn takes_seed(block: &str, fluid: Option<&str>, face_up: bool) -> bool {
    face_up && block == "rustcraft:tank" && fluid == Some(SOLUTION)
}

/// The solution a growing seed drinks a second, mB: the whole druse's over its growth.
pub fn drink_per_second(t: &Tuning) -> f64 {
    t.grow_buckets * BUCKET / (t.grow_minutes * 60.0).max(1.0)
}

/// The most solution a seed keeps, mB.
pub fn buffer(t: &Tuning) -> f64 {
    t.seed_buffer * BUCKET
}

/// A seed now: how far it has grown and the solution it holds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seed {
    pub progress: f64,
    pub solution: f64,
}

/// What a seed shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shown {
    /// Growing, this far (0..1).
    Growing(f64),
    /// «Нет раствора в баке»: growth stands.
    NoSolution(f64),
    Grown,
}

impl Seed {
    /// The most solution the seed takes from its tank, mB: the buffer, but never more than its
    /// growth still needs, so a grown druse has drunk exactly `grow_buckets` (README CR2).
    pub fn room(&self, t: &Tuning) -> f64 {
        let left = (1.0 - self.progress).max(0.0) * t.grow_buckets * BUCKET;
        buffer(t).min(left)
    }

    /// `dt` seconds of growth: it drinks from what it holds and grows by what it drank; with too
    /// little it grows that much less, with none it stands.
    pub fn grow(&mut self, dt: f64, t: &Tuning) -> Shown {
        if self.progress >= 1.0 - 1e-9 {
            self.progress = 1.0;
            return Shown::Grown;
        }
        let want = drink_per_second(t) * dt;
        let left_to_drink = (1.0 - self.progress) * t.grow_buckets * BUCKET;
        let drunk = want.min(self.solution.max(0.0)).min(left_to_drink);
        if drunk <= 1e-9 {
            return Shown::NoSolution(self.progress);
        }
        self.solution -= drunk;
        self.progress = (self.progress + drunk / (t.grow_buckets * BUCKET)).min(1.0);
        if self.progress >= 1.0 - 1e-9 {
            self.progress = 1.0;
            Shown::Grown
        } else {
            Shown::Growing(self.progress)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_grows_in_ten_minutes_on_four_buckets() {
        let t = Tuning::default();
        // 4 buckets over 600 s: 6.67 mB a second.
        assert!((drink_per_second(&t) * 600.0 - 4000.0).abs() < 1e-6);
        let mut s = Seed {
            progress: 0.0,
            solution: 1e9,
        };
        let mut seconds = 0;
        while s.grow(1.0, &t) != Shown::Grown {
            seconds += 1;
            assert!(seconds < 1000);
        }
        assert_eq!(seconds + 1, 600);
        assert!((1e9 - s.solution - 4000.0).abs() < 1e-3);
    }

    #[test]
    fn a_seed_stands_without_solution_and_goes_on_with_it() {
        let t = Tuning::default();
        let mut s = Seed {
            progress: 0.3,
            solution: 0.0,
        };
        assert_eq!(s.grow(1.0, &t), Shown::NoSolution(0.3));
        assert_eq!(s.progress, 0.3);
        // Half a second's solution: half a second's growth.
        s.solution = drink_per_second(&t) / 2.0;
        let before = s.progress;
        assert!(matches!(s.grow(1.0, &t), Shown::Growing(_)));
        let grown = s.progress - before;
        assert!((grown * 4000.0 - drink_per_second(&t) / 2.0).abs() < 1e-6);
    }

    #[test]
    fn a_seed_takes_from_the_tank_only_what_its_growth_still_needs() {
        let t = Tuning::default();
        let fresh = Seed::default();
        assert!((fresh.room(&t) - buffer(&t)).abs() < 1e-9);
        // 0.9 grown: 400 mB of the 4 buckets are left to drink, less than the buffer.
        let late = Seed {
            progress: 0.9,
            solution: 0.0,
        };
        assert!((late.room(&t) - 400.0).abs() < 1e-6);
        let grown = Seed {
            progress: 1.0,
            solution: 0.0,
        };
        assert_eq!(grown.room(&t), 0.0);
        // Grown from nothing on a full tank, the tank loses exactly 4 buckets.
        let mut s = Seed::default();
        let mut tank = 8000.0;
        while s.grow(1.0, &t) != Shown::Grown {
            let take = (s.room(&t) - s.solution).max(0.0).min(tank);
            tank -= take;
            s.solution += take;
        }
        assert!((8000.0 - tank - 4000.0).abs() < 1e-6, "tank lost {}", 8000.0 - tank);
        assert!(s.solution.abs() < 1e-6);
    }

    #[test]
    fn stages_follow_the_growth() {
        assert_eq!(stage(0.0), 0);
        assert_eq!(stage(0.49), 0);
        assert_eq!(stage(0.5), 1);
        assert_eq!(stage(0.99), 1);
        assert_eq!(stage(1.0), 2);
    }

    #[test]
    fn a_shard_seeds_only_the_top_of_a_tank_of_solution() {
        assert!(takes_seed("rustcraft:tank", Some(SOLUTION), true));
        assert!(!takes_seed("rustcraft:tank", Some(SOLUTION), false));
        assert!(!takes_seed("rustcraft:tank", Some("minecraft:water"), true));
        assert!(!takes_seed("rustcraft:tank", None, true));
        assert!(!takes_seed("minecraft:stone", Some(SOLUTION), true));
    }
}
