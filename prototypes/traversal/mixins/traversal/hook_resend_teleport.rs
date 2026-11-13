// Runs inside the client library's hub, which cannot see the game: the support module there
// keeps the correction until the game half takes it.
fn rc_traversal_resend_teleport(hub: &Hub, i: usize, position: Option<[f64; 3]>) {
    if let Some(g) = hub.guests.get(i) {
        crate::trv_setbacks::note(g.uuid, position.unwrap_or(g.position));
    }
}
