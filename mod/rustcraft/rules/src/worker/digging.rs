//! What the grab and the scoop take along the way (guide `tool_grab`, `tool_scoop`; spec 4.7,
//! 22). The grab breaks ore blocks in its zone (as the miner knows them) and picks up items lying
//! there; the scoop digs earth, sand, gravel and clay in a strip under the way, but never the
//! blocks the rails stand on, so the way does not fall in.

use crate::geom::Pos;

/// The blocks the scoop digs: «землю, дёрн, песок, гравий, глину».
pub const SCOOPED: [&str; 10] = [
    "minecraft:dirt",
    "minecraft:grass_block",
    "minecraft:coarse_dirt",
    "minecraft:podzol",
    "minecraft:rooted_dirt",
    "minecraft:mycelium",
    "minecraft:sand",
    "minecraft:red_sand",
    "minecraft:gravel",
    "minecraft:clay",
];

/// The tool the grab breaks ore with, for its drops (a player's iron pickaxe would do for every ore
/// but ancient debris; the miner's diamond pickaxe does for all).
pub const GRAB_TOOL: &str = "minecraft:diamond_pickaxe";
/// The scoop's tool: a shovel gives earth from grass, as a player's does.
pub const SCOOP_TOOL: &str = "minecraft:iron_shovel";

/// Whether the grab takes `block`: any ore and ancient debris (spec 4.9 L3 lists them all).
pub fn is_ore(block: &str) -> bool {
    block.ends_with("_ore") || block == "minecraft:ancient_debris"
}

pub fn is_scooped(block: &str) -> bool {
    SCOOPED.contains(&block)
}

/// The cells under rail cell `cell` the scoop looks at, top down: `side` cells aside and `depth`
/// blocks down. `under_rail(p)` says whether `p` holds a rail up, that is lies under a rail of the
/// network in its column within `depth` blocks: those stay.
pub fn scoop_cells(cell: Pos, side: i32, depth: i32, under_rail: impl Fn(Pos) -> bool) -> Vec<Pos> {
    let mut out = Vec::new();
    for dy in 1..=depth {
        for dx in -side..=side {
            for dz in -side..=side {
                let p = [cell[0] + dx, cell[1] - dy, cell[2] + dz];
                if !under_rail(p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// The box the grab picks lying items up from round rail cell `cell`: its zone of `side` cells
/// aside and `height` blocks up, as block coordinates' corners.
pub fn pickup_box(cell: Pos, side: i32, height: i32) -> ([f64; 3], [f64; 3]) {
    let c = cell.map(f64::from);
    let s = f64::from(side);
    (
        [c[0] - s, c[1], c[2] - s],
        [c[0] + s + 1.0, c[1] + f64::from(height), c[2] + s + 1.0],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grab_knows_every_ore_and_nothing_else() {
        for ore in [
            "minecraft:iron_ore",
            "minecraft:deepslate_diamond_ore",
            "minecraft:nether_quartz_ore",
            "minecraft:ancient_debris",
        ] {
            assert!(is_ore(ore), "{ore}");
        }
        for not in [
            "minecraft:stone",
            "minecraft:raw_iron_block",
            "rustcraft:monorail",
        ] {
            assert!(!is_ore(not), "{not}");
        }
    }

    #[test]
    fn the_scoop_digs_earth_and_spares_what_holds_the_rails() {
        assert!(is_scooped("minecraft:grass_block") && is_scooped("minecraft:clay"));
        assert!(!is_scooped("minecraft:stone"));
        // A rail at the origin along x: the column under it stays, the rest of the strip is dug.
        let rails = [[-1, 0, 0], [0, 0, 0], [1, 0, 0]];
        let under = |p: Pos| {
            rails
                .iter()
                .any(|r| r[0] == p[0] && r[2] == p[2] && p[1] < r[1])
        };
        let cells = scoop_cells([0, 0, 0], 1, 3, under);
        assert_eq!(cells.len(), 3 * 6);
        assert!(!cells.contains(&[0, -1, 0]));
        assert!(cells.contains(&[0, -1, 1]) && cells.contains(&[1, -3, -1]));
        // Top down: the first layer first.
        assert!(cells[..6].iter().all(|p| p[1] == -1));
        assert!(cells.iter().all(|p| p[1] < 0));
    }

    #[test]
    fn the_grab_picks_up_round_its_zone() {
        let (lo, hi) = pickup_box([10, 64, -5], 1, 3);
        assert_eq!(lo, [9.0, 64.0, -6.0]);
        assert_eq!(hi, [12.0, 67.0, -3.0]);
    }
}
