//! Runs the scenarios from the level's tick, all side by side, each on its own plot, and reports.
//! The bundle calls `Run::tick` once a level tick of the level the test runs in, after the
//! mod's own work for that tick, and writes the report it returns at the end.

use super::{Bench, Memo, Scenario, Stage};

/// How long the plots' chunks may take to load, in milliseconds: wall time, because under
/// `/tick sprint` ticks pass far faster than the chunk workers.
pub const LOAD_LIMIT_MS: u128 = 60_000;

/// What one scenario is doing.
struct Live {
    scenario: Scenario,
    stage: usize,
    /// The tick the stage under way started on.
    stage_tick: u64,
    started: Option<(u64, u128)>,
    memo: Memo,
    outcome: Option<Result<(), String>>,
    cost: (u64, u128),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// The plots' chunks asked for; the scenarios wait for them.
    Loading,
    Running,
    Done,
}

/// One self-test run.
pub struct Run {
    live: Vec<Live>,
    phase: Phase,
    begun: Option<(u64, u128)>,
    setup: (u64, u128),
}

/// One scenario's cost: level ticks and wall milliseconds from its first stage to its end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timing {
    pub name: String,
    pub ticks: u64,
    pub ms: u128,
}

/// What a finished run found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub passed: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub timings: Vec<Timing>,
    /// Holding, loading and clearing the plots before the scenarios start.
    pub setup: (u64, u128),
}

impl Run {
    pub fn new(scenarios: Vec<Scenario>) -> Self {
        Run {
            live: scenarios
                .into_iter()
                .map(|scenario| Live {
                    scenario,
                    stage: 0,
                    stage_tick: 0,
                    started: None,
                    memo: Memo::default(),
                    outcome: None,
                    cost: (0, 0),
                })
                .collect(),
            phase: Phase::Loading,
            begun: None,
            setup: (0, 0),
        }
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// One level tick `now` of the run; the report once every scenario has ended.
    pub fn tick(&mut self, bench: &mut dyn Bench, now: u64) -> Option<Report> {
        match self.phase {
            Phase::Loading => self.load(bench, now),
            Phase::Running => self.step_all(bench, now),
            Phase::Done => {}
        }
        (self.phase == Phase::Done).then(|| self.report())
    }

    fn load(&mut self, bench: &mut dyn Bench, now: u64) {
        let ms = bench.now_ms();
        let (since_tick, since_ms) = *self.begun.get_or_insert_with(|| {
            for l in &self.live {
                bench.hold(&l.scenario.area, true);
            }
            (now, ms)
        });
        let loaded = self.live.iter().all(|l| bench.loaded(&l.scenario.area));
        if !loaded && ms - since_ms < LOAD_LIMIT_MS {
            return;
        }
        self.setup = (now - since_tick, ms - since_ms);
        for l in &mut self.live {
            if !loaded {
                l.outcome = Some(Err(format!(
                    "its plot's chunks did not load in {LOAD_LIMIT_MS} ms"
                )));
            } else if let Err(why) = bench.clear(&l.scenario.area) {
                l.outcome = Some(Err(format!("the plot was not cleared: {why}")));
            }
        }
        bench.refresh();
        self.phase = Phase::Running;
        // Scenarios start on the next tick, once the cleared plots were read again.
    }

    fn step_all(&mut self, bench: &mut dyn Bench, now: u64) {
        let ms = bench.now_ms();
        for l in &mut self.live {
            if l.outcome.is_some() {
                continue;
            }
            let (start_tick, start_ms) = *l.started.get_or_insert_with(|| {
                l.stage_tick = now;
                (now, ms)
            });
            l.memo.elapsed = now - start_tick;
            if let Some(outcome) = step(l, bench, now) {
                l.outcome = Some(outcome);
                l.cost = (now - start_tick, bench.now_ms() - start_ms);
                if let Err(why) = bench.clear(&l.scenario.area) {
                    log_clear(l, &why);
                }
                bench.refresh();
            }
        }
        if self.live.iter().all(|l| l.outcome.is_some()) {
            for l in &self.live {
                bench.hold(&l.scenario.area, false);
            }
            self.phase = Phase::Done;
        }
    }

    fn report(&self) -> Report {
        let mut report = Report {
            setup: self.setup,
            ..Report::default()
        };
        for l in &self.live {
            let name = l.scenario.name.to_owned();
            match &l.outcome {
                Some(Ok(())) => report.passed.push(name.clone()),
                Some(Err(why)) => report.failed.push((name.clone(), why.clone())),
                None => report.failed.push((name.clone(), "did not end".into())),
            }
            report.timings.push(Timing {
                name,
                ticks: l.cost.0,
                ms: l.cost.1,
            });
        }
        report
    }
}

/// A plot left uncleared is noted in the scenario's outcome, which a passing one would hide.
fn log_clear(l: &mut Live, why: &str) {
    if let Some(Ok(())) = l.outcome {
        l.outcome = Some(Err(format!("passed, but its plot was not cleared: {why}")));
    }
}

/// Advances one scenario as far as this tick allows; its outcome once it ends.
fn step(l: &mut Live, bench: &mut dyn Bench, now: u64) -> Option<Result<(), String>> {
    loop {
        let Some(stage) = l.scenario.stages.get_mut(l.stage) else {
            return Some(Ok(()));
        };
        match stage {
            Stage::Do { what, act } => {
                if let Err(why) = act(bench, &mut l.memo) {
                    return Some(Err(format!("{what}: {why}")));
                }
                bench.refresh();
                next(l, now);
                // The world is read again at the end of this tick: the next stage sees it then.
                return None;
            }
            Stage::Wait(ticks) => {
                if now - l.stage_tick < *ticks {
                    return None;
                }
                next(l, now);
            }
            Stage::Until { what, limit, probe } => match probe(bench, &mut l.memo) {
                Ok(true) => next(l, now),
                Ok(false) if now - l.stage_tick < *limit => return None,
                Ok(false) => {
                    return Some(Err(format!("{what}: not so after {limit} ticks")));
                }
                Err(why) => {
                    return Some(Err(format!("{what}: {why}")));
                }
            },
        }
    }
}

fn next(l: &mut Live, now: u64) {
    l.stage += 1;
    l.stage_tick = now;
}

impl Report {
    /// The result file the joint bench reads: `{"mod","done","passed","failed","timings","setup"}`.
    pub fn to_json(&self, mod_id: &str) -> String {
        let passed: Vec<String> = self.passed.iter().map(|n| quote(n)).collect();
        let failed: Vec<String> = self
            .failed
            .iter()
            .map(|(n, why)| format!("{{\"name\": {}, \"why\": {}}}", quote(n), quote(why)))
            .collect();
        let timings: Vec<String> = self
            .timings
            .iter()
            .map(|t| {
                format!(
                    "{}: {{\"ticks\": {}, \"ms\": {}}}",
                    quote(&t.name),
                    t.ticks,
                    t.ms
                )
            })
            .collect();
        format!(
            "{{\n  \"mod\": {},\n  \"done\": true,\n  \"passed\": [{}],\n  \"failed\": [{}],\n  \"timings\": {{{}}},\n  \"setup\": {{\"ticks\": {}, \"ms\": {}}}\n}}\n",
            quote(mod_id),
            passed.join(", "),
            failed.join(", "),
            timings.join(", "),
            self.setup.0,
            self.setup.1
        )
    }

    /// The line the console gets.
    pub fn summary(&self) -> String {
        let ticks = self.timings.iter().map(|t| t.ticks).max().unwrap_or(0);
        format!(
            "{} passed, {} failed; setup {} ticks / {} ms, longest scenario {} ticks",
            self.passed.len(),
            self.failed.len(),
            self.setup.0,
            self.setup.1,
            ticks
        )
    }
}

/// A JSON string.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests;
