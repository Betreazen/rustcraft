// Round 31: the connectors («присоски») of the machines (rustcraft_rules::connectors). After each
// read of the magistral (rustcraft_server::rescan: a player's placing or breaking beside a
// housing, a chunk loading, and every ten seconds), every block with connectors shows one on each
// side a housing touches. The side is a boolean of the block's own state, so the chunk mesh draws
// the connector with the block (its blockstate's `<model>__link_<side>`); the block entity stays.
use crate::game::Game;
use rustcraft_rules::{connectors, geom::Pos, net::topology::Layout};

/// Sets each side of `blocks` that differs from the housings of `layout` around it. A side whose
/// neighbour's chunk is not loaded stays as it is: the scan cannot see a housing there, and the
/// connector would go and come back when that chunk loads.
pub(super) fn update(game: &mut Game<'_>, layout: &Layout, blocks: impl IntoIterator<Item = Pos>) {
    for pos in blocks {
        for (side, next) in connectors::sides(pos) {
            if !game.world.chunks.contains_key(&(next[0] >> 4, next[2] >> 4)) {
                continue;
            }
            let value = if layout.housings.contains_key(&next) { "true" } else { "false" };
            // A block without the property (a content mod of an older version) is left alone.
            if game
                .native_block_property(pos, side)
                .is_some_and(|now| now != value)
            {
                game.native_set_block_property(pos, side, value);
            }
        }
    }
}
