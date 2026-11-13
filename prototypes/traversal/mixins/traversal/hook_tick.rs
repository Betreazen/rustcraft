fn rc_traversal_tick(game: &mut crate::game::Game<'_>) {
    crate::game::trv_client::on_tick(game);
}
