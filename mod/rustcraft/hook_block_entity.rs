fn rustcraft_block_entity(
    block: &str,
    entity: &mut Option<(String, horizon_world::nbt::Compound)>,
) {
    crate::game::rustcraft_content::block_entity(block, entity);
}
