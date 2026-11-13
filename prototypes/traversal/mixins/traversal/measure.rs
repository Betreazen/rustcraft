// Game-free measurement of one attempt: how far the measured positions strayed from the plan,
// how even their steps were, and the one-line result for chat and the summary file.
use crate::game::trv_plans::Plan;
use crate::game::trv_rules::{
    self as rules, ARRIVE_TOLERANCE, FLOATING_KICK_TICKS, FREEZE_STEP, JUMP_SLACK, Kind, MOVE_STEP,
    Method, STALL_SHARE, V3,
};
use serde::{Deserialize, Serialize};

/// What one tick's sample says against the plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Distance from the planned path.
    pub off_path: f64,
    /// How far behind the plan's point for this tick (negative: ahead).
    pub lag: f64,
    /// Blocks moved since the last sample.
    pub step: Option<f64>,
}

/// The server's view of one attempt, tick by tick.
#[derive(Clone, Debug)]
pub struct Tracker {
    plan: Plan,
    last: Option<V3>,
    last_step: Option<f64>,
    pub samples: u32,
    pub max_off_path: f64,
    pub max_lag: f64,
    pub jumps: u32,
    pub stalls: u32,
    pub max_step_change: f64,
}

impl Tracker {
    pub fn new(plan: &Plan) -> Tracker {
        Tracker {
            plan: plan.clone(),
            last: None,
            last_step: None,
            samples: 0,
            max_off_path: 0.0,
            max_lag: 0.0,
            jumps: 0,
            stalls: 0,
            max_step_change: 0.0,
        }
    }

    /// Records where the player was after `tick` ticks of the plan.
    pub fn push(&mut self, tick: u32, position: V3) -> Sample {
        let (along, off_path) = self.plan.path.locate(position);
        let lag = self.plan.progress_at(tick) - along;
        self.samples += 1;
        self.max_off_path = self.max_off_path.max(off_path);
        self.max_lag = self.max_lag.max(lag.abs());
        let step = self.last.map(|last| rules::distance(last, position));
        if let Some(step) = step {
            let (planned, cruising) = self.plan.planned_step(tick);
            if step > self.plan.peak_step() + JUMP_SLACK {
                self.jumps += 1;
            }
            if cruising && step < STALL_SHARE * planned {
                self.stalls += 1;
            }
            if let Some(last_step) = self.last_step {
                self.max_step_change = self.max_step_change.max((step - last_step).abs());
            }
            self.last_step = Some(step);
        }
        self.last = Some(position);
        Sample {
            off_path,
            lag,
            step,
        }
    }

    /// How far the last sample is from the plan's end.
    pub fn end_gap(&self) -> Option<f64> {
        self.last
            .map(|last| rules::distance(last, self.plan.path.end()))
    }
}

/// Positions without a plan (a rider's own view, an observer's view of the rider): how evenly
/// they move.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Watcher {
    last: Option<V3>,
    steps: Vec<f64>,
}

impl Watcher {
    pub fn push(&mut self, position: V3) -> Option<f64> {
        let step = self.last.map(|last| rules::distance(last, position));
        if let Some(step) = step {
            self.steps.push(step);
        }
        self.last = Some(position);
        step
    }

    /// The lengths of the runs of still ticks between moving ones: what a viewer sees as hitches.
    /// A run of any length counts, so a server stall of several ticks is one long freeze.
    fn freeze_runs(&self) -> Vec<u32> {
        let mut runs = Vec::new();
        let mut moved_before = false;
        let mut still = 0;
        for &step in &self.steps {
            if step < FREEZE_STEP {
                if moved_before {
                    still += 1;
                }
            } else if step > MOVE_STEP {
                if still > 0 {
                    runs.push(still);
                }
                moved_before = true;
                still = 0;
            }
        }
        runs
    }

    /// Hitches: runs of still ticks between moving ones.
    pub fn freezes(&self) -> u32 {
        self.freeze_runs().len() as u32
    }

    /// The longest hitch, in ticks.
    pub fn longest_freeze(&self) -> u32 {
        self.freeze_runs().into_iter().max().unwrap_or(0)
    }

    pub fn max_step(&self) -> f64 {
        self.steps.iter().copied().fold(0.0, f64::max)
    }

    pub fn max_step_change(&self) -> f64 {
        self.steps
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max)
    }

    pub fn ticks(&self) -> usize {
        self.steps.len()
    }

    pub fn line(&self) -> String {
        format!(
            "{} тиков, шаг до {:.2}, скачок шага до {:.2}, замираний {} (самое долгое {} тиков)",
            self.ticks(),
            self.max_step(),
            self.max_step_change(),
            self.freezes(),
            self.longest_freeze()
        )
    }
}

/// What the client tells the server when it finishes following a plan (method a), and what
/// it prints for itself under any method.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClientStats {
    pub ticks: u32,
    /// Largest distance between the client's position and the plan (method a).
    pub max_error: f64,
    /// Ticks whose push was clamped (method a).
    pub clamped: u32,
    /// Teleport packets (ClientboundPlayerPosition) the client got during the attempt.
    pub position_packets: u32,
    /// Ticks the client's player jumped without interpolation (a teleport).
    pub snaps: u32,
    pub freezes: u32,
    pub max_step_change: f64,
}

impl ClientStats {
    pub fn line(&self) -> String {
        format!(
            "клиент: {} тиков, пакетов телепорта {}, рывков без сглаживания {}, замираний {}, скачок шага до {:.2}, отход от плана до {:.2}, срезано толчков {}",
            self.ticks,
            self.position_packets,
            self.snaps,
            self.freezes,
            self.max_step_change,
            self.max_error,
            self.clamped
        )
    }
}

/// One attempt as it ended.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub attempt: u64,
    pub kind: Kind,
    pub method: Method,
    pub ticks: u32,
    pub planned: u32,
    pub length: f64,
    pub end_gap: Option<f64>,
    /// Corrections the server sent (Hub::resend_teleport): moved too quickly, moved wrongly,
    /// the anticheat's setback, a teleport sent again.
    pub setbacks: u32,
    /// Every teleport the server sent the player, and those the method itself asked for.
    pub teleports: u32,
    pub intended_teleports: u32,
    pub max_off_path: f64,
    pub max_lag: f64,
    pub jumps: u32,
    pub stalls: u32,
    pub max_step_change: f64,
    pub max_floating: i32,
    /// Why it ended early, if it did.
    pub failure: Option<String>,
}

impl Summary {
    pub fn from_tracker(attempt: u64, plan: &Plan, method: Method, tracker: &Tracker) -> Summary {
        Summary {
            attempt,
            kind: plan.kind,
            method,
            ticks: tracker.samples,
            planned: plan.total_ticks(),
            length: plan.length(),
            end_gap: tracker.end_gap(),
            setbacks: 0,
            teleports: 0,
            intended_teleports: 0,
            max_off_path: tracker.max_off_path,
            max_lag: tracker.max_lag,
            jumps: tracker.jumps,
            stalls: tracker.stalls,
            max_step_change: tracker.max_step_change,
            max_floating: 0,
            failure: None,
        }
    }

    pub fn reached(&self) -> bool {
        self.end_gap.is_some_and(|gap| gap <= ARRIVE_TOLERANCE)
    }

    /// The spec's hard test: the player got there and the server never sent them back.
    pub fn passed(&self) -> bool {
        self.failure.is_none() && self.reached() && self.setbacks == 0
    }

    pub fn line(&self) -> String {
        let verdict = if self.passed() { "ДА" } else { "НЕТ" };
        let arrival = match (self.reached(), self.end_gap) {
            (true, _) => "дошёл".to_owned(),
            (false, Some(gap)) => format!("не дошёл ({gap:.1} бл до конца)"),
            (false, None) => "нет замеров".to_owned(),
        };
        let failure = self
            .failure
            .as_deref()
            .map_or(String::new(), |why| format!(" Причина: {why}."));
        format!(
            "#{} {}/{}: {verdict} — {arrival}, {:.1} бл за {} тиков (план {}), откатов {}, телепортов {} (по плану {}), отход от пути до {:.2}, отставание до {:.2}, скачков {}, остановок {}, скачок шага до {:.2}, зависание до {}/{} тиков.{failure}",
            self.attempt,
            self.kind.label(),
            self.method.id(),
            self.length,
            self.ticks,
            self.planned,
            self.setbacks,
            self.teleports,
            self.intended_teleports,
            self.max_off_path,
            self.max_lag,
            self.jumps,
            self.stalls,
            self.max_step_change,
            self.max_floating,
            FLOATING_KICK_TICKS
        )
    }

    pub const CSV_HEADER: &'static str = "attempt,kind,method,passed,reached,end_gap,ticks,planned,length,setbacks,teleports,intended_teleports,max_off_path,max_lag,jumps,stalls,max_step_change,max_floating,failure";

    pub fn csv_row(&self) -> String {
        let failure = self
            .failure
            .as_deref()
            .unwrap_or("")
            .replace([',', '\n', '\r'], " ");
        format!(
            "{},{},{},{},{},{},{},{},{:.3},{},{},{},{:.3},{:.3},{},{},{:.3},{},{failure}",
            self.attempt,
            self.kind.id(),
            self.method.id(),
            self.passed(),
            self.reached(),
            self.end_gap
                .map_or(String::new(), |gap| format!("{gap:.3}")),
            self.ticks,
            self.planned,
            self.length,
            self.setbacks,
            self.teleports,
            self.intended_teleports,
            self.max_off_path,
            self.max_lag,
            self.jumps,
            self.stalls,
            self.max_step_change,
            self.max_floating
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::trv_plans::{Rope, plan_ride};

    fn line_plan() -> Plan {
        let rope = Rope {
            low: [0.0; 3],
            high: [16.0, 0.0, 0.0],
        };
        plan_ride([0.0; 3], &rope).unwrap()
    }

    #[test]
    fn following_the_plan_exactly_is_smooth_and_arrives() {
        let plan = line_plan();
        let mut tracker = Tracker::new(&plan);
        for tick in 0..=plan.total_ticks() {
            let sample = tracker.push(tick, plan.target_at(tick));
            assert!(sample.off_path < 1e-9 && sample.lag.abs() < 1e-9);
        }
        assert_eq!((tracker.jumps, tracker.stalls), (0, 0));
        let mut summary = Summary::from_tracker(7, &plan, Method::D, &tracker);
        assert!(summary.reached() && summary.passed());
        assert!(summary.line().contains("ДА — дошёл"), "{}", summary.line());
        summary.setbacks = 1;
        assert!(!summary.passed());
        assert!(summary.line().contains("НЕТ"));
    }

    #[test]
    fn a_teleport_back_counts_as_a_jump_and_leaves_the_player_short() {
        let plan = line_plan();
        let mut tracker = Tracker::new(&plan);
        for tick in 0..=20 {
            tracker.push(tick, plan.target_at(tick));
        }
        // Sent back to the start, then stuck there.
        for tick in 21..=plan.total_ticks() {
            tracker.push(tick, [0.0; 3]);
        }
        assert_eq!(tracker.jumps, 1);
        assert!(tracker.stalls > 5);
        assert!(tracker.max_lag > 15.0);
        let summary = Summary::from_tracker(1, &plan, Method::A, &tracker);
        assert!(!summary.reached());
        assert!(
            summary.line().contains("не дошёл (16.0 бл до конца)"),
            "{}",
            summary.line()
        );
    }

    #[test]
    fn an_early_end_is_a_failure_with_its_reason() {
        let plan = line_plan();
        let mut tracker = Tracker::new(&plan);
        tracker.push(0, plan.path.start());
        let mut summary = Summary::from_tracker(2, &plan, Method::B, &tracker);
        summary.failure = Some("моб-сиденье не заспавнилось".to_owned());
        assert!(!summary.passed());
        assert!(
            summary
                .line()
                .ends_with("Причина: моб-сиденье не заспавнилось.")
        );
        let row = summary.csv_row();
        assert_eq!(
            row.split(',').count(),
            Summary::CSV_HEADER.split(',').count()
        );
    }

    #[test]
    fn no_samples_is_not_an_arrival() {
        let plan = line_plan();
        let summary = Summary::from_tracker(3, &plan, Method::C, &Tracker::new(&plan));
        assert!(!summary.reached());
        assert!(summary.line().contains("нет замеров"));
    }

    #[test]
    fn a_watcher_finds_the_hitches_of_a_snapping_ride() {
        let mut smooth = Watcher::default();
        for i in 0..20 {
            smooth.push([f64::from(i) * 0.4, 0.0, 0.0]);
        }
        assert_eq!(smooth.freezes(), 0);
        assert!(smooth.max_step_change() < 1e-9);
        let mut snapping = Watcher::default();
        let mut x = 0.0;
        for i in 0..20 {
            // Every other tick stands still, the next makes up for it.
            if i % 2 == 0 {
                x += 0.8;
            }
            snapping.push([x, 0.0, 0.0]);
        }
        assert!(snapping.freezes() >= 8, "{}", snapping.freezes());
        assert!((snapping.max_step_change() - 0.8).abs() < 1e-9);
        assert!(snapping.line().contains("замираний"));
    }

    #[test]
    fn a_stall_of_several_ticks_is_one_long_freeze() {
        // Field test 2026-11-11: a 382 ms server tick stopped a rider for 5 ticks mid-rope, and
        // the old count (one still tick between moving ones) reported no freeze at all.
        let mut stalled = Watcher::default();
        let mut x = 0.0;
        for i in 0..30 {
            if !(10..15).contains(&i) {
                x += 0.4;
            }
            stalled.push([x, 0.0, 0.0]);
        }
        assert_eq!(stalled.freezes(), 1);
        assert_eq!(stalled.longest_freeze(), 5);
        // Standing still before moving and after arriving is not a freeze.
        let mut ends = Watcher::default();
        for x in [0.0, 0.0, 0.0, 0.4, 0.8, 1.2, 1.2, 1.2] {
            ends.push([x, 0.0, 0.0]);
        }
        assert_eq!(ends.freezes(), 0);
    }

    #[test]
    fn client_stats_read_as_one_line() {
        let stats = ClientStats {
            ticks: 30,
            position_packets: 2,
            ..ClientStats::default()
        };
        assert!(stats.line().contains("пакетов телепорта 2"));
    }
}
