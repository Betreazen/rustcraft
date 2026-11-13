//! The station's window (docs/showcase/js/ui.js, window `station`): its name, the scheme of the
//! rail network with the stations as dots (T7), a list of the stations with distance and time,
//! the station's energy, and the trip to the chosen station with its cost and «Ехать». Only
//! «Ехать» reaches the server (a menu button naming the station by its place in the listing the
//! player was sent); choosing a station, the list's pages and typing the name are the window's
//! own.

use crate::geom::Pos;

pub const WIDTH: f32 = 214.0;
pub const NAME: Rect = [8.0, 5.0, 198.0, 12.0];
pub const SCHEME: Rect = [8.0, 20.0, 198.0, 62.0];
/// The list: ROWS rows under the scheme, with page arrows beside them.
pub const ROWS: usize = 3;
pub const ROW_Y: f32 = 85.0;
pub const ROW_H: f32 = 10.0;
pub const ROW_W: f32 = 184.0;
pub const PREV: Rect = [194.0, 85.0, 12.0, 14.0];
pub const NEXT: Rect = [194.0, 101.0, 12.0, 14.0];
pub const ENERGY_LABEL_Y: f32 = 119.0;
pub const GO: Rect = [162.0, 116.0, 44.0, 12.0];
pub const ENERGY: Rect = [8.0, 130.0, 198.0, 5.0];
/// The chosen trip's cost, or why there is none.
pub const STATUS: Rect = [8.0, 138.0, 198.0, 11.0];
pub const INVENTORY: [f32; 2] = [26.0, 160.0];
pub const INVENTORY_LABEL_Y: f32 = INVENTORY[1] - 10.0;
pub const HEIGHT: f32 = INVENTORY[1] + 80.0;
/// A dot of the scheme takes a click this far from its middle.
pub const DOT_REACH: f32 = 6.0;
/// The scheme keeps this much room round its lines.
const MARGIN: f32 = 8.0;
/// The menu button of «Ехать» to the listing's destination i is GO_BASE + i.
pub const GO_BASE: i32 = 1000;

/// A box: left, top, width, height.
pub type Rect = [f32; 4];

/// What a click in the window reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Name,
    Scheme,
    Row(usize),
    Prev,
    Next,
    Go,
}

pub fn inside(r: Rect, at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

pub fn row_rect(i: usize) -> Rect {
    [8.0, ROW_Y + ROW_H * i as f32, ROW_W, ROW_H]
}

/// The part of the window at `at`, in the window's own units.
pub fn part_at(at: [f32; 2]) -> Option<Part> {
    if inside(NAME, at) {
        return Some(Part::Name);
    }
    if inside(SCHEME, at) {
        return Some(Part::Scheme);
    }
    if let Some(i) = (0..ROWS).find(|&i| inside(row_rect(i), at)) {
        return Some(Part::Row(i));
    }
    [(PREV, Part::Prev), (NEXT, Part::Next), (GO, Part::Go)]
        .into_iter()
        .find(|(r, _)| inside(*r, at))
        .map(|(_, p)| p)
}

/// The menu button of «Ехать» to destination `index`, and back.
pub fn go_button(index: usize) -> i32 {
    GO_BASE + index as i32
}

pub fn go_index(button: i32) -> Option<usize> {
    usize::try_from(button.checked_sub(GO_BASE)?).ok()
}

/// How many pages the list has for `count` stations, at least one.
pub fn pages(count: usize) -> usize {
    count.div_ceil(ROWS).max(1)
}

/// How the scheme fits the world's x and z into its box: north up, one scale for both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    min: [f32; 2],
    scale: f32,
    origin: [f32; 2],
}

impl Projection {
    /// The projection that fits every point of `points` (x, z) into SCHEME's box.
    pub fn fit(points: impl IntoIterator<Item = [i32; 2]>) -> Projection {
        let mut lo = [f32::MAX; 2];
        let mut hi = [f32::MIN; 2];
        for p in points {
            for a in 0..2 {
                lo[a] = lo[a].min(p[a] as f32);
                hi[a] = hi[a].max(p[a] as f32);
            }
        }
        if lo[0] > hi[0] {
            lo = [0.0; 2];
            hi = [0.0; 2];
        }
        let room = [SCHEME[2] - 2.0 * MARGIN, SCHEME[3] - 2.0 * MARGIN];
        let span = [(hi[0] - lo[0]).max(1.0), (hi[1] - lo[1]).max(1.0)];
        let scale = (room[0] / span[0]).min(room[1] / span[1]).min(12.0);
        // Centred in the box.
        let used = [span[0] * scale, span[1] * scale];
        let origin = [
            SCHEME[0] + MARGIN + (room[0] - used[0]) / 2.0,
            SCHEME[1] + MARGIN + (room[1] - used[1]) / 2.0,
        ];
        Projection {
            min: lo,
            scale,
            origin,
        }
    }

    /// Where the middle of block column (x, z) is drawn, in window units.
    pub fn at(&self, p: [i32; 2]) -> [f32; 2] {
        [
            self.origin[0] + (p[0] as f32 - self.min[0]) * self.scale,
            self.origin[1] + (p[1] as f32 - self.min[1]) * self.scale,
        ]
    }
}

/// The (x, z) of a station block, for the scheme.
pub fn column(p: Pos) -> [i32; 2] {
    [p[0], p[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_fits_the_screen() {
        const { assert!(HEIGHT <= 240.0) };
        const { assert!(STATUS[1] + STATUS[3] <= INVENTORY_LABEL_Y) };
        const { assert!(GO[1] + GO[3] <= ENERGY[1]) };
        assert!(
            row_rect(ROWS - 1)[1] + ROW_H <= ENERGY_LABEL_Y,
            "the list runs into the energy"
        );
    }

    #[test]
    fn clicks_find_their_part() {
        assert_eq!(part_at([20.0, 8.0]), Some(Part::Name));
        assert_eq!(part_at([50.0, 50.0]), Some(Part::Scheme));
        assert_eq!(part_at([20.0, 96.0]), Some(Part::Row(1)));
        assert_eq!(part_at([200.0, 90.0]), Some(Part::Prev));
        assert_eq!(part_at([200.0, 110.0]), Some(Part::Next));
        assert_eq!(part_at([180.0, 120.0]), Some(Part::Go));
        assert_eq!(part_at([100.0, 140.0]), None);
    }

    #[test]
    fn go_buttons_name_their_destination() {
        assert_eq!(go_index(go_button(7)), Some(7));
        assert_eq!(go_index(3), None);
        assert_eq!(pages(0), 1);
        assert_eq!(pages(7), 3);
    }

    #[test]
    fn the_scheme_keeps_north_up_and_fits_its_box() {
        let pts = [[0, 0], [100, 0], [50, 50]];
        let p = Projection::fit(pts);
        let a = p.at([0, 0]);
        let b = p.at([100, 0]);
        let c = p.at([50, 50]);
        assert!(a[0] < b[0] && (a[1] - b[1]).abs() < 1e-3);
        assert!(c[1] > a[1]);
        for q in [a, b, c] {
            assert!(inside(SCHEME, q), "{q:?}");
        }
        // A lone station sits in the middle.
        let one = Projection::fit([[5, 5]]).at([5, 5]);
        assert!(inside(SCHEME, one));
    }
}
