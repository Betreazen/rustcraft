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
}
