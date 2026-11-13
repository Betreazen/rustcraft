fn rustcraft_net_from_client(player: u128, channel: &str, data: &[u8]) {
    crate::game::rustcraft_monorail::on_client_message(player, channel, data);
}
