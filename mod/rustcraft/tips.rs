// An item's description in its tooltip (spec 41, TT1–TT6; rustcraft_rules::tips decides).
//
// On the client only: `native_tooltip` (API 12, SRC/crates/horizon-client/src/game/
// native_hooks.rs:7911-7920, the point of NeoForge's ItemTooltipEvent) hands every tooltip the game
// shows — inventory and container slots, the creative tabs, the recipe book, chat's hover, the
// mod's own windows — with its lines once vanilla built them (game/stations.rs:2622-2626). For a
// RustCraft item the lines of its handbook page, in the client's language
// (`horizon_client::lang`), wrapped to rustcraft_rules::tips::WIDTH by the game's font
// (`native_text_width`, native_hooks.rs:10838-10840), go in grey right under the name. Nothing is
// written to the stack (TT5): old and new stacks still stack, and the server never sees it.

use crate::game::Game;
use horizon_client::{items::ItemStack, text::Text};
use rustcraft_rules::{handbook::wrap, tips};

/// A lang line of the client's language, None where it has none.
fn lookup(key: &str) -> Option<String> {
    horizon_client::lang::has(key).then(|| horizon_client::lang::get_or(key, ""))
}

pub(super) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let texts = tips::lines(&stack.item, &lookup);
    if texts.is_empty() {
        return;
    }
    let measure = |s: &str| game.native_text_width(s);
    let rows: Vec<(Text, u32)> = texts
        .iter()
        .flat_map(|t| wrap(t, tips::WIDTH, &measure))
        .map(|row| (Text::colored(row, tips::COLOR), u32::MAX))
        .collect();
    // Under the name (the first line), above enchantments, lore and the other mods' lines (TT4).
    let at = lines.len().min(1);
    lines.splice(at..at, rows);
}
