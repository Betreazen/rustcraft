//! Slice 14: the bore (guide `tool_bore`). At the end of its rails it bores the tunnel ahead —
//! the cell ahead and the one over it, three wide with a radius module — closes lava with a block
//! of its hold, fills a gap under the next rail, and lays that rail from its hold; then it rides
//! on. It is the one worker that makes its own way through rock.

use crate::geom::Pos;

/// The rail it lays (tier 1).
pub const RAIL: &str = "rustcraft:monorail";
/// A tunnel is this tall.
pub const HEIGHT: i32 = 2;

/// What it closes lava and gaps with.
pub fn is_filler(item: &str) -> bool {
    matches!(
        item,
        "minecraft:cobblestone" | "minecraft:cobbled_deepslate" | "minecraft:stone" | "minecraft:dirt" | "minecraft:netherrack" | "minecraft:deepslate"
    )
}

/// The cell ahead of `cell` coming from `from`, on the level: none for no heading or a climb.
pub fn ahead(from: Pos, cell: Pos) -> Option<Pos> {
    let d = [cell[0] - from[0], cell[2] - from[2]];
    if from[1] != cell[1] || d[0].abs() + d[1].abs() != 1 {
        return None;
    }
    Some([cell[0] + d[0], cell[1], cell[2] + d[1]])
}

/// The tunnel's size with `tunnel` corridor modules and a radius module: (cells to each side,
/// height). Each corridor module widens it by a cell each way and raises it by two, up to 9 × 9.
pub fn size(tunnel: u32, radius: bool) -> (i32, i32) {
    let n = tunnel.min(4) as i32;
    let side = n.max(i32::from(radius));
    let height = if n == 0 { HEIGHT } else { (2 * n + 1).max(HEIGHT) };
    (side, height)
}

/// The tunnel's cross-section at `ahead` going along `heading` (unit x or z): `side` cells to
/// each side, `height` up.
pub fn cells(ahead: Pos, heading: [i32; 2], side: i32, height: i32) -> Vec<Pos> {
    let across = [heading[1].abs(), heading[0].abs()];
    (0..height)
        .flat_map(|y| (-side..=side).map(move |k| [ahead[0] + across[0] * k, ahead[1] + y, ahead[2] + across[1] * k]))
        .collect()
}

/// The shell round a cross-section: the floor under it, the walls beside it, the roof over it —
/// what the build module makes good where it is open.
pub fn shell(ahead: Pos, heading: [i32; 2], side: i32, height: i32) -> Vec<Pos> {
    let across = [heading[1].abs(), heading[0].abs()];
    let at = |k: i32, y: i32| [ahead[0] + across[0] * k, ahead[1] + y, ahead[2] + across[1] * k];
    let mut out: Vec<Pos> = (-side..=side).flat_map(|k| [at(k, -1), at(k, height)]).collect();
    out.extend((0..height).flat_map(|y| [at(-side - 1, y), at(side + 1, y)]));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_bores_ahead_on_the_level_and_across_its_heading() {
        assert_eq!(ahead([0, 5, 0], [1, 5, 0]), Some([2, 5, 0]));
        assert_eq!(ahead([0, 5, 0], [0, 6, 0]), None);
        let c = cells([2, 5, 0], [1, 0], 1, 2);
        assert_eq!(c.len(), 6);
        assert_eq!(size(0, false), (0, 2));
        assert_eq!(size(0, true), (1, 2));
        assert_eq!(size(4, false), (4, 9), "9 × 9 at most");
        assert_eq!(shell([0, 5, 0], [1, 0], 0, 2).len(), 6);
        assert!(c.contains(&[2, 6, 1]) && c.contains(&[2, 5, -1]));
        assert!(is_filler("minecraft:cobblestone") && !is_filler(RAIL));
    }
}
