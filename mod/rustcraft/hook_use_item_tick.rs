fn rustcraft_use_item_tick(
    stack: &horizon_client::items::ItemStack,
    entity: Option<i32>,
    canceled: &mut bool,
) {
    crate::game::rustcraft_gear::on_use_tick(stack, entity, canceled);
}
