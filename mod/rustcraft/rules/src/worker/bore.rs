//! Slice 14: the bore (guide `tool_bore`). At the end of its rails it bores the tunnel ahead —
//! the cell ahead and the one over it, three wide with a radius module — closes lava with a block
//! of its hold, fills a gap under the next rail, and lays that rail from its hold; then it rides
//! on. It is the one worker that makes its own way through rock.

use crate::geom::Pos;

/// The rail it lays (tier 1).
pub const RAIL: &str = "rustcraft:monorail";
/// Slice 18: every rail it may carry, by tier (guide «кладёт монорельс из трюма»).
pub const RAILS: [&str; 4] = [
    "rustcraft:monorail",
    "rustcraft:monorail_2",
    "rustcraft:monorail_3",
    "rustcraft:monorail_4",
];

/// Whether `item` is a rail it lays.
pub fn is_rail(item: &str) -> bool {
    RAILS.contains(&item)
}

/// The best rail its hold has (`count` says how many of an item it has): the item and its tier.
pub fn best_rail(count: impl Fn(&str) -> u32) -> Option<(&'static str, u8)> {
    (0..RAILS.len())
        .rev()
        .find(|&i| count(RAILS[i]) > 0)
        .map(|i| (RAILS[i], i as u8 + 1))
}

/// Where the next rail goes, ahead of it at `ahead`: on the level, the gap under it filled, while
/// it has the height module (guide: «Высота держит путь на одном уровне»); without it the way
/// follows the ground down a step where the cell under `ahead` is open and the one under that
/// holds. The cell and whether the cell under it needs filling.
pub fn next_cell(ahead: Pos, open: impl Fn(Pos) -> bool, height_module: bool) -> (Pos, bool) {
    let under = [ahead[0], ahead[1] - 1, ahead[2]];
    if !open(under) {
        return (ahead, false);
    }
    let deeper = [ahead[0], ahead[1] - 2, ahead[2]];
    if !height_module && !open(deeper) {
        return (under, false);
    }
    (ahead, true)
}
/// A tunnel is this tall.
pub const HEIGHT: i32 = 2;

/// What it closes lava and gaps with.
pub fn is_filler(item: &str) -> bool {
    matches!(
        item,
        "minecraft:cobblestone" | "minecraft:cobbled_deepslate" | "minecraft:stone" | "minecraft:dirt" | "minecraft:netherrack" | "minecraft:deepslate"
    )
}

/// The cell ahead of `cell` coming from `from`, on `cell`'s level, along its heading across (a
/// step down it took keeps its heading): none for no heading or a ride straight up or down.
pub fn ahead(from: Pos, cell: Pos) -> Option<Pos> {
    let d = [cell[0] - from[0], cell[2] - from[2]];
    if d[0].abs() + d[1].abs() != 1 {
        return None;
    }
    Some([cell[0] + d[0], cell[1], cell[2] + d[1]])
}

/// The tunnel's size with `tunnel` corridor modules: (cells to each side, height). Each corridor
/// module widens it by a cell each way and raises it by two, up to 9 × 9. Slice 18 (cross): the
/// radius module is not the bore's (guide `mod_radius`, `modules::tool_uses`), only the corridor
/// widens it.
pub fn size(tunnel: u32) -> (i32, i32) {
    let n = tunnel.min(4) as i32;
    let side = n;
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
        assert_eq!(ahead([0, 5, 0], [1, 4, 0]), Some([2, 4, 0]), "after a step down");
        let c = cells([2, 5, 0], [1, 0], 1, 2);
        assert_eq!(c.len(), 6);
        assert_eq!(size(0), (0, 2));
        assert_eq!(size(1), (1, 3));
        assert_eq!(size(4), (4, 9), "9 × 9 at most");
        assert!(!crate::modules::tool_uses(super::super::Tool::Bore, crate::modules::Module::Radius));
        assert_eq!(shell([0, 5, 0], [1, 0], 0, 2).len(), 6);
        assert!(c.contains(&[2, 6, 1]) && c.contains(&[2, 5, -1]));
        assert!(is_filler("minecraft:cobblestone") && !is_filler(RAIL));
    }

    #[test]
    fn the_bore_lays_the_best_rail_it_carries() {
        let hold = |i: &str| u32::from(i == RAILS[0] || i == RAILS[2]);
        assert_eq!(best_rail(hold), Some((RAILS[2], 3)));
        assert_eq!(best_rail(|_| 0), None);
        assert!(is_rail(RAILS[3]) && !is_rail("minecraft:rail"));
    }

    #[test]
    fn the_height_module_keeps_the_level() {
        // Ground one block lower ahead: open at y 4, solid at y 3.
        let open = |p: Pos| p[1] == 4;
        assert_eq!(next_cell([2, 5, 0], open, true), ([2, 5, 0], true), "level, the gap filled");
        assert_eq!(next_cell([2, 5, 0], open, false), ([2, 4, 0], false), "down a step");
        // A deep gap: it stays level and fills even without the module.
        assert_eq!(next_cell([2, 5, 0], |p| p[1] < 5, false), ([2, 5, 0], true));
        assert_eq!(next_cell([2, 5, 0], |_| false, false), ([2, 5, 0], false));
    }

}

/// Wave 2 (guide `mod_height`, rules::modules::height_step): with the height module and a height
/// set, the next rail climbs or drops a step towards it from `level_ahead` (the cell ahead on the
/// rail's level); none when the way stays on its level.
pub fn toward_height(level_ahead: Pos, height_module: bool, target: Option<i32>) -> Option<Pos> {
    let step = crate::modules::height_step(height_module, target, level_ahead[1]);
    (step != 0).then_some([level_ahead[0], level_ahead[1] + step, level_ahead[2]])
}

#[cfg(test)]
mod wave2_height_tests {
    use super::toward_height;

    #[test]
    fn the_bore_climbs_and_drops_a_step_towards_the_height_set() {
        assert_eq!(toward_height([5, 40, 0], true, Some(45)), Some([5, 41, 0]));
        assert_eq!(toward_height([5, 40, 0], true, Some(30)), Some([5, 39, 0]));
        assert_eq!(toward_height([5, 40, 0], true, Some(40)), None, "at the height it stays level");
        assert_eq!(toward_height([5, 40, 0], false, Some(45)), None, "the module is needed");
        assert_eq!(toward_height([5, 40, 0], true, None), None, "no height set: level");
    }
}
