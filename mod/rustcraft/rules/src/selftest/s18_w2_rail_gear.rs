//! Slice 18, wave 2, rail and gear (plan `s18-w2-rail-gear.md`): what the second audit left of the
//! turret's windows and kills, the workers, the monorail and the gear.

use super::{Build, Scenario, at, plot_area};
use crate::geom::Pos;
use crate::modules::Module;
use crate::tuning::Tuning;

pub const ALL: &[Build] = &[turret_window_spells_the_shot, turret_kill_drops_the_owners_loot];

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
            for n in ["\"40\"", "\"3\"", "\"10\"", "\"70\"", "\"20%\""] {
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
