fn rustcraft_inventory_tick(game: &mut crate::game::Game<'_>, slot: i32) {
    crate::game::rustcraft_backpack::on_inventory_tick(game, slot);
    crate::game::rustcraft_gear::on_inventory_tick(game, slot);
}
