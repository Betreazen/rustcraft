//! Slice 18, wave 2: the magistral's and the energy's leftovers of the second audit
//! (docs/superpowers/plans/s18-w2-pipe-energy.md).

use super::magistral::{channel, line};
use super::{Build, Scenario, at, count_of, expect, plot_area};
use crate::net::topology::ChannelKind;
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 3] = [
    wrench_lifts_a_tier_3_machine,
    energy_channel_caps_all_its_passes,
    windmill_blades_where_the_rotor_turns,
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
