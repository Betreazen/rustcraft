//! R1: the rifle's shot, straight from the eyes up to 64 blocks, hits the first creature on the
//! line and stops at the first solid block; it does not pass through.

/// A box of a creature: its id and its corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub id: u32,
    pub lo: [f64; 3],
    pub hi: [f64; 3],
}

/// How far along `dir` (unit) from `origin` the ray enters the box, if it does within `reach`.
pub fn enters(origin: [f64; 3], dir: [f64; 3], lo: [f64; 3], hi: [f64; 3], reach: f64) -> Option<f64> {
    let mut t0 = 0.0f64;
    let mut t1 = reach;
    for a in 0..3 {
        if dir[a].abs() < 1e-12 {
            if origin[a] < lo[a] || origin[a] > hi[a] {
                return None;
            }
            continue;
        }
        let (mut near, mut far) = ((lo[a] - origin[a]) / dir[a], (hi[a] - origin[a]) / dir[a]);
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        t0 = t0.max(near);
        t1 = t1.min(far);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// The creature the shot hits: the nearest box the ray enters before `wall` (how far the first
/// solid block is) and `reach`.
pub fn hit(origin: [f64; 3], dir: [f64; 3], targets: &[Target], wall: f64, reach: f64) -> Option<u32> {
    let limit = wall.min(reach);
    targets
        .iter()
        .filter_map(|t| enters(origin, dir, t.lo, t.hi, limit).map(|d| (t.id, d)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(id: u32, x: f64) -> Target {
        Target { id, lo: [x - 0.3, 0.0, -0.3], hi: [x + 0.3, 1.8, 0.3] }
    }

    #[test]
    fn the_first_creature_on_the_line_is_hit_and_walls_stop_the_shot() {
        let o = [0.0, 1.6, 0.0];
        let d = [1.0, 0.0, 0.0];
        let t = [at(2, 20.0), at(1, 10.0), at(3, 70.0)];
        assert_eq!(hit(o, d, &t, f64::INFINITY, 64.0), Some(1));
        assert_eq!(hit(o, d, &t, 5.0, 64.0), None, "a wall at 5 blocks");
        assert_eq!(hit(o, d, &[at(3, 70.0)], f64::INFINITY, 64.0), None, "out of reach");
        assert_eq!(hit(o, [0.0, 0.0, 1.0], &t, f64::INFINITY, 64.0), None, "looking away");
    }
}
