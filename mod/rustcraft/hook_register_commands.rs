fn rustcraft_register_commands(commands: &mut Vec<crate::mod_api::NativeCommand>) {
    crate::game::rustcraft_selftest::on_register_commands(commands);
    crate::game::rustcraft_debug::on_register_commands(commands);
}
