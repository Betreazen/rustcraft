//! Slice 18, the cross-stream agent: what the streams handed each other after the merge.
//!
//! - The pump has the work modes (owner 2026-11-17): boost pumps a bucket in half the time for
//!   three times the energy, and its window shows the mode.
//! - The placer and the breaker work faster on a higher tier (A8): their windows' pace.
//! - A line-boost module in a node raises a fluid net's flow by half (guide `mod_line`).
//! - The electric furnace and the autocrafter report their work to Path-of-craft-tree (`smelt`,
//!   `craft`).
//! - The alloy smelter, the autocrafter and the pump take a catalyst from a tank they touch, and
//!   the accelerator speeds the pump up (guide `fluids`).
//! - A slice-12 machine that opens on tier 3 or 4 has four module sockets (A5).
//! - A stack module lets each of the kitchen's inputs hold 256 (guide `mod_stack`).
//! - A broken tiered block drops its own tier's item: the loot tables in 26.3's form
//!   (`match_block`), which the old `block_state_property` list was not.

use super::{Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::machines::fluid_windows::pump as pw;
use crate::machines::panel::ENERGY_SCALE;
use crate::machines::processor::WorkMode;
use crate::tuning::Tuning;

pub const ALL: [Build; 8] = [
    pump_boost_mode,
    placer_and_breaker_by_tier,
    tiered_blocks_drop_their_tier,
    line_boost_on_a_fluid_net,
    machines_report_smelt_and_craft,
    catalysts_in_more_machines,
    craft_machines_sockets_by_tier,
    kitchen_stack_module,
];

const STONE: &str = "minecraft:stone";
const LAVA: &str = "minecraft:lava";

fn pump_boost_mode(o: Pos, t: &Tuning) -> Scenario {
    let pump = at(o, [0, 1, 0]);
    let boost_ticks = (t.pump_ticks / 2.0).round() as i32;
    let boost_energy = t.pump_energy * 3.0;
    Scenario::new("s18_pump_boost_mode", plot_area(o))
        .then("a pump over lava, on a crystal", move |b, _| {
            for x in -1..=1 {
                for z in -1..=1 {
                    b.place(at(o, [x, -1, z]), STONE, &[])?;
                    if (x, z) != (0, 0) {
                        b.place(at(o, [x, 0, z]), STONE, &[])?;
                    }
                }
            }
            b.place(o, LAVA, &[])?;
            b.place(pump, "rustcraft:pump", &[])?;
            b.place(at(pump, [0, 1, 0]), "rustcraft:crystal", &[])
        })
        .wait(10)
        .check("it starts in the normal mode", move |b, _| {
            same("mode", pw::mode(&b.menu_data(pump)), WorkMode::Normal)
        })
        .then("the boost button is pressed", move |b, _| {
            b.press(pump, i32::from(WorkMode::Boost.code()), None)
        })
        .wait(10)
        .check("it saved boost, and its window says boost, half the ticks and three times the E", move |b, _| {
            same("saved mode", b.long(pump, "work"), i64::from(WorkMode::Boost.code()))?;
            let data = b.menu_data(pump);
            same("window mode", pw::mode(&data), WorkMode::Boost)?;
            same("ticks a bucket", data.get(pw::TICKS_AT).copied(), Some(boost_ticks))?;
            let asked = f64::from(data.get(4).copied().unwrap_or(0)) / ENERGY_SCALE;
            same("E/s asked", (asked * 10.0).round(), (boost_energy * 10.0).round())
        })
}

/// Seconds as the placer's window writes them: «2», «1,3».
fn secs(v: f64) -> String {
    format!("{v:.1}").trim_end_matches(".0").replace('.', ",")
}

fn pace_shown(b: &dyn super::Bench, pos: Pos, secs: &str) -> bool {
    b.sheet(pos).is_some_and(|s| {
        s.rows.iter().any(|r| {
            let r = r.to_string();
            // A whole number of seconds goes over the wire as a JSON number (`"with":[1,2]`).
            r.contains("rustcraft.placer.pace")
                && (r.contains(&format!("\"{secs}\"")) || r.contains(&format!("[{secs},")))
        })
    })
}

fn placer_and_breaker_by_tier(o: Pos, t: &Tuning) -> Scenario {
    let cells = [
        ("rustcraft:placer", "2", t.placer_ticks, at(o, [1, 1, 0])),
        ("rustcraft:placer", "4", t.placer_ticks, at(o, [3, 1, 0])),
        ("rustcraft:breaker", "2", t.breaker_ticks, at(o, [5, 1, 0])),
        ("rustcraft:breaker", "3", t.breaker_ticks, at(o, [7, 1, 0])),
    ];
    let step = t.tier_speed_step;
    let tps = t.ticks_per_second;
    Scenario::new("s18_placer_and_breaker_by_tier", plot_area(o))
        .then("a placer and a breaker of their first tier and of a higher one", move |b, _| {
            for (block, tier, _, p) in cells {
                b.place(at(p, [0, -1, 0]), STONE, &[])?;
                b.place(p, block, &[("tier", tier), ("facing", "north")])?;
            }
            Ok(())
        })
        .wait(40)
        .check("each window gives the pace of its tier", move |b, _| {
            for &(block, tier, ticks, p) in &cells {
                let above = f64::from(tier.parse::<u8>().unwrap_or(2) - 2);
                let want = secs(ticks / (1.0 + step * above) / tps);
                if !pace_shown(b, p, &want) {
                    let rows: Vec<String> = b.sheet(p).map(|s| s.rows.iter().map(|r| r.to_string()).collect()).unwrap_or_default();
                    return Err(format!(
                        "{block} tier {tier} (placed {:?}): not «{want} s» in {rows:?}",
                        b.property(p, "tier")
                    ));
                }
            }
            Ok(())
        })
}

fn tiered_blocks_drop_their_tier(o: Pos, _: &Tuning) -> Scenario {
    // Every machine family on its top tier, and the node and the monorail (their own tables).
    let blocks: Vec<&'static str> = crate::tiers::FAMILIES
        .iter()
        .map(|f| f.block)
        .chain(["rustcraft:node", "rustcraft:monorail"])
        .collect();
    let cell = move |i: usize| at(o, [(i % 7) as i32, 1, (i / 7) as i32 * 2 - 1]);
    let placed = blocks.clone();
    Scenario::new("s18_tiered_blocks_drop_their_tier", plot_area(o))
        .then("each tiered block placed on tier 4", move |b, _| {
            for (i, block) in placed.iter().enumerate() {
                b.place(at(cell(i), [0, -1, 0]), STONE, &[])?;
                b.place(cell(i), block, &[("tier", "4")])?;
            }
            Ok(())
        })
        .check("broken with a pickaxe, each drops its tier 4 item", move |b, _| {
            for (i, block) in blocks.iter().enumerate() {
                let drops = b.break_block(cell(i), Some("minecraft:iron_pickaxe"))?;
                let want = format!("{block}_4");
                if !drops.iter().any(|(item, n)| *item == want && *n == 1) {
                    return Err(format!("{block} on tier 4 dropped {drops:?}, not {want}"));
                }
            }
            Ok(())
        })
}

/// Ticks each flow is counted over.
const FLOW_WINDOW: u64 = 100;

fn line_boost_on_a_fluid_net(o: Pos, t: &Tuning) -> Scenario {
    use super::magistral::{channel, line};
    use crate::geom::Face;
    use crate::net::fluids::BUCKET;
    use crate::net::items::Mode;
    use crate::net::settings::JointSetting;
    use crate::net::topology::ChannelKind;
    const TANK: &str = "rustcraft:tank";
    const WATER: &str = "minecraft:water";
    // Two stacked tanks each end: 32 buckets, more than both windows move.
    let (from, to) = (at(o, [1, 0, -1]), at(o, [4, 0, -1]));
    let full = 2 * t.tank_buckets * BUCKET;
    let amount = move |b: &dyn super::Bench, p: Pos| b.tank(p).map_or(0, |g| g.amount) as f64;
    let boost = crate::net::node_window::boost(1);
    Scenario::new("s18_line_boost_on_a_fluid_net", plot_area(o))
        .then("a tier-1 fluid line from a full tank to an empty one", move |b, _| {
            line(b, o, 1, 4, [channel(ChannelKind::Fluid, 1), None, None, None])?;
            for p in [from, to] {
                b.place(p, TANK, &[])?;
                b.place(at(p, [0, 1, 0]), TANK, &[])?;
            }
            b.set_tank(from, WATER, full)?;
            b.set_joint(at(o, [1, 0, 0]), Face::ZNeg, 0, JointSetting::of_mode(Mode::From))
        })
        .wait(20)
        .then("count the plain flow", move |b, m| {
            m.mark("plain0", amount(b, to));
            Ok(())
        })
        .wait(FLOW_WINDOW)
        .then("a line-boost module into the node", move |b, m| {
            m.mark("plain", amount(b, to) - m.get("plain0"));
            let n = b.pipe("socket:0", o)?;
            if n != "1" {
                return Err(format!("{n} modules after the press"));
            }
            b.refresh();
            Ok(())
        })
        .wait(20)
        .then("count the boosted flow", move |b, m| {
            m.mark("boost0", amount(b, to));
            Ok(())
        })
        .wait(FLOW_WINDOW)
        .check("the boosted net carries half as much again", move |b, m| {
            let plain = m.get("plain");
            let boosted = amount(b, to) - m.get("boost0");
            if plain <= 0.0 {
                return Err("nothing flowed before the module".into());
            }
            let ratio = boosted / plain;
            if (ratio - boost).abs() <= 0.25 {
                Ok(())
            } else {
                Err(format!("plain {plain} mB, boosted {boosted} mB in {FLOW_WINDOW} ticks: ×{ratio:.2}, not ×{boost}"))
            }
        })
}

/// The owner of the reporting machines.
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0c05;

fn machines_report_smelt_and_craft(o: Pos, _: &Tuning) -> Scenario {
    use crate::machines::windows::crafter::CELL_BUTTON;
    const LOG: &str = "minecraft:oak_log";
    const CRYSTAL: &str = "rustcraft:crystal";
    let furnace = at(o, [1, 1, 0]);
    let crafter = at(o, [5, 1, 0]);
    let owner = format!("{OWNER:032x}");
    Scenario::new("s18_machines_report_smelt_and_craft", plot_area(o))
        .then("a furnace with raw iron and an autocrafter with a log pattern, both owned and powered", move |b, _| {
            for p in [furnace, crafter] {
                b.place(at(p, [0, -1, 0]), STONE, &[])?;
                b.place(at(p, [-1, -1, 0]), STONE, &[])?;
                b.place(at(p, [1, -1, 0]), STONE, &[])?;
            }
            b.place(furnace, "rustcraft:furnace", &[])?;
            b.set_owner(furnace, OWNER, 1)?;
            b.place(at(furnace, [-1, 0, 0]), CRYSTAL, &[])?;
            b.place(at(furnace, [1, 0, 0]), CRYSTAL, &[])?;
            b.put(furnace, 0, "minecraft:raw_iron", 2)?;
            b.place(crafter, "rustcraft:crafter", &[])?;
            b.set_owner(crafter, OWNER, 1)?;
            b.place(at(crafter, [-1, 0, 0]), CRYSTAL, &[])?;
            b.place(at(crafter, [1, 0, 0]), CRYSTAL, &[])?;
            b.press(crafter, CELL_BUTTON + 4, Some((LOG, 1)))?;
            let k = b.inputs(crafter).first().copied().ok_or("the crafter has no input")?;
            b.put(crafter, k, LOG, 1)
        })
        .until("Path-of-craft-tree heard a smelt of iron and a craft of planks", 600, move |b, _| {
            let lines = b.reports();
            let smelt = lines.iter().any(|r| r.contains(&format!("smelt {owner} minecraft:iron_ingot ")));
            let craft = lines
                .iter()
                .any(|r| r.contains(&format!("craft {owner} ")) && r.contains("minecraft:oak_planks 4 1"));
            Ok(smelt && craft)
        })
}

fn catalysts_in_more_machines(o: Pos, t: &Tuning) -> Scenario {
    use crate::machines::catalyst::{ACCELERATOR, Catalyst};
    let machines = [
        ("rustcraft:alloy", at(o, [1, 1, 0])),
        ("rustcraft:crafter", at(o, [4, 1, 0])),
        ("rustcraft:pump", at(o, [7, 1, 0])),
    ];
    let pump = machines[2].1;
    let fast = (t.pump_ticks / t.accelerator_speed).round() as i32;
    Scenario::new("s18_catalysts_in_more_machines", plot_area(o))
        .then("a smelter, an autocrafter and a pump, each beside a tank of accelerator", move |b, _| {
            for (block, p) in machines {
                b.place(at(p, [0, -1, 0]), STONE, &[])?;
                b.place(at(p, [0, -1, 1]), STONE, &[])?;
                b.place(p, block, &[])?;
                let tank = at(p, [0, 0, 1]);
                b.place(tank, "rustcraft:tank", &[])?;
                b.set_tank(tank, ACCELERATOR, 3000)?;
            }
            Ok(())
        })
        .until("each holds the accelerator", 100, move |b, _| {
            Ok(machines
                .iter()
                .all(|(_, p)| b.catalyst(*p).is_some_and(|c| c.active() == Some(Catalyst::Accelerator))))
        })
        .wait(10)
        .check("the pump's bucket takes the accelerator's time", move |b, _| {
            same("ticks a bucket", b.menu_data(pump).get(pw::TICKS_AT).copied(), Some(fast))
        })
}

fn craft_machines_sockets_by_tier(o: Pos, _: &Tuning) -> Scenario {
    let cells = [
        ("rustcraft:kitchen", 3usize, at(o, [1, 1, 0])),
        ("rustcraft:brewer", 4, at(o, [3, 1, 0])),
        ("rustcraft:enchanter", 4, at(o, [5, 1, 0])),
    ];
    Scenario::new("s18_craft_machines_sockets_by_tier", plot_area(o))
        .then("a kitchen (tier 2), a brewer (tier 3) and an enchanter (tier 4)", move |b, _| {
            for (block, _, p) in cells {
                b.place(at(p, [0, -1, 0]), STONE, &[])?;
                b.place(p, block, &[])?;
            }
            Ok(())
        })
        .wait(10)
        .check("their windows have 3, 4 and 4 sockets", move |b, _| {
            for (block, n, p) in cells {
                same(block, b.sheet(p).map(|s| s.modules.len()), Some(n))?;
            }
            Ok(())
        })
}

fn kitchen_stack_module(o: Pos, _: &Tuning) -> Scenario {
    const WHEAT: &str = "minecraft:wheat";
    let p = at(o, [3, 1, 0]);
    // More than its nine slots of 64 hold without the module.
    const MORE: u32 = 9 * 64 + 128;
    Scenario::new("s18_kitchen_stack_module", plot_area(o))
        .then("a kitchen with a stack module, no energy", move |b, _| {
            b.place(at(p, [0, -1, 0]), STONE, &[])?;
            b.place(p, "rustcraft:kitchen", &[])?;
            b.set_modules(p, &[crate::modules::Module::Stack.item()])
        })
        .until("it took more wheat than nine slots of 64", 400, move |b, m| {
            let got = b.insert(p, WHEAT, 64)?;
            m.mark("taken", m.get("taken") + f64::from(got));
            Ok(m.get("taken") >= f64::from(MORE))
        })
        .check("and keeps it: what its slots show is less than it took", move |b, m| {
            let shown = super::count_of(&b.items(p), WHEAT);
            if f64::from(shown) < m.get("taken") {
                Ok(())
            } else {
                Err(format!("its slots show {shown} of {}", m.get("taken")))
            }
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_loot_table_keeps_the_old_condition_list() {
        // 26.3 reads one `condition` object; `conditions` with `block_state_property` never held.
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/data/rustcraft/loot_table/blocks");
        let old: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                std::fs::read_to_string(e.path())
                    .is_ok_and(|t| t.contains("block_state_property") || t.contains("\"conditions\""))
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(old.is_empty(), "{old:?}");
    }
}
