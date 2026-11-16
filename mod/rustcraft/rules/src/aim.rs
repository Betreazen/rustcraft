//! Slice 18: where a turret's head and barrel point, and the beam its shot draws (guide `turret`,
//! scene: the head turns and the barrel follows the mob; the shot and the heal are beams).
//!
//! The turret's model points its barrel along +x. Its head turns about the vertical (yaw) and the
//! barrel about the head's z axis (pitch), so a target's direction gives both angles.

/// The turn about the vertical (counter-clockwise seen from above, from +x) and the tilt up, in
/// degrees, that point +x from `from` at `to`.
pub fn aim(from: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    let yaw = (-dz).atan2(dx).to_degrees();
    let pitch = dy.atan2(dx.hypot(dz)).to_degrees();
    (yaw as f32, pitch as f32)
}

/// Turns smaller than this are not sent to clients again, degrees.
pub const AIM_STEP: f32 = 4.0;

/// Whether the aim moved enough since `old` to tell clients.
pub fn aim_changed(old: (f32, f32), new: (f32, f32)) -> bool {
    let turn = (new.0 - old.0 + 540.0).rem_euclid(360.0) - 180.0;
    turn.abs() >= AIM_STEP || (new.1 - old.1).abs() >= AIM_STEP
}

/// Points along the line from `from` to `to`, one each `step` blocks, the last on `to`.
pub fn beam(from: [f64; 3], to: [f64; 3], step: f64) -> Vec<[f64; 3]> {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if !length.is_finite() || step <= 0.0 {
        return Vec::new();
    }
    let n = ((length / step).ceil() as usize).clamp(1, 256);
    (1..=n)
        .map(|i| {
            let k = i as f64 / n as f64;
            [from[0] + d[0] * k, from[1] + d[1] * k, from[2] + d[2] * k]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn the_head_turns_to_the_target_and_the_barrel_tilts() {
        let o = [0.5, 1.0, 0.5];
        assert!(near(aim(o, [5.5, 1.0, 0.5]).0, 0.0), "east is straight ahead");
        assert!(near(aim(o, [0.5, 1.0, -4.5]).0, 90.0), "north is a quarter turn left");
        assert!(near(aim(o, [0.5, 1.0, 5.5]).0, -90.0));
        assert!(near(aim(o, [5.5, 6.0, 0.5]).1, 45.0), "up");
        assert!(aim(o, [5.5, 0.0, 0.5]).1 < 0.0, "down");
    }

    #[test]
    fn small_turns_are_not_sent() {
        assert!(!aim_changed((10.0, 0.0), (12.0, 1.0)));
        assert!(aim_changed((10.0, 0.0), (15.0, 0.0)));
        assert!(!aim_changed((179.0, 0.0), (-179.0, 0.0)), "across the back is a small turn");
        assert!(aim_changed((0.0, 0.0), (0.0, -5.0)));
    }

    #[test]
    fn a_beam_runs_to_its_target() {
        let b = beam([0.0, 0.0, 0.0], [6.0, 0.0, 0.0], 0.5);
        assert_eq!(b.len(), 12);
        assert_eq!(b[11], [6.0, 0.0, 0.0]);
        assert!(beam([0.0; 3], [1000.0, 0.0, 0.0], 0.5).len() <= 256);
    }
}
