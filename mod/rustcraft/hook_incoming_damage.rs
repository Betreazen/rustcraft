fn rustcraft_incoming_damage(entity: i32, kind: &str, amount: &mut f32) {
    crate::game::rustcraft_base::on_incoming_damage(entity, kind, amount);
}
