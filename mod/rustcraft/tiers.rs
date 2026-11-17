// Slice 18: machine tiers and the right-click replacement (spec 12.1 U1–U4, guide `swap`). The rule
// is rustcraft_rules::tiers; this module ties it to the game.
//
// A tiered block keeps its tier in its state's `tier` property, which every family block declares
// (rustcraft_content). An item of a higher tier places the same block (`place_block`), and
// `state_for_placement` sets the property from the item, as the monorail does. A right click with
// a higher tier's item on a placed lower tier (not sneaking: sneaking places it beside, as any
// block) sets only the property: `native_set_block_property` keeps the block entity
// (SRC/crates/horizon-client/src/game/native_hooks.rs:10620-10627, «The block entity stays»), so
// the items, fluids, modules and settings stay (U3) and the magistral does not change; the old
// tier's item goes to the player. The click is heard before the block's own use
// (`native_right_click_block`, API 21), since a machine's use opens its window.
use crate::game::{Game, rustcraft_server as server};
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    tiers::{self, Family, Swap},
};

const CREATIVE: i32 = 1;

/// The tier of the tiered block at `pos`, on the server and in a client's copy of the world
/// (the windows' titles); 1 for a block of no family.
pub(super) fn tier_at(game: &Game<'_>, pos: Pos) -> u8 {
    let block = game.native_block(pos);
    tier_of(game, pos, &block)
}

/// The tier of `block` at `pos`, when the caller knows the block already.
pub(super) fn tier_of(game: &Game<'_>, pos: Pos, block: &str) -> u8 {
    match tiers::family_of_block(block) {
        Some(f) => f.tier_from(game.native_block_property(pos, tiers::PROPERTY).as_deref()),
        None => 1,
    }
}

/// The family of the block at `pos` and its tier.
pub(super) fn family_at(game: &Game<'_>, pos: Pos) -> Option<(Family, u8)> {
    let block = game.native_block(pos);
    let family = tiers::family_of_block(&block)?;
    Some((family, tier_of(game, pos, &block)))
}

/// The module sockets of the tiered block at `pos` (A5: 3 / 3 / 4 / 4).
pub(super) fn sockets_at(game: &Game<'_>, pos: Pos) -> usize {
    tiers::sockets(tier_at(game, pos))
}

/// `native_state_for_placement` (API 76): a family block is placed with the tier of the item
/// placing it.
pub(super) fn state_for_placement(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    context: &horizon_world::level::PlacementContext,
    answer: &mut Option<Option<horizon_world::registry::BlockStateId>>,
) {
    let Some(block) = level.native_state_block(state) else {
        return;
    };
    let Some(family) = tiers::family_of_block(block) else {
        return;
    };
    let Some(tier) = family.tier_of(&context.item) else {
        return;
    };
    // Another hook (a slice-7 block's facing) may have answered first: build on its state.
    let base = match answer {
        Some(Some(s)) => *s,
        Some(None) => return,
        None => state,
    };
    if let Some(placed) = level.native_state_with_property(base, tiers::PROPERTY, &tier.to_string())
    {
        *answer = Some(Some(placed));
    }
}

/// U2 itself: the block at `pos` becomes the tier `item` places, when that is higher. The old
/// tier's item on success; why not, else.
pub(super) fn swap_in_place(
    game: &mut Game<'_>,
    pos: Pos,
    item: &str,
) -> Result<(String, u8, u8), &'static str> {
    let block = game.native_block(pos);
    let placed = tier_of(game, pos, &block);
    match tiers::swap(item, &block, placed) {
        Swap::Up { to, give_back } => {
            if game.native_set_block_property(pos, tiers::PROPERTY, &to.to_string()) {
                Ok((give_back, placed, to))
            } else {
                Err("the block did not take the tier")
            }
        }
        Swap::NotHigher => Err("not a higher tier"),
        Swap::Other => Err("not a tier of this block"),
    }
}

/// `native_right_click_block`: a higher tier's item on a placed lower tier of the same block
/// replaces it, and the old one goes to the player's hand (U2).
pub(super) fn on_right_click_block(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    if *canceled {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    if player.sneaking {
        return;
    }
    let Some((item, _, plain)) = game.native_held() else {
        return;
    };
    if !plain || tiers::family_of_item(&item).is_none() {
        return;
    }
    let block = game.native_block(pos);
    if tiers::family_of_block(&block).is_none() {
        return;
    }
    let Ok((old, from, to)) = swap_in_place(game, pos, &item) else {
        return;
    };
    *canceled = true;
    if player.game_mode != CREATIVE {
        game.native_take_items(&item, 1);
        if game.native_give_items(&old, 1) == 0 {
            drop_one(game, &player.dimension, pos, &old);
        }
    }
    let name = Text::translate_with_fallback(
        &format!("block.{}", block.replace(':', ".")),
        &block,
        &[],
    );
    let text = server::line(
        "rustcraft.tier.swapped",
        "%s: tier %s took the place of tier %s",
        &[name, Text::literal(to.to_string()), Text::literal(from.to_string())],
    );
    server::tell(game, player.uuid, text);
}

/// One `item` as an item entity at the middle of the block at `pos`.
fn drop_one(game: &mut Game<'_>, dimension: &str, pos: Pos, item: &str) {
    let stack = Tag::compound([
        ("id", Tag::String(JavaString::from(item))),
        ("count", Tag::Int(1)),
    ]);
    let nbt = Compound::from([(JavaString::from("Item"), stack)]);
    let at = pos.map(|c| f64::from(c) + 0.5);
    if game.native_summon(dimension, "minecraft:item", at, nbt).is_none() {
        log::warn!("rustcraft: {item} from the swap at {pos:?} was lost");
    }
}
