fn rustcraft_world_primitives(
    game: &crate::game::Game<'_>,
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::rustcraft_zipline::ghost(game, primitives);
}
