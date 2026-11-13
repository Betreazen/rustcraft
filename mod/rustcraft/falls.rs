// Lives in the client library, where the fall handling is (`native_living_fall`,
// SRC/crates/horizon-client/src/landing.rs:32, hooked at the function: the named hook stays
// silent for a bundle without a trait mod, as the traversal prototype found). The game half
// (rustcraft_monorail) leaves here whose falls do no damage, and until when: a capsule's rider
// while riding and for a moment after being set down.
use std::time::{Duration, Instant};

static SPARED: std::sync::Mutex<Vec<(u32, Instant)>> = std::sync::Mutex::new(Vec::new());

/// The falls of `entity` do no damage for `time` from now.
pub fn spare_falls(entity: u32, time: Duration) {
    let now = Instant::now();
    let mut spared = SPARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    spared.retain(|(e, until)| *e != entity && *until > now);
    spared.push((entity, now + time));
}

pub fn falls_spared(entity: u32) -> bool {
    let now = Instant::now();
    SPARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .any(|(e, until)| *e == entity && *until > now)
}
