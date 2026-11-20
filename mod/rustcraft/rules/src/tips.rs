//! An item's description in its tooltip (spec 41, TT1–TT6): the summary of its handbook page and,
//! for a module, the number its summary lacks and what it fits. The client (rustcraft_tips) asks
//! for the lines of a hovered stack, wraps them to the tooltip's width and puts them under the
//! name. Nothing is written to stacks (TT5).

use crate::handbook::{PAGES, Page};

/// Lang keys of the lines a page's Russian stat gives in another language:
/// `rustcraft.tip.<page>.<stat>` (TT3). Not under `rustcraft.handbook.`, which
/// assets/models/handbook_mc.py rewrites.
pub const PREFIX: &str = "rustcraft.tip.";

/// The width the lines are wrapped to, in GUI pixels: about 40 characters of the game's font, so
/// a tooltip stays narrow next to the inventory (TT4).
pub const WIDTH: f32 = 240.0;

/// The colour of the lines, `0xRRGGBB`: the game's grey of an item's secondary lines (TT4).
pub const COLOR: u32 = 0xAAAAAA;

/// The tiers an item id may end in (`furnace_3`, `blade_4`): tiers 2…4 (spec 12.1).
const TIERS: std::ops::RangeInclusive<u8> = 2..=4;

/// Items no page shows or makes, with the pages that describe them (TT2).
const ALIASES: &[(&str, &[&str])] = &[
    ("rustcraft:crystal_shard", &["shard", "grow"]),
    ("rustcraft:druse", &["grow"]),
    ("rustcraft:seed", &["grow"]),
    ("rustcraft:zip_anchor", &["zipline"]),
    // 0.21.2: the buckets the terminal draws for the solution and the catalysts (fluid_icons).
    ("rustcraft:solution_bucket", &["fluids"]),
    ("rustcraft:accelerator_bucket", &["fluids"]),
    ("rustcraft:economizer_bucket", &["fluids"]),
    ("rustcraft:enricher_bucket", &["fluids"]),
];

/// The stats a page's tooltip shows after its summary, by index (`.stat.<n>`): the number the
/// summary does not name, then «Подходит» (TT3). Modules only: what a module does to a machine
/// is the line its window cannot show.
pub const STATS: &[(&str, &[usize])] = &[
    ("mod_speed", &[2]),
    ("mod_eco", &[2]),
    ("mod_cap", &[0, 2]),
    ("mod_stack", &[0, 4]),
    ("mod_yield", &[1, 2]),
    ("mod_store_cap", &[1]),
    ("mod_output", &[0, 1]),
    ("mod_line", &[0, 1]),
    ("mod_scout", &[0, 1]),
    ("mod_area", &[3]),
    ("mod_storage", &[0, 1]),
    ("mod_rare", &[0, 1]),
    ("mod_pick", &[1]),
    ("mod_stratum", &[4]),
    ("mod_tunnel", &[1]),
    ("mod_height", &[0]),
    ("mod_lava", &[0]),
    ("mod_build", &[0]),
    ("mod_radius", &[4]),
    ("mod_bleed", &[0, 3]),
    ("mod_poison", &[0, 4]),
    ("mod_slow", &[0, 3]),
    ("mod_heal", &[0, 4]),
    ("mod_guard", &[0, 4]),
    ("mod_power", &[0, 4]),
    ("mod_haste", &[0, 4]),
    ("mod_filter", &[0]),
];

/// The tooltip line of a page's stat in another language than the handbook's Russian.
pub fn stat_key(page: &str, stat: usize) -> String {
    format!("{PREFIX}{page}.{stat}")
}

fn page(id: &str) -> Option<&'static Page> {
    PAGES.iter().find(|p| p.id == id)
}

/// The page of an item as written: the page whose recipes make it, else the page it is the
/// icon of.
fn page_of_exact(item: &str) -> Option<&'static Page> {
    PAGES
        .iter()
        .find(|p| p.makes.contains(&item))
        .or_else(|| PAGES.iter().find(|p| p.item == Some(item)))
}

/// `furnace_3` → `furnace`: the id without a tier ending, if it has one.
fn untiered(item: &str) -> Option<&str> {
    let (base, tier) = item.rsplit_once('_')?;
    let tier: u8 = tier.parse().ok()?;
    TIERS.contains(&tier).then_some(base)
}

/// The pages describing an item (TT2), none for an item not RustCraft's.
pub fn pages_of(item: &str) -> Vec<&'static Page> {
    if !item.starts_with("rustcraft:") {
        return Vec::new();
    }
    if let Some((_, ids)) = ALIASES.iter().find(|(i, _)| *i == item) {
        return ids.iter().filter_map(|id| page(id)).collect();
    }
    if let Some(p) = page_of_exact(item) {
        return vec![p];
    }
    // A tier above the family's first: its family's page, by the first tier's id or, for
    // gear that opens on tier 2 (`blade_2`), by that one's.
    let Some(base) = untiered(item) else {
        return Vec::new();
    };
    std::iter::once(base.to_owned())
        .chain(TIERS.map(|t| format!("{base}_{t}")))
        .find_map(|id| page_of_exact(&id))
        .into_iter()
        .collect()
}

/// An item's tooltip lines, unwrapped, in the client's language (`lookup` reads its lang): each
/// page's summary, then the page's chosen stats (TT1, TT3). A line whose key the language lacks
/// is left out.
pub fn lines(item: &str, lookup: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let mut out = Vec::new();
    for p in pages_of(item) {
        out.extend(lookup(&p.key("sum")));
        let stats = STATS
            .iter()
            .find(|(id, _)| *id == p.id)
            .map_or(&[][..], |s| s.1);
        for &s in stats {
            out.extend(stat_line(p, s, lookup));
        }
    }
    out
}

/// «Key: value» from the handbook's stat where the language has it (Russian), else the tip's own
/// line (TT3).
fn stat_line(p: &Page, stat: usize, lookup: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let key = lookup(&p.key(&format!("stat.{stat}.key")));
    let value = lookup(&p.key(&format!("stat.{stat}.value")));
    match (key, value) {
        (Some(k), Some(v)) => Some(format!("{k}: {v}")),
        _ => lookup(&stat_key(p.id, stat)),
    }
}

#[cfg(test)]
mod tests;
