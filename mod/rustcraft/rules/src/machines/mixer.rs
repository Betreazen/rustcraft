//! The mixer (spec 4.3 «Смеситель», 4.12 K1, guide `mixer`, `fluids`): it makes fluids out of
//! water and items. Ingredients come into its three input slots (one kind a slot, as the alloy
//! smelter's), water into its water tank, and the product goes into its product tank, from where
//! a fluid channel or a tank takes it. Its recipes are the content mod's `rustcraft:mixing` files;
//! slice 10 has the nutrient solution, which feeds the crystal structures (E8). Amounts in
//! millibuckets.

use crate::machines::slots::{Slot, count_in, take_from};

/// The mixer's input slots.
pub const INPUTS: usize = 3;

/// One mixing recipe as its file says it.
#[derive(Clone, Debug, PartialEq)]
pub struct MixRecipe {
    /// Items and how many of each a batch takes.
    pub ingredients: Vec<(String, u32)>,
    /// Water a batch takes, mB.
    pub water: u64,
    /// The fluid it makes and how much a batch makes, mB.
    pub fluid: String,
    pub amount: u64,
    /// Ticks a batch takes in normal mode.
    pub ticks: f64,
}

/// What the mixer holds besides its items.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mixer {
    /// Water in its water tank, mB.
    pub water: u64,
    /// Its product tank: the fluid and how much.
    pub product: Option<(String, u64)>,
    /// How far the batch is, 0..1.
    pub progress: f64,
}

/// What a mixer shows (A4): one diagnosis at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Working,
    /// It works slower by that share: «Не хватает энергии: 50%».
    Slowed(u32),
    NoEnergy,
    /// «Нет ингредиентов».
    NoIngredients,
    /// «Нет воды».
    NoWater,
    /// «Готовое некуда отдать»: the product tank is full or holds another fluid.
    Full,
}

/// The numbers one tick runs on: the tanks' size, E/s in normal mode, and the work and energy
/// factors of its mode and modules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub tank: u64,
    pub energy: f64,
    pub work: f64,
    pub energy_factor: f64,
}

impl Mixer {
    /// The first recipe whose ingredients all lie in `inputs`.
    pub fn recipe<'a>(recipes: &'a [MixRecipe], inputs: &[Slot]) -> Option<&'a MixRecipe> {
        recipes.iter().find(|r| {
            !r.ingredients.is_empty()
                && r.ingredients
                    .iter()
                    .all(|(item, n)| count_in(inputs, item) >= *n)
        })
    }

    /// Room in the product tank for `fluid`.
    pub fn room_for(&self, fluid: &str, tank: u64) -> u64 {
        match &self.product {
            Some((f, n)) if f == fluid => tank.saturating_sub(*n),
            Some(_) => 0,
            None => tank,
        }
    }

    /// Why it cannot work now, if it cannot; else the recipe it works on.
    pub fn job<'a>(
        &self,
        recipes: &'a [MixRecipe],
        inputs: &[Slot],
        spec: &Spec,
    ) -> Result<&'a MixRecipe, Shown> {
        let r = Self::recipe(recipes, inputs).ok_or(Shown::NoIngredients)?;
        if self.water < r.water {
            return Err(Shown::NoWater);
        }
        if self.room_for(&r.fluid, spec.tank) < r.amount {
            return Err(Shown::Full);
        }
        Ok(r)
    }

    /// E a second it asks this tick.
    pub fn request(&self, recipes: &[MixRecipe], inputs: &[Slot], spec: &Spec) -> f64 {
        if self.job(recipes, inputs, spec).is_ok() {
            spec.energy * spec.energy_factor
        } else {
            0.0
        }
    }

    /// One tick on `share` of its request: the batch moves on, and when it is done its
    /// ingredients and water go and its product comes. What it shows.
    pub fn tick(
        &mut self,
        recipes: &[MixRecipe],
        inputs: &mut [Slot],
        spec: &Spec,
        share: f64,
    ) -> Shown {
        let r = match self.job(recipes, inputs, spec) {
            Ok(r) => r.clone(),
            Err(why) => {
                self.progress = 0.0;
                return why;
            }
        };
        if share <= 0.0 {
            return Shown::NoEnergy;
        }
        self.progress += share * spec.work / r.ticks.max(1.0);
        if self.progress >= 1.0 - 1e-9 {
            self.progress = 0.0;
            for (item, n) in &r.ingredients {
                take_from(inputs, item, *n);
            }
            self.water -= r.water;
            let had = self.product.as_ref().map_or(0, |(_, n)| *n);
            self.product = Some((r.fluid.clone(), had + r.amount));
        }
        if share < 1.0 - 1e-9 {
            Shown::Slowed(((share * 100.0).round() as u32).clamp(1, 99))
        } else {
            Shown::Working
        }
    }

    /// The product taken out, `amount` at most: what came out.
    pub fn take(&mut self, fluid: &str, amount: u64) -> u64 {
        let Some((f, have)) = self.product.as_mut() else {
            return 0;
        };
        if f != fluid {
            return 0;
        }
        let n = (*have).min(amount);
        *have -= n;
        if *have == 0 {
            self.product = None;
        }
        n
    }
}

/// What the self-test and the window read of a mixer: its tanks and what it showed last.
#[derive(Clone, Debug, PartialEq)]
pub struct MixerView {
    pub water: u64,
    pub product: Option<(String, u64)>,
    pub shown: Option<Shown>,
}

/// The items any recipe takes: what the input slots let in.
pub fn ingredients(recipes: &[MixRecipe]) -> Vec<String> {
    let mut out: Vec<String> = recipes
        .iter()
        .flat_map(|r| r.ingredients.iter().map(|(i, _)| i.clone()))
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines::processor::WorkMode;

    pub const SOLUTION: &str = "rustcraft:solution";

    fn solution() -> MixRecipe {
        MixRecipe {
            ingredients: vec![
                ("minecraft:bone_meal".into(), 1),
                ("minecraft:redstone".into(), 1),
            ],
            water: 1000,
            fluid: SOLUTION.into(),
            amount: 1000,
            ticks: 100.0,
        }
    }

    fn spec(mode: WorkMode) -> Spec {
        let (work, energy_factor) = mode.factors();
        Spec {
            tank: 4000,
            energy: 8.0,
            work,
            energy_factor,
        }
    }

    fn inputs() -> Vec<Slot> {
        vec![
            Some(("minecraft:bone_meal".into(), 2)),
            Some(("minecraft:redstone".into(), 2)),
            None,
        ]
    }

    #[test]
    fn a_batch_of_solution_takes_water_bone_meal_and_redstone() {
        let r = [solution()];
        let s = spec(WorkMode::Normal);
        let mut m = Mixer {
            water: 4000,
            ..Mixer::default()
        };
        let mut slots = inputs();
        assert_eq!(m.request(&r, &slots, &s), 8.0);
        let mut ticks = 0;
        while m.product.is_none() {
            assert_eq!(m.tick(&r, &mut slots, &s, 1.0), Shown::Working);
            ticks += 1;
        }
        assert_eq!(ticks, 100);
        assert_eq!(m.product, Some((SOLUTION.into(), 1000)));
        assert_eq!(m.water, 3000);
        assert_eq!(count_in(&slots, "minecraft:bone_meal"), 1);
        assert_eq!(count_in(&slots, "minecraft:redstone"), 1);
    }

    #[test]
    fn it_says_what_it_lacks() {
        let r = [solution()];
        let s = spec(WorkMode::Normal);
        let mut m = Mixer::default();
        let mut slots = inputs();
        assert_eq!(m.tick(&r, &mut slots, &s, 1.0), Shown::NoWater);
        assert_eq!(m.request(&r, &slots, &s), 0.0);
        m.water = 1000;
        let mut none = vec![Some(("minecraft:bone_meal".into(), 1)), None, None];
        assert_eq!(m.tick(&r, &mut none, &s, 1.0), Shown::NoIngredients);
        // A product tank of another fluid, or a full one: nowhere to put it.
        m.product = Some(("minecraft:lava".into(), 10));
        assert_eq!(m.tick(&r, &mut slots, &s, 1.0), Shown::Full);
        m.product = Some((SOLUTION.into(), 3500));
        assert_eq!(m.tick(&r, &mut slots, &s, 1.0), Shown::Full);
        m.product = Some((SOLUTION.into(), 3000));
        assert_eq!(m.tick(&r, &mut slots, &s, 0.0), Shown::NoEnergy);
        assert_eq!(m.tick(&r, &mut slots, &s, 0.5), Shown::Slowed(50));
    }

    #[test]
    fn boost_works_twice_as_fast_for_three_times_the_energy() {
        let r = [solution()];
        let s = spec(WorkMode::Boost);
        let mut m = Mixer {
            water: 1000,
            ..Mixer::default()
        };
        let mut slots = inputs();
        assert_eq!(m.request(&r, &slots, &s), 24.0);
        for _ in 0..50 {
            m.tick(&r, &mut slots, &s, 1.0);
        }
        assert!(m.product.is_some());
    }

    #[test]
    fn the_product_goes_out_and_the_tank_empties() {
        let mut m = Mixer {
            product: Some((SOLUTION.into(), 1500)),
            ..Mixer::default()
        };
        assert_eq!(m.take("minecraft:water", 100), 0);
        assert_eq!(m.take(SOLUTION, 1000), 1000);
        assert_eq!(m.take(SOLUTION, 1000), 500);
        assert_eq!(m.product, None);
        assert_eq!(m.room_for(SOLUTION, 4000), 4000);
        assert_eq!(
            ingredients(&[solution(), solution()]),
            vec!["minecraft:bone_meal", "minecraft:redstone"]
        );
    }
}
