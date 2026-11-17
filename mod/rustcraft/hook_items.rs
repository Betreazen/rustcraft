fn rustcraft_items(items: &mut Vec<horizon_mods::content::Item>) {
    crate::game::rustcraft_content::declare_items(items);
    crate::game::rustcraft_crystals::declare_items(items);
}
