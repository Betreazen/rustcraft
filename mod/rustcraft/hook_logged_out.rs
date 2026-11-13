fn rustcraft_logged_out(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::rustcraft_monorail::on_logout(game, player);
    crate::game::rustcraft_base::on_logout(game, player);
}
