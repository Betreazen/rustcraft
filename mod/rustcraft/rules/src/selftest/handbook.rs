//! The engineer's handbook (spec H2; rustcraft_rules::handbook).
//!
//! - A pretend player carrying no alloy gets nothing; with coppersteel they get the handbook,
//!   and with sparksteel after it nothing more: the handbook comes once.
//! - Every page names an item the game knows, and the handbook's own recipe, book and
//!   coppersteel, is in the game's recipes as the page's grid reads it and makes the handbook.
//! - Every item a page's scene (H3) draws is in the game's item registry.
//! - Pages show the machines' recipes as the guide does (slice 18): the alloys' page the
//!   coppersteel of the crafting table and the three alloys of the alloy smelter, the dusts' page
//!   the crusher's, the fluids' page the mixer's, and the blade's page the transmuting recipes of
//!   tiers 3 and 4.

use super::{Build, Scenario, plot_area, same};
use crate::geom::Pos;
use crate::handbook::recipes::{Made, Out, Shown};
use crate::handbook::{self, PAGES, scene};
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [
    first_alloy_gives_the_handbook_once,
    pages_name_known_items,
    scenes_show_known_items,
    pages_show_machine_recipes,
];

const PLAYER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0b00;
const COPPERSTEEL: &str = "rustcraft:coppersteel";
const BOOK: &str = "minecraft:book";

fn first_alloy_gives_the_handbook_once(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("handbook_first_alloy_gives_it_once", plot_area(o))
        .then("a pretend player who never had the handbook", |b, _| {
            b.handbook_forget(PLAYER);
            Ok(())
        })
        .check("no alloy, no handbook", |b, _| {
            same(
                "given for dirt and iron",
                b.handbook_offer(PLAYER, &["minecraft:dirt", "minecraft:iron_ingot"]),
                false,
            )
        })
        .check("the first coppersteel gives it", |b, _| {
            same(
                "given for coppersteel",
                b.handbook_offer(PLAYER, &["minecraft:dirt", COPPERSTEEL]),
                true,
            )
        })
        .check("sparksteel after it gives nothing more", |b, _| {
            let again = b.handbook_offer(PLAYER, &["rustcraft:sparksteel", COPPERSTEEL]);
            b.handbook_forget(PLAYER);
            same("given again", again, false)
        })
}

fn pages_name_known_items(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("handbook_pages_name_known_items", plot_area(o)).check(
        "every page's item is the game's, and book and coppersteel make the handbook",
        |b, _| {
            let unknown: Vec<&str> = PAGES
                .iter()
                .filter_map(|p| p.item)
                .chain(PAGES.iter().flat_map(|p| p.makes.iter().copied()))
                .filter(|i| *i != crate::handbook::recipes::ANY_FLUID && !b.item_known(i))
                .collect();
            if !unknown.is_empty() {
                return Err(format!(
                    "pages name items the game does not know: {unknown:?}"
                ));
            }
            let grid = b
                .recipe_grid(handbook::ITEM)
                .ok_or("the handbook's page finds no recipe")?;
            let mut cells: Vec<&str> = grid.iter().flatten().map(String::as_str).collect();
            cells.sort_unstable();
            same("the handbook's recipe", cells, vec![BOOK, COPPERSTEEL])?;
            let made = b.craft(&[
                Some(BOOK),
                Some(COPPERSTEEL),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ]);
            same(
                "book and coppersteel make",
                made,
                Some((handbook::ITEM.to_owned(), 1)),
            )
        },
    )
}

fn scenes_show_known_items(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("handbook_scenes_show_known_items", plot_area(o)).check(
        "every block a scene draws is an item of the game",
        |b, _| {
            let unknown: Vec<&str> = scene::LOOKS
                .iter()
                .filter_map(|l| match *l {
                    scene::Look::Item(id) | scene::Look::Model(id, _) => Some(id),
                    scene::Look::Tint(_) | scene::Look::Drop(_) => None,
                })
                .filter(|i| !b.item_known(i))
                .collect();
            if unknown.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "scenes draw items the game does not know: {unknown:?}"
                ))
            }
        },
    )
}

/// The page `id`'s recipes as (machine or "table", what it makes).
fn shown(b: &dyn super::Bench, id: &str) -> Result<Vec<(String, String)>, String> {
    let page = PAGES
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| format!("no page {id}"))?;
    Ok(b.handbook_recipes(page)
        .iter()
        .map(|s: &Shown| {
            let by = match &s.made {
                Made::Grid(_) => "table".to_owned(),
                Made::Machine { machine, .. } => (*machine).to_owned(),
            };
            let out = match &s.out {
                Out::Item(id, _) | Out::Fluid(id, _) => id.clone(),
            };
            (by, out)
        })
        .collect())
}

fn has(list: &[(String, String)], by: &str, out: &str) -> bool {
    list.iter().any(|(b, o)| b == by && o == out)
}

fn pages_show_machine_recipes(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("handbook_pages_show_machine_recipes", plot_area(o)).check(
        "the alloys, dusts, fluids and blade pages find their machines' and tiers' recipes",
        |b, _| {
            let alloys = shown(b, "alloys")?;
            let wanted = [
                ("table", COPPERSTEEL),
                ("rustcraft:alloy", "rustcraft:sparksteel"),
                ("rustcraft:alloy", "rustcraft:crystallite"),
                ("rustcraft:alloy", "rustcraft:starsteel"),
            ];
            for (by, out) in wanted {
                if !has(&alloys, by, out) {
                    return Err(format!("the alloys' page lacks {out} by {by}: {alloys:?}"));
                }
            }
            let dusts = shown(b, "dusts")?;
            if !has(&dusts, "rustcraft:crusher", "rustcraft:dust_iron") {
                return Err(format!("the dusts' page lacks the crusher's: {dusts:?}"));
            }
            let fluids = shown(b, "fluids")?;
            if !has(&fluids, "rustcraft:mixer", "rustcraft:solution") {
                return Err(format!("the fluids' page lacks the mixer's: {fluids:?}"));
            }
            let blade = shown(b, "blade")?;
            for out in [
                "rustcraft:blade_2",
                "rustcraft:blade_3",
                "rustcraft:blade_4",
            ] {
                if !has(&blade, "table", out) {
                    return Err(format!("the blade's page lacks {out}: {blade:?}"));
                }
            }
            Ok(())
        },
    )
}
