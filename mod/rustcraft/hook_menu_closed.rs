fn rustcraft_menu_closed(uuid: u128, window: i32) {
    crate::game::rustcraft_server::on_menu_closed(uuid, window);
}
