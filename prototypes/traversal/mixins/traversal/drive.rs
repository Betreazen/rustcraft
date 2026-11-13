// The five ways of moving the player, one server tick at a time. The server half (trv_server)
// owns the attempt; this module only moves the player for it and reports what failed.
use crate::game::{
    Game, native_mobs::NativeSpawn, trv_measure::ClientStats, trv_plans as plans,
    trv_rules as rules,
};
use horizon_protocol::packets::play::{Clientbound, SetEntityMotion};
use plans::Plan;
use rules::{Method, V3};

/// Where an attempt is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Method a: waiting this many ticks for the client to start.
    WaitClient(u32),
    /// Method b: the seat is to be spawned.
    Spawn,
    /// Method b: trying to seat the player, this many ticks so far.
    Mount(u32),
    Move,
}

impl Phase {
    pub fn id(self) -> &'static str {
        match self {
            Phase::WaitClient(_) => "wait_client",
            Phase::Spawn => "spawn",
            Phase::Mount(_) => "mount",
            Phase::Move => "move",
        }
    }
}

/// The invisible mob the player rides under method b.
#[derive(Clone, Copy, Debug)]
pub struct Seat {
    pub id: u32,
    /// How far the rider's feet are over the seat's.
    pub rise: f64,
    pub measured: bool,
}

/// One attempt being driven.
pub struct Run {
    pub player: u128,
    pub entity: i32,
    pub dimension: String,
    pub method: Method,
    pub plan: Plan,
    /// Ticks of the plan done.
    pub tick: u32,
    pub phase: Phase,
    pub seat: Option<Seat>,
    /// Teleports the method itself sent the player.
    pub intended_teleports: u32,
    pub client_started: bool,
    pub client_done: Option<ClientStats>,
    /// Method d: ticks the plan waited at its gate.
    pub gate_waits: u32,
}

/// The server's record of a player this tick (`Hub`'s guest).
#[derive(Clone, Copy, Debug)]
pub struct GuestView {
    pub position: V3,
    pub vehicle: Option<u32>,
    pub teleport_counter: i32,
    pub above_ground: i32,
    pub floating: bool,
    pub yaw: f32,
    pub pitch: f32,
}

pub fn guest_view(game: &Game<'_>, player: u128) -> Option<GuestView> {
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
        above_ground: g.above_ground_ticks,
        floating: g.client_is_floating,
        yaw: g.yaw,
        pitch: g.pitch,
    })
}

/// Whether the attempt has finished moving.
pub enum Step {
    Going,
    Finished,
}

/// One server tick of the attempt: moves the player as the method says. An error ends it.
pub fn step(game: &mut Game<'_>, run: &mut Run, guest: &GuestView) -> Result<Step, String> {
    match run.phase {
        Phase::WaitClient(waited) => wait_client(run, waited),
        Phase::Spawn => spawn_seat(game, run),
        Phase::Mount(tries) => mount(game, run, guest, tries),
        Phase::Move => match run.method {
            Method::A => follow_client(run),
            Method::B | Method::B2 => move_seat(game, run, guest),
            Method::C => teleport(game, run, guest),
            Method::D => send_velocity(game, run, guest),
        },
    }
}

fn finished(run: &Run) -> Step {
    if run.tick >= run.plan.total_ticks() {
        Step::Finished
    } else {
        Step::Going
    }
}

fn wait_client(run: &mut Run, waited: u32) -> Result<Step, String> {
    if run.client_started {
        run.phase = Phase::Move;
        return Ok(Step::Going);
    }
    if waited >= rules::START_TIMEOUT_TICKS {
        return Err(format!(
            "клиент не начал движение за {} тиков: на клиенте нет прототипа, хук tick молчит или сообщение не дошло",
            rules::START_TIMEOUT_TICKS
        ));
    }
    run.phase = Phase::WaitClient(waited + 1);
    Ok(Step::Going)
}

/// Method a: the client moves itself; the server only waits for its report.
fn follow_client(run: &mut Run) -> Result<Step, String> {
    run.tick += 1;
    if run.client_done.is_some() {
        return Ok(Step::Finished);
    }
    if run.tick > run.plan.total_ticks() + rules::OVERTIME_TICKS {
        return Err("клиент не прислал итог вовремя".to_owned());
    }
    Ok(Step::Going)
}

fn spawn_seat(game: &mut Game<'_>, run: &mut Run) -> Result<Step, String> {
    let at = plans::seat_spot(run.plan.target_at(0), rules::SEAT_RIDER_RISE);
    let options = NativeSpawn {
        no_ai: true,
        invisible: true,
        silent: true,
        invulnerable: true,
        no_gravity: true,
        ..NativeSpawn::default()
    };
    let id = game
        .native_spawn_mob(&run.dimension, rules::SEAT_KIND, at, &options)
        .ok_or_else(|| {
            format!(
                "моб-сиденье {} не заспавнилось: native_spawn_mob вернул None (чанк не загружен или вид неизвестен)",
                rules::SEAT_KIND
            )
        })?;
    run.seat = Some(Seat {
        id,
        rise: rules::SEAT_RIDER_RISE,
        measured: false,
    });
    run.phase = Phase::Mount(0);
    Ok(Step::Going)
}

fn mount(
    game: &mut Game<'_>,
    run: &mut Run,
    guest: &GuestView,
    tries: u32,
) -> Result<Step, String> {
    let seat = run.seat.ok_or("сиденья нет")?;
    if guest.vehicle == Some(seat.id) {
        run.phase = Phase::Move;
        return Ok(Step::Going);
    }
    if tries >= rules::MOUNT_TRIES {
        return Err(format!(
            "native_mount не посадил игрока на сиденье за {} тиков (игрок-наблюдатель? уже в транспорте?)",
            rules::MOUNT_TRIES
        ));
    }
    let Ok(entity) = u32::try_from(run.entity) else {
        return Err(format!("у игрока нет номера сущности ({})", run.entity));
    };
    // Seen as seated on the next tick's record; tried again until then.
    if !game.native_mount(entity, seat.id) {
        log::warn!(
            "rc_traversal: native_mount({entity}, {}) returned false",
            seat.id
        );
    }
    run.phase = Phase::Mount(tries + 1);
    Ok(Step::Going)
}

fn move_seat(game: &mut Game<'_>, run: &mut Run, guest: &GuestView) -> Result<Step, String> {
    let Some(mut seat) = run.seat else {
        return Err("сиденья нет".to_owned());
    };
    if guest.vehicle != Some(seat.id) {
        return Err("игрок больше не сидит на сиденье (слез сам или его сняло)".to_owned());
    }
    if !seat.measured {
        // The first seated tick tells how high the rider really sits.
        if let Some(mob) = game.native_mob(&run.dimension, seat.id) {
            let rise = guest.position[1] - mob.pos[1];
            if plans::plausible_rise(rise) {
                seat.rise = rise;
            }
        }
        seat.measured = true;
        run.seat = Some(seat);
    }
    run.tick += 1;
    let to = plans::seat_spot(run.plan.target_at(run.tick), seat.rise);
    let moved = if run.method == Method::B {
        // Its riders go with it, and a rider's client is sent a teleport.
        run.intended_teleports += 1;
        game.native_teleport_vehicle(&run.dimension, seat.id, to)
    } else {
        game.native_teleport_mob(&run.dimension, seat.id, to)
    };
    if !moved {
        let call = if run.method == Method::B {
            "native_teleport_vehicle"
        } else {
            "native_teleport_mob"
        };
        return Err(format!(
            "{call} вернул false (сиденье пропало или хук не в тике уровня)"
        ));
    }
    Ok(finished(run))
}

/// Method c: the baseline, the player teleported along the path every tick.
fn teleport(game: &mut Game<'_>, run: &mut Run, guest: &GuestView) -> Result<Step, String> {
    run.tick += 1;
    let to = run.plan.target_at(run.tick);
    if !game.native_teleport_player(run.player, &run.dimension, to, guest.yaw, guest.pitch) {
        return Err("native_teleport_player вернул false".to_owned());
    }
    run.intended_teleports += 1;
    Ok(finished(run))
}

/// Method d: the server sets the player's velocity with the packet knockback uses, so its
/// movement checks expect the move.
fn send_velocity(game: &mut Game<'_>, run: &mut Run, guest: &GuestView) -> Result<Step, String> {
    let velocity = match run.plan.gate {
        // At the edge with the feet still under the top: the plan waits while they are lifted.
        Some(gate) if plans::gate_holds(&gate, run.tick, guest.position, run.gate_waits) => {
            run.gate_waits += 1;
            plans::gate_velocity(gate.point, guest.position)
        }
        _ => {
            run.tick += 1;
            plan_velocity(run, guest)
        }
    };
    let packet = Clientbound::SetEntityMotion(SetEntityMotion {
        id: run.entity,
        velocity,
    });
    if !game.native_send(run.player, packet) {
        return Err("native_send (SetEntityMotion) вернул false: игрока нет на сервере".to_owned());
    }
    // One more tick sends the stop.
    Ok(if run.tick > run.plan.total_ticks() {
        Step::Finished
    } else {
        Step::Going
    })
}

/// Method d off the gate: the plan's step into `run.tick` and a share of the lag; past the
/// plan, the stop.
fn plan_velocity(run: &Run, guest: &GuestView) -> V3 {
    if run.tick > run.plan.total_ticks() {
        return [0.0; 3];
    }
    let step = rules::sub(
        run.plan.target_at(run.tick),
        run.plan.target_at(run.tick - 1),
    );
    let seen_tick = run.tick.saturating_sub(1 + rules::VELOCITY_LAG_TICKS);
    plans::velocity_toward(step, run.plan.target_at(seen_tick), guest.position)
}

/// Method b: how far the rider's spot on the seat is from the plan's end. Read before
/// `release`, it tells a seat that got there from a server record of the rider that did not.
pub fn seat_end_gap(game: &Game<'_>, run: &Run) -> Option<f64> {
    let seat = run.seat?;
    let mob = game.native_mob(&run.dimension, seat.id)?;
    Some(rules::distance(
        rules::up(mob.pos, seat.rise),
        run.plan.path.end(),
    ))
}

/// Ends method b's ride: the player off the seat, the seat gone and, after a full move, the
/// player put on the plan's end (getting off leaves them beside the seat). Problems are
/// returned as notes for the journal.
pub fn release(
    game: &mut Game<'_>,
    run: &mut Run,
    completed: bool,
    guest: Option<&GuestView>,
) -> Vec<String> {
    let mut notes = Vec::new();
    let Some(seat) = run.seat.take() else {
        return notes;
    };
    if let Ok(entity) = u32::try_from(run.entity) {
        if guest.is_some_and(|g| g.vehicle == Some(seat.id)) && !game.native_dismount(entity) {
            notes.push("native_dismount вернул false".to_owned());
        }
    }
    if !game.native_remove_mob(&run.dimension, seat.id) {
        notes.push(format!(
            "сиденье {} не убрано (native_remove_mob вернул false): уберите его /kill",
            seat.id
        ));
    }
    if completed && run.plan.hold_ticks == 0 {
        let (yaw, pitch) = guest.map_or((0.0, 0.0), |g| (g.yaw, g.pitch));
        let end = run.plan.path.end();
        if game.native_teleport_player(run.player, &run.dimension, end, yaw, pitch) {
            run.intended_teleports += 1;
        } else {
            notes.push(
                "игрок не поставлен в конец пути (native_teleport_player вернул false)".to_owned(),
            );
        }
    }
    notes
}
