fn rustcraft_block_entity_update_tag(
    entity: &horizon_client::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    crate::game::rustcraft_anim::update_tag(entity, tag);
    crate::game::rustcraft_storage::update_tag(entity, tag);
    crate::game::rustcraft_projector::update_tag(entity, tag);
    crate::game::rustcraft_pipe18::update_tag(entity, tag);
}
