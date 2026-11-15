fn rustcraft_visuals(
    game: &crate::game::Game<'_>,
    rims: &mut Vec<horizon_client::mod_api::NativeBlockRim>,
) {
    crate::game::rustcraft_zipline::visuals(game, rims);
}
