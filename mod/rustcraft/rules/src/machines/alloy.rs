//! The alloy smelter: up to four ingredients in, an alloy out into one of three output slots. On the
//! first tier it burns fuel; with energy it works faster and burns none. If an alloy in its output is
//! an ingredient of a recipe whose other ingredients are in, it takes that alloy back itself, so
//! sparksteel goes straight on to crystallite (AL1); a switch turns that off.

use crate::machines::processor::State;
use crate::machines::slots::{self, count_in, take_from};
use crate::net::items::ItemKey;

/// An alloy recipe: every ingredient with its count, and the alloy it gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlloyRecipe {
    pub inputs: Vec<(ItemKey, u32)>,
    pub output: (ItemKey, u32),
}

/// The fixed numbers of the smelter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlloySpec {
    /// Ticks a recipe takes on fuel.
    pub ticks_on_fuel: f64,
    /// Ticks a recipe takes on full energy.
    pub ticks_on_energy: f64,
    /// E a second it asks for when it has work.
    pub energy: f64,
    pub slot_capacity: u32,
}

pub const INPUTS: usize = 4;
pub const OUTPUTS: usize = 3;

/// The smelter's contents and settings.
#[derive(Clone, Debug, PartialEq)]
pub struct AlloySmelter {
    pub inputs: [Option<(ItemKey, u32)>; INPUTS],
    pub outputs: [Option<(ItemKey, u32)>; OUTPUTS],
    pub fuel: Option<(ItemKey, u32)>,
    /// Ticks of burning left from the fuel item already lit.
    pub burn_left: f64,
    pub progress: f64,
    /// AL1: take an alloy back from the output when a recipe needs it.
    pub take_own: bool,
}

impl Default for AlloySmelter {
    fn default() -> Self {
        AlloySmelter {
            inputs: Default::default(),
            outputs: Default::default(),
            fuel: None,
            burn_left: 0.0,
            progress: 0.0,
            take_own: true,
        }
    }
}

impl AlloySmelter {
    /// The E a second it asks for: its full rate while it has a recipe it can run.
    pub fn request(&self, recipes: &[AlloyRecipe], spec: &AlloySpec) -> f64 {
        if self.job(recipes, spec).is_ok() {
            spec.energy
        } else {
            0.0
        }
    }

    /// One tick with `share` of the requested energy; `burn_ticks` says how long a fuel item burns.
    pub fn tick(
        &mut self,
        recipes: &[AlloyRecipe],
        spec: &AlloySpec,
        share: f64,
        burn_ticks: &dyn Fn(&str) -> Option<f64>,
    ) -> State {
        let recipe = match self.job(recipes, spec) {
            Ok(r) => r.clone(),
            Err(state) => {
                if state == State::Idle {
                    self.progress = 0.0;
                }
                return state;
            }
        };
        if share > 0.0 {
            // On energy: faster, and no fuel.
            self.progress += share.min(1.0) / spec.ticks_on_energy;
        } else {
            if self.burn_left <= 0.0 && !self.light(burn_ticks) {
                return State::NoEnergy;
            }
            self.burn_left -= 1.0;
            self.progress += 1.0 / spec.ticks_on_fuel;
        }
        if self.progress >= 1.0 - 1e-9 {
            self.progress = 0.0;
            self.apply(&recipe, spec);
        }
        State::Working
    }

    /// How many of `item` the smelter can use: its inputs, and its outputs when it takes its own.
    fn available(&self, item: &str) -> u32 {
        let own = if self.take_own {
            count_in(&self.outputs, item)
        } else {
            0
        };
        count_in(&self.inputs, item) + own
    }

    /// The first recipe it can run, if its output has room once the ingredients are taken.
    fn job<'a>(
        &self,
        recipes: &'a [AlloyRecipe],
        spec: &AlloySpec,
    ) -> Result<&'a AlloyRecipe, State> {
        let recipe = recipes
            .iter()
            .find(|r| r.inputs.iter().all(|(i, n)| self.available(i) >= *n))
            .ok_or(State::Idle)?;
        let mut after = self.clone();
        after.take_ingredients(recipe);
        if after.place(&recipe.output, spec) {
            Ok(recipe)
        } else {
            Err(State::OutputFull)
        }
    }

    /// Lights the next fuel item; false when there is none that burns.
    fn light(&mut self, burn_ticks: &dyn Fn(&str) -> Option<f64>) -> bool {
        let Some((item, n)) = self.fuel.as_mut() else {
            return false;
        };
        let Some(ticks) = burn_ticks(item) else {
            return false;
        };
        *n -= 1;
        if *n == 0 {
            self.fuel = None;
        }
        self.burn_left += ticks;
        true
    }

    /// Takes a recipe's ingredients: from the inputs first, then from its own output (AL1).
    fn take_ingredients(&mut self, recipe: &AlloyRecipe) {
        for (item, n) in &recipe.inputs {
            let left = take_from(&mut self.inputs, item, *n);
            if left > 0 && self.take_own {
                take_from(&mut self.outputs, item, left);
            }
        }
    }

    /// Puts an alloy into the output slots; false when they have no room for all of it.
    fn place(&mut self, (item, n): &(ItemKey, u32), spec: &AlloySpec) -> bool {
        slots::place(&mut self.outputs, item, *n, spec.slot_capacity)
    }

    fn apply(&mut self, recipe: &AlloyRecipe, spec: &AlloySpec) {
        self.take_ingredients(recipe);
        self.place(&recipe.output, spec);
    }
}

/// Recipes in the order the smelter tries them: a recipe that takes another recipe's alloy
/// before that recipe, so an alloy goes on up when it can (AL1: sparksteel to crystallite before
/// more sparksteel). Recipes of one depth keep their order.
pub fn ordered(mut recipes: Vec<AlloyRecipe>) -> Vec<AlloyRecipe> {
    let depth = |r: &AlloyRecipe, all: &[AlloyRecipe]| -> usize {
        // The longest chain of recipes whose alloys lead into `r`, at most `all.len()` deep.
        let mut level = 0;
        let mut frontier: Vec<&AlloyRecipe> = vec![r];
        while level < all.len() {
            let next: Vec<&AlloyRecipe> = all
                .iter()
                .filter(|s| {
                    frontier
                        .iter()
                        .any(|f| f.inputs.iter().any(|(i, _)| *i == s.output.0))
                })
                .collect();
            if next.is_empty() {
                break;
            }
            level += 1;
            frontier = next;
        }
        level
    };
    let depths: Vec<usize> = recipes.iter().map(|r| depth(r, &recipes)).collect();
    let mut order: Vec<usize> = (0..recipes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(depths[i]));
    let mut taken: Vec<Option<AlloyRecipe>> = recipes.drain(..).map(Some).collect();
    order.into_iter().filter_map(|i| taken[i].take()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: AlloySpec = AlloySpec {
        ticks_on_fuel: 200.0,
        ticks_on_energy: 100.0,
        energy: 8.0,
        slot_capacity: 64,
    };

    /// Higher tiers first, so an alloy goes on up when it can (AL1).
    fn recipes() -> Vec<AlloyRecipe> {
        vec![
            AlloyRecipe {
                inputs: vec![
                    ("rc:sparksteel".into(), 1),
                    ("minecraft:diamond".into(), 1),
                    ("rc:crystal_shard".into(), 2),
                ],
                output: ("rc:crystallite".into(), 1),
            },
            AlloyRecipe {
                inputs: vec![
                    ("rc:coppersteel".into(), 1),
                    ("minecraft:gold_ingot".into(), 1),
                    ("minecraft:redstone".into(), 1),
                ],
                output: ("rc:sparksteel".into(), 1),
            },
        ]
    }

    fn coal(item: &str) -> Option<f64> {
        (item == "minecraft:coal").then_some(1600.0)
    }

    fn slot(item: &str, n: u32) -> Option<(ItemKey, u32)> {
        Some((item.into(), n))
    }

    fn sparksteel_kit() -> AlloySmelter {
        AlloySmelter {
            inputs: [
                slot("rc:coppersteel", 4),
                slot("minecraft:gold_ingot", 4),
                slot("minecraft:redstone", 4),
                None,
            ],
            fuel: slot("minecraft:coal", 2),
            ..AlloySmelter::default()
        }
    }

    fn run(s: &mut AlloySmelter, share: f64, ticks: u32) -> State {
        let mut last = State::Idle;
        for _ in 0..ticks {
            last = s.tick(&recipes(), &SPEC, share, &coal);
        }
        last
    }

    fn count(slots: &[Option<(ItemKey, u32)>], item: &str) -> u32 {
        slots
            .iter()
            .flatten()
            .filter(|(i, _)| i == item)
            .map(|(_, n)| n)
            .sum()
    }

    #[test]
    fn it_makes_sparksteel_on_fuel() {
        let mut s = sparksteel_kit();
        assert_eq!(run(&mut s, 0.0, 200), State::Working);
        assert_eq!(count(&s.outputs, "rc:sparksteel"), 1);
        assert_eq!(count(&s.inputs, "rc:coppersteel"), 3);
        // One coal was lit and burns on.
        assert_eq!(s.fuel, slot("minecraft:coal", 1));
        assert!(s.burn_left > 0.0);
    }

    #[test]
    fn with_energy_it_is_faster_and_burns_no_fuel() {
        let mut s = sparksteel_kit();
        assert_eq!(s.request(&recipes(), &SPEC), 8.0);
        run(&mut s, 1.0, 100);
        assert_eq!(count(&s.outputs, "rc:sparksteel"), 1);
        assert_eq!(s.fuel, slot("minecraft:coal", 2));
    }

    #[test]
    fn it_takes_its_own_sparksteel_on_to_crystallite() {
        // Rule AL1.
        let mut s = AlloySmelter {
            inputs: [
                slot("minecraft:diamond", 1),
                slot("rc:crystal_shard", 2),
                None,
                None,
            ],
            outputs: [slot("rc:sparksteel", 1), None, None],
            fuel: slot("minecraft:coal", 1),
            ..AlloySmelter::default()
        };
        run(&mut s, 0.0, 200);
        assert_eq!(count(&s.outputs, "rc:crystallite"), 1);
        assert_eq!(count(&s.outputs, "rc:sparksteel"), 0);
    }

    #[test]
    fn with_the_switch_off_it_leaves_its_output_alone() {
        let mut s = AlloySmelter {
            inputs: [
                slot("minecraft:diamond", 1),
                slot("rc:crystal_shard", 2),
                None,
                None,
            ],
            outputs: [slot("rc:sparksteel", 1), None, None],
            fuel: slot("minecraft:coal", 1),
            take_own: false,
            ..AlloySmelter::default()
        };
        assert_eq!(run(&mut s, 0.0, 10), State::Idle);
        assert_eq!(count(&s.outputs, "rc:sparksteel"), 1);
    }

    #[test]
    fn without_fuel_or_energy_it_waits() {
        let mut s = AlloySmelter {
            fuel: None,
            ..sparksteel_kit()
        };
        assert_eq!(run(&mut s, 0.0, 10), State::NoEnergy);
        assert_eq!(s.progress, 0.0);
    }

    #[test]
    fn recipes_that_take_an_alloy_go_first() {
        // The data files come in name order: crystallite, sparksteel, starsteel.
        let r = |inputs: &[&str], output: &str| AlloyRecipe {
            inputs: inputs.iter().map(|i| ((*i).into(), 1)).collect(),
            output: (output.into(), 2),
        };
        let files = vec![
            r(&["rc:sparksteel", "minecraft:diamond"], "rc:crystallite"),
            r(&["rc:coppersteel", "minecraft:gold_ingot"], "rc:sparksteel"),
            r(
                &["rc:crystallite", "minecraft:netherite_ingot"],
                "rc:starsteel",
            ),
        ];
        let outputs: Vec<String> = ordered(files).into_iter().map(|r| r.output.0).collect();
        assert_eq!(outputs, ["rc:starsteel", "rc:crystallite", "rc:sparksteel"]);
    }

    #[test]
    fn the_guides_sparksteel_takes_two_coppersteel_and_gives_two() {
        let guide = vec![AlloyRecipe {
            inputs: vec![
                ("rc:coppersteel".into(), 2),
                ("minecraft:gold_ingot".into(), 1),
                ("minecraft:redstone".into(), 1),
            ],
            output: ("rc:sparksteel".into(), 2),
        }];
        let mut s = sparksteel_kit();
        for _ in 0..200 {
            s.tick(&guide, &SPEC, 0.0, &coal);
        }
        assert_eq!(count(&s.outputs, "rc:sparksteel"), 2);
        assert_eq!(count(&s.inputs, "rc:coppersteel"), 2);
        assert_eq!(count(&s.inputs, "minecraft:gold_ingot"), 3);
    }

    #[test]
    fn full_outputs_stop_it() {
        let mut s = AlloySmelter {
            outputs: [slot("rc:a", 64), slot("rc:b", 64), slot("rc:c", 64)],
            ..sparksteel_kit()
        };
        assert_eq!(run(&mut s, 1.0, 5), State::OutputFull);
        assert_eq!(s.request(&recipes(), &SPEC), 0.0);
    }
}
