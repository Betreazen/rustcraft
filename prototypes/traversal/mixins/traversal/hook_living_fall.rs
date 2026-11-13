// Runs in the client library's fall handling (LivingEntity.causeFallDamage), on the server for
// a guest's landing. A player moved by an attempt, or just let go by one, takes no fall damage.
fn rc_traversal_living_fall(entity: u32, cancel: &mut bool) {
    if crate::trv_setbacks::falls_spared(entity) {
        *cancel = true;
    }
}
