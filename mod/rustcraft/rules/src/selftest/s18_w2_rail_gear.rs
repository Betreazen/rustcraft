//! Slice 18, wave 2, rail and gear (plan `s18-w2-rail-gear.md`): what the second audit left of the
//! turret's windows and kills, the workers, the monorail and the gear.

use super::{Build, Scenario, at, plot_area};
use crate::geom::Pos;
use crate::modules::Module;
use crate::tuning::Tuning;

pub const ALL: &[Build] = &[turret_window_spells_the_shot];

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
