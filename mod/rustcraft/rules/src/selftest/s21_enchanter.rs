//! 0.21 (spec 48, EW1–EW12): the enchanting machine's window and choice. The owner: «окно у
//! автомата вообще не понятное, как тут выбирать чары и так далее или хотя бы получать их».
//!
//! - A book, lapis and a full condenser beside it: an enchanted book comes out in the result slot,
//!   and the machine draws a new seed.
//! - The second offer chosen in the window: the item gets that offer's enchantments.
//! - «Форсаж» with its buffer full: it enchants (in 0.20 the step cost 960 E, the buffer held 600).
//! - On a storage's line with no lapis in its slot: it takes the lapis it needs from the storage,
//!   and its window says the result goes to the storage.
//! - «Повысить»: a sword with Sharpness II comes out with Sharpness III.

use super::slice15::{build, held, terminal_at};
use super::transport::{chest, fill};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::machines::processor::WorkMode;
use crate::machines::sheet::{Press, enchant_notes};
use crate::tuning::Tuning;
use crate::utility::enchanting::{OPTION_BUTTONS, VIEW_RAISE};
use serde_json::Value;

pub const ALL: [Build; 5] = [
    enchanter21_book_comes_out_enchanted,
    enchanter21_second_offer_gives_its_enchantments,
    enchanter21_boost_works_on_a_full_buffer,
    enchanter21_takes_lapis_from_the_storage,
    enchanter21_raises_an_enchantment,
];

const ENCHANTER: &str = "rustcraft:enchanter";
const TANK: &str = "rustcraft:xp_tank";
const STONE: &str = "minecraft:stone";
const BOOK: &str = "minecraft:book";
const ENCHANTED_BOOK: &str = "minecraft:enchanted_book";
const LAPIS: &str = "minecraft:lapis_lazuli";
const SWORD: &str = "minecraft:diamond_sword";
/// The condenser's points: a hundred levels' worth is its most; 3000 is past 30 levels.
const FULL: i64 = 3000;
/// Two steps of 10 s and the energy's fill.
const LIMIT: u64 = 800;

/// A machine at `p` with a full condenser east of it, both on stone.
fn machine(b: &mut dyn Bench, p: Pos, energy: f64) -> Result<(), String> {
    let tank = at(p, [1, 0, 0]);
    b.place(at(p, [0, -1, 0]), STONE, &[])?;
    b.place(at(tank, [0, -1, 0]), STONE, &[])?;
    b.place(p, ENCHANTER, &[])?;
    b.set_buffer(p, energy)?;
    b.place(tank, TANK, &[])?;
    b.set_long(tank, "util_xp", FULL)
}

/// The result slot's stack, as `slot_stacks` writes it.
fn result(b: &dyn Bench, p: Pos) -> Option<String> {
    b.slot_stacks(p).get(2).cloned().flatten()
}

/// The enchantment ids a text of the window names (`enchantment.minecraft.<id>` keys).
fn named(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(t)) = m.get("translate")
                && let Some(id) = t.strip_prefix("enchantment.minecraft.")
            {
                out.push(format!("minecraft:{id}"));
            }
            for (k, x) in m {
                if k != "translate" {
                    named(x, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| named(x, out)),
        _ => {}
    }
}

fn enchanter21_book_comes_out_enchanted(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("enchanter21_book_comes_out_enchanted", plot_area(o))
        .then("a machine with energy, a book, 3 lapis and a full condenser beside it", move |b, _| {
            machine(b, p, 600.0)?;
            b.put(p, 0, BOOK, 1)?;
            b.put(p, 1, LAPIS, 3)
        })
        .check("its window shows the ghosts of what is missing and «Лучшее» on", move |b, _| {
            let s = b.sheet(p).ok_or("no sheet")?;
            same("ghosts", s.ghosts.iter().map(Option::is_some).collect::<Vec<_>>(), vec![false, false, true])?;
            same("notes", s.notes.len(), enchant_notes::COUNT)?;
            if !s.toggles[OPTION_BUTTONS.start].on {
                return Err("«Лучшее» is not on".into());
            }
            Ok(())
        })
        .until("an enchanted book is in the result slot", LIMIT, move |b, _| {
            Ok(result(b, p).is_some_and(|r| r.starts_with(ENCHANTED_BOOK) && r.contains("minecraft:")))
        })
        .check("the book and the lapis are spent, the seed drawn anew", move |b, _| {
            same("books", count_of(&b.items(p), BOOK), 0)?;
            same("lapis", count_of(&b.items(p), LAPIS), 0)?;
            let r = result(b, p).unwrap_or_default();
            if r.split(' ').count() < 4 {
                return Err(format!("the book has no enchantment: {r}"));
            }
            if b.long(p, "enchant_seed") == 0 {
                return Err("no new seed".into());
            }
            Ok(())
        })
}

fn enchanter21_second_offer_gives_its_enchantments(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    // The second offer's enchantments, from the stage that reads them to the check.
    let offer = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let seen = offer.clone();
    Scenario::new("enchanter21_second_offer_gives_its_enchantments", plot_area(o))
        .then("a machine without energy, a sword, 3 lapis, a full condenser", move |b, _| {
            machine(b, p, 0.0)?;
            b.put(p, 0, SWORD, 1)?;
            b.put(p, 1, LAPIS, 3)
        })
        .then("the second offer chosen, the offer read, then the energy", move |b, _| {
            b.press(p, Press::Toggle(OPTION_BUTTONS.start + 2).id(), None)?;
            same("the pick kept", b.long(p, "enchant_pick"), 2)?;
            let s = b.sheet(p).ok_or("no sheet")?;
            let t = s.toggles.get(OPTION_BUTTONS.start + 2).ok_or("no second offer")?;
            if !t.on {
                return Err("the second offer is not on".into());
            }
            let mut ids = Vec::new();
            named(&t.hint, &mut ids);
            if ids.is_empty() {
                return Err(format!("the second offer names no enchantment: {}", t.hint));
            }
            *offer.lock().map_err(|e| e.to_string())? = ids;
            b.set_buffer(p, 600.0)
        })
        .until("the sword is enchanted", LIMIT, move |b, _| Ok(result(b, p).is_some_and(|r| r.starts_with(SWORD))))
        .check("with the offer's enchantments", move |b, _| {
            let r = result(b, p).unwrap_or_default();
            let ids = seen.lock().map_err(|e| e.to_string())?.clone();
            for id in &ids {
                if !r.contains(&format!("{id} ")) {
                    return Err(format!("{r} lacks {id} of the second offer {ids:?}"));
                }
            }
            Ok(())
        })
}


fn enchanter21_boost_works_on_a_full_buffer(o: Pos, _: &Tuning) -> Scenario {
    let p = at(o, [3, 1, 0]);
    Scenario::new("enchanter21_boost_works_on_a_full_buffer", plot_area(o))
        .then("a machine in «Форсаж», a book, lapis, a full condenser", move |b, _| {
            machine(b, p, 0.0)?;
            b.press(p, Press::Mode(WorkMode::Boost).id(), None)?;
            b.put(p, 0, BOOK, 1)?;
            b.put(p, 1, LAPIS, 3)
        })
        .then("its buffer full: a step's 960 E", move |b, _| b.set_buffer(p, 960.0))
        .check("the buffer keeps a step", move |b, _| {
            if b.buffer(p) + 1e-6 < 960.0 {
                return Err(format!("the buffer holds {} E, a step is 960", b.buffer(p)));
            }
            Ok(())
        })
        .until("an enchanted book is out", LIMIT, move |b, _| {
            Ok(result(b, p).is_some_and(|r| r.starts_with(ENCHANTED_BOOK)))
        })
}

fn enchanter21_takes_lapis_from_the_storage(o: Pos, _: &Tuning) -> Scenario {
    let p = chest(o, 3);
    let tank = at(o, [3, 0, -2]);
    Scenario::new("enchanter21_takes_lapis_from_the_storage", plot_area(o))
        .then("a storage line with an enchanting machine and a full condenser behind it", move |b, _| {
            build(b, o, &[])?;
            b.place(p, ENCHANTER, &[])?;
            b.set_buffer(p, 600.0)?;
            b.place(tank, TANK, &[])?;
            b.set_long(tank, "util_xp", FULL)?;
            fill(b, chest(o, 1), LAPIS, 8)
        })
        .until("the lapis is in the storage", 400, move |b, _| Ok(held(b, o, LAPIS)? == 8))
        .then("a book in the machine, its lapis slot empty", move |b, _| {
            same("lapis in the machine", count_of(&b.items(p), LAPIS), 0)?;
            b.put(p, 0, BOOK, 1)
        })
        .check("its window says the result goes to the storage", move |b, _| {
            let s = b.sheet(p).ok_or("no sheet")?;
            let note = s.notes.get(enchant_notes::RESULT).map(Value::to_string).unwrap_or_default();
            if !note.contains("to_storage") {
                return Err(format!("the result's label: {note}"));
            }
            Ok(())
        })
        .until("the book is enchanted with the storage's lapis", LIMIT, move |b, _| {
            let out = result(b, p).is_some_and(|r| r.starts_with(ENCHANTED_BOOK))
                || b.stored_stacks(terminal_at(o)).iter().any(|s| s.starts_with(ENCHANTED_BOOK));
            Ok(out)
        })
        .check("3 lapis left the storage", move |b, _| same("lapis stored", held(b, o, LAPIS)?, 5))
}

fn enchanter21_raises_an_enchantment(o: Pos, _: &Tuning) -> Scenario {
    const SHARP_2: &str = r#"{id:"minecraft:diamond_sword",count:1,components:{"minecraft:enchantments":{"minecraft:sharpness":2}}}"#;
    let p = at(o, [3, 1, 0]);
    Scenario::new("enchanter21_raises_an_enchantment", plot_area(o))
        .then("a machine without energy, a sword with Sharpness II, 5 lapis", move |b, _| {
            machine(b, p, 0.0)?;
            same("the sword in", b.insert_snbt(p, SHARP_2)?, 1)?;
            b.put(p, 1, LAPIS, 5)
        })
        .then("«Повысить», its first row, then the energy", move |b, _| {
            b.press(p, Press::Toggle(VIEW_RAISE).id(), None)?;
            b.press(p, Press::Toggle(OPTION_BUTTONS.start).id(), None)?;
            same("the pick", b.long(p, "enchant_pick"), 4)?;
            b.set_buffer(p, 600.0)
        })
        .until("the sword comes out", LIMIT, move |b, _| Ok(result(b, p).is_some_and(|r| r.starts_with(SWORD))))
        .check("with Sharpness III, the lapis spent", move |b, _| {
            let r = result(b, p).unwrap_or_default();
            if !r.contains("minecraft:sharpness 3") {
                return Err(format!("the sword: {r}"));
            }
            same("lapis", count_of(&b.items(p), LAPIS), 0)
        })
}
