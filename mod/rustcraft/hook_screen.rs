fn rustcraft_screen(block: &str, screen: &mut Option<horizon_client::mod_api::NativeScreen>) {
    crate::game::rustcraft_screens::layout(block, screen);
}
