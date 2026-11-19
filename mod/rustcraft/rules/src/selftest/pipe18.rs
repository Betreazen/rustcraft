//! Slice 18, stream 1: the magistral as the handbook says (docs/superpowers/plans/s18-pipe.md).
//!
//! - The housing and the node of every tier and the wrench are crafted from their handbook
//!   recipes (guide `housing`, `node`, `wrench`: «для ступеней 2–4 вместо медностали берётся сплав
//!   этой ступени»); round 31: the channels are built into the housing, and the channel items left
//!   from 0.19 are taken apart into their alloy.

use super::magistral::{line, place_housing, place_node};
use super::transport::{COBBLE, chest, fill, item_line, joint};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::housing::Diagnosis;
use crate::geom::{Face, Pos};
use crate::net::items::Mode;
use crate::net::settings::JointSetting;
use crate::net::topology::ITEM_SLOT;
use crate::tuning::Tuning;

pub const ALL: [Build; 10] = [
    recipes_craft,
    node_tiers,
    chests_work_both_ways,
    facade_and_lift,
    node_window_and_boost,
    wrench_turns_and_lifts,
    bottlenecks_marked,
    housing_tiers_swap_in_place,
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

const HOUSING_KEYS: [[(char, &str); 2]; 4] = [
    [('a', A1), ('g', "minecraft:glass")],
    [('a', A2), ('g', "minecraft:glass")],
    [('a', A3), ('g', "minecraft:glass")],
    [('a', A4), ('g', "minecraft:glass")],
];

/// Round 31: the housing of tier `tier` (1–4), copper-steel's recipe with that tier's alloy.
const fn housing(tier: usize, result: &'static str) -> Recipe {
    Recipe {
        rows: ["aga", "g g", "aga"],
        keys: &HOUSING_KEYS[tier - 1],
        result,
        count: 8,
    }
}

/// The recipes stream 1 adds, as data.js writes them.
pub const RECIPES: [Recipe; 9] = [
    housing(1, "rustcraft:housing"),
    housing(2, "rustcraft:housing_2"),
    housing(3, "rustcraft:housing_3"),
    housing(4, "rustcraft:housing_4"),
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
];

/// Round 31: the channel items of 0.19, by kind and tier; two of one are taken apart into one
/// alloy of their tier (the half alloy each was made of).
pub const OLD_CHANNELS: [(&str, &str); 12] = [
    ("rustcraft:item_channel", A1),
    ("rustcraft:item_channel_2", A2),
    ("rustcraft:item_channel_3", A3),
    ("rustcraft:item_channel_4", A4),
    ("rustcraft:fluid_channel", A1),
    ("rustcraft:fluid_channel_2", A2),
    ("rustcraft:fluid_channel_3", A3),
    ("rustcraft:fluid_channel_4", A4),
    ("rustcraft:energy_channel", A1),
    ("rustcraft:energy_channel_2", A2),
    ("rustcraft:energy_channel_3", A3),
    ("rustcraft:energy_channel_4", A4),
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
    Scenario::new("pipe_recipes_craft", super::plot_area(o))
        .check(
            "the housings, the nodes and the wrench craft as the handbook says",
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
        .check("two old channels of a tier give back its alloy; none is made any more", |b, _| {
            for (channel, alloy) in OLD_CHANNELS {
                let mut grid = [None; 9];
                grid[3] = Some(channel);
                grid[5] = Some(channel);
                same(channel, b.craft(&grid), Some((alloy.to_owned(), 1)))?;
            }
            let hopper = Some("minecraft:hopper");
            let a = Some(A1);
            let old = [None, a, None, a, hopper, a, None, a, None];
            match b.craft(&old) {
                Some((item, _)) if item.contains("channel") => Err(format!("{item} is still made")),
                _ => Ok(()),
            }
        })
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
            line(b, o, 1, TIER_LINE, 1)
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
/// block come back with it; the sneaking wrench then takes the housing itself, as its tier's item.
fn facade_and_lift(o: Pos, _t: &Tuning) -> Scenario {
    let h = at(o, [1, 0, 0]);
    Scenario::new("pipe_facade_whole_block_and_lift", plot_area(o))
        .then("a node and a tier-2 housing, hidden as bricks", move |b, _| {
            line(b, o, 1, 1, 2)?;
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
            let want = "rustcraft:housing_2,rustcraft:facade,minecraft:bricks";
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
/// item that, placed again, holds what the machine held. Every click is a player's click through
/// the game's own use (`Bench::click_block`), so a handler that claims the click first fails it.
fn wrench_turns_and_lifts(o: Pos, _t: &Tuning) -> Scenario {
    const WRENCH: &str = "rustcraft:wrench";
    const FURNACE: &str = "rustcraft:furnace";
    let f = at(o, [1, 0, 0]);
    let c = at(o, [3, 0, 0]);
    let under = at(o, [1, -1, 0]);
    Scenario::new("pipe_wrench_turns_and_lifts_machines", plot_area(o))
        .then("a furnace with 5 raw iron and a crusher facing north, stone under the furnace", move |b, _| {
            b.place(under, "minecraft:stone", &[])?;
            b.place(f, FURNACE, &[("facing", "north")])?;
            b.place(c, "rustcraft:crusher", &[("facing", "north")])?;
            b.put(f, 0, "minecraft:raw_iron", 5)
        })
        .then("a wrench click on each one's front", move |b, _| {
            b.click_block(f, Face::ZNeg, WRENCH, false)?;
            b.click_block(c, Face::ZNeg, WRENCH, false).map(|_| ())
        })
        .check("both turned east, the furnace still holding its iron", move |b, _| {
            let now = [b.property(f, "facing"), b.property(c, "facing")];
            let iron = count_of(&b.items(f), "minecraft:raw_iron");
            if now.iter().all(|p| p.as_deref() == Some("east")) && iron == 5 {
                Ok(())
            } else {
                Err(format!("facing {now:?}, {iron} raw iron"))
            }
        })
        .then("a sneaking wrench click lifts the furnace into the hand", move |b, _| {
            let got = b.click_block(f, Face::ZNeg, WRENCH, true)?;
            if got == [FURNACE] { Ok(()) } else { Err(format!("the click gave {got:?}")) }
        })
        .check("its place is empty", move |b, _| {
            let now = b.block(f);
            if now == "minecraft:air" { Ok(()) } else { Err(format!("{now} stayed")) }
        })
        .then("the lifted furnace placed back on the stone", move |b, _| {
            b.click_block(under, Face::YPos, &format!("got:{FURNACE}"), true).map(|_| ())
        })
        .check("it stands there holding its 5 raw iron again", move |b, _| {
            let block = b.block(f);
            let n = count_of(&b.items(f), "minecraft:raw_iron");
            if block == FURNACE && n == 5 {
                Ok(())
            } else {
                Err(format!("{block} with {n} raw iron after the lift"))
            }
        })
}

/// Guide `wrench`: on a line of housings of tiers 3, 1, 1, 3 the two tier-1 housings are its
/// bottleneck; clients get the mark with the housing.
fn bottlenecks_marked(o: Pos, _t: &Tuning) -> Scenario {
    Scenario::new("pipe_bottlenecks_marked_for_the_wrench", plot_area(o))
        .then("a line of tiers 3, 1, 1, 3", move |b, _| {
            place_node(b, o, 1)?;
            for (k, tier) in [(1, 3), (2, 1), (3, 1), (4, 3)] {
                place_housing(b, at(o, [k, 0, 0]), tier)?;
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

/// Round 31 (U2 for housings, as the machines and the node): a click with a housing of a higher
/// tier on a placed one sets its tier in place, its joints' settings kept, and the old tier comes
/// back to the hand; a housing of a tier not higher is placed beside it, as any block.
fn housing_tiers_swap_in_place(o: Pos, _t: &Tuning) -> Scenario {
    let h = at(o, [2, 0, 0]);
    let joint_of = move |b: &dyn Bench| {
        b.housing_joints(h)
            .and_then(|j| j.get(Face::ZNeg, ITEM_SLOT).map(|s| s.mode))
    };
    Scenario::new("pipe_housing_tiers_swap_in_place", plot_area(o))
        .then("a node and three tier-1 housings, a joint set on the middle one", move |b, _| {
            line(b, o, 1, 3, 1)?;
            b.set_joint(h, Face::ZNeg, ITEM_SLOT, JointSetting::of_mode(Mode::Off))
        })
        .then("a click with a tier-3 housing on the middle one", move |b, _| {
            let got = b.click_block(h, Face::YPos, "rustcraft:housing_3", false)?;
            same("given back", got, vec!["rustcraft:housing".to_owned()])
        })
        .check("it is of tier 3 and keeps its joint", move |b, _| {
            same("tier", b.property(h, "tier"), Some("3".to_owned()))?;
            same("joint", joint_of(b), Some(Mode::Off))
        })
        .then("a click with a tier-1 housing on top of it", move |b, _| {
            b.click_block(h, Face::YPos, "rustcraft:housing", false).map(|_| ())
        })
        .check("a tier-1 housing stands above it, still of tier 3", move |b, _| {
            same("above", b.block(at(h, [0, 1, 0])), "rustcraft:housing".to_owned())?;
            same("its tier", b.property(at(h, [0, 1, 0]), "tier"), Some("1".to_owned()))?;
            same("tier", b.property(h, "tier"), Some("3".to_owned()))
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
        .then("a housing and a chest beside it", move |b, _| item_line(b, o, 1, 1, &[1]))
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
        .check("a filter press on the fluid row, no joint for a chest, does nothing", move |b, _| {
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
            "housing_2" => include_str!("../../../../content/data/rustcraft/recipe/housing_2.json"),
            "housing_3" => include_str!("../../../../content/data/rustcraft/recipe/housing_3.json"),
            "housing_4" => include_str!("../../../../content/data/rustcraft/recipe/housing_4.json"),
            "node" => include_str!("../../../../content/data/rustcraft/recipe/node.json"),
            "node_2" => include_str!("../../../../content/data/rustcraft/recipe/node_2.json"),
            "node_3" => include_str!("../../../../content/data/rustcraft/recipe/node_3.json"),
            "node_4" => include_str!("../../../../content/data/rustcraft/recipe/node_4.json"),
            "wrench" => include_str!("../../../../content/data/rustcraft/recipe/wrench.json"),
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

    /// A file of the content mod, read at test time.
    fn content(path: &str) -> Value {
        let full = format!("{}/../../content/{path}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{full}: {e}"))
    }

    #[test]
    fn a_broken_housing_gives_back_its_own_tier() {
        // Round 31: tiers 2–4 by their state, anything else the first tier.
        let loot = content("data/rustcraft/loot_table/blocks/housing.json");
        let children = loot["pools"][0]["entries"][0]["children"].as_array().unwrap();
        for tier in 2..=4u8 {
            let child = children
                .iter()
                .find(|c| c["condition"]["state"]["tier"] == tier.to_string())
                .unwrap_or_else(|| panic!("no loot for tier {tier}"));
            assert_eq!(child["condition"]["type"], "minecraft:match_block");
            assert_eq!(child["name"], crate::housing::item_of_tier(tier));
        }
        let last = children.last().unwrap();
        assert!(last.get("condition").is_none());
        assert_eq!(last["name"], "rustcraft:housing");
    }

    #[test]
    fn old_channels_come_apart_into_their_alloy_and_none_is_made() {
        for (channel, alloy) in OLD_CHANNELS {
            let name = channel.trim_start_matches("rustcraft:");
            let r = content(&format!("data/rustcraft/recipe/{name}_apart.json"));
            assert_eq!(r["type"], "minecraft:crafting_shapeless", "{name}");
            assert_eq!(r["ingredients"], serde_json::json!([channel, channel]), "{name}");
            assert_eq!(r["result"]["id"], alloy, "{name}");
            assert_eq!(r["result"]["count"], 1, "{name}");
            let made = format!("{}/../../content/data/rustcraft/recipe/{name}.json", env!("CARGO_MANIFEST_DIR"));
            assert!(!std::path::Path::new(&made).exists(), "{name} is still made");
        }
    }

    #[test]
    fn the_housing_shows_its_tier_and_its_channels_along_its_joins() {
        // Round 31 (R5): a core and an arm a side for each tier, the channels by axis; every model
        // it names is there.
        let states = content("assets/rustcraft/blockstates/housing.json");
        let parts = states["multipart"].as_array().unwrap();
        let models: Vec<&str> = parts.iter().map(|p| p["apply"]["model"].as_str().unwrap()).collect();
        for tier in 1..=4 {
            assert!(models.contains(&format!("rustcraft:block/housing_core_{tier}").as_str()));
            for side in crate::housing::SIDES {
                assert!(models.contains(&format!("rustcraft:block/housing_arm_{side}_{tier}").as_str()));
            }
        }
        for axis in ["x", "y", "z"] {
            assert!(models.contains(&format!("rustcraft:block/housing_channels_{axis}").as_str()));
        }
        for model in models {
            let path = format!("assets/rustcraft/models/block/{}.json", model.trim_start_matches("rustcraft:block/"));
            assert!(content(&path)["elements"].as_array().is_some_and(|e| !e.is_empty()), "{path}");
        }
        for tier in 2..=4 {
            let item = content(&format!("assets/rustcraft/items/housing_{tier}.json"));
            assert_eq!(item["model"]["model"], format!("rustcraft:block/housing_inventory_{tier}"));
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
