// Runs in horizon-world as each loot table loads (API 75): a mob's player-kill loot also drops for
// its owner's guard or turret (crate::loot::rustcraft_loot_marks).
fn rustcraft_loot_table_load(id: &str, table: &mut serde_json::Value) {
    crate::loot::rustcraft_loot_marks::rewrite(id, table);
}
