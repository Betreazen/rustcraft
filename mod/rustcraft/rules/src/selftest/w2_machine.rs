//! Slice 18, wave 2: the machines' and modules' last differences from the handbook.
//!
//! - The trash takes from a hopper only what its joint's filter lets through, and its window
//!   names the filter's items and «самый низкий» priority (guide `trash`, `ui.js`).
//! - The disintegrator's window shows «Разборка»: what its item gives, in whole items and
//!   fractions (`ui.js` disintegrator).
//! - A charged piece stays in the charger's «Броня» cells with no net beside it (`ui.js` charger).

use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::{Face, Pos};
use crate::tuning::Tuning;

pub const ALL: [Build; 3] = [
    w2_trash_takes_by_joint_filter,
    w2_disintegrator_preview,
    w2_charger_keeps_charged_gear,
];

const STONE: &str = "minecraft:stone";
const COBBLE: &str = "minecraft:cobblestone";
const DIRT: &str = "minecraft:dirt";

fn floor(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=8 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    Ok(())
}

/// Whether the sheet of the block at `pos` holds `needle` anywhere in its texts.
fn sheet_says(b: &dyn Bench, pos: Pos, needle: &str) -> Result<bool, String> {
    let s = b.sheet(pos).ok_or("no sheet")?;
    Ok(serde_json::to_string(&s).unwrap_or_default().contains(needle))
}

fn w2_trash_takes_by_joint_filter(o: Pos, _: &Tuning) -> Scenario {
    let h = at(o, [2, 1, 0]);
    let p = at(o, [3, 1, 0]);
    Scenario::new("w2_trash_takes_by_joint_filter", plot_area(o))
        .then("a trash beside a housing whose joint lets cobblestone through, no energy", move |b, _| {
            floor(b, o)?;
            b.place(h, "rustcraft:housing", &[])?;
            b.place(p, "rustcraft:trash", &[])?;
            let mut j = crate::machines::tiered::trash_joint();
            j.filter.items.insert(COBBLE.into());
            b.set_joint(h, Face::XPos, 0, j)
        })
        .until("a hopper's cobblestone goes in once the trash read its joint", 80, move |b, _| {
            Ok(b.insert(p, COBBLE, 1)? == 1)
        })
        .check("a hopper's dirt does not go in", move |b, _| {
            same("dirt in", b.insert(p, DIRT, 4)?, 0)?;
            same("dirt held", count_of(&b.items(p), DIRT), 0)
        })
        .check("the window names the filter's item and the lowest priority", move |b, _| {
            if !sheet_says(b, p, "block.minecraft.cobblestone")? {
                return Err("the filter row does not name cobblestone".into());
            }
            if !sheet_says(b, p, "rustcraft.util.trash.priority_lowest")? {
                return Err("the priority row does not say «самый низкий»".into());
            }
            let s = b.sheet(p).ok_or("no sheet")?;
            same("rows", s.rows.len(), 4)?;
            same("bars", s.bars.len(), 2)
        })
}

fn w2_disintegrator_preview(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("w2_disintegrator_preview", plot_area(o))
        .then("a disintegrator with no energy and an iron pickaxe", move |b, _| {
            floor(b, o)?;
            b.place(p, "rustcraft:disintegrator", &[])?;
            b.put(p, 0, "minecraft:iron_pickaxe", 1)
        })
        .wait(5)
        .check("«Разборка»: one and a half iron ingots and a stick", move |b, _| {
            let s = b.sheet(p).ok_or("no sheet")?;
            same(
                "preview",
                s.preview,
                vec![
                    ("minecraft:iron_ingot".to_owned(), "1½".to_owned()),
                    ("minecraft:stick".to_owned(), "1".to_owned()),
                ],
            )?;
            if !sheet_says(b, p, "rustcraft.util.disintegrator.share_lost")? {
                return Err("no «чары теряются» row".into());
            }
            same("bars", s.bars.len(), 2)
        })
}

fn w2_charger_keeps_charged_gear(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("w2_charger_keeps_charged_gear", plot_area(o))
        .then("a charger with energy and a blade, no housing beside it", move |b, _| {
            floor(b, o)?;
            b.place(p, "rustcraft:charger", &[])?;
            b.set_buffer(p, 1000.0)?;
            b.put_gear(p, 0, "rustcraft:blade_2", 0.0)
        })
        .until("the blade holds charge", 200, move |b, _| Ok(b.gear_charge(p, 0).is_some_and(|c| c > 100.0)))
        .wait(100)
        .check("it is still in its «Броня» cell and the window has no work mode", move |b, _| {
            same("blade in cell 0", b.items(p).first().cloned().flatten().map(|s| s.0), Some("rustcraft:blade_2".to_owned()))?;
            let s = b.sheet(p).ok_or("no sheet")?;
            same("mode", s.mode, None)?;
            if !sheet_says(b, p, "rustcraft.util.charger.armour")? {
                return Err("no «Броня» label".into());
            }
            Ok(())
        })
}
