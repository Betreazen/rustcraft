fn rustcraft_menu_input_closed(
    uuid: u128,
    window: i32,
    slots: &mut Vec<Option<horizon_client::items::ItemStack>>,
) {
    crate::game::rustcraft_backpack::on_menu_input_closed(uuid, window, slots);
    crate::game::rustcraft_gear::on_menu_input_closed(uuid, window, slots);
}
