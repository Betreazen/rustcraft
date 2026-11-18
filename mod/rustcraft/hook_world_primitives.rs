fn rustcraft_world_primitives(
    game: &crate::game::Game<'_>,
    partial: f32,
    camera: [f64; 3],
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::rustcraft_zipline::ghost(game, primitives);
    crate::game::rustcraft_station::worker_boards(game, partial, camera, primitives);
}
