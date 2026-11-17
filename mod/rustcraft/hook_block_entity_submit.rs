fn rustcraft_block_entity_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: horizon_world::registry::BlockStateId,
    entity: &horizon_client::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    if crate::game::rustcraft_builder::draw_facade(game, entity, draw) {
        return;
    }
    // s18/pipe: a housing's channel strips and, with the wrench, its bottleneck.
    if block == "rustcraft:housing" {
        crate::game::rustcraft_pipe18::draw_housing(game, entity, draw);
        return;
    }
    if block == rustcraft_rules::traversal::ANCHOR {
        crate::game::rustcraft_zipline::draw_anchor(game, pos, entity, draw);
        return;
    }
    if block == "rustcraft:terminal" {
        crate::game::rustcraft_storage::draw_holes(game, pos, entity, draw);
    }
    if block == "rustcraft:projector" {
        crate::game::rustcraft_builder::draw_ghost(game, pos, entity, draw);
    }
    // Slice 18: crystals as one structure, the connectors of crystals, panels and windmills.
    if crate::game::rustcraft_power::draw(game, block, entity, draw) {
        return;
    }
    crate::game::rustcraft_anim::draw(game, block, state, entity, draw);
}
