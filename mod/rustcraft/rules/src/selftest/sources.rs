//! Slice 8: the energy sources and the battery. Solar panels by daylight as one structure through
//! one housing (E13, E18–E21), a windmill by its height and with its blades blocked (E14), the lava
//! generator burning lava from a tank it touches and from a fluid channel, a battery structure
//! charging, feeding a furnace and giving faster with an output module (E11, A7), and crystal
//! structures with and without solution and of a broken form (E7–E9). Expected numbers come from
//! `crate::power` on `Tuning`; a panel's from the time and rain the world has at that moment, so
//! the scenario changes neither for the other mods.

use super::magistral::{channel, line};
use super::{Bench, Build, Scenario, at, expect, near, plot_area, same};
use crate::geom::{Face, Pos};
use crate::machines::processor::Shown as Machine;
use crate::modules::Module;
use crate::net::items::Mode;
use crate::net::settings::JointSetting;
use crate::net::topology::ChannelKind;
use crate::power::{self as rules, Form, PowerView, Shown};
use crate::tuning::Tuning;

pub const ALL: [Build; 11] = [
    solar_by_daylight,
    windmill_by_height,
    lava_from_a_tank,
    lava_through_a_channel,
    battery_charges,
    battery_feeds_a_furnace,
    battery_output_module,
    crystal_pair_unfed,
    crystal_pair_fed,
    crystal_square_fed,
    crystal_broken_form,
];

const SOLAR: &str = "rustcraft:solar";
const WINDMILL: &str = "rustcraft:windmill";
const LAVA_GEN: &str = "rustcraft:gen_lava";
const BATTERY: &str = "rustcraft:battery";
const TANK: &str = "rustcraft:tank";
const CRYSTAL: &str = "rustcraft:crystal";
const FURNACE: &str = "rustcraft:furnace";
const CRUSHER: &str = "rustcraft:crusher";
const STONE: &str = "minecraft:stone";
const RAW_IRON: &str = "minecraft:raw_iron";
const LIMIT: u64 = 200;
/// Ticks a battery's or a lava generator's numbers are watched over.
const SPAN: u64 = 100;

fn view(b: &dyn Bench, pos: Pos) -> Result<PowerView, String> {
    b.power(pos)
        .ok_or_else(|| format!("nothing known of the source at {pos:?}"))
}

/// A furnace with work at `pos` (raw iron in its input slot).
fn furnace(b: &mut dyn Bench, pos: Pos, block: &str) -> Result<(), String> {
    b.place(pos, block, &[])?;
    let slot = b.inputs(pos).first().copied().ok_or("no input slot")?;
    b.put(pos, slot, RAW_IRON, 64)
}

/// The furnace at `pos` shows the rules' state on `supply` E/s of its `request`.
fn fed(b: &dyn Bench, pos: Pos, supply: f64, request: f64) -> bool {
    b.shown(pos) == Some(expect::working_shown(expect::share(supply, request)))
}

fn solar_by_daylight(o: Pos, t: &Tuning) -> Scenario {
    // A node and two housings with an energy channel; panels at y+1 over the second housing and
    // along x; the furnace under the first housing. A fourth panel lies under stone.
    let panels = [at(o, [2, 1, 0]), at(o, [3, 1, 0]), at(o, [4, 1, 0])];
    let shaded = at(o, [10, 1, 0]);
    let oven = at(o, [1, -1, 0]);
    let t1 = t.clone();
    let t2 = t.clone();
    Scenario::new("power_solar_panels_by_daylight", plot_area(o))
        .then("three panels on a housing, one under stone", move |b, _| {
            line(
                b,
                o,
                1,
                2,
                [channel(ChannelKind::Energy, 1), None, None, None],
            )?;
            for p in panels {
                b.place(p, SOLAR, &[])?;
            }
            b.place(shaded, SOLAR, &[])?;
            b.place(at(shaded, [0, 1, 0]), STONE, &[])?;
            furnace(b, oven, FURNACE)
        })
        .until(
            "E18, E20: one structure of three gives the rules' sum",
            LIMIT,
            move |b, _| {
                let (day, rain) = b.sky();
                let each = rules::solar_output(true, day, rain, &t1);
                let v = view(b, panels[2])?;
                Ok(v.blocks == 3
                    && (v.stored - 3.0 * each).abs() < 1e-6
                    && v.shown == Shown::Gives(v.stored))
            },
        )
        .until(
            "the furnace by the far housing gets it through the first panel",
            LIMIT,
            move |b, _| {
                let (day, rain) = b.sky();
                let total = 3.0 * rules::solar_output(true, day, rain, &t2);
                Ok(fed(b, oven, total, t2.furnace_energy))
            },
        )
        .until(
            "E21: the panel under stone has no sky",
            LIMIT,
            move |b, _| Ok(view(b, shaded)?.shown == Shown::NoSky),
        )
        .check("E19: under a panel the sky light is whole", move |b, _| {
            same(
                "sky light under a panel",
                b.light(at(panels[1], [0, -1, 0])).1,
                15,
            )
        })
}

fn windmill_by_height(o: Pos, t: &Tuning) -> Scenario {
    let mill = at(o, [2, 0, 0]);
    let blocked = at(o, [10, 0, 0]);
    let oven = at(o, [3, 0, 0]);
    let out = rules::windmill_output(mill[1], true, t);
    let need = t.furnace_energy;
    Scenario::new("power_windmill_by_height", plot_area(o))
        .then(
            "a windmill by a furnace, another under a block",
            move |b, _| {
                b.place(mill, WINDMILL, &[])?;
                furnace(b, oven, FURNACE)?;
                b.place(blocked, WINDMILL, &[])?;
                b.place(at(blocked, [1, 1, 0]), STONE, &[])
            },
        )
        .until(
            "E14: it gives the rules' output at its height",
            LIMIT,
            move |b, _| Ok(view(b, mill).is_ok_and(|v| (v.output - out).abs() < 1e-6)),
        )
        .until("the furnace runs on it", LIMIT, move |b, _| {
            Ok(fed(b, oven, out, need))
        })
        .until(
            "a block in its blades' way stops the other",
            LIMIT,
            move |b, _| Ok(view(b, blocked)?.shown == Shown::Blocked),
        )
}

fn lava_from_a_tank(o: Pos, t: &Tuning) -> Scenario {
    let tank = at(o, [1, 0, 0]);
    let generator = at(o, [2, 0, 0]);
    let oven = at(o, [3, 0, 0]);
    let need = t.furnace_energy;
    let burnt = rules::lava_burnt(need, SPAN as f64 / t.ticks_per_second, t);
    Scenario::new("power_lava_generator_burns_a_tank", plot_area(o))
        .then(
            "a tank of two buckets of lava, the generator, a furnace",
            move |b, _| {
                b.place(tank, TANK, &[])?;
                b.set_tank(tank, rules::LAVA, 2000)?;
                b.place(generator, LAVA_GEN, &[])?;
                furnace(b, oven, FURNACE)
            },
        )
        .until(
            "the generator drank the tank and the furnace works",
            LIMIT,
            move |b, _| {
                let empty = b.tank(tank).is_some_and(|g| g.amount == 0);
                Ok(empty && b.shown(oven) == Some(Machine::Working))
            },
        )
        .then("note its lava", move |b, m| {
            m.mark("lava", view(b, generator)?.fluid);
            Ok(())
        })
        .wait(SPAN)
        .check(
            "it burnt lava only for what the furnace took",
            move |b, m| {
                let gone = m.get("lava") - view(b, generator)?.fluid;
                near("lava burnt, mB", gone, burnt, burnt * 0.05 + 0.2)
            },
        )
}

fn lava_through_a_channel(o: Pos, _: &Tuning) -> Scenario {
    // The tank beside the first housing gives («из блока»), the generator beside the second takes
    // by default; they do not touch.
    let tank = at(o, [1, 0, 1]);
    let generator = at(o, [2, 0, -1]);
    Scenario::new("power_lava_through_a_channel", plot_area(o))
        .then(
            "a tank and the generator on a fluid channel",
            move |b, _| {
                line(
                    b,
                    o,
                    1,
                    2,
                    [channel(ChannelKind::Fluid, 1), None, None, None],
                )?;
                b.place(tank, TANK, &[])?;
                b.set_tank(tank, rules::LAVA, 1000)?;
                b.set_joint(
                    at(o, [1, 0, 0]),
                    Face::ZPos,
                    0,
                    JointSetting::of_mode(Mode::From),
                )?;
                b.place(generator, LAVA_GEN, &[])
            },
        )
        .until("its joint takes the lava", LIMIT * 2, move |b, _| {
            Ok(view(b, generator).is_ok_and(|v| v.fluid >= 999.0))
        })
}

fn battery_charges(o: Pos, t: &Tuning) -> Scenario {
    let a = at(o, [2, 0, 0]);
    let c = at(o, [3, 0, 0]);
    let capacity = rules::battery(2, &[Some(Module::StoreCapacity)], t).capacity;
    let gain = t.crystal_output * SPAN as f64 / t.ticks_per_second;
    let tick = t.crystal_output / t.ticks_per_second;
    Scenario::new("power_battery_charges_from_a_crystal", plot_area(o))
        .then(
            "a crystal by two batteries, one with a capacity module",
            move |b, _| {
                b.place(at(o, [1, 0, 0]), CRYSTAL, &[])?;
                b.place(a, BATTERY, &[])?;
                b.place(c, BATTERY, &[])?;
                b.set_modules(c, &[Module::StoreCapacity.item()])
            },
        )
        .until(
            "E11, A6: one structure, its capacity with the module",
            LIMIT,
            move |b, m| {
                let v = view(b, a)?;
                m.mark("stored", v.stored);
                Ok(v.blocks == 2 && (v.capacity - capacity).abs() < 1e-6)
            },
        )
        .wait(SPAN)
        .check("it took all the crystal gave", move |b, m| {
            near(
                "stored",
                view(b, c)?.stored - m.get("stored"),
                gain,
                2.0 * tick,
            )
        })
}

fn battery_feeds_a_furnace(o: Pos, t: &Tuning) -> Scenario {
    let battery = at(o, [2, 0, 0]);
    let oven = at(o, [3, 0, 0]);
    let used = t.furnace_energy * SPAN as f64 / t.ticks_per_second;
    let tick = t.furnace_energy / t.ticks_per_second;
    Scenario::new("power_battery_feeds_a_furnace", plot_area(o))
        .then("a charged battery by a furnace", move |b, _| {
            b.place(battery, BATTERY, &[])?;
            b.set_buffer(battery, 1000.0)?;
            furnace(b, oven, FURNACE)
        })
        .until("the furnace works on the battery", LIMIT, move |b, m| {
            m.mark("stored", view(b, battery).map_or(0.0, |v| v.stored));
            Ok(b.shown(oven) == Some(Machine::Working))
        })
        .wait(SPAN)
        .check("the battery gave what the furnace took", move |b, m| {
            near(
                "given",
                m.get("stored") - view(b, battery)?.stored,
                used,
                2.0 * tick,
            )
        })
}

fn battery_output_module(o: Pos, t: &Tuning) -> Scenario {
    let battery = at(o, [2, 0, 0]);
    let oven = at(o, [3, 0, 0]);
    let crusher = at(o, [1, 0, 0]);
    let boost = crate::machines::processor::WorkMode::Boost.code() as i32;
    let asked = 3.0 * t.furnace_energy + 3.0 * t.crusher_energy;
    let rate = rules::battery(1, &[], t).give;
    let faster = rules::battery(1, &[Some(Module::Output)], t).give;
    let need = 3.0 * t.furnace_energy;
    Scenario::new("power_battery_output_module", plot_area(o))
        .then(
            "a battery between a furnace and a crusher in boost",
            move |b, _| {
                b.place(battery, BATTERY, &[])?;
                b.set_buffer(battery, 5000.0)?;
                furnace(b, oven, FURNACE)?;
                furnace(b, crusher, CRUSHER)?;
                b.press(oven, boost, None)?;
                b.press(crusher, boost, None)
            },
        )
        .until(
            "A7: without a module it gives no faster than its rate",
            LIMIT,
            move |b, _| Ok(fed(b, oven, rate * need / asked, need)),
        )
        .then("an output module", move |b, _| {
            b.set_modules(battery, &[Module::Output.item()])
        })
        .check("the module's rate covers both", move |_, _| {
            if faster >= asked - 1e-9 {
                Ok(())
            } else {
                Err(format!("{faster} E/s for {asked}"))
            }
        })
        .until(
            "with it both machines get all they ask",
            LIMIT,
            move |b, _| {
                Ok(b.shown(oven) == Some(Machine::Working)
                    && b.shown(crusher) == Some(Machine::Working))
            },
        )
}

/// Two crystals in a row with a furnace beside the first; `fed` puts a tank of solution by it.
fn crystal_pair(name: &'static str, o: Pos, t: &Tuning, solution: bool) -> Scenario {
    let first = at(o, [2, 0, 0]);
    let oven = at(o, [1, 0, 0]);
    let tank = at(o, [2, 0, 1]);
    let total = rules::structure_output(2, Form::Box([1, 1, 2]), solution, t);
    let wanted = if solution {
        Shown::Gives(total)
    } else {
        Shown::NoSolution(total)
    };
    let need = t.furnace_energy;
    let s = Scenario::new(name, plot_area(o))
        .then("two crystals in a row, a furnace by one", move |b, _| {
            b.place(first, CRYSTAL, &[])?;
            b.place(at(o, [3, 0, 0]), CRYSTAL, &[])?;
            if solution {
                b.place(tank, TANK, &[])?;
                b.set_tank(tank, rules::SOLUTION, 1000)?;
            }
            furnace(b, oven, FURNACE)
        })
        .until(
            "E8, E8.a: the structure shows the rules' output",
            LIMIT,
            move |b, _| Ok(view(b, first).is_ok_and(|v| v.blocks == 2 && v.shown == wanted)),
        )
        .until(
            "the furnace by one crystal gets that crystal's half",
            LIMIT,
            move |b, _| Ok(fed(b, oven, total / 2.0, need)),
        );
    if !solution {
        return s;
    }
    s.until(
        "the tank's solution went into the structure",
        LIMIT,
        move |b, _| Ok(b.tank(tank).is_some_and(|g| g.amount == 0) && view(b, first)?.fluid > 0.0),
    )
}

fn crystal_pair_unfed(o: Pos, t: &Tuning) -> Scenario {
    crystal_pair("power_crystal_pair_without_solution", o, t, false)
}

fn crystal_pair_fed(o: Pos, t: &Tuning) -> Scenario {
    crystal_pair("power_crystal_pair_with_solution", o, t, true)
}

fn crystal_square_fed(o: Pos, t: &Tuning) -> Scenario {
    let cells = [[2, 0, 0], [3, 0, 0], [2, 0, 1], [3, 0, 1]].map(|d| at(o, d));
    let tank = at(o, [1, 0, 0]);
    let total = rules::structure_output(4, Form::Box([1, 2, 2]), true, t);
    let drink = rules::solution_per_second(4, t) * SPAN as f64 / t.ticks_per_second;
    Scenario::new("power_crystal_square_with_solution", plot_area(o))
        .then(
            "four crystals in a square and a tank of solution",
            move |b, _| {
                for c in cells {
                    b.place(c, CRYSTAL, &[])?;
                }
                b.place(tank, TANK, &[])?;
                b.set_tank(tank, rules::SOLUTION, 1000)
            },
        )
        .until("E7, E8: a 2×2 gives 32 E/s", LIMIT, move |b, m| {
            let v = view(b, cells[3])?;
            m.mark("solution", v.fluid);
            Ok(v.shown == Shown::Gives(total) && v.fluid > 0.0)
        })
        .wait(SPAN)
        .check(
            "spec 12.3: it drinks 0.1 bucket a minute a crystal",
            move |b, m| {
                near(
                    "drunk, mB",
                    m.get("solution") - view(b, cells[0])?.fluid,
                    drink,
                    0.5,
                )
            },
        )
}

fn crystal_broken_form(o: Pos, t: &Tuning) -> Scenario {
    let cells = [[2, 0, 0], [3, 0, 0], [3, 0, 1]].map(|d| at(o, d));
    let one = t.crystal_output;
    Scenario::new("power_crystal_broken_form", plot_area(o))
        .then("three crystals in an L", move |b, _| {
            for c in cells {
                b.place(c, CRYSTAL, &[])?;
            }
            Ok(())
        })
        .until(
            "it shows a broken form and gives one crystal's",
            LIMIT,
            move |b, _| Ok(view(b, cells[0]).is_ok_and(|v| v.shown == Shown::BrokenShape(one))),
        )
}
