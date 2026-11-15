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
    if block == "rustcraft:projector" {
        crate::game::rustcraft_builder::draw_ghost(game, pos, entity, draw);
    }
    crate::game::rustcraft_anim::draw(game, block, state, entity, draw);
}
