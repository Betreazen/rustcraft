fn rustcraft_screen_render_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::rustcraft_handbook::draw(game, batch, screen);
    // Slice 18, stream 6: the chassis window's panel beside its menu.
    crate::game::rustcraft_station::draw_chassis(game, batch, screen);

    // Slice 18, base and gear: a gear window's charge and modules.
    crate::game::rustcraft_gear::draw_window(game, batch, screen);
}
