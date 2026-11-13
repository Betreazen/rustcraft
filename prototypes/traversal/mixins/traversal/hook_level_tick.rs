fn rc_traversal_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::trv_server::on_level_tick(game, dimension);
}
