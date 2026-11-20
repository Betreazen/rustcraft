//! 0.21.2 (spec 49, «Числа в терминале вводятся с клавиатуры»): the terminal's numbers typed.
//!
//! - A storage on a smart line with the recipe book in its terminal holds cobblestone. «3938»
//!   typed into the cobblestone's limit field over its 128 and sent as the client sends it
//!   (`storage::typed::encode_terminal`) makes its limit 3938; a typed 0 a limit of 0; «×» still
//!   clears it. A limit for an item the storage does not list (bedrock: none held, no limit, no
//!   recipe) and a number past the field's most are refused.
//! - «250» typed into the order's count makes the «Заказ» press carry 250 (its id reads back
//!   250), and an order of that count queues one order of 250 pistons.
//! - «300» typed into the pistons' «Держать» makes their N 300; a typed 0 clears it.

use super::slice15::{PISTON, build, held, put_book, terminal_at};
use super::{Bench, Build, Scenario, plot_area, same};
use crate::geom::Pos;
use crate::storage::terminal::{self, LimitChange, Press};
use crate::storage::typed::{self, Field, MAX_VALUE, Set, Setting, Typing};
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [typed_numbers_set_limit_keep_and_order];

const COBBLE: &str = "minecraft:cobblestone";
const BEDROCK: &str = "minecraft:bedrock";
const LIMIT: u64 = 400;

/// What a field holds after `typed` is typed into it, opened on `now`.
fn typed_value(field: Field, now: Option<u64>, typed: &str) -> Result<u64, String> {
    let mut t = Typing::start(field, now);
    for c in typed.chars() {
        t.type_char(c);
    }
    t.value().ok_or_else(|| format!("«{typed}» gives no number"))
}

fn number(b: &dyn Bench, item: &str) -> Result<u16, String> {
    b.item_number(item).ok_or_else(|| format!("{item} has no number"))
}

/// The number typed into the terminal's window, sent as its client sends it.
fn send(b: &mut dyn Bench, o: Pos, setting: Setting, item: &str, value: u64) -> Result<(), String> {
    let item = (number(b, item)?, 0);
    let set = Set {
        setting,
        item,
        value,
    };
    b.terminal_typed(&typed::encode_terminal(terminal_at(o), set))
}

fn limit_of(b: &dyn Bench, o: Pos, item: &str) -> Result<Option<u64>, String> {
    let v = b.storage(terminal_at(o)).ok_or("the terminal shows no storage")?;
    Ok(v.store.limits.get(item).map(|l| l.max))
}

fn keep_of(b: &dyn Bench, o: Pos, item: &str) -> Result<Option<u64>, String> {
    let v = b.storage(terminal_at(o)).ok_or("the terminal shows no storage")?;
    Ok(v.store.keep.get(item).copied())
}

/// The queue's orders of pistons a player made (not «держать»'s), by their counts.
fn piston_orders(b: &dyn Bench, o: Pos) -> Vec<u32> {
    b.orders(terminal_at(o))
        .into_iter()
        .filter(|x| !x.keep && x.item == PISTON)
        .map(|x| x.count)
        .collect()
}

fn typed_numbers_set_limit_keep_and_order(o: Pos, _: &Tuning) -> Scenario {
    let terminal = terminal_at(o);
    Scenario::new("typed21_numbers_set_limit_keep_and_order", plot_area(o))
        .then("a storage on a smart line, a chest of cobblestone", move |b, _| {
            build(b, o, &[(COBBLE, 5)])
        })
        .until("the storage holds the cobblestone", LIMIT, move |b, _| {
            Ok(held(b, o, COBBLE)? == 5)
        })
        .then("the book in the terminal, the cobblestone's limit 128", move |b, _| {
            put_book(b, o)?;
            b.change_limit(terminal, COBBLE, LimitChange::Add(64))?;
            b.change_limit(terminal, COBBLE, LimitChange::Add(64))
        })
        .until("the book is built", LIMIT, move |b, _| {
            Ok(b.terminal_wire(terminal).is_some_and(|w| !w.orderable.is_empty()))
        })
        .check("«3938» typed over the limit's 128 makes it 3938", move |b, _| {
            same("limit before", limit_of(b, o, COBBLE)?, Some(128))?;
            let value = typed_value(Field::Limit, Some(128), "3938")?;
            send(b, o, Setting::Limit, COBBLE, value)?;
            same("limit", limit_of(b, o, COBBLE)?, Some(3938))
        })
        .check("a typed 0 is a limit of 0; «×» still clears it", move |b, _| {
            send(b, o, Setting::Limit, COBBLE, typed_value(Field::Limit, Some(3938), "0")?)?;
            same("limit after 0", limit_of(b, o, COBBLE)?, Some(0))?;
            b.change_limit(terminal, COBBLE, LimitChange::Clear)?;
            same("limit after ×", limit_of(b, o, COBBLE)?, None)
        })
        .check("an item the storage does not list and a number past the most are refused", move |b, _| {
            same("bedrock refused", send(b, o, Setting::Limit, BEDROCK, 64).is_err(), true)?;
            same("bedrock's limit", limit_of(b, o, BEDROCK)?, None)?;
            same("past the most refused", send(b, o, Setting::Limit, COBBLE, MAX_VALUE + 1).is_err(), true)?;
            same("cobblestone's limit", limit_of(b, o, COBBLE)?, None)
        })
        .until("«250» typed into the order's count orders 250 pistons", LIMIT, move |b, _| {
            if !piston_orders(b, o).is_empty() {
                return Ok(true);
            }
            let count = typed_value(Field::OrderCount, Some(1), "250")?;
            let count = u16::try_from(count).map_err(|e| e.to_string())?;
            let press = Press::Order {
                item: number(b, PISTON)?,
                count,
            };
            // The «Заказ» press carries the count; the bench orders as the press's server side
            // does (its menu presses have no acting player to find the terminal's storage by).
            let id = terminal::encode(press).ok_or("the order has no press")?;
            same("the press's count", terminal::decode(id), Some(press))?;
            Ok(b.order(terminal, PISTON, u32::from(count)).is_ok())
        })
        .check("one order of 250 pistons", move |b, _| {
            same("orders of pistons", piston_orders(b, o), vec![250])
        })
        .check("«300» typed into «Держать» makes N 300; a typed 0 clears it", move |b, _| {
            send(b, o, Setting::Keep, PISTON, typed_value(Field::Keep, None, "300")?)?;
            same("N", keep_of(b, o, PISTON)?, Some(300))?;
            send(b, o, Setting::Keep, PISTON, typed_value(Field::Keep, Some(300), "0")?)?;
            same("N after 0", keep_of(b, o, PISTON)?, None)
        })
}
