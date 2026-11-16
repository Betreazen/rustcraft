fn rustcraft_standalone_models(models: &mut Vec<String>) {
    crate::game::rustcraft_station::standalone_models(models);
    crate::game::rustcraft_anim::standalone_models(models);
    // Slice 18, base and gear: the gear seen on players.
    models.extend(crate::game::rustcraft_gear::WORN_MODELS.iter().map(|m| (*m).to_owned()));
}
