fn rustcraft_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::rustcraft_gear::on_use_item(game, stack, claimed);
    crate::game::rustcraft_tablet::on_use_item(game, stack, claimed);
    let t = crate::game::rustcraft_server::tuning();
    crate::game::rustcraft_backpack::on_use_item(game, stack, claimed, t);
}
