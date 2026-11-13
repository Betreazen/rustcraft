fn rc_traversal_net_from_client(
    game: &mut crate::game::Game<'_>,
    player: u128,
    channel: &str,
    data: &[u8],
) {
    crate::game::trv_server::on_client_message(game, player, channel, data);
}
