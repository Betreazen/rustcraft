// Runs in the client library's fall handling (LivingEntity.causeFallDamage), on the server for a
// guest's landing: a capsule's rider takes no fall damage.
fn rustcraft_living_fall(entity: u32, cancel: &mut bool) {
    if crate::rustcraft_falls::falls_spared(entity) {
        *cancel = true;
    }
}
