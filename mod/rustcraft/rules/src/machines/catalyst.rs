//! Slice 11: the catalysts (spec 4.12, K1, K2). A catalyst is a fluid out of the mixer. Poured
//! into a machine along a fluid channel or from a tank it touches, it acts while it lasts: the
//! accelerator makes the machine faster, the economizer makes it draw less, the enricher gives a
//! chance of an extra product. A machine holds one catalyst at a time (K2): its tank takes only
//! the fluid it holds until that runs out. A machine spends its catalyst only while it works.
//!
//! In this slice the electric furnace and the crusher take catalysts.

use serde::{Deserialize, Serialize};

use crate::net::fluids::BUCKET;
use crate::tuning::Tuning;

pub const ACCELERATOR: &str = "rustcraft:accelerator";
pub const ECONOMIZER: &str = "rustcraft:economizer";
pub const ENRICHER: &str = "rustcraft:enricher";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Catalyst {
    /// Speed +50%.
    Accelerator,
    /// Energy −30%.
    Economizer,
    /// A 15% chance of an extra product.
    Enricher,
}

impl Catalyst {
    pub const ALL: [Catalyst; 3] = [
        Catalyst::Accelerator,
        Catalyst::Economizer,
        Catalyst::Enricher,
    ];

    pub fn fluid(self) -> &'static str {
        match self {
            Catalyst::Accelerator => ACCELERATOR,
            Catalyst::Economizer => ECONOMIZER,
            Catalyst::Enricher => ENRICHER,
        }
    }

    pub fn of(fluid: &str) -> Option<Catalyst> {
        Catalyst::ALL.into_iter().find(|c| c.fluid() == fluid)
    }

    /// (speed, energy) multipliers it gives a machine.
    pub fn factors(self, t: &Tuning) -> (f64, f64) {
        match self {
            Catalyst::Accelerator => (t.accelerator_speed, 1.0),
            Catalyst::Economizer => (1.0, t.economizer_energy),
            Catalyst::Enricher => (1.0, 1.0),
        }
    }

    /// The chance of an extra product at the end of a recipe.
    pub fn extra_chance(self, t: &Tuning) -> f64 {
        match self {
            Catalyst::Enricher => t.enricher_chance.clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

/// A machine's catalyst tank: its fluid and how much of it, mB (fractions: it drains by the tick).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tank {
    pub fluid: Option<String>,
    pub amount: f64,
}

impl Tank {
    /// What the tank holds, if it holds a catalyst at all.
    pub fn active(&self) -> Option<Catalyst> {
        if self.amount <= 0.0 {
            return None;
        }
        self.fluid.as_deref().and_then(Catalyst::of)
    }

    /// The whole mB of `fluid` it has room for: none for a fluid that is no catalyst, none for
    /// another catalyst while it holds one (K2).
    pub fn room_for(&self, fluid: &str, t: &Tuning) -> u64 {
        if Catalyst::of(fluid).is_none() {
            return 0;
        }
        if self.amount > 0.0 && self.fluid.as_deref() != Some(fluid) {
            return 0;
        }
        let capacity = (t.catalyst_buckets * BUCKET) as f64;
        (capacity - self.amount).max(0.0).floor() as u64
    }

    /// Puts up to `amount` mB of `fluid` in; how much went in.
    pub fn put(&mut self, fluid: &str, amount: u64, t: &Tuning) -> u64 {
        let n = self.room_for(fluid, t).min(amount);
        if n > 0 {
            self.fluid = Some(fluid.to_owned());
            self.amount += n as f64;
        }
        n
    }

    /// Spends what `seconds` of work at `share` of full speed take (a machine short of energy
    /// works slower and spends less). An empty tank forgets its fluid.
    pub fn spend(&mut self, seconds: f64, share: f64, t: &Tuning) {
        if self.amount <= 0.0 {
            return;
        }
        let per_second = BUCKET as f64 / t.catalyst_seconds_per_bucket.max(1.0);
        self.amount = (self.amount - per_second * seconds * share.clamp(0.0, 1.0)).max(0.0);
        if self.amount <= 1e-6 {
            self.amount = 0.0;
            self.fluid = None;
        }
    }

    /// Seconds of work it has left.
    pub fn seconds_left(&self, t: &Tuning) -> f64 {
        self.amount * t.catalyst_seconds_per_bucket / BUCKET as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tank_takes_one_catalyst_at_a_time() {
        let t = Tuning::default();
        let mut tank = Tank::default();
        assert_eq!(tank.room_for("minecraft:water", &t), 0);
        assert_eq!(tank.put(ACCELERATOR, 400, &t), 400);
        assert_eq!(tank.room_for(ECONOMIZER, &t), 0, "K2");
        assert_eq!(tank.room_for(ACCELERATOR, &t), 600);
        assert_eq!(tank.active(), Some(Catalyst::Accelerator));
    }

    #[test]
    fn a_bucket_lasts_five_minutes_of_work_and_then_the_tank_forgets_it() {
        let t = Tuning::default();
        let mut tank = Tank::default();
        tank.put(ENRICHER, BUCKET, &t);
        assert!((tank.seconds_left(&t) - 300.0).abs() < 1e-6);
        tank.spend(150.0, 1.0, &t);
        assert!((tank.amount - 500.0).abs() < 1e-6);
        tank.spend(150.0, 0.5, &t);
        assert!((tank.amount - 250.0).abs() < 1e-6, "half the energy, half the use");
        tank.spend(1000.0, 1.0, &t);
        assert_eq!(tank.active(), None);
        assert_eq!(tank.fluid, None);
        assert_eq!(tank.room_for(ECONOMIZER, &t), BUCKET);
    }

    #[test]
    fn each_catalyst_has_its_effect() {
        let t = Tuning::default();
        assert_eq!(Catalyst::Accelerator.factors(&t), (1.5, 1.0));
        assert_eq!(Catalyst::Economizer.factors(&t), (1.0, 0.7));
        assert_eq!(Catalyst::Enricher.factors(&t), (1.0, 1.0));
        assert!((Catalyst::Enricher.extra_chance(&t) - 0.15).abs() < 1e-9);
        assert_eq!(Catalyst::Accelerator.extra_chance(&t), 0.0);
    }
}
