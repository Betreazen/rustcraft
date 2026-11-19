// Slice 18, stream 1: the magistral as the handbook says (docs/superpowers/plans/s18-pipe.md).
//
// - Node tiers 2–4 as items (guide `node`, `swap`): `rustcraft:node_2..4` place the node block
//   with their `tier` (`state_for_placement`, as the monorail's items do); a click with a node of
//   a higher tier on a placed node changes its `tier` in place (`native_set_block_property`, the
//   block entity and its facade stay, U2) and the old tier's item comes back to the player. The
//   content mod's loot table gives a broken node its own tier's item.
//   The tiers are rustcraft_rules::net::node_tier until stream 3's `rustcraft_rules::tiers` is
//   merged.
// - Round 31: the housing in tiers 1–4 (`rustcraft:housing`, `rustcraft:housing_2..4`,
//   rustcraft_rules::housing::TIERS), its three channels built in. A housing item of a higher tier
//   placed sets its `tier` once it stands (`housing_placed`: answering the placement state would
//   lose the sides the game joins it by), and a click with one on a placed housing of a lower tier
//   sets the tier in place, as the node's.
use crate::game::{
    Game,
    rustcraft_content as content,
    rustcraft_server::{self as server},
};
use horizon_client::{items::ItemStack, mod_api::NativeServerPlayer, save, text::Text};
use horizon_world::nbt::{Compound, JavaString, Tag};
use crate::game::rustcraft_utility::CUSTOM_DATA;
use std::{
    collections::{BTreeSet, HashMap},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};
use rustcraft_rules::{
    geom::Pos,
    net::{node_tier, wrench},
};

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

/// `native_use_item_on`, before everything else: a node or a housing of a higher tier clicked on
/// one of its kind takes its place (U2).
pub(super) fn on_use_item_on(game: &mut Game<'_>, stack: &ItemStack, pos: Pos, claimed: &mut bool) {
    // The sneaking wrench lifts a machine with its contents (guide `wrench`).
    if !*claimed
        && &*stack.item == content::WRENCH
        && let Some(player) = crate::game::rustcraft_server::acting(game)
        && player.sneaking
        && lift_to_player(game, &player, pos)
    {
        *claimed = true;
        return;
    }
    if !*claimed && game.native_block(pos) == content::HOUSING {
        upgrade_housing(game, stack, pos, claimed);
        return;
    }
    if *claimed || game.native_block(pos) != content::NODE {
        return;
    }
    let Some(held) = node_tier::tier_of_item(&stack.item) else {
        return;
    };
    let Some(player) = crate::game::rustcraft_server::acting(game) else {
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

/// Round 31 (U2): a housing of a higher tier clicked on the housing at `pos` sets its tier in
/// place, its block entity (its joints' settings and facade) staying; the old tier's item comes
/// back. A housing of a tier not higher, or a sneaking click, is placed beside it as any block.
fn upgrade_housing(game: &mut Game<'_>, stack: &ItemStack, pos: Pos, claimed: &mut bool) {
    use rustcraft_rules::housing;
    let Some(held) = housing::TIERS.tier_of(&stack.item) else {
        return;
    };
    let Some(player) = crate::game::rustcraft_server::acting(game) else {
        return;
    };
    let placed = server::housing_at(game, pos).tier;
    if player.sneaking || housing::swap(placed, held).is_none() {
        return;
    }
    *claimed = true;
    if !game.native_set_block_property(pos, content::TIER, &held.to_string()) {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: the housing at {pos:?} did not take tier {held}");
        return;
    }
    if player.game_mode != CREATIVE {
        game.native_take_items(&stack.item, 1);
        let old = housing::item_of_tier(placed);
        if game.native_give_items(&old, 1) == 0 {
            server::drop_one(game, &player.dimension, pos, &old);
        }
    }
    server::mark_level_stale(&player.dimension);
    let text = server::line(
        "rustcraft.housing.swapped",
        "Housing of tier %s took the place of tier %s",
        &[Text::literal(held.to_string()), Text::literal(placed.to_string())],
    );
    server::tell(game, player.uuid, text);
}

/// `native_place_block`: a housing placed by the item of a higher tier takes that tier. Its
/// placement state is the game's own, which joins it to its neighbours (`state_for_placement`
/// leaves the housing alone).
pub(super) fn housing_placed(game: &mut Game<'_>, stack: &ItemStack, pos: Pos) {
    let Some(tier) = rustcraft_rules::housing::TIERS.tier_of(&stack.item) else {
        return;
    };
    if tier <= 1 || game.native_block(pos) != content::HOUSING {
        return;
    }
    if !game.native_set_block_property(pos, content::TIER, &tier.to_string()) {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: the housing placed at {pos:?} did not take tier {tier}");
    }
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
/// housing comes off as its tier's item; what has no room drops.
pub(super) fn lift_housing(game: &mut Game<'_>, player: &NativeServerPlayer, pos: Pos) {
    let Some(mut items) = take_housing(game, &player.dimension, pos) else {
        return;
    };
    if player.game_mode == CREATIVE {
        items.retain(|i| rustcraft_rules::housing::TIERS.tier_of(i).is_none());
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

/// The housing at `pos` taken out of the world: the items it gives back (its tier's item, its
/// facade and the block of its look); none when nothing was taken.
fn take_housing(game: &mut Game<'_>, dimension: &str, pos: Pos) -> Option<Vec<String>> {
    if game.native_block(pos) != content::HOUSING {
        return None;
    }
    let entity = game.native_mod_block_entity(pos)?;
    // Round 31: a housing of 0.19 not yet read again still holds the channels set into it.
    let old = server::old_slots(&entity.nbt);
    let air = game.native_default_state("minecraft:air")?;
    let tier = server::housing_at(game, pos).tier;
    let mut items = vec![rustcraft_rules::housing::item_of_tier(tier)];
    items.extend(facade_items(game, pos));
    items.extend(
        old.into_iter()
            .flatten()
            .flatten()
            .map(|c| content::channel_item(c.kind, c.tier).to_owned()),
    );
    housing_handled(dimension, pos);
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
        // Wave 2: what a broken node gives back (rustcraft_server::on_destroy_block).
        "node_returns" => Ok(crate::game::rustcraft_node_window::node_returns(game, pos).join(",")),
        "weak" => Ok(game
            .native_mod_block_entity(pos)
            .and_then(|e| match e.nbt.get(&JavaString::from(WEAK_KEY)) {
                Some(Tag::Byte(b)) => Some(*b),
                _ => None,
            })
            .unwrap_or(0)
            .to_string()),
        "rotate" => rotate_machine(game, pos)
            .map(str::to_owned)
            .ok_or_else(|| format!("nothing turned at {pos:?}")),
        // Lifts the machine and places its item back at once, as a player would: what it holds after.
        "lift_and_place" => {
            let dimension = game.native_dimension().to_owned();
            let stack = lift_machine(game, &dimension, pos).ok_or("nothing lifted")?;
            if game.native_block(pos) != "minecraft:air" {
                return Err("the machine stayed".into());
            }
            // Placed as the item places it: its family's block on the item's tier.
            let (block, tier) = rustcraft_rules::tiers::family_of_item(&stack.item)
                .map_or((stack.item.to_string(), None), |(f, t)| (f.block.to_owned(), Some(t)));
            let state = game.native_default_state(&block).ok_or("no state")?;
            if !game.native_set_block(&dimension, pos, state, 3) {
                return Err("not placed back".into());
            }
            if let Some(t) = tier {
                game.native_set_block_property(pos, rustcraft_rules::tiers::PROPERTY, &t.to_string());
            }
            on_place_block(game, &stack, pos);
            Ok(stack.item.to_string())
        }
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
        // The joint window drawn as the guide's table (rustcraft_window, rustcraft_joint_screen).
        "joint_view" => {
            use rustcraft_rules::geom::Face;
            let face = arg.parse().ok().and_then(Face::from_index).ok_or_else(|| format!("face {arg:?}"))?;
            let dimension = game.native_dimension().to_owned();
            crate::game::rustcraft_window::screen_view(game, &dimension, pos, face)
                .map(|v| String::from_utf8_lossy(&rustcraft_rules::net::joint_screen::encode(&v)).into_owned())
                .ok_or_else(|| format!("no joint window at {pos:?}"))
        }
        "joint_press" => {
            use rustcraft_rules::geom::Face;
            let mut parts = arg.splitn(3, ';');
            let (face, held, json) = (parts.next(), parts.next(), parts.next());
            let face = face.and_then(|f| f.parse().ok()).and_then(Face::from_index).ok_or("no face")?;
            let held = held.filter(|h| !h.is_empty());
            let press: rustcraft_rules::net::joint_screen::Press =
                json.and_then(|j| serde_json::from_str(j).ok()).ok_or("the press does not read")?;
            let dimension = game.native_dimension().to_owned();
            crate::game::rustcraft_window::screen_press(game, &dimension, pos, face, &press, held)
                .map(|v| String::from_utf8_lossy(&rustcraft_rules::net::joint_screen::encode(&v)).into_owned())
        }
        _ => Err(format!("no action {name}")),
    }
}

// ---- The wrench on machines (guide `wrench`) ----
//
// A click turns a machine a quarter clockwise (`facing`, in place: its block entity stays); a
// sneaking click lifts it as one item: its block item with the block entity's compound and slots
// in its custom data (rustcraft_rules::net::wrench::LIFTED). The slots are emptied before the
// block goes, so nothing drops twice; placing the item puts both back (`on_place_block`).

/// The machine at `pos` turned a quarter clockwise; its new facing.
pub(super) fn rotate_machine(game: &mut Game<'_>, pos: Pos) -> Option<&'static str> {
    let block = game.native_block(pos);
    if content::Machine::of(&block).is_none() || !wrench::handles(&block) {
        return None;
    }
    let next = wrench::rotate(&game.native_block_property(pos, "facing")?)?;
    game.native_set_block_property(pos, "facing", next).then_some(next)
}

/// The machine at `pos` taken out of the world as one item with its contents and settings.
pub(super) fn lift_machine(game: &mut Game<'_>, dimension: &str, pos: Pos) -> Option<ItemStack> {
    let block = game.native_block(pos);
    if content::Machine::of(&block).is_none() || !wrench::handles(&block) {
        return None;
    }
    // A machine of tiers 2–4 is lifted as its tier's item (`rustcraft:furnace_3`), which places
    // that tier again (rustcraft_tiers::state_for_placement).
    let item = crate::game::rustcraft_tiers::family_at(game, pos)
        .and_then(|(family, tier)| family.item(tier))
        .unwrap_or_else(|| block.to_string());
    let entity = game.native_mod_block_entity(pos)?;
    let items: Vec<Tag> = entity
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let s = s.as_ref()?;
            let mut t = match save::item_to_nbt(s) {
                Tag::Compound(c) => c,
                _ => return None,
            };
            t.insert(JavaString::from("Slot"), Tag::Int(i as i32));
            Some(Tag::Compound(t))
        })
        .collect();
    let mut data = Compound::default();
    let mut lifted = Compound::default();
    lifted.insert(JavaString::from("nbt"), Tag::Compound(entity.nbt.clone()));
    lifted.insert(JavaString::from("items"), Tag::List(items));
    data.insert(JavaString::from(wrench::LIFTED), Tag::Compound(lifted));
    let air = game.native_default_state("minecraft:air")?;
    // Emptied first: the slots ride in the item, the block must not drop them too.
    game.native_with_mod_block_entity(pos, |e| e.items.iter_mut().for_each(|s| *s = None));
    if !game.native_set_block(dimension, pos, air, 3) {
        return None;
    }
    let mut stack = ItemStack::new(&item, 1);
    stack
        .data_mut()
        .set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
    server::mark_level_stale(dimension);
    Some(stack)
}

/// `native_place_block`: a lifted machine's item gives the new block its compound and slots back.
pub(super) fn on_place_block(game: &mut Game<'_>, stack: &ItemStack, pos: Pos) {
    let Some(data) = stack.custom_data() else {
        return;
    };
    let Some(Tag::Compound(lifted)) = data.get(&JavaString::from(wrench::LIFTED)) else {
        return;
    };
    let nbt = match lifted.get(&JavaString::from("nbt")) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::default(),
    };
    let items: Vec<(usize, ItemStack)> = match lifted.get(&JavaString::from("items")) {
        Some(Tag::List(list)) => list
            .iter()
            .filter_map(|t| {
                let Tag::Compound(c) = t else { return None };
                let slot = match c.get(&JavaString::from("Slot")) {
                    Some(Tag::Int(i)) => usize::try_from(*i).ok()?,
                    _ => return None,
                };
                Some((slot, save::item_from_nbt(t)?))
            })
            .collect(),
        _ => Vec::new(),
    };
    game.native_with_mod_block_entity(pos, move |e| {
        e.nbt = nbt;
        // A block entity just made may have no slots yet: they grow to the lifted ones.
        for (slot, item) in items {
            if e.items.len() <= slot {
                e.items.resize(slot + 1, None);
            }
            e.items[slot] = Some(item);
        }
    });
    game.native_block_entity_sync(pos);
}

/// The sneaking wrench on a machine: lifted into the player's inventory; what has no room drops.
fn lift_to_player(game: &mut Game<'_>, player: &NativeServerPlayer, pos: Pos) -> bool {
    let Some(stack) = lift_machine(game, &player.dimension, pos) else {
        return false;
    };
    if let Some(rest) = game.native_give_stack(player.uuid, stack) {
        let nbt = Compound::from([(JavaString::from("Item"), save::item_to_nbt(&rest))]);
        let at = pos.map(|c| f64::from(c) + 0.5);
        if game.native_summon(&player.dimension, "minecraft:item", at, nbt).is_none() {
            crate::game::rustcraft_debug::journal_warn!("rustcraft: the lifted {} at {pos:?} was lost", rest.item);
        }
    }
    let text = server::line("rustcraft.wrench.lifted", "Lifted with its contents", &[]);
    server::tell(game, player.uuid, text);
    true
}

// ---- The housing on the client: bottlenecks, see-through facades ----
//
// Guide `housing`: «сквозь окошки корпуса видно его каналы». Round 31: the three channels are
// built in, so the housing's block model shows them, along its line, and its tier on its collars
// (the content mod's housing blockstate): a client sees them as soon as it has the block, with no
// block entity data. Clients get each housing's bottleneck mark in its update tag; while the local
// player holds the wrench, its block entity draws a red frame round a housing whose tier holds its
// net back (rustcraft_rules::net::bottleneck) and no facade over any housing or node (F5:
// rustcraft_builder::draw_facade asks `wrench_in_hand`). A housing with a bottleneck and every
// facade are drawn each frame, not kept, so the wrench shows its change at once.

/// The housing's saved compound keeps its bottleneck mark under this key.
const WEAK_KEY: &str = "weak";
const WEAK_MODEL: &str = "rustcraft:block/housing_weak";
const WEAK_TINT: u32 = 0xFF3030;

/// `native_standalone_models`: the bottleneck frame.
pub(super) fn standalone_models(models: &mut Vec<String>) {
    if !models.iter().any(|x| x == WEAK_MODEL) {
        models.push(WEAK_MODEL.to_owned());
    }
}

/// `native_block_entity_update_tag`: a housing's bottleneck mark goes to clients.
pub(super) fn update_tag(entity: &horizon_client::containers::ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id != content::HOUSING {
        return;
    }
    for key in [WEAK_KEY] {
        if let Some(v) = entity.nbt.get(&JavaString::from(key)) {
            tag.get_or_insert_with(Compound::new)
                .insert(JavaString::from(key), v.clone());
        }
    }
    // Slice 18 (s18-last): the order items riding it (rustcraft_orders::draw_riding).
    // Wave 2: the plain flow of its item nets (show_flow).
    for key in [rustcraft_rules::net::flow::FLOW_KEY, rustcraft_rules::net::flow::FLOW_ITEMS_KEY] {
        if let Some(v) = entity.nbt.get(&JavaString::from(key)) {
            tag.get_or_insert_with(Compound::new)
                .insert(JavaString::from(key), v.clone());
        }
    }
    for key in [rustcraft_rules::orders::ride::RIDE_KEY, rustcraft_rules::orders::ride::RIDE_ITEMS_KEY] {
        if let Some(v) = entity.nbt.get(&JavaString::from(key)) {
            tag.get_or_insert_with(Compound::new)
                .insert(JavaString::from(key), v.clone());
        }
    }
}

/// On a client: whether the local player holds the wrench.
pub(super) fn wrench_in_hand(game: &Game<'_>) -> bool {
    game.native_held().is_some_and(|(item, _, _)| item == content::WRENCH)
}

/// On a client: the items riding a housing and, with the wrench in hand, its bottleneck frame.
pub(super) fn draw_housing(
    game: &Game<'_>,
    entity: &horizon_client::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    // Slice 18 (s18-last): an order's items riding it move each frame: nothing kept meanwhile.
    let riding = crate::game::rustcraft_orders::draw_riding(game, entity, draw);
    // Wave 2: and the plain flow of its item nets (M5).
    let riding = draw_flow(game, entity, draw) || riding;
    let weak = match entity.nbt.get(&JavaString::from(WEAK_KEY)) {
        Some(Tag::Byte(b)) => *b as u8,
        _ => 0,
    };
    if weak == 0 {
        // F5: a housing under a facade is drawn bare only while the wrench is in hand; kept, it
        // would stay bare after the wrench is put away (the kept geometry is not drawn again
        // when the held item changes, SRC native_block_entity_draw.rs:199-212).
        let covered = crate::game::rustcraft_builder::has_facade(&entity.nbt);
        if !riding && !covered {
            draw.keep();
        }
        return;
    }
    if wrench_in_hand(game) {
        game.native_draw_model(draw, WEAK_MODEL, WEAK_TINT);
    }
}

/// After a rescan: each housing's bottleneck mark written where it changed, and sent to clients.
pub(super) fn mark_weak(game: &mut Game<'_>, level: &server::LevelNet) {
    let weak = rustcraft_rules::net::bottleneck::weak_housings(&level.topology, &level.layout);
    let housings: Vec<Pos> = level.layout.housings.keys().copied().collect();
    for pos in housings {
        let want = u8::from(weak.contains(&pos));
        let Some(entity) = game.native_mod_block_entity(pos) else {
            continue;
        };
        let has = match entity.nbt.get(&JavaString::from(WEAK_KEY)) {
            Some(Tag::Byte(b)) => *b as u8,
            _ => 0,
        };
        if has == want {
            continue;
        }
        game.native_with_mod_block_entity(pos, |e| {
            if want == 0 {
                e.nbt.remove(&JavaString::from(WEAK_KEY));
            } else {
                e.nbt.insert(JavaString::from(WEAK_KEY), Tag::Byte(want as i8));
            }
        });
        game.native_block_entity_sync(pos);
    }
}

// ---- What a housing lost without a player leaves (explosion, /setblock) ----
//
// A housing a player breaks gives its facade back at once (rustcraft_server::on_destroy_block),
// one the wrench lifts too; one that goes another way (an explosion, a command, another mod) is
// found gone at the next rescan, and its facade and the block of its look drop where it stood. A
// housing in an unloaded chunk is not gone. Round 31: its channels are built in, so none drops.

static HANDLED: LazyLock<Mutex<BTreeSet<(String, Pos)>>> = LazyLock::new(Mutex::default);

/// Wave 2: each housing's facade and block of its look as items, as last seen (at a rescan or a
/// facade click), so that a housing gone without a player drops them too: by then its block
/// entity is gone.
static FACADES: LazyLock<Mutex<HashMap<(String, Pos), Vec<String>>>> = LazyLock::new(Mutex::default);

fn facades() -> MutexGuard<'static, HashMap<(String, Pos), Vec<String>>> {
    FACADES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A facade was set, changed or taken off the housing at `pos`: what it would give back now.
pub(super) fn remember_facade(game: &Game<'_>, dimension: &str, pos: Pos) {
    if game.native_block(pos) != content::HOUSING {
        return;
    }
    let items = facade_items(game, pos);
    let key = (dimension.to_owned(), pos);
    if items.is_empty() {
        facades().remove(&key);
    } else {
        facades().insert(key, items);
    }
}

fn handled() -> MutexGuard<'static, BTreeSet<(String, Pos)>> {
    HANDLED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A housing at `pos` broken by a player or lifted: what it held is already dealt with.
pub(super) fn housing_handled(dimension: &str, pos: Pos) {
    handled().insert((dimension.to_owned(), pos));
}

/// The self-test clears its plots: their housings are no loss.
pub(super) fn cleared(dimension: &str, min: Pos, max: Pos) {
    let inside = |p: &Pos| (0..3).all(|i| p[i] >= min[i] && p[i] <= max[i]);
    let gone: Vec<Pos> = server::with_level(dimension, |level| {
        level.layout.housings.keys().copied().filter(inside).collect()
    })
    .unwrap_or_default();
    let mut h = handled();
    for p in gone {
        h.insert((dimension.to_owned(), p));
    }
}

/// After a rescan: the housings of `before` that are gone from a loaded chunk drop their facades.
pub(super) fn drop_lost(
    game: &mut Game<'_>,
    dimension: &str,
    before: &HashMap<Pos, rustcraft_rules::net::topology::Housing>,
    now: &rustcraft_rules::net::topology::Layout,
) {
    let mut lost = Vec::new();
    {
        let mut h = handled();
        for pos in before.keys() {
            if now.housings.contains_key(pos) {
                continue;
            }
            if h.remove(&(dimension.to_owned(), *pos)) {
                continue;
            }
            let chunk = (pos[0] >> 4, pos[2] >> 4);
            if !game.world.chunks.contains_key(&chunk) || game.native_block(*pos) == content::HOUSING {
                continue;
            }
            lost.push(*pos);
        }
        h.retain(|(d, _)| d != dimension);
    }
    for pos in lost {
        // Wave 2: its facade and the block of its look (guide `facade`).
        let covered = facades().remove(&(dimension.to_owned(), pos)).unwrap_or_default();
        for item in covered {
            server::drop_one(game, dimension, pos, &item);
        }
    }
    // What the level's housings would give back now, for the next rescan.
    let now_covered: Vec<(Pos, Vec<String>)> = now
        .housings
        .keys()
        .map(|&p| (p, facade_items(game, p)))
        .filter(|(_, items)| !items.is_empty())
        .collect();
    let mut f = facades();
    f.retain(|(d, _), _| d != dimension);
    for (p, items) in now_covered {
        f.insert((dimension.to_owned(), p), items);
    }
}

// ---- Wave 2: the plain item flow seen in the housings (M5, rustcraft_rules::net::flow) ----

#[derive(Default)]
struct FlowLevel {
    /// This second's moves, as the item nets made them.
    moves: Vec<rustcraft_rules::net::flow::Move>,
    /// The housings that carry a flow now.
    shown: BTreeSet<Pos>,
}

static FLOW: LazyLock<Mutex<HashMap<String, FlowLevel>>> = LazyLock::new(Mutex::default);

fn flow() -> MutexGuard<'static, HashMap<String, FlowLevel>> {
    FLOW.lock().unwrap_or_else(PoisonError::into_inner)
}

/// rustcraft_transport::tick: the moves one item net `key` made.
pub(super) fn moved(
    dimension: &str,
    key: rustcraft_rules::orders::ship::NetKey,
    transfers: &[rustcraft_rules::net::items::Transfer],
) {
    if transfers.is_empty() {
        return;
    }
    let mut f = flow();
    let level = f.entry(dimension.to_owned()).or_default();
    // A second of a busy net is many moves of a few pairs; a pair once is enough to draw it.
    for t in transfers {
        if level.moves.len() >= 4 * rustcraft_rules::net::flow::MAX_HOUSINGS {
            break;
        }
        if !level.moves.iter().any(|m| m.0 == key && m.1 == t.from && m.2 == t.to) {
            level.moves.push((key, t.from, t.to, t.item.clone()));
        }
    }
}

/// Once a second: each housing the second's moves went through gets the stream's spans; one no
/// longer flowing is cleared; a housing whose spans changed goes to clients.
pub(super) fn show_flow(game: &mut Game<'_>, dimension: &str, level: &server::LevelNet) {
    use rustcraft_rules::{net::flow, orders::ride};
    let (moves, before) = {
        let mut f = flow();
        let l = f.entry(dimension.to_owned()).or_default();
        (std::mem::take(&mut l.moves), std::mem::take(&mut l.shown))
    };
    let now = game.world.game_time;
    let tuning = server::tuning();
    let mut want: std::collections::BTreeMap<Pos, Vec<(String, ride::Span)>> = Default::default();
    for (key, from, to, item) in flow::pick(&moves) {
        if want.len() >= flow::MAX_HOUSINGS {
            break;
        }
        let Some(path) = level.transport.path(&level.layout, tuning, key, from, to) else {
            continue;
        };
        let drawn = rustcraft_rules::storage::keys::item_of(&item).to_owned();
        for (housing, span) in flow::spans(&level.layout, &path, from, to, now) {
            want.entry(housing).or_default().push((drawn.clone(), span));
        }
    }
    let touched: BTreeSet<Pos> = before.iter().chain(want.keys()).copied().collect();
    for pos in touched {
        let (ints, items) = ride::encode(want.get(&pos).map_or(&[][..], Vec::as_slice));
        let Some(entity) = game.native_mod_block_entity(pos) else {
            continue;
        };
        let had = match entity.nbt.get(&JavaString::from(flow::FLOW_KEY)) {
            Some(Tag::IntArray(v)) => v.clone(),
            _ => Vec::new(),
        };
        if entity.id != content::HOUSING || had == ints {
            continue;
        }
        game.native_with_mod_block_entity(pos, |e| {
            if ints.is_empty() {
                e.nbt.remove(&JavaString::from(flow::FLOW_KEY));
                e.nbt.remove(&JavaString::from(flow::FLOW_ITEMS_KEY));
            } else {
                let list = items.iter().map(|i| Tag::String(JavaString::from(i.as_str()))).collect();
                e.nbt.insert(JavaString::from(flow::FLOW_KEY), Tag::IntArray(ints.clone()));
                e.nbt.insert(JavaString::from(flow::FLOW_ITEMS_KEY), Tag::List(list));
            }
        });
        game.native_block_entity_sync(pos);
    }
    flow()
        .entry(dimension.to_owned())
        .or_default()
        .shown = want.keys().copied().collect();
}

/// Proposal: the size a flowing item is drawn at, of a block (as an order's riding items).
const FLOW_SCALE: f32 = 0.35;

/// On a client: the plain flow's items in the housing, each where it is this frame; whether any
/// of them is still to come or under way (then its geometry is not kept).
fn draw_flow(
    game: &Game<'_>,
    entity: &horizon_client::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) -> bool {
    use rustcraft_rules::{net::flow, orders::ride};
    let ints = match entity.nbt.get(&JavaString::from(flow::FLOW_KEY)) {
        Some(Tag::IntArray(v)) => v.clone(),
        _ => return false,
    };
    let items: Vec<String> = match entity.nbt.get(&JavaString::from(flow::FLOW_ITEMS_KEY)) {
        Some(Tag::List(list)) => list
            .iter()
            .filter_map(|t| match t {
                Tag::String(s) => Some(s.to_string_lossy()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    let riding = ride::decode(&ints, &items);
    let now = draw.game_time as i32;
    if !riding.iter().any(|(_, s)| now.wrapping_sub(s.t1) <= 0) {
        return false;
    }
    for (k, (item, span)) in riding.iter().enumerate() {
        let Some(at) = ride::place(*span, draw.game_time, draw.partial) else {
            continue;
        };
        draw.push();
        draw.translate(at);
        draw.scale([FLOW_SCALE; 3]);
        game.native_draw_item(
            draw,
            &ItemStack::new(item.as_str(), 1),
            horizon_assets::item_model::DisplayContext::Ground,
            None,
            k as i32,
        );
        draw.pop();
    }
    true
}
