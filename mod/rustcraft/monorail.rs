// The monorail on the server (spec 4.6, T1–T12): rails, stations and the capsule.
//
// The network is a graph kept in a level attachment `rustcraft:monorail` (docs/design/
// 03-architecture.md: the monorail's net lives in the mod's data, not in loaded blocks), so a
// trip knows the rails of chunks nobody has loaded. Each rescan (rustcraft_server) reconciles it
// with the loaded chunks: a rail or station block entity found there is in it with its tier,
// mount and name; one the record has in a loaded chunk but the scan did not find is gone. A rail
// placed or broken by a player is taken in at the next tick, and the shapes of the rails round it
// are set at once (rustcraft_rules::monorail::shape, `native_set_block_property`, API 18).
//
// A station is a machine (rustcraft_machines): it fills a buffer of `station_buffer` E from the
// magistral or a source it touches. Its window (rustcraft_station on the client) is sent its
// listing as a mod message while it is open (`native_player_container_open`). «Ехать» is a menu
// button naming the destination by its place in that listing (`native_menu_button_v2`); the
// trip starts on the level's next tick, when the seat can be spawned.
//
// The capsule moves the player by the method the traversal prototype chose (b2, spec 16.2–16.3,
// decisions round 24): an invisible-to-the-rules seat mob, a pig with no AI and no gravity
// (`native_spawn_mob`, game/native_mobs.rs:167), carries the player (`native_mount`,
// game/native_travel.rs:156), and each level tick the server moves only the seat
// (`native_teleport_mob`, game/native_mobs.rs:244); clients smooth the mob's movement as any
// entity's, and the rider sits on it, so the movement checks see no move of the player's own.
// Every client is told which mob is a capsule and the corners of its way, and draws the guide's
// capsule round it (rustcraft_station, `native_mob_submit`). At the end the player is set down
// and the seat removed; while riding and for FALL_GRACE after, their falls do no damage
// (rustcraft_falls, hooked into `native_living_fall`).
use crate::game::{
    Game,
    native_mobs::NativeSpawn,
    rustcraft_content as content,
    rustcraft_machines::{read_f64, write_f64},
    rustcraft_server::{line, tell},
};
use horizon_client::{
    containers::ModBlockEntity,
    mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Holder, Host},
    text::Text,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    monorail::{
        self, Network, Rail, RailSpec, Station, TripError, capsule,
        route::{self, Route},
        shape, window,
        wire::{self, CapsuleNote, Destination, Listing, Reach, Rename},
    },
    tuning::Tuning,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

/// The station window's listing, server to client.
pub const LISTING: &str = "rustcraft:station";
/// A capsule's mob and way, server to every client.
pub const CAPSULE: &str = "rustcraft:capsule";
/// A station's new name, client to server.
pub const RENAME: &str = "rustcraft:station_name";
/// A station's saved compound: its name, its energy and the stations it has seen.
const NAME_KEY: &str = "name";
const ENERGY_KEY: &str = "energy";
const KNOWN_KEY: &str = "known";
/// The seat: a pig, whose rider sits capsule::SEAT_RISE over it.
const SEAT_KIND: &str = "minecraft:pig";
/// A measured rise outside this is not believed.
const SEAT_RISE_MAX: f64 = 2.5;
/// Ticks the server keeps trying to seat the player.
const MOUNT_TRIES: u32 = 10;
/// How often, in level ticks, open windows get their listing again and every player the
/// capsules on their way.
const SEND_EVERY: u64 = 20;
const CAPSULES_EVERY: u64 = 40;
/// Ticks the capsule stands at the destination before the player is set down: clients show a
/// mob a few ticks behind the server, so this lets them see it arrive and stop.
const ARRIVE_HOLD: u32 = 8;
/// Falls do no damage while riding and this long after.
const FALL_GRACE: Duration = Duration::from_secs(2);
/// A player renames a station from no further than this.
const RENAME_REACH: f64 = 8.0;
/// How far down from the end of the way the server looks for ground to set the player on.
const LAND_DEPTH: i32 = 4;
/// Blocks with no collision the capsule still does not pass through.
const NO_PASSAGE: [&str; 2] = ["minecraft:lava", "minecraft:fire"];

// ---- State ----

/// One level's rails and stations.
#[derive(Default)]
struct LevelRail {
    net: Network,
    loaded: bool,
    dirty: bool,
    /// Rails placed or broken since the last tick: their neighbourhood's shapes are due.
    touched: BTreeSet<Pos>,
    ticks: u64,
}

/// A player with a station's window open, and the destinations they were last sent, in order.
struct Viewer {
    dimension: String,
    station: Pos,
    sent: Option<Vec<Pos>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Spawn,
    Mount(u32),
    Move,
}

/// A trip under way.
struct Ride {
    entity: u32,
    dimension: String,
    from: Pos,
    to: Pos,
    destination: String,
    cost: f64,
    route: Route,
    phase: Phase,
    tick: u32,
    seat: Option<u32>,
    rise: f64,
    measured: bool,
    /// The rider's teleport counter when the move began: any teleport after it is a correction.
    counter_start: Option<i32>,
    corrections: u32,
}

#[derive(Default)]
struct Monorail {
    levels: HashMap<String, LevelRail>,
    viewers: HashMap<u128, Viewer>,
    rides: BTreeMap<u128, Ride>,
    /// «Ехать» pressed: player, level, station, the listing's index.
    requests: Vec<(u128, String, Pos, usize)>,
    renames: Vec<(u128, Rename)>,
    ticks: u64,
}

static STATE: LazyLock<Mutex<Monorail>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, Monorail> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- The saved record ----

#[derive(Clone, Debug, Default)]
struct Saved {
    rails: Vec<(Pos, Rail)>,
    stations: Vec<(Pos, String)>,
}

static SAVED: AttachmentType<Saved> = AttachmentType {
    namespace: "rustcraft",
    name: "monorail",
    default: Saved::default,
    codec: Some(AttachmentCodec {
        encode: encode_saved,
        decode: decode_saved,
    }),
    copy_on_death: false,
    sync: false,
};

/// `{rails: [x, y, z, mount * 8 + tier, …], stations: [{pos: [x, y, z], name}]}`.
fn encode_saved(saved: &Saved) -> Result<Compound, AttachmentError> {
    let mut rails = Vec::with_capacity(saved.rails.len() * 4);
    for (p, r) in &saved.rails {
        rails.extend(p);
        rails.push(i32::from(r.mount.index()) * 8 + i32::from(r.tier));
    }
    let stations = saved
        .stations
        .iter()
        .map(|(p, name)| {
            Tag::compound([
                ("pos", Tag::IntArray(p.to_vec())),
                ("name", Tag::String(JavaString::from(name.as_str()))),
            ])
        })
        .collect();
    Ok(Compound::from([
        (JavaString::from("rails"), Tag::IntArray(rails)),
        (JavaString::from("stations"), Tag::List(stations)),
    ]))
}

fn decode_saved(data: &Compound) -> Result<Saved, AttachmentError> {
    let mut saved = Saved::default();
    if let Some(Tag::IntArray(rails)) = data.get(&JavaString::from("rails")) {
        for c in rails.chunks_exact(4) {
            let (Ok(mount), Ok(tier)) = (u8::try_from(c[3] / 8), u8::try_from(c[3] % 8)) else {
                continue;
            };
            if let Some(mount) = Face::from_index(mount) {
                saved.rails.push(([c[0], c[1], c[2]], Rail { tier, mount }));
            }
        }
    }
    if let Some(Tag::List(list)) = data.get(&JavaString::from("stations")) {
        for t in list {
            let Tag::Compound(c) = t else { continue };
            let pos = match c.get(&JavaString::from("pos")) {
                Some(Tag::IntArray(p)) if p.len() == 3 => [p[0], p[1], p[2]],
                _ => continue,
            };
            let name = match c.get(&JavaString::from("name")) {
                Some(Tag::String(s)) => s.to_string_lossy(),
                _ => String::new(),
            };
            saved.stations.push((pos, name));
        }
    }
    Ok(saved)
}

fn load(game: &mut Game<'_>, level: &mut LevelRail) {
    if level.loaded {
        return;
    }
    let host: &mut dyn Host = game;
    let saved = SAVED.get(host, Holder::Level).unwrap_or_else(|e| {
        log::warn!("rustcraft: the monorail not read: {e:?}");
        Saved::default()
    });
    level.net.rails = saved.rails.into_iter().collect();
    level.net.stations = saved
        .stations
        .into_iter()
        .map(|(p, name)| (p, Station { name, energy: 0.0 }))
        .collect();
    level.loaded = true;
}

fn save(game: &mut Game<'_>, level: &mut LevelRail) {
    if !std::mem::replace(&mut level.dirty, false) {
        return;
    }
    let mut rails: Vec<(Pos, Rail)> = level.net.rails.iter().map(|(p, r)| (*p, *r)).collect();
    rails.sort_by_key(|(p, _)| *p);
    let saved = Saved {
        rails,
        stations: level
            .net
            .stations
            .iter()
            .map(|(p, s)| (*p, s.name.clone()))
            .collect(),
    };
    let host: &mut dyn Host = game;
    if let Err(e) = SAVED.set(host, Holder::Level, saved) {
        log::warn!("rustcraft: the monorail not saved: {e:?}");
    }
}

// ---- Reading the blocks ----

fn chunk_loaded(game: &Game<'_>, p: Pos) -> bool {
    game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4))
}

/// The rail at `pos` as its state says, if a monorail is there.
fn read_rail(game: &Game<'_>, pos: Pos) -> Option<Rail> {
    if game.native_block(pos) != content::MONORAIL {
        return None;
    }
    let tier = game
        .native_block_property(pos, content::TIER)
        .and_then(|t| t.parse().ok())
        .unwrap_or(1);
    let mount = game
        .native_block_property(pos, shape::MOUNT)
        .and_then(|m| shape::mount_of(&m))
        .unwrap_or(Face::YNeg);
    Some(Rail { tier, mount })
}

fn read_name(entity: &ModBlockEntity) -> String {
    match entity.nbt.get(&JavaString::from(NAME_KEY)) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    }
}

fn station_energy(game: &Game<'_>, pos: Pos) -> f64 {
    game.native_mod_block_entity(pos)
        .map_or(0.0, |e| read_f64(&e.nbt, ENERGY_KEY))
}

/// Whether the cabin passes through the block at `p` (capsule::room_cells): one with no
/// collision (air, water, plants, a rail; `native_block_collision`, game/native_hooks.rs:11143),
/// but not lava or fire.
fn passable(game: &Game<'_>, p: Pos) -> bool {
    game.native_block_collision(p).is_empty()
        && !NO_PASSAGE.contains(&game.native_block(p).as_str())
}

/// `rustcraft_server::rescan`: the record against the loaded chunks' rails and stations.
pub(super) fn rescan(game: &mut Game<'_>, dimension: &str, rails: &[Pos], stations: &[Pos]) {
    let mut level = take_level(dimension);
    load(game, &mut level);
    let found_rails: BTreeSet<Pos> = rails.iter().copied().collect();
    let found_stations: BTreeSet<Pos> = stations.iter().copied().collect();
    let gone: Vec<Pos> = level
        .net
        .rails
        .keys()
        .copied()
        .filter(|p| !found_rails.contains(p) && chunk_loaded(game, *p))
        .collect();
    for p in gone {
        level.net.rails.remove(&p);
        level.dirty = true;
    }
    for &p in &found_rails {
        if let Some(rail) = read_rail(game, p)
            && level.net.rails.insert(p, rail) != Some(rail)
        {
            level.dirty = true;
        }
    }
    let gone: Vec<Pos> = level
        .net
        .stations
        .keys()
        .copied()
        .filter(|p| !found_stations.contains(p) && chunk_loaded(game, *p))
        .collect();
    for p in gone {
        level.net.stations.remove(&p);
        level.dirty = true;
    }
    for &p in &found_stations {
        let name = game
            .native_mod_block_entity(p)
            .map(|e| read_name(&e))
            .unwrap_or_default();
        if level.net.stations.get(&p).map(|s| &s.name) != Some(&name) {
            level.net.stations.insert(p, Station { name, energy: 0.0 });
            level.dirty = true;
        }
    }
    update_shapes(game, &level.net, found_rails.iter().copied());
    put_level(dimension, level);
}

/// Sets the shape of each rail of `cells` that has a different one.
fn update_shapes(game: &mut Game<'_>, net: &Network, cells: impl IntoIterator<Item = Pos>) {
    let stations: BTreeSet<Pos> = net.stations.keys().copied().collect();
    for p in cells {
        if !net.rails.contains_key(&p) || !chunk_loaded(game, p) {
            continue;
        }
        let value = shape::shape_of(net, p, &stations).value();
        if game.native_block_property(p, shape::PROPERTY).as_deref() != Some(value.as_str()) {
            game.native_set_block_property(p, shape::PROPERTY, &value);
        }
    }
}

/// The cells whose shape a rail at `p` may change: its own, its neighbours by face and by climb,
/// and the stations' rails beside it.
fn around(p: Pos) -> impl Iterator<Item = Pos> {
    (-1..=1).flat_map(move |dx| {
        (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| [p[0] + dx, p[1] + dy, p[2] + dz]))
    })
}

fn take_level(dimension: &str) -> LevelRail {
    state().levels.remove(dimension).unwrap_or_default()
}

fn put_level(dimension: &str, mut level: LevelRail) {
    let mut s = state();
    if let Some(meanwhile) = s.levels.remove(dimension) {
        level.touched.extend(meanwhile.touched);
    }
    s.levels.insert(dimension.to_owned(), level);
}

// ---- Hooks ----

/// `native_state_for_placement` (game/native_hooks.rs:1776, API 76): a monorail takes its mount
/// from the clicked face (the side its block is behind is the face's opposite) and its tier from
/// the item placing it.
pub(super) fn state_for_placement(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    context: &horizon_world::level::PlacementContext,
    answer: &mut Option<Option<horizon_world::registry::BlockStateId>>,
) {
    if answer.is_some() || level.native_state_block(state) != Some(content::MONORAIL) {
        return;
    }
    let normal = context.face.normal();
    let Some(face) = Face::from_normal([normal[0], normal[1], normal[2]]) else {
        return;
    };
    // A rail hangs on a block, not on another rail: a click on a rail's plate would put the new
    // one in the air over it (vanilla's rails need a sturdy face for the same reason).
    let at = [context.pos.x, context.pos.y, context.pos.z];
    let support = step(at, face.opposite());
    let behind = level.world.block(support[0], support[1], support[2]);
    if level.native_state_block(behind) == Some(content::MONORAIL) {
        *answer = Some(None);
        return;
    }
    let tier = content::monorail_tier(&context.item).unwrap_or(1);
    let placed = level
        .native_state_with_property(state, shape::MOUNT, shape::mount_value(face.opposite()))
        .and_then(|s| level.native_state_with_property(s, content::TIER, &tier.to_string()));
    if let Some(placed) = placed {
        *answer = Some(Some(placed));
    }
}

/// `native_place_block` and `native_destroy_block` of a monorail or a station: taken in at the
/// level's next tick.
pub(super) fn touched(dimension: &str, pos: Pos) {
    let mut s = state();
    s.levels
        .entry(dimension.to_owned())
        .or_default()
        .touched
        .insert(pos);
}

/// `native_player_container_open` and `_close`: who has a station's window open.
pub(super) fn on_container_open(player: u128, dimension: &str, menu: &str, pos: Option<Pos>) {
    if let Some(station) = pos.filter(|_| menu == content::STATION) {
        state().viewers.insert(
            player,
            Viewer {
                dimension: dimension.to_owned(),
                station,
                sent: None,
            },
        );
    }
}

pub(super) fn on_container_close(player: u128, menu: &str) {
    if menu == content::STATION {
        state().viewers.remove(&player);
    }
}

/// `native_menu_button_v2` on a station: «Ехать», for the next tick.
pub(super) fn on_button(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, id: i32) {
    if entity.id != content::STATION {
        return;
    }
    let (Some(index), Some(player)) = (window::go_index(id), game.native_acting_player()) else {
        return;
    };
    state()
        .requests
        .push((player.uuid, player.dimension, pos, index));
}

/// `native_net_from_client`: a station's new name, for the next tick of its level.
pub(super) fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel != RENAME {
        return;
    }
    match wire::decode_rename(data) {
        Some(r) => state().renames.push((player, r)),
        None => log::debug!("rustcraft: a station name from {player:032x} that does not read"),
    }
}

/// `native_player_logged_out`: their trip ends, its seat goes.
pub(super) fn on_logout(game: &mut Game<'_>, player: u128) {
    let ride = {
        let mut s = state();
        s.viewers.remove(&player);
        s.requests.retain(|r| r.0 != player);
        s.rides.remove(&player)
    };
    if let Some(ride) = ride {
        drop_seat(game, &ride);
    }
}

/// `native_server_stopping`: no seat is left in the saved world.
pub(super) fn on_stopping(game: &mut Game<'_>) {
    let rides = std::mem::take(&mut state().rides);
    for ride in rides.values() {
        drop_seat(game, ride);
    }
}

/// `native_level_unload`.
pub(super) fn on_level_unload(dimension: &str) {
    state().levels.remove(dimension);
}

// ---- The station's energy, run by rustcraft_machines ----

/// A station as the machines' tick reads it.
pub struct StationHeld {
    pub energy: f64,
    pub request: f64,
}

pub(super) fn read_station(game: &Game<'_>, pos: Pos, tuning: &Tuning) -> Option<StationHeld> {
    let entity = game.native_mod_block_entity(pos)?;
    let energy = read_f64(&entity.nbt, ENERGY_KEY).clamp(0.0, tuning.station_buffer);
    let room = (tuning.station_buffer - energy).max(0.0) * tuning.ticks_per_second;
    Some(StationHeld {
        energy,
        request: tuning.station_charge.min(room),
    })
}

/// Fills the station's buffer by `share` of its request this tick.
pub(super) fn run_station(
    game: &mut Game<'_>,
    pos: Pos,
    held: StationHeld,
    share: f64,
    tuning: &Tuning,
) {
    let got = share * held.request / tuning.ticks_per_second;
    if got <= 0.0 {
        return;
    }
    let energy = (held.energy + got).min(tuning.station_buffer);
    game.native_with_mod_block_entity(pos, |e| write_f64(&mut e.nbt, ENERGY_KEY, energy));
}

fn add_energy(game: &mut Game<'_>, pos: Pos, by: f64, tuning: &Tuning) {
    let energy = (station_energy(game, pos) + by).clamp(0.0, tuning.station_buffer);
    game.native_with_mod_block_entity(pos, |e| write_f64(&mut e.nbt, ENERGY_KEY, energy));
}

// ---- The level's tick ----

/// `native_level_tick_post` (through rustcraft_server): rails placed or broken, renames, trips
/// asked for and under way, the windows' listings and the capsules' notes.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, tuning: &Tuning) {
    let mut level = take_level(dimension);
    load(game, &mut level);
    level.ticks += 1;
    take_touched(game, &mut level);
    let spec = tuning.rail_spec();
    renames(game, dimension, &mut level);
    requests(game, dimension, &mut level, &spec, tuning);
    rides(game, dimension, tuning);
    if level.ticks.is_multiple_of(SEND_EVERY) || state().viewers.values().any(|v| v.sent.is_none())
    {
        listings(game, dimension, &mut level, &spec, tuning);
    }
    save(game, &mut level);
    put_level(dimension, level);
    if dimension == "minecraft:overworld" {
        let resend = {
            let mut s = state();
            s.ticks += 1;
            s.ticks.is_multiple_of(CAPSULES_EVERY)
        };
        if resend {
            resend_capsules(game);
        }
    }
}

fn take_touched(game: &mut Game<'_>, level: &mut LevelRail) {
    let touched = std::mem::take(&mut level.touched);
    if touched.is_empty() {
        return;
    }
    for &p in &touched {
        match read_rail(game, p) {
            Some(rail) => {
                level.net.rails.insert(p, rail);
            }
            None => {
                level.net.rails.remove(&p);
            }
        }
        if game.native_block(p) == content::STATION {
            let name = game
                .native_mod_block_entity(p)
                .map(|e| read_name(&e))
                .unwrap_or_default();
            level.net.stations.insert(p, Station { name, energy: 0.0 });
        } else {
            level.net.stations.remove(&p);
        }
        level.dirty = true;
    }
    let cells: BTreeSet<Pos> = touched.iter().flat_map(|&p| around(p)).collect();
    update_shapes(game, &level.net, cells);
}

fn renames(game: &mut Game<'_>, dimension: &str, level: &mut LevelRail) {
    let renames = std::mem::take(&mut state().renames);
    let players = game.native_server_players();
    let mut left = Vec::new();
    for (player, rename) in renames {
        let Some(online) = players.iter().find(|p| p.uuid == player) else {
            continue;
        };
        if online.dimension != dimension {
            left.push((player, rename));
            continue;
        }
        let at = rename.station.map(|c| f64::from(c) + 0.5);
        let near = (0..3)
            .map(|i| (online.position[i] - at[i]).powi(2))
            .sum::<f64>()
            <= RENAME_REACH * RENAME_REACH;
        if !near || game.native_block(rename.station) != content::STATION {
            continue;
        }
        let Some(name) = wire::clean_name(&rename.name) else {
            continue;
        };
        let written = name.clone();
        game.native_with_mod_block_entity(rename.station, move |e| {
            e.nbt.insert(
                JavaString::from(NAME_KEY),
                Tag::String(JavaString::from(written.as_str())),
            );
        });
        level.net.stations.entry(rename.station).or_default().name = name;
        level.dirty = true;
        for v in state().viewers.values_mut() {
            v.sent = None;
        }
    }
    state().renames.extend(left);
}

/// The name a station shows: its own or its coordinates.
fn name_text(net: &Network, pos: Pos) -> Text {
    match net.stations.get(&pos).map(|s| s.name.as_str()) {
        Some(name) if !name.is_empty() => Text::literal(name.to_owned()),
        _ => line(
            "rustcraft.station.unnamed",
            "Station %s, %s",
            &[
                Text::literal(pos[0].to_string()),
                Text::literal(pos[2].to_string()),
            ],
        ),
    }
}

fn pos_text(p: Pos) -> Text {
    Text::literal(format!("{} {} {}", p[0], p[1], p[2]))
}

/// What a trip refused shows (T8.a, T8.b).
fn refusal_text(e: &TripError) -> Text {
    match e {
        TripError::NotEnoughEnergy { short } => line(
            "rustcraft.station.short",
            "Not enough energy for the trip: %s E more",
            &[Text::literal(format!("{}", short.ceil()))],
        ),
        TripError::Broken { ends_at } => line(
            "rustcraft.station.broken",
            "The way is broken at %s",
            &[pos_text(*ends_at)],
        ),
        TripError::NoRoom { at } => line(
            "rustcraft.station.no_room",
            "No room for the capsule at %s",
            &[pos_text(*at)],
        ),
        TripError::NoRail(_) | TripError::UnknownStation(_) => line(
            "rustcraft.station.no_rail",
            "The station stands by no rail",
            &[],
        ),
    }
}

/// The network with the live energy of the station at `from`.
fn with_energy(game: &Game<'_>, net: &Network, from: Pos) -> Network {
    let mut net = net.clone();
    if let Some(s) = net.stations.get_mut(&from) {
        s.energy = station_energy(game, from);
    }
    net
}

fn requests(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelRail,
    spec: &RailSpec,
    tuning: &Tuning,
) {
    let mine: Vec<(u128, Pos, usize)> = {
        let mut s = state();
        let (here, other): (Vec<_>, Vec<_>) = std::mem::take(&mut s.requests)
            .into_iter()
            .partition(|r| r.1 == dimension);
        s.requests = other;
        here.into_iter().map(|(p, _, st, i)| (p, st, i)).collect()
    };
    for (player, station, index) in mine {
        let destination = {
            let s = state();
            s.viewers
                .get(&player)
                .filter(|v| v.station == station)
                .and_then(|v| v.sent.as_ref()?.get(index).copied())
        };
        let Some(to) = destination else {
            continue;
        };
        start(game, dimension, level, spec, tuning, player, station, to);
    }
}

/// T8: the trip from `from` to `to` on `net` (with the start station's live energy), checked
/// against the blocks the capsule passes; the start station pays for it as it sends the capsule.
fn plan_and_pay(
    game: &mut Game<'_>,
    net: &Network,
    spec: &RailSpec,
    tuning: &Tuning,
    from: Pos,
    to: Pos,
) -> Result<monorail::Trip, TripError> {
    let trip = monorail::plan_trip(net, spec, from, to, |p| passable(game, p))?;
    add_energy(game, from, -trip.cost, tuning);
    Ok(trip)
}

/// The self-test's «Ехать» without a rider: the trip planned as `start` plans it and, with
/// `pay`, paid for.
pub(super) fn plan_for_selftest(
    game: &mut Game<'_>,
    dimension: &str,
    from: Pos,
    to: Pos,
    pay: bool,
    tuning: &Tuning,
) -> Result<monorail::Trip, TripError> {
    let net = with_energy(game, &network(dimension), from);
    let spec = tuning.rail_spec();
    if pay {
        plan_and_pay(game, &net, &spec, tuning, from, to)
    } else {
        monorail::plan_trip(&net, &spec, from, to, |p| passable(game, p))
    }
}

/// The level's monorail network as the server keeps it.
pub(super) fn network(dimension: &str) -> Network {
    state()
        .levels
        .get(dimension)
        .map(|l| l.net.clone())
        .unwrap_or_default()
}

/// The energy in the buffer of the station at `pos`, if a station stands there.
pub(super) fn energy_at(game: &Game<'_>, pos: Pos) -> Option<f64> {
    (game.native_block(pos) == content::STATION).then(|| station_energy(game, pos))
}

/// The station at `pos` with `energy` in its buffer; false when no station is there.
pub(super) fn set_energy(game: &mut Game<'_>, pos: Pos, energy: f64) -> bool {
    game.native_with_mod_block_entity(pos, |e| write_f64(&mut e.nbt, ENERGY_KEY, energy))
        .is_some()
}

/// The level's monorail record through `through` (the binary NBT a save writes and back) and its
/// codec: Err names what came back different.
pub(super) fn round_trip(
    dimension: &str,
    through: impl Fn(&Compound) -> Result<Compound, String>,
) -> Result<(), String> {
    let net = network(dimension);
    let mut rails: Vec<(Pos, Rail)> = net.rails.iter().map(|(p, r)| (*p, *r)).collect();
    rails.sort_by_key(|(p, _)| *p);
    let saved = Saved {
        rails,
        stations: net
            .stations
            .iter()
            .map(|(p, s)| (*p, s.name.clone()))
            .collect(),
    };
    let encoded = encode_saved(&saved).map_err(|e| format!("{e:?}"))?;
    let back = decode_saved(&through(&encoded)?).map_err(|e| format!("{e:?}"))?;
    if back.rails != saved.rails || back.stations != saved.stations {
        return Err(format!("the monorail {saved:?} came back as {back:?}"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn start(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelRail,
    spec: &RailSpec,
    tuning: &Tuning,
    player: u128,
    from: Pos,
    to: Pos,
) {
    let Some(online) = game
        .native_server_players()
        .into_iter()
        .find(|p| p.uuid == player)
    else {
        return;
    };
    if state().rides.contains_key(&player) {
        tell(
            game,
            player,
            line("rustcraft.station.busy", "You are on your way already", &[]),
        );
        return;
    }
    let Some(guest) = guest_view(game, player) else {
        return;
    };
    if guest.vehicle.is_some() {
        tell(
            game,
            player,
            line("rustcraft.station.riding", "Get off your ride first", &[]),
        );
        return;
    }
    let Ok(entity) = u32::try_from(online.entity_id) else {
        return;
    };
    let net = with_energy(game, &level.net, from);
    let trip = match plan_and_pay(game, &net, spec, tuning, from, to) {
        Ok(trip) => trip,
        Err(e) => {
            tell(game, player, refusal_text(&e));
            for v in state().viewers.values_mut() {
                v.sent = None;
            }
            return;
        }
    };
    let route = route::route(&net, spec, &trip.path);
    close_window(game, player);
    let destination = name_text(&net, to);
    let dest_plain = net
        .stations
        .get(&to)
        .map(|s| s.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("{} {}", to[0], to[2]));
    tell(
        game,
        player,
        line("rustcraft.station.departs", "Trip to %s", &[destination]),
    );
    log::info!(
        "rustcraft: {} leaves {:?} for {:?} «{dest_plain}»: {} rails, {:.1} blocks, {} ticks, {} E",
        online.name,
        from,
        to,
        trip.length,
        route.length(),
        route.ticks(tuning.ticks_per_second),
        trip.cost
    );
    state().rides.insert(
        player,
        Ride {
            entity,
            dimension: dimension.to_owned(),
            from,
            to,
            destination: dest_plain,
            cost: trip.cost,
            route,
            phase: Phase::Spawn,
            tick: 0,
            seat: None,
            rise: capsule::SEAT_RISE,
            measured: false,
            counter_start: None,
            corrections: 0,
        },
    );
}

/// Closes the player's window (the station's menu is a block menu, which `native_close_menu`
/// leaves open, game/native_menus.rs:218-228; `Hub::close_container` closes any).
fn close_window(game: &mut Game<'_>, player: u128) {
    if let Some(conn) = game.native_conn(player)
        && let Some(host) = game.lan.as_mut()
    {
        host.hub.close_container(conn);
    }
}

/// The server's record of a player this tick (`Hub`'s guest), as the traversal prototype reads it.
#[derive(Clone, Copy, Debug)]
struct GuestView {
    position: [f64; 3],
    vehicle: Option<u32>,
    teleport_counter: i32,
}

fn guest_view(game: &Game<'_>, player: u128) -> Option<GuestView> {
    let host = game.lan.as_ref()?;
    let g = host
        .hub
        .guests
        .iter()
        .find(|g| g.placed && g.uuid == player)?;
    Some(GuestView {
        position: g.position,
        vehicle: g.vehicle,
        teleport_counter: g.teleport_counter,
    })
}

fn send(game: &mut Game<'_>, player: u128, channel: &str, bytes: &[u8]) {
    if let Err(e) = game.native_net_send_to_player(player, channel, bytes) {
        log::debug!("rustcraft: {channel} not sent to {player:032x}: {e}");
    }
}

fn send_all(game: &mut Game<'_>, channel: &str, bytes: &[u8]) {
    for p in game.native_server_players() {
        send(game, p.uuid, channel, bytes);
    }
}

fn ride_note(ride: &Ride) -> Option<Vec<u8>> {
    Some(wire::encode_capsule(&CapsuleNote::Ride {
        seat: ride.seat?,
        rise: ride.rise,
        points: ride.route.corners(),
    }))
}

fn resend_capsules(game: &mut Game<'_>) {
    let notes: Vec<Vec<u8>> = state().rides.values().filter_map(ride_note).collect();
    for note in notes {
        send_all(game, CAPSULE, &note);
    }
}

/// Takes the seat away and tells every client the capsule is gone.
fn drop_seat(game: &mut Game<'_>, ride: &Ride) {
    let Some(seat) = ride.seat else {
        return;
    };
    if !game.native_remove_mob(&ride.dimension, seat) {
        log::warn!("rustcraft: the capsule's seat {seat} not removed");
    }
    send_all(
        game,
        CAPSULE,
        &wire::encode_capsule(&CapsuleNote::Gone { seat }),
    );
}

/// Every trip of this level, one tick each.
fn rides(game: &mut Game<'_>, dimension: &str, tuning: &Tuning) {
    let players: Vec<u128> = state()
        .rides
        .iter()
        .filter(|(_, r)| r.dimension == dimension)
        .map(|(p, _)| *p)
        .collect();
    for player in players {
        let Some(mut ride) = state().rides.remove(&player) else {
            continue;
        };
        match advance(game, player, &mut ride, tuning) {
            Ok(false) => {
                state().rides.insert(player, ride);
            }
            Ok(true) => arrive(game, player, &ride),
            Err(reason) => {
                log::info!("rustcraft: the trip of {player:032x} ended: {reason}");
                drop_seat(game, &ride);
            }
        }
    }
}

/// One tick of a trip: Ok(true) when it has arrived.
fn advance(
    game: &mut Game<'_>,
    player: u128,
    ride: &mut Ride,
    tuning: &Tuning,
) -> Result<bool, String> {
    let guest = guest_view(game, player).ok_or("the player is not on the server")?;
    horizon_client::rustcraft_falls::spare_falls(ride.entity, FALL_GRACE);
    match ride.phase {
        Phase::Spawn => {
            let at = sub_y(ride.route.start(), ride.rise);
            let options = NativeSpawn {
                no_ai: true,
                silent: true,
                invulnerable: true,
                no_gravity: true,
                ..NativeSpawn::default()
            };
            let Some(seat) = game.native_spawn_mob(&ride.dimension, SEAT_KIND, at, &options) else {
                refund(game, player, ride, tuning);
                return Err("the seat was not spawned".into());
            };
            ride.seat = Some(seat);
            ride.phase = Phase::Mount(0);
            if let Some(note) = ride_note(ride) {
                send_all(game, CAPSULE, &note);
            }
            Ok(false)
        }
        Phase::Mount(tries) => {
            let seat = ride.seat.ok_or("no seat")?;
            if guest.vehicle == Some(seat) {
                ride.phase = Phase::Move;
                ride.counter_start = Some(guest.teleport_counter);
                return Ok(false);
            }
            if tries >= MOUNT_TRIES {
                refund(game, player, ride, tuning);
                return Err(format!("not seated in {MOUNT_TRIES} ticks"));
            }
            if !game.native_mount(ride.entity, seat) {
                log::debug!("rustcraft: native_mount({}, {seat}) refused", ride.entity);
            }
            ride.phase = Phase::Mount(tries + 1);
            Ok(false)
        }
        Phase::Move => {
            let seat = ride.seat.ok_or("no seat")?;
            if guest.vehicle != Some(seat) {
                tell(
                    game,
                    player,
                    line(
                        "rustcraft.station.left",
                        "The capsule is gone: you got off on the way",
                        &[],
                    ),
                );
                return Err("the player got off".into());
            }
            if !ride.measured {
                // The first seated tick tells how high the rider really sits.
                if let Some(mob) = game.native_mob(&ride.dimension, seat) {
                    let rise = guest.position[1] - mob.pos[1];
                    if rise.is_finite() && rise > 0.0 && rise < SEAT_RISE_MAX {
                        ride.rise = rise;
                    }
                }
                ride.measured = true;
            }
            if let Some(start) = ride.counter_start {
                ride.corrections = guest.teleport_counter.wrapping_sub(start).clamp(0, 1000) as u32;
            }
            ride.tick += 1;
            let moving = ride.route.ticks(tuning.ticks_per_second);
            if ride.tick <= moving {
                let feet = ride.route.at_tick(ride.tick, tuning.ticks_per_second);
                if !game.native_teleport_mob(&ride.dimension, seat, sub_y(feet, ride.rise)) {
                    return Err("the seat could not be moved".into());
                }
            }
            Ok(ride.tick >= moving + ARRIVE_HOLD)
        }
    }
}

fn sub_y(p: [f64; 3], dy: f64) -> [f64; 3] {
    [p[0], p[1] - dy, p[2]]
}

fn refund(game: &mut Game<'_>, player: u128, ride: &Ride, tuning: &Tuning) {
    add_energy(game, ride.from, ride.cost, tuning);
    tell(
        game,
        player,
        line("rustcraft.station.failed", "The capsule did not leave", &[]),
    );
}

/// The trip's end (T9): the player off the seat and set on the ground by the end of the way, the
/// seat gone.
fn arrive(game: &mut Game<'_>, player: u128, ride: &Ride) {
    if let Some(seat) = ride.seat
        && guest_view(game, player).is_some_and(|g| g.vehicle == Some(seat))
        && !game.native_dismount(ride.entity)
    {
        log::warn!("rustcraft: the rider did not get off the capsule");
    }
    drop_seat(game, ride);
    let ground = landing(game, ride.route.end(), ride.to);
    let (yaw, pitch) = game
        .native_server_players()
        .into_iter()
        .find(|p| p.uuid == player)
        .map_or((0.0, 0.0), |p| (p.yaw, p.pitch));
    if !game.native_teleport_player(player, &ride.dimension, ground, yaw, pitch) {
        log::warn!("rustcraft: the rider was not set down at {ground:?}");
    }
    horizon_client::rustcraft_falls::spare_falls(ride.entity, FALL_GRACE);
    tell(
        game,
        player,
        line(
            "rustcraft.station.arrived",
            "Arrived: %s",
            &[Text::literal(ride.destination.clone())],
        ),
    );
    log::info!(
        "rustcraft: {player:032x} arrived at {:?} from {:?}: {} ticks, {} corrections of the rider",
        ride.to,
        ride.from,
        ride.tick,
        ride.corrections
    );
}

/// Where a player at the end `end` of a way stands: the first floor under it, with room for
/// them, within LAND_DEPTH blocks; else beside or on the destination station (a lift's top
/// floor); else the end itself.
fn landing(game: &Game<'_>, end: [f64; 3], station: Pos) -> [f64; 3] {
    let stands = |p: Pos| {
        passable(game, p)
            && passable(game, [p[0], p[1] + 1, p[2]])
            && !passable(game, [p[0], p[1] - 1, p[2]])
    };
    let x = end[0].floor() as i32;
    let z = end[2].floor() as i32;
    let top = (end[1] + 0.01).floor() as i32;
    if let Some(y) = (top - LAND_DEPTH..=top).rev().find(|&y| stands([x, y, z])) {
        return [end[0], f64::from(y), end[2]];
    }
    let beside = [Face::XNeg, Face::XPos, Face::ZNeg, Face::ZPos, Face::YPos]
        .into_iter()
        .map(|f| step(station, f))
        .find(|&p| stands(p));
    beside.map_or(end, |p| {
        [
            f64::from(p[0]) + 0.5,
            f64::from(p[1]),
            f64::from(p[2]) + 0.5,
        ]
    })
}

// ---- The windows ----

/// Sends each player with a station of this level open its listing.
fn listings(
    game: &mut Game<'_>,
    dimension: &str,
    level: &mut LevelRail,
    spec: &RailSpec,
    tuning: &Tuning,
) {
    let viewers: Vec<(u128, Pos)> = state()
        .viewers
        .iter()
        .filter(|(_, v)| v.dimension == dimension)
        .map(|(p, v)| (*p, v.station))
        .collect();
    for (player, station) in viewers {
        let listing = listing(game, level, spec, tuning, station);
        let order: Vec<Pos> = listing.destinations.iter().map(|d| d.station).collect();
        send(game, player, LISTING, &wire::encode_listing(&listing));
        if let Some(v) = state().viewers.get_mut(&player) {
            v.sent = Some(order);
        }
    }
}

/// What the window of the station at `station` shows.
fn listing(
    game: &mut Game<'_>,
    level: &LevelRail,
    spec: &RailSpec,
    tuning: &Tuning,
    station: Pos,
) -> Listing {
    let net = with_energy(game, &level.net, station);
    let energy = net.stations.get(&station).map_or(0.0, |s| s.energy);
    let rail = monorail::station_rail(&net, station);
    // T8.b: a station seen before and cut off since stays in the list with where the way breaks.
    let visible = monorail::visible_stations(&net, station);
    let mut known = read_known(game, station);
    let before = known.clone();
    known.extend(visible.iter().copied());
    known.retain(|p| net.stations.contains_key(p) && *p != station);
    if known != before {
        write_known(game, station, &known);
    }
    let mut destinations: Vec<Destination> = known
        .iter()
        .map(|&to| destination(game, &net, spec, tuning, station, to))
        .collect();
    destinations.sort_by(|a, b| {
        matches!(a.reach, Reach::Broken(_))
            .cmp(&matches!(b.reach, Reach::Broken(_)))
            .then(a.name.cmp(&b.name))
            .then(a.station.cmp(&b.station))
    });
    let segments = rail.map_or_else(Vec::new, |r| {
        let reach: Vec<Pos> = monorail::component(&net, r).into_iter().collect();
        wire::scheme(&reach, |a, b| monorail::neighbours(&net, a).contains(&b))
    });
    Listing {
        station,
        name: net
            .stations
            .get(&station)
            .map(|s| s.name.clone())
            .unwrap_or_default(),
        energy,
        capacity: tuning.station_buffer,
        on_rail: rail.is_some(),
        destinations,
        segments,
    }
}

fn destination(
    game: &Game<'_>,
    net: &Network,
    spec: &RailSpec,
    tuning: &Tuning,
    from: Pos,
    to: Pos,
) -> Destination {
    let name = net
        .stations
        .get(&to)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let measured = |path: &[Pos]| {
        let route = route::route(net, spec, path);
        (
            (path.len() - 1) as u32,
            route.ticks(tuning.ticks_per_second),
            f64::from((path.len() - 1) as u32) / spec.blocks_per_energy,
        )
    };
    let (reach, metrics) = match monorail::plan_trip(net, spec, from, to, |p| passable(game, p)) {
        Ok(trip) => (Reach::Ready, measured(&trip.path)),
        Err(TripError::NotEnoughEnergy { short }) => {
            let path = monorail::station_rail(net, from)
                .zip(monorail::station_rail(net, to))
                .and_then(|(a, b)| monorail::shortest_path(net, a, b).ok());
            (
                Reach::Short(short),
                path.map_or((0, 0, 0.0), |p| measured(&p)),
            )
        }
        Err(TripError::Broken { ends_at }) => (Reach::Broken(ends_at), (0, 0, 0.0)),
        Err(TripError::NoRoom { at }) => {
            let path = monorail::station_rail(net, from)
                .zip(monorail::station_rail(net, to))
                .and_then(|(a, b)| monorail::shortest_path(net, a, b).ok());
            (
                Reach::NoRoom(at),
                path.map_or((0, 0, 0.0), |p| measured(&p)),
            )
        }
        Err(TripError::NoRail(p) | TripError::UnknownStation(p)) => (Reach::Broken(p), (0, 0, 0.0)),
    };
    Destination {
        station: to,
        name,
        reach,
        length: metrics.0,
        ticks: metrics.1,
        cost: metrics.2,
    }
}

fn read_known(game: &Game<'_>, station: Pos) -> BTreeSet<Pos> {
    let Some(entity) = game.native_mod_block_entity(station) else {
        return BTreeSet::new();
    };
    match entity.nbt.get(&JavaString::from(KNOWN_KEY)) {
        Some(Tag::IntArray(v)) => v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
        _ => BTreeSet::new(),
    }
}

fn write_known(game: &mut Game<'_>, station: Pos, known: &BTreeSet<Pos>) {
    let flat: Vec<i32> = known.iter().flatten().copied().collect();
    game.native_with_mod_block_entity(station, move |e| {
        e.nbt
            .insert(JavaString::from(KNOWN_KEY), Tag::IntArray(flat));
    });
}

// ---- What a look shows ----

/// The line over the hotbar for a looked-at station or rail.
pub(super) fn diagnosis(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    block: &str,
    tuning: &Tuning,
) -> Option<Text> {
    if block == content::MONORAIL {
        let rail = read_rail(game, pos)?;
        let speed = tuning.rail_spec().speed_of(rail.tier);
        return Some(line(
            "rustcraft.monorail.look",
            "Monorail · tier %s · %s blocks a second",
            &[
                Text::literal(rail.tier.to_string()),
                Text::literal(format!("{speed}")),
            ],
        ));
    }
    if block != content::STATION {
        return None;
    }
    let s = state();
    let net = &s.levels.get(dimension)?.net;
    if monorail::station_rail(net, pos).is_none() {
        return Some(line(
            "rustcraft.station.no_rail",
            "The station stands by no rail",
            &[],
        ));
    }
    let name = name_text(net, pos);
    drop(s);
    Some(line(
        "rustcraft.station.look",
        "Station «%s» · %s of %s E",
        &[
            name,
            Text::literal(format!("{}", station_energy(game, pos).floor())),
            Text::literal(format!("{}", tuning.station_buffer)),
        ],
    ))
}
