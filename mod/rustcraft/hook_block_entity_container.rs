fn rustcraft_block_entity_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::rustcraft_content::block_entity_container(block, container);
}
