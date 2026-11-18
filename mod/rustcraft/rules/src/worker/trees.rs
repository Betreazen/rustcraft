//! What the saw fells (guide `tool_saw`, spec 4.7): a tree whose log is in its zone. A tree is the
//! logs joined to that log (by faces, edges and corners, as the base game's trees grow) that
//! touch leaves the game placed itself (`persistent=false`): a log wall of a house has none and
//! stays. The logs go top down, then the leaves round them; logs and saplings go to the hold, and
//! a sapling from the hold is planted where the tree stood on dirt.
//!
//! 0.18.5 (the owner's field test: a row of birch trunks beside the rail, their leaves decayed,
//! was ridden past): a bare trunk is a tree too (`bare_trunk`). Logs of one wood standing on
//! natural ground and touching nothing a player builds with: no planks, glass, doors, stone or
//! placed leaves next to any of them. A log wall of a house touches those and stays.
//!
//! 0.18.5 (the owner: «работники всё делают сами»): with saplings in its hold the saw plants free
//! ground of its zone (`spot`) so that each tree has room to grow: nothing but air and wild plants
//! in the eight cells round the spot (a free cell between trees), open sky six blocks up; dark and
//! pale oak as a 2×2.

use super::BlockView;
use crate::geom::{Face, Pos, step};
use std::collections::{BTreeSet, VecDeque};

/// A tree to fell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    /// Logs, highest first.
    pub logs: Vec<Pos>,
    /// Its leaves, farthest from the logs first.
    pub leaves: Vec<Pos>,
    /// The lowest logs that stand on dirt: where saplings go.
    pub roots: Vec<Pos>,
    /// The log block of its root, for the sapling that suits it.
    pub wood: String,
}

fn natural_leaves(view: &dyn BlockView, p: Pos) -> bool {
    view.is_leaves(p) && view.prop(p, "persistent").as_deref() != Some("true")
}

/// The tree whose log is at `start`, at most `max_logs` logs and leaves `leaf_reach` steps from
/// them; None for no log there or logs without natural leaves.
pub fn find(view: &dyn BlockView, start: Pos, max_logs: usize, leaf_reach: i32) -> Option<Tree> {
    if !view.is_log(start) {
        return None;
    }
    let mut logs = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let n = [p[0] + dx, p[1] + dy, p[2] + dz];
                    if logs.len() < max_logs && !logs.contains(&n) && view.is_log(n) {
                        logs.insert(n);
                        queue.push_back(n);
                    }
                }
            }
        }
    }
    // The leaves: steps of faces from the logs, natural leaves only.
    let mut leaves: Vec<(i32, Pos)> = Vec::new();
    let mut seen: BTreeSet<Pos> = logs.clone();
    let mut frontier: Vec<Pos> = logs.iter().copied().collect();
    for depth in 1..=leaf_reach {
        let mut next = Vec::new();
        for p in frontier {
            for f in Face::ALL {
                let n = step(p, f);
                if seen.insert(n) && natural_leaves(view, n) {
                    leaves.push((depth, n));
                    next.push(n);
                }
            }
        }
        frontier = next;
    }
    if leaves.is_empty() {
        return bare_trunk(view, start, max_logs);
    }
    let lowest = logs.iter().map(|p| p[1]).min().unwrap_or(start[1]);
    let roots: Vec<Pos> = logs
        .iter()
        .copied()
        .filter(|p| p[1] == lowest && view.is_dirt(step(*p, Face::YNeg)))
        .collect();
    let wood = view.block(roots.first().copied().unwrap_or(start));
    let mut logs: Vec<Pos> = logs.into_iter().collect();
    logs.sort_by(|a, b| b[1].cmp(&a[1]).then(a.cmp(b)));
    leaves.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    Some(Tree {
        logs,
        leaves: leaves.into_iter().map(|(_, p)| p).collect(),
        roots,
        wood,
    })
}

/// The ground a tree grows on: a bare trunk must stand on it.
const NATURAL_GROUND: [&str; 9] = [
    "minecraft:grass_block",
    "minecraft:dirt",
    "minecraft:podzol",
    "minecraft:coarse_dirt",
    "minecraft:rooted_dirt",
    "minecraft:mycelium",
    "minecraft:moss_block",
    "minecraft:mud",
    "minecraft:sand",
];

/// What grows or lies round a tree in the wild and may touch a bare trunk's logs.
const WILD: [&str; 41] = [
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:short_grass",
    "minecraft:grass",
    "minecraft:tall_grass",
    "minecraft:fern",
    "minecraft:large_fern",
    "minecraft:dead_bush",
    "minecraft:bush",
    "minecraft:firefly_bush",
    "minecraft:leaf_litter",
    "minecraft:wildflowers",
    "minecraft:pink_petals",
    "minecraft:snow",
    "minecraft:vine",
    "minecraft:glow_lichen",
    "minecraft:moss_carpet",
    "minecraft:pale_moss_carpet",
    "minecraft:pale_hanging_moss",
    "minecraft:hanging_roots",
    "minecraft:cocoa",
    "minecraft:bee_nest",
    "minecraft:dandelion",
    "minecraft:poppy",
    "minecraft:blue_orchid",
    "minecraft:allium",
    "minecraft:azure_bluet",
    "minecraft:red_tulip",
    "minecraft:orange_tulip",
    "minecraft:white_tulip",
    "minecraft:pink_tulip",
    "minecraft:oxeye_daisy",
    "minecraft:cornflower",
    "minecraft:lily_of_the_valley",
    "minecraft:sunflower",
    "minecraft:lilac",
    "minecraft:rose_bush",
    "minecraft:peony",
    "minecraft:brown_mushroom",
    "minecraft:red_mushroom",
    "minecraft:sweet_berry_bush",
];

/// Whether `block` at `p` may touch a bare trunk: wild plants, saplings, snow, natural ground,
/// leaves the game placed, and the monorail the saw rides beside it; never a block a player
/// builds with, nor placed leaves.
fn wild_beside(view: &dyn BlockView, p: Pos, block: &str) -> bool {
    WILD.contains(&block)
        || block.starts_with("rustcraft:monorail")
        || NATURAL_GROUND.contains(&block)
        || is_sapling(block)
        || natural_leaves(view, p)
}

/// 0.18.5: the bare trunk whose log is at `start`: the logs of its wood joined to it (by faces,
/// edges and corners, at most `max_logs`), the lowest standing on natural ground, none of them
/// touching anything but each other and what grows wild (`wild_beside`). None for a log wall,
/// logs on a building block, or more logs than a tree has.
pub fn bare_trunk(view: &dyn BlockView, start: Pos, max_logs: usize) -> Option<Tree> {
    if !view.is_log(start) {
        return None;
    }
    let wood = view.block(start);
    let mut logs = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let n = [p[0] + dx, p[1] + dy, p[2] + dz];
                    if logs.contains(&n) || view.block(n) != wood {
                        continue;
                    }
                    if logs.len() >= max_logs {
                        // More logs than a tree has: a building.
                        return None;
                    }
                    logs.insert(n);
                    queue.push_back(n);
                }
            }
        }
    }
    for &p in &logs {
        for f in Face::ALL {
            let n = step(p, f);
            if !logs.contains(&n) && !wild_beside(view, n, &view.block(n)) {
                return None;
            }
        }
    }
    let lowest = logs.iter().map(|p| p[1]).min()?;
    let bottom: Vec<Pos> = logs.iter().copied().filter(|p| p[1] == lowest).collect();
    let on_ground = |p: &Pos| NATURAL_GROUND.contains(&view.block(step(*p, Face::YNeg)).as_str());
    if !bottom.iter().all(on_ground) {
        return None;
    }
    let roots = bottom
        .into_iter()
        .filter(|p| view.is_dirt(step(*p, Face::YNeg)))
        .collect();
    let mut logs: Vec<Pos> = logs.into_iter().collect();
    logs.sort_by(|a, b| b[1].cmp(&a[1]).then(a.cmp(b)));
    Some(Tree {
        logs,
        leaves: Vec::new(),
        roots,
        wood,
    })
}

/// The ground the saw plants a sapling on.
const PLANT_GROUND: [&str; 6] = [
    "minecraft:grass_block",
    "minecraft:dirt",
    "minecraft:podzol",
    "minecraft:coarse_dirt",
    "minecraft:rooted_dirt",
    "minecraft:moss_block",
];
/// Saplings that grow a tree alone.
const SINGLE: [&str; 6] = [
    "minecraft:oak_sapling",
    "minecraft:birch_sapling",
    "minecraft:spruce_sapling",
    "minecraft:acacia_sapling",
    "minecraft:cherry_sapling",
    "minecraft:jungle_sapling",
];
/// Saplings that grow a tree only four together.
const SQUARE: [&str; 2] = ["minecraft:dark_oak_sapling", "minecraft:pale_oak_sapling"];
/// Open sky over a sapling the saw plants: the young tree's room.
pub const SKY: i32 = 6;

fn open(block: &str) -> bool {
    block == "minecraft:air" || block == "minecraft:cave_air"
}

/// 0.18.5: where the saw plants `sapling` at `pos` of its zone, if there: one cell, or four for a
/// dark or pale oak (`pos` its north-west one). Each cell is air over plant ground
/// (`PLANT_GROUND`, never farmland or a path) with open sky `SKY` blocks up, and nothing but
/// air, wild plants and the monorail stands round the spot on its level: no sapling, log or other
/// block, so trees planted one after another stand a free cell apart.
pub fn spot(view: &dyn BlockView, pos: Pos, sapling: &str) -> Option<Vec<Pos>> {
    let cells: Vec<Pos> = if SINGLE.contains(&sapling) {
        vec![pos]
    } else if SQUARE.contains(&sapling) {
        [[0, 0], [1, 0], [0, 1], [1, 1]]
            .iter()
            .map(|[dx, dz]| [pos[0] + dx, pos[1], pos[2] + dz])
            .collect()
    } else {
        return None;
    };
    for &c in &cells {
        if !PLANT_GROUND.contains(&view.block(step(c, Face::YNeg)).as_str()) {
            return None;
        }
        if !(0..=SKY).all(|dy| open(&view.block([c[0], c[1] + dy, c[2]]))) {
            return None;
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                let n = [c[0] + dx, c[1], c[2] + dz];
                if cells.contains(&n) {
                    continue;
                }
                let b = view.block(n);
                let wild = WILD.contains(&b.as_str()) && b != "minecraft:bee_nest" && b != "minecraft:cocoa";
                if !(wild || b.starts_with("rustcraft:monorail")) {
                    return None;
                }
            }
        }
    }
    Some(cells)
}

/// Whether the saw plants `item` (`spot`).
pub fn plants(item: &str) -> bool {
    SINGLE.contains(&item) || SQUARE.contains(&item)
}

/// How many saplings a spot of `item` takes.
pub fn per_spot(item: &str) -> u32 {
    if SQUARE.contains(&item) { 4 } else { 1 }
}

/// The sapling that grows a tree of `log`: its wood's own, the mangrove's propagule.
pub fn sapling_for(log: &str) -> Option<String> {
    let (ns, path) = log.split_once(':')?;
    let wood = path
        .strip_suffix("_log")
        .or_else(|| path.strip_suffix("_wood"))?;
    let wood = wood.strip_prefix("stripped_").unwrap_or(wood);
    Some(match wood {
        "mangrove" => format!("{ns}:mangrove_propagule"),
        _ => format!("{ns}:{wood}_sapling"),
    })
}

/// The items the saw keeps in its hold to plant, which the unload dock leaves.
pub fn is_sapling(item: &str) -> bool {
    item.ends_with("_sapling") || item.ends_with("_propagule")
}

#[cfg(test)]
mod tests {
    use super::super::tests::World;
    use super::*;

    /// An oak: a trunk of `h` logs on dirt at x 0, z 0 and a crown of leaves round its top.
    fn oak(w: &mut World, h: i32, persistent: &str) {
        w.set([0, 0, 0], "minecraft:dirt", &[]);
        for y in 1..=h {
            w.set([0, y, 0], "minecraft:oak_log", &[]);
        }
        for (x, z) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            w.set(
                [x, h, z],
                "minecraft:oak_leaves",
                &[("persistent", persistent)],
            );
        }
        w.set(
            [0, h + 1, 0],
            "minecraft:oak_leaves",
            &[("persistent", persistent)],
        );
    }

    #[test]
    fn a_tree_is_its_logs_top_down_and_its_leaves() {
        let mut w = World::default();
        oak(&mut w, 4, "false");
        let t = find(&w, [0, 2, 0], 128, 6).unwrap();
        assert_eq!(t.logs.first(), Some(&[0, 4, 0]));
        assert_eq!(t.logs.len(), 4);
        assert_eq!(t.leaves.len(), 5);
        assert_eq!(t.roots, vec![[0, 1, 0]]);
        assert_eq!(
            sapling_for(&t.wood).as_deref(),
            Some("minecraft:oak_sapling")
        );
    }

    #[test]
    fn logs_with_placed_leaves_are_a_building_and_stay() {
        let mut w = World::default();
        oak(&mut w, 4, "true");
        assert_eq!(find(&w, [0, 2, 0], 128, 6), None);
    }

    /// A bare birch trunk of `h` logs at `x`, z 0, on `ground`.
    fn trunk(w: &mut World, x: i32, h: i32, ground: &str) {
        w.set([x, 0, 0], ground, &[]);
        for y in 1..=h {
            w.set([x, y, 0], "minecraft:birch_log", &[("axis", "y")]);
        }
    }

    #[test]
    fn a_bare_trunk_on_grass_is_felled() {
        // The owner's field test: birch trunks 5 to 8 logs tall, their leaves gone.
        let mut w = World::default();
        trunk(&mut w, 0, 7, "minecraft:grass_block");
        w.set([1, 1, 0], "minecraft:short_grass", &[]);
        w.set([0, 1, 1], "minecraft:birch_sapling", &[]);
        // The rail the saw rides right beside it.
        w.set([0, 1, -1], "rustcraft:monorail", &[]);
        w.set([0, 2, -1], "rustcraft:monorail_2", &[]);
        let t = find(&w, [0, 2, 0], 128, 6).expect("a bare trunk is a tree");
        assert_eq!(t.logs.len(), 7);
        assert_eq!(t.logs.first(), Some(&[0, 7, 0]), "top down");
        assert!(t.leaves.is_empty());
        assert_eq!(t.roots, vec![[0, 1, 0]]);
        assert_eq!(sapling_for(&t.wood).as_deref(), Some("minecraft:birch_sapling"));
        // A few leaves the game left on it make it a tree as before.
        w.set([0, 8, 0], "minecraft:birch_leaves", &[("persistent", "false")]);
        assert_eq!(find(&w, [0, 2, 0], 128, 6).unwrap().leaves.len(), 1);
    }

    #[test]
    fn a_bare_trunk_on_sand_is_felled_and_planted_nowhere() {
        let mut w = World::default();
        trunk(&mut w, 0, 5, "minecraft:sand");
        let t = find(&w, [0, 1, 0], 128, 6).unwrap();
        assert_eq!(t.logs.len(), 5);
        assert!(t.roots.is_empty(), "no sapling grows on sand");
    }

    #[test]
    fn a_log_wall_of_a_house_stays_and_a_trunk_beside_it_goes() {
        // A wall of birch logs on dirt with planks and a glass pane in it, and a bare birch trunk
        // a block away from it on grass.
        let mut w = World::default();
        for x in -4..=-2 {
            w.set([x, 0, 0], "minecraft:dirt", &[]);
            for y in 1..=3 {
                w.set([x, y, 0], "minecraft:birch_log", &[]);
            }
        }
        w.set([-3, 2, 1], "minecraft:birch_planks", &[]);
        w.set([-3, 2, -1], "minecraft:glass_pane", &[]);
        trunk(&mut w, 0, 6, "minecraft:grass_block");
        for y in 1..=3 {
            for x in -4..=-2 {
                assert_eq!(find(&w, [x, y, 0], 128, 6), None, "the wall stays");
            }
        }
        assert_eq!(find(&w, [0, 1, 0], 128, 6).unwrap().logs.len(), 6, "the trunk goes");
    }

    #[test]
    fn a_bare_trunk_touching_a_building_stays() {
        let t = |w: &mut World| trunk(w, 0, 5, "minecraft:grass_block");
        for (beside, at) in [
            ("minecraft:oak_planks", [1, 3, 0]),
            ("minecraft:stone", [0, 6, 0]),
            ("minecraft:oak_door", [0, 2, 1]),
            ("minecraft:oak_log", [-1, 2, 0]),
        ] {
            let mut w = World::default();
            t(&mut w);
            w.set(at, beside, &[]);
            assert_eq!(find(&w, [0, 1, 0], 128, 6), None, "beside {beside}");
        }
        // Placed leaves round it make a decoration, not a tree.
        let mut w = World::default();
        t(&mut w);
        w.set([1, 5, 0], "minecraft:birch_leaves", &[("persistent", "true")]);
        assert_eq!(find(&w, [0, 1, 0], 128, 6), None);
        // Logs on stone, no ground under them: a building.
        let mut w = World::default();
        trunk(&mut w, 0, 5, "minecraft:stone");
        assert_eq!(find(&w, [0, 1, 0], 128, 6), None);
        // More logs than a tree has.
        let mut w = World::default();
        trunk(&mut w, 0, 9, "minecraft:dirt");
        assert_eq!(find(&w, [0, 1, 0], 8, 6), None);
        assert!(find(&w, [0, 1, 0], 9, 6).is_some());
    }

    #[test]
    fn saplings_suit_their_wood() {
        assert_eq!(
            sapling_for("minecraft:mangrove_log").as_deref(),
            Some("minecraft:mangrove_propagule")
        );
        assert_eq!(
            sapling_for("minecraft:spruce_log").as_deref(),
            Some("minecraft:spruce_sapling")
        );
        assert_eq!(sapling_for("minecraft:stone"), None);
        assert!(is_sapling("minecraft:birch_sapling"));
    }

    #[test]
    fn the_saw_plants_with_room_round_each_tree() {
        // 0.18.5, the owner: «сажает саженцы в порядке, чтобы деревья хорошо росли».
        let mut w = World::default();
        for x in -3..=5 {
            for z in -3..=3 {
                w.set([x, 0, z], "minecraft:grass_block", &[]);
            }
        }
        let oak = "minecraft:oak_sapling";
        assert_eq!(spot(&w, [2, 1, 2], oak), Some(vec![[2, 1, 2]]));
        assert!(spot(&w, [1, 1, 1], oak).is_some(), "anywhere with room");
        // The monorail beside it and grass round it are no hindrance; a sapling or a log is.
        w.set([2, 1, 1], "rustcraft:monorail", &[]);
        w.set([3, 1, 3], "minecraft:short_grass", &[]);
        assert!(spot(&w, [2, 1, 2], oak).is_some());
        w.set([1, 1, 3], "minecraft:birch_sapling", &[]);
        assert_eq!(spot(&w, [2, 1, 2], oak), None, "a tree next to it");
        w.set([4, 1, -1], "minecraft:oak_log", &[]);
        assert_eq!(spot(&w, [4, 1, 0], oak), None, "a log beside it");
        assert_eq!(spot(&w, [4, 1, -2], oak), None, "a log beside it");
        w.set([0, 1, 2], "minecraft:oak_log", &[]);
        assert_eq!(spot(&w, [0, 1, 2], oak), None, "taken");
        // Farmland and stone are no ground for it; a roof over it is no sky.
        w.set([0, 0, 0], "minecraft:farmland", &[]);
        assert_eq!(spot(&w, [0, 1, 0], oak), None);
        w.set([0, 0, -2], "minecraft:stone", &[]);
        assert_eq!(spot(&w, [0, 1, -2], oak), None);
        w.set([-2, 6, -2], "minecraft:oak_planks", &[]);
        assert_eq!(spot(&w, [-2, 1, -2], oak), None, "a ceiling 5 up");
        assert!(spot(&w, [-2, 1, 0], oak).is_some());
        assert_eq!(spot(&w, [-2, 1, 0], "minecraft:mangrove_propagule"), None, "kinds it knows");
    }

    #[test]
    fn a_dark_oak_is_planted_four_together_or_not_at_all() {
        let mut w = World::default();
        for x in 0..=3 {
            for z in 0..=3 {
                w.set([x, 0, z], "minecraft:grass_block", &[]);
            }
        }
        let dark = "minecraft:dark_oak_sapling";
        assert_eq!(spot(&w, [0, 1, 0], dark).map(|c| c.len()), Some(4));
        assert_eq!(per_spot(dark), 4);
        w.set([1, 0, 1], "minecraft:stone", &[]);
        assert_eq!(spot(&w, [0, 1, 0], dark), None, "one of the four on stone");
        assert!(plants("minecraft:cherry_sapling") && !plants("minecraft:stick"));
    }
}
