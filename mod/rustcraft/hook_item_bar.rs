fn rustcraft_item_bar(
    stack: &horizon_client::items::ItemStack,
    bar: &mut Option<(i32, u32)>,
) {
    crate::game::rustcraft_gear::item_bar(stack, bar);
}
