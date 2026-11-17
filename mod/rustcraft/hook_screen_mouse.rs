fn rustcraft_screen_mouse(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::rustcraft_tablet::on_mouse(game, screen, button, canceled);
    // Slice 18: the backpack's own screen.
    crate::game::rustcraft_backpack::on_mouse(game, screen, button, canceled);
    crate::game::rustcraft_handbook::on_mouse(game, screen, button, canceled);
    crate::game::rustcraft_node_window::on_mouse(game, screen, button, canceled);
    crate::game::rustcraft_joint_screen::on_mouse(game, screen, button, canceled);
    // Slice 18, wave 2: the chassis window, the mod's own screen.
    crate::game::rustcraft_station::chassis_mouse(game, screen, button, canceled);
}
