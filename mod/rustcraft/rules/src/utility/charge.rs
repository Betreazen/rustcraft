//! The charger and the charge plate (guide `charger`, `charge_plate`): the mod's charged items keep
//! their charge and their battery's size in their custom data, under [`CHARGE`] and [`CAPACITY`]
//! (the technical armour, the energy blade, shield and rifle and the jetpack carry them). An item
//! without a capacity is not charged.

pub const CHARGE: &str = "rustcraft:charge";
pub const CAPACITY: &str = "rustcraft:charge_max";

/// A battery of an item: its charge and its size, E.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Battery {
    pub charge: f64,
    pub capacity: f64,
}

impl Battery {
    pub fn full(&self) -> bool {
        self.capacity <= 0.0 || self.charge >= self.capacity - 1e-9
    }

    /// Puts up to `offer` E in: (the battery after, what it took).
    pub fn fill(self, offer: f64) -> (Battery, f64) {
        let room = (self.capacity - self.charge).max(0.0);
        let took = offer.max(0.0).min(room);
        (
            Battery {
                charge: self.charge + took,
                ..self
            },
            took,
        )
    }
}

/// How `energy` E a tick is shared among `batteries`: in order, each filled before the next.
pub fn share(batteries: &[Battery], energy: f64) -> (Vec<Battery>, f64) {
    let mut left = energy;
    let after = batteries
        .iter()
        .map(|b| {
            let (n, took) = b.fill(left);
            left -= took;
            n
        })
        .collect();
    (after, energy - left)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_fills_one_battery_after_another() {
        let b = [
            Battery { charge: 90.0, capacity: 100.0 },
            Battery { charge: 0.0, capacity: 50.0 },
        ];
        let (after, used) = share(&b, 30.0);
        assert_eq!(used, 30.0);
        assert!(after[0].full());
        assert_eq!(after[1].charge, 20.0);
        let (_, used) = share(&after, 1000.0);
        assert_eq!(used, 30.0, "only what fits");
    }
}
