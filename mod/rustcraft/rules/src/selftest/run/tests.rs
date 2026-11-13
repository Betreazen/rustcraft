use super::*;
use crate::geom::{Face, Pos};
use crate::housing::Diagnosis;
use crate::machines::alloy::AlloyRecipe;
use crate::machines::processor::Shown;
use crate::monorail::{Network, Trip, TripError};
use crate::net::settings::JointSetting;
use crate::net::tanks::TankGroup;
use crate::net::topology::Channel;
use crate::selftest::{Area, Slot, StorageView};
use crate::storage::terminal::LimitChange;
use crate::tuning::Tuning;
use std::collections::BTreeMap;

/// A world of named blocks and nothing else; chunks load after `load_after` asks.
#[derive(Default)]
struct Fake {
    tuning: Tuning,
    blocks: BTreeMap<Pos, String>,
    held: usize,
    load_after: u32,
    asked: u32,
    refreshes: u32,
    clears: u32,
    ms: u128,
}

impl Bench for Fake {
    fn hold(&mut self, _: &Area, hold: bool) {
        if hold {
            self.held += 1;
        } else {
            self.held -= 1;
        }
    }
    fn loaded(&self, _: &Area) -> bool {
        self.asked >= self.load_after
    }
    fn clear(&mut self, area: &Area) -> Result<(), String> {
        self.clears += 1;
        self.blocks
            .retain(|p, _| (0..3).any(|k| p[k] < area.min[k] || p[k] > area.max[k]));
        Ok(())
    }
    fn refresh(&mut self) {
        self.refreshes += 1;
    }
    fn now_ms(&self) -> u128 {
        self.ms
    }
    fn place(&mut self, pos: Pos, block: &str, _: &[(&str, &str)]) -> Result<(), String> {
        self.blocks.insert(pos, block.to_owned());
        Ok(())
    }
    fn put(&mut self, _: Pos, _: usize, _: &str, _: u32) -> Result<(), String> {
        Ok(())
    }
    fn insert(&mut self, _: Pos, _: &str, count: u32) -> Result<u32, String> {
        Ok(count)
    }
    fn set_channels(&mut self, _: Pos, _: [Option<Channel>; 4]) -> Result<(), String> {
        Ok(())
    }
    fn set_joint(&mut self, _: Pos, _: Face, _: usize, _: JointSetting) -> Result<(), String> {
        Ok(())
    }
    fn press(&mut self, _: Pos, _: i32, _: Option<(&str, u32)>) -> Result<(), String> {
        Ok(())
    }
    fn set_tank(&mut self, _: Pos, _: &str, _: u64) -> Result<(), String> {
        Ok(())
    }
    fn set_station_energy(&mut self, _: Pos, _: f64) -> Result<(), String> {
        Ok(())
    }
    fn change_limit(&mut self, _: Pos, _: &str, _: LimitChange) -> Result<(), String> {
        Ok(())
    }
    fn tuning(&self) -> &Tuning {
        &self.tuning
    }
    fn block(&self, pos: Pos) -> String {
        self.blocks
            .get(&pos)
            .cloned()
            .unwrap_or_else(|| "minecraft:air".into())
    }
    fn property(&self, _: Pos, _: &str) -> Option<String> {
        None
    }
    fn items(&self, _: Pos) -> Vec<Slot> {
        Vec::new()
    }
    fn inputs(&self, _: Pos) -> Vec<usize> {
        Vec::new()
    }
    fn housing(&self, _: Pos) -> Option<Diagnosis> {
        None
    }
    fn shown(&self, _: Pos) -> Option<Shown> {
        None
    }
    fn receiver_full(&self, _: Pos) -> bool {
        false
    }
    fn storage(&self, _: Pos) -> Option<StorageView> {
        None
    }
    fn tank(&self, _: Pos) -> Option<TankGroup> {
        None
    }
    fn no_lane(&self, _: Pos) -> Option<String> {
        None
    }
    fn station_energy(&self, _: Pos) -> Option<f64> {
        None
    }
    fn network(&self) -> Network {
        Network::default()
    }
    fn plan_trip(&mut self, from: Pos, _: Pos, _: bool) -> Result<Trip, TripError> {
        Err(TripError::NoRail(from))
    }
    fn cook_ticks(&self, _: &str) -> Option<f64> {
        None
    }
    fn crush(&self, _: &str) -> Option<(String, u32, f64)> {
        None
    }
    fn alloy_recipes(&self) -> Vec<AlloyRecipe> {
        Vec::new()
    }
    fn craft(&self, _: &[Option<&str>; 9]) -> Option<(String, u32)> {
        None
    }
    fn round_trip(&self, _: Pos) -> Result<(), String> {
        Ok(())
    }
    fn spawn_mob(&mut self, _: &str, _: [f64; 3], _: Option<f32>) -> Result<u32, String> {
        Ok(1)
    }
    fn mob(&self, _: u32) -> Option<crate::selftest::MobView> {
        None
    }
    fn set_modules(&mut self, _: Pos, _: &[&str]) -> Result<(), String> {
        Ok(())
    }
    fn set_owner(&mut self, _: Pos, _: u128, _: u64) -> Result<(), String> {
        Ok(())
    }
    fn set_buffer(&mut self, _: Pos, _: f64) -> Result<(), String> {
        Ok(())
    }
    fn buffer(&self, _: Pos) -> f64 {
        0.0
    }
    fn turret(&self, _: Pos) -> Option<crate::turret::Shown> {
        None
    }
    fn bleeding(&self, _: u32) -> bool {
        false
    }
    fn souls_found(&self) -> bool {
        false
    }
    fn spawn_worker(
        &mut self,
        _: Pos,
        _: u128,
        _: Option<crate::worker::Tool>,
        _: &[(&str, u32)],
        _: Option<f64>,
    ) -> Result<u64, String> {
        Ok(1)
    }
    fn worker(&self, _: u64) -> Option<crate::worker::Worker> {
        None
    }
    fn reports(&self) -> Vec<String> {
        Vec::new()
    }
    fn pretend_online(&mut self, _: u128, _: bool) {}
    fn anchor(&self, _: Pos) -> Option<crate::anchor::State> {
        None
    }
    fn held_chunks(&self) -> Vec<[i32; 2]> {
        Vec::new()
    }
    fn workers_round_trip(&self) -> Result<(), String> {
        Ok(())
    }
    fn power(&self, _: Pos) -> Option<crate::power::PowerView> {
        None
    }
    fn sky(&self) -> (i64, f32) {
        (0, 0.0)
    }
    fn light(&self, _: Pos) -> (u8, u8) {
        (0, 15)
    }
    fn min_y(&self) -> i32 {
        -64
    }
    fn mining(&self, _: Pos) -> Option<crate::mining::Shown> {
        None
    }
    fn set_ores_off(&mut self, _: Pos, _: u16) -> Result<(), String> {
        Ok(())
    }
    fn role(&self, _: Pos) -> Option<String> {
        None
    }
    fn testkit(&mut self, _: &str) -> String {
        String::new()
    }
}

const PLOT: Area = Area {
    min: [0, 100, 0],
    max: [3, 103, 3],
};

/// Ticks the run from tick 1 until it reports, at most `limit` ticks.
fn drive(run: &mut Run, bench: &mut Fake, limit: u64) -> (Report, u64) {
    for now in 1..=limit {
        bench.asked += 1;
        bench.ms += 1;
        if let Some(report) = run.tick(bench, now) {
            return (report, now);
        }
    }
    panic!("no report in {limit} ticks");
}

#[test]
fn a_scenario_builds_waits_checks_and_its_plot_is_cleared() {
    let s = Scenario::new("stone", PLOT)
        .then("place", |b, _| b.place([1, 101, 1], "minecraft:stone", &[]))
        .wait(5)
        .check("it stands", |b, m| {
            crate::selftest::same("block", b.block([1, 101, 1]), "minecraft:stone".into())?;
            crate::selftest::same("elapsed", m.elapsed, 5)
        });
    let mut bench = Fake {
        load_after: 3,
        ..Fake::default()
    };
    let mut run = Run::new(vec![s]);
    let (report, _) = drive(&mut run, &mut bench, 100);
    assert_eq!(report.passed, vec!["stone".to_owned()], "{report:?}");
    assert!(report.failed.is_empty());
    assert_eq!(report.timings[0].ticks, 5);
    assert_eq!(report.setup.0, 2);
    assert!(bench.blocks.is_empty(), "the plot was cleared");
    assert_eq!(bench.held, 0, "the chunks were let go");
    assert_eq!(bench.clears, 2);
}

#[test]
fn a_check_that_never_holds_fails_after_its_limit_and_names_why() {
    let s = Scenario::new("never", PLOT).until("pigs fly", 10, |_, _| Ok(false));
    let fails = Scenario::new("broken", PLOT).then("act", |_, _| Err("no block entity".into()));
    let mut bench = Fake::default();
    let mut run = Run::new(vec![s, fails]);
    let (report, _) = drive(&mut run, &mut bench, 100);
    assert_eq!(
        report.failed,
        vec![
            (
                "never".to_owned(),
                "pigs fly: not so after 10 ticks".to_owned()
            ),
            ("broken".to_owned(), "act: no block entity".to_owned())
        ]
    );
    assert_eq!(report.timings[0].ticks, 10);
}

#[test]
fn chunks_that_never_load_fail_every_scenario_by_wall_time() {
    let s = Scenario::new("far", PLOT).wait(1);
    let mut bench = Fake {
        load_after: u32::MAX,
        ..Fake::default()
    };
    let mut run = Run::new(vec![s]);
    bench.ms = 0;
    assert!(run.tick(&mut bench, 1).is_none());
    bench.ms = LOAD_LIMIT_MS + 1;
    let mut report = None;
    for now in 2..10 {
        report = run.tick(&mut bench, now);
        if report.is_some() {
            break;
        }
    }
    let report = report.expect("a report");
    assert_eq!(report.failed.len(), 1);
    assert!(report.failed[0].1.contains("did not load"), "{report:?}");
}

#[test]
fn the_report_is_the_bench_json() {
    let report = Report {
        passed: vec!["a".into()],
        failed: vec![("b \"x\"".into(), "line\nbreak".into())],
        timings: vec![Timing {
            name: "a".into(),
            ticks: 12,
            ms: 3,
        }],
        setup: (4, 5),
    };
    let json = report.to_json("rustcraft");
    assert!(json.contains("\"mod\": \"rustcraft\""));
    assert!(json.contains("\"done\": true"));
    assert!(json.contains("\"passed\": [\"a\"]"));
    assert!(json.contains("{\"name\": \"b \\\"x\\\"\", \"why\": \"line\\nbreak\"}"));
    assert!(json.contains("\"timings\": {\"a\": {\"ticks\": 12, \"ms\": 3}}"));
    assert!(json.contains("\"setup\": {\"ticks\": 4, \"ms\": 5}"));
}
