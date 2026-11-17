fn rustcraft_screen_render_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::rustcraft_handbook::draw(game, batch, screen);
    crate::game::rustcraft_node_window::draw(game, batch, screen);
    crate::game::rustcraft_joint_screen::draw(game, batch, screen);
}
