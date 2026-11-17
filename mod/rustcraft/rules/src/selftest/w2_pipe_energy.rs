//! Slice 18, wave 2: the magistral's and the energy's leftovers of the second audit
//! (docs/superpowers/plans/s18-w2-pipe-energy.md).

use super::magistral::{channel, line};
use super::{Build, Scenario, at, count_of, expect, plot_area};
use crate::net::topology::ChannelKind;
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 6] = [
    wrench_lifts_a_tier_3_machine,
    energy_channel_caps_all_its_passes,
    windmill_blades_where_the_rotor_turns,
    broken_node_gives_back,
    lost_housing_drops_its_facade,
    node_window_shows_what_is_stored,
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
