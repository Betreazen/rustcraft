//! 0.21.1 (spec 47, OT12): on a big storage the search still finds what the book can make.
//!
//! - A storage on a smart line with the recipe book in its terminal holds 400 kinds of named
//!   enchanted books (the owner's storage: about 400 kinds, many of them enchanted). Its list
//!   fits one message and names every kind, and has no room left for the items the book makes;
//!   their own messages bring every one of them (a piston among them), and the window's list
//!   takes them whole (`storage::orderable::complete`), not cut.

use super::slice15::{PISTON, build, put_book, terminal_at};
use super::{Bench, Build, Scenario, plot_area, same};
use crate::geom::Pos;
use crate::storage::orderable::{self, Assembler};
use crate::storage::wire::{self, Listing, MAX_BYTES};
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [big_storage_still_orders];

/// Kinds of enchanted books in the storage.
const KINDS: usize = 400;
const LIMIT: u64 = 200;

fn book(i: usize) -> String {
    format!(
        r#"{{id:"minecraft:enchanted_book",count:1,components:{{"minecraft:stored_enchantments":{{"minecraft:sharpness":{}}},"minecraft:custom_name":"Book {i} of the big storage"}}}}"#,
        i % 5 + 1
    )
}

/// The terminal's list and the book's items, put together as the client does.
fn sent(b: &dyn Bench, terminal: Pos) -> Result<(Vec<u8>, Listing, Vec<Vec<u8>>, Assembler), String> {
    let (bytes, parts) = b.terminal_wire(terminal).ok_or("the terminal sends nothing")?;
    let listing = wire::decode(&bytes).ok_or("the terminal's list does not read")?;
    let mut a = Assembler::default();
    for p in &parts {
        a.take(p);
    }
    Ok((bytes, listing, parts, a))
}

fn big_storage_still_orders(o: Pos, _: &Tuning) -> Scenario {
    let terminal = terminal_at(o);
    Scenario::new("orderable21_big_storage_still_orders", plot_area(o))
        .then("a storage on a smart line, the book in its terminal", move |b, _| {
            build(b, o, &[])?;
            put_book(b, o)
        })
        .then("400 kinds of named enchanted books in the storage", move |b, _| {
            for i in 0..KINDS {
                b.stock_snbt(terminal, &book(i), 1 + i as u64)?;
            }
            Ok(())
        })
        .until("the book is built and the list names every kind", LIMIT, move |b, _| {
            let Ok((_, listing, parts, _)) = sent(b, terminal) else {
                return Ok(false);
            };
            Ok(!parts.is_empty() && listing.lines.len() >= KINDS)
        })
        .check("the list fits one message, the book in the terminal", move |b, _| {
            let (bytes, listing, _, _) = sent(b, terminal)?;
            same("fits one message", bytes.len() <= MAX_BYTES, true)?;
            same("the book in the terminal", listing.book, true)?;
            same("every kind listed", listing.lines.len() >= KINDS, true)
        })
        .check("its own messages bring every item the book makes", move |b, _| {
            let (_, listing, parts, a) = sent(b, terminal)?;
            same("each part fits a message", parts.iter().all(|p| p.len() <= MAX_BYTES), true)?;
            let whole = a.list().ok_or("the parts make no whole list")?.to_vec();
            same("the stamp names the list", a.stamp(), Some(orderable::stamp(&whole)))?;
            same("hundreds of items", whole.len() > 100, true)?;
            // The list alone (0.21.0) had no room for them: what this scenario is for.
            same("the list alone cuts them", listing.orderable.len() < whole.len(), true)?;
            let piston = b.item_number(PISTON).ok_or("the piston has no number")?;
            same("a piston among them", whole.contains(&piston), true)?;
            let mut shown = listing;
            orderable::complete(&mut shown, a.list());
            same("the window orders a piston", shown.orderable.contains(&piston), true)?;
            same("its list is whole", shown.orderable_cut, false)?;
            same("as long as the book's", shown.orderable.len(), whole.len())
        })
}
