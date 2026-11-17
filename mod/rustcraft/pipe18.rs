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

/// `native_use_item_on`, before everything else: a node of a higher tier clicked on a node takes
/// its place (U2).
pub(super) fn on_use_item_on(game: &mut Game<'_>, stack: &ItemStack, pos: Pos, claimed: &mut bool) {
    // The sneaking wrench lifts a machine with its contents (guide `wrench`).
    if !*claimed
        && &*stack.item == content::WRENCH
        && let Some(player) = game.native_acting_player()
        && player.sneaking
        && lift_to_player(game, &player, pos)
    {
        *claimed = true;
        return;
    }
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
            let state = game.native_default_state(&stack.item).ok_or("no state")?;
            if !game.native_set_block(&dimension, pos, state, 3) {
                return Err("not placed back".into());
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
    let mut stack = ItemStack::new(&block, 1);
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
            log::warn!("rustcraft: the lifted {} at {pos:?} was lost", rest.item);
        }
    }
    let text = server::line("rustcraft.wrench.lifted", "Lifted with its contents", &[]);
    server::tell(game, player.uuid, text);
    true
}

// ---- The housing on the client: channel strips, bottlenecks, see-through facades ----
//
// Guide `housing`: «сквозь окошки корпуса видно, какие каналы стоят». Clients get each housing's
// slots and its bottleneck mask in its update tag; its block entity draws one tinted strip a
// channel (standalone models, API 151) and, while the local player holds the wrench, a red frame
// round a housing whose channel holds its net back (rustcraft_rules::net::bottleneck) and no
// facade over any housing or node (F5: rustcraft_builder::draw_facade asks `wrench_in_hand`).
// A housing with a bottleneck and every facade are drawn each frame, not kept, so the wrench
// shows its change at once.

/// The housing's saved compound keeps its bottleneck mask under this key.
const WEAK_KEY: &str = "weak";
const SLOTS_KEY: &str = "slots";
const STRIPS: [&str; 4] = [
    "rustcraft:block/housing_strip_0",
    "rustcraft:block/housing_strip_1",
    "rustcraft:block/housing_strip_2",
    "rustcraft:block/housing_strip_3",
];
const WEAK_MODEL: &str = "rustcraft:block/housing_weak";
const WEAK_TINT: u32 = 0xFF3030;

/// `native_standalone_models`: the strips and the bottleneck frame.
pub(super) fn standalone_models(models: &mut Vec<String>) {
    for m in STRIPS.iter().chain(std::iter::once(&WEAK_MODEL)) {
        if !models.iter().any(|x| x == m) {
            models.push((*m).to_owned());
        }
    }
}

/// `native_block_entity_update_tag`: a housing's slots and bottleneck mask go to clients.
pub(super) fn update_tag(entity: &horizon_client::containers::ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id != content::HOUSING {
        return;
    }
    for key in [SLOTS_KEY, WEAK_KEY] {
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

/// On a client: a housing's strips and, with the wrench in hand, its bottleneck frame.
pub(super) fn draw_housing(
    game: &Game<'_>,
    entity: &horizon_client::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    let held = server::read_slots(&entity.nbt);
    for (i, slot) in held.slots.iter().enumerate() {
        if let Some(c) = slot {
            game.native_draw_model(draw, STRIPS[i], rustcraft_rules::net::bottleneck::strip_colour(c.kind));
        }
    }
    let weak = match entity.nbt.get(&JavaString::from(WEAK_KEY)) {
        Some(Tag::Byte(b)) => *b as u8,
        _ => 0,
    };
    if weak == 0 {
        draw.keep();
        return;
    }
    if wrench_in_hand(game) {
        game.native_draw_model(draw, WEAK_MODEL, WEAK_TINT);
    }
}

/// After a rescan: each housing's bottleneck mask written where it changed, and sent to clients.
pub(super) fn mark_weak(game: &mut Game<'_>, level: &server::LevelNet) {
    let weak = rustcraft_rules::net::bottleneck::weak_slots(&level.topology, &level.layout);
    let housings: Vec<Pos> = level.layout.housings.keys().copied().collect();
    for pos in housings {
        let want = weak.get(&pos).copied().unwrap_or(0);
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

// ---- Channels of a housing lost without a player (explosion, /setblock) ----
//
// A housing a player breaks gives its channels back at once (rustcraft_server::on_destroy_block),
// one the wrench lifts too; one that goes another way (an explosion, a command, another mod) is
// found gone at the next rescan, and its channels drop where it stood. A housing in an unloaded
// chunk is not gone.

static HANDLED: LazyLock<Mutex<BTreeSet<(String, Pos)>>> = LazyLock::new(Mutex::default);

fn handled() -> MutexGuard<'static, BTreeSet<(String, Pos)>> {
    HANDLED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A housing at `pos` broken by a player or lifted: its channels are already dealt with.
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

/// After a rescan: the housings of `before` that are gone from a loaded chunk drop their channels.
pub(super) fn drop_lost(
    game: &mut Game<'_>,
    dimension: &str,
    before: &HashMap<Pos, rustcraft_rules::net::topology::Housing>,
    now: &rustcraft_rules::net::topology::Layout,
) {
    let mut lost = Vec::new();
    {
        let mut h = handled();
        for (pos, housing) in before {
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
            lost.push((*pos, *housing));
        }
        h.retain(|(d, _)| d != dimension);
    }
    for (pos, housing) in lost {
        for c in housing.slots.into_iter().flatten() {
            server::drop_one(game, dimension, pos, content::channel_item(c.kind, c.tier));
        }
    }
}
