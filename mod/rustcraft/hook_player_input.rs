fn rustcraft_player_input(game: &mut crate::game::Game<'_>, player: u128, flags: i32) {
    crate::game::rustcraft_gear::on_player_input(game, player, flags);
}
