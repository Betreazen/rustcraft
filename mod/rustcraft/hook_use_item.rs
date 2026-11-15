fn rustcraft_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    let t = crate::game::rustcraft_server::tuning();
    crate::game::rustcraft_backpack::on_use_item(game, stack, claimed, t);
}
