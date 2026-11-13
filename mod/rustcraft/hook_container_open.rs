fn rustcraft_container_open(player: u128, dimension: &str, menu: &str, pos: Option<[i32; 3]>) {
    crate::game::rustcraft_storage::on_container_open(player, dimension, menu, pos);
    crate::game::rustcraft_monorail::on_container_open(player, dimension, menu, pos);
}
