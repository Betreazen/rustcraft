fn rustcraft_screen_key(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    canceled: &mut bool,
) {
    crate::game::rustcraft_terminal::on_key(game, screen, key, canceled);
    crate::game::rustcraft_station::on_key(game, screen, key, canceled);
}
