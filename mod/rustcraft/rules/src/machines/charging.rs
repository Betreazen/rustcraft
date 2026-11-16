//! Slice 18: the charger's and the charge plate's numbers (guide `charger`, `charge_plate`).
//!
//! - The plate gives each player standing on it up to 128 E/s, from its buffer.
//! - The charger's speed module charges faster and its economy module draws less from its buffer
//!   for each E it gives (`modules::factors`: work ×2 for energy ×2.5, work ×0.85 for ×0.6): it
//!   gives `rate × work` and spends `energy / work` of its buffer an E given.
//! - A player standing on the charger is charged as from its slots.

/// The charge plate's rate for each player on it, E a second (guide `charge_plate`).
pub const PLATE_PER_PLAYER: f64 = 128.0;

/// What a charging block says after a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charged {
    /// It gave energy.
    Working,
    /// Something to charge, but its buffer is empty: «Нет энергии».
    NoEnergy,
    /// Everything on it is charged.
    Full,
    /// Nothing and nobody to charge.
    Empty,
}

/// The step's outcome: whether there was anything to charge, whether any of it needed energy,
/// and what it gave.
pub fn charged(anything: bool, needs: bool, given: f64) -> Charged {
    if !anything {
        Charged::Empty
    } else if given > 1e-9 {
        Charged::Working
    } else if needs {
        Charged::NoEnergy
    } else {
        Charged::Full
    }
}

/// What one player on the plate may get this step of `secs` seconds, from `buffer` E left.
pub fn plate_offer(buffer: f64, secs: f64) -> f64 {
    (PLATE_PER_PLAYER * secs).min(buffer.max(0.0))
}

/// The charger's step of `secs` seconds at `draw` E/s with the modules' `(work, energy)`
/// factors and `buffer` E: (E it may give, E of its buffer an E given costs).
pub fn charger_offer(draw: f64, secs: f64, (work, energy): (f64, f64), buffer: f64) -> (f64, f64) {
    let per = if work > 0.0 { energy / work } else { 1.0 };
    let wanted = draw * secs * work;
    let affordable = if per > 0.0 { buffer.max(0.0) / per } else { wanted };
    (wanted.min(affordable), per)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plate_gives_each_player_128_a_second() {
        assert_eq!(plate_offer(1000.0, 1.0), 128.0);
        assert_eq!(plate_offer(50.0, 1.0), 50.0);
        assert_eq!(plate_offer(-1.0, 1.0), 0.0);
    }

    #[test]
    fn an_empty_buffer_says_no_energy() {
        assert_eq!(charged(true, true, 0.0), Charged::NoEnergy);
        assert_eq!(charged(true, false, 0.0), Charged::Full);
        assert_eq!(charged(true, true, 5.0), Charged::Working);
        assert_eq!(charged(false, false, 0.0), Charged::Empty);
    }

    #[test]
    fn the_chargers_modules_matter() {
        let (plain, per) = charger_offer(128.0, 1.0, (1.0, 1.0), 1000.0);
        assert_eq!((plain, per), (128.0, 1.0));
        let (fast, per) = charger_offer(128.0, 1.0, (2.0, 2.5), 1000.0);
        assert_eq!(fast, 256.0, "a speed module charges twice as fast");
        assert!((per - 1.25).abs() < 1e-9);
        let (slow, per) = charger_offer(128.0, 1.0, (0.85, 0.6), 1000.0);
        assert!((slow - 108.8).abs() < 1e-9);
        assert!(per < 1.0, "an economy module draws less a given E");
        let (short, _) = charger_offer(128.0, 1.0, (2.0, 2.5), 100.0);
        assert!((short - 80.0).abs() < 1e-9, "its buffer limits it");
    }
}
