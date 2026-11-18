fn rustcraft_chunk_load_druses(dimension: &str, chunk: [i32; 2], new: bool) {
    crate::game::rustcraft_crystals::on_chunk_load(dimension, chunk, new);
}
