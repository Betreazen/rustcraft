fn rustcraft_menu_button_v2(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
    id: i32,
    carried: &mut Option<horizon_client::items::ItemStack>,
) {
    crate::game::rustcraft_server::on_menu_button_v2(game, pos, entity, id, carried);
}
