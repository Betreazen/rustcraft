// 0.21 (spec 46, LP4–LP7; the owner 2026-11-19: «турели убивают врагов, мне дается опыт, а также
// предметы тоже засасываются и там в магистрали … со складом туда отправляются»): a turret vacuums
// up its kills' drops (rustcraft_rules::turret_loot decides) into its six loot slots, which the
// item net takes from like any machine's products (its joint's default works both ways, and it
// has no input) and a hopper below it empties. The kill's experience still goes to its owner as
// before (rustcraft_base::deaths).
//
// - rustcraft_base tells this module which turret shot which mob ([`hit`], before each shot's
//   blow) and which mob died where ([`died`], from `native_living_death`); a death within
//   [`HIT_MEMORY`] of the turret's last shot is that turret's kill, whoever owns it.
// - Every `GATHER_EVERY` ticks each turret with recent kills in the level picks up the items near
//   where they died; arrows stay (LP6). What does not fit stays on the ground and its window says
//   so (LP7).
// - A turret saved before 0.21 had only its menu's hidden slot: its slots grow to six here, as the
//   engine grows them when its window opens.
use crate::game::{Game, rustcraft_content as content, rustcraft_machines as machines, rustcraft_mining as mining};
use rustcraft_rules::{
    geom::Pos,
    turret_loot::{self as rules, GATHER_EVERY, Kill, LOOT_SLOTS},
};
use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

/// A death this long after a turret's shot is its kill: rustcraft_base's KILL_MEMORY, the base
/// game's 100 ticks for a player's hit.
const HIT_MEMORY: u64 = 100;

#[derive(Default)]
struct State {
    /// rustcraft_base's tick count, as of its last level tick.
    now: u64,
    /// Mob → the turret that shot it last (its level and place) and when.
    shot_by: HashMap<u32, (String, Pos, u64)>,
    /// Each turret's recent kills.
    kills: HashMap<(String, Pos), Vec<Kill>>,
    /// The turrets that left something they take on the ground at their last gathering.
    stuck: HashSet<(String, Pos)>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The turret at `turret` in `dimension` shoots mob `mob` on tick `now`.
pub(super) fn hit(dimension: &str, turret: Pos, mob: u32, now: u64) {
    state().shot_by.insert(mob, (dimension.to_owned(), turret, now));
}

/// `native_living_death`: mob `mob` died at `at`; a turret's kill when one shot it lately.
pub(super) fn died(mob: u32, at: [f64; 3]) {
    let mut s = state();
    let now = s.now;
    let Some((dimension, turret, tick)) = s.shot_by.remove(&mob) else {
        return;
    };
    if now.saturating_sub(tick) > HIT_MEMORY {
        return;
    }
    s.kills.entry((dimension, turret)).or_default().push(Kill { at, tick: now });
}

/// One tick of the level: every `GATHER_EVERY` ticks its turrets pick up their kills' drops.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, now: u64) {
    let due: Vec<(Pos, Vec<Kill>)> = {
        let mut s = state();
        s.now = now;
        s.shot_by.retain(|_, (_, _, tick)| now.saturating_sub(*tick) <= HIT_MEMORY);
        if !now.is_multiple_of(GATHER_EVERY) {
            return;
        }
        for kills in s.kills.values_mut() {
            *kills = rules::recent(kills, now);
        }
        s.kills.retain(|_, k| !k.is_empty());
        s.kills
            .iter()
            .filter(|((d, _), _)| d == dimension)
            .map(|((_, p), k)| (*p, k.clone()))
            .collect()
    };
    for (pos, kills) in due {
        gather(game, dimension, pos, &kills, now);
    }
}

/// The turret at `pos` picks up what lies near its `kills` into its loot slots.
fn gather(game: &mut Game<'_>, dimension: &str, pos: Pos, kills: &[Kill], now: u64) {
    let key = (dimension.to_owned(), pos);
    if game.native_block(pos) != content::TURRET {
        let mut s = state();
        s.kills.remove(&key);
        s.stuck.remove(&key);
        return;
    }
    let Some((lo, hi)) = rules::search_box(kills) else {
        return;
    };
    let mut stuck = false;
    for e in game.native_item_entities(dimension, lo, hi) {
        if !rules::is_kill_drop(kills, now, e.position, &e.stack.item) {
            continue;
        }
        let max = machines::max_stack(game, &e.stack.item);
        let stack = e.stack.clone();
        let left = game
            .native_with_mod_block_entity(pos, move |b| {
                if b.items.len() < LOOT_SLOTS {
                    b.items.resize(LOOT_SLOTS, None);
                }
                mining::store(&mut b.items[..LOOT_SLOTS], stack, max)
            })
            .flatten();
        let left_count = left.as_ref().map_or(0, |l| l.count);
        stuck |= left_count > 0;
        if left_count == e.stack.count {
            continue;
        }
        let mut rest = e.stack.clone();
        rest.count = left_count;
        game.native_set_item_entity(dimension, e.id, rest);
    }
    let mut s = state();
    if stuck {
        s.stuck.insert(key);
    } else {
        s.stuck.remove(&key);
    }
}

/// LP7: whether the turret at `pos` left its kills' drops on the ground for want of room.
pub(super) fn stuck(dimension: &str, pos: Pos) -> bool {
    state().stuck.contains(&(dimension.to_owned(), pos))
}

/// The self-test's plot cleared: its turrets' kills go.
pub(super) fn forget_area(dimension: &str, min: Pos, max: Pos) {
    let inside = |p: Pos| (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
    let mut s = state();
    s.kills.retain(|(d, p), _| d != dimension || !inside(*p));
    s.stuck.retain(|(d, p)| d != dimension || !inside(*p));
    s.shot_by.retain(|_, (d, p, _)| d.as_str() != dimension || !inside(*p));
}
