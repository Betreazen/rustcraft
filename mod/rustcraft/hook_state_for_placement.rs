fn rustcraft_state_for_placement(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    context: &horizon_world::level::PlacementContext,
    answer: &mut Option<Option<horizon_world::registry::BlockStateId>>,
) {
    crate::game::rustcraft_monorail::state_for_placement(level, state, context, answer);
    crate::game::rustcraft_base::state_for_placement(level, state, context, answer);
    // Slice 18, stream 3: a tiered block takes the tier of the item placing it.
    crate::game::rustcraft_tiers::state_for_placement(level, state, context, answer);
    crate::game::rustcraft_pipe18::state_for_placement(level, state, context, answer);
}
