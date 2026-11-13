fn rc_traversal_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::trv_client::on_command(game, command, ran);
}
