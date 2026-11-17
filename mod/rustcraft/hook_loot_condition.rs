// Runs in horizon-world when a table names a mod's own condition (API 133): ours holds where a
// device's lethal hit was marked, at the dead mob's place (`ctx.place`, API 156).
fn rustcraft_loot_condition(kind: &str, ctx: &crate::loot::NativeLootContext<'_>, holds: &mut bool) {
    let place = ctx.place.as_deref();
    let dimension = place.and_then(|p| p.level.as_ref()).map(|l| l.dimension.as_str());
    let at = place.and_then(|p| p.origin);
    if let Some(h) = crate::loot::rustcraft_loot_marks::holds(kind, dimension, at) {
        *holds = h;
    }
}
