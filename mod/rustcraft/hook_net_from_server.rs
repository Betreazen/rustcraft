fn rustcraft_net_from_server(channel: &str, data: &[u8]) {
    crate::game::rustcraft_terminal::on_net(channel, data);
    crate::game::rustcraft_station::on_net(channel, data);
    crate::game::rustcraft_sheets::on_net(channel, data);
    crate::game::rustcraft_gear::on_net(channel, data);
    crate::game::rustcraft_handbook::on_net(channel, data);
    crate::game::rustcraft_tablet::on_net(channel, data);
    crate::game::rustcraft_node_window::on_net(channel, data);
    crate::game::rustcraft_joint_screen::on_net(channel, data);
}
