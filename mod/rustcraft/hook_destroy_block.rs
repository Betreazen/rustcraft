fn rustcraft_destroy_block(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::rustcraft_server::on_destroy_block(game, pos);
}
