//! Where a channel net meets the blocks around it. A joint is a face of a working housing of the
//! net that touches a block with an inventory (M7); an energy port is a source or a machine that
//! touches a housing of an energy net. Housings and nodes touching the net are pipe, not joints.
//! The bundle says which blocks around the magistral have an inventory and their default mode,
//! and what each housing keeps of its joints' settings (per face and per slot, M7); these
//! functions only pair them with the net, so the result does not depend on the game.

use std::collections::{BTreeMap, BTreeSet};

use crate::geom::{Face, Pos, step};
use crate::net::items::{Joint, Mode};
use crate::net::settings::{HousingJoints, JointSetting};
use crate::net::topology::{ChannelNet, Layout};

/// The default direction of an energy joint: a source gives and a machine takes through it.
pub const ENERGY_DEFAULT: Mode = Mode::Both;

/// What an energy port does on its circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Port {
    /// A crystal or a generator: it gives.
    Source,
    /// A machine: it takes.
    Machine,
    /// A battery (E11): it gives and takes, so any direction but «выключен» joins it.
    Storage,
}

/// The blocks next to `net`'s housings, each with the face of the housing that looks at it, in a
/// stable order (housings by position, faces in `Face::ALL` order); pipe is left out.
fn around<'a>(
    net: &'a ChannelNet,
    layout: &'a Layout,
) -> impl Iterator<Item = (Pos, Face, Pos)> + 'a {
    net.housings.iter().flat_map(move |&housing| {
        Face::ALL.into_iter().filter_map(move |face| {
            let block = step(housing, face);
            let pipe = layout.housings.contains_key(&block) || layout.nodes.contains_key(&block);
            (!pipe).then_some((housing, face, block))
        })
    })
}

/// The joints of an item net: every face of its housings that touches a block of
/// `inventories`, with the setting its housing keeps for that face and the net's slot (M7), or
/// else the block's default mode, no filter and priority 0. A block touched by several housings
/// has a joint on each; `items::tick` counts it once.
pub fn item_joints(
    net: &ChannelNet,
    layout: &Layout,
    inventories: &BTreeMap<Pos, Mode>,
    settings: &BTreeMap<Pos, HousingJoints>,
) -> Vec<Joint> {
    around(net, layout)
        .filter_map(|(housing, face, block)| {
            let default = *inventories.get(&block)?;
            let s = settings
                .get(&housing)
                .and_then(|h| h.get(face, net.slot))
                .cloned()
                .unwrap_or_else(|| JointSetting::of_mode(default));
            Some(Joint {
                housing,
                face,
                block,
                mode: s.mode,
                filter: s.filter,
                priority: s.priority,
            })
        })
        .collect()
}

/// The blocks of `ports` (sources and machines) that touch a housing of `net`.
pub fn touching(net: &ChannelNet, layout: &Layout, ports: &BTreeSet<Pos>) -> BTreeSet<Pos> {
    around(net, layout)
        .map(|(_, _, block)| block)
        .filter(|block| ports.contains(block))
        .collect()
}

/// The ports of `ports` on the circuit of energy net `net`: those touching it through at least
/// one joint whose direction lets them work, a source through one that gives (from the block,
/// or both ways) and a machine through one that takes (into the block, or both ways). The
/// direction of each face and slot is the housing's setting or ENERGY_DEFAULT (M7: a machine on
/// an energy joint «в блок» never gives its energy away).
pub fn energy_members(
    net: &ChannelNet,
    layout: &Layout,
    ports: &BTreeMap<Pos, Port>,
    settings: &BTreeMap<Pos, HousingJoints>,
) -> BTreeSet<Pos> {
    around(net, layout)
        .filter_map(|(housing, face, block)| {
            let port = *ports.get(&block)?;
            let mode = settings
                .get(&housing)
                .and_then(|h| h.get(face, net.slot))
                .map_or(ENERGY_DEFAULT, |s| s.mode);
            let works = match port {
                Port::Source => mode.gives(),
                Port::Machine => mode.takes(),
                Port::Storage => mode.gives() || mode.takes(),
            };
            works.then_some(block)
        })
        .collect()
}

/// The faces of `housing` that look at a block in `blocks` and are not pipe: where its joints
/// are, in `Face::ALL` order.
pub fn joint_faces(housing: Pos, layout: &Layout, blocks: &BTreeSet<Pos>) -> Vec<Face> {
    Face::ALL
        .into_iter()
        .filter(|&face| {
            let block = step(housing, face);
            let pipe = layout.housings.contains_key(&block) || layout.nodes.contains_key(&block);
            !pipe && blocks.contains(&block)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::settings::Edit;
    use crate::net::topology::{Channel, ChannelKind, Housing, Node, analyse};
    use crate::tuning::Tuning;

    fn channel(kind: ChannelKind) -> Option<Channel> {
        Some(Channel { kind, tier: 1 })
    }

    fn none() -> BTreeMap<Pos, HousingJoints> {
        BTreeMap::new()
    }

    /// A node at x=0, then housings at x=1..=3 with an item channel in slot 0 and an energy
    /// channel in slot 1.
    fn line() -> Layout {
        let mut layout = Layout::default();
        layout.nodes.insert([0, 0, 0], Node { tier: 1 });
        for x in 1..=3 {
            layout.housings.insert(
                [x, 0, 0],
                Housing {
                    slots: [
                        channel(ChannelKind::Item),
                        channel(ChannelKind::Energy),
                        None,
                        None,
                    ],
                },
            );
        }
        layout
    }

    #[test]
    fn a_block_with_an_inventory_beside_the_net_is_a_joint() {
        // M7: a chest above the first housing, a furnace at the end of the line.
        let layout = line();
        let topology = analyse(&layout, &Tuning::default());
        let net = topology.net_of([1, 0, 0], 0).unwrap();
        let chest: Pos = [1, 1, 0];
        let furnace: Pos = [4, 0, 0];
        let inventories = BTreeMap::from([
            (chest, Mode::From),
            (furnace, Mode::Both),
            ([9, 9, 9], Mode::Into),
        ]);
        let joints = item_joints(net, &layout, &inventories, &none());
        assert_eq!(joints.len(), 2);
        assert_eq!(
            (joints[0].housing, joints[0].face, joints[0].block),
            ([1, 0, 0], Face::YPos, chest)
        );
        assert_eq!(joints[0].mode, Mode::From);
        assert_eq!(
            (joints[1].housing, joints[1].face, joints[1].block),
            ([3, 0, 0], Face::XPos, furnace)
        );
        assert_eq!(joints[1].mode, Mode::Both);
    }

    #[test]
    fn the_node_and_other_housings_are_never_joints() {
        // The node at x=0 is in `inventories` by mistake: it is pipe, so no joint.
        let layout = line();
        let topology = analyse(&layout, &Tuning::default());
        let net = topology.net_of([2, 0, 0], 0).unwrap();
        let inventories = BTreeMap::from([([0, 0, 0], Mode::Into), ([2, 0, 0], Mode::Into)]);
        assert!(item_joints(net, &layout, &inventories, &none()).is_empty());
    }

    #[test]
    fn a_block_touched_by_two_housings_has_two_joints() {
        let layout = line();
        let topology = analyse(&layout, &Tuning::default());
        let net = topology.net_of([1, 0, 0], 0).unwrap();
        // Below housings 2 and 3 there is one block each; a block at (2,-1,0) touches only one.
        let inventories = BTreeMap::from([([2, 0, 1], Mode::Into)]);
        assert_eq!(item_joints(net, &layout, &inventories, &none()).len(), 1);
        // A block in a corner between two housings of an L touches both.
        let mut layout = line();
        layout.housings.insert(
            [3, 0, 1],
            Housing {
                slots: [channel(ChannelKind::Item), None, None, None],
            },
        );
        let topology = analyse(&layout, &Tuning::default());
        let net = topology.net_of([1, 0, 0], 0).unwrap();
        let corner = BTreeMap::from([([2, 0, 1], Mode::Into)]);
        assert_eq!(item_joints(net, &layout, &corner, &none()).len(), 2);
    }

    #[test]
    fn energy_ports_are_the_sources_and_machines_touching_the_net() {
        let layout = line();
        let topology = analyse(&layout, &Tuning::default());
        let energy = topology.net_of([1, 0, 0], 1).unwrap();
        assert_eq!(energy.kind, ChannelKind::Energy);
        let crystal: Pos = [2, -1, 0];
        let furnace: Pos = [4, 0, 0];
        let far: Pos = [6, 0, 0];
        let ports = BTreeSet::from([crystal, furnace, far]);
        assert_eq!(
            touching(energy, &layout, &ports),
            BTreeSet::from([crystal, furnace])
        );
    }

    #[test]
    fn a_housings_setting_for_its_face_and_slot_wins_over_the_blocks_default() {
        // M7: the chest above housing 1 is «в блок» by default; its housing says «из блока»
        // with a white list and priority 2 for slot 0, and something else for slot 1 and for
        // its other faces, which the item net of slot 0 never reads.
        let layout = line();
        let topology = analyse(&layout, &Tuning::default());
        let net = topology.net_of([1, 0, 0], 0).unwrap();
        let chest: Pos = [1, 1, 0];
        let wanted = JointSetting::of_mode(Mode::Into)
            .apply(&Edit::NextMode)
            .apply(&Edit::AddItem("minecraft:raw_iron".into()))
            .apply(&Edit::ToggleList)
            .apply(&Edit::Priority(2));
        let off = JointSetting::of_mode(Mode::Off);
        let settings = BTreeMap::from([(
            [1, 0, 0],
            HousingJoints::default()
                .with(Face::YPos, 0, wanted.clone())
                .with(Face::YPos, 1, off.clone())
                .with(Face::ZPos, 0, off),
        )]);
        let inventories = BTreeMap::from([(chest, Mode::Into), ([2, 1, 0], Mode::Into)]);
        let joints = item_joints(net, &layout, &inventories, &settings);
        assert_eq!(joints.len(), 2);
        assert_eq!(joints[0].block, chest);
        assert_eq!(joints[0].mode, Mode::From);
        assert_eq!(joints[0].filter, wanted.filter);
        assert_eq!(joints[0].priority, 2);
        // The chest above housing 2 has no setting: its default.
        assert_eq!(joints[1].block, [2, 1, 0]);
        assert_eq!(joints[1].mode, Mode::Into);
        assert_eq!(joints[1].priority, 0);
    }

    #[test]
    fn an_energy_joint_lets_a_port_work_only_in_its_direction() {
        // M7, example «энергия не утекает из машины»: the furnace at the end on an energy joint
        // «в блок» still takes; a crystal on a joint «в блок» gives nothing; «выключен» drops a
        // machine from the circuit; with no setting both work (ENERGY_DEFAULT).
        let layout = line();
        let topology = analyse(&layout, &Tuning::default());
        let energy = topology.net_of([1, 0, 0], 1).unwrap();
        let furnace: Pos = [4, 0, 0];
        let crystal: Pos = [2, -1, 0];
        let other_crystal: Pos = [1, -1, 0];
        let other_machine: Pos = [3, 1, 0];
        let ports = BTreeMap::from([
            (furnace, Port::Machine),
            (crystal, Port::Source),
            (other_crystal, Port::Source),
            (other_machine, Port::Machine),
        ]);
        let into = JointSetting::of_mode(Mode::Into);
        let settings = BTreeMap::from([
            (
                [2, 0, 0],
                HousingJoints::default().with(Face::YNeg, 1, into.clone()),
            ),
            (
                [3, 0, 0],
                HousingJoints::default().with(Face::XPos, 1, into).with(
                    Face::YPos,
                    1,
                    JointSetting::of_mode(Mode::Off),
                ),
            ),
        ]);
        assert_eq!(
            energy_members(energy, &layout, &ports, &settings),
            BTreeSet::from([furnace, other_crystal])
        );
        assert_eq!(
            energy_members(energy, &layout, &ports, &none()),
            BTreeSet::from([furnace, crystal, other_crystal, other_machine])
        );
    }

    #[test]
    fn joint_faces_are_the_faces_toward_a_listed_block_that_are_not_pipe() {
        let layout = line();
        let blocks = BTreeSet::from([[2, 1, 0], [2, 0, -1], [1, 0, 0], [7, 7, 7]]);
        assert_eq!(
            joint_faces([2, 0, 0], &layout, &blocks),
            vec![Face::YPos, Face::ZNeg]
        );
    }
}
