// Client half of the traversal prototype: the /proto command, method a's push each tick, the
// rider's and the observers' journals, and the rope and harpoon lines drawn in the world.
use crate::game::{
    Game, trv_journal as journal, trv_measure as measure, trv_plans as plans, trv_rules as rules,
    trv_wire as wire,
};
use horizon_client::{mod_api::ClientboundPlayPacket, world_primitives::WorldPrimitives};
use journal::Journal;
use measure::{ClientStats, Watcher};
use plans::{Plan, Rope};
use rules::{Command, Hit, Kind, Method, V3};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Instant,
};
use wire::{ToClient, ToServer};

const ROPE_COLOUR: u32 = 0xFF_C8_96_50;
const ROPE_WIDTH: f32 = 4.0;
const POST_COLOUR: u32 = 0xFF_6E_4A_28;
const POST_WIDTH: f32 = 6.0;
const HOOK_COLOUR: u32 = 0xFF_D0_D0_D8;
const HOOK_WIDTH: f32 = 3.0;
/// The harpoon line leaves the puller this high over the feet.
const HAND_HEIGHT: f64 = 1.2;
const HOOK_MARK: f64 = 0.12;
/// A step this long with no interpolation behind it is a snap (a teleport the eye sees).
const SNAP_STEP: f64 = 0.05;
/// Ticks past the plan the client waits for the server's summary.
const SUMMARY_WAIT_TICKS: u32 = 200;

/// Teleport packets (ClientboundPlayerPosition) heard, ever: the attempts count from a start.
static POSITION_PACKETS: AtomicU32 = AtomicU32::new(0);

/// This player's own attempt, any method.
struct Own {
    attempt: u64,
    planned: u32,
    journal: Option<Journal>,
    began: Instant,
    packets_at_start: u32,
    tick: u32,
    last: Option<V3>,
    watcher: Watcher,
    snaps: u32,
    follow: Option<Follow>,
    stats: ClientStats,
}

/// Method a: the plan this client steers its player along.
struct Follow {
    plan: Plan,
    tick: u32,
    started_sent: bool,
    done_sent: bool,
}

/// Another player's attempt, as this screen shows the rider.
struct Watch {
    entity: i32,
    label: String,
    journal: Option<Journal>,
    began: Instant,
    watcher: Watcher,
    tick: u32,
    missing: u32,
}

struct HookLine {
    mine: bool,
    point: V3,
}

#[derive(Default)]
struct Client {
    inbox: Vec<Result<ToClient, String>>,
    ticks: u32,
    /// A request sent and not answered yet: since which tick, and whether it got as far as
    /// the server's queue.
    waiting: Option<(u32, bool)>,
    own: Option<Own>,
    ropes: BTreeMap<u64, Rope>,
    hooks: BTreeMap<i32, HookLine>,
    watches: BTreeMap<u64, Watch>,
}

static CLIENT: LazyLock<Mutex<Client>> = LazyLock::new(Mutex::default);

fn client() -> MutexGuard<'static, Client> {
    CLIENT.lock().unwrap_or_else(PoisonError::into_inner)
}

fn folder() -> Option<PathBuf> {
    Game::native_config_dir()
        .ok()
        .map(|dir| dir.join(journal::FOLDER))
}

/// A chat line, and the same line in the client's event log.
fn say(game: &mut Game<'_>, line: &str) {
    game.native_chat(line);
    if let Some(dir) = folder()
        && let Err(e) = journal::event(&dir, line)
    {
        log::warn!("rc_traversal: event log not written: {e}");
    }
}

fn send(game: &mut Game<'_>, message: &ToServer) -> bool {
    match game.native_net_send_to_server(wire::TO_SERVER, &wire::encode(message)) {
        Ok(()) => true,
        Err(e) => {
            say(game, &format!("Запрос не отправлен: {e}"));
            false
        }
    }
}

fn vec3(v: horizon_core::geometry::Vec3) -> V3 {
    [v.x, v.y, v.z]
}

/// The local player's feet and velocity.
fn body(game: &Game<'_>) -> Option<(V3, V3)> {
    game.player
        .as_ref()
        .map(|p| (vec3(p.position), vec3(p.velocity)))
}

fn open(
    kind: Kind,
    method: Method,
    attempt: u64,
    role: &str,
    header: &str,
) -> Result<Journal, String> {
    let dir = folder().ok_or("нет папки config")?;
    let name = journal::file_name(journal::unix_seconds(), attempt, kind, method, role);
    Journal::create(&dir, &name, header)
}

// ---- Hooks ----

/// `command`: `/proto ...` is this prototype's; any other command is left alone.
pub fn on_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let Some(parsed) = rules::parse_command(command) else {
        return;
    };
    *ran = true;
    match parsed {
        Err(e) => say(game, &e),
        Ok(Command::Help) => {
            for line in rules::HELP {
                game.native_chat(line);
            }
        }
        Ok(Command::Stop) => {
            send(game, &ToServer::Stop);
        }
        Ok(Command::Clear) => {
            send(game, &ToServer::Clear);
        }
        Ok(Command::Start(kind, method)) => start(game, kind, method),
    }
}

fn start(game: &mut Game<'_>, kind: Kind, method: Method) {
    if client().own.is_some() {
        say(game, "Попытка уже идёт: /proto stop");
        return;
    }
    let hit = match kind.reach() {
        None => None,
        Some(reach) => match game.native_raycast(reach as f32) {
            Some(h) => Some(Hit {
                block: [h.block_x, h.block_y, h.block_z],
                point: [h.x, h.y, h.z],
            }),
            None => {
                say(
                    game,
                    &format!("Цель не найдена: под прицелом нет блока ближе {reach:.0} блоков"),
                );
                return;
            }
        },
    };
    if send(game, &ToServer::Start { kind, method, hit }) {
        let mut state = client();
        let now = state.ticks;
        state.waiting = Some((now, false));
    }
}

/// `net_from_server`: kept for the next tick, which has the game.
pub fn on_server_message(channel: &str, data: &[u8]) {
    if channel == wire::TO_CLIENT {
        client().inbox.push(wire::decode::<ToClient>(data));
    }
}

/// Set by a join (ClientboundLoginPacket); the next tick drops what the last connection left.
static JOINED: AtomicBool = AtomicBool::new(false);

/// `client_packet`: every teleport the server sends this player, and every join.
pub fn on_packet(packet: &ClientboundPlayPacket) {
    match packet {
        ClientboundPlayPacket::PlayerPosition(_) => {
            POSITION_PACKETS.fetch_add(1, Ordering::Relaxed);
        }
        ClientboundPlayPacket::Login(_) => JOINED.store(true, Ordering::Relaxed),
        _ => {}
    }
}

/// After a kick or a lost connection the server has forgotten this player's attempt, so the
/// client closes its own and drops ropes, hooks and watches; otherwise the next command is
/// refused until the reply timeout (field test 2026-11-11).
fn forget_last_connection(game: &mut Game<'_>, state: &mut Client) {
    state.waiting = None;
    state.ropes.clear();
    state.hooks.clear();
    for (attempt, watch) in std::mem::take(&mut state.watches) {
        close_watch(game, attempt, watch);
    }
    if let Some(own) = state.own.take() {
        close_own(game, own, Some("соединение прервалось, итога от сервера нет"));
    }
}

/// `tick`, on the window's game only.
pub fn on_tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let mut state = client();
    state.ticks += 1;
    if JOINED.swap(false, Ordering::Relaxed) {
        forget_last_connection(game, &mut state);
    }
    for message in std::mem::take(&mut state.inbox) {
        match message {
            Ok(message) => handle(game, &mut state, message),
            Err(e) => say(
                game,
                &format!("Сообщение сервера не прочитано ({e}): версии прототипа разные?"),
            ),
        }
    }
    own_tick(game, &mut state);
    watch_tick(game, &mut state);
    if let Some((since, queued)) = state.waiting
        && state.ticks - since > rules::REPLY_TIMEOUT_TICKS
    {
        state.waiting = None;
        drop(state);
        say(
            game,
            if queued {
                "Сервер принял запрос, но не начал попытку за 3 с: тик уровня (хук level_tick_post) на сервере не работает"
            } else {
                "Сервер не ответил за 3 с: на сервере нет прототипа или хук net_from_client молчит"
            },
        );
    }
}

fn handle(game: &mut Game<'_>, state: &mut Client, message: ToClient) {
    match message {
        ToClient::Queued => {
            if let Some((_, queued)) = state.waiting.as_mut() {
                *queued = true;
            }
        }
        ToClient::Refused { reason } => {
            state.waiting = None;
            say(game, &format!("Сервер отказал: {reason}"));
        }
        ToClient::Accepted {
            attempt,
            kind,
            method,
            planned,
            length,
        } => {
            state.waiting = None;
            let journal = match open(kind, method, attempt, "rider", journal::RIDER_HEADER) {
                Ok(journal) => Some(journal),
                Err(e) => {
                    say(game, &format!("Журнал клиента не создан: {e}"));
                    None
                }
            };
            state.own = Some(Own {
                attempt,
                planned,
                journal,
                began: Instant::now(),
                packets_at_start: POSITION_PACKETS.load(Ordering::Relaxed),
                tick: 0,
                last: None,
                watcher: Watcher::default(),
                snaps: 0,
                follow: None,
                stats: ClientStats::default(),
            });
            game.native_chat(&format!(
                "Попытка #{attempt}: {} {}, {length:.1} бл, план {planned} тиков",
                kind.label(),
                method.label()
            ));
        }
        ToClient::Follow { attempt, plan } => {
            match state.own.as_mut().filter(|o| o.attempt == attempt) {
                Some(own) => {
                    own.follow = Some(Follow {
                        plan,
                        tick: 0,
                        started_sent: false,
                        done_sent: false,
                    });
                }
                None => say(
                    game,
                    &format!("План попытки #{attempt} пришёл без её начала"),
                ),
            }
        }
        // The server's own chat line already showed `line`: it goes into the journal only.
        ToClient::Finished { attempt, line } => {
            if let Some(own) = state.own.take_if(|o| o.attempt == attempt) {
                close_own(game, own, Some(&line));
            }
        }
        ToClient::Rope { id, rope } => match rope {
            Some(rope) => {
                state.ropes.insert(id, rope);
            }
            None => {
                state.ropes.remove(&id);
            }
        },
        ToClient::Hook {
            entity,
            mine,
            point,
        } => match point {
            Some(point) => {
                state.hooks.insert(entity, HookLine { mine, point });
            }
            None => {
                state.hooks.remove(&entity);
            }
        },
        ToClient::Watch {
            attempt,
            entity,
            kind,
            method,
            label,
        } => {
            let journal = open(kind, method, attempt, "observer", journal::OBSERVER_HEADER)
                .map_err(|e| say(game, &format!("Журнал наблюдателя не создан: {e}")))
                .ok();
            game.native_chat(&format!("Наблюдаю попытку #{attempt}: {label}"));
            state.watches.insert(
                attempt,
                Watch {
                    entity,
                    label,
                    journal,
                    began: Instant::now(),
                    watcher: Watcher::default(),
                    tick: 0,
                    missing: 0,
                },
            );
        }
        ToClient::Unwatch { attempt } => {
            if let Some(watch) = state.watches.remove(&attempt) {
                close_watch(game, attempt, watch);
            }
        }
    }
}

// ---- This player's attempt ----

fn own_tick(game: &mut Game<'_>, state: &mut Client) {
    let Some(own) = state.own.as_mut() else {
        return;
    };
    let Some((position, velocity)) = body(game) else {
        return;
    };
    let packets = POSITION_PACKETS
        .load(Ordering::Relaxed)
        .wrapping_sub(own.packets_at_start);
    let step = own.last.map(|last| rules::distance(last, position));
    // A teleport moves the body with no interpolation: the previous position is the new one.
    let snap = step.is_some_and(|s| s > SNAP_STEP)
        && rules::distance(vec3(game.previous_position), position) < 1e-6;
    own.snaps += u32::from(snap);
    own.watcher.push(position);
    let mut target = None;
    let mut push = None;
    let mut error = None;
    if let Some(follow) = own.follow.as_mut() {
        let attempt = own.attempt;
        if !follow.started_sent {
            // Once only: a failed send is said in chat, and the server's wait then runs out.
            follow.started_sent = true;
            send(game, &ToServer::Started { attempt });
        }
        let total = follow.plan.total_ticks();
        if follow.tick < total {
            let next = follow.plan.target_at(follow.tick + 1);
            let (kick, clamped) = plans::push_toward(next, position, velocity);
            game.native_push_player(kick.map(|v| v as f32));
            let off = rules::distance(position, follow.plan.target_at(follow.tick));
            own.stats.max_error = own.stats.max_error.max(off);
            own.stats.clamped += u32::from(clamped);
            target = Some(next);
            push = Some(kick);
            error = Some(off);
            follow.tick += 1;
        } else if !follow.done_sent {
            // The plan's end: stop the body there, then report.
            let (stop, _) = plans::push_toward(position, position, velocity);
            game.native_push_player(stop.map(|v| v as f32));
            push = Some(stop);
            own.stats.ticks = own.tick;
            own.stats.position_packets = packets;
            own.stats.snaps = own.snaps;
            own.stats.freezes = own.watcher.freezes();
            own.stats.max_step_change = own.watcher.max_step_change();
            follow.done_sent = true;
            send(
                game,
                &ToServer::Done {
                    attempt,
                    stats: own.stats,
                },
            );
        }
    }
    let line = format!(
        "{},{},{},{},{},{},{},{},{},{},{}",
        own.tick,
        own.began.elapsed().as_millis(),
        journal::cells(Some(position)),
        journal::cells(Some(velocity)),
        journal::cells(target),
        journal::cells(push),
        journal::cell(error),
        journal::cell(step),
        packets,
        game.riding.map_or(String::new(), |v| v.to_string()),
        snap,
    );
    if let Some(journal) = own.journal.as_mut() {
        journal.line(&line);
    }
    own.last = Some(position);
    own.tick += 1;
    if own.tick > own.planned + rules::OVERTIME_TICKS + SUMMARY_WAIT_TICKS
        && let Some(own) = state.own.take()
    {
        say(
            game,
            &format!("Итог попытки #{} от сервера не пришёл", own.attempt),
        );
        close_own(game, own, None);
    }
}

fn close_own(game: &mut Game<'_>, mut own: Own, server_line: Option<&str>) {
    own.stats.ticks = own.tick;
    own.stats.position_packets = POSITION_PACKETS
        .load(Ordering::Relaxed)
        .wrapping_sub(own.packets_at_start);
    own.stats.snaps = own.snaps;
    own.stats.freezes = own.watcher.freezes();
    own.stats.max_step_change = own.watcher.max_step_change();
    let mut line = own.stats.line();
    if let Some(mut journal) = own.journal.take() {
        if let Some(server_line) = server_line {
            journal.line(&format!("# сервер: {server_line}"));
        }
        journal.line(&format!("# {line}"));
        match journal.finish() {
            Ok(path) => line.push_str(&format!(". Журнал: {}", path.display())),
            Err(e) => line.push_str(&format!(". Журнал не дописан: {e}")),
        }
    }
    say(game, &line);
}

// ---- Other players' attempts ----

fn watch_tick(game: &mut Game<'_>, state: &mut Client) {
    for watch in state.watches.values_mut() {
        let Ok(id) = u32::try_from(watch.entity) else {
            continue;
        };
        let position = game
            .native_entity(id)
            .and_then(|entity| game.native_entity_position(entity));
        let step = position.and_then(|p| watch.watcher.push(p));
        if position.is_none() {
            watch.missing += 1;
        }
        let line = format!(
            "{},{},{},{},{}",
            watch.tick,
            watch.began.elapsed().as_millis(),
            journal::cells(position),
            journal::cell(step),
            if position.is_none() { "no entity" } else { "" },
        );
        if let Some(journal) = watch.journal.as_mut() {
            journal.line(&line);
        }
        watch.tick += 1;
    }
}

fn close_watch(game: &mut Game<'_>, attempt: u64, mut watch: Watch) {
    let mut line = format!(
        "Наблюдение #{attempt} ({}): {}",
        watch.label,
        watch.watcher.line()
    );
    if watch.missing > 0 {
        line.push_str(&format!(", сущность не видна {} тиков", watch.missing));
    }
    if let Some(journal) = watch.journal.take() {
        if let Err(e) = journal.finish() {
            line.push_str(&format!(". Журнал не дописан: {e}"));
        }
    }
    say(game, &line);
}

// ---- Drawing ----

/// Where the local player is drawn this frame.
fn drawn_feet(game: &Game<'_>, partial: f32) -> Option<V3> {
    let (now, _) = body(game)?;
    let before = vec3(game.previous_position);
    let t = f64::from(partial.clamp(0.0, 1.0));
    Some(rules::add(before, rules::scale(rules::sub(now, before), t)))
}

/// `native_world_primitives`: every rope, and each harpoon line from its puller to the hook.
pub fn draw(game: &Game<'_>, partial: f32, primitives: &mut WorldPrimitives) {
    let state = client();
    for rope in state.ropes.values() {
        let [low, high] = rope.anchors();
        primitives.line(low, high, ROPE_COLOUR, ROPE_WIDTH);
        primitives.line(rope.low, low, POST_COLOUR, POST_WIDTH);
        primitives.line(rope.high, high, POST_COLOUR, POST_WIDTH);
    }
    for (entity, hook) in &state.hooks {
        let from = if hook.mine {
            drawn_feet(game, partial)
        } else {
            u32::try_from(*entity)
                .ok()
                .and_then(|id| game.native_entity(id))
                .and_then(|e| game.native_entity_position(e))
        };
        if let Some(from) = from {
            primitives.line(
                rules::up(from, HAND_HEIGHT),
                hook.point,
                HOOK_COLOUR,
                HOOK_WIDTH,
            );
        }
        let corner = [HOOK_MARK; 3];
        primitives.box_outline(
            rules::sub(hook.point, corner),
            rules::add(hook.point, corner),
            HOOK_COLOUR,
            HOOK_WIDTH,
        );
    }
}
