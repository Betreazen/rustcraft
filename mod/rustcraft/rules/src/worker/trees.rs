//! What the saw fells (guide `tool_saw`, spec 4.7): a tree whose log is in its zone. A tree is the
//! logs joined to that log (by faces, edges and corners, as the base game's trees grow) that
//! touch leaves the game placed itself (`persistent=false`): a log wall of a house has none and
//! stays. The logs go top down, then the leaves round them; logs and saplings go to the hold, and
//! a sapling from the hold is planted where the tree stood on dirt.

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
        return None;
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
}
