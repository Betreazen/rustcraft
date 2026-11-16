fn rustcraft_config_reloading(config: &crate::game::NativeConfigFile) {
    crate::game::rustcraft_workers::config_read(config);
}
