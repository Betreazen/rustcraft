//! 0.21 (spec 48, EW1–EW12): the enchanting machine's window and choice. The owner: «окно у
//! автомата вообще не понятное, как тут выбирать чары и так далее или хотя бы получать их».
//!
//! - A book, lapis and a full condenser beside it: an enchanted book comes out in the result slot,
//!   and the machine draws a new seed.
//! - The second offer chosen in the window: the item gets that offer's enchantments.
//! - «Форсаж» with its buffer full: it enchants (in 0.20 the step cost 960 E, the buffer held 600).
//! - On a storage's line with no lapis in its slot: it takes the lapis it needs from the storage,
//!   and its window says the result goes to the storage.
//! - «Свои чары»: a sword with Sharpness II comes out with Sharpness III.
//! - «×»: a book of Sharpness III and Unbreaking II comes out with Unbreaking II alone, for a lapis;
//!   a book whose only enchantment comes off is a plain book.
//! - The anvil keeps a level above the most: a book of Efficiency VI on a pickaxe of Efficiency V
//!   gives VI; two of IV give V; two of VI stay VI.
//! - 0.21.2 (EW17; the owner: «нужно сделать тумблер уносить сразу на склад или нет»): on a
//!   storage's line with the result slot's toggle on «здесь», the enchanted book stays in the
//!   machine and the storage gets none; switched back to «на склад», it goes to the storage.

use super::slice15::{build, held, terminal_at};
use super::transport::{chest, fill};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::machines::processor::WorkMode;
use crate::machines::sheet::{Press, enchant_notes};
use crate::tuning::Tuning;
use crate::utility::enchanting::{KEEP_BUTTON, OPTION_BUTTONS, REMOVE_BUTTONS, VIEW_RAISE};
use serde_json::Value;

pub const ALL: [Build; 9] = [
    enchanter21_book_comes_out_enchanted,
    enchanter21_second_offer_gives_its_enchantments,
    enchanter21_boost_works_on_a_full_buffer,
    enchanter21_takes_lapis_from_the_storage,
    enchanter21_raises_an_enchantment,
    enchanter21_takes_an_enchantment_off_a_book,
    enchanter21_a_book_with_none_left_is_plain,
    enchanter21_anvil_keeps_a_level_above_the_most,
    enchanter21_keeps_the_result_here,
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
            keep_label(b, p, "to_storage", false)
        })
        .until("the book is enchanted with the storage's lapis", LIMIT, move |b, _| {
            let out = result(b, p).is_some_and(|r| r.starts_with(ENCHANTED_BOOK))
                || b.stored_stacks(terminal_at(o)).iter().any(|s| s.starts_with(ENCHANTED_BOOK));
            Ok(out)
        })
        .check("3 lapis left the storage", move |b, _| same("lapis stored", held(b, o, LAPIS)?, 5))
}

/// EW17: the result slot's toggle says `key` (`rustcraft.util.enchanter.<key>`) and is lit or not.
fn keep_label(b: &dyn Bench, p: Pos, key: &str, on: bool) -> Result<(), String> {
    let s = b.sheet(p).ok_or("no sheet")?;
    let lit = s.toggles.get(KEEP_BUTTON).ok_or("no result toggle")?.on;
    let text = s.toggle_texts.get(KEEP_BUTTON).cloned().flatten().unwrap_or(Value::Null).to_string();
    if !text.contains(&format!("rustcraft.util.enchanter.{key}\"")) || lit != on {
        return Err(format!("the result's label: {text}, lit {lit}; wanted {key}, lit {on}"));
    }
    Ok(())
}

/// The enchanted books the storage of the line at `o` holds.
fn stored_books(b: &dyn Bench, o: Pos) -> usize {
    b.stored_stacks(terminal_at(o)).iter().filter(|s| s.starts_with(ENCHANTED_BOOK)).count()
}

fn enchanter21_keeps_the_result_here(o: Pos, _: &Tuning) -> Scenario {
    let p = chest(o, 3);
    let tank = at(o, [3, 0, -2]);
    Scenario::new("enchanter21_keeps_the_result_here", plot_area(o))
        .then("a storage line with an enchanting machine set to «здесь», a full condenser", move |b, _| {
            build(b, o, &[])?;
            b.place(p, ENCHANTER, &[])?;
            b.set_buffer(p, 600.0)?;
            b.place(tank, TANK, &[])?;
            b.set_long(tank, "util_xp", FULL)?;
            b.press(p, Press::Toggle(KEEP_BUTTON).id(), None)?;
            same("kept in the block entity", b.long(p, "enchant_keep"), 1)?;
            keep_label(b, p, "keep_here", true)?;
            b.put(p, 1, LAPIS, 3)?;
            b.put(p, 0, BOOK, 1)
        })
        .until("an enchanted book is in the result slot", LIMIT, move |b, _| {
            Ok(result(b, p).is_some_and(|r| r.starts_with(ENCHANTED_BOOK)))
        })
        .wait(100)
        .check("it stays there, the storage gets none, the status says to take it", move |b, _| {
            if !result(b, p).is_some_and(|r| r.starts_with(ENCHANTED_BOOK)) {
                return Err(format!("the result slot: {:?}", result(b, p)));
            }
            same("enchanted books stored", stored_books(b, o), 0)?;
            let s = b.sheet(p).ok_or("no sheet")?;
            let status = s.status.map(|x| x.text.to_string()).unwrap_or_default();
            if !status.contains("enchanter.done\"") {
                return Err(format!("the status: {status}"));
            }
            Ok(())
        })
        .then("switched back to «на склад»", move |b, _| {
            b.press(p, Press::Toggle(KEEP_BUTTON).id(), None)?;
            same("kept in the block entity", b.long(p, "enchant_keep"), 0)?;
            keep_label(b, p, "to_storage", false)
        })
        .until("the book goes to the storage", 400, move |b, _| Ok(result(b, p).is_none() && stored_books(b, o) == 1))
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

/// The row of «Свои чары» that names `id`.
fn row_of(b: &dyn Bench, p: Pos, id: &str) -> Result<usize, String> {
    let s = b.sheet(p).ok_or("no sheet")?;
    (0..4)
        .find(|k| {
            let mut ids = Vec::new();
            named(&s.toggles[OPTION_BUTTONS.start + k].name, &mut ids);
            ids.iter().any(|i| i == id)
        })
        .ok_or_else(|| format!("no row of «Свои чары» names {id}"))
}

/// A machine with the book `snbt` and a lapis; «Свои чары», the «×» of `id`, then the energy.
fn take_off(b: &mut dyn Bench, p: Pos, snbt: &str, id: &str) -> Result<(), String> {
    machine(b, p, 0.0)?;
    same("the book in", b.insert_snbt(p, snbt)?, 1)?;
    b.put(p, 1, LAPIS, 1)?;
    b.press(p, Press::Toggle(VIEW_RAISE).id(), None)?;
    let k = row_of(b, p, id)?;
    b.press(p, Press::Toggle(REMOVE_BUTTONS.start + k).id(), None)?;
    same("the pick", b.long(p, "enchant_pick"), 5)?;
    b.set_buffer(p, 600.0)
}

fn enchanter21_takes_an_enchantment_off_a_book(o: Pos, _: &Tuning) -> Scenario {
    const TWO: &str = r#"{id:"minecraft:enchanted_book",count:1,components:{"minecraft:stored_enchantments":{"minecraft:sharpness":3,"minecraft:unbreaking":2}}}"#;
    let p = at(o, [3, 1, 0]);
    Scenario::new("enchanter21_takes_an_enchantment_off_a_book", plot_area(o))
        .then("a book of Sharpness III and Unbreaking II, «×» on Sharpness", move |b, _| take_off(b, p, TWO, "minecraft:sharpness"))
        .until("the book comes out", LIMIT, move |b, _| Ok(result(b, p).is_some()))
        .check("with Unbreaking II alone, the lapis spent", move |b, _| {
            same("the book", result(b, p), Some(format!("{ENCHANTED_BOOK} x1 minecraft:unbreaking 2")))?;
            same("lapis", count_of(&b.items(p), LAPIS), 0)
        })
}

fn enchanter21_a_book_with_none_left_is_plain(o: Pos, _: &Tuning) -> Scenario {
    const ONE: &str = r#"{id:"minecraft:enchanted_book",count:1,components:{"minecraft:stored_enchantments":{"minecraft:smite":2}}}"#;
    let p = at(o, [3, 1, 0]);
    Scenario::new("enchanter21_a_book_with_none_left_is_plain", plot_area(o))
        .then("a book of Smite II, «×» on Smite", move |b, _| take_off(b, p, ONE, "minecraft:smite"))
        .until("a plain book comes out", LIMIT, move |b, _| Ok(result(b, p) == Some(format!("{BOOK} x1"))))
}

fn enchanter21_anvil_keeps_a_level_above_the_most(o: Pos, _: &Tuning) -> Scenario {
    fn pick(level: u32) -> String {
        format!(r#"{{id:"minecraft:diamond_pickaxe",count:1,components:{{"minecraft:enchantments":{{"minecraft:efficiency":{level}}}}}}}"#)
    }
    fn book(level: u32) -> String {
        format!(r#"{{id:"minecraft:enchanted_book",count:1,components:{{"minecraft:stored_enchantments":{{"minecraft:efficiency":{level}}}}}}}"#)
    }
    Scenario::new("enchanter21_anvil_keeps_a_level_above_the_most", plot_area(o)).check(
        "Efficiency VI on V gives VI, IV and IV give V, VI and VI stay VI",
        move |b, _| {
            for (left, right, want) in [(5, 6, 6), (6, 5, 6), (4, 4, 5), (6, 6, 6)] {
                let made = b.anvil_snbt(&pick(left), &book(right))?.ok_or_else(|| format!("the anvil made nothing of {left} and {right}"))?;
                if !made.contains(&format!("minecraft:efficiency {want}")) {
                    return Err(format!("{left} and {right}: {made}, not Efficiency {want}"));
                }
            }
            Ok(())
        },
    )
}
