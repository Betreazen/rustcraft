//! Which piece a rail shows (T2: the shape is chosen by the neighbouring rails, guide `monorail`
//! and spec 18.6). The model is the guide's floor rail (assets/models/transport.py `rail`): a hub
//! in the cell's middle and a half rail to each of four sides, a quarter-circle arc for a corner,
//! and a 45° climbing piece. Its blockstate turns the floor piece onto the rail's mount
//! (`MOUNT_TURN`, the same as vanilla's end rod turns its model by `facing`), so the four sides of
//! the piece are four sides in the world that depend on the mount (`local_sides`).
//!
//! A rail's `shape` state is one of:
//! - `s<mask>`: the halves to show, bit k for local side k (0 to 15). Exactly two halves at a
//!   right angle show as the arc between them; one half or none show as a straight piece;
//! - `c<k>`: the climbing piece turned to local side k (only floor and ceiling rails climb). A
//!   floor climb sits in the lower of its two cells, rising toward k; a ceiling climb sits in the
//!   upper one, under the ceiling, and its piece is the floor's turned over, so it rises away
//!   from local side k.
//!
//! A rail that ends beside a station turns into it (spec 18.6): its one half gets the half
//! toward the station at its side.

use std::collections::BTreeSet;

use super::Network;
use crate::geom::{Face, Pos, step};

/// The shape state's name.
pub const PROPERTY: &str = "shape";
/// The mount state's name; its values are `MOUNT_VALUES`, `Face::ALL`'s order.
pub const MOUNT: &str = "mount";
pub const MOUNT_VALUES: [&str; 6] = ["west", "east", "down", "up", "north", "south"];

/// The value `mount` has in the block's state.
pub fn mount_value(mount: Face) -> &'static str {
    MOUNT_VALUES[usize::from(mount.index())]
}

pub fn mount_of(value: &str) -> Option<Face> {
    MOUNT_VALUES
        .iter()
        .position(|v| *v == value)
        .and_then(|i| Face::from_index(i as u8))
}

/// The blockstate's turn of the floor piece onto a mount: (x, y) in degrees, as vanilla's end
/// rod turns its upright model for each `facing` (the rail faces away from its mount).
pub fn mount_turn(mount: Face) -> (u32, u32) {
    match mount {
        Face::YNeg => (0, 0),
        Face::YPos => (180, 0),
        Face::ZPos => (90, 0),
        Face::ZNeg => (90, 180),
        Face::XNeg => (90, 90),
        Face::XPos => (90, 270),
    }
}

/// The world side local side k (0 to 3) of the floor piece lands on for a rail on `mount`. On the
/// floor: east, south, west, north (the model's x+, y+, x-, y-).
pub fn local_sides(mount: Face) -> [Face; 4] {
    let (x, y) = mount_turn(mount);
    [Face::XPos, Face::ZPos, Face::XNeg, Face::ZNeg].map(|f| turned(turned_x(f, x), y))
}

/// `face` after vanilla's blockstate `x` turn (90 takes up to north and south to up).
fn turned_x(face: Face, degrees: u32) -> Face {
    (0..degrees / 90).fold(face, |f, _| match f {
        Face::YPos => Face::ZNeg,
        Face::ZNeg => Face::YNeg,
        Face::YNeg => Face::ZPos,
        Face::ZPos => Face::YPos,
        other => other,
    })
}

/// `face` after vanilla's blockstate `y` turn (90 takes north to east).
fn turned(face: Face, degrees: u32) -> Face {
    (0..degrees / 90).fold(face, |f, _| match f {
        Face::ZNeg => Face::XPos,
        Face::XPos => Face::ZPos,
        Face::ZPos => Face::XNeg,
        Face::XNeg => Face::ZNeg,
        other => other,
    })
}

/// A rail's piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Halves by local side, bit k for side k.
    Halves(u8),
    /// The climbing piece turned to local side k.
    Climb(u8),
}

impl Shape {
    pub fn value(self) -> String {
        match self {
            Shape::Halves(mask) => format!("s{mask}"),
            Shape::Climb(k) => format!("c{k}"),
        }
    }

    /// Every value the state takes, in the order the block declares them.
    pub fn values() -> Vec<String> {
        (0..16)
            .map(|m| Shape::Halves(m).value())
            .chain((0..4).map(|k| Shape::Climb(k).value()))
            .collect()
    }
}

const HORIZONTAL: [Face; 4] = [Face::XPos, Face::ZPos, Face::XNeg, Face::ZNeg];

fn up(p: Pos, dy: i32) -> Pos {
    [p[0], p[1] + dy, p[2]]
}

/// Whether a rail of `mount` stands at `p`.
fn rail_with(net: &Network, p: Pos, mount: Face) -> bool {
    net.rails.get(&p).is_some_and(|r| r.mount == mount)
}

/// The piece of the rail at `pos`; `stations` are the station blocks it may turn into.
pub fn shape_of(net: &Network, pos: Pos, stations: &BTreeSet<Pos>) -> Shape {
    let Some(rail) = net.rails.get(&pos) else {
        return Shape::Halves(0);
    };
    let sides = local_sides(rail.mount);
    let local = |face: Face| sides.iter().position(|&s| s == face).map(|k| k as u8);
    // The climbing piece: a floor rail with a floor rail one across and one up, or a ceiling rail
    // with a ceiling rail one across the other way and one down.
    match rail.mount {
        Face::YNeg => {
            if let Some(d) = HORIZONTAL
                .into_iter()
                .find(|&d| rail_with(net, up(step(pos, d), 1), Face::YNeg))
                && let Some(k) = local(d)
            {
                return Shape::Climb(k);
            }
        }
        Face::YPos => {
            // Rises toward d: its partner is behind it, one down; the turned piece is the one for
            // the opposite side.
            if let Some(d) = HORIZONTAL
                .into_iter()
                .find(|&d| rail_with(net, up(step(pos, d.opposite()), -1), Face::YPos))
                && let Some(k) = local(d.opposite())
            {
                return Shape::Climb(k);
            }
        }
        _ => {}
    }
    let mut mask = 0u8;
    for (k, &side) in sides.iter().enumerate() {
        let next = step(pos, side);
        let by_face = net.rails.contains_key(&next);
        // The upper end of a floor climb, or the lower end of a ceiling climb, shows a half
        // toward the climbing piece.
        let by_climb = match rail.mount {
            Face::YNeg => rail_with(net, up(next, -1), Face::YNeg),
            Face::YPos => rail_with(net, up(next, 1), Face::YPos),
            _ => false,
        };
        if by_face || by_climb {
            mask |= 1 << k;
        }
    }
    if mask.count_ones() == 1 {
        let k = mask.trailing_zeros() as usize;
        if let Some(turn) = [(k + 1) % 4, (k + 3) % 4]
            .into_iter()
            .find(|&j| stations.contains(&step(pos, sides[j])))
        {
            mask |= 1 << turn;
        }
    }
    Shape::Halves(mask)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{floor_rail, rail};
    use super::*;

    #[test]
    fn the_floor_piece_lands_on_each_mount_as_the_end_rod_does() {
        // Each mount's sides are the four sides around it.
        for mount in Face::ALL {
            let sides = local_sides(mount);
            for s in sides {
                assert_ne!(s, mount);
                assert_ne!(s, mount.opposite());
            }
            assert_eq!(sides[0].opposite(), sides[2]);
            assert_eq!(sides[1].opposite(), sides[3]);
        }
        assert_eq!(
            local_sides(Face::YNeg),
            [Face::XPos, Face::ZPos, Face::XNeg, Face::ZNeg]
        );
        // Turned over about x: south and north swap.
        assert_eq!(
            local_sides(Face::YPos),
            [Face::XPos, Face::ZNeg, Face::XNeg, Face::ZPos]
        );
        // On the wall to the south (the end rod's facing=north): up the wall is local side 1.
        assert_eq!(
            local_sides(Face::ZPos),
            [Face::XPos, Face::YPos, Face::XNeg, Face::YNeg]
        );
        assert_eq!(
            local_sides(Face::XNeg),
            [Face::ZPos, Face::YPos, Face::ZNeg, Face::YNeg]
        );
    }

    #[test]
    fn a_mount_survives_its_state_value() {
        for mount in Face::ALL {
            assert_eq!(mount_of(mount_value(mount)), Some(mount));
        }
        assert_eq!(mount_value(Face::YNeg), "down");
    }

    #[test]
    fn a_straight_floor_rail_shows_both_halves() {
        let mut net = Network::default();
        for x in 0..3 {
            floor_rail(&mut net, [x, 0, 0]);
        }
        let none = BTreeSet::new();
        assert_eq!(shape_of(&net, [1, 0, 0], &none), Shape::Halves(0b0101));
        assert_eq!(shape_of(&net, [0, 0, 0], &none), Shape::Halves(0b0001));
    }

    #[test]
    fn a_corner_shows_two_halves_at_a_right_angle() {
        let mut net = Network::default();
        floor_rail(&mut net, [0, 0, 0]);
        floor_rail(&mut net, [1, 0, 0]);
        floor_rail(&mut net, [1, 0, 1]);
        assert_eq!(
            shape_of(&net, [1, 0, 0], &BTreeSet::new()),
            Shape::Halves(0b0110)
        );
    }

    #[test]
    fn a_rail_ending_beside_a_station_turns_into_it() {
        // Spec 18.6: the track comes from the west and ends; the station is to the south.
        let mut net = Network::default();
        floor_rail(&mut net, [0, 0, 0]);
        floor_rail(&mut net, [1, 0, 0]);
        let stations = BTreeSet::from([[1, 0, 1]]);
        assert_eq!(shape_of(&net, [1, 0, 0], &stations), Shape::Halves(0b0110));
    }

    #[test]
    fn a_floor_climb_sits_in_the_lower_cell() {
        let mut net = Network::default();
        floor_rail(&mut net, [0, 0, 0]);
        floor_rail(&mut net, [1, 0, 0]);
        floor_rail(&mut net, [2, 1, 0]);
        floor_rail(&mut net, [3, 1, 0]);
        let none = BTreeSet::new();
        assert_eq!(shape_of(&net, [1, 0, 0], &none), Shape::Climb(0));
        // The upper end shows its half toward the climb.
        assert_eq!(shape_of(&net, [2, 1, 0], &none), Shape::Halves(0b0101));
    }

    #[test]
    fn a_ceiling_climb_sits_in_the_upper_cell() {
        let mut net = Network::default();
        rail(&mut net, [0, 0, 0], Face::YPos);
        rail(&mut net, [1, 1, 0], Face::YPos);
        let none = BTreeSet::new();
        // [1, 1, 0] rises toward the east: the turned-over piece of local side 2 (west).
        assert_eq!(shape_of(&net, [1, 1, 0], &none), Shape::Climb(2));
        assert_eq!(shape_of(&net, [0, 0, 0], &none), Shape::Halves(0b0001));
    }

    #[test]
    fn a_wall_rail_runs_up_its_wall() {
        let mut net = Network::default();
        for y in 0..3 {
            rail(&mut net, [0, y, 0], Face::ZPos);
        }
        assert_eq!(
            shape_of(&net, [0, 1, 0], &BTreeSet::new()),
            Shape::Halves(0b1010)
        );
    }

    #[test]
    fn the_shape_values_are_twenty() {
        let values = Shape::values();
        assert_eq!(values.len(), 20);
        assert_eq!(values[0], "s0");
        assert_eq!(values[19], "c3");
    }
}
