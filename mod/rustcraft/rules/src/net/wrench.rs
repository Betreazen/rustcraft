//! What the wrench does to a machine (guide `wrench`): a click turns it a quarter clockwise as seen
//! from above, a sneaking click lifts it as one item with its contents and settings.

use crate::geom::{Face, Pos, step};

/// The horizontal facings in the order a click turns them.
pub const FACINGS: [&str; 4] = ["north", "east", "south", "west"];

/// The facing after one click; none for a facing a click does not turn (up, down, unknown).
pub fn rotate(facing: &str) -> Option<&'static str> {
    let i = FACINGS.iter().position(|f| *f == facing)?;
    Some(FACINGS[(i + 1) % FACINGS.len()])
}

/// The custom-data key a lifted machine's item keeps its block entity under.
pub const LIFTED: &str = "rustcraft:lifted";

/// Machines the wrench neither turns nor lifts: the station (the monorail's network keeps it by
/// place), the portal dock (a click pairs it) and the holo-projector (w2: a click arms it for the
/// two corners of its area, a sneaking click lifts it with its plan, rustcraft_projector).
pub const KEPT: [&str; 3] = ["rustcraft:station", "rustcraft:portal_dock", "rustcraft:projector"];

/// Whether the wrench turns and lifts `block`, given it is a machine.
pub fn handles(block: &str) -> bool {
    !KEPT.contains(&block)
}

/// 0.21 (round 32, SP9; the owner: «писало что на стыке настроить нужно, но его меню вообще не
/// открывалось»): machines whose joint decides everything they do. The machine covers the face of
/// the housing that looks at it, so a wrench click on it opens that joint's window instead of
/// turning it; with no housing beside it, it turns as any machine.
pub const OPENS_JOINT: [&str; 2] = ["rustcraft:trash", "rustcraft:disintegrator"];

/// Whether a wrench click on `block` opens the joint window of a housing beside it first.
pub fn opens_joint(block: &str) -> bool {
    OPENS_JOINT.contains(&block)
}

/// The housing beside the block at `pos` whose joint window a wrench click on the block opens,
/// and the face of that housing that looks at the block: the first beside it in `Face::ALL`
/// order.
pub fn joint_beside(pos: Pos, is_housing: impl Fn(Pos) -> bool) -> Option<(Pos, Face)> {
    Face::ALL.into_iter().find_map(|f| {
        let housing = step(pos, f);
        is_housing(housing).then_some((housing, f.opposite()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_turns_a_machine_clockwise() {
        assert_eq!(rotate("north"), Some("east"));
        assert_eq!(rotate("west"), Some("north"));
        assert_eq!(rotate("up"), None);
    }

    #[test]
    fn the_station_and_the_portal_dock_stay() {
        assert!(!handles("rustcraft:station"));
        assert!(!handles("rustcraft:portal_dock"));
        assert!(handles("rustcraft:furnace"));
    }

    #[test]
    fn the_projector_is_kept_so_its_wrench_sets_corners() {
        assert!(!handles("rustcraft:projector"));
    }

    #[test]
    fn the_trash_and_the_disintegrator_open_their_joint_others_turn() {
        // SP9: the owner could not reach the trash's joint, the trash covered it.
        assert!(opens_joint("rustcraft:trash"));
        assert!(opens_joint("rustcraft:disintegrator"));
        for other in ["rustcraft:furnace", "rustcraft:crusher", "rustcraft:repair", "minecraft:chest"] {
            assert!(!opens_joint(other), "{other}");
        }
    }

    #[test]
    fn the_joint_beside_is_the_housings_face_that_looks_at_the_block() {
        // A trash at the origin, a housing to its west: the housing's east face is the joint.
        let west = [-1, 0, 0];
        assert_eq!(joint_beside([0, 0, 0], |p| p == west), Some((west, Face::XPos)));
        let above = [0, 1, 0];
        assert_eq!(joint_beside([0, 0, 0], |p| p == above), Some((above, Face::YNeg)));
        assert_eq!(joint_beside([0, 0, 0], |_| false), None);
    }
}
