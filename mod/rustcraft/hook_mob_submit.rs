fn rustcraft_mob_submit(
    game: &crate::game::Game<'_>,
    mob: &crate::game::native_mob_draw::NativeMobRender<'_>,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
    drawn: &mut bool,
) {
    crate::game::rustcraft_station::draw_capsule(game, mob, draw, drawn);
    crate::game::rustcraft_station::draw_worker(game, mob, draw, drawn);
}
