fn rustcraft_screen_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    crate::game::rustcraft_terminal::on_char(game, screen, c, canceled);
    crate::game::rustcraft_station::on_char(game, screen, c, canceled);
}
