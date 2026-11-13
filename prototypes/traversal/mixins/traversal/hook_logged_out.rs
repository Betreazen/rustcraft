fn rc_traversal_logged_out(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::trv_server::on_logout(game, player);
}
