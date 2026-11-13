//! What the scenarios expect, worked out by running the rules themselves on the numbers of
//! `Tuning`: the ticks a machine takes on the energy it gets, what a net moves in a while. The
//! world then has to agree with the rules, through every glue in between.

use crate::machines::alloy::{AlloyRecipe, AlloySmelter, AlloySpec};
use crate::machines::crafter::{CraftRecipe, Crafter, CrafterSpec, Page, Product};
use crate::machines::processor::{Processor, Recipe, Shown, Spec, State};
use crate::machines::pump::{Pump, Source, Under};
use crate::tuning::Tuning;

/// The longest a simulated machine may run before it is taken to never finish.
const MAX_TICKS: u64 = 100_000;

/// The share of its request a machine asking `request` E/s gets from `supply` E/s.
pub fn share(supply: f64, request: f64) -> f64 {
    if request <= 0.0 {
        0.0
    } else {
        (supply / request).min(1.0)
    }
}

/// What a one-input machine shows while it works on `share`.
pub fn working_shown(share: f64) -> Shown {
    Shown::of(State::Working, share)
}

/// Ticks a one-input machine (the electric furnace, the crusher) of `ticks` a recipe and `energy`
/// E/s takes to give `outputs` results on `supply` E/s.
pub fn processor_ticks(ticks: f64, energy: f64, supply: f64, outputs: u32) -> Option<u64> {
    let recipe = [Recipe {
        input: "in".into(),
        input_count: 1,
        output: "out".into(),
        output_count: 1,
    }];
    let spec = Spec {
        ticks,
        energy,
        output_capacity: u32::MAX,
    };
    let mut p = Processor {
        input: Some(("in".into(), outputs)),
        ..Processor::default()
    };
    let share = share(supply, p.request(&recipe, &spec));
    (1..=MAX_TICKS).find(|_| {
        p.tick(&recipe, &spec, share);
        p.output.as_ref().is_some_and(|(_, n)| *n >= outputs)
    })
}

/// Ticks the electric furnace takes to smelt `outputs` items of a recipe the base game's furnace
/// smelts in `cook_ticks`, on `supply` E/s.
pub fn furnace_ticks(tuning: &Tuning, cook_ticks: f64, supply: f64, outputs: u32) -> Option<u64> {
    let ticks = (cook_ticks / tuning.furnace_speedup).max(1.0);
    processor_ticks(ticks, tuning.furnace_energy, supply, outputs)
}

/// Ticks the alloy smelter takes to make `recipe` once on `supply` E/s.
pub fn alloy_ticks(tuning: &Tuning, recipe: &AlloyRecipe, supply: f64) -> Option<u64> {
    let spec = AlloySpec {
        ticks_on_fuel: tuning.alloy_ticks_on_fuel,
        ticks_on_energy: tuning.alloy_ticks_on_energy,
        energy: tuning.alloy_energy,
        slot_capacity: 64,
    };
    let mut smelter = AlloySmelter::default();
    for (slot, (item, n)) in smelter.inputs.iter_mut().zip(&recipe.inputs) {
        *slot = Some((item.clone(), *n));
    }
    let recipes = std::slice::from_ref(recipe);
    let share = share(supply, smelter.request(recipes, &spec));
    (1..=MAX_TICKS).find(|_| {
        smelter.tick(recipes, &spec, share, &|_| None);
        smelter
            .outputs
            .iter()
            .flatten()
            .any(|(i, _)| *i == recipe.output.0)
    })
}

/// Ticks the autocrafter takes to craft a one-ingredient pattern `crafts` times on `supply` E/s.
pub fn crafter_ticks(tuning: &Tuning, made: u32, crafts: u32, supply: f64) -> Option<u64> {
    let spec = CrafterSpec {
        ticks: tuning.crafter_ticks,
        energy: tuning.crafter_energy,
    };
    let recipe = CraftRecipe::of_pattern(
        &[Some("in".to_owned())],
        vec![Product {
            item: "out".into(),
            count: made,
            stack: 64,
        }],
    );
    let mut c = Crafter {
        pages: vec![Page {
            recipe: Some(recipe),
            order: None,
        }],
        buffer: vec![("in".into(), crafts)],
        output: vec![None; 3],
        ..Crafter::default()
    };
    let wanted = made * crafts;
    (1..=MAX_TICKS).find(|_| {
        let share = share(supply, c.request(&spec));
        c.tick(&spec, share);
        c.output.iter().flatten().map(|(_, n)| n).sum::<u32>() >= wanted
    })
}

/// Ticks the pump takes to pump its first bucket on `supply` E/s.
pub fn pump_ticks(tuning: &Tuning, supply: f64) -> Option<u64> {
    let under = Under {
        source: Some(Source::Lava),
        water_beside: 0,
        held_up: true,
    };
    let mut pump = Pump::default();
    let share = share(supply, pump.request(&under, tuning.pump_energy));
    (1..=MAX_TICKS).find(|_| pump.tick(&under, share, tuning.pump_ticks).pumped.is_some())
}

/// Items an item net of `tier` moves in `ticks` level ticks of steady work.
pub fn items_in(tuning: &Tuning, tier: u8, ticks: u64) -> f64 {
    tuning.items_of(tier) * ticks as f64 / tuning.ticks_per_second
}

/// E a station's buffer gains in `ticks` from `supply` E/s.
pub fn station_gain(tuning: &Tuning, supply: f64, ticks: u64) -> f64 {
    supply.min(tuning.station_charge) * ticks as f64 / tuning.ticks_per_second
}

/// What a trip of `length` blocks of rail costs (T8).
pub fn trip_cost(tuning: &Tuning, length: u32) -> f64 {
    f64::from(length) / tuning.rail_blocks_per_energy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_furnace_smelts_twice_as_fast_on_full_energy_and_half_on_half() {
        let t = Tuning::default();
        assert_eq!(furnace_ticks(&t, 200.0, t.furnace_energy, 1), Some(100));
        assert_eq!(furnace_ticks(&t, 200.0, t.crystal_output, 1), Some(200));
        assert_eq!(furnace_ticks(&t, 200.0, t.furnace_energy, 2), Some(200));
        assert_eq!(furnace_ticks(&t, 200.0, 0.0, 1), None);
        assert_eq!(
            working_shown(share(t.crystal_output, t.furnace_energy)),
            Shown::Slowed { percent: 50 }
        );
    }

    #[test]
    fn the_other_machines_take_their_tuned_ticks() {
        let t = Tuning::default();
        let recipe = AlloyRecipe {
            inputs: vec![("a".into(), 2), ("b".into(), 1)],
            output: ("c".into(), 1),
        };
        assert_eq!(alloy_ticks(&t, &recipe, t.alloy_energy), Some(100));
        assert_eq!(crafter_ticks(&t, 4, 2, t.crafter_energy), Some(40));
        assert_eq!(pump_ticks(&t, t.pump_energy), Some(200));
        assert_eq!(processor_ticks(100.0, 8.0, 8.0, 1), Some(100));
    }

    #[test]
    fn nets_and_stations_follow_their_rates() {
        let t = Tuning::default();
        assert_eq!(items_in(&t, 1, 40), 8.0);
        assert_eq!(items_in(&t, 4, 40), 512.0);
        assert_eq!(station_gain(&t, t.crystal_output, 40), 8.0);
        assert_eq!(trip_cost(&t, 8), 2.0);
    }
}
