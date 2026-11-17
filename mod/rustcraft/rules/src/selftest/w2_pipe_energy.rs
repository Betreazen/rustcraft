//! Slice 18, wave 2: the magistral's and the energy's leftovers of the second audit
//! (docs/superpowers/plans/s18-w2-pipe-energy.md).

use super::{Build, Scenario, at, count_of, plot_area};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [wrench_lifts_a_tier_3_machine];

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
