//! Slice 18, stream 1: the magistral as the handbook says (docs/superpowers/plans/s18-pipe.md).
//!
//! - The housing, the node of every tier, the wrench and the tier-4 channels are crafted from
//!   their handbook recipes (guide `housing`, `node`, `wrench`, `ch_*`: «для ступеней 2–4 вместо
//!   медностали берётся сплав этой ступени»).

use super::magistral::{channel, line};
use super::transport::{COBBLE, chest, fill, item_line, joint};
use super::{Bench, Build, Scenario, at, count_of, plot_area};
use crate::housing::Diagnosis;
use crate::net::topology::ChannelKind;
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 10] = [
    recipes_craft,
    node_tiers,
    chests_work_both_ways,
    facade_and_lift,
    node_window_and_boost,
    wrench_turns_and_lifts,
    bottlenecks_marked,
    lost_housing_drops_channels,
    joint_window_table,
    quickstart_line,
];

/// A handbook recipe: its three rows, what each letter is, what it makes and how many.
pub struct Recipe {
    pub rows: [&'static str; 3],
    pub keys: &'static [(char, &'static str)],
    pub result: &'static str,
    pub count: u32,
}

const A1: &str = "rustcraft:coppersteel";
const A2: &str = "rustcraft:sparksteel";
const A3: &str = "rustcraft:crystallite";
const A4: &str = "rustcraft:starsteel";
/// The guide's crystal shard; stream 2 brings `rustcraft:crystal_shard` and the integrator
/// switches every recipe to it.
const SHARD: &str = "rustcraft:crystal_shard";

const NODE_KEYS: [[(char, &str); 3]; 4] = [
    [('a', A1), ('c', "minecraft:copper_ingot"), ('s', SHARD)],
    // Tiers 2–4 take the node of the tier below in the middle, not a shard: sparksteel, copper
    // and a shard is the handbook's recipe of the output module too (data.js `mod_output`).
    [('a', A2), ('c', "minecraft:copper_ingot"), ('s', "rustcraft:node")],
    [('a', A3), ('c', "minecraft:copper_ingot"), ('s', "rustcraft:node_2")],
    [('a', A4), ('c', "minecraft:copper_ingot"), ('s', "rustcraft:node_3")],
];

/// The node of tier `tier` (1–4): copper-steel's recipe with that tier's alloy.
const fn node(tier: usize, result: &'static str) -> Recipe {
    Recipe {
        rows: ["aca", "csc", "aca"],
        keys: &NODE_KEYS[tier - 1],
        result,
        count: 1,
    }
}

/// The recipes stream 1 adds, as data.js writes them.
pub const RECIPES: [Recipe; 9] = [
    Recipe {
        rows: ["aga", "g g", "aga"],
        keys: &[('a', A1), ('g', "minecraft:glass")],
        result: "rustcraft:housing",
        count: 8,
    },
    node(1, "rustcraft:node"),
    node(2, "rustcraft:node_2"),
    node(3, "rustcraft:node_3"),
    node(4, "rustcraft:node_4"),
    Recipe {
        rows: ["a a", " a ", " a "],
        keys: &[('a', A1)],
        result: "rustcraft:wrench",
        count: 1,
    },
    Recipe {
        rows: [" a ", "aha", " a "],
        keys: &[('a', A4), ('h', "minecraft:hopper")],
        result: "rustcraft:item_channel_4",
        count: 8,
    },
    Recipe {
        rows: [" a ", "aha", " a "],
        keys: &[('a', A4), ('h', "minecraft:bucket")],
        result: "rustcraft:fluid_channel_4",
        count: 8,
    },
    Recipe {
        rows: [" a ", "aha", " a "],
        keys: &[('a', A4), ('h', "minecraft:copper_ingot")],
        result: "rustcraft:energy_channel_4",
        count: 8,
    },
];

impl Recipe {
    /// The 3 × 3 grid the recipe fills.
    pub fn grid(&self) -> [Option<&'static str>; 9] {
        let mut grid = [None; 9];
        for (r, row) in self.rows.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                grid[r * 3 + c] = self.keys.iter().find(|(k, _)| *k == ch).map(|(_, i)| *i);
            }
        }
        grid
    }
}

fn recipes_craft(o: Pos, _t: &Tuning) -> Scenario {
    Scenario::new("pipe_recipes_craft", super::plot_area(o)).check(
        "the housing, the nodes, the wrench and the tier-4 channels craft as the handbook says",
        |b: &mut dyn Bench, _| {
            for r in &RECIPES {
                let made = b.craft(&r.grid());
                if made != Some((r.result.to_owned(), r.count)) {
                    return Err(format!("{} × {} expected, {made:?} crafted", r.count, r.result));
                }
            }
            Ok(())
        },
    )
}

/// Housings of the node-tier line: past a tier-1 node's range, within a tier-2 node's.
const TIER_LINE: i32 = 40;

/// U2 on the node: a node of tier 1 serves 32 blocks, so the line's last housings are too far; a
/// node of tier 2 swapped into its place in place serves them all; a lower tier does not swap.
fn node_tiers(o: Pos, t: &Tuning) -> Scenario {
    let last = at(o, [TIER_LINE, 0, 0]);
    let limit_1 = t.range_of(1);
    let limit_2 = t.range_of(2);
    Scenario::new("pipe_node_tiers_swap_in_place", plot_area(o))
        .then("a tier-1 node and a line of 40 housings", move |b, _| {
            line(b, o, 1, TIER_LINE, [channel(ChannelKind::Item, 1), None, None, None])
        })
        .until("the last housing is beyond a tier-1 node's range", 100, move |b, _| {
            Ok(matches!(b.housing(last), Some(Diagnosis::TooFar { limit, .. }) if limit == limit_1))
        })
        .then("a tier-2 node swapped in", move |b, _| {
            let back = b.pipe("swap_node:2", o)?;
            if back != "1" {
                return Err(format!("tier {back} came back, 1 expected"));
            }
            b.refresh();
            Ok(())
        })
        .until("the node is of tier 2 and serves the whole line", 100, move |b, _| {
            Ok(b.property(o, "tier").as_deref() == Some("2")
                && matches!(b.housing(last), Some(Diagnosis::Works { limit, .. }) if limit == limit_2))
        })
        .check("a node of a lower tier takes no place", move |b, _| {
            match b.pipe("swap_node:1", o) {
                Err(_) if b.property(o, "tier").as_deref() == Some("2") => Ok(()),
                other => Err(format!("a lower tier swapped: {other:?}")),
            }
        })
}

/// Guide `quickstart` 5: ore in the left chest, an electric furnace beside the line with a
/// crystal against it, an empty chest on the right; no joint is set. The magistral gives the
/// furnace the ore and the right chest the ingots, and never moves ore from chest to chest.
fn chests_work_both_ways(o: Pos, _t: &Tuning) -> Scenario {
    let (left, right) = (chest(o, 1), chest(o, 3));
    let furnace = at(o, [2, 0, -1]);
    Scenario::new("pipe_chests_work_both_ways_by_default", plot_area(o))
        .then("chest, furnace with a crystal, chest on an item line", move |b, _| {
            item_line(b, o, 1, 3, &[1, 3])?;
            b.place(furnace, "rustcraft:furnace", &[])?;
            b.place(at(o, [2, 0, -2]), "rustcraft:crystal", &[])?;
            fill(b, left, "minecraft:raw_iron", 2)
        })
        .until("an ingot reaches the right chest", 1200, move |b, _| {
            let items = b.items(right);
            if count_of(&items, "minecraft:raw_iron") > 0 {
                return Err("ore went from chest to chest".into());
            }
            Ok(count_of(&items, "minecraft:iron_ingot") >= 1)
        })
        .check("the left chest gave its ore", move |b, _| {
            let n = count_of(&b.items(left), "minecraft:raw_iron");
            if n == 0 { Ok(()) } else { Err(format!("{n} ore left in the left chest")) }
        })
}

/// Guide `quickstart` 3–5 (in-game check w3b): the left chest, a crusher and an electric furnace
/// beside the line, each with its own crystal, and the right chest; no joint is set. Ore goes
/// from the left chest to the crusher (not the furnace), the dust to the furnace (never into a
/// chest), the ingots into the right chest, the one nearest the furnace.
fn quickstart_line(o: Pos, _t: &Tuning) -> Scenario {
    const RAW: &str = "minecraft:raw_iron";
    const DUST: &str = "rustcraft:dust_iron";
    const INGOT: &str = "minecraft:iron_ingot";
    const ORE: u32 = 2;
    let (left, right) = (chest(o, 1), chest(o, 4));
    let (crusher, furnace) = (at(o, [2, 0, -1]), at(o, [3, 0, -1]));
    Scenario::new("pipe_quickstart_ore_crusher_furnace_right_chest", plot_area(o))
        .then("chest, crusher, furnace, chest on an item line, a crystal each", move |b, _| {
            item_line(b, o, 1, 4, &[1, 4])?;
            b.place(crusher, "rustcraft:crusher", &[])?;
            b.place(furnace, "rustcraft:furnace", &[])?;
            // Apart, so they stay two crystals (guide `quickstart` 4).
            b.place(at(o, [2, 1, -1]), "rustcraft:crystal", &[])?;
            b.place(at(o, [3, 0, -2]), "rustcraft:crystal", &[])?;
            fill(b, left, RAW, ORE)
        })
        .until("two ingots an ore reach the right chest", 2400, move |b, _| {
            let (l, r) = (b.items(left), b.items(right));
            let f = b.items(furnace);
            if count_of(&l, DUST) + count_of(&r, DUST) > 0 {
                return Err("dust went into a chest, not on to the furnace".into());
            }
            if count_of(&l, INGOT) > 0 {
                return Err("an ingot went back into the left chest".into());
            }
            if count_of(&r, RAW) + count_of(&f, RAW) > 0 {
                return Err("ore skipped the crusher".into());
            }
            Ok(count_of(&r, INGOT) >= 2 * ORE)
        })
        .check("the left chest gave all its ore", move |b, _| {
            let n = count_of(&b.items(left), RAW);
            if n == 0 { Ok(()) } else { Err(format!("{n} ore left in the left chest")) }
        })
}

/// Guide `facade`: a hidden housing is a whole block (F4, `covered`); the facade and its look's
/// block come back with it; the sneaking wrench then takes the housing itself, channels and all.
fn facade_and_lift(o: Pos, _t: &Tuning) -> Scenario {
    let h = at(o, [1, 0, 0]);
    Scenario::new("pipe_facade_whole_block_and_lift", plot_area(o))
        .then("a node and a housing with two channels, hidden as bricks", move |b, _| {
            line(b, o, 1, 1, [channel(ChannelKind::Item, 2), channel(ChannelKind::Energy, 1), None, None])?;
            b.pipe("cover:minecraft:bricks", h).map(|_| ())
        })
        .check("the hidden housing is a whole block and keeps its facade", move |b, _| {
            if b.property(h, crate::housing::COVERED).as_deref() != Some("true") {
                return Err(format!("covered = {:?}", b.property(h, crate::housing::COVERED)));
            }
            let items = b.pipe("facade_items", h)?;
            if items != "rustcraft:facade,minecraft:bricks" {
                return Err(format!("the facade gives back {items:?}"));
            }
            Ok(())
        })
        .then("the sneaking wrench takes the housing", move |b, m| {
            let items = b.pipe("lift_housing", h)?;
            let want = "rustcraft:housing,rustcraft:facade,minecraft:bricks,rustcraft:item_channel_2,rustcraft:energy_channel";
            if items != want {
                return Err(format!("{items:?} came back, {want:?} expected"));
            }
            m.mark("lifted", 1.0);
            Ok(())
        })
        .check("the housing is gone", move |b, _| {
            let block = b.block(h);
            if block == "minecraft:air" { Ok(()) } else { Err(format!("{block} stayed")) }
        })
}

/// The window ticks and measures over this many ticks.
const BOOST_WINDOW: u64 = 40;

/// Guide `node`, `mod_line`: the node's window counts the line's housings and the farthest one
/// it serves; a line-boost module in its socket raises the item channel's throughput by half.
fn node_window_and_boost(o: Pos, t: &Tuning) -> Scenario {
    let base = super::expect::items_in(t, 1, BOOST_WINDOW);
    let (a, bb) = (chest(o, 1), chest(o, 4));
    Scenario::new("pipe_node_window_and_line_boost", plot_area(o))
        .then("a tier-1 node, 4 housings, a chest giving and a chest taking", move |b, _| {
            item_line(b, o, 1, 4, &[1, 4])?;
            joint(b, o, 1, crate::net::items::Mode::From, 0)?;
            fill(b, a, COBBLE, 27 * 64)
        })
        .until("the window shows 4 housings, the farthest 4 of 32", 100, move |b, _| {
            let json = b.pipe("node_view:0", o)?;
            let v = crate::net::node_window::decode(json.as_bytes()).ok_or("the view does not read")?;
            Ok(v.housings == 4 && v.farthest == 4 && v.limit == 32 && v.items == Some((4.0, 4.0)))
        })
        .then("a line-boost module into the first socket", move |b, _| {
            let n = b.pipe("socket:0", o)?;
            if n != "1" {
                return Err(format!("{n} modules after the press"));
            }
            b.refresh();
            Ok(())
        })
        .wait(40)
        .then("count what the taking chest has", move |b, m| {
            m.mark("start", f64::from(count_of(&b.items(bb), COBBLE)));
            m.mark("at", m.elapsed as f64);
            Ok(())
        })
        .wait(BOOST_WINDOW)
        .check("the boosted net carries half as much again", move |b, m| {
            let moved = f64::from(count_of(&b.items(bb), COBBLE)) - m.get("start");
            let want = base * crate::net::node_window::boost(1);
            // One move's worth either way, as the plain rate scenarios allow.
            let tolerance = 1.0 + super::expect::items_in(&Tuning::default(), 1, 4) * 1.5;
            if (moved - want).abs() <= tolerance {
                Ok(())
            } else {
                Err(format!("{moved} moved in {BOOST_WINDOW} ticks, {want} expected"))
            }
        })
}

/// Guide `wrench`: a click turns a machine a quarter clockwise; a sneaking click lifts it as one
/// item that, placed again, holds what the machine held.
fn wrench_turns_and_lifts(o: Pos, _t: &Tuning) -> Scenario {
    let f = at(o, [1, 0, 0]);
    Scenario::new("pipe_wrench_turns_and_lifts_machines", plot_area(o))
        .then("an electric furnace facing north with 5 raw iron", move |b, _| {
            b.place(f, "rustcraft:furnace", &[("facing", "north")])?;
            b.put(f, 0, "minecraft:raw_iron", 5)
        })
        .check("a click turns it east", move |b, _| {
            let to = b.pipe("rotate", f)?;
            let now = b.property(f, "facing");
            if to == "east" && now.as_deref() == Some("east") {
                Ok(())
            } else {
                Err(format!("turned to {to}, facing {now:?}"))
            }
        })
        .then("lifted and placed back", move |b, _| b.pipe("lift_and_place", f).map(|_| ()))
        .check("it holds its 5 raw iron again", move |b, _| {
            let n = count_of(&b.items(f), "minecraft:raw_iron");
            if n == 5 { Ok(()) } else { Err(format!("{n} raw iron after the lift")) }
        })
}

/// Guide `wrench`: on a line of tier-3, tier-1, tier-1, tier-3 item channels the two tier-1
/// housings are its bottleneck; clients get the mark with the housing.
fn bottlenecks_marked(o: Pos, _t: &Tuning) -> Scenario {
    Scenario::new("pipe_bottlenecks_marked_for_the_wrench", plot_area(o))
        .then("a line of tiers 3, 1, 1, 3", move |b, _| {
            line(b, o, 1, 4, [channel(ChannelKind::Item, 3), None, None, None])?;
            for k in [2, 3] {
                b.set_channels(at(o, [k, 0, 0]), [channel(ChannelKind::Item, 1), None, None, None])?;
            }
            b.refresh();
            Ok(())
        })
        .until("the tier-1 housings carry the bottleneck mark, the others none", 100, move |b, _| {
            let w: Vec<String> = (1..=4)
                .map(|k| b.pipe("weak", at(o, [k, 0, 0])))
                .collect::<Result<_, _>>()?;
            Ok(w == ["0", "1", "1", "0"])
        })
}

/// Guide `housing`: a housing that goes without a player (an explosion, a command) drops its
/// channels where it stood at the next rescan.
fn lost_housing_drops_channels(o: Pos, _t: &Tuning) -> Scenario {
    let h = at(o, [1, 0, 0]);
    Scenario::new("pipe_lost_housing_drops_its_channels", plot_area(o))
        .then("a housing with two channels", move |b, _| {
            line(b, o, 1, 1, [channel(ChannelKind::Item, 2), channel(ChannelKind::Fluid, 1), None, None])?;
            b.refresh();
            Ok(())
        })
        .wait(40)
        .then("the housing set to air, as a command does", move |b, _| {
            b.place(h, "minecraft:air", &[])?;
            b.refresh();
            Ok(())
        })
        .until("its channels lie where it stood", 100, move |b, _| {
            let c = h.map(f64::from);
            let lying = b.lying([c[0] - 1.0, c[1] - 1.0, c[2] - 1.0], [c[0] + 2.0, c[1] + 2.0, c[2] + 2.0]);
            let has = |item: &str| lying.iter().any(|(i, n)| i == item && *n >= 1);
            Ok(has("rustcraft:item_channel_2") && has("rustcraft:fluid_channel"))
        })
}


/// Guide `wrench` (ui.js «Стык: …»): the joint window as the drawn table. A press goes through
/// the same rules a player's does: a direction, the off hand's item on the filter, the list and
/// the priority, each kept in the housing.
fn joint_window_table(o: Pos, _t: &Tuning) -> Scenario {
    use crate::net::joint_screen::{self as js, JointView, Press};
    let h = at(o, [1, 0, 0]);
    let face = crate::geom::Face::ZNeg.index();
    let press = move |b: &mut dyn Bench, held: &str, p: Press| -> Result<JointView, String> {
        let json = serde_json::to_string(&p).map_err(|e| e.to_string())?;
        let out = b.pipe(&format!("joint_press:{face};{held};{json}"), h)?;
        js::decode(out.as_bytes()).ok_or_else(|| "the view does not read".to_owned())
    };
    Scenario::new("pipe_joint_window_is_the_guides_table", plot_area(o))
        .then("a housing with an item channel and a chest beside it", move |b, _| item_line(b, o, 1, 1, &[1]))
        .check("the window names the chest and shows an item row", move |b, _| {
            let out = b.pipe(&format!("joint_view:{face}"), h)?;
            let v = js::decode(out.as_bytes()).ok_or("the view does not read")?;
            let r = &v.rows[0];
            if v.block == "minecraft:chest" && r.filtered() && !v.rows[1].directed() {
                Ok(())
            } else {
                Err(format!("{v:?}"))
            }
        })
        .check("direction, filter from the off hand, white list and priority are kept", move |b, _| {
            let first = press(b, "", Press::Mode { slot: 0, back: false })?.rows[0].mode;
            press(b, "minecraft:raw_iron", Press::Add { slot: 0 })?;
            press(b, "", Press::List { slot: 0 })?;
            press(b, "", Press::Priority { slot: 0, delta: 1 })?;
            let out = b.pipe(&format!("joint_view:{face}"), h)?;
            let v = js::decode(out.as_bytes()).ok_or("the view does not read")?;
            let r = &v.rows[0];
            let kept = r.mode == first
                && r.items == ["minecraft:raw_iron"]
                && r.allow_only
                && r.priority == 1;
            if kept { Ok(()) } else { Err(format!("{r:?} after the presses (mode {first})")) }
        })
        .check("a filter press on the empty slot does nothing", move |b, _| {
            match press(b, "minecraft:dirt", Press::Add { slot: 1 }) {
                Err(e) if e.contains("nothing") => Ok(()),
                other => Err(format!("{other:?}")),
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// The content mod's recipe file of `name`.
    fn file(name: &str) -> Value {
        let text = match name {
            "housing" => include_str!("../../../../content/data/rustcraft/recipe/housing.json"),
            "node" => include_str!("../../../../content/data/rustcraft/recipe/node.json"),
            "node_2" => include_str!("../../../../content/data/rustcraft/recipe/node_2.json"),
            "node_3" => include_str!("../../../../content/data/rustcraft/recipe/node_3.json"),
            "node_4" => include_str!("../../../../content/data/rustcraft/recipe/node_4.json"),
            "wrench" => include_str!("../../../../content/data/rustcraft/recipe/wrench.json"),
            "item_channel_4" => {
                include_str!("../../../../content/data/rustcraft/recipe/item_channel_4.json")
            }
            "fluid_channel_4" => {
                include_str!("../../../../content/data/rustcraft/recipe/fluid_channel_4.json")
            }
            "energy_channel_4" => {
                include_str!("../../../../content/data/rustcraft/recipe/energy_channel_4.json")
            }
            _ => panic!("no recipe file {name}"),
        };
        serde_json::from_str(text).expect("recipe JSON")
    }

    #[test]
    fn the_recipe_files_are_the_handbook_recipes() {
        for r in &RECIPES {
            let name = r.result.trim_start_matches("rustcraft:");
            let f = file(name);
            let rows: Vec<&str> = f["pattern"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
            assert_eq!(rows, r.rows, "{name} pattern");
            for (k, item) in r.keys {
                assert_eq!(f["key"][k.to_string()], *item, "{name} key {k}");
            }
            assert_eq!(f["key"].as_object().unwrap().len(), r.keys.len(), "{name} keys");
            assert_eq!(f["result"]["id"], r.result);
            assert_eq!(f["result"]["count"], r.count);
        }
    }

    #[test]
    fn a_broken_node_gives_back_its_own_tier() {
        let loot: Value = serde_json::from_str(include_str!(
            "../../../../content/data/rustcraft/loot_table/blocks/node.json"
        ))
        .unwrap();
        let children = loot["pools"][0]["entries"][0]["children"].as_array().unwrap();
        for tier in 1..=4u8 {
            let child = children
                .iter()
                // Slice 18 (cross): 26.3's form, one `match_block` condition on the tier.
                .find(|c| {
                    c["condition"]["type"] == "minecraft:match_block"
                        && c["condition"]["state"]["tier"] == tier.to_string()
                })
                .unwrap_or_else(|| panic!("no loot for tier {tier}"));
            assert_eq!(child["name"], crate::net::node_tier::item_of_tier(tier));
        }
    }

    #[test]
    fn a_grid_puts_each_letter_where_the_rows_say() {
        let g = RECIPES[0].grid();
        assert_eq!(g[0], Some(A1));
        assert_eq!(g[1], Some("minecraft:glass"));
        assert_eq!(g[4], None);
    }
}
