fn rustcraft_net_from_client(player: u128, channel: &str, data: &[u8]) {
    crate::game::rustcraft_monorail::on_client_message(player, channel, data);
    // Slice 18, base and gear: the gear's keys.
    crate::game::rustcraft_gear_keys::on_client_message(player, channel, data);
    // Slice 18, wave 2: the rifle fires from its client.
    crate::game::rustcraft_gear::on_client_message(player, channel, data);
    crate::game::rustcraft_tablet::on_client_message(player, channel, data);
    // Slice 18: the backpack's own screen.
    crate::game::rustcraft_backpack::on_client_message(player, channel, data);
    crate::game::rustcraft_node_window::on_client_message(player, channel, data);
    crate::game::rustcraft_window::on_client_message(player, channel, data);
}
