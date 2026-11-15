//! The engineer's handbook (spec H2; rustcraft_rules::handbook).
//!
//! - A pretend player carrying no alloy gets nothing; with coppersteel they get the handbook,
//!   and with sparksteel after it nothing more: the handbook comes once.
//! - Every page names an item the game knows, and the handbook's own recipe, book and
//!   coppersteel, is in the game's recipes as the page's grid reads it and makes the handbook.

use super::{Build, Scenario, plot_area, same};
use crate::geom::Pos;
use crate::handbook::{self, PAGES};
use crate::tuning::Tuning;

pub const ALL: [Build; 2] = [first_alloy_gives_the_handbook_once, pages_name_known_items];

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
                .filter(|i| !b.item_known(i))
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
