fn rustcraft_block_entity_bounds(
    pos: [i32; 3],
    block: &str,
    entity: &horizon_client::containers::ModBlockEntity,
    bounds: &mut [f32; 6],
) {
    if block == rustcraft_rules::traversal::ANCHOR {
        crate::game::rustcraft_zipline::bounds(pos, entity, bounds);
    }
    crate::game::rustcraft_anim::bounds(block, bounds);
    if block == "rustcraft:crystal" {
        crate::game::rustcraft_power::bounds(entity, bounds);
    }
    if block == "rustcraft:terminal" {
        crate::game::rustcraft_storage::hole_bounds(entity, bounds);
    }
    if block == "rustcraft:projector" {
        crate::game::rustcraft_projector::ghost_bounds(entity, bounds);
    }
}
