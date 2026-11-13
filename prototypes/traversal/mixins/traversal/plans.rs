// Game-free plans of the traversal prototype: the harpoon's pull and climb, the zipline and the
// ride along it, and the steering each method uses to follow a plan. Numbers are in trv_rules.
use crate::game::trv_rules::{
    CLIMB_SPEED, EASE_TICKS, EDGE_REACH, GATE_GAIN, GATE_MARGIN, GATE_MAX_TICKS, HANG_GAP,
    HANG_HANDS, HANG_TICKS, HOOK_RANGE, HOOK_SPEED, Hit, Kind, LIFT_CLEARANCE, PLAYER_HALF_WIDTH,
    PLAYER_HEIGHT, Path, RIDE_REACH, ROPE_HEIGHT, SEAT_RISE_MAX, STEP_MAX, Schedule, V3,
    VELOCITY_GAIN, ZIP_MIN_RISE, ZIP_RANGE, ZIP_SPEED, add, check_reach, distance, face_of, scale,
    sub, up,
};
use serde::{Deserialize, Serialize};

/// The middle of the block's top: where a player who climbs onto it stands.
fn top_centre(block: [i32; 3]) -> V3 {
    [
        f64::from(block[0]) + 0.5,
        f64::from(block[1]) + 1.0,
        f64::from(block[2]) + 0.5,
    ]
}

/// Where a mover from `from` crosses onto the block whose top centre is `stand`, or leaves it
/// toward `from`: EDGE_REACH out from the centre toward `from` and LIFT_CLEARANCE over the top.
/// None when `from` is about straight above or below, with no edge to cross.
fn edge_point(stand: V3, from: V3) -> Option<V3> {
    let (dx, dz) = (from[0] - stand[0], from[2] - stand[2]);
    let big = dx.abs().max(dz.abs());
    if big <= EDGE_REACH {
        return None;
    }
    let k = EDGE_REACH / big;
    Some([
        stand[0] + dx * k,
        stand[1] + LIFT_CLEARANCE,
        stand[2] + dz * k,
    ])
}

/// Each component clamped to ±`max`; whether any was cut.
fn clamp_each(v: V3, max: f64) -> (V3, bool) {
    let cut = v.map(|c| c.clamp(-max, max));
    (cut, cut != v)
}

/// A zipline: where a rider stands at each end. The rope hangs ROPE_HEIGHT over both.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rope {
    pub low: V3,
    pub high: V3,
}

impl Rope {
    pub fn anchors(&self) -> [V3; 2] {
        [up(self.low, ROPE_HEIGHT), up(self.high, ROPE_HEIGHT)]
    }

    /// How far `feet` are from the nearer end.
    pub fn end_gap(&self, feet: V3) -> f64 {
        distance(feet, self.low).min(distance(feet, self.high))
    }
}

/// The point over a block's edge where the plan turns from rising to crossing the top. Method
/// d holds the plan there until the feet are seen over the top.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    /// The plan's tick at the point.
    pub tick: u32,
    pub point: V3,
    /// The feet must be seen at least this high to go on.
    pub min_y: f64,
}

/// One planned move: the path of the feet, its legs in order, and the ticks held at the end.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub kind: Kind,
    pub path: Path,
    pub legs: Vec<Schedule>,
    pub hold_ticks: u32,
    pub climb: bool,
    pub hook: Option<V3>,
    pub rope: Option<Rope>,
    #[serde(default)]
    pub gate: Option<Gate>,
}

impl Plan {
    /// A plan whose legs follow groups of points, each at its own speed.
    fn from_legs(kind: Kind, legs: &[(&[V3], f64)], hold_ticks: u32) -> Result<Plan, String> {
        let mut points: Vec<V3> = Vec::new();
        let mut schedules = Vec::new();
        for (group, speed) in legs {
            let leg = Path::new(group)?;
            schedules.push(Schedule::new(leg.length(), *speed, EASE_TICKS));
            points.extend_from_slice(group);
        }
        Ok(Plan {
            kind,
            path: Path::new(&points)?,
            legs: schedules,
            hold_ticks,
            climb: false,
            hook: None,
            rope: None,
            gate: None,
        })
    }

    /// The first tick at which the plan has gone `along` blocks.
    fn tick_reaching(&self, along: f64) -> u32 {
        (0..=self.move_ticks())
            .find(|&t| self.progress_at(t) >= along - 1e-9)
            .unwrap_or(self.move_ticks())
    }

    /// A gate at `point`, `along` blocks into the path, over a block whose top is at `top`.
    fn with_gate(mut self, along: f64, point: V3, top: f64) -> Plan {
        self.gate = Some(Gate {
            tick: self.tick_reaching(along),
            point,
            min_y: top + GATE_MARGIN,
        });
        self
    }

    pub fn move_ticks(&self) -> u32 {
        self.legs.iter().map(|leg| leg.ticks).sum()
    }

    pub fn total_ticks(&self) -> u32 {
        self.move_ticks() + self.hold_ticks
    }

    pub fn length(&self) -> f64 {
        self.path.length()
    }

    /// Blocks along the path after `tick` ticks.
    pub fn progress_at(&self, tick: u32) -> f64 {
        let mut done = 0.0;
        let mut left = tick;
        for leg in &self.legs {
            if left <= leg.ticks {
                return done + leg.distance_at(left);
            }
            done += leg.length;
            left -= leg.ticks;
        }
        done
    }

    /// Where the feet should be after `tick` ticks.
    pub fn target_at(&self, tick: u32) -> V3 {
        self.path.at(self.progress_at(tick))
    }

    pub fn peak_step(&self) -> f64 {
        self.legs
            .iter()
            .map(Schedule::peak_step)
            .fold(0.0, f64::max)
    }

    /// The planned step into `tick`, and whether that tick is in a leg's steady cruise.
    pub fn planned_step(&self, tick: u32) -> (f64, bool) {
        let step = self.progress_at(tick) - self.progress_at(tick.saturating_sub(1));
        let mut left = tick;
        for leg in &self.legs {
            if left <= leg.ticks {
                return (step, leg.cruising(left));
            }
            left -= leg.ticks;
        }
        (step, false)
    }
}

/// The harpoon: pulled to the face hit, then over the edge when two blocks above are free,
/// else held at the face for HANG_TICKS. A top face is climbed straight onto.
pub fn plan_hook(feet: V3, hit: &Hit, room_above: bool) -> Result<Plan, String> {
    check_reach(feet, hit, HOOK_RANGE)?;
    let normal = face_of(hit);
    let stand = top_centre(hit.block);
    let top = stand[1];
    let p = hit.point;
    let mut plan = if normal == [0, 1, 0] {
        if !room_above {
            return Err("Над блоком нет двух свободных блоков: встать некуда".to_owned());
        }
        // From below, the feet come up past the edge first: a straight line would cut it.
        let edge = edge_point(stand, feet).filter(|_| feet[1] < top);
        let mut plan = match edge {
            Some(edge) => Plan::from_legs(
                Kind::Hook,
                &[(&[feet, edge], HOOK_SPEED), (&[edge, stand], CLIMB_SPEED)],
                0,
            )?
            .with_gate(distance(feet, edge), edge, top),
            None => Plan::from_legs(Kind::Hook, &[(&[feet, stand], HOOK_SPEED)], 0)?,
        };
        plan.climb = true;
        plan
    } else if normal[1] == -1 {
        let hang = [p[0], p[1] - PLAYER_HEIGHT - HANG_GAP, p[2]];
        Plan::from_legs(Kind::Hook, &[(&[feet, hang], HOOK_SPEED)], HANG_TICKS)?
    } else {
        let off = PLAYER_HALF_WIDTH + HANG_GAP;
        let hang = [
            p[0] + f64::from(normal[0]) * off,
            p[1] - HANG_HANDS,
            p[2] + f64::from(normal[2]) * off,
        ];
        if room_above {
            let lift = [hang[0], top + LIFT_CLEARANCE, hang[2]];
            // The climb eases to a stop at the lift point, where method d's gate is, and
            // then steps over the edge.
            let legs: &[(&[V3], f64)] = &[
                (&[feet, hang], HOOK_SPEED),
                (&[hang, lift], CLIMB_SPEED),
                (&[lift, stand], CLIMB_SPEED),
            ];
            let mut plan = Plan::from_legs(Kind::Hook, legs, 0)?.with_gate(
                distance(feet, hang) + distance(hang, lift),
                lift,
                top,
            );
            plan.climb = true;
            plan
        } else {
            Plan::from_legs(Kind::Hook, &[(&[feet, hang], HOOK_SPEED)], HANG_TICKS)?
        }
    };
    plan.hook = Some(p);
    Ok(plan)
}

/// The zipline: a rope from the feet to the top of the block aimed at, ridden up at once.
pub fn plan_zip(feet: V3, hit: &Hit, room_above: bool) -> Result<Plan, String> {
    check_reach(feet, hit, ZIP_RANGE)?;
    if !room_above {
        return Err(
            "Над целевым блоком нет двух свободных блоков: верхний якорь не встанет".to_owned(),
        );
    }
    let high = top_centre(hit.block);
    if high[1] < feet[1] + ZIP_MIN_RISE {
        return Err(format!(
            "Цельтесь в блок выше себя: верх якоря должен быть хотя бы на {ZIP_MIN_RISE:.0} блок выше ног"
        ));
    }
    let mut plan = ride_up(feet, &[feet], high)?;
    plan.kind = Kind::Zip;
    plan.rope = Some(Rope { low: feet, high });
    Ok(plan)
}

/// Up a rope through `start` (the feet first) to `high`, a block's top: along the rope to its
/// edge, then over it onto the top, gated.
fn ride_up(feet: V3, start: &[V3], high: V3) -> Result<Plan, String> {
    let last = *start.last().unwrap_or(&feet);
    let Some(edge) = edge_point(high, last) else {
        let mut points = start.to_vec();
        points.push(high);
        return Plan::from_legs(Kind::Ride, &[(&points, ZIP_SPEED)], 0);
    };
    let mut points = start.to_vec();
    points.push(edge);
    let along = Path::new(&points).map_or(0.0, |p| p.length());
    Ok(Plan::from_legs(
        Kind::Ride,
        &[(&points, ZIP_SPEED), (&[edge, high], CLIMB_SPEED)],
        0,
    )?
    .with_gate(along, edge, high[1]))
}

/// A ride on `rope` from its nearer end to the other.
pub fn plan_ride(feet: V3, rope: &Rope) -> Result<Plan, String> {
    let (near, far) = if distance(feet, rope.low) <= distance(feet, rope.high) {
        (rope.low, rope.high)
    } else {
        (rope.high, rope.low)
    };
    let gap = distance(feet, near);
    if gap > RIDE_REACH {
        return Err(format!(
            "До ближнего конца троса {gap:.1} блока: подойдите ближе {RIDE_REACH:.0}"
        ));
    }
    let mut plan = if near == rope.low {
        ride_up(feet, &[feet, near], far)?
    } else if let Some(edge) = edge_point(near, far) {
        // Down: over the edge first, then along the rope; the top is under the feet, no gate.
        Plan::from_legs(
            Kind::Ride,
            &[
                (&[feet, near, edge], CLIMB_SPEED),
                (&[edge, far], ZIP_SPEED),
            ],
            0,
        )?
    } else {
        Plan::from_legs(Kind::Ride, &[(&[feet, near, far], ZIP_SPEED)], 0)?
    };
    plan.rope = Some(*rope);
    Ok(plan)
}

// ---- Steering ----

/// Method a: the push that takes the client from `position`, moving at `velocity` blocks a
/// tick, to `next` in one tick; whether a component was clamped.
pub fn push_toward(next: V3, position: V3, velocity: V3) -> (V3, bool) {
    clamp_each(sub(sub(next, position), velocity), STEP_MAX)
}

/// Method d: the velocity the server sends: the plan's step plus a share of the error between
/// where the server sees the player and where the plan had them then.
pub fn velocity_toward(step: V3, seen_target: V3, seen: V3) -> V3 {
    clamp_each(
        add(step, scale(sub(seen_target, seen), VELOCITY_GAIN)),
        STEP_MAX,
    )
    .0
}

/// Method d held at a gate: the velocity that steers the feet seen at `seen` to `point`.
pub fn gate_velocity(point: V3, seen: V3) -> V3 {
    clamp_each(scale(sub(point, seen), GATE_GAIN), STEP_MAX).0
}

/// Method d: whether the plan waits at its gate this tick, the plan being at `tick`, the feet
/// seen at `seen` and the gate held `waited` ticks so far.
pub fn gate_holds(gate: &Gate, tick: u32, seen: V3, waited: u32) -> bool {
    gate.tick == tick && seen[1] < gate.min_y && waited < GATE_MAX_TICKS
}

/// Method b: where the seat goes so its rider's feet are at `feet`.
pub fn seat_spot(feet: V3, rise: f64) -> V3 {
    up(feet, -rise)
}

pub fn plausible_rise(rise: f64) -> bool {
    rise.is_finite() && rise > 0.0 && rise < SEAT_RISE_MAX
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::trv_rules::{SEAT_RIDER_RISE, TICKS_PER_SECOND};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn close3(a: V3, b: V3) -> bool {
        distance(a, b) < 1e-6
    }

    fn hit_side(feet: V3) -> (V3, Hit) {
        // The west face of a block 15 blocks east, hit 1.2 below its top.
        (
            feet,
            Hit {
                block: [15, 70, 0],
                point: [15.0, 70.8, 0.5],
            },
        )
    }

    #[test]
    fn a_harpoon_with_room_above_pulls_to_the_face_then_climbs_on_top() {
        let (feet, hit) = hit_side([0.5, 64.0, 0.5]);
        let plan = plan_hook(feet, &hit, true).unwrap();
        assert!(plan.climb);
        assert_eq!(plan.hold_ticks, 0);
        assert_eq!(plan.legs.len(), 3);
        let points = plan.path.points();
        let hang = points[1];
        assert!(close(hang[0], 15.0 - PLAYER_HALF_WIDTH - HANG_GAP));
        assert!(close(hang[1], 70.8 - HANG_HANDS));
        assert!(close3(points[2], [hang[0], 71.0 + LIFT_CLEARANCE, 0.5]));
        assert!(close3(plan.path.end(), [15.5, 71.0, 0.5]));
        assert!(close3(plan.target_at(0), feet));
        assert!(close3(plan.target_at(plan.total_ticks()), plan.path.end()));
        // The pull ends at the face before the climb starts.
        assert!(close3(plan.target_at(plan.legs[0].ticks), hang));
        assert_eq!(plan.hook, Some(hit.point));
    }

    #[test]
    fn a_harpoon_without_room_above_hangs_at_the_face() {
        let (feet, hit) = hit_side([0.5, 64.0, 0.5]);
        let plan = plan_hook(feet, &hit, false).unwrap();
        assert!(!plan.climb);
        assert_eq!(plan.hold_ticks, HANG_TICKS);
        assert_eq!(plan.total_ticks(), plan.move_ticks() + HANG_TICKS);
        let end = plan.path.end();
        assert!(close3(plan.target_at(plan.total_ticks()), end));
        assert!(close3(plan.target_at(plan.move_ticks() + 5), end));
        // Hanging: the box stays within the 1/16 the floating check looks around it.
        assert!(15.0 - (end[0] + PLAYER_HALF_WIDTH) < 0.0625);
    }

    #[test]
    fn a_harpoon_on_a_top_face_lands_on_it_and_one_from_below_hangs_under_it() {
        let feet = [0.5, 80.0, 0.5];
        let top = Hit {
            block: [8, 70, 0],
            point: [8.4, 71.0, 0.6],
        };
        let plan = plan_hook(feet, &top, true).unwrap();
        assert!(plan.climb && plan.legs.len() == 1);
        assert!(close3(plan.path.end(), [8.5, 71.0, 0.5]));
        assert!(plan_hook(feet, &top, false).is_err());
        let under = Hit {
            block: [0, 75, 0],
            point: [0.5, 75.0, 0.5],
        };
        let plan = plan_hook([0.5, 64.0, 0.5], &under, true).unwrap();
        assert!(!plan.climb);
        assert!(close(plan.path.end()[1], 75.0 - PLAYER_HEIGHT - HANG_GAP));
    }

    #[test]
    fn a_harpoon_refuses_a_block_out_of_range() {
        let far = Hit {
            block: [30, 64, 0],
            point: [30.0, 64.5, 0.5],
        };
        let refusal = plan_hook([0.5, 64.0, 0.5], &far, true).unwrap_err();
        assert!(refusal.contains("не дальше 20"), "{refusal}");
    }

    #[test]
    fn the_numbers_of_a_full_harpoon_pull() {
        // A 20-block pull straight up a wall's face takes about 1.35 s, and its fastest step
        // stays under the push's clamp.
        let feet = [0.5, 64.0, 0.5];
        let hit = Hit {
            block: [1, 85, 0],
            point: [1.0, 85.5, 0.5],
        };
        let plan = plan_hook(feet, &hit, false).unwrap();
        let seconds = f64::from(plan.move_ticks()) / TICKS_PER_SECOND;
        assert!((1.0..=1.5).contains(&seconds), "{seconds}");
        assert!(plan.peak_step() < STEP_MAX);
    }

    #[test]
    fn a_zipline_runs_from_the_feet_to_the_top_of_a_higher_block() {
        let feet = [0.3, 64.0, 0.7];
        let hit = Hit {
            block: [20, 72, 4],
            point: [20.0, 72.5, 4.5],
        };
        let plan = plan_zip(feet, &hit, true).unwrap();
        let rope = plan.rope.unwrap();
        assert!(close3(rope.low, feet));
        assert!(close3(rope.high, [20.5, 73.0, 4.5]));
        assert!(close3(rope.anchors()[1], [20.5, 73.0 + ROPE_HEIGHT, 4.5]));
        assert!(close3(plan.path.end(), rope.high));
        let speed = plan.peak_step() * TICKS_PER_SECOND;
        assert!(
            speed <= ZIP_SPEED + 1e-9 && speed > ZIP_SPEED * 0.95,
            "{speed}"
        );
    }

    #[test]
    fn a_zipline_refuses_a_low_block_and_one_with_no_room() {
        let feet = [0.5, 64.0, 0.5];
        let low = Hit {
            block: [10, 63, 0],
            point: [10.0, 63.5, 0.5],
        };
        assert!(
            plan_zip(feet, &low, true)
                .unwrap_err()
                .contains("выше себя")
        );
        let high = Hit {
            block: [10, 70, 0],
            point: [10.0, 70.5, 0.5],
        };
        assert!(plan_zip(feet, &high, false).is_err());
        let far = Hit {
            block: [40, 70, 0],
            point: [40.0, 70.5, 0.5],
        };
        assert!(plan_zip(feet, &far, true).is_err());
    }

    #[test]
    fn a_ride_goes_from_the_nearer_end_to_the_other_both_ways() {
        let rope = Rope {
            low: [0.5, 64.0, 0.5],
            high: [20.5, 73.0, 0.5],
        };
        let down = plan_ride([19.8, 73.0, 0.9], &rope).unwrap();
        assert!(close3(down.path.end(), rope.low));
        let up_ride = plan_ride([1.0, 64.0, 1.0], &rope).unwrap();
        assert!(close3(up_ride.path.end(), rope.high));
        assert_eq!(up_ride.path.points()[1], rope.low);
        let refusal = plan_ride([10.0, 64.0, 10.0], &rope).unwrap_err();
        assert!(refusal.contains("подойдите ближе"), "{refusal}");
        assert!(close(rope.end_gap([0.5, 66.0, 0.5]), 2.0));
    }

    #[test]
    fn the_planned_step_knows_the_cruise() {
        let rope = Rope {
            low: [0.0; 3],
            high: [16.0, 0.0, 0.0],
        };
        let plan = plan_ride([0.0; 3], &rope).unwrap();
        let (first, cruising) = plan.planned_step(1);
        assert!(!cruising && first < plan.peak_step());
        let (middle, cruising) = plan.planned_step(plan.move_ticks() / 2);
        assert!(cruising && close(middle, plan.peak_step()));
        assert_eq!(plan.planned_step(plan.move_ticks() + 3), (0.0, false));
    }

    #[test]
    fn the_push_cancels_the_velocity_and_is_clamped() {
        let (push, cut) = push_toward([1.0, 0.0, 0.0], [0.0; 3], [0.2, -0.08, 0.0]);
        assert!(close3(push, [0.8, 0.08, 0.0]) && !cut);
        let (push, cut) = push_toward([10.0, 0.0, 0.0], [0.0; 3], [0.0; 3]);
        assert!(close3(push, [STEP_MAX, 0.0, 0.0]) && cut);
        let (stop, _) = push_toward([5.0; 3], [5.0; 3], [0.3, 0.1, -0.2]);
        assert!(close3(stop, [-0.3, -0.1, 0.2]));
    }

    #[test]
    fn the_server_velocity_adds_a_share_of_the_lag() {
        let v = velocity_toward([0.4, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        assert!(close3(v, [0.4 + VELOCITY_GAIN, 0.0, 0.0]));
        assert!(close3(
            velocity_toward([9.0, 0.0, 0.0], [0.0; 3], [0.0; 3]),
            [STEP_MAX, 0.0, 0.0]
        ));
    }

    /// Whether the player's box with its feet at `feet` cuts into the column of blocks from
    /// `block` down: a mover that does is stopped by the face.
    fn cuts_column(feet: V3, block: [i32; 3]) -> bool {
        let e = 1e-6;
        let (bx, by, bz) = (
            f64::from(block[0]),
            f64::from(block[1]),
            f64::from(block[2]),
        );
        feet[0] + PLAYER_HALF_WIDTH > bx + e
            && feet[0] - PLAYER_HALF_WIDTH < bx + 1.0 - e
            && feet[2] + PLAYER_HALF_WIDTH > bz + e
            && feet[2] - PLAYER_HALF_WIDTH < bz + 1.0 - e
            && feet[1] < by + 1.0 - e
    }

    fn never_cuts(plan: &Plan, block: [i32; 3]) -> bool {
        let steps = (plan.length() / 0.02) as u32;
        (0..=steps).all(|i| !cuts_column(plan.path.at(f64::from(i) * 0.02), block))
    }

    #[test]
    fn a_harpoon_climb_and_a_zipline_never_cut_the_block_they_reach() {
        let (feet, hit) = hit_side([0.5, 64.0, 0.5]);
        assert!(never_cuts(&plan_hook(feet, &hit, true).unwrap(), hit.block));
        let tower = Hit {
            block: [20, 72, 4],
            point: [20.0, 72.5, 4.5],
        };
        for feet in [[0.3, 64.0, 0.7], [20.5, 64.0, -10.0], [30.0, 66.0, 14.0]] {
            let zip = plan_zip(feet, &tower, true).unwrap();
            assert!(never_cuts(&zip, tower.block), "{feet:?}");
            let rope = zip.rope.unwrap();
            assert!(never_cuts(
                &plan_ride(rope.low, &rope).unwrap(),
                tower.block
            ));
            assert!(never_cuts(
                &plan_ride(rope.high, &rope).unwrap(),
                tower.block
            ));
        }
        // A harpoon shot from below at a top face comes up past the edge too.
        let top = Hit {
            block: [8, 70, 0],
            point: [8.4, 71.0, 0.6],
        };
        assert!(never_cuts(
            &plan_hook([0.5, 64.0, 0.5], &top, true).unwrap(),
            top.block
        ));
    }

    #[test]
    fn a_climb_waits_at_the_edge_until_the_feet_are_over_the_top() {
        let (feet, hit) = hit_side([0.5, 64.0, 0.5]);
        let plan = plan_hook(feet, &hit, true).unwrap();
        let gate = plan.gate.unwrap();
        assert!(close3(gate.point, plan.path.points()[2]));
        assert!(close3(plan.target_at(gate.tick), gate.point));
        assert!(close(gate.min_y, 71.0 + GATE_MARGIN));
        let under = [gate.point[0], 70.9, gate.point[2]];
        assert!(gate_holds(&gate, gate.tick, under, 0));
        assert!(!gate_holds(&gate, gate.tick, under, GATE_MAX_TICKS));
        assert!(!gate_holds(&gate, gate.tick, gate.point, 0));
        assert!(!gate_holds(&gate, gate.tick + 1, under, 0));
        let lift = gate_velocity(gate.point, under);
        assert!(lift[1] > 0.0 && close(lift[0], 0.0));
        // A hanging harpoon has no edge to wait at; a zipline up does, a ride down does not.
        assert!(plan_hook(feet, &hit, false).unwrap().gate.is_none());
        let tower = Hit {
            block: [20, 72, 4],
            point: [20.0, 72.5, 4.5],
        };
        let zip = plan_zip([0.3, 64.0, 0.7], &tower, true).unwrap();
        assert!(zip.gate.is_some());
        let rope = zip.rope.unwrap();
        assert!(plan_ride(rope.high, &rope).unwrap().gate.is_none());
        assert!(plan_ride(rope.low, &rope).unwrap().gate.is_some());
    }

    #[test]
    fn the_seat_sits_under_the_feet_and_only_believable_rises_count() {
        assert!(close3(seat_spot([1.0, 70.0, 1.0], 0.5), [1.0, 69.5, 1.0]));
        assert!(plausible_rise(SEAT_RIDER_RISE));
        assert!(!plausible_rise(-0.1) && !plausible_rise(3.0) && !plausible_rise(f64::NAN));
    }
}
