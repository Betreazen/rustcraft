//! The blocks a player looks through, for the diagnosis of the housing they look at. The server
//! knows each player's eyes and view angles but has no ray of its own for them.

use crate::geom::Pos;

/// The unit vector a player looks along, from their yaw and pitch in degrees, as Horizon's
/// `view_vector` computes it (SRC/crates/horizon-client/src/game.rs:3507-3510).
pub fn look_direction(yaw: f32, pitch: f32) -> [f64; 3] {
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}

/// The blocks a ray from `origin` along `dir` passes through, nearest first, the one holding
/// `origin` included, while they start within `reach` blocks (Amanatides and Woo's voxel walk).
/// `dir` need not be normalised; a zero `dir` gives only the origin's block.
pub fn blocks_along(origin: [f64; 3], dir: [f64; 3], reach: f64) -> Vec<Pos> {
    let length = dir.iter().map(|d| d * d).sum::<f64>().sqrt();
    let mut cell = origin.map(|o| o.floor() as i32);
    let mut out = vec![cell];
    if length == 0.0 || !length.is_finite() || reach <= 0.0 {
        return out;
    }
    let unit = dir.map(|d| d / length);
    let step = unit.map(|d| if d > 0.0 { 1 } else { -1 });
    // How far along the ray the next boundary of each axis is, and the distance between two.
    let mut next = [0.0f64; 3];
    let mut delta = [f64::INFINITY; 3];
    for axis in 0..3 {
        if unit[axis] == 0.0 {
            next[axis] = f64::INFINITY;
            continue;
        }
        delta[axis] = 1.0 / unit[axis].abs();
        let boundary = if step[axis] > 0 {
            f64::from(cell[axis]) + 1.0
        } else {
            f64::from(cell[axis])
        };
        next[axis] = (boundary - origin[axis]) / unit[axis];
    }
    loop {
        let axis = (0..3)
            .min_by(|&a, &b| next[a].total_cmp(&next[b]))
            .unwrap_or(0);
        if next[axis] > reach {
            return out;
        }
        cell[axis] += step[axis];
        next[axis] += delta[axis];
        out.push(cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    }

    #[test]
    fn yaw_and_pitch_point_as_in_the_game() {
        // Yaw 0 looks south (+z), 90 west (-x), 180 north, -90 east; pitch 90 looks down.
        assert!(close(look_direction(0.0, 0.0), [0.0, 0.0, 1.0]));
        assert!(close(look_direction(90.0, 0.0), [-1.0, 0.0, 0.0]));
        assert!(close(look_direction(-90.0, 0.0), [1.0, 0.0, 0.0]));
        assert!(close(look_direction(0.0, 90.0), [0.0, -1.0, 0.0]));
    }

    #[test]
    fn a_ray_along_an_axis_visits_each_block() {
        let blocks = blocks_along([0.5, 64.5, 0.5], [1.0, 0.0, 0.0], 3.0);
        assert_eq!(blocks, vec![[0, 64, 0], [1, 64, 0], [2, 64, 0], [3, 64, 0]]);
    }

    #[test]
    fn a_ray_looking_down_reaches_the_floor() {
        // Eyes 1.62 above a player standing on y = 64.
        let blocks = blocks_along([0.5, 65.62, 0.5], look_direction(0.0, 90.0), 5.0);
        assert_eq!(blocks[..3], [[0, 65, 0], [0, 64, 0], [0, 63, 0]]);
        assert_eq!(blocks.last(), Some(&[0, 60, 0]));
    }

    #[test]
    fn a_diagonal_ray_steps_one_face_at_a_time() {
        let blocks = blocks_along([0.2, 0.5, 0.7], [1.0, 0.0, -1.0], 3.0);
        for pair in blocks.windows(2) {
            let moved: i32 = (0..3).map(|i| (pair[1][i] - pair[0][i]).abs()).sum();
            assert_eq!(moved, 1, "{pair:?}");
        }
        // The z boundary is nearer (0.7 against 0.8), so the ray leaves through -z first.
        assert_eq!(
            blocks,
            vec![[0, 0, 0], [0, 0, -1], [1, 0, -1], [1, 0, -2], [2, 0, -2]]
        );
    }

    #[test]
    fn negative_coordinates_floor_down() {
        let blocks = blocks_along([-0.5, 10.0, -3.2], [0.0, 0.0, -1.0], 1.0);
        assert_eq!(blocks, vec![[-1, 10, -4], [-1, 10, -5]]);
    }

    #[test]
    fn no_direction_gives_the_origin_only() {
        assert_eq!(
            blocks_along([1.5, 2.5, 3.5], [0.0; 3], 5.0),
            vec![[1, 2, 3]]
        );
    }
}
