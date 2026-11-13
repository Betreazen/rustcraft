// Game-free rules of the traversal prototype: every tuning number, the chat command, the block
// aimed at, and paths with their timing. The plans built on them are in plans.rs. The game
// compiles this file as `trv_rules`; lib.rs builds it alone for `cargo test`.
use serde::{Deserialize, Serialize};

/// A point or a vector in world blocks.
pub type V3 = [f64; 3];

// ---- Tuning: every number of the prototype lives here. ----

pub const TICKS_PER_SECOND: f64 = 20.0;
/// Harpoon reach and cruise speed: 20 blocks take 27 ticks (1.35 s) with the easing below.
pub const HOOK_RANGE: f64 = 20.0;
pub const HOOK_SPEED: f64 = 18.0;
/// Climbing over the edge after the pull, blocks a second.
pub const CLIMB_SPEED: f64 = 6.0;
/// Zipline reach and ride speed: a first-tier minecart's 8 blocks a second.
pub const ZIP_RANGE: f64 = 32.0;
pub const ZIP_SPEED: f64 = 8.0;
/// Ticks spent speeding up and slowing down at each end of a leg.
pub const EASE_TICKS: u32 = 4;
/// How long a harpoon that cannot climb keeps the player at the face before letting go.
pub const HANG_TICKS: u32 = 40;
/// The player's box (vanilla) and eyes.
pub const PLAYER_HALF_WIDTH: f64 = 0.3;
pub const PLAYER_HEIGHT: f64 = 1.8;
pub const EYE_HEIGHT: f64 = 1.62;
/// Gap between the box and the face the player hangs on. It is inside the 1/16 the server's
/// floating check looks around the box, so a hanging player does not count as floating.
pub const HANG_GAP: f64 = 0.03;
/// Feet below the harpoon's hit point while hanging on a side face: the hands are at the hook.
pub const HANG_HANDS: f64 = 1.5;
/// How high over the block's top the climb lifts the feet before stepping over the edge. Method
/// d's player trails the plan by about 0.3 (first test, 2026-11-11): at 0.1 their feet were still
/// under the top when the plan turned over the edge, so they hit the face and fell.
pub const LIFT_CLEARANCE: f64 = 0.4;
/// Method d: at the edge the plan waits until the server sees the feet this far over the top,
/// at most GATE_MAX_TICKS, steering the feet to the lift point at GATE_GAIN of the error a tick.
pub const GATE_MARGIN: f64 = 0.05;
pub const GATE_MAX_TICKS: u32 = 10;
pub const GATE_GAIN: f64 = 0.6;
/// A zipline's rider leaves or reaches the top anchor over the edge, this far out from the
/// block's centre along the bigger horizontal axis: half a block, half the player and a gap.
/// A straight rope line would cut the block's corner.
pub const EDGE_REACH: f64 = 0.5 + PLAYER_HALF_WIDTH + 0.1;
/// Falls of a player being moved do no damage, nor for this long after the attempt.
pub const FALL_GRACE_MS: u64 = 2000;
/// Height of the rope over the stand point at each anchor.
pub const ROPE_HEIGHT: f64 = 2.0;
/// How near an end of a rope a player must stand for `/proto ride`.
pub const RIDE_REACH: f64 = 3.0;
/// The upper anchor stands at least this much above the feet.
pub const ZIP_MIN_RISE: f64 = 1.0;
/// Slack on the range check: the server sees the player a tick or two late.
pub const RANGE_SLACK: f64 = 1.5;
/// A move shorter than this is refused as pointless.
pub const MIN_PATH: f64 = 0.5;
/// Ending within this of the plan's end counts as arrived.
pub const ARRIVE_TOLERANCE: f64 = 0.75;
/// The seat of method b: an invisible pig with no AI and no gravity. A player's feet sit this
/// far over the pig's (Pig passenger attachment 0.86875 less a player's 0.6); measured again
/// once the player is seated.
pub const SEAT_KIND: &str = "minecraft:pig";
pub const SEAT_RIDER_RISE: f64 = 0.26875;
/// A measured seat rise outside this is not believed.
pub const SEAT_RISE_MAX: f64 = 2.5;
/// Ticks the server keeps trying to seat the player.
pub const MOUNT_TRIES: u32 = 10;
/// Ticks the server waits for the client to start following (method a).
pub const START_TIMEOUT_TICKS: u32 = 40;
/// Ticks past the plan before an attempt is ended as unfinished.
pub const OVERTIME_TICKS: u32 = 60;
/// Ticks the client waits for the server's answer to a command.
pub const REPLY_TIMEOUT_TICKS: u32 = 60;
/// Method d: share of the position error the server corrects each tick, and the ticks the
/// server's view of the player lags the velocity it sent.
pub const VELOCITY_GAIN: f64 = 0.25;
pub const VELOCITY_LAG_TICKS: u32 = 1;
/// A push (method a) or velocity (method d) component is clamped to this. native_push_player
/// clamps at 4 too.
pub const STEP_MAX: f64 = 4.0;
/// The server kicks a player who hovers longer than this (vanilla's floating check).
pub const FLOATING_KICK_TICKS: i32 = 80;
/// Smoothness: a step this much longer than the plan's fastest is a jump; in the steady part
/// of a leg a step shorter than this share of the planned one is a stall.
pub const JUMP_SLACK: f64 = 0.5;
pub const STALL_SHARE: f64 = 0.25;
/// For a log without a plan (an observer's): a still tick between two moving ones is a freeze.
pub const FREEZE_STEP: f64 = 0.02;
pub const MOVE_STEP: f64 = 0.1;
/// Ropes are sent again this often, so a player who just joined sees them.
pub const ROPE_RESEND_TICKS: u64 = 100;

/// The chat command, without its slash.
pub const COMMAND: &str = "proto";
pub const HELP: [&str; 6] = [
    "Прототип перемещения RustCraft:",
    "/proto hook <способ> — гарпун в блок под прицелом (до 20 блоков)",
    "/proto zip <способ> — трос от ног к блоку под прицелом (выше вас, до 32 блоков) и езда вверх",
    "/proto ride <способ> — езда по ближайшему тросу от ближнего конца к дальнему",
    "/proto stop — прервать попытку; /proto clear — убрать свой трос",
    "Способы: a толчок на клиенте, b сиденье+teleport_vehicle, b2 сиденье+teleport_mob, c телепорт каждый тик, d скорость от сервера",
];

// ---- Vectors ----

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn length(a: V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

pub fn distance(a: V3, b: V3) -> f64 {
    length(sub(a, b))
}

pub fn up(a: V3, dy: f64) -> V3 {
    [a[0], a[1] + dy, a[2]]
}

fn finite(a: V3) -> bool {
    a.iter().all(|v| v.is_finite())
}

// ---- What the player asks for ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Hook,
    Zip,
    Ride,
}

impl Kind {
    fn parse(word: &str) -> Option<Kind> {
        match word {
            "hook" => Some(Kind::Hook),
            "zip" => Some(Kind::Zip),
            "ride" => Some(Kind::Ride),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Kind::Hook => "hook",
            Kind::Zip => "zip",
            Kind::Ride => "ride",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Hook => "гарпун",
            Kind::Zip => "зиплайн",
            Kind::Ride => "езда по тросу",
        }
    }

    /// How far the block the player aims at may be, for the kinds that aim.
    pub fn reach(self) -> Option<f64> {
        match self {
            Kind::Hook => Some(HOOK_RANGE),
            Kind::Zip => Some(ZIP_RANGE),
            Kind::Ride => None,
        }
    }
}

/// How the player is moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    /// The client pushes its own player each tick (native_push_player).
    A,
    /// The player rides an invisible seat the server teleports with native_teleport_vehicle.
    B,
    /// The same seat moved with native_teleport_mob, which leaves its rider alone.
    B2,
    /// The server teleports the player every tick (native_teleport_player).
    C,
    /// The server sends the player's velocity every tick (ClientboundSetEntityMotion).
    D,
}

impl Method {
    fn parse(word: &str) -> Option<Method> {
        match word {
            "a" => Some(Method::A),
            "b" => Some(Method::B),
            "b2" => Some(Method::B2),
            "c" => Some(Method::C),
            "d" => Some(Method::D),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Method::A => "a",
            Method::B => "b",
            Method::B2 => "b2",
            Method::C => "c",
            Method::D => "d",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Method::A => "толчок на клиенте",
            Method::B => "сиденье, teleport_vehicle",
            Method::B2 => "сиденье, teleport_mob",
            Method::C => "телепорт каждый тик",
            Method::D => "скорость от сервера",
        }
    }

    pub fn seated(self) -> bool {
        matches!(self, Method::B | Method::B2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Start(Kind, Method),
    Stop,
    Clear,
    Help,
}

/// The `/proto` command; None for any other command.
pub fn parse_command(line: &str) -> Option<Result<Command, String>> {
    let lower = line.trim().to_lowercase();
    let mut words = lower.split_whitespace();
    if words.next()? != COMMAND {
        return None;
    }
    let rest: Vec<&str> = words.collect();
    Some(match rest.as_slice() {
        [] | ["help"] => Ok(Command::Help),
        ["stop"] => Ok(Command::Stop),
        ["clear"] => Ok(Command::Clear),
        [kind] if Kind::parse(kind).is_some() => {
            Err(format!("Укажите способ: /proto {kind} a|b|b2|c|d"))
        }
        [kind, method] => match (Kind::parse(kind), Method::parse(method)) {
            (Some(kind), Some(method)) => Ok(Command::Start(kind, method)),
            (None, _) => Err(format!(
                "Нет такого действия «{kind}»: hook, zip, ride, stop, clear"
            )),
            (_, None) => Err(format!("Нет такого способа «{method}»: a, b, b2, c, d")),
        },
        _ => Err(format!("Не понял «/{}». Справка: /proto help", line.trim())),
    })
}

// ---- The block aimed at ----

/// A block the player's view meets and where.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub block: [i32; 3],
    pub point: V3,
}

/// The face of the block the hit point lies on, as its outward normal. Ties go to the top.
pub fn face_of(hit: &Hit) -> [i32; 3] {
    let local = [0, 1, 2].map(|i| hit.point[i] - f64::from(hit.block[i]));
    let faces: [([i32; 3], f64); 6] = [
        ([0, 1, 0], 1.0 - local[1]),
        ([0, -1, 0], local[1]),
        ([1, 0, 0], 1.0 - local[0]),
        ([-1, 0, 0], local[0]),
        ([0, 0, 1], 1.0 - local[2]),
        ([0, 0, -1], local[2]),
    ];
    let mut best = faces[0];
    for face in faces {
        if face.1.abs() < best.1.abs() {
            best = face;
        }
    }
    best.0
}

/// Whether a player's body may stand in a block of this kind.
pub fn is_free(block: &str) -> bool {
    matches!(
        block,
        "minecraft:air"
            | "minecraft:cave_air"
            | "minecraft:void_air"
            | "minecraft:light"
            | "minecraft:short_grass"
            | "minecraft:fern"
            | "minecraft:dandelion"
            | "minecraft:poppy"
    )
}

/// Refuses a block farther than `range` from the player's eyes.
pub fn check_reach(feet: V3, hit: &Hit, range: f64) -> Result<(), String> {
    let away = distance(up(feet, EYE_HEIGHT), hit.point);
    if away > range + RANGE_SLACK {
        return Err(format!(
            "Цель в {away:.1} блоках, а можно не дальше {range:.0}"
        ));
    }
    Ok(())
}

// ---- Paths and timing ----

/// A polyline the player's feet follow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Path {
    points: Vec<V3>,
}

impl Path {
    /// A path through `points`, repeated points dropped.
    pub fn new(points: &[V3]) -> Result<Path, String> {
        if !points.iter().all(|p| finite(*p)) {
            return Err("в пути есть нечисловая точка".to_owned());
        }
        let mut kept: Vec<V3> = Vec::with_capacity(points.len());
        for p in points {
            if kept.last().is_none_or(|last| distance(*last, *p) > 1e-6) {
                kept.push(*p);
            }
        }
        let path = Path { points: kept };
        if path.points.len() < 2 || path.length() < MIN_PATH {
            return Err(format!("Цель слишком близко: путь короче {MIN_PATH} блока"));
        }
        Ok(path)
    }

    pub fn points(&self) -> &[V3] {
        &self.points
    }

    pub fn length(&self) -> f64 {
        self.points.windows(2).map(|w| distance(w[0], w[1])).sum()
    }

    pub fn start(&self) -> V3 {
        self.points.first().copied().unwrap_or_default()
    }

    pub fn end(&self) -> V3 {
        self.points.last().copied().unwrap_or_default()
    }

    /// The point `along` blocks from the start, held at both ends.
    pub fn at(&self, along: f64) -> V3 {
        let mut left = along.max(0.0);
        for w in self.points.windows(2) {
            let span = distance(w[0], w[1]);
            if left <= span && span > 0.0 {
                return add(w[0], scale(sub(w[1], w[0]), left / span));
            }
            left -= span;
        }
        self.end()
    }

    /// How far along the path the point nearest to `p` is, and how far `p` is from it.
    pub fn locate(&self, p: V3) -> (f64, f64) {
        let mut best = (0.0, f64::INFINITY);
        let mut walked = 0.0;
        for w in self.points.windows(2) {
            let span = sub(w[1], w[0]);
            let span_length = length(span);
            let t = if span_length > 0.0 {
                let dot: f64 = (0..3).map(|i| (p[i] - w[0][i]) * span[i]).sum();
                (dot / (span_length * span_length)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let nearest = add(w[0], scale(span, t));
            let off = distance(p, nearest);
            if off < best.1 {
                best = (walked + t * span_length, off);
            }
            walked += span_length;
        }
        best
    }
}

/// How one leg of a move spreads its length over ticks: speeding up over `ease` ticks, a steady
/// cruise, slowing down over `ease` ticks.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    pub length: f64,
    pub ticks: u32,
    pub ease: u32,
}

impl Schedule {
    /// `length` blocks at a cruise of `speed` blocks a second.
    pub fn new(length: f64, speed: f64, ease: u32) -> Schedule {
        let per_tick = (speed / TICKS_PER_SECOND).max(1e-3);
        let cruise = (length.max(0.0) / per_tick).ceil() as u32;
        let ticks = (cruise + ease).max(2 * ease).max(1);
        Schedule {
            length: length.max(0.0),
            ticks,
            ease,
        }
    }

    fn ease_ticks(&self) -> u32 {
        self.ease.min(self.ticks / 2)
    }

    /// The fastest step, blocks a tick.
    pub fn peak_step(&self) -> f64 {
        self.length / f64::from(self.ticks - self.ease_ticks())
    }

    /// Blocks covered after `tick` ticks.
    pub fn distance_at(&self, tick: u32) -> f64 {
        let t = f64::from(tick.min(self.ticks));
        let total = f64::from(self.ticks);
        let e = f64::from(self.ease_ticks());
        if e == 0.0 {
            return self.length * t / total;
        }
        let v = self.length / (total - e);
        if t < e {
            v * t * t / (2.0 * e)
        } else if t <= total - e {
            v * (t - e / 2.0)
        } else {
            self.length - v * (total - t).powi(2) / (2.0 * e)
        }
    }

    /// Whether `tick` is in the steady cruise, where the step is the full peak step.
    pub fn cruising(&self, tick: u32) -> bool {
        let e = self.ease_ticks();
        tick > e && tick <= self.ticks - e
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn close3(a: V3, b: V3) -> bool {
        distance(a, b) < 1e-6
    }

    #[test]
    fn the_command_takes_an_action_and_a_method() {
        assert_eq!(
            parse_command("proto hook a"),
            Some(Ok(Command::Start(Kind::Hook, Method::A)))
        );
        assert_eq!(
            parse_command("  PROTO zip B2 "),
            Some(Ok(Command::Start(Kind::Zip, Method::B2)))
        );
        assert_eq!(
            parse_command("proto ride d"),
            Some(Ok(Command::Start(Kind::Ride, Method::D)))
        );
        assert_eq!(parse_command("proto"), Some(Ok(Command::Help)));
        assert_eq!(parse_command("proto stop"), Some(Ok(Command::Stop)));
        assert_eq!(parse_command("proto clear"), Some(Ok(Command::Clear)));
    }

    #[test]
    fn other_commands_are_left_alone_and_mistakes_are_explained() {
        assert_eq!(parse_command("tree"), None);
        assert_eq!(parse_command("protox hook a"), None);
        assert_eq!(parse_command(""), None);
        let no_method = parse_command("proto hook").unwrap().unwrap_err();
        assert!(no_method.contains("a|b|b2|c|d"), "{no_method}");
        let bad_method = parse_command("proto hook e").unwrap().unwrap_err();
        assert!(bad_method.contains("«e»"), "{bad_method}");
        let bad_kind = parse_command("proto fly a").unwrap().unwrap_err();
        assert!(bad_kind.contains("«fly»"), "{bad_kind}");
        assert!(parse_command("proto hook a now").unwrap().is_err());
    }

    #[test]
    fn the_face_hit_is_the_one_the_point_lies_on() {
        let block = [10, 64, -3];
        let hit = |point| Hit { block, point };
        assert_eq!(face_of(&hit([10.5, 65.0, -2.5])), [0, 1, 0]);
        assert_eq!(face_of(&hit([10.5, 64.0, -2.5])), [0, -1, 0]);
        assert_eq!(face_of(&hit([11.0, 64.4, -2.5])), [1, 0, 0]);
        assert_eq!(face_of(&hit([10.0, 64.4, -2.5])), [-1, 0, 0]);
        assert_eq!(face_of(&hit([10.3, 64.4, -2.0])), [0, 0, 1]);
        assert_eq!(face_of(&hit([10.3, 64.4, -3.0])), [0, 0, -1]);
        // An edge belongs to the top.
        assert_eq!(face_of(&hit([11.0, 65.0, -2.5])), [0, 1, 0]);
    }

    #[test]
    fn a_path_walks_its_segments_and_finds_the_nearest_point() {
        let path = Path::new(&[[0.0; 3], [3.0, 0.0, 0.0], [3.0, 4.0, 0.0]]).unwrap();
        assert!(close(path.length(), 7.0));
        assert!(close3(path.at(1.5), [1.5, 0.0, 0.0]));
        assert!(close3(path.at(5.0), [3.0, 2.0, 0.0]));
        assert!(close3(path.at(-1.0), [0.0; 3]));
        assert!(close3(path.at(99.0), [3.0, 4.0, 0.0]));
        let (along, off) = path.locate([4.0, 1.0, 0.0]);
        assert!(close(along, 4.0) && close(off, 1.0), "{along} {off}");
    }

    #[test]
    fn a_path_drops_repeats_and_refuses_a_pointless_move() {
        let path = Path::new(&[[0.0; 3], [0.0; 3], [2.0, 0.0, 0.0]]).unwrap();
        assert_eq!(path.points().len(), 2);
        assert!(Path::new(&[[0.0; 3], [0.2, 0.0, 0.0]]).is_err());
        assert!(Path::new(&[[0.0; 3], [f64::NAN, 0.0, 0.0]]).is_err());
    }

    #[test]
    fn a_schedule_eases_in_and_out_and_ends_at_its_length() {
        let s = Schedule::new(20.0, HOOK_SPEED, EASE_TICKS);
        // 20 blocks at 0.9 a tick: 23 cruise ticks plus the easing, 1.35 seconds.
        assert_eq!(s.ticks, 27);
        assert!(close(s.distance_at(0), 0.0));
        assert!(close(s.distance_at(s.ticks), 20.0));
        assert!(close(s.distance_at(1000), 20.0));
        let steps: Vec<f64> = (1..=s.ticks)
            .map(|t| s.distance_at(t) - s.distance_at(t - 1))
            .collect();
        assert!(steps.iter().all(|step| *step > 0.0));
        assert!(steps.iter().all(|step| *step <= s.peak_step() + 1e-9));
        // No step changes by more than the easing allows: the move has no jolt.
        let jolt = steps
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max);
        assert!(
            jolt <= s.peak_step() / f64::from(EASE_TICKS) + 1e-9,
            "{jolt}"
        );
    }

    #[test]
    fn the_numbers_meet_the_spec() {
        let hook = Schedule::new(HOOK_RANGE, HOOK_SPEED, EASE_TICKS);
        let seconds = f64::from(hook.ticks) / TICKS_PER_SECOND;
        assert!((1.0..=1.5).contains(&seconds), "20 blocks in {seconds} s");
        let zip = Schedule::new(8.0, ZIP_SPEED, EASE_TICKS);
        assert!(close(zip.peak_step() * TICKS_PER_SECOND, 8.0));
        assert!(hook.peak_step() < STEP_MAX);
    }

    #[test]
    fn a_short_schedule_still_has_room_to_ease() {
        let s = Schedule::new(0.6, ZIP_SPEED, EASE_TICKS);
        assert_eq!(s.ticks, 2 * EASE_TICKS);
        assert!(close(s.distance_at(EASE_TICKS), 0.3));
        assert!(close(s.distance_at(s.ticks), 0.6));
        let linear = Schedule::new(4.0, 20.0, 0);
        assert_eq!(linear.ticks, 4);
        assert!(close(linear.distance_at(1), 1.0));
    }

    #[test]
    fn only_air_and_plants_leave_room_for_a_body() {
        assert!(is_free("minecraft:air") && is_free("minecraft:short_grass"));
        assert!(!is_free("minecraft:stone") && !is_free("minecraft:water"));
    }
}
