fn rustcraft_tick(game: &mut crate::game::Game<'_>) {
    crate::game::rustcraft_station::on_tick(game);
    crate::game::rustcraft_gear::client_tick(game);
    crate::game::rustcraft_handbook::client_tick(game);
    crate::game::rustcraft_tablet::client_tick(game);
    // Slice 18: the backpack's own screen.
    crate::game::rustcraft_backpack::client_tick(game);
    crate::game::rustcraft_node_window::client_tick(game);
    crate::game::rustcraft_joint_screen::client_tick(game);
}
