//! The way a capsule's passenger goes along a trip's rails: a line of points for the feet
//! (capsule::feet_at for each rail), through the middle of each rail cell, round a corner on the
//! quarter circle the rail draws, up a climb along the 45° piece; and when it is where, with the
//! speed of each rail's tier (T4) and a short start and stop.

use super::{Network, RailSpec, capsule};
use crate::geom::{Face, Pos};

/// A point of the way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub at: [f64; 3],
    /// The mount of the rail the capsule holds from here to the next point.
    pub mount: Face,
    /// Blocks a second from here to the next point.
    pub speed: f64,
}

/// A trip's way and its timing.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<Point>,
    /// Seconds from the start to each point at full speed.
    pub times: Vec<f64>,
}

/// Ticks the capsule takes to reach full speed, and to stop.
pub const EASE_TICKS: u32 = 4;
/// Points on a corner's quarter circle between its two ends.
const ARC_STEPS: u32 = 4;

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn unit(a: Pos, b: Pos) -> [f64; 3] {
    [
        f64::from(b[0] - a[0]),
        f64::from(b[1] - a[1]),
        f64::from(b[2] - a[2]),
    ]
}

/// How a step from one rail to the next goes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    /// Through a shared face.
    Face,
    /// Up or down a climb; `slope` is the cell holding the climbing piece, `dir` the horizontal
    /// step from the lower cell to the upper.
    Climb { slope: Pos, dir: [i32; 3] },
}

fn step_kind(net: &Network, a: Pos, b: Pos) -> Step {
    if (0..3).map(|i| (b[i] - a[i]).abs()).sum::<i32>() == 1 {
        return Step::Face;
    }
    let (low, high) = if a[1] < b[1] { (a, b) } else { (b, a) };
    let dir = [high[0] - low[0], 0, high[2] - low[2]];
    let ceiling = net.rails.get(&a).is_some_and(|r| r.mount == Face::YPos);
    Step::Climb {
        slope: if ceiling { high } else { low },
        dir,
    }
}

/// The feet at the edge of `cell` toward `dir` (a horizontal unit step), at the climbing piece's
/// end on that side: its floor end for a floor climb, its ceiling end for a ceiling one.
fn climb_end(net: &Network, slope: Pos, dir_low_to_high: [i32; 3], toward: [i32; 3]) -> [f64; 3] {
    let mount = net.rails[&slope].mount;
    let base = capsule::feet_at(slope, mount);
    let up_side = toward == dir_low_to_high;
    let dy = match (mount, up_side) {
        // A floor piece rises from its cell's floor to the floor of the cell above the next.
        (Face::YPos, true) => 0.0,
        (Face::YPos, false) => -1.0,
        (_, true) => 1.0,
        (_, false) => 0.0,
    };
    add(
        base,
        [0.5 * f64::from(toward[0]), dy, 0.5 * f64::from(toward[2])],
    )
}

/// The feet where the capsule passes from `a` to `b`; `before` is the step into `a`, `after`
/// the step out of `b`.
fn joint(
    net: &Network,
    a: Pos,
    b: Pos,
    before: Option<Step>,
    this: Step,
    after: Option<Step>,
) -> [f64; 3] {
    if let Step::Climb { slope, dir } = this {
        let other = if slope == a { b } else { a };
        return climb_end(net, slope, dir, across(slope, other));
    }
    // A face shared with a climbing piece: that piece's other end.
    for (cell, other, near) in [(a, b, before), (b, a, after)] {
        if let Some(Step::Climb { slope, dir }) = near
            && slope == cell
        {
            return climb_end(net, cell, dir, across(cell, other));
        }
    }
    let pa = capsule::feet_at(a, net.rails[&a].mount);
    let pb = capsule::feet_at(b, net.rails[&b].mount);
    scale(add(pa, pb), 0.5)
}

/// The horizontal unit step from `a` toward `b`.
fn across(a: Pos, b: Pos) -> [i32; 3] {
    [(b[0] - a[0]).signum(), 0, (b[2] - a[2]).signum()]
}

/// Whether `cell` holds the piece of a climb its path steps along.
fn is_slope(steps: &[Step], i: usize, cell: Pos) -> bool {
    let at = |j: usize| steps.get(j).copied();
    [i.checked_sub(1).and_then(at), at(i)]
        .into_iter()
        .flatten()
        .any(|s| matches!(s, Step::Climb { slope, .. } if slope == cell))
}

/// The way along `path`, a trip's rails from start to end.
pub fn route(net: &Network, spec: &RailSpec, path: &[Pos]) -> Route {
    let rail = |p: &Pos| net.rails[p];
    let steps: Vec<Step> = path
        .windows(2)
        .map(|w| step_kind(net, w[0], w[1]))
        .collect();
    let mut points: Vec<Point> = Vec::new();
    let mut push = |at: [f64; 3], cell: Pos| {
        let r = rail(&cell);
        if points.last().is_some_and(|p| distance(p.at, at) < 1e-9) {
            return;
        }
        points.push(Point {
            at,
            mount: r.mount,
            speed: spec.speed_of(r.tier),
        });
    };
    for (i, &cell) in path.iter().enumerate() {
        let mount = rail(&cell).mount;
        let joints = |i: usize| -> Option<[f64; 3]> {
            let a = *path.get(i)?;
            let b = *path.get(i + 1)?;
            let before = i.checked_sub(1).map(|j| steps[j]);
            Some(joint(
                net,
                a,
                b,
                before,
                steps[i],
                steps.get(i + 1).copied(),
            ))
        };
        let entry = i.checked_sub(1).and_then(joints);
        let exit = joints(i);
        if let Some(e) = entry {
            push(e, cell);
        }
        if is_slope(&steps, i, cell) {
            // The climbing piece is the way from its one end to the other: no middle point.
            if entry.is_none() || exit.is_none() {
                let a = entry.or(exit).expect("a path of one cell has no climb");
                push(scale(add(a, capsule::feet_at(cell, mount)), 0.5), cell);
            }
            continue;
        }
        let centre = capsule::feet_at(cell, mount);
        // A corner: in by one side and out by another at a right angle, both by a face and on the
        // same mount, round the quarter circle about the corner they share.
        if entry.is_some()
            && exit.is_some()
            && i > 0
            && steps[i - 1] == Step::Face
            && steps[i] == Step::Face
        {
            let prev = path[i - 1];
            let next = path[i + 1];
            let din = unit(cell, prev);
            let dout = unit(cell, next);
            let dot: f64 = (0..3).map(|k| din[k] * dout[k]).sum();
            let same = rail(&prev).mount == mount && rail(&next).mount == mount;
            let in_plane = |d: [f64; 3]| {
                let o = mount.offset();
                (0..3).all(|k| f64::from(o[k]) * d[k] == 0.0)
            };
            if dot.abs() < 1e-9 && same && in_plane(din) && in_plane(dout) {
                let corner = add(centre, add(scale(din, 0.5), scale(dout, 0.5)));
                for s in 1..ARC_STEPS {
                    let theta = std::f64::consts::FRAC_PI_2 * f64::from(s) / f64::from(ARC_STEPS);
                    let at = add(
                        corner,
                        scale(add(scale(dout, theta.cos()), scale(din, theta.sin())), -0.5),
                    );
                    push(at, cell);
                }
                continue;
            }
        }
        push(centre, cell);
    }
    let mut times = Vec::with_capacity(points.len());
    let mut t = 0.0;
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            let prev = &points[i - 1];
            t += distance(prev.at, p.at) / prev.speed;
        }
        times.push(t);
    }
    Route { points, times }
}

impl Route {
    /// Seconds from start to end at full speed.
    pub fn seconds(&self) -> f64 {
        self.times.last().copied().unwrap_or(0.0)
    }

    /// Blocks along the way.
    pub fn length(&self) -> f64 {
        self.points
            .windows(2)
            .map(|w| distance(w[0].at, w[1].at))
            .sum()
    }

    pub fn start(&self) -> [f64; 3] {
        self.points.first().map_or([0.0; 3], |p| p.at)
    }

    pub fn end(&self) -> [f64; 3] {
        self.points.last().map_or([0.0; 3], |p| p.at)
    }

    /// Ticks the whole trip takes, the start and the stop included.
    pub fn ticks(&self, ticks_per_second: f64) -> u32 {
        (self.seconds() * ticks_per_second).ceil() as u32 + EASE_TICKS
    }

    /// The feet `tick` ticks after the start.
    pub fn at_tick(&self, tick: u32, ticks_per_second: f64) -> [f64; 3] {
        self.at_time(self.time_at(tick, ticks_per_second))
    }

    /// Seconds of full-speed way done after `tick` ticks: speeding up over EASE_TICKS, a steady
    /// run, slowing down over EASE_TICKS (the traversal prototype's schedule).
    fn time_at(&self, tick: u32, ticks_per_second: f64) -> f64 {
        let total = f64::from(self.ticks(ticks_per_second));
        let length = self.seconds() * ticks_per_second;
        let e = f64::from(EASE_TICKS).min(total / 2.0);
        let t = f64::from(tick).min(total);
        if length <= 0.0 {
            return 0.0;
        }
        let v = length / (total - e);
        let done = if e <= 0.0 {
            length * t / total
        } else if t < e {
            v * t * t / (2.0 * e)
        } else if t <= total - e {
            v * (t - e / 2.0)
        } else {
            length - v * (total - t).powi(2) / (2.0 * e)
        };
        done.clamp(0.0, length) / ticks_per_second
    }

    /// The feet `seconds` of full-speed way from the start, held at both ends.
    pub fn at_time(&self, seconds: f64) -> [f64; 3] {
        let i = self.times.partition_point(|&t| t <= seconds);
        if i == 0 {
            return self.start();
        }
        if i >= self.points.len() {
            return self.end();
        }
        let (a, b) = (&self.points[i - 1], &self.points[i]);
        let span = self.times[i] - self.times[i - 1];
        let f = if span > 0.0 {
            (seconds - self.times[i - 1]) / span
        } else {
            1.0
        };
        add(
            a.at,
            scale([b.at[0] - a.at[0], b.at[1] - a.at[1], b.at[2] - a.at[2]], f),
        )
    }

    /// The points with those in a straight line on one mount left out: what the client needs to
    /// draw the capsule's heading and bracket.
    pub fn corners(&self) -> Vec<Point> {
        let mut out: Vec<Point> = Vec::new();
        for (i, p) in self.points.iter().enumerate() {
            if let (Some(prev), Some(next)) = (out.last(), self.points.get(i + 1))
                && prev.mount == p.mount
                && straight(prev.at, p.at, next.at)
            {
                continue;
            }
            out.push(*p);
        }
        out
    }
}

/// Whether `b` lies on the straight line from `a` to `c`, between them.
fn straight(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> bool {
    (distance(a, b) + distance(b, c) - distance(a, c)).abs() < 1e-6
}

#[cfg(test)]
mod tests {
    use super::super::tests::{floor_rail, fork, rail};
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        distance(a, b) < 1e-6
    }

    #[test]
    fn a_straight_run_goes_through_the_middle_of_each_rail() {
        let net = fork();
        let path: Vec<Pos> = (0..=10).map(|x| [x, 0, 0]).collect();
        let r = route(&net, &RailSpec::default(), &path);
        assert!(close(r.start(), [0.5, 0.05, 0.5]));
        assert!(close(r.end(), [10.5, 0.05, 0.5]));
        assert!((r.length() - 10.0).abs() < 1e-9);
        // 10 blocks at 8 a second.
        assert!((r.seconds() - 1.25).abs() < 1e-9);
        assert_eq!(r.corners().len(), 2);
    }

    #[test]
    fn a_corner_goes_round_a_quarter_circle() {
        let mut net = Network::default();
        floor_rail(&mut net, [0, 0, 0]);
        floor_rail(&mut net, [1, 0, 0]);
        floor_rail(&mut net, [1, 0, 1]);
        let r = route(
            &net,
            &RailSpec::default(),
            &[[0, 0, 0], [1, 0, 0], [1, 0, 1]],
        );
        // No point at the corner cell's middle; every point of the arc 0.5 from the cell corner.
        assert!(!r.points.iter().any(|p| close(p.at, [1.5, 0.05, 0.5])));
        let corner = [1.0, 0.05, 1.0];
        let on_arc: Vec<_> = r.points[1..r.points.len() - 1].iter().collect();
        assert_eq!(on_arc.len(), 5);
        for p in on_arc {
            assert!((distance(p.at, corner) - 0.5).abs() < 1e-9, "{:?}", p.at);
        }
    }

    #[test]
    fn a_climb_runs_along_the_climbing_piece() {
        let mut net = Network::default();
        floor_rail(&mut net, [0, 0, 0]);
        floor_rail(&mut net, [1, 0, 0]);
        floor_rail(&mut net, [2, 1, 0]);
        floor_rail(&mut net, [3, 1, 0]);
        let path = [[0, 0, 0], [1, 0, 0], [2, 1, 0], [3, 1, 0]];
        let r = route(&net, &RailSpec::default(), &path);
        let at: Vec<[f64; 3]> = r.points.iter().map(|p| p.at).collect();
        // Middle of the first, the piece's foot, its top, the middle of the next and so on.
        let expected = [
            [0.5, 0.05, 0.5],
            [1.0, 0.05, 0.5],
            [2.0, 1.05, 0.5],
            [2.5, 1.05, 0.5],
            [3.0, 1.05, 0.5],
            [3.5, 1.05, 0.5],
        ];
        assert_eq!(at.len(), expected.len(), "{at:?}");
        for (a, e) in at.iter().zip(expected) {
            assert!(close(*a, e), "{a:?} {e:?}");
        }
    }

    #[test]
    fn a_ceiling_climb_runs_under_its_piece() {
        let mut net = Network::default();
        rail(&mut net, [0, 0, 0], Face::YPos);
        rail(&mut net, [1, 1, 0], Face::YPos);
        rail(&mut net, [2, 1, 0], Face::YPos);
        let r = route(
            &net,
            &RailSpec::default(),
            &[[0, 0, 0], [1, 1, 0], [2, 1, 0]],
        );
        let low = capsule::feet_at([0, 0, 0], Face::YPos);
        let high = capsule::feet_at([2, 1, 0], Face::YPos);
        let at: Vec<[f64; 3]> = r.points.iter().map(|p| p.at).collect();
        assert!(close(at[0], low));
        // Foot of the piece under the lower ceiling, its top under the upper.
        assert!(close(at[1], [1.0, low[1], 0.5]));
        assert!(close(at[2], [2.0, high[1], 0.5]));
        assert!(close(*at.last().unwrap(), high));
    }

    #[test]
    fn the_capsule_speeds_up_runs_and_stops_on_time() {
        let mut net = Network::default();
        for x in 0..=64 {
            net.rails.insert(
                [x, 0, 0],
                super::super::Rail {
                    tier: 4,
                    mount: Face::YNeg,
                },
            );
        }
        let path: Vec<Pos> = (0..=64).map(|x| [x, 0, 0]).collect();
        let r = route(&net, &RailSpec::default(), &path);
        // 64 blocks at 32 a second: 40 ticks and the start and stop.
        assert_eq!(r.ticks(20.0), 44);
        assert!(close(r.at_tick(0, 20.0), r.start()));
        assert!(close(r.at_tick(44, 20.0), r.end()));
        // Cruising: 1.6 blocks a tick.
        let a = r.at_tick(20, 20.0);
        let b = r.at_tick(21, 20.0);
        assert!((distance(a, b) - 1.6).abs() < 1e-9);
        // Never a step longer than the cruise.
        for t in 0..44 {
            assert!(distance(r.at_tick(t, 20.0), r.at_tick(t + 1, 20.0)) <= 1.6 + 1e-9);
        }
    }

    #[test]
    fn a_wall_run_keeps_its_mount_and_the_floor_switches_at_the_joint() {
        let mut net = Network::default();
        floor_rail(&mut net, [0, 0, 0]);
        rail(&mut net, [1, 0, 0], Face::ZNeg);
        rail(&mut net, [2, 0, 0], Face::ZNeg);
        let r = route(
            &net,
            &RailSpec::default(),
            &[[0, 0, 0], [1, 0, 0], [2, 0, 0]],
        );
        assert_eq!(r.points[0].mount, Face::YNeg);
        // From the joint on, the wall's bracket.
        assert_eq!(r.points[1].mount, Face::ZNeg);
        assert!(close(r.end(), [2.5, 0.05, 0.5 + capsule::WALL_SHIFT]));
    }
}
