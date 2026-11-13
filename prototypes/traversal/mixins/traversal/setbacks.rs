// Lives in the client library, where the server's hub and the fall handling are. The
// corrections the hub sends a player (Hub::resend_teleport) wait here until the game half's
// level tick takes them, and the game half leaves here whose falls do no damage.
use std::time::{Duration, Instant};

pub static SETBACKS: std::sync::Mutex<Vec<(u128, [f64; 3])>> = std::sync::Mutex::new(Vec::new());

/// More than this many waiting means nobody takes them: the oldest are kept.
pub const SETBACKS_MAX: usize = 4096;

pub fn note(player: u128, to: [f64; 3]) {
    let mut heard = SETBACKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if heard.len() < SETBACKS_MAX {
        heard.push((player, to));
    }
}

pub fn take_all() -> Vec<(u128, [f64; 3])> {
    std::mem::take(
        &mut *SETBACKS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )
}

/// Entities whose falls do no damage, until when. The fall hook is a free function that cannot
/// see the game, so the game half leaves the entities here.
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
