//! 0.21.1 (spec 47, OT12, OT13): on a big storage the search still finds what the book can make,
//! and every enchanted stack keeps its names and enchantments.
//!
//! - A storage on a smart line with the recipe book in its terminal holds 400 kinds of named
//!   enchanted books (the owner's storage: about 400 kinds, many of them enchanted). Its list
//!   fits one message and names every kind, and has room neither for every stack's saved form
//!   nor for the items the book makes. Their own messages bring every item the book makes (a
//!   piston among them) and every stack, one for each line of a stack; the window's list takes
//!   them whole (`storage::orderable::complete`, `storage::variants::complete`), not cut.

use super::slice15::{PISTON, build, put_book, terminal_at};
use super::{Bench, Build, Scenario, plot_area, same};
use crate::geom::Pos;
use crate::storage::terminal::VARIANT;
use crate::storage::wire::{self, Listing, MAX_BYTES};
use crate::storage::{orderable, variants};
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

/// What the terminal sends, put together as the client does.
struct Sent {
    bytes: Vec<u8>,
    listing: Listing,
    parts: Vec<Vec<u8>>,
    items: orderable::Assembler,
    stacks: variants::Assembler,
}

fn sent(b: &dyn Bench, terminal: Pos) -> Result<Sent, String> {
    let w = b.terminal_wire(terminal).ok_or("the terminal sends nothing")?;
    let listing = wire::decode(&w.list).ok_or("the terminal's list does not read")?;
    let mut items = orderable::Assembler::default();
    for p in &w.orderable {
        items.take(p);
    }
    let mut stacks = variants::Assembler::default();
    for p in &w.variants {
        stacks.take(p);
    }
    let parts = w.orderable.into_iter().chain(w.variants).collect();
    Ok(Sent {
        bytes: w.list,
        listing,
        parts,
        items,
        stacks,
    })
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
            let Ok(s) = sent(b, terminal) else {
                return Ok(false);
            };
            Ok(s.items.list().is_some() && s.listing.lines.len() >= KINDS)
        })
        .check("the list fits one message, the book in the terminal", move |b, _| {
            let s = sent(b, terminal)?;
            same("fits one message", s.bytes.len() <= MAX_BYTES, true)?;
            same("each part fits a message", s.parts.iter().all(|p| p.len() <= MAX_BYTES), true)?;
            same("the book in the terminal", s.listing.book, true)?;
            same("every kind listed", s.listing.lines.len() >= KINDS, true)
        })
        .check("its own messages bring every item the book makes", move |b, _| {
            let s = sent(b, terminal)?;
            let whole = s.items.list().ok_or("the parts make no whole list")?.to_vec();
            same("the stamp names the list", s.items.stamp(), Some(orderable::stamp(&whole)))?;
            same("hundreds of items", whole.len() > 100, true)?;
            // The list alone (0.21.0) had no room for them: what this scenario is for.
            same("the list alone cuts them", s.listing.orderable.len() < whole.len(), true)?;
            let piston = b.item_number(PISTON).ok_or("the piston has no number")?;
            same("a piston among them", whole.contains(&piston), true)?;
            let mut shown = s.listing;
            orderable::complete(&mut shown, s.items.list());
            same("the window orders a piston", shown.orderable.contains(&piston), true)?;
            same("its list is whole", shown.orderable_cut, false)?;
            same("as long as the book's", shown.orderable.len(), whole.len())
        })
        .check("its own messages bring every enchanted stack", move |b, _| {
            let s = sent(b, terminal)?;
            let whole = s.stacks.list().ok_or("the stacks' parts make no whole list")?.to_vec();
            same("a stack for each kind", whole.len(), KINDS)?;
            same("the list names their stamp", s.listing.variants_stamp, s.stacks.stamp())?;
            // The list alone (0.21.0) cut them: the stacks past its room lost their names.
            same("the list alone cuts them", s.listing.variants.len() < KINDS, true)?;
            let mut shown = s.listing;
            variants::complete(&mut shown, &s.stacks);
            same("the window has every stack", shown.variants.len(), KINDS)?;
            let lines: Vec<(u16, u16)> = shown
                .lines
                .iter()
                .filter(|l| l.damage & VARIANT != 0)
                .map(|l| (l.item, l.damage & !VARIANT))
                .collect();
            same("a line for each stack", lines.len(), KINDS)?;
            let named = lines
                .iter()
                .all(|k| shown.variants.iter().any(|v| (v.item, v.index) == *k && !v.payload.is_empty()));
            same("each line has its saved stack", named, true)
        })
}
