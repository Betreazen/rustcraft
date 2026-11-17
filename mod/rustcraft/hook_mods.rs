fn rustcraft_mods(mods: &mut Vec<horizon_client::mod_api::NativeMod>) {
    crate::game::rustcraft_gear_keys::mods(mods);
}
