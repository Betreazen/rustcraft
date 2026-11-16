fn rustcraft_projectile_impact(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    projectile: u32,
    kind: &str,
    owner: u32,
    entity: Option<u32>,
    entity_kind: &str,
    at: [f64; 3],
    canceled: &mut bool,
) {
    crate::game::rustcraft_gear::on_projectile_impact(game, dimension, projectile, kind, owner, entity, entity_kind, at, canceled);
}
