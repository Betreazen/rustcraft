// Slice 18, wave 2: a guard's or turret's kill drops the owner's loot (guide `tool_gun`, `turret`:
// «добыча и опыт как если бы владелец убил сам») when the owner could not strike the blow himself
// (he is in another level, or bleeding finished the mob).
//
// A module of horizon-world's loot.rs (`pub`, so rustcraft_base reaches it as
// `horizon_world::loot::rustcraft_loot_marks`): the loot hooks run in horizon-world, which cannot
// see the bundle (SRC/crates/horizon-world/src/native_load_hooks.rs). rustcraft_base marks where the mobs
// a device hit lately stand (vanilla remembers a player's hit for 100 ticks), each tick and a
// moment before each device's hit; `native_loot_table_load` turns every
// `minecraft:killed_by_player` condition into «killed by a player, or `rustcraft:killed_for_owner`»
// (API 75), and `native_loot_condition` holds that kind at a marked place (API 133; `ctx.place`
// brings the dead mob's position and level, API 156). Plain std and serde_json: the rules crate
// compiles this same file in its tests.
use serde_json::Value;
use std::sync::{Mutex, PoisonError};

/// The condition kind the rewritten tables name.
pub const KIND: &str = "rustcraft:killed_for_owner";
const PLAYER_KILL: &str = "minecraft:killed_by_player";
/// How far from the mark the mob may die (a hit's knock-back) and how long a mark lasts, in the
/// marking side's ticks.
pub const REACH: f64 = 2.0;
pub const LIFE: u64 = 40;

#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub dimension: String,
    /// The mob marked: a newer mark of it takes the old one's place.
    pub id: u32,
    pub at: [f64; 3],
    pub tick: u64,
}

static MARKS: Mutex<Vec<Mark>> = Mutex::new(Vec::new());

/// Mob `id`, which a device hit lately, stands at `at` in `dimension` on tick `tick`.
pub fn mark(dimension: &str, id: u32, at: [f64; 3], tick: u64) {
    let mut m = MARKS.lock().unwrap_or_else(PoisonError::into_inner);
    m.retain(|x| !(x.id == id && x.dimension == dimension));
    m.push(Mark { dimension: dimension.to_owned(), id, at, tick });
}

/// Marks older than `LIFE` at tick `now` go.
pub fn forget_before(now: u64) {
    let mut m = MARKS.lock().unwrap_or_else(PoisonError::into_inner);
    m.retain(|x| now.saturating_sub(x.tick) <= LIFE);
}

/// Whether a mob dying at `at` in `dimension` was marked.
pub fn marked(dimension: &str, at: [f64; 3]) -> bool {
    let m = MARKS.lock().unwrap_or_else(PoisonError::into_inner);
    m.iter().any(|x| near(x, dimension, at))
}

pub fn near(x: &Mark, dimension: &str, at: [f64; 3]) -> bool {
    let d2: f64 = (0..3).map(|i| (x.at[i] - at[i]).powi(2)).sum();
    x.dimension == dimension && d2 <= REACH * REACH
}

/// Every `killed_by_player` condition of a mob's table, anywhere in it, also holds for a marked
/// kill. Other tables stay as they are. Whether it changed anything.
pub fn rewrite(id: &str, table: &mut Value) -> bool {
    if !id.contains("entities/") {
        return false;
    }
    rewrite_value(table)
}

fn rewrite_value(v: &mut Value) -> bool {
    match v {
        Value::Object(o) => {
            if o.get("type").and_then(Value::as_str) == Some(PLAYER_KILL) {
                let original = Value::Object(std::mem::take(o));
                *v = serde_json::json!({
                    "type": "minecraft:any_of",
                    "terms": [original, { "type": KIND }],
                });
                return true;
            }
            let mut changed = false;
            for x in o.values_mut() {
                changed |= rewrite_value(x);
            }
            changed
        }
        Value::Array(a) => a.iter_mut().fold(false, |c, x| rewrite_value(x) | c),
        _ => false,
    }
}

/// `native_loot_condition`: the kind holds where a mark is.
pub fn holds(kind: &str, dimension: Option<&str>, at: Option<[f64; 3]>) -> Option<bool> {
    if kind != KIND {
        return None;
    }
    Some(match (dimension, at) {
        (Some(d), Some(p)) => marked(d, p),
        _ => false,
    })
}
