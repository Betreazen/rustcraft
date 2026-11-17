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

/// Slice 18, wave 2 (`ui.js` charger: four «Броня» cells): a charged piece leaves them for the
/// hidden output only when the net beside the charger can take it from there.
pub fn to_output(full: bool, housing_beside: bool) -> bool {
    full && housing_beside
}

/// Slice 18, wave 2 (guide `charger`, `charge_plate`): the glow's pulse while it charges, 0..1,
/// `t` in seconds; one beat every [`PULSE_SECONDS`].
pub const PULSE_SECONDS: f32 = 1.5;
pub fn pulse(t: f32) -> f32 {
    0.5 - 0.5 * (std::f32::consts::TAU * t / PULSE_SECONDS).cos()
}

/// The glowing coils of each block's model (`models/block/charger.json`, `charge_plate.json`:
/// the elements with `light_emission`), in texels, grown a little so a glow drawn over them is
/// seen; the palette cell (u0, v0, u1, v1) their texture uses.
pub const CHARGER_COILS: [([f32; 3], [f32; 3]); 16] = [
    ([15.5, 3.5, 2.0], [16.1, 5.0, 14.0]),
    ([2.0, 3.5, 15.5], [14.0, 5.0, 16.1]),
    ([-0.1, 3.5, 2.0], [0.5, 5.0, 14.0]),
    ([2.0, 3.5, -0.1], [14.0, 5.0, 0.5]),
    ([15.5, 7.0, 2.0], [16.1, 8.5, 14.0]),
    ([2.0, 7.0, 15.5], [14.0, 8.5, 16.1]),
    ([-0.1, 7.0, 2.0], [0.5, 8.5, 14.0]),
    ([2.0, 7.0, -0.1], [14.0, 8.5, 0.5]),
    ([15.5, 10.5, 2.0], [16.1, 12.0, 14.0]),
    ([2.0, 10.5, 15.5], [14.0, 12.0, 16.1]),
    ([-0.1, 10.5, 2.0], [0.5, 12.0, 14.0]),
    ([2.0, 10.5, -0.1], [14.0, 12.0, 0.5]),
    ([13.0, 16.65, 3.0], [14.0, 16.9, 13.0]),
    ([3.0, 16.65, 13.0], [13.0, 16.9, 14.0]),
    ([2.0, 16.65, 3.0], [3.0, 16.9, 13.0]),
    ([3.0, 16.65, 2.0], [13.0, 16.9, 3.0]),
];
pub const CHARGER_COIL_UV: [f32; 4] = [8.5, 0.5, 9.5, 1.5];
pub const PLATE_COILS: [([f32; 3], [f32; 3]); 9] = [
    ([3.0, 1.0, 3.0], [13.0, 1.15, 3.75]),
    ([12.25, 1.0, 3.0], [13.0, 1.15, 13.0]),
    ([3.0, 1.0, 12.25], [13.0, 1.15, 13.0]),
    ([3.0, 1.0, 3.0], [3.75, 1.15, 13.0]),
    ([5.5, 1.0, 5.5], [10.5, 1.15, 6.0]),
    ([10.0, 1.0, 5.5], [10.5, 1.15, 10.5]),
    ([5.5, 1.0, 10.0], [10.5, 1.15, 10.5]),
    ([5.5, 1.0, 5.5], [6.0, 1.15, 10.5]),
    ([7.25, 1.0, 7.25], [8.75, 1.2, 8.75]),
];
pub const PLATE_COIL_UV: [f32; 4] = [4.5, 0.5, 5.5, 1.5];
/// How far a glow box stands out of its coil, texels.
pub const GLOW_GROW: f32 = 0.06;

/// A coil box grown by [`GLOW_GROW`] on every side.
pub fn glow_box((from, to): ([f32; 3], [f32; 3])) -> ([f32; 3], [f32; 3]) {
    (from.map(|v| v - GLOW_GROW), to.map(|v| v + GLOW_GROW))
}

/// The glow's tint at pulse `p` (0..1): from the coil's own cyan up to near white.
pub fn glow_tint(p: f32) -> u32 {
    let lerp = |a: u32, b: u32| (a as f32 + (b as f32 - a as f32) * p.clamp(0.0, 1.0)).round() as u32;
    let (r, g, b) = (lerp(0x45, 0xE0), lerp(0x9A, 0xFF), lerp(0xA0, 0xFF));
    (r << 16) | (g << 8) | b
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_charged_piece_leaves_for_the_net_only() {
        assert!(super::to_output(true, true));
        assert!(!super::to_output(true, false));
        assert!(!super::to_output(false, true));
    }

    #[test]
    fn the_pulse_stays_in_range_and_beats() {
        use super::*;
        assert!(pulse(0.0).abs() < 1e-6);
        assert!((pulse(PULSE_SECONDS / 2.0) - 1.0).abs() < 1e-5);
        assert!((pulse(PULSE_SECONDS) - pulse(0.0)).abs() < 1e-5);
        for i in 0..100 {
            let p = pulse(i as f32 * 0.07);
            assert!((0.0..=1.0).contains(&p));
        }
        assert!(glow_tint(1.0) > glow_tint(0.0));
        // Every grown coil stays within a block and a texel of it.
        for c in CHARGER_COILS.iter().chain(&PLATE_COILS) {
            let (f, t) = glow_box(*c);
            assert!(f.iter().zip(&t).all(|(a, b)| a < b && *a > -1.0 && *b < 17.0));
        }
    }

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
