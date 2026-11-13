fn rustcraft_container_accepts(
    entity: &horizon_client::containers::ModBlockEntity,
    slot: usize,
    stack: &horizon_client::items::ItemStack,
    cancel: &mut bool,
) {
    crate::game::rustcraft_server::on_container_accepts(entity, slot, stack, cancel);
}
