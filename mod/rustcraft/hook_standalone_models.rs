fn rustcraft_standalone_models(models: &mut Vec<String>) {
    crate::game::rustcraft_station::standalone_models(models);
    crate::game::rustcraft_anim::standalone_models(models);
    crate::game::rustcraft_pipe18::standalone_models(models);
}
