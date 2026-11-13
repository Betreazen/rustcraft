fn rustcraft_living_death(m: &horizon_client::mobs::Mob) {
    crate::game::rustcraft_base::on_living_death(m.id, &m.kind.id);
}
