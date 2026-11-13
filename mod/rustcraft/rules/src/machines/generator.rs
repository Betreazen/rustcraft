//! The fuel generator (spec 4.2, guide `gen_fuel`): it burns any furnace fuel and gives its
//! output, 16 E/s, while the fuel burns. A fuel item burns as long as in a base-game furnace
//! (coal 1600 ticks: 80 s at 16 E/s is 1280 E). It lights a new item only when its circuit asks
//! for energy, and burns as fast as the circuit takes: a circuit that takes half its output makes
//! an item last twice as long, so an item always gives its full energy (E1: no losses).

use crate::machines::slots::Slot;
use crate::net::items::ItemKey;

/// What the generator holds and how far its fuel has burnt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Generator {
    pub fuel: Slot,
    /// Ticks at full output the lit item still burns.
    pub burn_left: f64,
    /// Ticks at full output the lit item burnt from the start: the window's flame.
    pub burn_total: f64,
}

/// What the generator did on a tick (A4 for a source).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenState {
    /// It gave energy.
    Burning,
    /// «Нет топлива»: nothing lit and nothing in the fuel slot that burns.
    NoFuel,
    /// It could give energy, but its circuit asked for none.
    NoDemand,
}

/// The burn time of a fuel item in ticks, none for an item that is no fuel.
pub type BurnTicks<'a> = &'a dyn Fn(&str) -> Option<f64>;

impl Generator {
    fn has_burnable(&self, burns: BurnTicks) -> bool {
        self.fuel
            .as_ref()
            .is_some_and(|(item, n)| *n > 0 && burns(item).is_some_and(|t| t > 0.0))
    }

    /// The E a second it offers its circuit this tick: its full output while something burns or
    /// waits in its fuel slot, else nothing.
    pub fn offer(&self, output: f64, burns: BurnTicks) -> f64 {
        if self.burn_left > 0.0 || self.has_burnable(burns) {
            output
        } else {
            0.0
        }
    }

    /// The tick after its circuit took `used` of its `output` E a second. Lights the next fuel
    /// item when nothing burns and energy was taken; returns its state and the item it lit (the
    /// bundle puts a lava bucket's empty bucket back into the slot).
    pub fn burn(
        &mut self,
        used: f64,
        output: f64,
        burns: BurnTicks,
    ) -> (GenState, Option<ItemKey>) {
        if used <= 0.0 || output <= 0.0 {
            let state = if self.burn_left > 0.0 || self.has_burnable(burns) {
                GenState::NoDemand
            } else {
                GenState::NoFuel
            };
            return (state, None);
        }
        let mut lit = None;
        if self.burn_left <= 0.0 {
            match self.light(burns) {
                Some(item) => lit = Some(item),
                None => return (GenState::NoFuel, None),
            }
        }
        self.burn_left = (self.burn_left - (used / output).min(1.0)).max(0.0);
        (GenState::Burning, lit)
    }

    /// Takes one fuel item and lights it; its item, none when nothing burns.
    fn light(&mut self, burns: BurnTicks) -> Option<ItemKey> {
        let (item, n) = self.fuel.as_mut()?;
        let ticks = burns(item).filter(|t| *t > 0.0)?;
        let lit = item.clone();
        *n -= 1;
        if *n == 0 {
            self.fuel = None;
        }
        self.burn_left += ticks;
        self.burn_total = ticks;
        Some(lit)
    }

    /// How much of the lit item is left, 0 to 1: the window's flame.
    pub fn flame(&self) -> f64 {
        if self.burn_total > 0.0 {
            (self.burn_left / self.burn_total).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT: f64 = 16.0;

    fn coal(item: &str) -> Option<f64> {
        (item == "minecraft:coal").then_some(1600.0)
    }

    fn with_coal(n: u32) -> Generator {
        Generator {
            fuel: Some(("minecraft:coal".into(), n)),
            ..Generator::default()
        }
    }

    #[test]
    fn one_coal_gives_1280_e_over_80_seconds_at_full_use() {
        let mut g = with_coal(1);
        let mut given = 0.0;
        let mut ticks = 0;
        while g.offer(OUTPUT, &coal) > 0.0 {
            let (state, _) = g.burn(OUTPUT, OUTPUT, &coal);
            assert_eq!(state, GenState::Burning);
            given += OUTPUT / 20.0;
            ticks += 1;
        }
        assert_eq!(ticks, 1600);
        assert!((given - 1280.0).abs() < 1e-6);
        assert_eq!(g.burn(OUTPUT, OUTPUT, &coal).0, GenState::NoFuel);
    }

    #[test]
    fn half_the_use_burns_half_as_fast() {
        let mut g = with_coal(1);
        for _ in 0..1600 {
            g.burn(OUTPUT / 2.0, OUTPUT, &coal);
        }
        assert!((g.burn_left - 800.0).abs() < 1e-6);
        assert!((g.flame() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn without_demand_it_keeps_its_fuel() {
        let mut g = with_coal(3);
        assert_eq!(g.offer(OUTPUT, &coal), OUTPUT);
        let (state, lit) = g.burn(0.0, OUTPUT, &coal);
        assert_eq!(state, GenState::NoDemand);
        assert_eq!(lit, None);
        assert_eq!(g, with_coal(3));
    }

    #[test]
    fn it_lights_the_next_item_when_one_burns_out() {
        let mut g = with_coal(2);
        let (_, lit) = g.burn(OUTPUT, OUTPUT, &coal);
        assert_eq!(lit.as_deref(), Some("minecraft:coal"));
        assert_eq!(g.fuel, Some(("minecraft:coal".into(), 1)));
        for _ in 0..1599 {
            assert_eq!(g.burn(OUTPUT, OUTPUT, &coal).1, None);
        }
        assert_eq!(g.burn_left, 0.0);
        assert!(g.burn(OUTPUT, OUTPUT, &coal).1.is_some());
        assert_eq!(g.fuel, None);
    }

    #[test]
    fn an_item_that_does_not_burn_is_no_fuel() {
        let mut g = Generator {
            fuel: Some(("minecraft:dirt".into(), 5)),
            ..Generator::default()
        };
        assert_eq!(g.offer(OUTPUT, &coal), 0.0);
        assert_eq!(g.burn(OUTPUT, OUTPUT, &coal).0, GenState::NoFuel);
        assert_eq!(
            Generator::default().burn(0.0, OUTPUT, &coal).0,
            GenState::NoFuel
        );
    }
}
