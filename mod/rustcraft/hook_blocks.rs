fn rustcraft_blocks(blocks: &mut Vec<horizon_mods::content::Block>) {
    crate::game::rustcraft_content::declare_blocks(blocks);
    crate::game::rustcraft_crystals::declare_blocks(blocks);
}
