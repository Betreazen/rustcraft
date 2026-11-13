//! Slice 5: tanks, pumps and fluid channels. Touching tanks are one, a tank of another fluid stays
//! apart; a pump keeps endless water but drains lava (round 15); a tier-1 fluid channel carries
//! one fluid at a time and names the other on its source, a tier-2 one carries both (M13).

use super::energy::CRYSTAL;
use super::magistral::{channel, line};
use super::{Bench, Build, Scenario, at, expect, near, plot_area, same};
use crate::geom::{Face, Pos};
use crate::machines::pump::{LAVA, WATER};
use crate::net::fluids::BUCKET;
use crate::net::items::Mode;
use crate::net::settings::JointSetting;
use crate::net::tanks::TankGroup;
use crate::net::topology::ChannelKind;
use crate::tuning::Tuning;

pub const ALL: [Build; 5] = [
    tanks_join,
    pump_keeps_endless_water,
    pump_drains_lava,
    one_fluid_on_tier_1,
    two_fluids_on_tier_2,
];

const TANK: &str = "rustcraft:tank";
const PUMP: &str = "rustcraft:pump";
const STONE: &str = "minecraft:stone";
/// The fluid tick runs every few ticks: a pumped bucket reaches its tank that much later.
const FLUID_SLACK: f64 = 5.0;

fn group(b: &dyn Bench, pos: Pos) -> Result<TankGroup, String> {
    b.tank(pos).ok_or_else(|| format!("no tank at {pos:?}"))
}

fn tanks_join(o: Pos, t: &Tuning) -> Scenario {
    let per_block = t.tank_buckets * BUCKET;
    let (low, high, side) = (o, at(o, [0, 1, 0]), at(o, [1, 0, 0]));
    Scenario::new("fluids_touching_tanks_are_one", plot_area(o))
        .then(
            "build two stacked tanks with water, and one with lava beside",
            move |b, _| {
                b.place(low, TANK, &[])?;
                b.place(high, TANK, &[])?;
                b.place(side, TANK, &[])?;
                b.set_tank(high, WATER, 10 * BUCKET)?;
                b.set_tank(side, LAVA, BUCKET)
            },
        )
        .check("the stacked tanks are one tank of water", move |b, _| {
            let g = group(b, low)?;
            same("blocks", g.blocks.len(), 2)?;
            same("capacity", g.capacity, 2 * per_block)?;
            same("fluid", g.fluid.as_deref(), Some(WATER))?;
            same("amount", g.amount, 10 * BUCKET)
        })
        .check("the lava tank stays apart and says why", move |b, _| {
            let g = group(b, side)?;
            same("blocks", g.blocks.len(), 1)?;
            same("fluid", g.fluid.as_deref(), Some(LAVA))?;
            same("another fluid touches it", g.mixed, true)
        })
}

/// A stone cup round `cells` (one block deep) with a floor under them, and `fluid` in them.
fn pool(b: &mut dyn Bench, cells: &[Pos], fluid: &str) -> Result<(), String> {
    for &c in cells {
        b.place(at(c, [0, -1, 0]), STONE, &[])?;
        for f in [Face::XNeg, Face::XPos, Face::ZNeg, Face::ZPos] {
            let wall = crate::geom::step(c, f);
            if !cells.contains(&wall) {
                b.place(wall, STONE, &[])?;
            }
        }
    }
    for &c in cells {
        b.place(c, fluid, &[])?;
    }
    Ok(())
}

/// A pump over the middle of `cells`, a crystal on it and a tank beside it; the bucket reaches
/// the tank in the rules' time.
fn pump_scenario(
    o: Pos,
    t: &Tuning,
    name: &'static str,
    cells: Vec<Pos>,
    fluid: &'static str,
    after: fn(&dyn Bench, Pos) -> Result<(), String>,
) -> Scenario {
    let source = o;
    let pump = at(o, [0, 1, 0]);
    let tank = at(pump, [0, 0, 1]);
    let ticks = expect::pump_ticks(t, t.crystal_output);
    Scenario::new(name, plot_area(o))
        .then(
            "build a pool, a pump on a crystal and a tank",
            move |b, _| {
                pool(b, &cells, fluid)?;
                b.place(pump, PUMP, &[])?;
                b.place(at(pump, [0, 1, 0]), CRYSTAL, &[])?;
                b.place(tank, TANK, &[])
            },
        )
        .until("a bucket is in the tank", 600, move |b, m| {
            let g = group(b, tank)?;
            let done = g.fluid.as_deref() == Some(fluid) && g.amount >= BUCKET;
            if done {
                m.mark("done", m.elapsed as f64);
            }
            Ok(done)
        })
        .check("it took the rules' time", move |_, m| {
            let ticks = ticks.ok_or("the rules say the pump never pumps")? as f64;
            near(
                "ticks",
                m.get("done"),
                ticks + FLUID_SLACK / 2.0,
                FLUID_SLACK / 2.0 + 1.0,
            )
        })
        .check("the source is as the rules say", move |b, _| {
            after(b, source)
        })
}

fn pump_keeps_endless_water(o: Pos, t: &Tuning) -> Scenario {
    let cells = vec![at(o, [-1, 0, 0]), o, at(o, [1, 0, 0])];
    pump_scenario(
        o,
        t,
        "fluids_pump_keeps_endless_water",
        cells,
        WATER,
        |b, p| {
            same("block", b.block(p), WATER.to_owned())?;
            same("level", b.property(p, "level"), Some("0".to_owned()))
        },
    )
}

fn pump_drains_lava(o: Pos, t: &Tuning) -> Scenario {
    pump_scenario(o, t, "fluids_pump_drains_lava", vec![o], LAVA, |b, p| {
        same("block", b.block(p), "minecraft:air".to_owned())
    })
}

/// Water and lava tanks giving into one fluid net with two empty tanks to fill.
fn two_sources(b: &mut dyn Bench, o: Pos, tier: u8) -> Result<(), String> {
    line(
        b,
        o,
        1,
        6,
        [channel(ChannelKind::Fluid, tier), None, None, None],
    )?;
    for (k, fluid) in [(1, WATER), (3, LAVA)] {
        let tank = at(o, [k, 0, -1]);
        b.place(tank, TANK, &[])?;
        b.set_tank(tank, fluid, 5 * BUCKET)?;
        b.set_joint(
            at(o, [k, 0, 0]),
            Face::ZNeg,
            0,
            JointSetting::of_mode(Mode::From),
        )?;
    }
    b.place(at(o, [5, 0, -1]), TANK, &[])?;
    b.place(at(o, [6, 0, 1]), TANK, &[])
}

/// How much each source tank gave since it was filled, and which says it found no lane.
fn sources(b: &dyn Bench, o: Pos) -> Result<[(u64, bool); 2], String> {
    let mut out = [(0, false); 2];
    for (i, k) in [1, 3].into_iter().enumerate() {
        let tank = at(o, [k, 0, -1]);
        out[i] = (
            5 * BUCKET - group(b, tank)?.amount,
            b.no_lane(tank).is_some(),
        );
    }
    Ok(out)
}

fn one_fluid_on_tier_1(o: Pos, t: &Tuning) -> Scenario {
    let kinds = t.kinds_of(1);
    Scenario::new("fluids_tier_1_channel_carries_one_fluid", plot_area(o))
        .then(
            "build water and lava sources on a tier-1 fluid net",
            move |b, _| two_sources(b, o, 1),
        )
        .wait(40)
        .check("one fluid flows, the other has no lane", move |b, _| {
            same("fluids a tier-1 channel carries", kinds, 1)?;
            let [water, lava] = sources(b, o)?;
            let flowing = [water, lava].iter().filter(|(gave, _)| *gave > 0).count();
            let refused = [water, lava].iter().filter(|(_, no)| *no).count();
            same("fluids that flowed", flowing, 1)?;
            same("sources told «no lane»", refused, 1)?;
            same(
                "the refused one gave nothing",
                [water, lava].iter().any(|(g, n)| *n && *g > 0),
                false,
            )
        })
}

fn two_fluids_on_tier_2(o: Pos, t: &Tuning) -> Scenario {
    let kinds = t.kinds_of(2);
    Scenario::new("fluids_tier_2_channel_carries_two", plot_area(o))
        .then(
            "build water and lava sources on a tier-2 fluid net",
            move |b, _| two_sources(b, o, 2),
        )
        .wait(40)
        .check("both flow", move |b, _| {
            if kinds < 2 {
                return Err(format!("a tier-2 channel carries {kinds} fluids"));
            }
            let [water, lava] = sources(b, o)?;
            same(
                "water flowed, lava flowed",
                (water.0 > 0, lava.0 > 0),
                (true, true),
            )?;
            same("no lane refused", (water.1, lava.1), (false, false))
        })
}
