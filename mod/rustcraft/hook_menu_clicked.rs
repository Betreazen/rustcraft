fn rustcraft_menu_clicked(
    game: &mut crate::game::Game<'_>,
    uuid: u128,
    window: i32,
    slot: i32,
    button: i32,
    mode: i32,
) {
    crate::game::rustcraft_server::on_menu_clicked(game, uuid, window, slot, button, mode);
}
