// The zipline's ropes and anchors (12.5 Z1–Z3) and the harpoon on mobs (G5); the rules are
// rustcraft_rules::traversal.
//
// - Z1, on the client: while the player holds the zipline, the block its use would hook is
//   outlined (`native_visuals`, a depth-tested rim of the block's selection shape, API 86's
//   visual sources, game/native_visuals.rs): green when the anchor goes on top of it, red when it
//   has no room, blue on a rope's anchor. Slice 18: over a block that takes it, the anchor's
//   translucent ghost too (`native_world_primitives`' filled boxes of its model's shape).
// - Z2: a use ties a rope: an anchor block at the player's feet and one on the target
//   (`native_set_block`), each with the other end in its block entity (rope id, other end, which
//   one draws). The rope is saved in the world's `data/rustcraft_ropes.json`; a player keeps
//   rules::MAX_ROPES, the oldest coming down for a new one; slice 18: a rope comes down by itself
//   five minutes after it was tied (rules::ROPE_LIFE_MS, the wall clock, kept in the file). Breaking an anchor (`native_destroy_
//   block`) unties the rope: the broken one drops by its loot table, the other goes. Each second a
//   sweep unties a rope whose loaded end is no longer its anchor (an explosion, a piston, the
//   self-test's clearing); the magistral's rescan removes an anchor of a rope that came down
//   while its chunk was away.
// - Clients draw a rope from its start anchor's block entity (`native_block_entity_submit`, kept
//   with `draw.keep()` since it does not move), each anchor a cord from its ring up to the rope, at
//   the rider's hands (rules::ROPE_HANDS), widened for culling to the other end
//   (`native_block_entity_render_bounding_box`); the update tag carries the keys
//   (rustcraft_anim::EXTRA_KEYS).
// - Z3: a use with the look on a rope within reach rides it from there to the end the look points
//   to; a use aimed at an anchor, or a right click on one, rides from the end near the player to
//   the other. No charge, anyone's rope. The ride is the zipline's seat (rustcraft_monorail::
//   zipline, method b2), drawn as nothing (the anchors draw the rope).
// - G5 on mobs: the harpoon's line meeting a mob first hooks it; a small one is moved to the
//   player along the line, a tick at a time at the harpoon's speed (`native_teleport_mob`, which
//   stops its path and fall, game/native_mobs.rs:241-263), only as far as its column has room; a
//   big one pulls the player to it on the harpoon's seat. Seats (invulnerable, no gravity) are
//   never hooked.
use crate::game::{
    Game,
    native_block_entity_draw::BlockEntityDraw,
    rustcraft_monorail as monorail,
    rustcraft_server::{line, tell},
    rustcraft_station::rope_bar,
};
use horizon_client::{
    containers::ModBlockEntity,
    mod_api::{NativeBlockRim, NativeServerPlayer},
    text::Text,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    gear::{self as gear_rules, shot},
    geom::Pos,
    monorail::route::Route,
    ray,
    traversal::{self as rules, ANCHOR, Aim, Cell, End, EndSeen, Hold, Rope, Ropes},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

/// The anchor's block entity: its rope's id, the other end, and whether it draws the rope.
const ROPE_KEY: &str = "zip_rope";
const TO_KEY: &str = "zip_to";
const MAIN_KEY: &str = "zip_main";
/// What clients draw from (rustcraft_anim::update_tag).
pub(super) const KEYS: [&str; 2] = [TO_KEY, MAIN_KEY];
const SWEEP_EVERY: u64 = 20;
/// The eye over the feet, standing (the client has no sneaking flag to read here).
const EYE: f64 = 1.62;
const TICKS_PER_SECOND: f64 = 20.0;
const AIR: &str = "minecraft:air";
/// Z1's outlines: the anchor fits, it does not, a rope's anchor.
const AIM_OK: u32 = 0xFF55_FF55;
const AIM_BAD: u32 = 0xFFFF_5555;
const AIM_ROPE: u32 = 0xFF55_AAFF;
const AIM_WIDTH: f32 = 2.0;

/// A mob the harpoon moves to its player.
struct Pull {
    mob: u32,
    dimension: String,
    route: Route,
    tick: u32,
}

#[derive(Default)]
struct State {
    path: Option<PathBuf>,
    ropes: Ropes,
    ticks: HashMap<String, u64>,
    pulls: Vec<Pull>,
    /// Wave 2: the rope each rider is on, while they ride (its five minutes wait for them).
    riders: HashMap<u128, u64>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The store, read from the world's file the first time a server asks.
fn loaded(game: &Game<'_>) -> MutexGuard<'static, State> {
    let mut s = state();
    if s.path.is_none()
        && let Some(world) = game.native_world_dir()
    {
        let path = world.join("data").join("rustcraft_ropes.json");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<Ropes>(&text) {
                Ok(mut r) => {
                    // Slice 18: ropes of an older store count their five minutes from now.
                    r.stamp(now_ms());
                    s.ropes = r;
                }
                Err(e) => log::warn!("rustcraft: {} does not read: {e}", path.display()),
            }
        }
        s.path = Some(path);
    }
    s
}

fn save(s: &State) {
    let Some(path) = s.path.clone() else { return };
    let Ok(json) = serde_json::to_string(&s.ropes) else {
        return;
    };
    let fresh = path.with_extension("json.tmp");
    let done = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&fresh, json))
        .and_then(|()| std::fs::rename(&fresh, &path));
    if let Err(e) = done {
        log::warn!("rustcraft: the ropes were not saved: {e}");
    }
}

/// `native_level_unload` and the server's stop: the next world reads its own file.
pub(super) fn forget() {
    *state() = State::default();
}

// ---- The world as the rules see it ----

fn cell(game: &Game<'_>, p: Pos) -> Cell {
    match game.native_block(p).as_str() {
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => Cell::Air,
        "minecraft:short_grass" => Cell::Grass,
        "minecraft:water" | "minecraft:tall_grass" => Cell::Through,
        ANCHOR => Cell::Anchor,
        _ if !game.native_block_collision(p).is_empty() => Cell::Solid,
        _ => Cell::Thin,
    }
}

fn aim_of(game: &Game<'_>, eye: [f64; 3], dir: [f64; 3]) -> Aim {
    let path = ray::blocks_along(eye, dir, gear_rules::ZIP_RANGE);
    rules::aim(&path, |p| cell(game, p))
}

fn chunk_loaded(game: &Game<'_>, p: Pos) -> bool {
    game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4))
}

fn rope_id(nbt: &Compound) -> Option<u64> {
    match nbt.get(&JavaString::from(ROPE_KEY)) {
        Some(Tag::Long(v)) => u64::try_from(*v).ok(),
        _ => None,
    }
}

fn other_end(nbt: &Compound) -> Option<Pos> {
    match nbt.get(&JavaString::from(TO_KEY)) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        _ => None,
    }
}

fn draws_rope(nbt: &Compound) -> bool {
    matches!(nbt.get(&JavaString::from(MAIN_KEY)), Some(Tag::Byte(v)) if *v != 0)
}

/// What a sweep finds at `p` for rope `id`.
fn seen(game: &Game<'_>, p: Pos, id: u64) -> EndSeen {
    let loaded = chunk_loaded(game, p);
    let anchor = loaded
        && game.native_block(p) == ANCHOR
        && game
            .native_mod_block_entity(p)
            .and_then(|e| rope_id(&e.nbt))
            == Some(id);
    EndSeen { loaded, anchor }
}

fn eye_of(player: &NativeServerPlayer) -> [f64; 3] {
    let p = player.position;
    [p[0], p[1] + f64::from(player.eye_height), p[2]]
}

// ---- Tying and untying ----

/// Sets the anchor of rope `id` at `p`, its other end `to`; `main` draws the rope.
fn set_anchor(game: &mut Game<'_>, dimension: &str, p: Pos, id: u64, to: Pos, main: bool) -> bool {
    let Some(state_id) = game.native_default_state(ANCHOR) else {
        return false;
    };
    if game.native_block(p) != ANCHOR && !game.native_set_block(dimension, p, state_id, 3) {
        return false;
    }
    let written = game.native_with_mod_block_entity(p, |e| {
        e.nbt.insert(
            JavaString::from(ROPE_KEY),
            Tag::Long(id.min(i64::MAX as u64) as i64),
        );
        e.nbt
            .insert(JavaString::from(TO_KEY), Tag::IntArray(to.to_vec()));
        e.nbt
            .insert(JavaString::from(MAIN_KEY), Tag::Byte(i8::from(main)));
    });
    game.native_block_entity_sync(p);
    written.is_some()
}

/// The wall clock, ms: a rope's five minutes.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The self-test: rope `id` was tied `ms` earlier than it was.
pub(super) fn age_rope_for_selftest(game: &Game<'_>, id: u64, ms: u64) -> Result<(), String> {
    let mut s = loaded(game);
    let r = s.ropes.ropes.iter_mut().find(|r| r.id == id).ok_or(format!("no rope {id}"))?;
    r.tied_ms = r.tied_ms.saturating_sub(ms).max(1);
    Ok(())
}

/// Takes the anchor of rope `id` at `p` away, if it stands there loaded (no drop).
fn clear_anchor(game: &mut Game<'_>, dimension: &str, p: Pos, id: u64) {
    if !seen(game, p, id).anchor {
        return;
    }
    if let Some(air) = game.native_default_state(AIR) {
        game.native_set_block(dimension, p, air, 3);
    }
}

/// Z2: ties `owner`'s rope from `a` to `b` in `dimension`; ropes of theirs beyond the limit come
/// down. Its id; none when an anchor could not be set.
fn tie(game: &mut Game<'_>, owner: u128, dimension: &str, a: Pos, b: Pos) -> Option<(u64, usize)> {
    let (id, untied) = {
        let mut s = loaded(game);
        let out = s.ropes.tie(owner, dimension, a, b);
        s.ropes.stamp(now_ms());
        save(&s);
        out
    };
    for r in &untied {
        clear_anchor(game, &r.dimension, r.a, r.id);
        clear_anchor(game, &r.dimension, r.b, r.id);
    }
    if !(set_anchor(game, dimension, a, id, b, true)
        && set_anchor(game, dimension, b, id, a, false))
    {
        let mut s = state();
        s.ropes.untie(id);
        save(&s);
        drop(s);
        clear_anchor(game, dimension, a, id);
        clear_anchor(game, dimension, b, id);
        return None;
    }
    log::info!("rustcraft: rope {id} tied from {a:?} to {b:?} in {dimension}");
    Some((id, untied.len()))
}

/// Unties rope `id`, taking its anchors away but the one at `breaking`, which the game is
/// breaking now and drops by its loot table.
fn untie(game: &mut Game<'_>, id: u64, breaking: Option<Pos>) {
    let rope = {
        let mut s = loaded(game);
        let r = s.ropes.untie(id);
        save(&s);
        r
    };
    if let Some(r) = rope {
        for p in [r.a, r.b].into_iter().filter(|p| Some(*p) != breaking) {
            clear_anchor(game, &r.dimension, p, r.id);
        }
    }
}

fn rope_at(game: &Game<'_>, dimension: &str, p: Pos) -> Option<Rope> {
    let id = game
        .native_mod_block_entity(p)
        .and_then(|e| rope_id(&e.nbt))?;
    loaded(game)
        .ropes
        .get(id)
        .filter(|r| r.dimension == dimension && r.end_at(p).is_some())
        .cloned()
}

// ---- Riding ----

/// Z3: the player rides `rope` from `start` (their feet's point on it) to the end `to`.
fn ride(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    rope: &Rope,
    start: [f64; 3],
    to: End,
) -> bool {
    let end = rope.at(to);
    let points = [player.position, start, rules::feet(end)];
    let rides = monorail::zipline(
        game,
        player.uuid,
        player.entity_id as u32,
        &player.dimension,
        &points,
        end,
        gear_rules::ZIP_SPEED,
        false,
    );
    // Slice 18: the rider is seen holding the handle with its rollers.
    if rides {
        crate::game::rustcraft_gear::note_zip_ride(player.uuid);
        state().riders.insert(player.uuid, rope.id);
    }
    rides
}

fn ride_from_end(game: &mut Game<'_>, player: &NativeServerPlayer, rope: &Rope, from: End) -> bool {
    ride(game, player, rope, rules::feet(rope.at(from)), from.other())
}

/// What a zipline's use comes to.
pub(super) enum Plan {
    /// It rode a rope or told the player why not: no charge.
    Done,
    /// A new rope from `start` to `far`, for a charge.
    Tie { start: Pos, far: Pos },
}

/// Z2, Z3: a use of the zipline by `player`: a rope in reach is ridden, else a new one is planned.
pub(super) fn plan(game: &mut Game<'_>, player: &NativeServerPlayer) -> Plan {
    if monorail::riding(player.uuid) {
        return Plan::Done;
    }
    let eye = eye_of(player);
    let dir = ray::look_direction(player.yaw, player.pitch);
    let grabbed = {
        let s = loaded(game);
        s.ropes
            .in_level(&player.dimension)
            .filter_map(|r| rules::grab(eye, dir, r).map(|(t, d)| (r.clone(), t, d)))
            .min_by(|x, y| x.2.total_cmp(&y.2))
    };
    if let Some((rope, t, _)) = grabbed {
        let (start, to) = rules::grab_ride(&rope, t, dir);
        ride(game, player, &rope, start, to);
        return Plan::Done;
    }
    match aim_of(game, eye, dir) {
        Aim::Far => tell(
            game,
            player.uuid,
            line(
                "rustcraft.zipline.far",
                "Nothing to hook within 32 blocks",
                &[],
            ),
        ),
        Aim::NoRoom(_) => tell(
            game,
            player.uuid,
            line(
                "rustcraft.zipline.no_room",
                "No room on top of that block",
                &[],
            ),
        ),
        Aim::Anchor(p) => match rope_at(game, &player.dimension, p)
            .map(|r| (rules::end_ride(&r, player.position), r))
        {
            Some((Some((from, _)), rope)) => {
                ride_from_end(game, player, &rope, from);
            }
            Some((None, _)) => tell(
                game,
                player.uuid,
                line(
                    "rustcraft.zipline.come_closer",
                    "Come closer to an end of this rope",
                    &[],
                ),
            ),
            None => tell(
                game,
                player.uuid,
                line(
                    "rustcraft.zipline.no_room",
                    "No room on top of that block",
                    &[],
                ),
            ),
        },
        Aim::Valid { anchor, .. } => {
            let Some(start) = rules::start_cell(player.position, |p| cell(game, p)) else {
                tell(
                    game,
                    player.uuid,
                    line(
                        "rustcraft.zipline.no_start",
                        "Stand on firm ground to tie a rope",
                        &[],
                    ),
                );
                return Plan::Done;
            };
            if rustcraft_rules::monorail::route::distance(rules::feet(start), rules::feet(anchor))
                < rules::MIN_ROPE
            {
                tell(
                    game,
                    player.uuid,
                    line(
                        "rustcraft.zipline.too_close",
                        "Too close to tie a rope",
                        &[],
                    ),
                );
                return Plan::Done;
            }
            return Plan::Tie { start, far: anchor };
        }
    }
    Plan::Done
}

/// Z2: the charge is paid; ties the rope and rides it. False when no rope could be tied.
pub(super) fn tie_and_ride(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    start: Pos,
    far: Pos,
) -> bool {
    let Some((id, untied)) = tie(game, player.uuid, &player.dimension, start, far) else {
        return false;
    };
    if untied > 0 {
        let most = Text::literal(rules::MAX_ROPES.to_string());
        tell(
            game,
            player.uuid,
            line(
                "rustcraft.zipline.oldest_gone",
                "Your oldest rope came down: a player keeps %s ropes",
                &[most],
            ),
        );
    }
    let rope = loaded(game).ropes.get(id).cloned();
    if let Some(rope) = rope {
        ride(game, player, &rope, player.position, End::B);
    }
    true
}

// ---- Hooks ----

/// `native_right_click_block`: a click on a rope's anchor rides it to the other end.
pub(super) fn on_right_click_block(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    if *canceled || game.native_block(pos) != ANCHOR {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let Some(rope) = rope_at(game, &player.dimension, pos) else {
        return;
    };
    let Some(from) = rope.end_at(pos) else { return };
    *canceled = true;
    if !monorail::riding(player.uuid) {
        ride_from_end(game, &player, &rope, from);
    }
}

/// `native_destroy_block`: an anchor broken unties its rope; its loot table drops it.
pub(super) fn on_anchor_broken(game: &mut Game<'_>, dimension: &str, pos: Pos) {
    if let Some(rope) = rope_at(game, dimension, pos) {
        untie(game, rope.id, Some(pos));
    }
}

/// The magistral's rescan found these anchors: one of a rope that came down while it was away
/// goes. An anchor with no rope (placed by hand) stays.
pub(super) fn rescan(game: &mut Game<'_>, dimension: &str, anchors: &[Pos]) {
    let stale: Vec<(Pos, u64)> = {
        let s = loaded(game);
        anchors
            .iter()
            .filter_map(|&p| {
                let id = game
                    .native_mod_block_entity(p)
                    .and_then(|e| rope_id(&e.nbt))?;
                s.ropes.get(id).is_none().then_some((p, id))
            })
            .collect()
    };
    for (p, id) in stale {
        clear_anchor(game, dimension, p, id);
    }
}

/// `native_level_tick_post`: the harpoon's pulls, and each second the sweep of broken ropes.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    pulls(game, dimension);
    let n = {
        let mut s = state();
        let t = s.ticks.entry(dimension.to_owned()).or_default();
        *t += 1;
        *t
    };
    if !n.is_multiple_of(SWEEP_EVERY) {
        return;
    }
    // Slice 18: a rope comes down by itself five minutes after it was tied.
    let expired: Vec<u64> = {
        // Wave 2: a rope somebody rides stays up until they get off.
        state().riders.retain(|uuid, _| monorail::riding(*uuid));
        let s = loaded(game);
        let ridden: std::collections::BTreeSet<u64> = s.riders.values().copied().collect();
        s.ropes
            .expired_free(now_ms(), &ridden)
            .into_iter()
            .filter(|id| s.ropes.get(*id).is_some_and(|r| r.dimension == dimension))
            .collect()
    };
    for id in expired {
        log::info!("rustcraft: rope {id} came down after its five minutes");
        untie(game, id, None);
    }
    let ropes: Vec<Rope> = loaded(game).ropes.in_level(dimension).cloned().collect();
    for r in ropes {
        if rules::broken(seen(game, r.a, r.id), seen(game, r.b, r.id)) {
            log::info!("rustcraft: rope {} lost an anchor and came down", r.id);
            untie(game, r.id, None);
        }
    }
}

// ---- The harpoon on mobs ----

/// G5: the first mob on the harpoon's line within its range before the block `wall` blocks away:
/// its id, width, height and feet. Seats (invulnerable, no gravity) are left out.
pub(super) fn harpoon_target(
    game: &Game<'_>,
    player: &NativeServerPlayer,
    wall: f64,
) -> Option<(u32, f64, f64, [f64; 3])> {
    let me = player.entity_id as u32;
    let targets: Vec<shot::Target> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.alive() && m.id != me && !m.invulnerable_flag && !m.no_gravity)
        .map(|m| {
            let (w, h) = (f64::from(m.width()) / 2.0, f64::from(m.height()));
            let p = m.position;
            shot::Target {
                id: m.id,
                lo: [p.x - w, p.y, p.z - w],
                hi: [p.x + w, p.y + h, p.z + w],
            }
        })
        .collect();
    let eye = eye_of(player);
    let dir = ray::look_direction(player.yaw, player.pitch);
    let id = shot::hit(eye, dir, &targets, wall, gear_rules::HARPOON_RANGE)?;
    let m = game.mobs.iter().find(|m| m.id == id)?;
    Some((
        id,
        f64::from(m.width()),
        f64::from(m.height()),
        [m.position.x, m.position.y, m.position.z],
    ))
}

/// Whether a mob `height` tall has room with its feet at `at` (its middle column).
fn room(game: &Game<'_>, at: [f64; 3], height: f64) -> bool {
    let (x, z) = (at[0].floor() as i32, at[2].floor() as i32);
    let (lo, hi) = (at[1].floor() as i32, (at[1] + height - 0.01).floor() as i32);
    (lo..=hi).all(|y| game.native_block_collision([x, y, z]).is_empty())
}

/// G5: the harpoon, held with feet at `feet`, has hooked mob `id`: a small one is moved to the
/// player, a big one pulls the player (`player`, when there is one) to it. False when nothing
/// moves (no room, or a big mob with no player to pull).
#[allow(clippy::too_many_arguments)]
pub(super) fn hook_mob(
    game: &mut Game<'_>,
    player: Option<&NativeServerPlayer>,
    dimension: &str,
    feet: [f64; 3],
    id: u32,
    width: f64,
    height: f64,
    at: [f64; 3],
) -> bool {
    match rules::hold(width, height) {
        Hold::Mob => {
            let way = rules::samples(at, rules::mob_stop(feet, at, width));
            let n = rules::free_run(&way, |p| room(game, p, height));
            if n == 0 {
                return false;
            }
            let route = rules::way(&[at, way[n - 1]], gear_rules::HARPOON_SPEED);
            if route.points.len() < 2 {
                return false;
            }
            state().pulls.push(Pull {
                mob: id,
                dimension: dimension.to_owned(),
                route,
                tick: 0,
            });
            true
        }
        Hold::Player => {
            let Some(p) = player else { return false };
            let stop = rules::player_stop(feet, at, width);
            let cell = stop.map(|v| v.floor() as i32);
            monorail::zipline(
                game,
                p.uuid,
                p.entity_id as u32,
                dimension,
                &[feet, stop],
                cell,
                gear_rules::HARPOON_SPEED,
                true,
            )
        }
    }
}

/// One tick of each pull of this level.
fn pulls(game: &mut Game<'_>, dimension: &str) {
    let mine: Vec<Pull> = {
        let mut s = state();
        let (mine, rest) = std::mem::take(&mut s.pulls)
            .into_iter()
            .partition(|p| p.dimension == dimension);
        s.pulls = rest;
        mine
    };
    let mut going = Vec::new();
    for mut p in mine {
        p.tick += 1;
        let at = p.route.at_tick(p.tick, TICKS_PER_SECOND);
        if game.native_teleport_mob(dimension, p.mob, at)
            && p.tick < p.route.ticks(TICKS_PER_SECOND)
        {
            going.push(p);
        }
    }
    state().pulls.extend(going);
}

// ---- The client ----

/// `native_visuals` (Z1): the block the zipline in hand would hook, outlined.
pub(super) fn visuals(game: &Game<'_>, rims: &mut Vec<NativeBlockRim>) {
    if game
        .native_held()
        .is_none_or(|(item, _, _)| item != "rustcraft:zipline")
    {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let eye = [p.x, p.y + EYE, p.z];
    let dir = ray::look_direction(p.yaw, p.pitch);
    let (pos, argb) = match aim_of(game, eye, dir) {
        Aim::Valid { block, .. } => (block, AIM_OK),
        Aim::Anchor(a) => (a, AIM_ROPE),
        Aim::NoRoom(b) => (b, AIM_BAD),
        Aim::Far => return,
    };
    rims.push(NativeBlockRim {
        pos,
        argb,
        width: AIM_WIDTH,
    });
}

/// The ghost's colour: the outline's green, translucent.
const GHOST: u32 = 0x6655_FF55;

/// `native_world_primitives` (slice 18): where a zipline's use would set its anchor, the anchor's
/// translucent ghost, with the outline above (guide `zipline`).
pub(super) fn ghost(game: &Game<'_>, primitives: &mut horizon_client::world_primitives::WorldPrimitives) {
    if game.native_held().is_none_or(|(item, _, _)| item != "rustcraft:zipline") {
        return;
    }
    let Some(p) = game.native_player() else { return };
    let eye = [p.x, p.y + EYE, p.z];
    let Aim::Valid { anchor, .. } = aim_of(game, eye, ray::look_direction(p.yaw, p.pitch)) else {
        return;
    };
    for (lo, hi) in rules::ghost_boxes(anchor) {
        primitives.filled_box(lo, hi, GHOST);
    }
}

/// `native_block_entity_submit`: an anchor's cord up to the rope, and the start anchor's rope to
/// the other end (blocks from the anchor's corner).
pub(super) fn draw_anchor(
    game: &Game<'_>,
    pos: Pos,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    let Some(to) = other_end(&entity.nbt) else {
        draw.keep();
        return;
    };
    let hands = [0.5, rules::ROPE_HANDS, 0.5];
    rope_bar(game, draw, [0.5, rules::ANCHOR_TOP, 0.5], hands);
    if draws_rope(&entity.nbt) {
        let d = [to[0] - pos[0], to[1] - pos[1], to[2] - pos[2]].map(f64::from);
        rope_bar(
            game,
            draw,
            hands,
            [d[0] + 0.5, d[1] + rules::ROPE_HANDS, d[2] + 0.5],
        );
    }
    draw.keep();
}

/// `native_block_entity_render_bounding_box`: an anchor draws up to the rope, and the start
/// anchor along it to the other end.
pub(super) fn bounds(pos: Pos, entity: &ModBlockEntity, bounds: &mut [f32; 6]) {
    bounds[4] = bounds[4].max(rules::ROPE_HANDS as f32 + 0.5);
    let Some(to) = other_end(&entity.nbt).filter(|_| draws_rope(&entity.nbt)) else {
        return;
    };
    let d = [to[0] - pos[0], to[1] - pos[1], to[2] - pos[2]].map(|v| v as f32);
    let far = [d[0], d[1] + rules::ROPE_HANDS as f32, d[2]];
    for k in 0..3 {
        bounds[k] = bounds[k].min(far[k] - 0.5);
        bounds[k + 3] = bounds[k + 3].max(far[k] + 1.5);
    }
}

// ---- The self-test ----

pub(super) fn tie_for_selftest(
    game: &mut Game<'_>,
    owner: u128,
    dimension: &str,
    a: Pos,
    b: Pos,
) -> Result<u64, String> {
    tie(game, owner, dimension, a, b)
        .map(|(id, _)| id)
        .ok_or_else(|| format!("no rope tied from {a:?} to {b:?}"))
}

pub(super) fn ropes_for_selftest(game: &Game<'_>) -> Vec<Rope> {
    loaded(game).ropes.ropes.clone()
}

pub(super) fn pull_for_selftest(
    game: &mut Game<'_>,
    dimension: &str,
    feet: [f64; 3],
    id: u32,
) -> Result<(), String> {
    let m = game
        .mobs
        .iter()
        .find(|m| m.id == id && !m.removed)
        .ok_or("no such mob")?;
    let (w, h, at) = (
        f64::from(m.width()),
        f64::from(m.height()),
        [m.position.x, m.position.y, m.position.z],
    );
    if hook_mob(game, None, dimension, feet, id, w, h, at) {
        Ok(())
    } else {
        Err(format!("the mob {id} at {at:?} was not pulled"))
    }
}
