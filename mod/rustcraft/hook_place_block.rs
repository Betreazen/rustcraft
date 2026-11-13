fn rustcraft_place_block(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
) {
    crate::game::rustcraft_server::on_place_block(game, stack, pos);
}
