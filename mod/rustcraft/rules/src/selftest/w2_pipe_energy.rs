//! Slice 18, wave 2: the magistral's and the energy's leftovers of the second audit
//! (docs/superpowers/plans/s18-w2-pipe-energy.md).

use super::magistral::{channel, line};
use super::{Build, Scenario, at, count_of, expect, plot_area};
use crate::net::topology::ChannelKind;
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 9] = [
    wrench_lifts_a_tier_3_machine,
    energy_channel_caps_all_its_passes,
    windmill_blades_where_the_rotor_turns,
    broken_node_gives_back,
    lost_housing_drops_its_facade,
    node_window_shows_what_is_stored,
    miner_with_chest_and_magistral,
    two_miners_both_fill_their_docks,
    miner_stack_module_keeps_to_the_menu,
];

/// Guide `wrench`: a machine of a higher tier lifted by the wrench comes off as its tier's item
/// and is placed back on that tier, with what it held.
fn wrench_lifts_a_tier_3_machine(o: Pos, _t: &Tuning) -> Scenario {
    let f = at(o, [1, 0, 0]);
    Scenario::new("w2_wrench_lifts_a_tier_3_machine_as_tier_3", plot_area(o))
        .then("an electric furnace of tier 3 with 4 raw iron", move |b, _| {
            b.place(f, "rustcraft:furnace", &[("facing", "north"), ("tier", "3")])?;
            b.put(f, 0, "minecraft:raw_iron", 4)
        })
        .check("lifted as rustcraft:furnace_3, placed back on tier 3 with its iron", move |b, _| {
            let item = b.pipe("lift_and_place", f)?;
            let tier = b.property(f, "tier");
            let n = count_of(&b.items(f), "minecraft:raw_iron");
            if item == "rustcraft:furnace_3" && tier.as_deref() == Some("3") && n == 4 {
                Ok(())
            } else {
                Err(format!("lifted as {item}, tier {tier:?}, {n} raw iron"))
            }
        })
}

/// Guide `ch_energy` (32 / 128 / 512 / 2048 a tick): a tier-1 energy net with a 16 E/s fuel
/// generator, a charged battery and five furnaces asking 40 in all. The generator gives 16 and
/// the battery covers the rest only as far as the channel lets through: 32 of 40, not 40.
fn energy_channel_caps_all_its_passes(o: Pos, t: &Tuning) -> Scenario {
    let generator = at(o, [1, 0, -1]);
    let battery = at(o, [2, 0, 1]);
    let furnaces = [
        at(o, [4, 0, -1]),
        at(o, [5, 0, -1]),
        at(o, [6, 0, -1]),
        at(o, [5, 0, 1]),
        at(o, [6, 0, 1]),
    ];
    let request = t.furnace_energy * furnaces.len() as f64;
    let supply = t.energy_of(1).min(request);
    let want = expect::working_shown(expect::share(supply, request));
    Scenario::new("w2_energy_channel_caps_all_its_passes", plot_area(o))
        .then("a generator, a charged battery and five furnaces on a tier-1 energy net", move |b, _| {
            line(b, o, 1, 6, [channel(ChannelKind::Energy, 1), None, None, None])?;
            b.place(generator, "rustcraft:gen_fuel", &[])?;
            let fuel = b.inputs(generator).first().copied().ok_or("no fuel slot")?;
            b.put(generator, fuel, "minecraft:coal", 16)?;
            b.place(battery, "rustcraft:battery", &[])?;
            b.set_buffer(battery, 20_000.0)?;
            for f in furnaces {
                b.place(f, "rustcraft:furnace", &[])?;
                let slot = b.inputs(f).first().copied().ok_or("no input slot")?;
                b.put(f, slot, "minecraft:raw_iron", 64)?;
            }
            b.refresh();
            Ok(())
        })
        .until("every furnace works on its share of the channel's 32", 200, move |b, _| {
            Ok(furnaces.iter().all(|&f| b.shown(f) == Some(want)))
        })
}

/// Guide `windmill` («место вокруг лопастей должно быть свободно»): the blades turn a block to
/// either side, one and two blocks up. A block beside it two up stops one windmill; a block
/// behind it one up, where the blades never reach, does not stop another.
fn windmill_blades_where_the_rotor_turns(o: Pos, _t: &Tuning) -> Scenario {
    use crate::power::Shown;
    let side = at(o, [2, 0, 0]);
    let behind = at(o, [8, 0, 0]);
    let shown = |b: &dyn super::Bench, p: Pos| b.power(p).map(|v| v.shown);
    Scenario::new("w2_windmill_blades_checked_where_the_rotor_turns", plot_area(o))
        .then("two windmills facing north, stone beside one two up, behind the other one up", move |b, _| {
            b.place(side, "rustcraft:windmill", &[("facing", "north")])?;
            b.place(at(side, [1, 2, 0]), "minecraft:stone", &[])?;
            b.place(behind, "rustcraft:windmill", &[("facing", "north")])?;
            b.place(at(behind, [0, 1, 1]), "minecraft:stone", &[])
        })
        .until("the first is blocked, the second turns", 200, move |b, _| {
            Ok(shown(b, side) == Some(Shown::Blocked)
                && matches!(shown(b, behind), Some(Shown::Gives(_))))
        })
}

/// Guide `facade` («блок тратится и возвращается»), `mod_line`: a node broken in survival gives
/// back its facade, the block of its look and its line-boost modules.
fn broken_node_gives_back(o: Pos, _t: &Tuning) -> Scenario {
    Scenario::new("w2_broken_node_gives_back_facade_look_and_modules", plot_area(o))
        .then("a node with two line-boost modules, hidden as bricks", move |b, _| {
            line(b, o, 1, 1, [channel(ChannelKind::Item, 1), None, None, None])?;
            for i in 0..2 {
                b.pipe(&format!("socket:{i}"), o)?;
            }
            b.pipe("cover:minecraft:bricks", o).map(|_| ())
        })
        .check("broken, it gives back the facade, the bricks and both modules", move |b, _| {
            let items = b.pipe("node_returns", o)?;
            let want = "rustcraft:facade,minecraft:bricks,rustcraft:module_line,rustcraft:module_line";
            if items == want { Ok(()) } else { Err(format!("{items:?}, {want:?} expected")) }
        })
}

/// Guide `facade`, `housing`: a housing under a facade that goes without a player (an explosion,
/// a command) drops its facade and the block of its look with its channels at the next rescan.
fn lost_housing_drops_its_facade(o: Pos, _t: &Tuning) -> Scenario {
    let h = at(o, [1, 0, 0]);
    Scenario::new("w2_lost_housing_drops_its_facade_and_look", plot_area(o))
        .then("a housing with a channel, hidden as bricks", move |b, _| {
            line(b, o, 1, 1, [channel(ChannelKind::Item, 1), None, None, None])?;
            b.pipe("cover:minecraft:bricks", h)?;
            b.refresh();
            Ok(())
        })
        .wait(40)
        .then("the housing set to air, as a command does", move |b, _| {
            b.place(h, "minecraft:air", &[])?;
            b.refresh();
            Ok(())
        })
        .until("its facade, the bricks and its channel lie where it stood", 100, move |b, _| {
            let c = h.map(f64::from);
            let lying = b.lying([c[0] - 1.0, c[1] - 1.0, c[2] - 1.0], [c[0] + 2.0, c[1] + 2.0, c[2] + 2.0]);
            let has = |item: &str| lying.iter().any(|(i, n)| i == item && *n >= 1);
            Ok(has("rustcraft:facade") && has("minecraft:bricks") && has("rustcraft:item_channel"))
        })
}

/// ui.js `node` (`balance`: «В запасе 12 400 из 40 000 Е»): the node's window counts the charge
/// and the size of the batteries on its line's circuits.
fn node_window_shows_what_is_stored(o: Pos, t: &Tuning) -> Scenario {
    let battery = at(o, [2, 0, 1]);
    let capacity = crate::power::battery(1, &[], t).capacity;
    Scenario::new("w2_node_window_shows_what_is_stored", plot_area(o))
        .then("a node, an energy line and a charged battery on it", move |b, _| {
            line(b, o, 1, 3, [channel(ChannelKind::Energy, 1), None, None, None])?;
            b.place(battery, "rustcraft:battery", &[])?;
            b.set_buffer(battery, 5_000.0)?;
            b.refresh();
            Ok(())
        })
        .until("its window has the battery's charge of its size", 200, move |b, _| {
            let json = b.pipe("node_view:0", o)?;
            let v = crate::net::node_window::decode(json.as_bytes()).ok_or("the view does not read")?;
            Ok(v.stored > 4_000.0 && (v.capacity - capacity).abs() < 1e-6)
        })
}

/// Two iron ores under a miner at `m`, each walled in stone (so no air reaches them).
fn ores_under(b: &mut dyn super::Bench, ores: &[Pos]) -> Result<(), String> {
    for &p in ores {
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            b.place(at(p, d), "minecraft:stone", &[])?;
        }
        b.place(p, "minecraft:iron_ore", &[])?;
    }
    Ok(())
}

fn raw_iron(b: &dyn super::Bench, p: Pos) -> u32 {
    count_of(&b.items(p), "minecraft:raw_iron")
}

/// MN1 (guide `miner`: «если рядом сундук, всё идёт в него»): a miner with a chest beside it and
/// a housing of an item net beside it too puts all it digs into the chest; the net's chest gets
/// none of it.
fn miner_with_chest_and_magistral(o: Pos, _t: &Tuning) -> Scenario {
    let m = at(o, [5, 6, 6]);
    let chest = at(m, [0, 0, 1]);
    let net_chest = at(m, [1, 0, -2]);
    let ores = [at(m, [2, -6, -1]), at(m, [-2, -6, 1])];
    Scenario::new("w2_miner_with_a_chest_beside_fills_only_the_chest", plot_area(o))
        .then("a miner on two crystals, a chest and an item net beside it, two iron ores", move |b, _| {
            b.place(m, "rustcraft:miner", &[])?;
            b.place(at(m, [1, 0, 0]), "rustcraft:crystal", &[])?;
            b.place(at(m, [-1, 0, 0]), "rustcraft:crystal", &[])?;
            b.place(chest, "minecraft:chest", &[])?;
            super::magistral::place_node(b, at(m, [0, 0, -3]), 1)?;
            for k in [1, 2] {
                let h = at(m, [0, 0, -k]);
                b.place(h, super::magistral::HOUSING, &[])?;
                b.set_channels(h, [channel(ChannelKind::Item, 1), None, None, None])?;
            }
            b.place(net_chest, "minecraft:chest", &[])?;
            ores_under(b, &ores)?;
            b.refresh();
            Ok(())
        })
        .until("both raw iron are in the chest beside it", 1500, move |b, _| Ok(raw_iron(b, chest) >= 2))
        .wait(40)
        .check("the net's chest got none", move |b, _| {
            let n = raw_iron(b, net_chest);
            if n == 0 { Ok(()) } else { Err(format!("{n} raw iron went into the magistral")) }
        })
}

/// MN1, wave 2: two miners side by side, each with a load dock on it, each fill their own dock
/// (one push count for every machine left one of two never on its turn).
fn two_miners_both_fill_their_docks(o: Pos, _t: &Tuning) -> Scenario {
    let a = at(o, [5, 6, 6]);
    let c = at(o, [22, 6, 6]);
    let ores = [at(a, [2, -6, -1]), at(c, [-2, -6, 1])];
    Scenario::new("w2_two_miners_both_fill_their_load_docks", plot_area(o))
        .then("two miners on crystals, a load dock on each, an iron ore under each", move |b, _| {
            for m in [a, c] {
                b.place(m, "rustcraft:miner", &[])?;
                b.place(at(m, [1, 0, 0]), "rustcraft:crystal", &[])?;
                b.place(at(m, [-1, 0, 0]), "rustcraft:crystal", &[])?;
                b.place(at(m, [0, 1, 0]), "rustcraft:dock_load", &[])?;
            }
            ores_under(b, &ores)
        })
        .until("each load dock has its miner's raw iron", 1500, move |b, _| {
            Ok(raw_iron(b, at(a, [0, 1, 0])) >= 1 && raw_iron(b, at(c, [0, 1, 0])) >= 1)
        })
}

/// Guide `mod_stack` against the menu's 99 a slot: a miner with a stack module and its nine
/// window slots full of 64 raw iron puts the next raw iron into a hidden cell, not a window slot
/// of 65; its window counts the stack module's seven hidden cells.
fn miner_stack_module_keeps_to_the_menu(o: Pos, _t: &Tuning) -> Scenario {
    let m = at(o, [5, 6, 6]);
    let ores = [at(m, [2, -6, -1])];
    let hidden_row = move |b: &dyn super::Bench| -> Option<(u64, u64)> {
        let s = b.sheet(m)?;
        let r = s.rows.iter().find(|r| r["translate"] == "rustcraft.miner.hidden")?;
        let n = |i: usize| r["with"][i].as_u64().or_else(|| r["with"][i].as_str()?.parse().ok());
        Some((n(0)?, n(2)?))
    };
    Scenario::new("w2_miner_stack_module_keeps_the_window_to_a_stack", plot_area(o))
        .then("a miner on two crystals with a stack module, its slots full, an iron ore", move |b, _| {
            b.place(m, "rustcraft:miner", &[])?;
            b.place(at(m, [1, 0, 0]), "rustcraft:crystal", &[])?;
            b.place(at(m, [-1, 0, 0]), "rustcraft:crystal", &[])?;
            b.set_modules(m, &["rustcraft:module_stack"])?;
            for slot in 0..9 {
                b.put(m, slot, "minecraft:raw_iron", 64)?;
            }
            ores_under(b, &ores)
        })
        .until("the raw iron went into a hidden cell of the seven", 1500, move |b, _| {
            Ok(hidden_row(b).is_some_and(|(items, cells)| items >= 1 && cells == 7))
        })
        .check("no window slot holds more than a stack", move |b, _| {
            let most = b.items(m).into_iter().flatten().map(|(_, n)| n).max().unwrap_or(0);
            if most <= 64 { Ok(()) } else { Err(format!("a window slot holds {most}")) }
        })
}
