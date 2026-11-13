fn rc_traversal_world_primitives(
    game: &crate::game::Game<'_>,
    partial: f32,
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::trv_client::draw(game, partial, primitives);
}
