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
use horizon_client::{items::ItemStack, mod_api::NativeServerPlayer, text::Text};
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

// ---- The facade ----

/// The diagnosis of a housing or a node under a facade (guide `facade`): «Под фальш-панелью»,
/// then what the block itself shows; none when it is not covered.
pub(super) fn covered_diagnosis(
    game: &Game<'_>,
    pos: Pos,
    block: &str,
    own: impl FnOnce() -> Option<Text>,
) -> Option<Text> {
    if block != content::HOUSING && block != content::NODE {
        return None;
    }
    let entity = game.native_mod_block_entity(pos)?;
    if !crate::game::rustcraft_builder::has_facade(&entity.nbt) {
        return None;
    }
    let mut text = server::line("rustcraft.facade.covered", "Under a facade", &[]);
    if let Some(rest) = own() {
        text.push(Text::literal(" · "));
        text.push(rest);
    }
    Some(text)
}

/// The facade of the housing or node at `pos` and the block of its look, as items: what comes
/// back when it is broken or lifted.
pub(super) fn facade_items(game: &Game<'_>, pos: Pos) -> Vec<String> {
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return Vec::new();
    };
    if !crate::game::rustcraft_builder::has_facade(&entity.nbt) {
        return Vec::new();
    }
    let mut out = vec![crate::game::rustcraft_builder::FACADE.to_owned()];
    out.extend(crate::game::rustcraft_builder::look(&entity.nbt));
    out
}

/// The sneaking wrench on an open housing (guide `facade`: «следующий клик — сам корпус»): the
/// housing comes off as an item, its channels with it; what has no room drops.
pub(super) fn lift_housing(game: &mut Game<'_>, player: &NativeServerPlayer, pos: Pos) {
    let Some(mut items) = take_housing(game, &player.dimension, pos) else {
        return;
    };
    if player.game_mode == CREATIVE {
        items.retain(|i| i != content::HOUSING);
    }
    for item in &items {
        if game.native_give_items(item, 1) == 0 {
            server::drop_one(game, &player.dimension, pos, item);
        }
    }
    server::mark_level_stale(&player.dimension);
    let text = server::line("rustcraft.housing.lifted", "The housing taken off", &[]);
    server::tell(game, player.uuid, text);
}

/// The housing at `pos` taken out of the world: the items it gives back (itself, its facade and
/// the block of its look, its channels); none when nothing was taken.
fn take_housing(game: &mut Game<'_>, dimension: &str, pos: Pos) -> Option<Vec<String>> {
    if game.native_block(pos) != content::HOUSING {
        return None;
    }
    let entity = game.native_mod_block_entity(pos)?;
    let held = server::read_slots(&entity.nbt);
    let air = game.native_default_state("minecraft:air")?;
    let mut items = vec![content::HOUSING.to_owned()];
    items.extend(facade_items(game, pos));
    items.extend(
        held.slots
            .into_iter()
            .flatten()
            .map(|c| content::channel_item(c.kind, c.tier).to_owned()),
    );
    // Flags 3: neighbours hear it and the players see it.
    game.native_set_block(dimension, pos, air, 3).then_some(items)
}

/// The facade set on the housing or node at `pos` with the look `look` (none: smooth stone), as
/// a click with it does (rustcraft_builder).
fn cover(game: &mut Game<'_>, pos: Pos, look: Option<&str>) -> bool {
    use horizon_world::nbt::{JavaString, Tag};
    let set = game.native_with_mod_block_entity(pos, |e| {
        e.nbt.insert(JavaString::from(crate::game::rustcraft_builder::FACADE_KEY), Tag::Long(1));
        if let Some(look) = look {
            e.nbt.insert(
                JavaString::from(crate::game::rustcraft_builder::LOOK_KEY),
                Tag::String(JavaString::from(look)),
            );
        }
    });
    if set.is_none() {
        return false;
    }
    game.native_set_block_property(pos, rustcraft_rules::housing::COVERED, "true");
    game.native_block_entity_sync(pos);
    true
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
        "cover" => cover(game, pos, (!arg.is_empty()).then_some(arg))
            .then(String::new)
            .ok_or_else(|| format!("no facade block entity at {pos:?}")),
        "lift_housing" => {
            let dimension = game.native_dimension().to_owned();
            take_housing(game, &dimension, pos)
                .map(|items| items.join(","))
                .ok_or_else(|| format!("no housing taken at {pos:?}"))
        }
        "facade_items" => Ok(facade_items(game, pos).join(",")),
        "node_view" => {
            let axis: usize = arg.parse().unwrap_or(0);
            let dimension = game.native_dimension().to_owned();
            crate::game::rustcraft_node_window::view_at(game, &dimension, pos, axis)
                .map(|v| String::from_utf8_lossy(&rustcraft_rules::net::node_window::encode(&v)).into_owned())
                .ok_or_else(|| format!("no node window at {pos:?}"))
        }
        "socket" => {
            let i: u8 = arg.parse().map_err(|_| format!("socket {arg:?}"))?;
            crate::game::rustcraft_node_window::socket(game, 0, pos, i)
                .map(|n| n.to_string())
                .ok_or_else(|| format!("socket {i} at {pos:?} did nothing"))
        }
        _ => Err(format!("no action {name}")),
    }
}
