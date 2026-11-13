fn rc_traversal_client_packet(packet: &horizon_client::mod_api::ClientboundPlayPacket) {
    crate::game::trv_client::on_packet(packet);
}
