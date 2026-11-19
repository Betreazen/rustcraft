fn rustcraft_inventory_tick(game: &mut crate::game::Game<'_>, slot: i32) {
    crate::game::rustcraft_backpack::on_inventory_tick(game, slot);
    crate::game::rustcraft_gear::on_inventory_tick(game, slot);
    // 0.21 (spec 45): the old channel items become their alloy.
    crate::game::rustcraft_server::on_inventory_tick(game, slot);
}
