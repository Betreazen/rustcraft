// 0.21 (spec 38, FR1–FR6): a fishing station takes its rod from the storage on its item net and
// the storage orders new rods (rustcraft_rules::utility::rods decides).
//
// The owner: «рыболовные станции сами должны отправлять заказы на новые удочки. Я видел, что
// чинить их достаточно дорого, проще крафтить новые… научи рыболовные станции забирать их из
// склада». On its step (rustcraft_farm::fish):
//
// - A station with no rod, or with a worn one, takes the least worn whole rod of that storage
//   straight into its rod cell, as the building machine takes its blocks (FR1). The worn rod goes
//   into the storage; with no room for it there the station keeps it and says «Удочка изношена»
//   (FR2). A rod away at the repair stand is left to its trip.
// - With no whole rod in the storage a worn rod is left to a repair stand on the station's nets,
//   as in 0.20 (FR2); a station with no stand, or with no rod at all, waits for one, and the
//   storage orders rods for its waiting stations: one order at a time, gathered for 3 s, at most
//   16 rods (FR3, FR4). The station's status names the order's state or why it was not taken
//   (FR5).
// - A station with no storage on its nets works as before (FR6).
use crate::game::{
    Game,
    rustcraft_orders as orders,
    rustcraft_server::LevelNet,
    rustcraft_storage as storage,
    rustcraft_utility::with_items,
};
use horizon_client::items::ItemStack;
use rustcraft_rules::{
    geom::Pos,
    orders::queue::Wait,
    storage::surplus::Source,
    utility::{
        Shown,
        rods::{self, Ask, ROD, Step},
    },
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

#[derive(Default)]
struct State {
    /// Each storage's ask for rods, by (dimension, storage id).
    asks: HashMap<(String, u32), Ask>,
    /// The status the last refused order of a storage gave its stations.
    refused: HashMap<(String, u32), &'static str>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Why a station could not take a rod.
enum Lack {
    /// The storage has no whole rod.
    NoRod,
    /// The storage has no room for its worn rod (FR2).
    NoRoom,
}

/// FR1–FR6: the station at `pos` with no rod (`worn` none) or with the worn rod `worn` takes a rod
/// from its storage: the rod it fishes with now, or what it shows instead.
pub(super) fn restock(
    game: &mut Game<'_>,
    dimension: &str,
    level: &LevelNet,
    pos: Pos,
    worn: Option<ItemStack>,
    now: u64,
) -> Result<ItemStack, Shown> {
    let fallback = if worn.is_some() { Shown::Blocked("fish_rod_worn") } else { Shown::Blocked("fish_no_rod") };
    // FR6: no storage on its nets, as before.
    let Some(id) = storage::storage_by(dimension, pos, &level.topology) else {
        return Err(fallback);
    };
    match take(game, dimension, id, pos, worn.as_ref()) {
        Ok(rod) => {
            served(dimension, pos);
            Ok(rod)
        }
        Err(Lack::NoRoom) => Err(fallback),
        // FR2: a worn rod goes to the repair stand of its nets, as in 0.20.
        Err(Lack::NoRod) if worn.is_some() && crate::game::rustcraft_trips::stand_near(game, level, pos) => {
            Err(fallback)
        }
        Err(Lack::NoRod) => Err(ask(dimension, id, pos, now, fallback)),
    }
}

/// The station at `pos` has a rod to fish with: it waits on no storage.
pub(super) fn served(dimension: &str, pos: Pos) {
    let mut s = state();
    for ((d, _), ask) in &mut s.asks {
        if d == dimension && ask.counts(pos) {
            ask.served(pos);
        }
    }
}

/// FR1, FR2: the least worn whole rod of the storage `id` into the station's rod cell, the worn
/// rod `worn` into the storage.
fn take(game: &mut Game<'_>, dimension: &str, id: u32, pos: Pos, worn: Option<&ItemStack>) -> Result<ItemStack, Lack> {
    let worn_key = worn.map(storage::net_key);
    if let Some(k) = &worn_key
        && storage::room_in(game, dimension, id, k, Source::Machine) == 0
    {
        return Err(Lack::NoRoom);
    }
    let max = game.items().get(ROD).map_or(0, |i| i.max_damage);
    let stored: Vec<(String, ItemStack)> = storage::stored_keys_of(dimension, id, ROD)
        .into_iter()
        .map(|(k, _)| {
            let stack = storage::stored_stack(&k, 1);
            (k, stack)
        })
        .collect();
    let damages: Vec<(u32, u32)> = stored.iter().map(|(_, s)| (s.damage, max)).collect();
    let (key, rod) = rods::pick(&damages).map(|i| stored[i].clone()).ok_or(Lack::NoRod)?;
    if storage::take_from(dimension, id, &key, 1) != 1 {
        return Err(Lack::NoRod);
    }
    // The cell must still hold what the step saw: nothing, or the worn rod.
    let was = worn.map(|w| (w.item.to_string(), w.damage));
    let new = rod.clone();
    let placed = with_items(game, pos, move |cells| {
        if cells.is_empty() {
            cells.push(None);
        }
        let now = cells[0].as_ref().filter(|s| s.count > 0).map(|s| (s.item.to_string(), s.damage));
        if now != was {
            return false;
        }
        cells[0] = Some(new);
        true
    }) == Some(true);
    if !placed {
        storage::restore_into(dimension, id, &key, 1);
        return Err(Lack::NoRod);
    }
    let route = format!("storage#{id} -> rustcraft:fishery({})", rustcraft_rules::debug::pos_text(pos));
    crate::game::rustcraft_debug::flow(dimension, &route, &key, 1);
    if let Some(k) = worn_key {
        // The room was counted first; kept, never lost.
        if storage::put_into(game, dimension, id, &k, 1, Source::Machine) == 0 {
            storage::restore_into(dimension, id, &k, 1);
        }
        let back = format!("rustcraft:fishery({}) -> storage#{id}", rustcraft_rules::debug::pos_text(pos));
        crate::game::rustcraft_debug::flow(dimension, &back, &k, 1);
    }
    log::info!("rustcraft: fishing station {pos:?} took a rod from storage #{id}");
    Ok(rod)
}

/// FR3–FR5: the station waits for a rod of the storage `id`; the storage orders rods for its
/// waiting stations once gathered, unless an order for rods is open. What the station shows.
fn ask(dimension: &str, id: u32, pos: Pos, now: u64, fallback: Shown) -> Shown {
    let open = orders::queue_of(dimension, id).into_iter().find(|o| o.item == ROD);
    let key = (dimension.to_owned(), id);
    let step = {
        let mut s = state();
        let ask = s.asks.entry(key.clone()).or_default();
        ask.waits(pos, now);
        ask.step(open.is_some(), now)
    };
    match step {
        Step::Open => Shown::Blocked(match open.map(|o| o.wait) {
            Some(Wait::Items(_)) => "fish_rod_order_items",
            Some(Wait::Machines(_)) => "fish_rod_order_machines",
            _ => "fish_rod_ordered",
        }),
        Step::Gather => fallback,
        Step::Retry => state().refused.get(&key).copied().map_or(fallback, Shown::Blocked),
        Step::Order(n) => match orders::order(dimension, id, ROD, n) {
            Ok(()) => {
                state().refused.remove(&key);
                log::info!("rustcraft: storage #{id} ordered {n} fishing rods for its stations");
                Shown::Blocked("fish_rod_ordered")
            }
            Err((why, _)) => {
                let shown = if why == "rustcraft.orders.low_tier" {
                    "fish_rod_order_low_tier"
                } else {
                    "fish_rod_order_refused"
                };
                let mut s = state();
                if let Some(ask) = s.asks.get_mut(&key) {
                    ask.refused(now);
                }
                s.refused.insert(key, shown);
                log::info!("rustcraft: storage #{id} did not take an order for {n} fishing rods: {why}");
                Shown::Blocked(shown)
            }
        },
    }
}
