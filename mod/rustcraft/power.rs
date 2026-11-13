// Slice 8's energy on the server (spec 4.2, 18.2, 21, 12.3): the structures of crystals (E7–E9),
// solar panels (E13, E18–E21), windmills (E14), the lava generator and the battery (E11, A6, A7).
// rustcraft_rules::power decides what each offers; rustcraft_machines shares it out with the
// rest (net::energy::supply_with_storage) and this module writes back what was burnt, drunk and
// stored, and shows it.
//
// - Structures: touching crystals, touching batteries and panels side by side on one level are
//   groups found again with each rescan; a net that reaches one block of a group reaches them all
//   (net::energy::join_groups), and each block offers its share of the group's output.
// - The sky over a panel: the world-surface heightmap of its column (`native_surface_heights`,
//   API 127) is right above the panel; the time of day and the rain (`native_day_time`,
//   `native_weather`, API 97) set the rest. A windmill's height is its y.
// - Fluids: the lava generator keeps its lava (`lava`, mB) and a crystal structure its solution
//   (`solution`, on the structure's least block) in their block entities; rustcraft_fluids fills
//   them from tanks they touch and from fluid channels (`sinks`, `write_sinks`).
// - A battery structure's charge is spread evenly over its blocks' `energy`, as a tank's fluid is.
use crate::game::{
    Game, rustcraft_base as base,
    rustcraft_content::{self as content, Machine},
    rustcraft_machines::{self as machines, read_f64, write_f64},
    rustcraft_server::{LevelNet, line},
    rustcraft_window::named,
};
use horizon_client::{items::ItemStack, mod_api::NativeHeightmap, text::Text};
use rustcraft_rules::{
    geom::Pos,
    modules::Module,
    net::energy::{Producer, Shared, Store, join_groups},
    power::{self as rules, Battery, Form, PowerView, Shown},
    tuning::Tuning,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const LAVA_KEY: &str = "lava";
const SOLUTION_KEY: &str = "solution";
/// Panels and windmills look at the sky and the wind once a second.
const REFRESH_EVERY: u64 = 20;
const AIR: [&str; 3] = ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"];

/// One level's structures and what its sources showed last.
#[derive(Default)]
pub struct Sources {
    crystals: Vec<BTreeSet<Pos>>,
    panels: Vec<BTreeSet<Pos>>,
    batteries: Vec<BTreeSet<Pos>>,
    windmills: Vec<Pos>,
    lava: Vec<Pos>,
    /// Panels' and windmills' outputs as last looked at, with what they show.
    outputs: HashMap<Pos, (f64, Shown)>,
    ticks: u64,
    /// What each block shows now, for its look, its window and the self-test.
    pub views: HashMap<Pos, PowerView>,
}

/// After a rescan: the groups again, and the nets joined to whole structures.
pub(super) fn rescan(level: &mut LevelNet) {
    let of = |m: Machine| -> BTreeSet<Pos> {
        level
            .machines
            .iter()
            .filter(|(_, k)| **k == m)
            .map(|(p, _)| *p)
            .collect()
    };
    let s = &mut level.sources;
    s.crystals = rules::groups(&level.crystals, false);
    s.panels = rules::groups(&of(Machine::Solar), true);
    s.batteries = rules::groups(&of(Machine::Battery), false);
    s.windmills = of(Machine::Windmill).into_iter().collect();
    s.lava = of(Machine::LavaGenerator).into_iter().collect();
    s.outputs.clear();
    s.views
        .retain(|p, _| level.crystals.contains(p) || level.machines.contains_key(p));
    let groups: Vec<BTreeSet<Pos>> = s
        .crystals
        .iter()
        .chain(&s.panels)
        .chain(&s.batteries)
        .filter(|g| g.len() > 1)
        .cloned()
        .collect();
    join_groups(&mut level.power.circuits, &groups);
}

// ---- What the sources offer ----

fn read_key(game: &Game<'_>, pos: Pos, key: &str) -> f64 {
    game.native_mod_block_entity(pos)
        .map_or(0.0, |e| read_f64(&e.nbt, key))
}

fn write_key(game: &mut Game<'_>, pos: Pos, key: &str, value: f64) {
    game.native_with_mod_block_entity(pos, |e| write_f64(&mut e.nbt, key, value.max(0.0)));
}

/// A crystal group's solution, mB, kept on its least block (the rest is moved there).
fn solution_of(game: &Game<'_>, group: &BTreeSet<Pos>) -> f64 {
    group.iter().map(|&p| read_key(game, p, SOLUTION_KEY)).sum()
}

/// Each crystal offers its share of its group's output (E7, E8, E8.a).
pub(super) fn crystal_producers(game: &Game<'_>, s: &Sources, t: &Tuning) -> Vec<Producer> {
    let mut out = Vec::new();
    for g in &s.crystals {
        let fed = g.len() > 1 && solution_of(game, g) > 0.0;
        let total = rules::structure_output(g.len(), rules::form(g), fed, t);
        let each = rules::shares(total, g.len());
        out.extend(g.iter().map(|&block| Producer {
            block,
            output: each,
        }));
    }
    out
}

/// Once a second: each panel's sky, time and rain, each windmill's height and room.
fn refresh(game: &Game<'_>, s: &mut Sources, t: &Tuning) {
    s.ticks += 1;
    let due = s.ticks % REFRESH_EVERY == 1
        || s.panels
            .iter()
            .flatten()
            .chain(&s.windmills)
            .any(|p| !s.outputs.contains_key(p));
    if !due {
        return;
    }
    let day = game.native_day_time();
    let rain = game.native_weather().0;
    let panels: Vec<Pos> = s.panels.iter().flatten().copied().collect();
    if !panels.is_empty() {
        let columns: Vec<(i32, i32)> = panels.iter().map(|p| (p[0], p[2])).collect();
        let heights = game
            .native_surface_heights(
                game.native_dimension(),
                &columns,
                NativeHeightmap::WorldSurface,
            )
            .unwrap_or_default();
        for (i, p) in panels.iter().enumerate() {
            let sky = heights.get(i).copied().flatten() == Some(p[1] + 1);
            let out = rules::solar_output(sky, day, rain, t);
            let shown = if sky { Shown::Gives(out) } else { Shown::NoSky };
            s.outputs.insert(*p, (out, shown));
        }
    }
    for &w in &s.windmills {
        let clear = rules::windmill_room(w)
            .into_iter()
            .all(|c| AIR.contains(&game.native_block(c).as_str()));
        let out = rules::windmill_output(w[1], clear, t);
        let shown = if clear {
            Shown::Gives(out)
        } else {
            Shown::Blocked
        };
        s.outputs.insert(w, (out, shown));
    }
}

/// A panel, a windmill or a lava generator as the machines' tick reads it.
pub struct SourceHeld {
    pub output: f64,
    machine: Machine,
    lava: f64,
}

pub(super) fn read_source(
    game: &Game<'_>,
    s: &Sources,
    pos: Pos,
    machine: Machine,
    t: &Tuning,
) -> Option<SourceHeld> {
    match machine {
        Machine::LavaGenerator => {
            let lava = read_key(game, pos, LAVA_KEY);
            Some(SourceHeld {
                output: rules::lava_offer(lava, t),
                machine,
                lava,
            })
        }
        _ => Some(SourceHeld {
            output: s.outputs.get(&pos).map_or(0.0, |o| o.0),
            machine,
            lava: 0.0,
        }),
    }
}

/// A source after the sharing: a lava generator burns for what it gave.
pub(super) fn run_source(
    game: &mut Game<'_>,
    s: &mut Sources,
    pos: Pos,
    held: SourceHeld,
    used: f64,
    t: &Tuning,
) {
    let view = match held.machine {
        Machine::LavaGenerator => {
            let burnt = rules::lava_burnt(used, 1.0 / t.ticks_per_second, t);
            let lava = (held.lava - burnt).max(0.0);
            if burnt > 0.0 {
                write_key(game, pos, LAVA_KEY, lava);
            }
            PowerView {
                output: held.output,
                shown: if held.output > 0.0 {
                    Shown::Gives(held.output)
                } else {
                    Shown::NoLava
                },
                blocks: 1,
                stored: 0.0,
                capacity: 0.0,
                fluid: lava,
            }
        }
        _ => {
            let (output, shown) = s
                .outputs
                .get(&pos)
                .copied()
                .unwrap_or((0.0, Shown::Gives(0.0)));
            let group = s.panels.iter().find(|g| g.contains(&pos));
            let total: f64 = group.map_or(output, |g| {
                g.iter()
                    .map(|p| s.outputs.get(p).map_or(0.0, |o| o.0))
                    .sum()
            });
            // A structure's look names the whole structure's output (E20).
            let shown = match shown {
                Shown::Gives(_) if group.is_some_and(|g| g.len() > 1) => Shown::Gives(total),
                other => other,
            };
            PowerView {
                output,
                shown,
                blocks: group.map_or(1, BTreeSet::len),
                stored: total,
                capacity: 0.0,
                fluid: 0.0,
            }
        }
    };
    s.views.insert(pos, view);
}

/// The machines' tick starts here: panels and windmills look round once a second.
pub(super) fn before_tick(game: &Game<'_>, level: &mut LevelNet, t: &Tuning) {
    refresh(game, &mut level.sources, t);
}

// ---- The batteries ----

/// A battery structure now: its blocks' charge and modules.
fn battery_of(game: &Game<'_>, g: &BTreeSet<Pos>, t: &Tuning) -> (f64, Battery) {
    let stored: f64 = g.iter().map(|&p| machines::buffer_energy(game, p)).sum();
    let slots: Vec<Option<Module>> = g.iter().flat_map(|&p| base::modules_at(game, p)).collect();
    (stored, rules::battery(g.len(), &slots, t))
}

/// What each battery block may give and take this tick: its share of its structure's.
pub(super) fn stores(game: &Game<'_>, s: &Sources, t: &Tuning) -> Vec<Store> {
    let mut out = Vec::new();
    for g in &s.batteries {
        let (stored, b) = battery_of(game, g, t);
        let (give, take) = b.flow(stored, t);
        let n = g.len();
        out.extend(g.iter().map(|&block| Store {
            block,
            give: rules::shares(give, n),
            take: rules::shares(take, n),
        }));
    }
    out
}

/// After the sharing: batteries keep what they took and lose what they gave, spread over their
/// blocks; crystal structures drink their solution.
pub(super) fn settle(game: &mut Game<'_>, s: &mut Sources, shared: &Shared, t: &Tuning) {
    let dt = 1.0 / t.ticks_per_second;
    for g in s.batteries.clone() {
        let (stored, b) = battery_of(game, &g, t);
        let given: f64 = g.iter().filter_map(|p| shared.given.get(p)).sum();
        let taken: f64 = g.iter().filter_map(|p| shared.taken.get(p)).sum();
        let after = (stored + (taken - given) * dt).clamp(0.0, b.capacity);
        if (after - stored).abs() > 1e-9 {
            let each = rules::shares(after, g.len());
            for &p in &g {
                machines::set_buffer_energy(game, p, each);
            }
        }
        let view = PowerView {
            output: given,
            shown: Battery::shown(given, taken),
            blocks: g.len(),
            stored: after,
            capacity: b.capacity,
            fluid: taken,
        };
        for &p in &g {
            s.views.insert(p, view);
        }
    }
    for g in s.crystals.clone() {
        let solution = solution_of(game, &g);
        let form = rules::form(&g);
        let fed = g.len() > 1 && solution > 0.0;
        let total = rules::structure_output(g.len(), form, fed, t);
        let mut left = solution;
        if fed && matches!(form, Form::Box(_)) {
            left = (solution - rules::solution_per_second(g.len(), t) * dt).max(0.0);
            let anchor = rules::anchor(&g);
            for &p in &g {
                let want = if p == anchor { left } else { 0.0 };
                if (read_key(game, p, SOLUTION_KEY) - want).abs() > 1e-9 {
                    write_key(game, p, SOLUTION_KEY, want);
                }
            }
        }
        let view = PowerView {
            output: total,
            shown: rules::structure_shown(g.len(), form, fed, t),
            blocks: g.len(),
            stored: 0.0,
            capacity: 0.0,
            fluid: left,
        };
        for &p in &g {
            s.views.insert(p, view);
        }
    }
}

// ---- Fluids into the lava generators and the crystal structures ----

/// A block that drinks a fluid: a lava generator, or a crystal structure by any of its crystals.
#[derive(Clone, Debug, PartialEq)]
pub struct Sink {
    pub key: Pos,
    pub blocks: Vec<Pos>,
    pub fluid: &'static str,
    pub held: u64,
    pub capacity: u64,
}

impl Sink {
    pub fn room_for(&self, fluid: &str) -> u64 {
        if fluid == self.fluid {
            self.capacity.saturating_sub(self.held)
        } else {
            0
        }
    }
}

/// The level's sinks now.
pub(super) fn sinks(game: &Game<'_>, s: &Sources, t: &Tuning) -> Vec<Sink> {
    let mut out: Vec<Sink> = s
        .lava
        .iter()
        .map(|&p| Sink {
            key: p,
            blocks: vec![p],
            fluid: rules::LAVA,
            held: read_key(game, p, LAVA_KEY).floor() as u64,
            capacity: rules::lava_capacity(t) as u64,
        })
        .collect();
    out.extend(s.crystals.iter().filter(|g| g.len() > 1).map(|g| Sink {
        key: rules::anchor(g),
        blocks: g.iter().copied().collect(),
        fluid: rules::SOLUTION,
        held: solution_of(game, g).floor() as u64,
        capacity: rules::solution_room(g.len(), t) as u64,
    }));
    out
}

/// What the fluid tick put into each sink, written back on its key block.
pub(super) fn write_sinks(game: &mut Game<'_>, before: &[Sink], after: &BTreeMap<Pos, Sink>) {
    for b in before {
        let Some(a) = after.get(&b.key) else { continue };
        if a.held <= b.held {
            continue;
        }
        let add = (a.held - b.held) as f64;
        let key = if a.fluid == rules::LAVA {
            LAVA_KEY
        } else {
            SOLUTION_KEY
        };
        let now = read_key(game, b.key, key);
        write_key(game, b.key, key, now + add);
    }
}

// ---- What they show ----

fn e(n: f64) -> Text {
    let r = (n * 10.0).round() / 10.0;
    Text::literal(if (r - r.round()).abs() < 1e-9 {
        format!("{r:.0}")
    } else {
        format!("{r:.1}").replace('.', ",")
    })
}

fn shown_text(v: &PowerView) -> Text {
    match v.shown {
        Shown::Gives(x) if v.blocks > 1 => line(
            "rustcraft.power.structure_gives",
            "Structure of %s · gives %s E/s",
            &[Text::literal(v.blocks.to_string()), e(x)],
        ),
        Shown::Gives(x) => line("rustcraft.power.gives", "Gives %s E/s", &[e(x)]),
        Shown::NoSky => line("rustcraft.power.no_sky", "No sky over the panel", &[]),
        Shown::Blocked => line(
            "rustcraft.power.blocked",
            "A block is in the blades' way",
            &[],
        ),
        Shown::NoLava => line("rustcraft.power.no_lava", "No lava", &[]),
        Shown::NoSolution(x) => line(
            "rustcraft.power.no_solution",
            "No solution: gives %s E/s, as one crystal",
            &[e(x)],
        ),
        Shown::BrokenShape(x) => line(
            "rustcraft.power.broken_shape",
            "Broken structure form: gives %s E/s, as one crystal",
            &[e(x)],
        ),
        Shown::Charging | Shown::Discharging | Shown::Holding => {
            let mut text = match v.shown {
                Shown::Charging => line("rustcraft.battery.charging", "Charging", &[]),
                Shown::Discharging => line("rustcraft.battery.discharging", "Giving", &[]),
                _ => line("rustcraft.battery.holding", "Holding", &[]),
            };
            text.push(Text::literal(" · "));
            text.push(line(
                "rustcraft.battery.stored",
                "%s of %s E",
                &[e(v.stored.floor()), e(v.capacity)],
            ));
            text
        }
    }
}

/// The look of a crystal or a slice-8 power block.
pub(super) fn diagnosis(
    _game: &Game<'_>,
    level: &LevelNet,
    pos: Pos,
    block: &str,
    t: &Tuning,
) -> Option<Text> {
    let ours = block == content::CRYSTAL
        || matches!(
            Machine::of(block),
            Some(Machine::Solar | Machine::Windmill | Machine::LavaGenerator | Machine::Battery)
        );
    if !ours {
        return None;
    }
    let v = level.sources.views.get(&pos)?;
    let mut text = shown_text(v);
    if Machine::of(block) == Some(Machine::LavaGenerator) {
        text.push(Text::literal(" · "));
        text.push(line(
            "rustcraft.power.lava",
            "lava %s of %s mB",
            &[e(v.fluid.floor()), e(rules::lava_capacity(t))],
        ));
    }
    Some(text)
}

/// The cells of a power block's window (one row): the battery's modules, the numbers, the state.
pub(super) fn cells(
    game: &Game<'_>,
    level: &LevelNet,
    pos: Pos,
    machine: Machine,
    t: &Tuning,
    out: &mut [Option<ItemStack>],
) {
    let v = level.sources.views.get(&pos).copied();
    let numbers =
        |item: &str, title: Text, lore: Vec<Text>| Some(named(item, 1, title, &lore, false));
    match machine {
        Machine::Battery => {
            base::module_cells(&base::modules_at(game, pos), out);
            if let Some(v) = v {
                out[4] = numbers(
                    "minecraft:redstone_block",
                    line(
                        "rustcraft.battery.stored",
                        "%s of %s E",
                        &[e(v.stored.floor()), e(v.capacity)],
                    ),
                    vec![
                        line(
                            "rustcraft.battery.blocks",
                            "Blocks in the structure: %s",
                            &[Text::literal(v.blocks.to_string())],
                        ),
                        line("rustcraft.battery.takes", "Takes %s E/s", &[e(v.fluid)]),
                        line("rustcraft.battery.gives", "Gives %s E/s", &[e(v.output)]),
                    ],
                );
            }
        }
        Machine::LavaGenerator => {
            if let Some(v) = v {
                out[4] = numbers(
                    "minecraft:lava_bucket",
                    line(
                        "rustcraft.power.lava",
                        "lava %s of %s mB",
                        &[e(v.fluid.floor()), e(rules::lava_capacity(t))],
                    ),
                    vec![line(
                        "rustcraft.power.lava_hint",
                        "A bucket gives %s E at %s E/s",
                        &[e(t.lava_energy_per_bucket), e(t.lava_output)],
                    )],
                );
            }
        }
        _ => {
            if let Some(v) = v {
                let (item, hint) = if machine == Machine::Solar {
                    (
                        "minecraft:daylight_detector",
                        line(
                            "rustcraft.power.solar_hint",
                            "By day %s E/s, in rain half, at night a quarter",
                            &[e(t.solar_day)],
                        ),
                    )
                } else {
                    (
                        "minecraft:white_wool",
                        line(
                            "rustcraft.power.wind_hint",
                            "%s E/s at sea level, %s E/s high up",
                            &[e(t.windmill_low), e(t.windmill_high)],
                        ),
                    )
                };
                let mut lore = vec![hint];
                if machine == Machine::Solar && v.blocks > 1 {
                    lore.push(line(
                        "rustcraft.power.structure_gives",
                        "Structure of %s · gives %s E/s",
                        &[Text::literal(v.blocks.to_string()), e(v.stored)],
                    ));
                }
                out[4] = numbers(
                    item,
                    line("rustcraft.power.gives", "Gives %s E/s", &[e(v.output)]),
                    lore,
                );
            }
        }
    }
    let good = v.is_some_and(|v| {
        !matches!(
            v.shown,
            Shown::NoSky
                | Shown::Blocked
                | Shown::NoLava
                | Shown::NoSolution(_)
                | Shown::BrokenShape(_)
        )
    });
    out[8] = Some(named(
        if good {
            "minecraft:lime_dye"
        } else {
            "minecraft:red_dye"
        },
        1,
        v.map_or_else(|| Text::literal("…"), |v| shown_text(&v)),
        &[],
        false,
    ));
}

/// What the self-test reads of a power block or crystal.
pub(super) fn view(level: &LevelNet, pos: Pos) -> Option<PowerView> {
    level.sources.views.get(&pos).copied()
}
