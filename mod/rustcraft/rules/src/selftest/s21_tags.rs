//! 0.21 (spec 47): orders take any item of a tag. The owner: «склад при заказе книжных полок
//! почему-то ждёт только определённое бревно, а не берёт то, что есть в складу» — his storage
//! held thousands of birch logs, and an order for bookshelves waited for oak logs.
//!
//! Here a storage holds books and no wood: the order for a bookshelf names planks of any wood
//! (OT4). Birch logs come: the autocrafter makes birch planks from them and the bookshelf of
//! those (OT2), the two planks left over go back to the storage. An order for crystal shards,
//! which no recipe makes, is refused at once (OT8).

use super::slice15::{CRAFTER, FILL, build, crafter_at, held, holds, machine, put_book, terminal_at};
use super::transport::{chest, fill};
use super::{Build, Scenario, count_of, plot_area, same};
use crate::geom::Pos;
use crate::orders::groups::{GROUPS, group_key};
use crate::orders::queue::Wait;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [bookshelves_take_the_birch_logs_the_storage_holds];

const SHELF: &str = "minecraft:bookshelf";
const BOOK: &str = "minecraft:book";
const BIRCH_LOG: &str = "minecraft:birch_log";
const BIRCH_PLANKS: &str = "minecraft:birch_planks";
const OAK_LOG: &str = "minecraft:oak_log";
const OAK_PLANKS: &str = "minecraft:oak_planks";
const SHARD: &str = "rustcraft:crystal_shard";
const LOGS: u32 = 16;

fn bookshelves_take_the_birch_logs_the_storage_holds(o: Pos, _: &Tuning) -> Scenario {
    let books = [(BOOK, 3)];
    Scenario::new("orders21_bookshelves_take_the_birch_logs_the_storage_holds", plot_area(o))
        .then("a storage on a tier 3 line, a chest of 3 books, no wood", move |b, _| build(b, o, &books))
        .until("the storage holds the books", FILL, move |b, _| holds(b, o, &books))
        .then("an autocrafter with energy on the line, the book in the terminal", move |b, _| {
            machine(b, crafter_at(o), CRAFTER, true)?;
            put_book(b, o)
        })
        .until("the storage takes an order for a bookshelf", FILL, move |b, _| {
            Ok(b.order(terminal_at(o), SHELF, 1).is_ok())
        })
        .until("the order names 6 planks of any wood, not oak logs", 200, move |b, _| {
            let want = Wait::Items(vec![(group_key(GROUPS[0]), 6)]);
            Ok(b.orders(terminal_at(o)).first().is_some_and(|q| q.wait == want))
        })
        .check("an order for crystal shards, which no recipe makes, is refused (OT8)", move |b, _| {
            match b.order(terminal_at(o), SHARD, 9) {
                Err(why) if why.contains("not made on machines") => Ok(()),
                other => Err(format!("the shard order: {other:?}")),
            }
        })
        .then("16 birch logs come", move |b, _| fill(b, chest(o, 1), BIRCH_LOG, LOGS))
        .until("a bookshelf is back in the storage", 1600, move |b, _| Ok(held(b, o, SHELF)? == 1))
        .until("the order is done", 200, move |b, _| Ok(b.orders(terminal_at(o)).is_empty()))
        .until("the two birch planks left over are back in the storage", 400, move |b, _| {
            Ok(held(b, o, BIRCH_PLANKS)? == 2)
        })
        .check("two birch logs and the books went into it, no oak", move |b, _| {
            same("birch logs", held(b, o, BIRCH_LOG)?, u64::from(LOGS) - 2)?;
            same("books", held(b, o, BOOK)?, 0)?;
            same("oak logs", held(b, o, OAK_LOG)?, 0)?;
            same("oak planks", held(b, o, OAK_PLANKS)?, 0)?;
            same("birch planks in the crafter", count_of(&b.items(crafter_at(o)), BIRCH_PLANKS), 0)
        })
}
