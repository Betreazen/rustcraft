//! Where the capsule rides on a rail, as one geometry both halves share: the server moves the
//! passenger's feet along it, the client draws the guide's capsule model (assets/models/
//! transport.py `capsule`) around them.
//!
//! The model fills a 16-pixel cell in the guide, its cabin 10 × 8 × 8 pixels with a bracket to a
//! rail below, above or beside it. In the game it is drawn MODEL_SCALE times larger, so a seated
//! player (from about half a block over the feet to 1.9 blocks) fits its cabin (0.51 to 1.99
//! blocks over the model's floor). Where it hangs from each kind of rail:
//!
//! - a floor rail: the model's floor on the rail's cell floor, the bracket's clamp on the rail;
//! - a ceiling rail: the bracket's clamp (15.25 pixels up the model) against the ceiling;
//! - a wall rail: the cabin as on a floor, WALL_SHIFT off the wall, its side bracket lowered to
//!   the rail in the middle of the wall's cell and pushed in to reach it.
//!
//! So a cabin on a floor or wall rail needs the block over the rail free, and one on a ceiling
//! rail the block under it (`room_cells`).

use crate::geom::{Face, Pos, step};

/// One model pixel, in blocks, at the guide's size.
pub const PIXEL: f64 = 1.0 / 16.0;
/// How much larger than the guide's cell the game draws the model.
pub const MODEL_SCALE: f64 = 2.5;
/// The passenger's feet over the model's floor, in blocks.
pub const FEET_OVER_FLOOR: f64 = 0.05;
/// The top of the ceiling bracket's clamp, in model pixels.
const CEILING_CLAMP_TOP: f64 = 15.25;
/// How far the side bracket's clamp stands out from the cabin's middle, in model pixels: the
/// clamp's outer face is 0.75 pixels from the cell's side.
const SIDE_CLAMP_OUT: f64 = 8.0 - 0.75;
/// The side bracket's middle over the model's floor, in model pixels.
const SIDE_CLAMP_HEIGHT: f64 = 8.0;
/// How far a cabin on a wall rail keeps off the wall: its half-width less the cell's.
pub const WALL_SHIFT: f64 = 0.15;
/// A seat's rider sits this far over the seat (a pig's passenger point, 0.86875, less a player's
/// 0.6); the server measures it again once the player is seated.
pub const SEAT_RISE: f64 = 0.26875;

/// Blocks per model pixel in the game.
pub fn block_per_pixel() -> f64 {
    PIXEL * MODEL_SCALE
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

/// The blocks the cabin passes through at the rail `cell` besides the rail's own: the one over
/// it on a floor or a wall, the one under it on a ceiling.
pub fn room_cells(cell: Pos, mount: Face) -> Vec<Pos> {
    match mount {
        Face::YPos => vec![step(cell, Face::YNeg)],
        _ => vec![step(cell, Face::YPos)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn a_seated_player_fits_the_cabin() {
        // The cabin's floor and roof (3.25 and 12.75 pixels) around a seated player, who shows
        // from about 0.5 to 1.9 blocks over the feet.
        let floor = 3.25 * block_per_pixel();
        let roof = 12.75 * block_per_pixel();
        assert!(FEET_OVER_FLOOR + 0.5 >= floor);
        assert!(FEET_OVER_FLOOR + 1.9 <= roof);
    }

    #[test]
    fn a_ceiling_cabin_hangs_its_clamp_against_the_ceiling() {
        let feet = feet_at([0, 10, 0], Face::YPos);
        let clamp_top = floor_under(feet[1]) + CEILING_CLAMP_TOP * block_per_pixel();
        assert!(close(clamp_top, 11.0));
        assert_eq!(room_cells([0, 10, 0], Face::YPos), vec![[0, 9, 0]]);
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
    fn a_floor_cabin_needs_the_block_over_its_rail() {
        assert_eq!(room_cells([1, 2, 3], Face::YNeg), vec![[1, 3, 3]]);
        assert_eq!(room_cells([1, 2, 3], Face::ZPos), vec![[1, 3, 3]]);
        assert!(close(feet_at([1, 2, 3], Face::YNeg)[1], 2.05));
    }
}
