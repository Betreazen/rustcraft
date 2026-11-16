// Slice 18, stream 1: the magistral as the handbook says (docs/superpowers/plans/s18-pipe.md).
//
// - Node tiers 2–4 as items (guide `node`, `swap`): `rustcraft:node_2..4` place the node block
//   with their `tier` (`state_for_placement`, as the monorail's items do); a click with a node of
//   a higher tier on a placed node changes its `tier` in place (`native_set_block_property`, the
//   block entity and its facade stay, U2) and the old tier's item comes back to the player. The
//   content mod's loot table gives a broken node its own tier's item.
//   The tiers are rustcraft_rules::net::node_tier until stream 3's `rustcraft_rules::tiers` is
//   merged.
use crate::game::{
    Game,
    rustcraft_content as content,
    rustcraft_server::{self as server},
};
use horizon_client::{items::ItemStack, text::Text};
use rustcraft_rules::{geom::Pos, net::node_tier};

/// `NativeServerPlayer::game_mode` of creative.
const CREATIVE: i32 = 1;

/// `native_state_for_placement`: a node item places its own tier.
pub(super) fn state_for_placement(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    context: &horizon_world::level::PlacementContext,
    answer: &mut Option<Option<horizon_world::registry::BlockStateId>>,
) {
    if answer.is_some() || level.native_state_block(state) != Some(content::NODE) {
        return;
    }
    let Some(tier) = node_tier::tier_of_item(&context.item) else {
        return;
    };
    if let Some(placed) = level.native_state_with_property(state, content::TIER, &tier.to_string()) {
        *answer = Some(Some(placed));
    }
}

/// `native_use_item_on`, before everything else: a node of a higher tier clicked on a node takes
/// its place (U2).
pub(super) fn on_use_item_on(game: &mut Game<'_>, stack: &ItemStack, pos: Pos, claimed: &mut bool) {
    if *claimed || game.native_block(pos) != content::NODE {
        return;
    }
    let Some(held) = node_tier::tier_of_item(&stack.item) else {
        return;
    };
    let Some(player) = game.native_acting_player() else {
        return;
    };
    // A sneaking click places a node beside it, as any block does.
    if player.sneaking {
        return;
    }
    *claimed = true;
    let Some(back) = swap_node(game, pos, held) else {
        let text = server::line(
            "rustcraft.node.not_higher",
            "Only a node of a higher tier takes its place",
            &[],
        );
        server::tell(game, player.uuid, text);
        return;
    };
    if player.game_mode != CREATIVE {
        game.native_take_items(node_tier::item_of_tier(held), 1);
        let old = node_tier::item_of_tier(back);
        if game.native_give_items(old, 1) == 0 {
            server::drop_one(game, &player.dimension, pos, old);
        }
    }
    server::mark_level_stale(&player.dimension);
    let text = server::line(
        "rustcraft.node.swapped",
        "Node of tier %s took the place of tier %s",
        &[Text::literal(held.to_string()), Text::literal(back.to_string())],
    );
    server::tell(game, player.uuid, text);
}

/// The node at `pos` raised to tier `held` in place; the tier it had, or none when `held` is
/// not higher.
fn swap_node(game: &mut Game<'_>, pos: Pos, held: u8) -> Option<u8> {
    let placed = game
        .native_block_property(pos, content::TIER)
        .and_then(|t| t.parse().ok())
        .unwrap_or(1);
    let back = node_tier::swap(placed, held)?;
    game.native_set_block_property(pos, content::TIER, &held.to_string())
        .then_some(back)
}

// ---- The self-test's actions (rustcraft_rules::selftest::Bench::pipe) ----

/// One action of stream 1's self-test on the block at `pos`: `swap_node:<tier>`.
pub(super) fn bench(game: &mut Game<'_>, action: &str, pos: Pos) -> Result<String, String> {
    let (name, arg) = action.split_once(':').unwrap_or((action, ""));
    match name {
        "swap_node" => {
            let held: u8 = arg.parse().map_err(|_| format!("tier {arg:?}"))?;
            swap_node(game, pos, held)
                .map(|back| back.to_string())
                .ok_or_else(|| format!("no swap to tier {held} at {pos:?}"))
        }
        _ => Err(format!("no action {name}")),
    }
}
