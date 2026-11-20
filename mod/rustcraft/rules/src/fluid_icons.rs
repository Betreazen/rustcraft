//! 0.21.2: the item a window draws for a fluid. The storage terminal (and the tablet, the order
//! lines, a joint's fluid filter) shows a fluid as a bucket: the game's own for water, lava and
//! milk, and for the mod's nutrient solution and catalysts a bucket of the mod's that exists only
//! to be drawn. No bucket carries those fluids (machines::bucket: they live in tanks and
//! channels), so these items have no recipe, no use and no place in the creative tab;
//! assets/models/icons_mc.py paints them in the fluid's tank colour (anim::fluid_tint).

use crate::machines::bucket::{EMPTY, LAVA_BUCKET, WATER_BUCKET};

/// Each mod fluid a bucket only shows, and its display item: `<fluid>_bucket`, as a joint's fluid
/// filter already names a fluid's bucket.
pub const ICONS: [(&str, &str); 4] = [
    ("rustcraft:solution", "rustcraft:solution_bucket"),
    ("rustcraft:accelerator", "rustcraft:accelerator_bucket"),
    ("rustcraft:economizer", "rustcraft:economizer_bucket"),
    ("rustcraft:enricher", "rustcraft:enricher_bucket"),
];

/// The game's buckets of the fluids the game has a bucket for.
const GAME: [(&str, &str); 4] = [
    ("minecraft:water", WATER_BUCKET),
    ("minecraft:lava", LAVA_BUCKET),
    ("minecraft:milk", "minecraft:milk_bucket"),
    // Round 31: the storage's milk.
    ("rustcraft:milk", "minecraft:milk_bucket"),
];

/// The bucket `fluid` is drawn as; none for a fluid this table does not know.
pub fn known_icon(fluid: &str) -> Option<&'static str> {
    GAME.iter()
        .chain(ICONS.iter())
        .find(|(f, _)| *f == fluid)
        .map(|(_, icon)| *icon)
}

/// The bucket `fluid` is drawn as: an unknown fluid gets the empty bucket.
pub fn icon_of(fluid: &str) -> &'static str {
    known_icon(fluid).unwrap_or(EMPTY)
}

/// The bucket a joint's fluid filter shows `fluid` as (s18/pipe): this table's, else the item
/// `<fluid>_bucket`, which another mod's fluid may have.
pub fn filter_icon(fluid: &str) -> String {
    known_icon(fluid).map_or_else(|| format!("{fluid}_bucket"), str::to_owned)
}

/// Whether `item` is one of the display-only buckets.
pub fn is_icon_item(item: &str) -> bool {
    ICONS.iter().any(|(_, icon)| *icon == item)
}

#[cfg(test)]
mod tests;
