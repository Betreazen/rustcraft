fn rustcraft_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::rustcraft_server::on_level_tick(game, dimension);
}
