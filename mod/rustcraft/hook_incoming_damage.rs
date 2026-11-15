fn rustcraft_incoming_damage(
    entity: i32,
    kind: &str,
    damage_type: &str,
    causing: i32,
    amount: &mut f32,
    canceled: &mut bool,
) {
    crate::game::rustcraft_gear::on_incoming_damage(entity, kind, damage_type, causing, amount, canceled);
    crate::game::rustcraft_base::on_incoming_damage(entity, kind, amount);
}
