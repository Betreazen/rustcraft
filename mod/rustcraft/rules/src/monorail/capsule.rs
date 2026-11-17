//! Where the capsule rides on a rail, as one geometry both halves share: the server moves the
//! passenger's feet along it, the client draws the guide's capsule model (assets/models/
//! transport.py `capsule`) around them.
//!
//! The model fills a 16-pixel cell in the guide, its cabin 10 × 8 × 8 pixels with a bracket to a
//! rail below, above or beside it. The guide draws it inside one cell, so in the game the cabin
//! keeps inside a passage one block wide: MODEL_SCALE makes its 8-pixel width one block. A seated
//! player (from about half a block over the feet to 1.9 blocks, `RIDER`) is 1.4 blocks tall and
//! the cabin 1.19 inside (3.25 to 12.75 pixels), so the rider sits with the cabin's middle at
//! their middle: their shoes and hair show under 2 of their pixels (0.12 blocks) past the floor
//! and the roof. Where it hangs from each kind of rail:
//!
//! - a floor rail: the model's floor on the rail's cell floor, the bracket's clamp on the rail;
//! - a ceiling rail: the bracket's clamp (15.25 pixels up the model) against the ceiling;
//! - a wall rail: the cabin as on a floor, WALL_SHIFT off the wall, its side bracket lowered to
//!   the rail in the middle of the wall's cell and pushed in to reach it; on a stretch straight
//!   up or down the wall it runs along the wall (`along_wall`), not into it.
//!
//! The passage it needs is every block the cabin and its rider touch at each rail of the way
//! (`room_cells`): the one over a floor or wall rail, the one under a ceiling rail; none beside
//! the rail, as the cabin is no wider than a block. Its ends reach a third of a block past the
//! rail's cell along the way, over the next rail of the way, which is checked as its own cell; at
//! a stop and on a corner's arc the cabin turns about its middle there, as the guide's does.

use crate::geom::{Face, Pos};

/// One model pixel, in blocks, at the guide's size.
pub const PIXEL: f64 = 1.0 / 16.0;
/// How much larger than the guide's cell the game draws the model: its cabin, 8 pixels wide, is
/// one block wide.
pub const MODEL_SCALE: f64 = 2.0;
/// The cabin's width, in model pixels (the window strips stand a quarter pixel proud of it).
const CABIN_WIDTH: f64 = 8.0;
/// The cabin's floor and roof over the model's floor, in model pixels.
const CABIN_FLOOR: f64 = 3.25;
const CABIN_ROOF: f64 = 12.75;
/// What of a seated player shows, over their feet, in blocks.
pub const RIDER: (f64, f64) = (0.5, 1.9);
/// The passenger's feet over the model's floor, in blocks: the rider's middle at the cabin's.
pub const FEET_OVER_FLOOR: f64 =
    (CABIN_FLOOR + CABIN_ROOF) / 2.0 * PIXEL * MODEL_SCALE - (RIDER.0 + RIDER.1) / 2.0;
/// The top of the ceiling bracket's clamp, in model pixels.
const CEILING_CLAMP_TOP: f64 = 15.25;
/// How far the side bracket's clamp stands out from the cabin's middle, in model pixels: the
/// clamp's outer face is 0.75 pixels from the cell's side.
const SIDE_CLAMP_OUT: f64 = 8.0 - 0.75;
/// The side bracket's middle over the model's floor, in model pixels.
const SIDE_CLAMP_HEIGHT: f64 = 8.0;
/// How far a cabin on a wall rail keeps off the wall: its half-width less the cell's, none now
/// that it is one block wide.
pub const WALL_SHIFT: f64 = 0.0;
/// A seat's rider sits this far over the seat (a pig's passenger point, 0.86875, less a player's
/// 0.6); the server measures it again once the player is seated.
pub const SEAT_RISE: f64 = 0.26875;
/// How far into a block the cabin or its rider may reach without needing it free: less than a
/// model pixel (the window strips, a quarter pixel proud of the cabin's sides).
const TOUCH: f64 = 1.0 / 16.0;

/// Blocks per model pixel in the game.
pub fn block_per_pixel() -> f64 {
    PIXEL * MODEL_SCALE
}

/// The cabin's half length and its roof over the model's floor, in blocks: 10 model pixels long.
const CABIN_HALF: f64 = 5.0 * PIXEL * MODEL_SCALE;
const CABIN_HEIGHT: f64 = CABIN_ROOF * PIXEL * MODEL_SCALE;

/// The camera a capsule is drawn for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Camera {
    /// The player's own view: whether they ride this capsule and look in first person.
    Own { rides: bool, first_person: bool },
    /// A camera loose from the player (the studio's free camera, which LetsCam's director drives
    /// in a replay), at this place.
    Free([f64; 3]),
}

/// Whether the cabin is left out for `camera`: only when it looks out from inside the cabin, whose
/// passenger's feet are at `feet` (a block model has no inner faces, so from inside it would only
/// hide the world). A replay rides the recorded player in first person while its own camera films,
/// so riding alone does not hide it.
pub fn hidden_from(camera: Camera, feet: [f64; 3]) -> bool {
    match camera {
        Camera::Own {
            rides,
            first_person,
        } => rides && first_person,
        Camera::Free(at) => {
            let floor = floor_under(feet[1]);
            (at[0] - feet[0]).abs() <= CABIN_HALF
                && (at[2] - feet[2]).abs() <= CABIN_HALF
                && (floor..=floor + CABIN_HEIGHT).contains(&at[1])
        }
    }
}

/// The model's floor under a passenger's feet.
pub fn floor_under(feet_y: f64) -> f64 {
    feet_y - FEET_OVER_FLOOR
}

/// Where the passenger's feet are at the rail `cell` mounted on `mount`.
pub fn feet_at(cell: Pos, mount: Face) -> [f64; 3] {
    let [x, y, z] = cell.map(f64::from);
    let centre = [x + 0.5, y, z + 0.5];
    match mount {
        Face::YNeg => [centre[0], y + FEET_OVER_FLOOR, centre[2]],
        Face::YPos => {
            let floor = y + 1.0 - CEILING_CLAMP_TOP * block_per_pixel();
            [centre[0], floor + FEET_OVER_FLOOR, centre[2]]
        }
        wall => {
            let o = wall.offset();
            [
                centre[0] - f64::from(o[0]) * WALL_SHIFT,
                y + FEET_OVER_FLOOR,
                centre[2] - f64::from(o[2]) * WALL_SHIFT,
            ]
        }
    }
}

/// How the side bracket is moved from where the model holds it, in blocks along its own axes:
/// toward the cabin (so its clamp reaches the wall the cabin keeps WALL_SHIFT off) and down (to
/// the rail in the middle of its cell).
pub fn side_bracket_shift() -> (f64, f64) {
    let reach = SIDE_CLAMP_OUT * block_per_pixel();
    let inward = reach - 0.5 - WALL_SHIFT;
    let down = SIDE_CLAMP_HEIGHT * block_per_pixel() - 0.5;
    (inward, down)
}

/// The blocks the cabin and its rider touch at the rail `cell` on `mount`, besides the rail's
/// own, facing along each of `along` (horizontal faces; their sign does not matter): those it
/// reaches into deeper than TOUCH across its width and from its floor (or the rider's shoes) to
/// its roof (or the rider's hair). Along the way it is taken within the rail's cell (see the
/// module's note).
pub fn room_cells(cell: Pos, mount: Face, along: &[Face]) -> Vec<Pos> {
    let feet = feet_at(cell, mount);
    let floor = floor_under(feet[1]);
    let bottom = (floor + CABIN_FLOOR * block_per_pixel()).min(feet[1] + RIDER.0);
    let top = (floor + CABIN_ROOF * block_per_pixel()).max(feet[1] + RIDER.1);
    let half_width = CABIN_WIDTH / 2.0 * block_per_pixel();
    let centre = |a: usize| f64::from(cell[a]) + 0.5;
    let blocks =
        |(lo, hi): (f64, f64)| ((lo + TOUCH).floor() as i32)..=((hi - TOUCH).floor() as i32);
    let axes: &[Face] = if along.is_empty() { &[Face::XPos] } else { along };
    let mut out = Vec::new();
    for &way in axes {
        let span = |a: usize| {
            if way.offset()[a] != 0 {
                (centre(a) - 0.5, centre(a) + 0.5)
            } else {
                (feet[a] - half_width, feet[a] + half_width)
            }
        };
        for x in blocks(span(0)) {
            for y in blocks((bottom, top)) {
                for z in blocks(span(2)) {
                    let p = [x, y, z];
                    if p != cell && !out.contains(&p) {
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

/// The horizontal axes the cabin faces at the rail `cell` (by the cells before and after it on
/// the way): a step's own across it, or for a step straight up or down a wall the axis along the
/// wall (`along_wall`); a step straight up or down from a floor or a ceiling adds none.
pub fn ways_at(cell: Pos, mount: Face, before: Option<Pos>, after: Option<Pos>) -> Vec<Face> {
    let mut out = Vec::new();
    for other in [before, after].into_iter().flatten() {
        let d = [other[0] - cell[0], other[2] - cell[2]];
        let way = if d[0] != 0 {
            Some(Face::XPos)
        } else if d[1] != 0 {
            Some(Face::ZPos)
        } else {
            match mount {
                Face::XNeg | Face::XPos => Some(Face::ZPos),
                Face::ZNeg | Face::ZPos => Some(Face::XPos),
                _ => None,
            }
        };
        if let Some(w) = way
            && !out.contains(&w)
        {
            out.push(w);
        }
    }
    out
}

/// The heading (vanilla's yaw: 0 south, 90 west) of a cabin going straight up or down a rail on
/// `wall`: along the wall, the way of the two nearest to `previous`, so it does not ride with its
/// nose in the wall.
pub fn along_wall(wall: Face, previous: f32) -> f32 {
    let ways: [f32; 2] = match wall {
        Face::XNeg | Face::XPos => [0.0, 180.0],
        _ => [90.0, -90.0],
    };
    let off = |a: f32| {
        let d = (a - previous).rem_euclid(360.0);
        d.min(360.0 - d)
    };
    if off(ways[0]) <= off(ways[1]) {
        ways[0]
    } else {
        ways[1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn only_a_camera_inside_the_cabin_hides_it() {
        let feet = feet_at([10, 65, 3], Face::YNeg);
        // Its own rider in first person: hidden; in third person or another player: drawn.
        let own = |rides, first_person| Camera::Own {
            rides,
            first_person,
        };
        assert!(hidden_from(own(true, true), feet));
        assert!(!hidden_from(own(true, false), feet));
        assert!(!hidden_from(own(false, true), feet));
        // A replay's director camera a few blocks off: drawn, though the replay rides in first
        // person; the director's first-person framing at the rider's eye: hidden.
        assert!(!hidden_from(Camera::Free([14.0, 66.5, 5.0]), feet));
        assert!(!hidden_from(Camera::Free([10.5, 67.0, 3.5]), feet));
        assert!(hidden_from(Camera::Free([10.5, 66.4, 3.5]), feet));
        assert!(hidden_from(Camera::Free([11.0, 65.2, 3.0]), feet));
    }

    #[test]
    fn the_cabin_keeps_inside_a_one_block_passage() {
        // The guide's capsule fills one cell; in the game its cabin is one block wide.
        assert!(close(CABIN_WIDTH * block_per_pixel(), 1.0));
        assert!(close(
            WALL_SHIFT,
            (CABIN_WIDTH / 2.0 * block_per_pixel() - 0.5).max(0.0)
        ));
        // A rider shows past the floor and the roof by the same, under 2 of a player's pixels
        // (1.875 / 32 blocks each).
        let floor = CABIN_FLOOR * block_per_pixel();
        let roof = CABIN_ROOF * block_per_pixel();
        let under = floor - (FEET_OVER_FLOOR + RIDER.0);
        let over = FEET_OVER_FLOOR + RIDER.1 - roof;
        assert!(close(under, over), "{under} {over}");
        assert!(over < 2.0 * 1.875 / 32.0, "{over}");
    }

    #[test]
    fn a_ceiling_cabin_hangs_its_clamp_against_the_ceiling() {
        let feet = feet_at([0, 10, 0], Face::YPos);
        let clamp_top = floor_under(feet[1]) + CEILING_CLAMP_TOP * block_per_pixel();
        assert!(close(clamp_top, 11.0));
        assert_eq!(
            room_cells([0, 10, 0], Face::YPos, &[Face::XPos]),
            vec![[0, 9, 0]]
        );
    }

    #[test]
    fn a_wall_cabin_keeps_off_the_wall_and_its_bracket_reaches_it() {
        // A rail on a wall to the west (its block at x - 1).
        let feet = feet_at([4, 10, 4], Face::XNeg);
        assert!(close(feet[0], 4.5 + WALL_SHIFT));
        assert!(close(feet[2], 4.5));
        let (inward, down) = side_bracket_shift();
        let clamp = feet[0] - SIDE_CLAMP_OUT * block_per_pixel() + inward;
        assert!(close(clamp, 4.0), "{clamp}");
        let clamp_middle = floor_under(feet[1]) + SIDE_CLAMP_HEIGHT * block_per_pixel() - down;
        assert!(close(clamp_middle, 10.5));
    }

    #[test]
    fn the_room_is_every_block_the_cabin_and_its_rider_touch() {
        // On a floor or a wall: the block over the rail, nothing beside it whichever way it
        // faces, nothing under the floor.
        let ways: [&[Face]; 4] = [&[Face::XPos], &[Face::ZPos], &[Face::XPos, Face::ZPos], &[]];
        for along in ways {
            assert_eq!(room_cells([1, 2, 3], Face::YNeg, along), vec![[1, 3, 3]]);
            assert_eq!(room_cells([1, 2, 3], Face::ZPos, along), vec![[1, 3, 3]]);
        }
        assert!(close(feet_at([1, 2, 3], Face::YNeg)[1], 2.0 + FEET_OVER_FLOOR));
        // It reaches the next block up deeper than TOUCH, but neither the side blocks nor the
        // second block up.
        let half = CABIN_WIDTH / 2.0 * block_per_pixel();
        assert!(half - 0.5 < TOUCH);
        let top = FEET_OVER_FLOOR + RIDER.1;
        assert!(top > 1.0 + TOUCH && top < 2.0 - TOUCH);
    }

    #[test]
    fn the_cabin_faces_its_way_and_along_a_wall_going_up() {
        let c = [5, 5, 5];
        assert_eq!(
            ways_at(c, Face::YNeg, Some([4, 5, 5]), Some([5, 5, 6])),
            vec![Face::XPos, Face::ZPos]
        );
        // Up a wall to the east: it faces along the wall, north or south.
        assert_eq!(
            ways_at(c, Face::XPos, Some([5, 4, 5]), Some([5, 6, 5])),
            vec![Face::ZPos]
        );
        // From a floor rail up to a wall rail: only the way it came in.
        assert_eq!(
            ways_at(c, Face::YNeg, Some([4, 5, 5]), Some([5, 6, 5])),
            vec![Face::XPos]
        );
        // Coming east (yaw -90) to a wall to the east, it turns to the nearer of north and south.
        assert_eq!(along_wall(Face::XPos, -100.0), 180.0);
        assert_eq!(along_wall(Face::XPos, -80.0), 0.0);
        assert_eq!(along_wall(Face::ZNeg, 10.0), 90.0);
        assert_eq!(along_wall(Face::ZNeg, 260.0), -90.0);
    }
}
