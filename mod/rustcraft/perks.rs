// 0.20 (round 31, 4): Path-of-craft-tree's tree on its owner's machines (rustcraft_rules::perks
// decides). PCT keeps a summary on every player (`pct:summary` in the player's persistent data,
// `native_player_persistent_data`, the compound all mods share; PCT's docs/combat-summary.md).
// Once a second on the server's tick this module reads it for the players online and keeps it;
// an owner who left keeps the last summary read, and the world's `data/rustcraft_pct.json` keeps
// them over a restart. A machine's work then takes less time by its owner's level and learned
// ranks in its branch (`speed`): the miner by the miner's branch, the furnace by the smith's, a
// worker's saw by the woodcutter's. Without PCT, or for a machine nobody owns, nothing changes.
// RustCraft never writes PCT's key.
use crate::game::{Game, rustcraft_base as base, rustcraft_content::Machine};
use horizon_world::nbt::Tag;
use rustcraft_rules::{
    geom::Pos,
    perks::{self, Cache},
    utility::Util,
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

#[derive(Default)]
struct State {
    cache: Cache,
    path: Option<PathBuf>,
    loaded: bool,
    dirty: bool,
    /// The last whole second read, so several levels' ticks read once.
    read_at: Option<std::time::Instant>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The world's saved summaries, read the first time.
fn load(game: &Game<'_>, s: &mut State) {
    if s.loaded {
        return;
    }
    let Some(world) = game.native_world_dir() else {
        return;
    };
    let path = world.join("data").join("rustcraft_pct.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        s.cache = Cache::from_json(&text);
    }
    s.path = Some(path);
    s.loaded = true;
}

/// The server's tick: once a second, the summaries of the players online.
pub(super) fn refresh(game: &Game<'_>) {
    let now = std::time::Instant::now();
    {
        let mut s = state();
        if s.read_at.is_some_and(|at| now.duration_since(at).as_millis() < 1000) {
            return;
        }
        s.read_at = Some(now);
        load(game, &mut s);
    }
    for p in game.native_server_players() {
        // None: the engine cannot reach the player now; what was known stays.
        let Some(data) = game.native_player_persistent_data(p.uuid) else {
            continue;
        };
        let held = data.get(perks::KEY).map(to_json);
        let (changed, error) = state().cache.update(p.uuid, held.as_ref());
        if let Some(e) = error {
            log::warn!("rustcraft: pct summary of {} does not read: {e}", p.name);
        }
        if changed {
            state().dirty = true;
            log::info!("rustcraft: pct summary of {} read", p.name);
        }
    }
    save();
}

fn save() {
    let (path, json) = {
        let mut s = state();
        if !s.dirty {
            return;
        }
        s.dirty = false;
        let Some(path) = s.path.clone() else { return };
        (path, s.cache.to_json())
    };
    let fresh = path.with_extension("json.tmp");
    let done = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&fresh, json))
        .and_then(|()| std::fs::rename(&fresh, &path));
    if let Err(e) = done {
        crate::game::rustcraft_debug::journal_warn!("rustcraft: the pct summaries were not saved: {e}");
    }
}

/// An NBT tag as JSON: a string stays a string (PCT's JSON text), a compound becomes an object.
fn to_json(tag: &Tag) -> Value {
    match tag {
        Tag::End => Value::Null,
        Tag::Byte(v) => Value::from(*v),
        Tag::Short(v) => Value::from(*v),
        Tag::Int(v) => Value::from(*v),
        Tag::Long(v) => Value::from(*v),
        Tag::Float(bits) => Value::from(f64::from(f32::from_bits(*bits))),
        Tag::Double(bits) => Value::from(f64::from_bits(*bits)),
        Tag::ByteArray(v) => Value::from(v.iter().map(|b| i64::from(*b as i8)).collect::<Vec<_>>()),
        Tag::String(s) => Value::String(s.to_string_lossy()),
        Tag::List(v) => Value::Array(v.iter().map(to_json).collect()),
        Tag::Compound(c) => Value::Object(c.iter().map(|(k, v)| (k.to_string_lossy(), to_json(v))).collect()),
        Tag::IntArray(v) => Value::from(v.clone()),
        Tag::LongArray(v) => Value::from(v.clone()),
    }
}

/// The branch of the tree a machine's work belongs to (the handbook's table).
pub(super) fn branch_of(machine: Machine) -> Option<&'static str> {
    match machine {
        Machine::Furnace | Machine::Crusher | Machine::Alloy => Some("smithing"),
        Machine::Crafter => Some("crafting"),
        Machine::Mixer => Some("alchemy"),
        Machine::Miner | Machine::Breaker => Some("mining"),
        Machine::Placer => Some("building"),
        Machine::Butcher => Some("husbandry"),
        Machine::Util(u) => perks::util_branch(u),
        _ => None,
    }
}

/// How much faster `owner`'s work of `branch` goes (1 without a summary).
pub(super) fn speed(owner: Option<u128>, branch: Option<&str>) -> f64 {
    state().cache.speed(owner, branch)
}

/// How much faster the machine `machine` at `pos` works for its owner.
pub(super) fn machine_speed(game: &Game<'_>, pos: Pos, machine: Machine) -> f64 {
    let Some(branch) = branch_of(machine) else {
        return 1.0;
    };
    speed(base::owner_at(game, pos), Some(branch))
}

/// A slice-12 machine's.
pub(super) fn util_speed(game: &Game<'_>, pos: Pos, u: Util) -> f64 {
    machine_speed(game, pos, Machine::Util(u))
}

/// The self-test's summary for a made-up owner, as PCT would write it (`summary` its JSON text,
/// none takes it away).
pub(super) fn set_for_selftest(owner: u128, summary: Option<&str>) -> Result<(), String> {
    let held = summary.map(|s| Value::String(s.to_owned()));
    let (_, error) = state().cache.update(owner, held.as_ref());
    error.map_or(Ok(()), Err)
}
