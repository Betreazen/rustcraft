//! What the wrench does to a machine (guide `wrench`): a click turns it a quarter clockwise as seen
//! from above, a sneaking click lifts it as one item with its contents and settings.

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
}
