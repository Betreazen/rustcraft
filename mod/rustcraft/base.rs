// Base defence and base keeping (slice 7): the turret (spec 4.13, round 30 / spec 27, 27.1), the
// base anchor (B1–B6), the owners of slice-7 blocks (X2) and the reports to Path-of-craft-tree
// (spec 7.1, the contract). rustcraft_rules::turret and ::anchor decide; this module finds the
// mobs and players, hurts and buffs them, holds the chunks and shows the windows.
//
// - Shots: `native_hurt_entity` (game/native_entities.rs:375) with no attacker, so the base game
//   and Path-of-craft-tree do not take the kill for a player's own; the owner gets the kill's
//   experience from RustCraft (`native_give_experience`, API 101) and Path-of-craft-tree the
//   `kill` report (D5, X8), both from `native_living_death` of a mob a turret hit lately.
// - Effects: the base game's `MobEffectInstance` on the mob, as `/effect give` puts it on one
//   (game/commands/effects.rs:74-90): poison, slowness, and SoulsCraft's `soulscraft:bleeding`
//   when that effect is registered (`horizon_client::brewing::effect`); without SoulsCraft the
//   turret keeps its own bleeding timer, damage of `minecraft:magic`, which armour does not cut.
// - Resistances: SoulsCraft's mobs.json, where Tuning::soulscraft_mobs says it may be, read once;
//   undead from the world's `#minecraft:undead` entity tag.
// - Support buffs: heal through `native_set_player_status`; guard in
//   `native_living_incoming_damage` for a player in its list; power and haste as attribute
//   modifiers of the base game's `/attribute`, taken off again 2 s after the player left.
// - The anchor: chunk tickets of the mod (`native_force_chunk`, API 412), kept in a level
//   attachment `rustcraft:anchors` so they are held again after a restart (B4).
use crate::game::{
    Game,
    rustcraft_content::{self as content, Kind, Machine},
    rustcraft_machines as machines, rustcraft_mining as mining, rustcraft_power as power,
    rustcraft_server::{LevelNet, line},
    rustcraft_workers as workers,
};
use horizon_client::{
    containers::ModBlockEntity,
    items::ItemStack,
    mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Holder, Host},
    text::Text,
};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    anchor::{self, Anchor, State as AnchorState},
    geom::Pos,
    machines::{
        sheet::{
            Bar as SheetBar, Cursor, Kind as SheetKind, Press, Sheet, Status as SheetStatus,
            press_socket, put_free, severity_code,
        },
        style::Severity,
    },
    modules::{Holder as ModuleHolder, Module},
    reports::{self, Report},
    tuning::Tuning,
    turret::{self, Effect, Loadout, Resist, Role, Shown, SoulsData},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const OWNER_KEY: &str = "owner";
const PLACED_KEY: &str = "placed";
const MODULES_KEY: &str = "modules";
/// A turret's role in its block entity, for other mods (SoulsCraft's altar waves): `attack` or
/// `support`, written whenever its modules change.
const ROLE_KEY: &str = "role";
const MODULE_SLOTS: usize = 3;
const SHOT_TYPE: &str = "minecraft:generic";
const BLEED_TYPE: &str = "minecraft:magic";
const POISON: &str = "minecraft:poison";
const SLOWNESS: &str = "minecraft:slowness";
const OWN_BLEED: &str = "soulscraft:bleeding";
/// A death this long after a turret's hit is the turret's kill (the base game's 100 ticks for a
/// player's).
const KILL_MEMORY: u64 = 100;
/// The chunk ticket of the anchors.
const TICKET: &str = "rustcraft:anchor";
const ANCHOR_EVERY: u64 = 20;
/// Reports kept for the self-test to read.
const KEPT_REPORTS: usize = 512;
const POWER_ID: &str = "rustcraft:turret_power";
const HASTE_ID: &str = "rustcraft:turret_haste";

// ---- State ----

#[derive(Default)]
struct TurretState {
    cooldown: f64,
    heal_wait: f64,
    shown: Option<Shown>,
    target: Option<String>,
    /// The target's entity id: whether the engine lets poison onto it.
    target_id: Option<u32>,
}

/// A mob a turret hit: its owner and when.
#[derive(Clone, Copy)]
struct Hit {
    owner: u128,
    tick: u64,
}

/// The turret's own bleeding on a mob, without SoulsCraft.
struct Bleed {
    dimension: String,
    damage: f64,
    until: u64,
}

/// What support turrets give one player now.
#[derive(Clone, Copy, Default, PartialEq)]
struct Buff {
    guard: f64,
    power: f64,
    haste: f64,
    until: u64,
}

#[derive(Default)]
struct Base {
    ticks: u64,
    turrets: HashMap<(String, Pos), TurretState>,
    hits: HashMap<u32, Hit>,
    bleeds: HashMap<u32, Bleed>,
    deaths: Vec<(u32, String)>,
    /// Player → the buffs on them now, and what was applied as modifiers.
    buffs: HashMap<u128, Buff>,
    applied: HashMap<u128, (f64, f64)>,
    /// Player entity id → the guard fraction (read in the damage hook).
    guards: HashMap<i32, f64>,
    /// Anchors: share of their draw each got last tick, their states, the chunks held.
    shares: HashMap<Pos, f64>,
    anchor_states: HashMap<(String, Pos), AnchorState>,
    held: HashMap<String, BTreeSet<[i32; 2]>>,
    anchors: HashMap<String, Vec<Anchor>>,
    anchors_loaded: BTreeSet<String>,
    /// Blocks placed by a player, owners due.
    owners_due: Vec<(u128, String, Pos)>,
    broken: Vec<(String, Pos)>,
    /// Path-of-craft-tree.
    reports: Vec<Report>,
    pct: Option<bool>,
    sent: VecDeque<String>,
    counts: BTreeMap<&'static str, u64>,
    souls: Option<Option<SoulsData>>,
    sessions: HashMap<u128, Session>,
    opening: Vec<(u128, String, Pos)>,
    clicks: Vec<(u128, i32, usize)>,
    /// `rustcraft_testkit turret`: turrets to place on the Overworld's next tick, and the placed
    /// ones whose buffer is kept full.
    testkit_due: Vec<(Pos, Vec<Option<Module>>, Option<u128>)>,
    testkit: BTreeSet<(String, Pos)>,
}

struct Session {
    dimension: String,
    pos: Pos,
    window: i32,
}

static STATE: LazyLock<Mutex<Base>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, Base> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- Owners and modules in the block entity ----

fn read_owner(nbt: &Compound) -> Option<u128> {
    match nbt.get(&JavaString::from(OWNER_KEY)) {
        Some(Tag::String(s)) => u128::from_str_radix(&s.to_string_lossy(), 16).ok(),
        _ => None,
    }
}

pub(super) fn owner_at(game: &Game<'_>, pos: Pos) -> Option<u128> {
    game.native_mod_block_entity(pos)
        .and_then(|e| read_owner(&e.nbt))
}

pub(super) fn set_owner(game: &mut Game<'_>, pos: Pos, owner: u128, placed: u64) -> bool {
    game.native_with_mod_block_entity(pos, |e| {
        e.nbt.insert(
            JavaString::from(OWNER_KEY),
            Tag::String(JavaString::from(format!("{owner:032x}").as_str())),
        );
        e.nbt
            .insert(JavaString::from(PLACED_KEY), Tag::Long(placed as i64));
    })
    .is_some()
}

fn read_modules(nbt: &Compound) -> Vec<Option<String>> {
    let mut out = vec![None; MODULE_SLOTS];
    if let Some(Tag::List(list)) = nbt.get(&JavaString::from(MODULES_KEY)) {
        for (i, t) in list.iter().take(MODULE_SLOTS).enumerate() {
            if let Tag::String(s) = t {
                let s = s.to_string_lossy();
                out[i] = (!s.is_empty()).then_some(s);
            }
        }
    }
    out
}

fn write_modules(nbt: &mut Compound, modules: &[Option<String>]) {
    let list = modules
        .iter()
        .map(|m| Tag::String(JavaString::from(m.as_deref().unwrap_or(""))))
        .collect();
    nbt.insert(JavaString::from(MODULES_KEY), Tag::List(list));
}

pub(super) fn modules_at(game: &Game<'_>, pos: Pos) -> Vec<Option<Module>> {
    game.native_mod_block_entity(pos)
        .map(|e| rustcraft_rules::modules::read(&read_modules(&e.nbt)))
        .unwrap_or_else(|| vec![None; MODULE_SLOTS])
}

pub(super) fn set_modules(game: &mut Game<'_>, pos: Pos, modules: &[Option<Module>]) -> bool {
    let items: Vec<Option<String>> = modules
        .iter()
        .map(|m| m.map(|m| m.item().to_owned()))
        .collect();
    let turret = game.native_block(pos) == content::TURRET;
    let role = Loadout::of(modules).role();
    game.native_with_mod_block_entity(pos, |e| {
        write_modules(&mut e.nbt, &items);
        if turret {
            write_role(&mut e.nbt, role);
        }
    })
    .is_some()
}

fn write_role(nbt: &mut Compound, role: Role) {
    let word = match role {
        Role::Attack => "attack",
        Role::Support => "support",
    };
    nbt.insert(
        JavaString::from(ROLE_KEY),
        Tag::String(JavaString::from(word)),
    );
}

/// The turret's role as its block entity says it, for the self-test.
pub(super) fn role_at(game: &Game<'_>, pos: Pos) -> Option<String> {
    match game
        .native_mod_block_entity(pos)?
        .nbt
        .get(&JavaString::from(ROLE_KEY))
    {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

// ---- Path-of-craft-tree ----

/// A report to send at the next tick.
pub(super) fn report(r: Report) {
    state().reports.push(r);
}

/// Sends the reports waiting, once a tick of the server: only where commands run (no player's
/// state swapped in). Without Path-of-craft-tree they are counted and dropped.
fn flush_reports(game: &mut Game<'_>) {
    let (due, known) = {
        let mut s = state();
        if s.reports.is_empty() {
            return;
        }
        (std::mem::take(&mut s.reports), s.pct)
    };
    let present = known.unwrap_or_else(|| {
        let p = game.native_command_info(reports::COMMAND).is_some();
        log::info!(
            "rustcraft: Path-of-craft-tree reports {}",
            if p {
                "on"
            } else {
                "off: no pct-machine command"
            }
        );
        state().pct = Some(p);
        p
    });
    for r in due {
        let line = r.command();
        let ran = present && game.native_run_mod_command(&line);
        let mut s = state();
        *s.counts
            .entry(if ran { r.kind() } else { "unsent" })
            .or_default() += 1;
        if s.sent.len() >= KEPT_REPORTS {
            s.sent.pop_front();
        }
        s.sent
            .push_back(if ran { line } else { format!("unsent {line}") });
    }
}

/// The report lines sent lately, for the self-test.
pub(super) fn sent_reports() -> Vec<String> {
    state().sent.iter().cloned().collect()
}

// ---- Hooks ----

/// The blocks with an owner: slice 7's and slice 8's.
fn has_owner(block: &str) -> bool {
    Machine::of(block).is_some_and(Machine::owned)
}

/// The blocks whose window is a server menu: the solar panel alone since slice 9.
fn has_server_window(block: &str) -> bool {
    Machine::of(block).is_some_and(|m| m.owned() && !m.has_menu())
}

/// `native_place_block`: a slice-7 or slice-8 block placed by a player is theirs (X2).
pub(super) fn on_place_block(game: &mut Game<'_>, stack: &ItemStack, pos: Pos) {
    if !has_owner(&stack.item) {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    state()
        .owners_due
        .push((player.uuid, player.dimension.clone(), pos));
}

/// `native_destroy_block`: a turret's or an anchor's modules go back to the player.
pub(super) fn on_destroy_block(game: &mut Game<'_>, pos: Pos, dimension: &str, creative: bool) {
    let block = game.native_block(pos);
    if holder_of(&block).is_none() {
        return;
    }
    state().broken.push((dimension.to_owned(), pos));
    if creative {
        return;
    }
    for m in modules_at(game, pos).into_iter().flatten() {
        game.native_give_items(m.item(), 1);
    }
}

/// `native_living_death`: a mob a turret hit lately died; its kill is reported at the next tick.
pub(super) fn on_living_death(entity: u32, kind: &str) {
    let mut s = state();
    if s.hits.contains_key(&entity) {
        s.deaths.push((entity, kind.to_owned()));
    }
    s.bleeds.remove(&entity);
}

/// `native_living_incoming_damage`: a guard turret's players take less (spec 27 «Защита»).
pub(super) fn on_incoming_damage(entity: i32, kind: &str, amount: &mut f32) {
    if kind != "minecraft:player" {
        return;
    }
    if let Some(g) = state().guards.get(&entity) {
        *amount *= (1.0 - g.clamp(0.0, 1.0)) as f32;
    }
}

/// `native_right_click_block`: a turret, an anchor or a dock opens its window.
pub(super) fn on_right_click_block(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    let block = game.native_block(pos);
    if !has_server_window(&block) || *canceled {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    if player.sneaking {
        return;
    }
    // A panel or a battery in hand goes on beside the one clicked: structures grow so.
    let held = game
        .native_player_stacks(player.uuid)
        .and_then(|s| s.get(usize::from(player.selected_slot)).cloned().flatten());
    if held.is_some_and(|h| *h.item == *block) {
        return;
    }
    *canceled = true;
    state()
        .opening
        .push((player.uuid, player.dimension.clone(), pos));
}

pub(super) fn on_menu_clicked(uuid: u128, window: i32, slot: i32, mode: i32) {
    let mut s = state();
    if !s.sessions.get(&uuid).is_some_and(|x| x.window == window) || mode > 1 {
        return;
    }
    if let Ok(slot) = usize::try_from(slot) {
        s.clicks.push((uuid, window, slot));
    }
}

pub(super) fn on_menu_closed(uuid: u128, window: i32) {
    let mut s = state();
    if s.sessions.get(&uuid).is_some_and(|x| x.window == window) {
        s.sessions.remove(&uuid);
    }
}

/// `native_player_logged_out` and `native_server_stopping`: the buffs come off.
pub(super) fn on_logout(game: &mut Game<'_>, player: u128) {
    let had = {
        let mut s = state();
        s.buffs.remove(&player);
        s.applied.remove(&player)
    };
    if had.is_some() {
        let name = game
            .native_server_players()
            .into_iter()
            .find(|p| p.uuid == player)
            .map(|p| p.name);
        if let Some(name) = name {
            remove_modifiers(game, &name);
        }
    }
}

pub(super) fn on_stopping(game: &mut Game<'_>) {
    let players: Vec<u128> = state().applied.keys().copied().collect();
    for p in players {
        on_logout(game, p);
    }
}

// ---- The tick ----

/// One tick of the level's turrets and anchors, then the owners due and the reports.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &LevelNet, t: &Tuning) {
    let now = {
        let mut s = state();
        s.ticks += 1;
        s.ticks
    };
    owners(game, dimension);
    let turrets: Vec<Pos> = level
        .machines
        .iter()
        .filter(|(_, m)| **m == Machine::Turret)
        .map(|(p, _)| *p)
        .collect();
    for pos in turrets {
        run_turret(game, dimension, pos, now, t);
    }
    bleed(game, dimension, now, t);
    deaths(game, t);
    if now.is_multiple_of(ANCHOR_EVERY) {
        buffs(game, now);
        anchors(game, dimension, level, t);
    }
    windows(game, dimension, level, t);
    crate::game::rustcraft_sheets::tick(game, dimension, level, t);
    flush_reports(game);
}

fn owners(game: &mut Game<'_>, dimension: &str) {
    let due: Vec<(u128, Pos)> = {
        let mut s = state();
        let (here, later): (Vec<_>, Vec<_>) = std::mem::take(&mut s.owners_due)
            .into_iter()
            .partition(|o| o.1 == dimension);
        s.owners_due = later;
        here.into_iter().map(|(u, _, p)| (u, p)).collect()
    };
    for (owner, pos) in due {
        let placed = now_ms();
        if !set_owner(game, pos, owner, placed) {
            continue;
        }
        let block = game.native_block(pos);
        if block == content::ANCHOR {
            add_anchor(game, dimension, Anchor { pos, owner, placed });
        } else if block == content::TURRET {
            let slots = modules_at(game, pos);
            set_modules(game, pos, &slots);
        }
    }
    testkit(game, dimension);
}

// ---- The turret ----

fn souls(game: &Game<'_>, t: &Tuning) -> Option<SoulsData> {
    if let Some(d) = &state().souls {
        return d.clone();
    }
    let root = game
        .native_world_dir()
        .and_then(|w| w.parent().map(std::path::Path::to_path_buf));
    let data = root.and_then(|root| {
        t.soulscraft_mobs.iter().find_map(|rel| {
            let text = std::fs::read_to_string(root.join(rel)).ok()?;
            let d = turret::parse_souls(&text);
            if d.is_some() {
                log::info!("rustcraft: turret resistances from {rel}");
            }
            d
        })
    });
    if data.is_none() {
        log::info!("rustcraft: no SoulsCraft mobs.json; turrets use the base game's rules only");
    }
    state().souls = Some(data.clone());
    data
}

/// An attack effect's name for the window, in the player's language.
fn effect_name(e: Effect) -> Text {
    match e {
        Effect::Bleed => line("rustcraft.turret.effect.bleed", "bleeding", &[]),
        Effect::Poison => line("rustcraft.turret.effect.poison", "poison", &[]),
        Effect::Slow => line("rustcraft.turret.effect.slow", "slowness", &[]),
    }
}

/// Parts of one window row, joined with " · ".
fn joined(parts: Vec<Text>) -> Text {
    let mut row = Text::literal("");
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            row.push(Text::literal(" · "));
        }
        row.push(p);
    }
    row
}

/// Whether poison takes on mob `id`: the `#minecraft:undead` tag and the engine's own refusals
/// (a mob type with `flags.undead`, a spider; `Mob::can_be_affected`, mobs.rs:4323).
fn poison_refused(game: &Game<'_>, id: u32, kind: &str) -> bool {
    undead(game, kind)
        || game
            .mobs
            .iter()
            .find(|m| m.id == id && !m.removed)
            .is_some_and(|m| !m.can_be_affected(POISON))
}

fn undead(game: &Game<'_>, kind: &str) -> bool {
    game.stations()
        .enchantments
        .entity_tags
        .get("minecraft:undead")
        .map_or_else(|| turret::UNDEAD.contains(&kind), |tag| tag.contains(kind))
}

/// Gives mob `id` the effect `effect` as `/effect give` would; whether it took.
fn give_mob_effect(game: &mut Game<'_>, id: u32, effect: &str, ticks: u32, amplifier: u8) -> bool {
    let Some(def) = horizon_client::brewing::effect(effect) else {
        return false;
    };
    let name = horizon_client::brewing::namespaced(def.id);
    game.mobs
        .iter_mut()
        .find(|m| m.id == id && !m.removed)
        .is_some_and(|m| m.add_effect_with(name, ticks as i32, i32::from(amplifier), false, true))
}

/// Whether SoulsCraft's bleeding is a registered effect.
fn shared_bleeding() -> bool {
    horizon_client::brewing::effect(OWN_BLEED).is_some()
}

fn center(pos: Pos) -> [f64; 3] {
    [
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + 1.0,
        f64::from(pos[2]) + 0.5,
    ]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn run_turret(game: &mut Game<'_>, dimension: &str, pos: Pos, now: u64, t: &Tuning) {
    let slots = modules_at(game, pos);
    let l = Loadout::of(&slots);
    let owner = owner_at(game, pos);
    let key = (dimension.to_owned(), pos);
    let mut ts = state().turrets.remove(&key).unwrap_or_default();
    if state().testkit.contains(&key) {
        machines::set_buffer_energy(game, pos, t.turret_buffer);
    }
    let energy = machines::buffer_energy(game, pos);
    let doing = match l.role() {
        Role::Attack => attack(game, dimension, pos, &l, owner, energy, now, &mut ts, t),
        Role::Support => support(game, pos, &l, energy, now, &mut ts, t),
    };
    ts.shown = Some(doing);
    state().turrets.insert(key, ts);
}

#[allow(clippy::too_many_arguments)]
fn attack(
    game: &mut Game<'_>,
    dimension: &str,
    pos: Pos,
    l: &Loadout,
    owner: Option<u128>,
    energy: f64,
    now: u64,
    ts: &mut TurretState,
    t: &Tuning,
) -> Shown {
    ts.cooldown -= 1.0;
    let at = center(pos);
    let range = l.range(t);
    let target = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.alive() && m.is_enemy())
        .map(|m| {
            (
                m.id,
                m.kind.id.to_string(),
                distance(at, [m.position.x, m.position.y, m.position.z]),
            )
        })
        .filter(|(_, _, d)| *d <= range)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((id, kind, _)) = target else {
        ts.target = None;
        ts.target_id = None;
        return turret::shown(l, true, Shown::NoTargets);
    };
    ts.target = Some(kind.clone());
    ts.target_id = Some(id);
    let cost = l.shot_energy(t);
    if energy < cost {
        return turret::shown(l, false, Shown::NoEnergy);
    }
    if ts.cooldown > 0.0 {
        return turret::shown(l, true, Shown::Shooting(kind));
    }
    ts.cooldown = l.interval(t);
    machines::set_buffer_energy(game, pos, energy - cost);
    if let Some(owner) = owner {
        state().hits.insert(id, Hit { owner, tick: now });
    }
    let resist = souls(game, t)
        .and_then(|d| d.resist.get(&kind).cloned())
        .unwrap_or_else(Resist::default);
    let is_undead = poison_refused(game, id, &kind);
    for g in turret::effects(l, t, &resist, is_undead) {
        match g.effect {
            Effect::Poison => {
                give_mob_effect(game, id, POISON, g.ticks, g.amplifier);
            }
            Effect::Slow => {
                give_mob_effect(game, id, SLOWNESS, g.ticks, g.amplifier);
            }
            Effect::Bleed => {
                if !(shared_bleeding()
                    && give_mob_effect(game, id, OWN_BLEED, g.ticks, g.amplifier))
                {
                    let until = now + u64::from(g.ticks);
                    state().bleeds.insert(
                        id,
                        Bleed {
                            dimension: dimension.to_owned(),
                            damage: g.damage,
                            until,
                        },
                    );
                }
            }
        }
    }
    game.native_hurt_entity(dimension, id, t.turret_damage as f32, SHOT_TYPE, None);
    turret::shown(l, true, Shown::Shooting(kind))
}

/// The turret's own bleeding: its damage once a second while it lasts.
fn bleed(game: &mut Game<'_>, dimension: &str, now: u64, t: &Tuning) {
    let period = t.ticks_per_second.max(1.0) as u64;
    if !now.is_multiple_of(period) {
        return;
    }
    let due: Vec<(u32, f64)> = {
        let mut s = state();
        s.bleeds.retain(|_, b| b.until > now);
        s.bleeds
            .iter()
            .filter(|(_, b)| b.dimension == dimension)
            .map(|(id, b)| (*id, b.damage))
            .collect()
    };
    for (id, damage) in due {
        if !game.native_hurt_entity(dimension, id, damage as f32, BLEED_TYPE, None) {
            state().bleeds.remove(&id);
        }
    }
}

/// D5, X8: a turret's kill gives its owner the kill's experience and Path-of-craft-tree a `kill`.
fn deaths(game: &mut Game<'_>, t: &Tuning) {
    let (deaths, now) = {
        let mut s = state();
        (std::mem::take(&mut s.deaths), s.ticks)
    };
    for (entity, kind) in deaths {
        let Some(hit) = state().hits.remove(&entity) else {
            continue;
        };
        if now.saturating_sub(hit.tick) > KILL_MEMORY {
            continue;
        }
        game.native_give_experience(hit.owner, t.turret_kill_experience);
        report(Report::Kill {
            owner: hit.owner,
            entity,
            kind,
        });
    }
    state()
        .hits
        .retain(|_, h| now.saturating_sub(h.tick) <= KILL_MEMORY);
}

fn support(
    game: &mut Game<'_>,
    pos: Pos,
    l: &Loadout,
    energy: f64,
    now: u64,
    ts: &mut TurretState,
    t: &Tuning,
) -> Shown {
    let at = center(pos);
    let range = l.range(t);
    let players: Vec<_> = game
        .native_server_players()
        .into_iter()
        .filter(|p| p.health > 0.0 && p.game_mode != 3 && distance(at, p.position) <= range)
        .collect();
    if players.is_empty() {
        return turret::shown(l, true, Shown::NoPlayers);
    }
    let cost = l.support_draw(t) / t.ticks_per_second;
    if energy < cost {
        return turret::shown(l, false, Shown::NoEnergy);
    }
    machines::set_buffer_energy(game, pos, energy - cost);
    let b = turret::buffs(l, t);
    {
        let mut s = state();
        for p in &players {
            let e = s.buffs.entry(p.uuid).or_default();
            e.guard = e.guard.max(b.guard);
            e.power = e.power.max(b.power);
            e.haste = e.haste.max(b.haste);
            e.until = now + t.support_linger_ticks;
            if b.guard > 0.0 {
                s.guards
                    .insert(p.entity_id, turret::guard_of(&[b.guard], t));
            }
        }
    }
    let mut doing = Shown::Supporting;
    if b.heal > 0.0 {
        ts.heal_wait -= 1.0;
        let hurt: Vec<(u128, f64, f64)> = players
            .iter()
            .map(|p| (p.uuid, f64::from(p.health), 20.0))
            .collect();
        if let Some(who) = turret::most_hurt(&hurt) {
            let name = players
                .iter()
                .find(|p| p.uuid == who)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            if ts.heal_wait <= 0.0 {
                ts.heal_wait = b.heal_ticks;
                let health = players
                    .iter()
                    .find(|p| p.uuid == who)
                    .map_or(0.0, |p| p.health);
                game.native_set_player_status(who, Some(health + b.heal as f32), None, None);
            }
            doing = Shown::Healing(name);
        }
    }
    turret::shown(l, true, doing)
}

/// Each second: buffs past their time come off; power and haste follow as modifiers.
fn buffs(game: &mut Game<'_>, now: u64) {
    let players = game.native_server_players();
    let changes: Vec<(String, Option<(f64, f64)>)> = {
        let mut s = state();
        s.buffs.retain(|_, b| b.until > now);
        let alive: BTreeSet<u128> = s.buffs.keys().copied().collect();
        let mut guards = HashMap::new();
        for p in &players {
            if let Some(b) = s.buffs.get(&p.uuid)
                && b.guard > 0.0
            {
                guards.insert(p.entity_id, b.guard);
            }
        }
        s.guards = guards;
        let mut out = Vec::new();
        for p in &players {
            let want = s
                .buffs
                .get(&p.uuid)
                .map(|b| (b.power, b.haste))
                .filter(|w| w.0 > 0.0 || w.1 > 0.0);
            let had = s.applied.get(&p.uuid).copied();
            if want != had {
                match want {
                    Some(w) => s.applied.insert(p.uuid, w),
                    None => s.applied.remove(&p.uuid),
                };
                out.push((p.name.clone(), want));
            }
        }
        s.applied
            .retain(|u, _| alive.contains(u) || players.iter().any(|p| p.uuid == *u));
        out
    };
    for (name, want) in changes {
        remove_modifiers(game, &name);
        if let Some((power, haste)) = want {
            if power > 0.0 {
                game.native_run_command(&format!("attribute {name} minecraft:attack_damage modifier add {POWER_ID} {power} add_multiplied_total"));
            }
            if haste > 0.0 {
                game.native_run_command(&format!("attribute {name} minecraft:movement_speed modifier add {HASTE_ID} {haste} add_multiplied_total"));
            }
        }
    }
}

fn remove_modifiers(game: &mut Game<'_>, name: &str) {
    game.native_run_command(&format!(
        "attribute {name} minecraft:attack_damage modifier remove {POWER_ID}"
    ));
    game.native_run_command(&format!(
        "attribute {name} minecraft:movement_speed modifier remove {HASTE_ID}"
    ));
}

// ---- The anchor ----

static ANCHORS: AttachmentType<Vec<Anchor>> = AttachmentType {
    namespace: "rustcraft",
    name: "anchors",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_anchors,
        decode: decode_anchors,
    }),
    copy_on_death: false,
    sync: false,
};

/// `{anchors: [{pos: [x, y, z], owner: "<hex>", placed: L}]}`.
fn encode_anchors(list: &Vec<Anchor>) -> Result<Compound, AttachmentError> {
    let items = list
        .iter()
        .map(|a| {
            Tag::compound([
                ("pos", Tag::IntArray(a.pos.to_vec())),
                (
                    "owner",
                    Tag::String(JavaString::from(format!("{:032x}", a.owner).as_str())),
                ),
                ("placed", Tag::Long(a.placed as i64)),
            ])
        })
        .collect();
    Ok(Compound::from([(
        JavaString::from("anchors"),
        Tag::List(items),
    )]))
}

fn decode_anchors(data: &Compound) -> Result<Vec<Anchor>, AttachmentError> {
    let mut out = Vec::new();
    if let Some(Tag::List(list)) = data.get(&JavaString::from("anchors")) {
        for t in list {
            let Tag::Compound(c) = t else { continue };
            let pos = match c.get(&JavaString::from("pos")) {
                Some(Tag::IntArray(p)) if p.len() == 3 => [p[0], p[1], p[2]],
                _ => continue,
            };
            let owner = match c.get(&JavaString::from("owner")) {
                Some(Tag::String(s)) => u128::from_str_radix(&s.to_string_lossy(), 16).unwrap_or(0),
                _ => 0,
            };
            let placed = match c.get(&JavaString::from("placed")) {
                Some(Tag::Long(l)) => *l as u64,
                _ => 0,
            };
            out.push(Anchor { pos, owner, placed });
        }
    }
    Ok(out)
}

fn load_anchors(game: &mut Game<'_>, dimension: &str) {
    if state().anchors_loaded.contains(dimension) {
        return;
    }
    let host: &mut dyn Host = game;
    let list = ANCHORS.get(host, Holder::Level).unwrap_or_default();
    let mut s = state();
    s.anchors.insert(dimension.to_owned(), list);
    s.anchors_loaded.insert(dimension.to_owned());
}

fn save_anchors(game: &mut Game<'_>, dimension: &str) {
    let list = state().anchors.get(dimension).cloned().unwrap_or_default();
    let host: &mut dyn Host = game;
    if let Err(e) = ANCHORS.set(host, Holder::Level, list) {
        log::warn!("rustcraft: the anchors not saved: {e:?}");
    }
}

fn add_anchor(game: &mut Game<'_>, dimension: &str, a: Anchor) {
    load_anchors(game, dimension);
    {
        let mut s = state();
        let list = s.anchors.entry(dimension.to_owned()).or_default();
        list.retain(|b| b.pos != a.pos);
        list.push(a);
    }
    save_anchors(game, dimension);
}

/// `machines::tick` asks what an anchor draws: nothing unless it is its owner's working one.
pub(super) fn anchor_request(game: &Game<'_>, pos: Pos, t: &Tuning) -> f64 {
    let dim = game.native_dimension().to_owned();
    let s = state();
    let st = s.anchor_states.get(&(dim, pos)).copied();
    drop(s);
    match st {
        Some(AnchorState::Holding | AnchorState::NoEnergy) => {
            let slots = modules_at(game, pos);
            anchor::draw(anchor::chunks(pos, &slots, t).len(), &slots, t)
        }
        _ => 0.0,
    }
}

pub(super) fn anchor_supplied(pos: Pos, share: f64) {
    state().shares.insert(pos, share);
}

/// Each second: the anchors of the level, their states and the chunks they hold.
fn anchors(game: &mut Game<'_>, dimension: &str, level: &LevelNet, t: &Tuning) {
    load_anchors(game, dimension);
    let broken: Vec<Pos> = {
        let mut s = state();
        let (here, later): (Vec<_>, Vec<_>) = std::mem::take(&mut s.broken)
            .into_iter()
            .partition(|b| b.0 == dimension);
        s.broken = later;
        here.into_iter().map(|b| b.1).collect()
    };
    // Anchors loaded but not in the record (placed before it existed, or by a command).
    let mut changed = !broken.is_empty();
    let found: Vec<Pos> = level
        .machines
        .iter()
        .filter(|(_, m)| **m == Machine::Anchor)
        .map(|(p, _)| *p)
        .collect();
    let mut list = state().anchors.get(dimension).cloned().unwrap_or_default();
    list.retain(|a| !broken.contains(&a.pos));
    // A loaded chunk without the anchor its record names: it was taken away.
    list.retain(|a| {
        let gone = game
            .world
            .chunks
            .contains_key(&(a.pos[0] >> 4, a.pos[2] >> 4))
            && game.native_block(a.pos) != content::ANCHOR;
        changed |= gone;
        !gone
    });
    for p in found {
        if !list.iter().any(|a| a.pos == p)
            && let Some(owner) = owner_at(game, p)
        {
            let placed = game
                .native_mod_block_entity(p)
                .and_then(|e| match e.nbt.get(&JavaString::from(PLACED_KEY)) {
                    Some(Tag::Long(l)) => Some(*l as u64),
                    _ => None,
                })
                .unwrap_or(0);
            list.push(Anchor {
                pos: p,
                owner,
                placed,
            });
            changed = true;
        }
    }
    state().anchors.insert(dimension.to_owned(), list.clone());
    if changed {
        save_anchors(game, dimension);
    }
    let mut online: BTreeSet<u128> = game
        .native_server_players()
        .into_iter()
        .map(|p| p.uuid)
        .collect();
    online.extend(pretended());
    let working = anchor::working(&list);
    let mut want: BTreeSet<[i32; 2]> = BTreeSet::new();
    for a in &list {
        let loaded = game
            .world
            .chunks
            .contains_key(&(a.pos[0] >> 4, a.pos[2] >> 4));
        let share = state().shares.get(&a.pos).copied();
        let slots = if loaded {
            modules_at(game, a.pos)
        } else {
            Vec::new()
        };
        // Not loaded yet: it is held so it can load and draw; its energy decides at once after.
        let st = anchor::state(
            a.owner != 0,
            working.contains(&a.pos),
            online.contains(&a.owner),
            share.unwrap_or(1.0),
        );
        state()
            .anchor_states
            .insert((dimension.to_owned(), a.pos), st);
        if st == AnchorState::Holding || (st == AnchorState::NoEnergy && share.is_none()) {
            want.extend(anchor::chunks(a.pos, &slots, t));
        }
    }
    state().shares.clear();
    let held = state().held.remove(dimension).unwrap_or_default();
    for c in held.difference(&want) {
        game.native_force_chunk(TICKET, *c, false);
    }
    for c in want.difference(&held) {
        if !game.native_force_chunk(TICKET, *c, true) {
            log::warn!("rustcraft: anchor chunk {c:?} not held");
        }
    }
    state().held.insert(dimension.to_owned(), want);
}

/// The chunks the anchors of `dimension` hold now, for the self-test.
pub(super) fn held_chunks(dimension: &str) -> BTreeSet<[i32; 2]> {
    state().held.get(dimension).cloned().unwrap_or_default()
}

pub(super) fn anchor_state(dimension: &str, pos: Pos) -> Option<AnchorState> {
    state()
        .anchor_states
        .get(&(dimension.to_owned(), pos))
        .copied()
}

pub(super) fn turret_shown(dimension: &str, pos: Pos) -> Option<Shown> {
    state()
        .turrets
        .get(&(dimension.to_owned(), pos))
        .and_then(|t| t.shown.clone())
}

/// The self-test's chunks come back to it: a test anchor's record goes.
pub(super) fn forget_anchor(game: &mut Game<'_>, dimension: &str, pos: Pos) {
    {
        let mut s = state();
        if let Some(list) = s.anchors.get_mut(dimension) {
            list.retain(|a| a.pos != pos);
        }
    }
    save_anchors(game, dimension);
}

// ---- What they show ----

fn shown_text(s: &Shown) -> Text {
    match s {
        Shown::Shooting(kind) => line(
            "rustcraft.turret.shooting",
            "Shooting: %s",
            &[entity_name(kind)],
        ),
        Shown::NoTargets => line(
            "rustcraft.turret.no_targets",
            "No hostile mobs in range",
            &[],
        ),
        Shown::Healing(name) => line(
            "rustcraft.turret.healing",
            "Healing: %s",
            &[Text::literal(name.clone())],
        ),
        Shown::Supporting => line(
            "rustcraft.turret.supporting",
            "Supporting players in range",
            &[],
        ),
        Shown::NoPlayers => line("rustcraft.turret.no_players", "No players in range", &[]),
        Shown::NoEnergy => line("rustcraft.machine.no_energy", "No energy", &[]),
        Shown::IdleEffects => line(
            "rustcraft.turret.idle_effects",
            "Effect modules do not work in support mode",
            &[],
        ),
    }
}

fn entity_name(kind: &str) -> Text {
    Text::translate(&format!("entity.{}", kind.replace(':', ".")), &[])
}

fn anchor_text(st: AnchorState, chunks: usize) -> Text {
    match st {
        AnchorState::Holding => line(
            "rustcraft.anchor.holding",
            "Holding %s chunks",
            &[Text::literal(chunks.to_string())],
        ),
        AnchorState::NoEnergy => line("rustcraft.machine.no_energy", "No energy", &[]),
        AnchorState::OwnerAway => line(
            "rustcraft.anchor.owner_away",
            "The owner left the server",
            &[],
        ),
        AnchorState::Replaced => line(
            "rustcraft.anchor.replaced",
            "Off: its owner placed a newer anchor",
            &[],
        ),
        AnchorState::NoOwner => line("rustcraft.anchor.no_owner", "No owner: place it again", &[]),
    }
}

/// The looked-at line of a slice-7 block.
pub(super) fn diagnosis(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    block: &str,
    t: &Tuning,
) -> Option<Text> {
    match Machine::of(block)? {
        Machine::Turret => Some(turret_shown(dimension, pos).map_or_else(
            || line("rustcraft.turret.idle", "Turret", &[]),
            |s| shown_text(&s),
        )),
        Machine::Anchor => {
            let slots = modules_at(game, pos);
            let n = anchor::chunks(pos, &slots, t).len();
            Some(anchor_state(dimension, pos).map_or_else(
                || line("rustcraft.anchor.idle", "Base anchor", &[]),
                |s| anchor_text(s, n),
            ))
        }
        m @ (Machine::DockUnload | Machine::DockCharge) => {
            Some(workers::dock_text(game, dimension, pos, m))
        }
        _ => None,
    }
}

// ---- Windows ----

/// The rows of the one server menu left.
const SERVER_ROWS: u8 = 1;

/// The one block left with a server menu: the solar panel, whose template (glass, for the sky's
/// light under it) opens no menu of the block's own. The others' windows are sheets
/// (rustcraft_sheets) since slice 9.
fn windows(game: &mut Game<'_>, dimension: &str, level: &LevelNet, t: &Tuning) {
    let (opening, clicks) = {
        let mut s = state();
        let (here, later): (Vec<_>, Vec<_>) = std::mem::take(&mut s.opening)
            .into_iter()
            .partition(|o| o.1 == dimension);
        s.opening = later;
        let mine: Vec<u128> = s
            .sessions
            .iter()
            .filter(|(_, x)| x.dimension == dimension)
            .map(|(u, _)| *u)
            .collect();
        let (c, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.clicks)
            .into_iter()
            .partition(|c| mine.contains(&c.0));
        s.clicks = rest;
        (here, c)
    };
    for (uuid, _, pos) in opening {
        let block = game.native_block(pos);
        let items = cells(game, level, pos, &block, t);
        let title = Text::translate(&format!("block.{}", block.replace(':', ".")), &[]);
        if let Some(window) = game.native_open_menu(uuid, SERVER_ROWS, title, items) {
            state().sessions.insert(
                uuid,
                Session {
                    dimension: dimension.to_owned(),
                    pos,
                    window,
                },
            );
        }
    }
    // The panel's window has nothing to press: a click only shows it again.
    for (uuid, window, _) in clicks {
        let pos = match state().sessions.get(&uuid) {
            Some(x) if x.window == window => x.pos,
            _ => continue,
        };
        let block = game.native_block(pos);
        if Machine::of(&block).is_none() {
            game.native_close_menu(uuid);
            continue;
        }
        let items = cells(game, level, pos, &block, t);
        game.native_set_menu_items(uuid, items);
    }
    if state().ticks.is_multiple_of(20) {
        let open: Vec<(u128, Pos)> = state()
            .sessions
            .iter()
            .filter(|(_, x)| x.dimension == dimension)
            .map(|(u, x)| (*u, x.pos))
            .collect();
        for (uuid, pos) in open {
            let block = game.native_block(pos);
            let items = cells(game, level, pos, &block, t);
            game.native_set_menu_items(uuid, items);
        }
    }
}

const CELLS: usize = 9;

/// The modules' holder of a block with module sockets.
fn holder_of(block: &str) -> Option<ModuleHolder> {
    SheetKind::of(block).and_then(SheetKind::holder)
}

fn cells(
    game: &Game<'_>,
    level: &LevelNet,
    pos: Pos,
    block: &str,
    t: &Tuning,
) -> Vec<Option<ItemStack>> {
    let mut out: Vec<Option<ItemStack>> = vec![None; CELLS];
    if let Some(m) = Machine::of(block).filter(|m| m.kind() == Kind::Power) {
        power::cells(game, level, pos, m, t, &mut out);
    }
    out
}

// ---- Sheets: the slice-7 and slice-8 blocks' drawn windows (rustcraft_sheets) ----

/// A text as the sheet carries it: the game's text component, which each client reads in its own
/// language.
pub(super) fn v(t: &Text) -> Value {
    t.component()
}

/// The sockets' modules of the block at `pos`, as item ids, one a socket.
pub(super) fn module_items(game: &Game<'_>, pos: Pos) -> Vec<Option<String>> {
    let mut items = game
        .native_mod_block_entity(pos)
        .map(|e| read_modules(&e.nbt))
        .unwrap_or_default();
    items.resize(MODULE_SLOTS, None);
    items
}

/// What the window of a slice-7 or slice-8 block at `pos` shows: what its server menu showed,
/// part by part.
pub(super) fn sheet(
    game: &Game<'_>,
    dimension: &str,
    level: &LevelNet,
    pos: Pos,
    block: &str,
    t: &Tuning,
) -> Option<Sheet> {
    let kind = SheetKind::of(block)?;
    let mut s = Sheet {
        pos,
        ..Sheet::default()
    };
    if kind.holder().is_some() {
        s.modules = module_items(game, pos);
    }
    match kind {
        SheetKind::Turret => turret_sheet(game, dimension, pos, t, &mut s),
        SheetKind::Anchor => anchor_sheet(game, dimension, pos, t, &mut s),
        SheetKind::DockUnload | SheetKind::DockCharge => {
            dock_sheet(game, dimension, pos, kind, t, &mut s)
        }
        SheetKind::Battery | SheetKind::Windmill | SheetKind::LavaGenerator => {
            power::sheet(game, level, pos, kind, t, &mut s);
        }
        SheetKind::Miner | SheetKind::Placer | SheetKind::Breaker => {
            mining::sheet(game, dimension, pos, kind, t, &mut s);
        }
    }
    Some(s)
}

fn status(severity: Severity, text: &Text) -> Option<SheetStatus> {
    Some(SheetStatus {
        severity: severity_code(severity),
        text: v(text),
    })
}

fn turret_sheet(game: &Game<'_>, dimension: &str, pos: Pos, t: &Tuning, s: &mut Sheet) {
    let slots = modules_at(game, pos);
    let l = Loadout::of(&slots);
    let support = l.role() == Role::Support;
    s.support = Some(support);
    s.role = vec![
        v(&if support {
            line("rustcraft.turret.mode_support", "Mode: support", &[])
        } else {
            line("rustcraft.turret.mode_attack", "Mode: attack", &[])
        }),
        v(&line(
            "rustcraft.turret.targets",
            "Hostile mobs only: never players, pets or peaceful mobs",
            &[],
        )),
    ];
    let range = Text::literal(format!("{:.0}", l.range(t)));
    s.rows.push(v(&line(
        "rustcraft.turret.range",
        "Range: %s blocks",
        &[range],
    )));
    if support {
        let b = turret::buffs(&l, t);
        let pct = |f: f64| Text::literal(format!("{:.0}%", f * 100.0));
        let mut parts = Vec::new();
        if b.heal > 0.0 {
            parts.push(line(
                "rustcraft.turret.buff.heal",
                "heal +%s HP every %s s",
                &[
                    Text::literal(format!("{:.0}", b.heal)),
                    Text::literal(format!("{:.0}", b.heal_ticks / t.ticks_per_second)),
                ],
            ));
        }
        if b.guard > 0.0 {
            parts.push(line(
                "rustcraft.turret.buff.guard",
                "protection −%s",
                &[pct(b.guard)],
            ));
        }
        if b.power > 0.0 {
            parts.push(line(
                "rustcraft.turret.buff.power",
                "strength +%s",
                &[pct(b.power)],
            ));
        }
        if b.haste > 0.0 {
            parts.push(line(
                "rustcraft.turret.buff.haste",
                "speed +%s",
                &[pct(b.haste)],
            ));
        }
        if !parts.is_empty() {
            s.rows.push(v(&joined(parts)));
        }
        s.rows
            .push(v(&Text::literal(format!("{:.0} E/s", l.support_draw(t)))));
    } else {
        s.rows.push(v(&line(
            "rustcraft.turret.shot",
            "Shot: %s E",
            &[Text::literal(format!("{:.0}", l.shot_energy(t)))],
        )));
        let e = turret::effects(&l, t, &Resist::default(), false);
        if !e.is_empty() {
            s.rows.push(v(&joined(
                e.iter().map(|g| effect_name(g.effect)).collect(),
            )));
        }
    }
    let aimed = state()
        .turrets
        .get(&(dimension.to_owned(), pos))
        .and_then(|x| Some((x.target.clone()?, x.target_id)));
    if let Some((target, target_id)) = aimed {
        let mut l2 = line(
            "rustcraft.turret.target",
            "Target: %s",
            &[entity_name(&target)],
        );
        let refused = target_id.map_or_else(
            || undead(game, &target),
            |id| poison_refused(game, id, &target),
        );
        if refused && l.poison > 0 {
            l2.push(Text::literal(" · "));
            l2.push(line(
                "rustcraft.turret.undead",
                "undead: poison does not work",
                &[],
            ));
        }
        s.rows.push(v(&l2));
    }
    let e = machines::buffer_energy(game, pos);
    s.bars.push(SheetBar {
        label: v(&line("rustcraft.window.energy", "Energy", &[])),
        value: v(&Text::literal(format!("{e:.0} / {:.0} E", t.turret_buffer))),
        share: e / t.turret_buffer.max(1.0),
        energy: true,
    });
    let shown = turret_shown(dimension, pos);
    let severity = match shown {
        Some(Shown::NoEnergy) => Severity::Bad,
        Some(Shown::Shooting(_) | Shown::Healing(_) | Shown::Supporting) => Severity::Ok,
        _ => Severity::Warn,
    };
    let text = shown.map_or_else(
        || line("rustcraft.turret.idle", "Turret", &[]),
        |x| shown_text(&x),
    );
    s.status = status(severity, &text);
}

fn anchor_sheet(game: &Game<'_>, dimension: &str, pos: Pos, t: &Tuning, s: &mut Sheet) {
    let slots = modules_at(game, pos);
    let name = owner_at(game, pos)
        .and_then(|o| {
            game.native_server_players()
                .into_iter()
                .find(|p| p.uuid == o)
                .map(|p| p.name)
        })
        .map_or_else(
            || {
                line(
                    "rustcraft.anchor.owner_off",
                    "the owner is off the server",
                    &[],
                )
            },
            Text::literal,
        );
    s.rows
        .push(v(&line("rustcraft.anchor.owner", "Owner: %s", &[name])));
    let chunks = anchor::chunks(pos, &slots, t);
    let side = (chunks.len() as f64).sqrt().round() as usize;
    s.rows.push(v(&line(
        "rustcraft.anchor.area",
        "Holds %s chunks",
        &[Text::literal(format!("{side}×{side}"))],
    )));
    s.hints = vec![
        Vec::new(),
        vec![v(&line(
            "rustcraft.anchor.area_hint",
            "A radius module: 3×3, two: 5×5",
            &[],
        ))],
    ];
    s.rows.push(v(&Text::literal(format!(
        "{:.0} E/s",
        anchor::draw(chunks.len(), &slots, t)
    ))));
    let st = anchor_state(dimension, pos);
    let severity = match st {
        Some(AnchorState::Holding) => Severity::Ok,
        Some(AnchorState::NoEnergy | AnchorState::NoOwner) => Severity::Bad,
        _ => Severity::Warn,
    };
    let text = st.map_or_else(
        || line("rustcraft.anchor.idle", "Base anchor", &[]),
        |x| anchor_text(x, chunks.len()),
    );
    s.status = status(severity, &text);
}

fn dock_sheet(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    kind: SheetKind,
    t: &Tuning,
    s: &mut Sheet,
) {
    let machine = if kind == SheetKind::DockCharge {
        Machine::DockCharge
    } else {
        Machine::DockUnload
    };
    let text = workers::dock_text(game, dimension, pos, machine);
    if machine == Machine::DockCharge {
        let e = machines::buffer_energy(game, pos);
        s.bars.push(SheetBar {
            label: v(&line("rustcraft.window.energy", "Energy", &[])),
            value: v(&Text::literal(format!("{e:.0} / {:.0} E", t.dock_buffer))),
            share: e / t.dock_buffer.max(1.0),
            energy: true,
        });
        s.rows.push(v(&line(
            "rustcraft.dock.draw",
            "Draws up to %s E/s",
            &[Text::literal(format!("{:.0}", t.dock_draw))],
        )));
        let severity = if e > 0.0 { Severity::Ok } else { Severity::Bad };
        s.status = status(severity, &text);
    } else {
        let full = game.native_mod_block_entity(pos).is_some_and(|e| {
            e.items.len() >= content::DOCK_SLOTS
                && e.items
                    .iter()
                    .all(|x| x.as_ref().is_some_and(|x| x.count > 0))
        });
        let severity = if full { Severity::Warn } else { Severity::Ok };
        s.status = status(severity, &text);
    }
}

/// A press on a sheet window (`native_menu_button_v2`; rustcraft_rules::machines::sheet::Press):
/// a socket takes or gives a module as a slot does, a work mode is set, an ore toggled. The block
/// entity is out of the world during the call, so everything is written to `entity`.
pub(super) fn on_sheet_button(
    entity: &mut ModBlockEntity,
    id: i32,
    carried: &mut Option<ItemStack>,
) {
    let Some(kind) = SheetKind::of(&entity.id) else {
        return;
    };
    let Some(press) = Press::of(id) else {
        return;
    };
    match press {
        Press::Socket(k) => {
            let Some(holder) = kind.holder() else {
                return;
            };
            let mut slots = rustcraft_rules::modules::read(&read_modules(&entity.nbt));
            let cursor = carried.as_ref().map(|c| Cursor {
                item: c.item.to_string(),
                count: c.count,
            });
            let Some(after) = press_socket(&mut slots, k, holder, cursor.as_ref()) else {
                return;
            };
            *carried = after.map(|c| {
                let mut stack = carried
                    .clone()
                    .filter(|x| *x.item == *c.item)
                    .unwrap_or_else(|| ItemStack::new(c.item.as_str(), c.count));
                stack.count = c.count;
                stack
            });
            write_sockets(entity, kind, &slots);
        }
        Press::Mode(mode) if kind.shape().mode => machines::write_work(&mut entity.nbt, mode),
        Press::Toggle(i) if kind == SheetKind::Miner => mining::toggle_ore(&mut entity.nbt, i),
        _ => {}
    }
}

fn write_sockets(entity: &mut ModBlockEntity, kind: SheetKind, slots: &[Option<Module>]) {
    let items: Vec<Option<String>> = slots
        .iter()
        .map(|m| m.map(|m| m.item().to_owned()))
        .collect();
    write_modules(&mut entity.nbt, &items);
    if kind == SheetKind::Turret {
        write_role(&mut entity.nbt, Loadout::of(slots).role());
    }
}

/// `native_menu_quick_move` on a sheet window: a module shift-clicked in the inventory goes into
/// the first free socket the block's modules take, one of the stack; anything else goes the
/// game's way (the placer's slots, the docks' and the miner's slots to the inventory).
pub(super) fn on_sheet_quick_move(
    entity: &mut ModBlockEntity,
    container_slots: usize,
    slot: usize,
    stack: &mut ItemStack,
    handled: &mut bool,
) {
    if *handled || slot < container_slots {
        return;
    }
    let Some(kind) = SheetKind::of(&entity.id) else {
        return;
    };
    let Some(holder) = kind.holder() else {
        return;
    };
    let mut slots = rustcraft_rules::modules::read(&read_modules(&entity.nbt));
    if put_free(&mut slots, holder, &stack.item) {
        stack.count -= 1;
        write_sockets(entity, kind, &slots);
        *handled = true;
    }
}

/// The server folder's SoulsCraft data, for the self-test: whether it was found.
pub(super) fn souls_found(game: &Game<'_>, t: &Tuning) -> bool {
    souls(game, t).is_some()
}

/// `native_state_for_placement`: a slice-7 block faces the player placing it, as a furnace does.
pub(super) fn state_for_placement(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    context: &horizon_world::level::PlacementContext,
    answer: &mut Option<Option<horizon_world::registry::BlockStateId>>,
) {
    if answer.is_some() {
        return;
    }
    let Some(block) = level.native_state_block(state) else {
        return;
    };
    if !Machine::of(block).is_some_and(|m| !m.has_menu()) {
        return;
    }
    // The player looks along yaw (0 south, 90 west); the front looks back at them.
    let quarter = (((f64::from(context.yaw) / 90.0).round() as i64).rem_euclid(4)) as usize;
    let looks = ["south", "west", "north", "east"][quarter];
    let front = match looks {
        "south" => "north",
        "west" => "east",
        "north" => "south",
        _ => "west",
    };
    if let Some(placed) = level.native_state_with_property(state, content::FACING, front) {
        *answer = Some(Some(placed));
    }
}

// ---- The console's test kit (`rustcraft_testkit`, for other mods' self-tests) ----

const TESTKIT: &str = "rustcraft_testkit";
const OVERWORLD: &str = "minecraft:overworld";

/// `rustcraft_testkit turret <x> <y> <z> [modules=bleed,poison,...] [owner=<uuid|name>]`: a turret
/// in the Overworld with those modules and owner and a buffer kept full, so it shoots at once with
/// no energy net. Placed on the Overworld's next tick; the answer says what will be placed.
pub(super) fn on_testkit_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let Some(args) = command
        .strip_prefix(TESTKIT)
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
    else {
        return;
    };
    *ran = true;
    let answer = match testkit_turret(game, args) {
        Ok(text) => text,
        Err(why) => format!(
            "{TESTKIT}: {why}; usage: {TESTKIT} turret <x> <y> <z> [modules=bleed,poison,slow,heal,guard,power,haste,speed,economy,radius] [owner=<uuid|name>]"
        ),
    };
    log::info!("rustcraft: {answer}");
    game.native_feedback(Text::literal(answer));
}

fn testkit_turret(game: &Game<'_>, args: &str) -> Result<String, String> {
    let mut words = args.split_whitespace();
    if words.next() != Some("turret") {
        return Err("only `turret` is known".into());
    }
    let mut pos = [0; 3];
    for c in &mut pos {
        *c = words
            .next()
            .and_then(|w| w.parse().ok())
            .ok_or("three whole coordinates")?;
    }
    let mut modules: Vec<Option<Module>> = Vec::new();
    let mut owner = None;
    for w in words {
        if let Some(list) = w.strip_prefix("modules=") {
            for name in list.split(',').filter(|n| !n.is_empty()) {
                let m = Module::of(&format!("rustcraft:module_{name}"))
                    .filter(|m| ModuleHolder::Turret.takes(*m))
                    .ok_or_else(|| format!("no turret module {name}"))?;
                modules.push(Some(m));
            }
        } else if let Some(who) = w.strip_prefix("owner=") {
            let hex = who.replace('-', "");
            owner = Some(match u128::from_str_radix(&hex, 16) {
                Ok(u) if hex.len() == 32 => u,
                _ => game
                    .native_server_players()
                    .into_iter()
                    .find(|p| p.name.eq_ignore_ascii_case(who))
                    .map(|p| p.uuid)
                    .ok_or_else(|| format!("no player {who} on the server"))?,
            });
        } else {
            return Err(format!("what is {w}?"));
        }
    }
    if modules.len() > MODULE_SLOTS {
        return Err(format!("at most {MODULE_SLOTS} modules"));
    }
    modules.resize(MODULE_SLOTS, None);
    let role = match Loadout::of(&modules).role() {
        Role::Attack => "attack",
        Role::Support => "support",
    };
    let text = format!(
        "{TESTKIT}: a {role} turret at {} {} {} on the next tick",
        pos[0], pos[1], pos[2]
    );
    state().testkit_due.push((pos, modules, owner));
    Ok(text)
}

/// The test kit's turrets due, placed on the Overworld's tick.
fn testkit(game: &mut Game<'_>, dimension: &str) {
    if dimension != OVERWORLD {
        return;
    }
    let due = std::mem::take(&mut state().testkit_due);
    for (pos, modules, owner) in due {
        let Some(st) = game.native_default_state(content::TURRET) else {
            continue;
        };
        game.native_set_block(dimension, pos, st, 3);
        if game.native_block(pos) != content::TURRET {
            log::warn!("rustcraft: {TESTKIT}: no turret at {pos:?}");
            continue;
        }
        if let Some(o) = owner {
            set_owner(game, pos, o, now_ms());
        }
        set_modules(game, pos, &modules);
        crate::game::rustcraft_server::mark_level_stale(dimension);
        machines::set_buffer_energy(
            game,
            pos,
            crate::game::rustcraft_server::tuning().turret_buffer,
        );
        state().testkit.insert((dimension.to_owned(), pos));
    }
    // A test turret taken away stops being topped up.
    let gone: Vec<(String, Pos)> = state()
        .testkit
        .iter()
        .filter(|(d, p)| {
            d == dimension
                && game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4))
                && game.native_block(*p) != content::TURRET
        })
        .cloned()
        .collect();
    for g in gone {
        state().testkit.remove(&g);
    }
}

// ---- For the self-test ----

/// Owners the anchors take as on the server: the self-test's pretend players.
static PRETEND: LazyLock<Mutex<BTreeSet<u128>>> = LazyLock::new(Mutex::default);

pub(super) fn pretend_online(owner: u128, online: bool) {
    let mut p = PRETEND.lock().unwrap_or_else(PoisonError::into_inner);
    if online {
        p.insert(owner);
    } else {
        p.remove(&owner);
    }
}

fn pretended() -> BTreeSet<u128> {
    PRETEND
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Whether mob `id` bleeds: SoulsCraft's effect on it, or the turret's own timer.
pub(super) fn bleeding(game: &Game<'_>, id: u32) -> bool {
    state().bleeds.contains_key(&id)
        || game
            .mobs
            .iter()
            .find(|m| m.id == id)
            .is_some_and(|m| m.has_effect(OWN_BLEED))
}

/// The self-test's plot cleared: its anchors and turrets are forgotten, its chunks let go.
pub(super) fn forget_area(game: &mut Game<'_>, dimension: &str, min: Pos, max: Pos) {
    let inside = |p: Pos| (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
    let gone: Vec<Pos> = state()
        .anchors
        .get(dimension)
        .map(|l| l.iter().filter(|a| inside(a.pos)).map(|a| a.pos).collect())
        .unwrap_or_default();
    for p in gone {
        forget_anchor(game, dimension, p);
    }
    mining::forget_area(dimension, min, max);
    let mut s = state();
    s.turrets.retain(|(d, p), _| d != dimension || !inside(*p));
    s.anchor_states
        .retain(|(d, p), _| d != dimension || !inside(*p));
}
