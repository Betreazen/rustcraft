fn rustcraft_use_item_start(
    stack: &horizon_client::items::ItemStack,
    entity: Option<i32>,
    canceled: &mut bool,
) {
    crate::game::rustcraft_gear::on_use_start(stack, entity, canceled);
}
