//! Slice 18, wave 2, rail and gear (plan `s18-w2-rail-gear.md`): what the second audit left of the
//! turret's windows and kills, the workers, the monorail and the gear.

use super::{Build, Scenario, at, plot_area};
use crate::geom::Pos;
use crate::modules::Module;
use crate::tuning::Tuning;

pub const ALL: &[Build] = &[turret_window_spells_the_shot, turret_kill_drops_the_owners_loot, rail_takes_another_tier_in_place, chassis_window_sets_the_bores_height];

const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_00c1;

/// Every `translate` key in a window row's text, its `with` and `extra` parts too.
pub(crate) fn keys_of(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(o) => {
            if let Some(k) = o.get("translate").and_then(|t| t.as_str()) {
                out.push(k.to_owned());
            }
            for x in o.values() {
                keys_of(x, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| keys_of(x, out)),
        _ => {}
    }
}

/// Task 1 (base-gear #12): an attack turret with three effect modules spells its shot
/// «Выстрел: 40 Е + 3 × 10 Е = 70 Е» and its slowness «−20% скорости».
fn turret_window_spells_the_shot(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("w2_turret_window_spells_the_shot", plot_area(o))
        .then("an attack turret with bleeding, poison and slowness", move |b, _| {
            b.place(at(o, [3, 0, 0]), "minecraft:stone", &[])?;
            b.place(p, "rustcraft:turret", &[])?;
            b.set_owner(p, OWNER, 1)?;
            b.set_modules(p, &[Module::Bleed.item(), Module::Poison.item(), Module::Slow.item()])
        })
        .check("its rows read «40 Е + 3 × 10 Е = 70 Е» and «−20% скорости»", move |b, _| {
            let sheet = b.sheet(p).ok_or("no turret window")?;
            let mut keys = Vec::new();
            let mut text = String::new();
            for r in &sheet.rows {
                keys_of(r, &mut keys);
                text.push_str(&r.to_string());
            }
            for want in ["rustcraft.turret.shot_parts", "rustcraft.turret.effect.slow_share"] {
                if !keys.iter().any(|k| k == want) {
                    return Err(format!("no {want} row in {keys:?}"));
                }
            }
            for n in ["[40,3,10,70]", "20%"] {
                if !text.contains(n) {
                    return Err(format!("no {n} in {text}"));
                }
            }
            Ok(())
        })
}

/// Task 2 (base-gear #3, rail-worker «Страж: добыча killed_by_player»): with the owner away (the
/// bench has no player) a turret's kills still drop the loot only a player's kill gets. A spider
/// drops its eye on a player's kill a third of the time: of 14 such kills at least one drops one
/// (all fourteen without one: 0.3%).
fn turret_kill_drops_the_owners_loot(o: Pos, t: &Tuning) -> Scenario {
    const NAMES: [&str; 14] = ["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "s12", "s13"];
    let turret = at(o, [10, 1, 0]);
    let full = t.turret_buffer;
    Scenario::new("w2_turret_kill_drops_the_owners_loot", plot_area(o))
        .then("a floor walled in and the turret with a full buffer, its owner away", move |b, _| {
            for x in 0..24 {
                for z in -4..=4 {
                    b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
                    if x == 0 || x == 23 || z.abs() == 4 {
                        for y in 1..=3 {
                            b.place(at(o, [x, y, z]), "minecraft:stone", &[])?;
                        }
                    }
                }
            }
            b.place(turret, "rustcraft:turret", &[])?;
            b.set_owner(turret, OWNER, 1)?;
            b.set_buffer(turret, full)
        })
        .then("fourteen weak spiders", move |b, m| {
            for (i, name) in (0i32..).zip(NAMES) {
                let x = f64::from(o[0] + 3 + i);
                let id = b.spawn_mob("minecraft:spider", [x + 0.5, f64::from(o[1] + 1), f64::from(o[2]) + 2.5], Some(1.0))?;
                m.mark(name, f64::from(id));
            }
            Ok(())
        })
        .until("the turret shot them all", 1200, move |b, m| {
            Ok(NAMES.iter().all(|n| b.mob(m.get(n) as u32).is_none()))
        })
        .until("a spider eye lies on the plot", 60, move |b, _| {
            let lying = b.lying(at(o, [-1, 0, -5]).map(f64::from), at(o, [25, 5, 5]).map(f64::from));
            Ok(lying.iter().any(|(item, _)| item == "minecraft:spider_eye"))
        })
}

/// Task 7 (guide `monorail`): a tier-1 rail clicked with a tier-3 rail takes tier 3 in place, its
/// mount kept, and gives the tier-1 rail back; a click with the same tier changes nothing.
fn rail_takes_another_tier_in_place(o: Pos, _: &Tuning) -> Scenario {
    use crate::monorail::shape;
    let p = at(o, [3, 1, 0]);
    Scenario::new("w2_rail_takes_another_tier_in_place", plot_area(o))
        .then("a tier-1 rail on the floor", move |b, _| {
            b.place(at(o, [3, 0, 0]), "minecraft:stone", &[])?;
            b.place(p, "rustcraft:monorail", &[(shape::MOUNT, shape::mount_value(crate::geom::Face::YNeg))])
        })
        .check("a tier-3 rail swaps in, the tier-1 one comes back", move |b, _| {
            let old = b.swap_rail(p, "rustcraft:monorail_3")?;
            if old != "rustcraft:monorail" {
                return Err(format!("gave back {old}"));
            }
            let tier = b.property(p, "tier");
            if tier.as_deref() != Some("3") {
                return Err(format!("tier {tier:?}"));
            }
            let mount = b.property(p, shape::MOUNT);
            if mount.as_deref() != Some(shape::mount_value(crate::geom::Face::YNeg)) {
                return Err(format!("mount {mount:?}"));
            }
            if b.swap_rail(p, "rustcraft:monorail_3").is_ok() {
                return Err("the same tier swapped".into());
            }
            Ok(())
        })
}

/// Task 12 (guide `chassis`, `mod_height`): the chassis window's presses reach the worker — the
/// filter clears, the bore's height field rises by one and by ten (right click) — and a bore with
/// the height module and a height set two above its rails climbs a step a rail to it, then
/// keeps level.
fn chassis_window_sets_the_bores_height(o: Pos, _: &Tuning) -> Scenario {
    use crate::monorail::shape;
    use crate::worker::{Tool, screen::{FILTER, Press}};
    const MONORAIL: &str = "rustcraft:monorail";
    let top = at(o, [6, 3, 0]);
    Scenario::new("w2_chassis_window_sets_the_bores_height", plot_area(o))
        .then("rails on a floor, a bore with the height module, rails and cobblestone", move |b, m| {
            for x in -1..=10 {
                for z in -2..=2 {
                    b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
                }
            }
            for x in 0..=3 {
                b.place(at(o, [x, 1, 0]), MONORAIL, &[(shape::MOUNT, shape::mount_value(crate::geom::Face::YNeg))])?;
            }
            let id = b.spawn_worker(at(o, [0, 1, 0]), OWNER, Some(Tool::Bore), &[(MONORAIL, 8), ("minecraft:cobblestone", 16)], None)?;
            m.mark("worker", id as f64);
            b.set_worker_modules(id, &[crate::modules::Module::Height.item()])?;
            b.set_worker_filter(id, &["minecraft:stone"])
        })
        .then("the window: the filter cleared, the height up by ten then down by eight", move |b, m| {
            let id = m.get("worker") as u64;
            b.chassis_press(id, Press::Cell { cell: FILTER, right: false })?;
            b.chassis_press(id, Press::Height { by: 10 })?;
            b.chassis_press(id, Press::Height { by: -8 })
        })
        .until("the worker holds the height two over its rails and no filter", 40, move |b, m| {
            let w = b.worker(m.get("worker") as u64).ok_or("no worker")?;
            Ok(w.filter.is_empty() && w.height == Some(o[1] + 3))
        })
        .until("its rails climb to the height and go on level", 1600, move |b, _| {
            Ok(b.block(at(o, [4, 2, 0])) == MONORAIL && b.block(at(o, [5, 3, 0])) == MONORAIL && b.block(top) == MONORAIL)
        })
}
