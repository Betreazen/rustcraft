//! Slice 8: the energy sources and the battery (spec 4.2, 18.2 E10–E17, 21 E18–E21, 12.3; guide
//! `solar`, `windmill`, `gen_lava`, `battery`, `structure`). What each source offers this tick,
//! which blocks form one structure, and what a battery structure holds and passes. The sharing
//! itself is `net::energy`'s.

use std::collections::{BTreeSet, VecDeque};

use crate::geom::{Face, Pos, step};
use crate::modules::{self, Module};
use crate::tuning::Tuning;

/// The fluid a crystal structure drinks (E8). It exists only in tanks and channels (spec 7).
pub const SOLUTION: &str = "rustcraft:solution";
pub const LAVA: &str = "minecraft:lava";
/// Millibuckets in a bucket.
pub const BUCKET: f64 = 1000.0;

/// What a power block shows (A4, E9): its window's status line and its look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shown {
    /// Gives this many E a second (a structure: the whole structure).
    Gives(f64),
    /// A solar panel under a block (E21).
    NoSky,
    /// A windmill whose blades a block is in the way of (guide `windmill`).
    Blocked,
    /// A lava generator with no lava.
    NoLava,
    /// A crystal structure without solution gives one crystal's output (E8.a).
    NoSolution(f64),
    /// Crystals that touch but are no allowed box (guide `structure`): one crystal's output.
    BrokenShape(f64),
    /// A battery structure: takes, gives, or neither, with what it holds.
    Charging,
    Discharging,
    Holding,
}

/// What the self-test and the windows read of a power block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowerView {
    /// E a second the block (or its whole structure) offers this tick.
    pub output: f64,
    pub shown: Shown,
    /// Blocks in its structure.
    pub blocks: usize,
    /// A battery structure's charge and size, E.
    pub stored: f64,
    pub capacity: f64,
    /// The fluid it holds, mB: a lava generator's lava, a crystal structure's solution.
    pub fluid: f64,
}

// ---- Structures ----

/// The blocks of `blocks` that touch by a face, one set a group; with `flat`, only side by side on
/// one level (E18: panels at another height are another structure).
pub fn groups(blocks: &BTreeSet<Pos>, flat: bool) -> Vec<BTreeSet<Pos>> {
    let mut left = blocks.clone();
    let mut out = Vec::new();
    while let Some(&first) = left.iter().next() {
        left.remove(&first);
        let mut group = BTreeSet::from([first]);
        let mut queue = VecDeque::from([first]);
        while let Some(p) = queue.pop_front() {
            for f in Face::ALL {
                if flat && matches!(f, Face::YNeg | Face::YPos) {
                    continue;
                }
                let n = step(p, f);
                if left.remove(&n) {
                    group.insert(n);
                    queue.push_back(n);
                }
            }
        }
        out.push(group);
    }
    out
}

/// The block a structure keeps its shared numbers in: its least position.
pub fn anchor(group: &BTreeSet<Pos>) -> Pos {
    group.iter().next().copied().unwrap_or([0, 0, 0])
}

/// The form of touching crystals (E7): a single crystal, a solid box of an allowed size, or
/// anything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Single,
    /// 2×1, 2×2, 3×3, 3×3×3.
    Box([i32; 3]),
    Broken,
}

pub fn form(group: &BTreeSet<Pos>) -> Form {
    if group.len() <= 1 {
        return Form::Single;
    }
    let lo: Vec<i32> = (0..3)
        .map(|k| group.iter().map(|p| p[k]).min().unwrap_or(0))
        .collect();
    let hi: Vec<i32> = (0..3)
        .map(|k| group.iter().map(|p| p[k]).max().unwrap_or(0))
        .collect();
    let mut size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let volume = size.iter().product::<i32>() as usize;
    size.sort_unstable();
    let allowed = [[1, 1, 2], [1, 2, 2], [1, 3, 3], [3, 3, 3]];
    if volume == group.len() && allowed.contains(&size) {
        Form::Box(size)
    } else {
        Form::Broken
    }
}

/// E6–E8: what a group of `n` touching crystals of form `form` gives, E/s in all: a single
/// crystal its own; a fed box 4 × N^1.5; an unfed box or a broken form one crystal's (E8.a).
pub fn structure_output(n: usize, form: Form, fed: bool, t: &Tuning) -> f64 {
    match form {
        Form::Box(_) if fed => t.structure_base * (n as f64).powf(t.structure_power),
        _ => t.crystal_output,
    }
}

/// What a crystal group shows (E9).
pub fn structure_shown(n: usize, form: Form, fed: bool, t: &Tuning) -> Shown {
    let out = structure_output(n, form, fed, t);
    match form {
        Form::Single => Shown::Gives(out),
        Form::Broken => Shown::BrokenShape(out),
        Form::Box(_) if fed => Shown::Gives(out),
        Form::Box(_) => Shown::NoSolution(out),
    }
}

/// The solution a box of `n` crystals drinks, mB a second (spec 12.3: 0.1 bucket a minute each).
pub fn solution_per_second(n: usize, t: &Tuning) -> f64 {
    n as f64 * t.solution_per_minute * BUCKET / 60.0
}

/// The most solution a structure keeps, mB.
pub fn solution_room(n: usize, t: &Tuning) -> f64 {
    let _ = n;
    t.solution_buffer * BUCKET
}

// ---- The sun and the wind ----

/// Whether the sun is up at `day_time` (ticks, any day): from dawn at 23 500 to dusk at 12 500.
pub fn is_day(day_time: i64) -> bool {
    let t = day_time.rem_euclid(24_000);
    !(12_500..23_500).contains(&t)
}

/// E13, E21: a panel's output with or without sky above it, at `day_time` in rain `rain` (0..1).
pub fn solar_output(sky: bool, day_time: i64, rain: f32, t: &Tuning) -> f64 {
    if !sky {
        0.0
    } else if !is_day(day_time) {
        t.solar_day * t.solar_night_share
    } else if f64::from(rain) >= t.solar_rain_from {
        t.solar_day * t.solar_rain_share
    } else {
        t.solar_day
    }
}

/// E14: a windmill's output at height `y`, nothing when its blades are blocked.
pub fn windmill_output(y: i32, clear: bool, t: &Tuning) -> f64 {
    if !clear {
        return 0.0;
    }
    let span = f64::from((t.windmill_high_y - t.windmill_low_y).max(1));
    let k = (f64::from(y - t.windmill_low_y) / span).clamp(0.0, 1.0);
    t.windmill_low + (t.windmill_high - t.windmill_low) * k
}

/// The cells a windmill at `pos` needs free: the two above it (its mast) and the ring round the
/// first of them, where the blades turn.
pub fn windmill_room(pos: Pos) -> Vec<Pos> {
    let up = |n: i32| [pos[0], pos[1] + n, pos[2]];
    let mut cells = vec![up(1), up(2)];
    for dx in -1..=1 {
        for dz in -1..=1 {
            if dx != 0 || dz != 0 {
                cells.push([pos[0] + dx, pos[1] + 1, pos[2] + dz]);
            }
        }
    }
    cells
}

// ---- Lava ----

/// What a lava generator holding `lava` mB offers, E/s: its full output while a tick's worth lasts.
pub fn lava_offer(lava: f64, t: &Tuning) -> f64 {
    let can = lava.max(0.0) / BUCKET * t.lava_energy_per_bucket * t.ticks_per_second;
    t.lava_output.min(can)
}

/// The lava `used` E/s burns over `dt` seconds, mB: it burns only for what it gave (as the fuel
/// generator does), so a bucket always gives all its energy.
pub fn lava_burnt(used: f64, dt: f64, t: &Tuning) -> f64 {
    used.max(0.0) * dt / t.lava_energy_per_bucket * BUCKET
}

/// The lava a generator holds at most, mB.
pub fn lava_capacity(t: &Tuning) -> f64 {
    t.lava_buckets as f64 * BUCKET
}

// ---- The battery ----

/// A battery structure's size: what it holds, and how fast it takes and gives, E and E/s.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Battery {
    pub capacity: f64,
    pub take: f64,
    pub give: f64,
}

/// E11, A6, A7: `blocks` batteries with `slots` the modules of all of them: each capacity module
/// adds half of the whole store, each output module half of the giving rate (A9: in full).
pub fn battery(blocks: usize, slots: &[Option<Module>], t: &Tuning) -> Battery {
    let n = blocks as f64;
    let caps = f64::from(modules::count(slots, Module::StoreCapacity));
    let outs = f64::from(modules::count(slots, Module::Output));
    Battery {
        capacity: n * t.battery_capacity * (1.0 + t.battery_capacity_module * caps),
        take: n * t.battery_rate,
        give: n * t.battery_rate * (1.0 + t.battery_output_module * outs),
    }
}

impl Battery {
    /// What a structure holding `stored` may give and take this tick, E/s.
    pub fn flow(&self, stored: f64, t: &Tuning) -> (f64, f64) {
        let give = self.give.min(stored.max(0.0) * t.ticks_per_second);
        let take = self
            .take
            .min((self.capacity - stored).max(0.0) * t.ticks_per_second);
        (give, take)
    }

    /// What it shows after giving `given` and taking `taken`, E/s.
    pub fn shown(given: f64, taken: f64) -> Shown {
        if taken > given + 1e-9 {
            Shown::Charging
        } else if given > taken + 1e-9 {
            Shown::Discharging
        } else {
            Shown::Holding
        }
    }
}

/// Splits `total` among `n` blocks evenly, as tanks lay out their fluid: what each block keeps.
pub fn shares(total: f64, n: usize) -> f64 {
    if n == 0 { 0.0 } else { total / n as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(list: &[Pos]) -> BTreeSet<Pos> {
        list.iter().copied().collect()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn touching_blocks_are_one_group_and_flat_ones_keep_their_level() {
        let blocks = set(&[[0, 0, 0], [1, 0, 0], [1, 1, 0], [5, 0, 0]]);
        let g = groups(&blocks, false);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].len(), 3);
        // E18: a panel above another is a structure of its own.
        let flat = groups(&blocks, true);
        assert_eq!(flat.len(), 3);
    }

    #[test]
    fn crystal_forms_follow_the_allowed_boxes() {
        assert_eq!(form(&set(&[[0, 0, 0]])), Form::Single);
        assert_eq!(form(&set(&[[0, 0, 0], [1, 0, 0]])), Form::Box([1, 1, 2]));
        let square = set(&[[0, 0, 0], [1, 0, 0], [0, 0, 1], [1, 0, 1]]);
        assert_eq!(form(&square), Form::Box([1, 2, 2]));
        // A row of three and an L are no allowed box.
        assert_eq!(form(&set(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]])), Form::Broken);
        assert_eq!(form(&set(&[[0, 0, 0], [1, 0, 0], [1, 0, 1]])), Form::Broken);
    }

    #[test]
    fn a_fed_structure_gives_the_spec_table_and_an_unfed_one_a_crystal() {
        // Spec 4.2: 2×1 11, 2×2 32, 3×3 108, 3×3×3 561 (rounded).
        let t = Tuning::default();
        let b = Form::Box([1, 1, 2]);
        assert_eq!(structure_output(2, b, true, &t).round(), 11.0);
        assert!(close(structure_output(4, b, true, &t), 32.0));
        assert!(close(structure_output(9, b, true, &t), 108.0));
        assert_eq!(structure_output(27, b, true, &t).round(), 561.0);
        assert!(close(structure_output(4, b, false, &t), 4.0));
        assert!(close(structure_output(3, Form::Broken, true, &t), 4.0));
        assert_eq!(structure_shown(4, b, false, &t), Shown::NoSolution(4.0));
        // 0.1 bucket a minute a crystal: a 2×2 drinks 400 mB a minute.
        assert!(close(solution_per_second(4, &t) * 60.0, 400.0));
    }

    #[test]
    fn a_panel_gives_by_day_less_in_rain_and_a_quarter_at_night() {
        // Guide `solar`: 2 by day, 1 in rain, 0.5 at night; nothing under a block (E21).
        let t = Tuning::default();
        assert!(close(solar_output(true, 6_000, 0.0, &t), 2.0));
        assert!(close(solar_output(true, 6_000, 1.0, &t), 1.0));
        assert!(close(solar_output(true, 18_000, 0.0, &t), 0.5));
        assert!(close(solar_output(true, 24_000 * 3 + 100, 0.0, &t), 2.0));
        assert!(close(solar_output(false, 6_000, 0.0, &t), 0.0));
    }

    #[test]
    fn a_windmill_gives_more_higher_up_and_nothing_when_blocked() {
        // Guide `windmill`: 4 at sea level, 10 high in the mountains.
        let t = Tuning::default();
        assert!(close(windmill_output(63, true, &t), 4.0));
        assert!(close(windmill_output(30, true, &t), 4.0));
        assert!(close(windmill_output(160, true, &t), 10.0));
        assert!(close(windmill_output(300, true, &t), 10.0));
        let mid = windmill_output(111, true, &t);
        assert!(mid > 6.9 && mid < 7.0, "{mid}");
        assert!(close(windmill_output(200, false, &t), 0.0));
        assert_eq!(windmill_room([0, 10, 0]).len(), 10);
    }

    #[test]
    fn a_bucket_of_lava_gives_8000_e_at_40_a_second() {
        let t = Tuning::default();
        assert!(close(lava_offer(1000.0, &t), 40.0));
        assert!(close(lava_offer(0.0, &t), 0.0));
        // 1 mB is 8 E; a tick's 2 E of it is 0.25 mB: the offer drops to what is left.
        assert!(close(lava_offer(0.1, &t), 16.0));
        assert!(close(lava_burnt(40.0, 200.0, &t), 1000.0));
        assert!(close(lava_capacity(&t), 16_000.0));
    }

    #[test]
    fn a_battery_structure_grows_with_its_blocks_and_modules() {
        // Guide `battery`: 40 000 E and 32 E/s a block; +50% a module.
        let t = Tuning::default();
        let b = battery(2, &[], &t);
        assert!(close(b.capacity, 80_000.0));
        assert!(close(b.give, 64.0));
        let m = battery(
            1,
            &[
                Some(Module::StoreCapacity),
                Some(Module::StoreCapacity),
                Some(Module::Output),
            ],
            &t,
        );
        assert!(close(m.capacity, 80_000.0));
        assert!(close(m.give, 48.0));
        assert!(close(m.take, 32.0));
        // Empty it gives nothing; full it takes nothing.
        assert_eq!(b.flow(0.0, &t).0, 0.0);
        assert_eq!(b.flow(80_000.0, &t).1, 0.0);
        assert!(close(b.flow(1.0, &t).0, 20.0));
        assert_eq!(Battery::shown(0.0, 4.0), Shown::Charging);
        assert_eq!(Battery::shown(8.0, 0.0), Shown::Discharging);
    }
}
