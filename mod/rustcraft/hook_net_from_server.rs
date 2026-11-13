fn rustcraft_net_from_server(channel: &str, data: &[u8]) {
    crate::game::rustcraft_terminal::on_net(channel, data);
    crate::game::rustcraft_station::on_net(channel, data);
}
