fn rustcraft_container_close(player: u128, menu: &str) {
    crate::game::rustcraft_storage::on_container_close(player, menu);
    crate::game::rustcraft_monorail::on_container_close(player, menu);
}
