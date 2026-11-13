fn rc_traversal_net_from_server(channel: &str, data: &[u8]) {
    crate::game::trv_client::on_server_message(channel, data);
}
