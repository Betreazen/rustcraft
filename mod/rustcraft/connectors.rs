// Round 31: the connectors («присоски») of the machines (rustcraft_rules::connectors). After each
// read of the magistral (rustcraft_server::rescan: a player's placing or breaking beside a
// housing, a chunk loading, and every ten seconds), every block with connectors shows one on each
// side a housing touches. The side is a boolean of the block's own state, so the chunk mesh draws
// the connector with the block (its blockstate's `<model>__link_<side>`); the block entity stays.
//
// Round 32: the bottom. A sixth boolean would double the states (the 16-bit palette,
// rustcraft_rules::connectors), so the server writes whether a housing stands below into the
// block entity (`connectors::BOTTOM_KEY`, sent in the update tag by rustcraft_anim::update_tag)
// and the client draws `<model>__link_down` from its block entity (`draw_bottom`).
use crate::game::{
    Game,
    native_block_entity_draw::BlockEntityDraw,
    rustcraft_machines::{change_entity, entity},
};
use horizon_client::containers::ModBlockEntity;
use horizon_world::nbt::{Compound, JavaString, Tag};
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
        // Round 32: the cell below is in the block's own chunk, loaded with it.
        update_bottom(game, pos, layout.housings.contains_key(&connectors::below(pos)));
    }
}

/// The key of the block entity's compound, made once: a client reads it each frame it draws.
static BOTTOM_KEY: std::sync::LazyLock<JavaString> =
    std::sync::LazyLock::new(|| JavaString::from(connectors::BOTTOM_KEY));

/// Whether a block entity's compound says a housing stands below.
pub(super) fn bottom_of(nbt: &Compound) -> bool {
    matches!(nbt.get(&*BOTTOM_KEY), Some(Tag::Byte(v)) if *v != 0)
}

/// Round 32, on the server: writes `housed` into the block entity at `pos` and tells clients, only
/// when it changed. The entity is read in place (rustcraft_machines::entity), not copied.
fn update_bottom(game: &mut Game<'_>, pos: Pos, housed: bool) {
    if entity(game, pos).is_none_or(|e| bottom_of(&e.nbt) == housed) {
        return;
    }
    change_entity(game, pos, false, |e| {
        e.nbt.insert(BOTTOM_KEY.clone(), Tag::Byte(i8::from(housed)));
    });
    game.native_block_entity_sync(pos);
}

/// `native_standalone_models` (API 151): the bottom connectors.
pub(super) fn standalone_models(models: &mut Vec<String>) {
    models.extend(connectors::BOTTOM_MODELS.iter().map(|m| (*m).to_owned()));
}

/// Round 32, `native_block_entity_submit` (API 71): the bottom connector of a block standing on a
/// housing, unturned (the sides are the world's), drawn before any other part pushes a pose. True
/// when it drew one. It reads the block entity alone, so the caller may keep the drawing.
pub(super) fn draw_bottom(game: &Game<'_>, block: &str, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) -> bool {
    if !bottom_of(&entity.nbt) {
        return false;
    }
    let Some(model) = connectors::bottom_model(block) else {
        return false;
    };
    game.native_draw_model(draw, model, 0xFF_FFFF)
}
