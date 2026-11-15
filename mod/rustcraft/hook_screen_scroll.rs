fn rustcraft_screen_scroll(game: &crate::game::Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    crate::game::rustcraft_handbook::on_scroll(game, scroll, canceled);
}
