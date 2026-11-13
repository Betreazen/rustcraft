//! Slices 2 and 4: energy and the machines on it. The electric furnace on a crystal by contact
//! (E6, E10, E3: half the energy, half the speed) and on two crystals through an energy channel
//! (E2), the fuel generator burning for a furnace, the alloy smelter, the crusher, the
//! autocrafter's pattern, and a station's buffer filling.

use super::magistral::{channel, line};
use super::{Bench, Build, Memo, Scenario, at, count_of, expect, near, plot_area, same};
use crate::geom::Pos;
use crate::machines::alloy::AlloyRecipe;
use crate::machines::windows::crafter::CELL_BUTTON;
use crate::net::topology::ChannelKind;
use crate::tuning::Tuning;

pub const ALL: [Build; 7] = [
    furnace_on_one_crystal,
    furnace_through_a_channel,
    generator_burns_for_a_furnace,
    alloy_smelter,
    crusher,
    autocrafter,
    station_charges,
];

pub const CRYSTAL: &str = "rustcraft:crystal";
const FURNACE: &str = "rustcraft:furnace";
const GENERATOR: &str = "rustcraft:gen_fuel";
const ALLOY: &str = "rustcraft:alloy";
const CRUSHER: &str = "rustcraft:crusher";
const CRAFTER: &str = "rustcraft:crafter";
pub const STATION: &str = "rustcraft:station";
const RAW_IRON: &str = "minecraft:raw_iron";
const IRON: &str = "minecraft:iron_ingot";
const COAL: &str = "minecraft:coal";
const LOG: &str = "minecraft:oak_log";
/// The alloy the smelter's scenario makes, when the content mod has its recipe.
const SPARKSTEEL: &str = "rustcraft:sparksteel";
/// How far past the rules' tick count a machine may finish: the tick the world is read again in,
/// and the tick its result is seen in.
const SLACK: f64 = 2.0;

/// The input slot `k` of the machine at `pos`.
fn input(b: &dyn Bench, pos: Pos, k: usize) -> Result<usize, String> {
    b.inputs(pos)
        .get(k)
        .copied()
        .ok_or_else(|| format!("the machine at {pos:?} has no input {k}"))
}

/// Notes when a machine at `pos` first holds `wanted` of `item`, and what it showed meanwhile.
fn finished(b: &dyn Bench, m: &mut Memo, pos: Pos, item: &str, wanted: u32) -> bool {
    if let Some(shown) = b.shown(pos)
        && !m.marks.contains_key("shown")
        && m.elapsed > 2
    {
        m.mark("shown", shown_code(shown));
    }
    let done = count_of(&b.items(pos), item) >= wanted;
    if done {
        m.mark("done", m.elapsed as f64);
    }
    done
}

/// A `Shown` as a number for the memo: its percent while slowed, 100 while working, else -1.
fn shown_code(shown: crate::machines::processor::Shown) -> f64 {
    use crate::machines::processor::Shown;
    match shown {
        Shown::Working => 100.0,
        Shown::Slowed { percent } => f64::from(percent),
        _ => -1.0,
    }
}

/// The machine took the rules' ticks and showed the rules' state while it worked.
fn took(m: &Memo, ticks: Option<u64>, supply: f64, request: f64) -> Result<(), String> {
    let ticks = ticks.ok_or("the rules say it never finishes")?;
    near("ticks", m.get("done"), ticks as f64, SLACK)?;
    same(
        "shown while working",
        m.get("shown"),
        shown_code(expect::working_shown(expect::share(supply, request))),
    )
}

fn furnace_on_one_crystal(o: Pos, t: &Tuning) -> Scenario {
    let (supply, request) = (t.crystal_output, t.furnace_energy);
    let tuning = t.clone();
    Scenario::new("energy_crystal_slows_the_furnace", plot_area(o))
        .then("build a furnace touching one crystal", move |b, m| {
            b.place(o, FURNACE, &[])?;
            b.place(at(o, [1, 0, 0]), CRYSTAL, &[])?;
            b.put(o, input(b, o, 0)?, RAW_IRON, 1)?;
            m.mark(
                "cook",
                b.cook_ticks(RAW_IRON).ok_or("raw iron does not smelt")?,
            );
            Ok(())
        })
        .until("the ingot is out", 600, move |b, m| {
            Ok(finished(b, m, o, IRON, 1))
        })
        .check("it took the rules' time on half the energy", move |_, m| {
            took(
                m,
                expect::furnace_ticks(&tuning, m.get("cook"), supply, 1),
                supply,
                request,
            )
        })
}

fn furnace_through_a_channel(o: Pos, t: &Tuning) -> Scenario {
    let supply = (2.0 * t.crystal_output).min(t.energy_of(1));
    let request = t.furnace_energy;
    let furnace = at(o, [6, 0, -1]);
    let tuning = t.clone();
    Scenario::new("energy_channel_feeds_the_furnace", plot_area(o))
        .then(
            "build two crystals and a furnace on an energy net",
            move |b, m| {
                line(
                    b,
                    o,
                    1,
                    6,
                    [channel(ChannelKind::Energy, 1), None, None, None],
                )?;
                b.place(at(o, [1, 0, -1]), CRYSTAL, &[])?;
                b.place(at(o, [1, 0, 1]), CRYSTAL, &[])?;
                b.place(furnace, FURNACE, &[])?;
                b.put(furnace, input(b, furnace, 0)?, RAW_IRON, 1)?;
                m.mark(
                    "cook",
                    b.cook_ticks(RAW_IRON).ok_or("raw iron does not smelt")?,
                );
                Ok(())
            },
        )
        .until("the ingot is out", 600, move |b, m| {
            Ok(finished(b, m, furnace, IRON, 1))
        })
        .check(
            "it took the rules' time on the net's energy",
            move |_, m| {
                took(
                    m,
                    expect::furnace_ticks(&tuning, m.get("cook"), supply, 1),
                    supply,
                    request,
                )
            },
        )
}

fn generator_burns_for_a_furnace(o: Pos, t: &Tuning) -> Scenario {
    let (supply, request) = (t.generator_output, t.furnace_energy);
    let furnace = at(o, [1, 0, 0]);
    let tuning = t.clone();
    Scenario::new("energy_generator_burns_for_a_furnace", plot_area(o))
        .then(
            "build a generator with coal beside a furnace",
            move |b, m| {
                b.place(o, GENERATOR, &[])?;
                b.place(furnace, FURNACE, &[])?;
                b.put(o, input(b, o, 0)?, COAL, 4)?;
                b.put(furnace, input(b, furnace, 0)?, RAW_IRON, 1)?;
                m.mark(
                    "cook",
                    b.cook_ticks(RAW_IRON).ok_or("raw iron does not smelt")?,
                );
                Ok(())
            },
        )
        .until("the ingot is out", 600, move |b, m| {
            Ok(finished(b, m, furnace, IRON, 1))
        })
        .check("the furnace ran at full speed", move |_, m| {
            took(
                m,
                expect::furnace_ticks(&tuning, m.get("cook"), supply, 1),
                supply,
                request,
            )
        })
        .check("the generator lit one coal", move |b, _| {
            same("coal left", count_of(&b.items(o), COAL), 3)
        })
}

fn alloy_smelter(o: Pos, t: &Tuning) -> Scenario {
    let supply = 2.0 * t.crystal_output;
    let request = t.alloy_energy;
    let tuning = t.clone();
    Scenario::new("energy_alloy_smelter_makes_its_alloy", plot_area(o))
        .then(
            "build a smelter between two crystals, with a recipe's inputs",
            move |b, _| {
                let recipe = chosen_recipe(&b.alloy_recipes())?;
                b.place(o, ALLOY, &[])?;
                b.place(at(o, [1, 0, 0]), CRYSTAL, &[])?;
                b.place(at(o, [-1, 0, 0]), CRYSTAL, &[])?;
                for (k, (item, n)) in recipe.inputs.iter().enumerate() {
                    b.put(o, input(b, o, k)?, item, *n)?;
                }
                Ok(())
            },
        )
        .until("the alloy is out", 600, move |b, m| {
            let (item, n) = chosen_recipe(&b.alloy_recipes())?.output;
            Ok(finished(b, m, o, &item, n))
        })
        .check(
            "it took the rules' time and used the inputs",
            move |b, m| {
                let recipe = chosen_recipe(&b.alloy_recipes())?;
                took(
                    m,
                    expect::alloy_ticks(&tuning, &recipe, supply),
                    supply,
                    request,
                )?;
                for (item, _) in &recipe.inputs {
                    same(&format!("{item} left"), count_of(&b.items(o), item), 0)?;
                }
                Ok(())
            },
        )
}

/// The recipe the smelter's scenario runs: sparksteel's, else the first there is.
fn chosen_recipe(recipes: &[AlloyRecipe]) -> Result<AlloyRecipe, String> {
    recipes
        .iter()
        .find(|r| r.output.0 == SPARKSTEEL)
        .or(recipes.first())
        .cloned()
        .ok_or_else(|| "the content mod has no alloying recipe".into())
}

fn crusher(o: Pos, t: &Tuning) -> Scenario {
    let supply = 2.0 * t.crystal_output;
    let request = t.crusher_energy;
    Scenario::new("energy_crusher_crushes_raw_iron", plot_area(o))
        .then(
            "build a crusher between two crystals with raw iron",
            move |b, m| {
                let (_, count, ticks) =
                    b.crush(RAW_IRON).ok_or("no crushing recipe for raw iron")?;
                m.mark("count", f64::from(count));
                m.mark("ticks", ticks);
                b.place(o, CRUSHER, &[])?;
                b.place(at(o, [1, 0, 0]), CRYSTAL, &[])?;
                b.place(at(o, [-1, 0, 0]), CRYSTAL, &[])?;
                b.put(o, input(b, o, 0)?, RAW_IRON, 1)
            },
        )
        .until("the dust is out", 600, move |b, m| {
            let (dust, count, _) = b.crush(RAW_IRON).ok_or("no crushing recipe for raw iron")?;
            Ok(finished(b, m, o, &dust, count))
        })
        .check("it took the recipe's time", move |_, m| {
            let ticks = expect::processor_ticks(m.get("ticks"), request, supply, 1);
            took(m, ticks, supply, request)
        })
}

fn autocrafter(o: Pos, t: &Tuning) -> Scenario {
    const CRAFTS: u32 = 2;
    let supply = 2.0 * t.crystal_output;
    let request = t.crafter_energy;
    let tuning = t.clone();
    let mut grid: [Option<&'static str>; 9] = [None; 9];
    grid[4] = Some(LOG);
    Scenario::new("energy_autocrafter_crafts_its_pattern", plot_area(o))
        .then("build a crafter with a pattern and logs", move |b, _| {
            b.place(o, CRAFTER, &[])?;
            b.place(at(o, [1, 0, 0]), CRYSTAL, &[])?;
            b.place(at(o, [-1, 0, 0]), CRYSTAL, &[])?;
            b.press(o, CELL_BUTTON + 4, Some((LOG, 1)))?;
            b.put(o, input(b, o, 0)?, LOG, CRAFTS)
        })
        .until("both crafts are out", 400, move |b, m| {
            let (made, n) = b.craft(&grid).ok_or("a log makes nothing")?;
            Ok(finished(b, m, o, &made, n * CRAFTS))
        })
        .check("it took the rules' time", move |b, m| {
            let (_, n) = b.craft(&grid).ok_or("a log makes nothing")?;
            took(
                m,
                expect::crafter_ticks(&tuning, n, CRAFTS, supply),
                supply,
                request,
            )?;
            same("logs left", count_of(&b.items(o), LOG), 0)
        })
}

fn station_charges(o: Pos, t: &Tuning) -> Scenario {
    const SPAN: u64 = 40;
    let wanted = expect::station_gain(t, t.crystal_output, SPAN);
    // One tick's charge either way.
    let tolerance = expect::station_gain(t, t.crystal_output, 1) + 1e-6;
    Scenario::new("energy_station_charges", plot_area(o))
        .then("build a station touching a crystal", move |b, _| {
            b.place(o, STATION, &[])?;
            b.place(at(o, [1, 0, 0]), CRYSTAL, &[])
        })
        .until("it starts charging", 100, move |b, m| {
            let e = b.station_energy(o).ok_or("no station")?;
            m.mark("start", e);
            Ok(e > 0.0)
        })
        .wait(SPAN)
        .check("it gained the crystal's output", move |b, m| {
            let e = b.station_energy(o).ok_or("no station")?;
            near("E gained", e - m.get("start"), wanted, tolerance)
        })
}
