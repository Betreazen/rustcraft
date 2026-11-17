//! How the client draws a worker (slice 18, guide `chassis`, docs/showcase/js/models.js
//! `M.worker` and poses.js `P.worker`): the chassis in its tool's colour, the bracket of the rail
//! it holds (below a floor rail, above a ceiling rail, beside a wall rail and turned to the wall),
//! and its tool's parts, which turn to what the tool works on, slide out and spin while it works.
//!
//! Frames: the model's own, in blocks with Minecraft axes, x forward (the tool's side), y up, z to
//! the right of the chassis as the exporter swaps the guide's axes; the model spans 0..1 in x and
//! z, its floor at the worker's feet. The client turns that frame to the worker's heading.

use super::Tool;
use super::pivots::PIVOTS;
use crate::geom::Face;

/// One move of a part, in the model's frame: applied in order, the first outermost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// Turn `deg` degrees about `axis` (0 x, 1 y, 2 z) through `pivot`.
    Turn { axis: u8, deg: f64, pivot: [f64; 3] },
    Move([f64; 3]),
}

/// A part model of the worker and how it moves.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub model: &'static str,
    pub ops: Vec<Op>,
}

/// The tool's colour on the chassis (models.js `TOOL`); the bare chassis's grey.
pub fn colour(tool: Option<Tool>) -> u32 {
    match tool {
        Some(Tool::Saw) => 0xd8dce4,
        Some(Tool::Reaper) => 0x8fd35a,
        Some(Tool::Hold) => 0xa8793f,
        Some(Tool::Grab) => 0xffa53c,
        Some(Tool::Bore) => 0x9aa3b2,
        Some(Tool::Shooter) => 0xe0564a,
        Some(Tool::Scoop) => 0xe08a3a,
        Some(Tool::Map) => 0x5ab0e6,
        None => 0x8a919d,
    }
}

fn pivot(part: &str) -> [f64; 3] {
    PIVOTS
        .iter()
        .find(|(p, _)| *p == part)
        .map_or([0.5, 0.5, 0.5], |(_, v)| *v)
}

fn turn(part: &str, axis: u8, rad: f64) -> Op {
    Op::Turn {
        axis,
        deg: rad.to_degrees(),
        pivot: pivot(part),
    }
}

/// poses.js `pulse`: 0..1 at `speed` radians a second.
fn pulse(t: f64, speed: f64) -> f64 {
    0.5 + 0.5 * (t * speed).sin()
}

/// The guide's pitch limits for aim_pitch parts (player.js `PITCH_DOWN`, `PITCH_UP`).
const PITCH_DOWN: f64 = -0.6;
const PITCH_UP: f64 = 1.2;
/// reach_* parts slide out by up to half a block (poses.js, «reach»).
const REACH: f64 = 0.5;

/// How the tool's parts turn and slide toward `aim`, a point in the model's frame (None: back to
/// rest), and whether the tool is at work, `t` seconds into the client's clock.
pub fn tool_parts(tool: Tool, busy: bool, t: f64, aim: Option<[f64; 3]>) -> Vec<Part> {
    let yaw_to = |part: &str| -> f64 {
        let Some(a) = aim else { return 0.0 };
        let p = pivot(part);
        let (dx, dz) = (a[0] - p[0], a[2] - p[2]);
        if dx.hypot(dz) < 1e-6 {
            return 0.0;
        }
        (-dz).atan2(dx)
    };
    let reach = |part: &str| -> f64 {
        let Some(a) = aim else { return 0.0 };
        let p = pivot(part);
        let far = (a[0] - p[0]).hypot(a[2] - p[2]);
        // Out toward the aim, by up to half a block, when busy: the guide's saw reaches the tree.
        if busy { (far - 0.5).clamp(0.0, REACH) } else { 0.0 }
    };
    let part = |model: &'static str, ops: Vec<Op>| Part { model, ops };
    match tool {
        Tool::Saw => {
            let yaw = turn("aim_yaw_saw", 1, yaw_to("aim_yaw_saw"));
            let slide = Op::Move([reach("reach_saw"), 0.0, 0.0]);
            let spin = turn("saw_disc", 2, if busy { t * 14.0 } else { 0.0 });
            vec![
                part("rustcraft:block/worker_saw", vec![]),
                part("rustcraft:block/worker_aim_yaw_saw", vec![yaw]),
                part("rustcraft:block/worker_reach_saw", vec![yaw, slide]),
                part("rustcraft:block/worker_saw_disc", vec![yaw, slide, spin]),
            ]
        }
        Tool::Reaper => vec![
            part("rustcraft:block/worker_reaper", vec![]),
            part(
                "rustcraft:block/worker_reaper_reel",
                vec![turn("reaper_reel", 2, if busy { t * 5.0 } else { 0.0 })],
            ),
        ],
        Tool::Hold => vec![part("rustcraft:block/worker_hold", vec![])],
        Tool::Grab => {
            let s = if busy { 1.25 / 16.0 * pulse(t, 5.0) } else { 0.0 };
            vec![
                part("rustcraft:block/worker_grab", vec![]),
                part("rustcraft:block/worker_grab_left", vec![Op::Move([0.0, 0.0, s])]),
                part("rustcraft:block/worker_grab_right", vec![Op::Move([0.0, 0.0, -s])]),
            ]
        }
        Tool::Bore => vec![
            part("rustcraft:block/worker_bore", vec![]),
            part(
                "rustcraft:block/worker_bore_bit",
                vec![turn("bore_bit", 0, if busy { t * 16.0 } else { 0.0 })],
            ),
        ],
        Tool::Shooter => {
            let yaw = turn("aim_yaw_gun", 1, yaw_to("aim_yaw_gun"));
            let pitch = aim.map_or(0.0, |a| {
                let p = pivot("aim_pitch_gun");
                let flat = (a[0] - p[0]).hypot(a[2] - p[2]);
                (a[1] - p[1]).atan2(flat).clamp(PITCH_DOWN, PITCH_UP)
            });
            let recoil = if busy {
                -0.8 / 16.0 * (t * 9.0).sin().max(0.0)
            } else {
                0.0
            };
            vec![
                part("rustcraft:block/worker_gun", vec![]),
                part("rustcraft:block/worker_aim_yaw_gun", vec![yaw]),
                part(
                    "rustcraft:block/worker_aim_pitch_gun",
                    vec![yaw, turn("aim_pitch_gun", 2, pitch), Op::Move([recoil, 0.0, 0.0])],
                ),
            ]
        }
        Tool::Scoop => {
            let yaw = turn("aim_yaw_scoop", 1, yaw_to("aim_yaw_scoop"));
            let slide = Op::Move([reach("reach_scoop"), 0.0, 0.0]);
            let curl = turn("scoop_bucket", 2, if busy { 0.7 * pulse(t, 3.0) } else { 0.0 });
            vec![
                part("rustcraft:block/worker_scoop", vec![]),
                part("rustcraft:block/worker_aim_yaw_scoop", vec![yaw]),
                part("rustcraft:block/worker_reach_scoop", vec![yaw, slide]),
                part("rustcraft:block/worker_scoop_bucket", vec![yaw, slide, curl]),
            ]
        }
        Tool::Map => vec![
            part("rustcraft:block/worker_map", vec![]),
            part(
                "rustcraft:block/worker_map_drum",
                vec![turn("map_drum", 1, if busy { t * 2.5 } else { 0.0 })],
            ),
        ],
    }
}

/// The bracket model for a rail hung on `mount`.
pub fn bracket(mount: Face) -> &'static str {
    match mount {
        Face::YNeg => "rustcraft:block/worker_below",
        Face::YPos => "rustcraft:block/worker_above",
        _ => "rustcraft:block/worker_side",
    }
}

/// A world direction (x, z) in the model's frame of a worker heading `yaw` (the base game's: 0
/// south, 90 west): (forward, right).
pub fn to_model(d: [f64; 2], yaw: f64) -> [f64; 2] {
    let (s, c) = yaw.to_radians().sin_cos();
    let forward = [-s, c];
    let right = [-c, -s];
    [
        d[0] * forward[0] + d[1] * forward[1],
        d[0] * right[0] + d[1] * right[1],
    ]
}

/// A point `rel` blocks from the worker's feet (world axes) in the model's frame.
pub fn point_in_model(rel: [f64; 3], yaw: f64) -> [f64; 3] {
    let [x, z] = to_model([rel[0], rel[2]], yaw);
    [x + 0.5, rel[1], z + 0.5]
}

/// The turn about the upright, degrees, that points the side bracket (which clamps toward the
/// model's -z) at the wall of a wall rail hung on `mount`.
pub fn bracket_turn(mount: Face, yaw: f64) -> f64 {
    let o = mount.offset();
    let [wx, wz] = to_model([f64::from(o[0]), f64::from(o[2])], yaw);
    (-wx).atan2(-wz).to_degrees()
}

/// The heading a worker riding straight up or down a wall rail keeps: away from the wall, so its
/// tool looks into its zone.
pub fn off_the_wall(mount: Face) -> Option<f32> {
    let o = mount.opposite().offset();
    let (dx, dz) = (f64::from(o[0]), f64::from(o[2]));
    (dx.hypot(dz) > 0.0).then(|| (-dx).atan2(dz).to_degrees() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn the_model_frame_follows_the_heading() {
        // Heading south (yaw 0): forward is +z, the model's right (its +z) is -x.
        assert_eq!(to_model([0.0, 1.0], 0.0).map(|v| v.round()), [1.0, 0.0]);
        assert_eq!(to_model([-1.0, 0.0], 0.0).map(|v| v.round()), [0.0, 1.0]);
        // Heading west (yaw 90): forward is -x.
        assert_eq!(to_model([-1.0, 0.0], 90.0).map(|v| v.round()), [1.0, 0.0]);
        let p = point_in_model([0.0, 0.3, 2.0], 0.0);
        assert!(close(p[0], 2.5) && close(p[1], 0.3) && close(p[2], 0.5));
    }

    #[test]
    fn the_side_bracket_turns_to_the_wall() {
        // Heading south, a wall to the east (+x) is on the model's left, its -z: no turn.
        assert!(close(bracket_turn(Face::XPos, 0.0), 0.0));
        // A wall behind (north, -z): the bracket turns to the model's -x.
        let back = bracket_turn(Face::ZNeg, 0.0);
        assert!(close(back.abs(), 90.0), "{back}");
        assert_eq!(bracket(Face::YPos), "rustcraft:block/worker_above");
        // Riding up a wall to the west it faces east, away from it.
        assert_eq!(off_the_wall(Face::XNeg).map(|y| y.round()), Some(-90.0));
        assert_eq!(off_the_wall(Face::YNeg), None);
    }

    #[test]
    fn a_working_saw_turns_to_its_tree_reaches_and_spins() {
        // A tree straight to the model's right (+z).
        let parts = tool_parts(Tool::Saw, true, 1.0, Some([0.90625, 0.5, 3.0]));
        let disc = parts.iter().find(|p| p.model.ends_with("saw_disc")).unwrap();
        let Op::Turn { axis: 1, deg, .. } = disc.ops[0] else { panic!("{disc:?}") };
        assert!(close(deg, -90.0), "turns right: {deg}");
        assert_eq!(disc.ops[1], Op::Move([REACH, 0.0, 0.0]));
        let Op::Turn { axis: 2, deg, .. } = disc.ops[2] else { panic!() };
        assert!(close(deg, 14f64.to_degrees()));
        // At rest: no turn, no reach, no spin.
        let rest = tool_parts(Tool::Saw, false, 1.0, None);
        for p in rest {
            for op in p.ops {
                match op {
                    Op::Turn { deg, .. } => assert!(close(deg, 0.0)),
                    Op::Move(v) => assert_eq!(v, [0.0; 3]),
                }
            }
        }
    }

    #[test]
    fn every_tool_has_parts_and_its_colour() {
        for t in Tool::ALL {
            assert!(!tool_parts(t, true, 0.5, Some([2.0, 0.5, 0.5])).is_empty());
            assert_ne!(colour(Some(t)), colour(None));
        }
        // The gun's barrel never tilts past the guide's limits.
        let gun = tool_parts(Tool::Shooter, false, 0.0, Some([0.75, 9.0, 0.5]));
        let Op::Turn { deg, .. } = gun[2].ops[1] else { panic!() };
        assert!(close(deg, PITCH_UP.to_degrees()));
    }
}

/// Wave 2 (poses.js `P.worker`: `glow: busy ? 0.2 + 0.2·pulse(t, 8) : 0.1 + 0.08·pulse(t, 2)`):
/// the worker's glow at `t` seconds, 0 to 0.4.
pub fn glow(busy: bool, t: f64) -> f64 {
    if busy {
        0.2 + 0.2 * pulse(t, 8.0)
    } else {
        0.1 + 0.08 * pulse(t, 2.0)
    }
}

/// The packed light (16 × sky light in bits 0..8, 16 × block light in bits 8..16) a worker draws
/// with at `t`: the glow lifts its block light, up to full at the busy pulse's top, never below
/// the light it stands in. An idle worker's faint glow lifts it to at most a third.
pub fn glow_light(light: u32, busy: bool, t: f64) -> u32 {
    let level = (glow(busy, t) / 0.4 * 15.0).round().clamp(0.0, 15.0) as u32;
    let block = ((light >> 8) & 0xFF) / 16;
    let lifted = block.max(level);
    (light & !0xFF00) | ((lifted * 16) << 8)
}

#[cfg(test)]
mod wave2_glow_tests {
    use super::*;

    #[test]
    fn a_working_worker_glows_and_an_idle_one_barely() {
        let dark = 0x00F0; // sky 15, block 0
        let top = std::f64::consts::FRAC_PI_2 / 8.0; // pulse(t, 8) = 1
        assert_eq!(glow_light(dark, true, top), 0xF0F0, "full block light at the pulse's top");
        let idle = glow_light(dark, false, 0.0);
        assert!((idle >> 8) / 16 <= 7, "idle stays dim: {idle:#x}");
        assert_eq!(glow_light(0xF0F0, false, 0.0), 0xF0F0, "never darker than where it stands");
        assert_eq!(glow_light(dark, true, top) & 0xFF, 0xF0, "sky light kept");
    }
}
