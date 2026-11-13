//! The pump (guide `pump`, spec 4.3 and round 15): it takes the water or lava source block under
//! it, a bucket every 10 seconds on the first tier with full energy (4 E/s), and gives it to a tank
//! it touches or a fluid channel. Water stays endless where the game would fill the source again
//! (two or more water sources beside it, on a block that holds water up); lava, and water anywhere
//! else, is gone once pumped. The pump holds one bucket: until it has given it away it takes no
//! more, so no source is lost for want of room (M6).

use crate::machines::processor::{Shown, State};
use crate::net::fluids::{BUCKET, FluidKey};

pub const WATER: &str = "minecraft:water";
pub const LAVA: &str = "minecraft:lava";

/// What is under the pump.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Under {
    /// A source block of this fluid, none for anything else (flowing fluid included).
    pub source: Option<Source>,
    /// Water source blocks beside that source, on its four sides.
    pub water_beside: u8,
    /// The block under the source holds water up: solid, or a water source.
    pub held_up: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Water,
    Lava,
}

impl Source {
    pub fn fluid(self) -> &'static str {
        match self {
            Source::Water => WATER,
            Source::Lava => LAVA,
        }
    }
}

impl Under {
    /// Whether the game fills the source again once it is pumped (vanilla's infinite water:
    /// `FlowingFluid.getNewLiquid` makes a source of water with two sources beside it on a
    /// supporting block).
    pub fn endless(&self) -> bool {
        self.source == Some(Source::Water) && self.water_beside >= 2 && self.held_up
    }
}

/// A pump's work as its block entity keeps it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pump {
    /// Work done on the next bucket, 0 to 1.
    pub progress: f64,
    /// The fluid it pumped and has not given away yet, in millibuckets.
    pub held: Option<(FluidKey, u64)>,
}

/// What one tick of the pump did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick {
    pub state: State,
    /// A bucket was pumped this tick: take the source out of the world unless it is endless.
    pub pumped: Option<Source>,
}

impl Pump {
    /// E a second it asks for: its draw while it has a source and room for a bucket.
    pub fn request(&self, under: &Under, energy: f64) -> f64 {
        if self.held.is_none() && under.source.is_some() {
            energy
        } else {
            0.0
        }
    }

    /// One tick on `share` of its request; a bucket takes `ticks` on full energy.
    pub fn tick(&mut self, under: &Under, share: f64, ticks: f64) -> Tick {
        let idle = |state| Tick {
            state,
            pumped: None,
        };
        if self.held.is_some() {
            return idle(State::OutputFull);
        }
        let Some(source) = under.source else {
            self.progress = 0.0;
            return idle(State::Idle);
        };
        if share <= 0.0 {
            return idle(State::NoEnergy);
        }
        self.progress += share.min(1.0) / ticks.max(1.0);
        if self.progress + 1e-9 < 1.0 {
            return idle(State::Working);
        }
        self.progress = 0.0;
        self.held = Some((source.fluid().to_owned(), BUCKET));
        Tick {
            state: State::Working,
            pumped: Some(source),
        }
    }

    /// What its window and its look show.
    pub fn shown(tick: Tick, share: f64) -> Shown {
        Shown::of(tick.state, share)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WATER_POND: Under = Under {
        source: Some(Source::Water),
        water_beside: 3,
        held_up: true,
    };

    #[test]
    fn a_bucket_takes_ten_seconds_on_full_energy() {
        // Round 15: tier 1, one bucket every 10 s.
        let mut p = Pump::default();
        let mut pumped = 0;
        for _ in 0..200 {
            if p.tick(&WATER_POND, 1.0, 200.0).pumped.is_some() {
                pumped += 1;
                p.held = None;
            }
        }
        assert_eq!(pumped, 1);
    }

    #[test]
    fn half_the_energy_pumps_at_half_the_speed() {
        let mut p = Pump::default();
        let t = (0..399).map(|_| p.tick(&WATER_POND, 0.5, 200.0)).last();
        assert_eq!(t.map(|t| t.pumped), Some(None));
        assert_eq!(p.tick(&WATER_POND, 0.5, 200.0).pumped, Some(Source::Water));
        assert_eq!(
            Pump::shown(p.tick(&WATER_POND, 0.5, 200.0), 0.5),
            Shown::OutputFull
        );
    }

    #[test]
    fn water_is_endless_only_where_the_game_refills_it() {
        // Spec 4.3: «вода бесконечна по ванильным правилам, лава конечна».
        assert!(WATER_POND.endless());
        let lone = Under {
            water_beside: 1,
            ..WATER_POND
        };
        assert!(!lone.endless());
        let lava = Under {
            source: Some(Source::Lava),
            ..WATER_POND
        };
        assert!(!lava.endless());
        let floating = Under {
            held_up: false,
            ..WATER_POND
        };
        assert!(!floating.endless());
    }

    #[test]
    fn it_asks_for_energy_only_with_a_source_and_room() {
        let mut p = Pump::default();
        assert_eq!(p.request(&WATER_POND, 4.0), 4.0);
        assert_eq!(p.request(&Under::default(), 4.0), 0.0);
        assert_eq!(p.tick(&Under::default(), 1.0, 200.0).state, State::Idle);
        p.held = Some((WATER.into(), BUCKET));
        assert_eq!(p.request(&WATER_POND, 4.0), 0.0);
        assert_eq!(p.tick(&WATER_POND, 0.0, 200.0).state, State::OutputFull);
    }
}
