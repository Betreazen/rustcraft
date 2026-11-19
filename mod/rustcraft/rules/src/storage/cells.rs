//! Which cells make one storage, and which storage a terminal shows. Cells that touch face to face
//! are one storage (S1); its contents are kept apart from the cells, under an id that each cell's
//! block entity also keeps, so a storage keeps its items whichever of its cells is broken. A new
//! cell that joins two storages makes one of them, holding what both held; a storage split in two
//! keeps its items in the bigger part, and the other part starts empty. Nothing is lost either way.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use crate::geom::{Face, Pos, step};

/// One storage: its id, its cells, and the ids of the storages that joined it since last time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub id: u32,
    pub cells: BTreeSet<Pos>,
    pub merged: Vec<u32>,
}

impl Group {
    /// The block that stands for the whole storage on an item net: its first cell.
    pub fn anchor(&self) -> Pos {
        self.cells.first().copied().unwrap_or_default()
    }
}

/// Joins `cells` (each with the storage id its block entity keeps, if any) into storages. A
/// storage keeps the smallest id its cells name; ids taken already go to the bigger part, and a
/// part without an id of its own gets `next_id`, which then moves on.
pub fn group(cells: &BTreeMap<Pos, Option<u32>>, next_id: &mut u32) -> Vec<Group> {
    if let Some(max) = cells.values().flatten().max() {
        *next_id = (*next_id).max(max + 1);
    }
    let mut parts = components(cells.keys().copied().collect());
    // The bigger part first, so it keeps the id when a storage splits.
    parts.sort_by(|a, b| {
        b.len()
            .cmp(&a.len())
            .then_with(|| a.first().cmp(&b.first()))
    });
    let mut claimed = BTreeSet::new();
    parts
        .into_iter()
        .map(|part| {
            let ids: BTreeSet<u32> = part
                .iter()
                .filter_map(|c| cells.get(c).copied().flatten())
                .filter(|id| !claimed.contains(id))
                .collect();
            claimed.extend(ids.iter().copied());
            let mut ids = ids.into_iter();
            let id = ids.next().unwrap_or_else(|| {
                let id = *next_id;
                *next_id += 1;
                id
            });
            Group {
                id,
                cells: part,
                merged: ids.collect(),
            }
        })
        .collect()
}

/// Cells joined face to face.
fn components(mut left: BTreeSet<Pos>) -> Vec<BTreeSet<Pos>> {
    let mut parts = Vec::new();
    while let Some(start) = left.pop_first() {
        let mut part = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(at) = queue.pop_front() {
            for face in Face::ALL {
                let next = step(at, face);
                if left.remove(&next) {
                    part.insert(next);
                    queue.push_back(next);
                }
            }
        }
        parts.push(part);
    }
    parts
}

/// The storage a terminal at `terminal` shows: that of the first cell it touches.
pub fn terminal_storage(terminal: Pos, cell_of: &HashMap<Pos, u32>) -> Option<u32> {
    Face::ALL
        .into_iter()
        .find_map(|face| cell_of.get(&step(terminal, face)).copied())
}

/// S6 (slice 18): the storage an extra terminal at `terminal`, touching no cell, shows: one whose
/// blocks (`storages`: each storage's cells and the terminals that touch them) touch a housing
/// of an item net that a housing beside the terminal is on; the lowest id when there are more.
pub fn terminal_on_net(
    terminal: Pos,
    storages: &BTreeMap<u32, Vec<Pos>>,
    topology: &crate::net::topology::Topology,
) -> Option<u32> {
    use crate::net::topology::ChannelKind;
    let beside: BTreeSet<Pos> = Face::ALL.into_iter().map(|f| step(terminal, f)).collect();
    let nets: Vec<&BTreeSet<Pos>> = topology
        .nets
        .iter()
        .filter(|n| n.kind == ChannelKind::Item && n.housings.iter().any(|h| beside.contains(h)))
        .map(|n| &n.housings)
        .collect();
    storages
        .iter()
        .find(|(_, blocks)| {
            blocks.iter().any(|b| {
                Face::ALL
                    .into_iter()
                    .any(|f| nets.iter().any(|n| n.contains(&step(*b, f))))
            })
        })
        .map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(list: &[(Pos, Option<u32>)]) -> BTreeMap<Pos, Option<u32>> {
        list.iter().copied().collect()
    }

    #[test]
    fn an_extra_terminal_on_the_net_shows_the_storage() {
        // S6: housings x 1..=10; cells beside housing 10, a terminal beside housing 2; another
        // net's housing far off.
        use crate::net::topology::{Housing, Layout, Node, analyse};
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        for x in 1..=10 {
            layout.housings.insert([x, 0, 0], Housing::default());
        }
        layout.nodes.insert([50, 0, 0], Node { tier: 1 });
        layout.housings.insert([51, 0, 0], Housing::default());
        let t = analyse(&layout, &crate::tuning::Tuning::default());
        let storages = BTreeMap::from([(4, vec![[10, 1, 0], [11, 1, 0]])]);
        assert_eq!(terminal_on_net([2, 1, 0], &storages, &t), Some(4));
        assert_eq!(terminal_on_net([51, 1, 0], &storages, &t), None, "another net");
        assert_eq!(terminal_on_net([2, 5, 0], &storages, &t), None, "off the magistral");
    }

    #[test]
    fn touching_cells_are_one_storage_with_a_new_id() {
        let mut next = 1;
        let g = group(
            &cells(&[([0, 0, 0], None), ([1, 0, 0], None), ([5, 0, 0], None)]),
            &mut next,
        );
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].cells.len(), 2);
        assert_eq!((g[0].id, g[1].id, next), (1, 2, 3));
        assert_eq!(g[0].anchor(), [0, 0, 0]);
    }

    #[test]
    fn a_cell_that_joins_two_storages_merges_them() {
        let mut next = 9;
        let g = group(
            &cells(&[
                ([0, 0, 0], Some(4)),
                ([1, 0, 0], None),
                ([2, 0, 0], Some(7)),
            ]),
            &mut next,
        );
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].id, g[0].merged.clone()), (4, vec![7]));
        assert_eq!(next, 9);
    }

    #[test]
    fn a_split_storage_keeps_its_id_in_the_bigger_part() {
        // The middle cell of five in a row is gone: three on one side, one on the other.
        let mut next = 2;
        let g = group(
            &cells(&[
                ([0, 0, 0], Some(1)),
                ([2, 0, 0], Some(1)),
                ([3, 0, 0], Some(1)),
                ([4, 0, 0], Some(1)),
            ]),
            &mut next,
        );
        assert_eq!(g.len(), 2);
        assert_eq!((g[0].id, g[0].cells.len()), (1, 3));
        assert_eq!((g[1].id, g[1].cells.len()), (2, 1));
        assert!(g[1].merged.is_empty());
    }

    #[test]
    fn a_new_id_never_repeats_one_a_cell_keeps() {
        let mut next = 1;
        let g = group(
            &cells(&[([0, 0, 0], Some(5)), ([9, 0, 0], None)]),
            &mut next,
        );
        assert_eq!(g.iter().map(|g| g.id).collect::<Vec<_>>(), vec![5, 6]);
    }

    #[test]
    fn a_terminal_shows_the_storage_it_touches() {
        let cell_of = HashMap::from([([0, 0, 0], 3)]);
        assert_eq!(terminal_storage([0, 1, 0], &cell_of), Some(3));
        assert_eq!(terminal_storage([0, 2, 0], &cell_of), None);
    }
}
