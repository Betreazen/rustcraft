// Server half of the traversal prototype: takes a player's request, plans the move, drives it
// each level tick with the chosen method (trv_drive), and writes the journal and the summary.
use crate::game::{
    Game, trv_drive as drive, trv_journal as journal, trv_measure as measure, trv_plans as plans,
    trv_rules as rules, trv_wire as wire,
};
use drive::{GuestView, Phase, Run, Step};
use horizon_client::{mod_api::NativeServerPlayer, text::Text};
use journal::Journal;
use plans::{Plan, Rope};
use rules::{Hit, Kind, Method};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::Instant,
};
use wire::{ToClient, ToServer};

/// One attempt with its measurements.
struct Session {
    attempt: u64,
    run: Run,
    tracker: measure::Tracker,
    journal: Option<Journal>,
    began: Instant,
    last_counter: Option<i32>,
    teleports: u32,
    setbacks: u32,
    max_floating: i32,
    /// A note for the next journal line.
    note: String,
}

#[derive(Default)]
struct Server {
    next_attempt: u64,
    level_ticks: u64,
    queue: Vec<(u128, ToServer)>,
    sessions: BTreeMap<u128, Session>,
    ropes: BTreeMap<u128, (u64, Rope)>,
    next_rope: u64,
}

static SERVER: LazyLock<Mutex<Server>> = LazyLock::new(Mutex::default);

fn server() -> MutexGuard<'static, Server> {
    SERVER.lock().unwrap_or_else(PoisonError::into_inner)
}

fn folder() -> Option<PathBuf> {
    Game::native_config_dir()
        .ok()
        .map(|dir| dir.join(journal::FOLDER))
}

/// A line in the server's event log and the server console.
fn event(text: &str) {
    log::info!("rc_traversal: {text}");
    if let Some(dir) = folder()
        && let Err(e) = journal::event(&dir, text)
    {
        log::warn!("rc_traversal: event log not written: {e}");
    }
}

fn send(game: &mut Game<'_>, player: u128, message: &ToClient) {
    if let Err(e) = game.native_net_send_to_player(player, wire::TO_CLIENT, &wire::encode(message))
    {
        log::warn!("rc_traversal: message to {player:032x} not sent: {e}");
    }
}

fn send_all(game: &mut Game<'_>, message: &ToClient) {
    for player in game.native_server_players() {
        send(game, player.uuid, message);
    }
}

fn tell(game: &mut Game<'_>, player: u128, line: &str) {
    if !game.native_tell(player, Text::literal(line.to_owned())) {
        log::warn!("rc_traversal: chat line to {player:032x} not shown: {line}");
    }
}

fn refuse(game: &mut Game<'_>, player: u128, reason: &str) {
    event(&format!("{player:032x} refused: {reason}"));
    send(
        game,
        player,
        &ToClient::Refused {
            reason: reason.to_owned(),
        },
    );
}

// ---- Hooks ----

/// `net_from_client`: requests wait for the level tick of the player's level.
pub fn on_client_message(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != wire::TO_SERVER {
        return;
    }
    let message = match wire::decode::<ToServer>(data) {
        Ok(message) => message,
        Err(e) => {
            refuse(
                game,
                player,
                &format!("сообщение клиента не прочитано ({e}): версии прототипа разные?"),
            );
            return;
        }
    };
    let mut state = server();
    match message {
        ToServer::Started { attempt } => {
            if let Some(s) = state
                .sessions
                .get_mut(&player)
                .filter(|s| s.attempt == attempt)
            {
                s.run.client_started = true;
            }
        }
        ToServer::Done { attempt, stats } => {
            if let Some(s) = state
                .sessions
                .get_mut(&player)
                .filter(|s| s.attempt == attempt)
            {
                s.run.client_done = Some(stats);
            }
        }
        other => {
            let queued = matches!(other, ToServer::Start { .. });
            state.queue.push((player, other));
            drop(state);
            if queued {
                send(game, player, &ToClient::Queued);
            }
        }
    }
}

/// `level_tick_post`: requests and attempts of the players in `dimension`, which is the level
/// in place here (the mob and teleport helpers act on it).
pub fn on_level_tick(game: &mut Game<'_>, dimension: &str) {
    let setbacks = horizon_client::trv_setbacks::take_all();
    let players = game.native_server_players();
    let mut state = server();
    for (player, to) in setbacks {
        if let Some(s) = state.sessions.get_mut(&player) {
            s.setbacks += 1;
            s.note.push_str(&format!(
                "setback to {:.2} {:.2} {:.2}; ",
                to[0], to[1], to[2]
            ));
        }
    }
    let (here, elsewhere): (Vec<_>, Vec<_>) = std::mem::take(&mut state.queue)
        .into_iter()
        .partition(|(player, _)| {
            players
                .iter()
                .any(|p| p.uuid == *player && p.dimension == dimension)
        });
    // A request from a player in no level at all (they left) is dropped.
    state.queue = elsewhere
        .into_iter()
        .filter(|(player, _)| players.iter().any(|p| p.uuid == *player))
        .collect();
    for (player, request) in here {
        if let Some(online) = players.iter().find(|p| p.uuid == player) {
            handle(game, &mut state, online, request);
        }
    }
    let ids: Vec<u128> = state
        .sessions
        .iter()
        .filter(|(_, s)| s.run.dimension == dimension)
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        advance(game, &mut state, id);
    }
    if dimension == "minecraft:overworld" {
        state.level_ticks += 1;
        if state.level_ticks.is_multiple_of(rules::ROPE_RESEND_TICKS) {
            let ropes: Vec<(u64, Rope)> = state.ropes.values().copied().collect();
            for (id, rope) in ropes {
                send_all(
                    game,
                    &ToClient::Rope {
                        id,
                        rope: Some(rope),
                    },
                );
            }
        }
    }
}

/// `player_logged_out`: the attempt ends without a summary to anyone, the seat goes.
pub fn on_logout(game: &mut Game<'_>, player: u128) {
    let mut state = server();
    state.queue.retain(|(p, _)| *p != player);
    if state.sessions.contains_key(&player) {
        finish(game, &mut state, player, Some("игрок вышел".to_owned()));
    }
    let rope = state.ropes.remove(&player);
    drop(state);
    if let Some((id, _)) = rope {
        send_all(game, &ToClient::Rope { id, rope: None });
    }
}

/// `server_stopping`: no seat is left behind in the saved world.
pub fn on_stopping(game: &mut Game<'_>) {
    let mut state = server();
    let ids: Vec<u128> = state.sessions.keys().copied().collect();
    for id in ids {
        finish(game, &mut state, id, Some("сервер остановлен".to_owned()));
    }
}

// ---- Requests ----

fn handle(game: &mut Game<'_>, state: &mut Server, online: &NativeServerPlayer, request: ToServer) {
    let player = online.uuid;
    match request {
        ToServer::Stop => {
            if state.sessions.contains_key(&player) {
                finish(
                    game,
                    state,
                    player,
                    Some("остановлено командой /proto stop".to_owned()),
                );
            } else {
                tell(game, player, "Попытки нет.");
            }
        }
        ToServer::Clear => match state.ropes.remove(&player) {
            Some((id, _)) => {
                send_all(game, &ToClient::Rope { id, rope: None });
                tell(game, player, "Трос убран.");
            }
            None => tell(game, player, "Своего троса нет."),
        },
        ToServer::Start { kind, method, hit } => {
            if let Err(reason) = start(game, state, online, kind, method, hit) {
                refuse(game, player, &reason);
            }
        }
        ToServer::Started { .. } | ToServer::Done { .. } => {}
    }
}

fn room_above(game: &Game<'_>, hit: &Hit) -> bool {
    let [x, y, z] = hit.block;
    (1..=2).all(|dy| rules::is_free(&game.native_block([x, y + dy, z])))
}

fn plan_for(
    game: &Game<'_>,
    state: &Server,
    feet: rules::V3,
    kind: Kind,
    hit: Option<Hit>,
) -> Result<Plan, String> {
    match kind {
        Kind::Hook | Kind::Zip => {
            let hit = hit.ok_or("клиент не прислал блок под прицелом")?;
            let room = room_above(game, &hit);
            if kind == Kind::Hook {
                plans::plan_hook(feet, &hit, room)
            } else {
                plans::plan_zip(feet, &hit, room)
            }
        }
        Kind::Ride => {
            let rope = state
                .ropes
                .values()
                .map(|(_, rope)| rope)
                .min_by(|a, b| a.end_gap(feet).total_cmp(&b.end_gap(feet)))
                .ok_or("Тросов нет: сначала /proto zip <способ>")?;
            plans::plan_ride(feet, rope)
        }
    }
}

fn start(
    game: &mut Game<'_>,
    state: &mut Server,
    online: &NativeServerPlayer,
    kind: Kind,
    method: Method,
    hit: Option<Hit>,
) -> Result<(), String> {
    let player = online.uuid;
    if state.sessions.contains_key(&player) {
        return Err("Попытка уже идёт: /proto stop".to_owned());
    }
    let guest = drive::guest_view(game, player)
        .ok_or("сервер не видит запись игрока (хаб недоступен в тике уровня)")?;
    if guest.vehicle.is_some() {
        return Err("Слезьте с транспорта".to_owned());
    }
    let plan = plan_for(game, state, guest.position, kind, hit)?;
    if method == Method::D || method.seated() {
        u32::try_from(online.entity_id)
            .ok()
            .filter(|id| *id != 0)
            .ok_or("сервер не знает номер сущности игрока")?;
    }
    state.next_attempt += 1;
    let attempt = state.next_attempt;
    if kind == Kind::Zip
        && let Some(rope) = plan.rope
    {
        state.next_rope += 1;
        let id = state.next_rope;
        if let Some((old, _)) = state.ropes.insert(player, (id, rope)) {
            send_all(
                game,
                &ToClient::Rope {
                    id: old,
                    rope: None,
                },
            );
        }
        send_all(
            game,
            &ToClient::Rope {
                id,
                rope: Some(rope),
            },
        );
    }
    let journal = open_journal(game, player, attempt, kind, method);
    let phase = match method {
        Method::A => Phase::WaitClient(0),
        Method::B | Method::B2 => Phase::Spawn,
        Method::C | Method::D => Phase::Move,
    };
    let planned = plan.total_ticks();
    let length = plan.length();
    let label = format!("{} {}: {}", online.name, kind.label(), method.label());
    send(
        game,
        player,
        &ToClient::Accepted {
            attempt,
            kind,
            method,
            planned,
            length,
        },
    );
    if method == Method::A {
        send(
            game,
            player,
            &ToClient::Follow {
                attempt,
                plan: plan.clone(),
            },
        );
    }
    if let Some(point) = plan.hook {
        for other in game.native_server_players() {
            let mine = other.uuid == player;
            send(
                game,
                other.uuid,
                &ToClient::Hook {
                    entity: online.entity_id,
                    mine,
                    point: Some(point),
                },
            );
        }
    }
    for other in game
        .native_server_players()
        .into_iter()
        .filter(|p| p.uuid != player)
    {
        let message = ToClient::Watch {
            attempt,
            entity: online.entity_id,
            kind,
            method,
            label: label.clone(),
        };
        send(game, other.uuid, &message);
    }
    event(&format!(
        "#{attempt} {label}: {length:.1} blocks, {planned} ticks, {} legs, climb {}",
        plan.legs.len(),
        plan.climb
    ));
    let run = Run {
        player,
        entity: online.entity_id,
        dimension: online.dimension.clone(),
        method,
        plan: plan.clone(),
        tick: 0,
        phase,
        seat: None,
        intended_teleports: 0,
        client_started: false,
        client_done: None,
        gate_waits: 0,
    };
    state.sessions.insert(
        player,
        Session {
            attempt,
            run,
            tracker: measure::Tracker::new(&plan),
            journal,
            began: Instant::now(),
            last_counter: None,
            teleports: 0,
            setbacks: 0,
            max_floating: 0,
            note: String::new(),
        },
    );
    Ok(())
}

fn open_journal(
    game: &mut Game<'_>,
    player: u128,
    attempt: u64,
    kind: Kind,
    method: Method,
) -> Option<Journal> {
    let Some(dir) = folder() else {
        tell(
            game,
            player,
            "Журнал не создан: у сервера нет папки config.",
        );
        return None;
    };
    let name = journal::file_name(journal::unix_seconds(), attempt, kind, method, "server");
    match Journal::create(&dir, &name, journal::SERVER_HEADER) {
        Ok(journal) => Some(journal),
        Err(e) => {
            tell(game, player, &format!("Журнал не создан: {e}"));
            None
        }
    }
}

// ---- Attempts ----

fn advance(game: &mut Game<'_>, state: &mut Server, player: u128) {
    let Some(guest) = drive::guest_view(game, player) else {
        finish(
            game,
            state,
            player,
            Some("игрока нет в записях сервера".to_owned()),
        );
        return;
    };
    let Some(session) = state.sessions.get_mut(&player) else {
        return;
    };
    // A ride ends lower than it started and a failed climb drops the player: while moved, and
    // FALL_GRACE_MS after, their falls do no damage.
    if let Ok(entity) = u32::try_from(session.run.entity) {
        horizon_client::trv_setbacks::spare_falls(
            entity,
            std::time::Duration::from_millis(rules::FALL_GRACE_MS),
        );
    }
    if let Some(last) = session.last_counter {
        session.teleports += guest.teleport_counter.wrapping_sub(last).clamp(0, 1000) as u32;
    }
    session.last_counter = Some(guest.teleport_counter);
    session.max_floating = session.max_floating.max(guest.above_ground);
    // The record read now shows the moves of the ticks before: it is sampled before this
    // tick's step, against the plan's tick it belongs to.
    record(game, session, &guest);
    let outcome = drive::step(game, &mut session.run, &guest);
    match outcome {
        Ok(Step::Going) => {}
        Ok(Step::Finished) => finish(game, state, player, None),
        Err(reason) => finish(game, state, player, Some(reason)),
    }
}

/// The tick's journal line, and the tracker's sample once the player is moving.
fn record(game: &Game<'_>, s: &mut Session, guest: &GuestView) {
    let moving = s.run.phase == Phase::Move;
    let sample = moving.then(|| s.tracker.push(s.run.tick, guest.position));
    let target = moving.then(|| s.run.plan.target_at(s.run.tick));
    let seat = s
        .run
        .seat
        .and_then(|seat| game.native_mob(&s.run.dimension, seat.id))
        .map(|mob| mob.pos);
    let line = format!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        s.run.tick,
        s.began.elapsed().as_millis(),
        s.run.phase.id(),
        journal::cells(Some(guest.position)),
        journal::cells(target),
        journal::cell(sample.map(|x| x.off_path)),
        journal::cell(sample.map(|x| x.lag)),
        journal::cells(seat),
        guest.vehicle.map_or(String::new(), |v| v.to_string()),
        s.teleports,
        s.setbacks,
        guest.above_ground,
        guest.floating,
        std::mem::take(&mut s.note).replace(',', ";"),
    );
    if let Some(journal) = s.journal.as_mut() {
        journal.line(&line);
    }
}

/// Ends `player`'s attempt: the seat goes, the summary is told and written. `failure` says
/// why it ended early.
fn finish(game: &mut Game<'_>, state: &mut Server, player: u128, failure: Option<String>) {
    let Some(mut s) = state.sessions.remove(&player) else {
        return;
    };
    let guest = drive::guest_view(game, player);
    let seat_gap = drive::seat_end_gap(game, &s.run);
    let notes = drive::release(game, &mut s.run, failure.is_none(), guest.as_ref());
    let mut summary =
        measure::Summary::from_tracker(s.attempt, &s.run.plan, s.run.method, &s.tracker);
    summary.setbacks = s.setbacks;
    summary.teleports = s.teleports;
    summary.intended_teleports = s.run.intended_teleports;
    summary.max_floating = s.max_floating;
    summary.failure = failure;
    let mut line = summary.line();
    if let Some(gap) = seat_gap {
        line.push_str(&format!(" Сиденье до конца пути: {gap:.2} бл."));
    }
    // Method d: how long the plan waited at the edge, and whether the cap ended the wait with the
    // feet still under the top. Lag in the summary includes these ticks.
    if s.run.gate_waits > 0 {
        let capped = if s.run.gate_waits >= rules::GATE_MAX_TICKS {
            ", предел: ноги так и не поднялись выше верха"
        } else {
            ""
        };
        line.push_str(&format!(
            " Ждал у края {} тиков{capped}.",
            s.run.gate_waits
        ));
    }
    for note in &notes {
        line.push_str(&format!(" {note}."));
    }
    if let Some(stats) = s.run.client_done {
        if let Some(journal) = s.journal.as_mut() {
            journal.line(&format!("# {}", stats.line()));
        }
    }
    if let Some(mut journal) = s.journal.take() {
        journal.line(&format!("# {line}"));
        if let Err(e) = journal.finish() {
            line.push_str(&format!(" Журнал не дописан: {e}."));
        }
    }
    if let Some(dir) = folder()
        && let Err(e) = journal::append(
            &dir,
            journal::SUMMARY_FILE,
            Some(measure::Summary::CSV_HEADER),
            &summary.csv_row(),
        )
    {
        line.push_str(&format!(" Сводка не записана: {e}."));
    }
    event(&line);
    // In chat even on a client without the prototype; the prototype's client adds its own line.
    tell(game, player, &line);
    send(
        game,
        player,
        &ToClient::Finished {
            attempt: s.attempt,
            line,
        },
    );
    if s.run.plan.hook.is_some() {
        send_all(
            game,
            &ToClient::Hook {
                entity: s.run.entity,
                mine: false,
                point: None,
            },
        );
    }
    send_all(game, &ToClient::Unwatch { attempt: s.attempt });
}
