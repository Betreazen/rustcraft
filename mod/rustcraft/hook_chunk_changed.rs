fn rustcraft_chunk_changed(dimension: &str) {
    crate::game::rustcraft_server::on_chunk_changed(dimension);
}
