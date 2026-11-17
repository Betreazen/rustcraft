fn rustcraft_screen_mouse(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::rustcraft_handbook::on_mouse(game, screen, button, canceled);
    crate::game::rustcraft_node_window::on_mouse(game, screen, button, canceled);
    crate::game::rustcraft_joint_screen::on_mouse(game, screen, button, canceled);
}
