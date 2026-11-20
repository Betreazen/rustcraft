//! 0.21.2 (spec 32, SP10–SP15): a storage's limit never refuses what is given to it; what it holds
//! over the limit goes round the disintegrator and the trash until every item is at its limit.
//!
//! - A machine's products past the limit leave the machine for the storage, which holds them with
//!   nowhere to send them («некуда девать»); a trash put on the line burns them down to the limit
//!   (SP10, SP12, SP15).
//! - Iron pickaxes the player puts in past a limit of 0: the disintegrator takes them apart, the
//!   ingots past their limit and the sticks burn; the storage ends at its limits (SP10, SP12).
//! - A chest on its default joint gives the storage only up to its limit, so it is not emptied
//!   into the trash (SP11).
//! - The player's deposit past the limit is taken with no sink on the line (SP10).

use super::magistral::line;
use super::s21_surplus::{TRASH, build, dis_at, held, limit, machine, trash_at, trash_joint};
use super::slice15::{CELL, FURNACE, TERMINAL, terminal_at};
use super::transport::{chest, fill};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same, total};
use crate::geom::Pos;
use crate::storage::wire::{self, Line};
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [
    machine_output_goes_in_past_the_limit,
    deposit_recycled_to_the_limits,
    default_chest_not_drained,
    deposit_past_the_limit_with_no_sink,
];

const PICK: &str = "minecraft:iron_pickaxe";
const INGOT: &str = "minecraft:iron_ingot";
const STICK: &str = "minecraft:stick";
const COBBLE: &str = "minecraft:cobblestone";
/// The electric furnace's product slot.
const FURNACE_OUTPUT: usize = 2;

/// The terminal's list line of `item` as the server sends it.
fn wire_line(b: &dyn Bench, t: Pos, item: &str) -> Result<Line, String> {
    let number = b.item_number(item).ok_or_else(|| format!("{item} has no number"))?;
    let w = b.terminal_wire(t).ok_or("the terminal sends no list")?;
    let listing = wire::decode(&w.list).ok_or("the list does not read")?;
    listing
        .lines
        .into_iter()
        .find(|l| l.item == number && l.damage == 0)
        .ok_or_else(|| format!("no line of {item}"))
}

fn machine_output_goes_in_past_the_limit(o: Pos, _: &Tuning) -> Scenario {
    // The owner: «вещи … не забираются из блоков машин, если я выставил лимит».
    let (f, t) = (chest(o, 1), terminal_at(o));
    Scenario::new("limits21_machine_output_goes_in_past_the_limit", plot_area(o))
        .then("an electric furnace and a storage on one line, no trash", move |b, _| {
            line(b, o, 2, 6, 2)?;
            b.place(f, FURNACE, &[])?;
            for c in [[3, 0, 1], [4, 0, 1], [3, 0, 2], [4, 0, 2]] {
                b.place(at(o, c), CELL, &[])?;
            }
            b.place(terminal_at(o), TERMINAL, &[])
        })
        .until("the storage is found", 200, move |b, _| Ok(b.storage(t).is_some()))
        .then("a limit of 5 iron ingots; 20 ingots in the furnace's product slot", move |b, _| {
            limit(b, t, INGOT, 5)?;
            b.put(f, FURNACE_OUTPUT, INGOT, 20)
        })
        .until("the furnace is emptied: the storage holds all 20 past its limit", 600, move |b, _| {
            Ok(count_of(&b.items(f), INGOT) == 0 && held(b, t, INGOT)? == 20)
        })
        .check("the terminal says 15 over the limit with nowhere to go", move |b, _| {
            let l = wire_line(b, t, INGOT)?;
            same("count", l.count, 20)?;
            same("limit", l.limit.map(|l| l.max), Some(5))?;
            same("routed", l.routed, false)
        })
        .then("a trash on the line", move |b, _| {
            machine(b, trash_at(o), TRASH)?;
            trash_joint(b, o, 6)
        })
        .until("the 15 over the limit burn", 600, move |b, _| {
            Ok(held(b, t, INGOT)? == 5 && total(&b.items(trash_at(o))) == 0)
        })
        .wait(40)
        .check("the storage keeps its 5; the terminal says the surplus is sent on", move |b, _| {
            same("ingots", held(b, t, INGOT)?, 5)?;
            same("routed", wire_line(b, t, INGOT)?.routed, true)
        })
}

fn deposit_recycled_to_the_limits(o: Pos, _: &Tuning) -> Scenario {
    // The owner: «если выставлен лимит, тогда начинается цикл переработки и снова и снова, пока
    // именно после работы дезинтегратора и утилизатора не останется предметов которые помещаются
    // в лимит». Two pickaxes give 3 ingots and 2 sticks (DS1): one ingot kept, the rest burns.
    let (dis, trash, t) = (dis_at(o), trash_at(o), terminal_at(o));
    Scenario::new("limits21_deposit_recycled_to_the_limits", plot_area(o))
        .then("a storage, a disintegrator and a trash on one line", move |b, _| build(b, o, 2, true, true))
        .until("the storage is found", 200, move |b, _| Ok(b.storage(t).is_some()))
        .then("limits: no pickaxes, one ingot, no sticks", move |b, _| {
            limit(b, t, PICK, 0)?;
            limit(b, t, INGOT, 1)?;
            limit(b, t, STICK, 0)
        })
        .then("the player puts two iron pickaxes in", move |b, _| {
            same("taken past the limit", b.deposit(t, PICK, 2)?, 2)
        })
        .until("taken apart; one ingot kept, the other ingots and the sticks burnt", 2400, move |b, _| {
            Ok(total(&b.items(dis)) == 0
                && total(&b.items(trash)) == 0
                && held(b, t, PICK)? == 0
                && held(b, t, INGOT)? == 1
                && held(b, t, STICK)? == 0)
        })
        .wait(100)
        .check("the loop has ended at the limits", move |b, _| {
            same("pickaxes", held(b, t, PICK)?, 0)?;
            same("ingots", held(b, t, INGOT)?, 1)?;
            same("sticks", held(b, t, STICK)?, 0)?;
            same("in the disintegrator", total(&b.items(dis)), 0)
        })
}

fn default_chest_not_drained(o: Pos, _: &Tuning) -> Scenario {
    // SP11: the player's own chest beside a line keeps what is over the storage's limit, a trash on
    // the line or not. (A chest set «из блока» empties: surplus21_trash_without_a_filter_takes_only_surplus.)
    let (own, trash, t) = (at(o, [2, 0, 1]), trash_at(o), terminal_at(o));
    Scenario::new("limits21_default_chest_not_drained", plot_area(o))
        .then("a storage, a trash and a chest on its default", move |b, _| {
            build(b, o, 2, false, true)?;
            b.place(own, "minecraft:chest", &[])
        })
        .until("the storage is found", 200, move |b, _| Ok(b.storage(t).is_some()))
        .then("a limit of 5 cobblestone; 20 in the chest on its default", move |b, _| {
            limit(b, t, COBBLE, 5)?;
            fill(b, own, COBBLE, 20)
        })
        .until("the storage holds its 5", 400, move |b, _| Ok(held(b, t, COBBLE)? == 5))
        .wait(100)
        .check("the chest on its default keeps the other 15, nothing burnt", move |b, _| {
            same("in the chest", count_of(&b.items(own), COBBLE), 15)?;
            same("stored", held(b, t, COBBLE)?, 5)?;
            same("in the trash", total(&b.items(trash)), 0)
        })
}

fn deposit_past_the_limit_with_no_sink(o: Pos, _: &Tuning) -> Scenario {
    // The owner: «вещи не кладутся в склад из моего инвентаря … если я выставил лимит».
    let t = terminal_at(o);
    Scenario::new("limits21_deposit_past_the_limit_with_no_sink", plot_area(o))
        .then("a storage on a line, no disintegrator, no trash", move |b, _| build(b, o, 2, false, false))
        .until("the storage is found", 200, move |b, _| Ok(b.storage(t).is_some()))
        .then("a limit of 10 cobblestone; the player puts 64 in", move |b, _| {
            limit(b, t, COBBLE, 10)?;
            same("taken", b.deposit(t, COBBLE, 64)?, 64)
        })
        .wait(60)
        .check("the storage holds all 64; the terminal says 54 over with nowhere to go", move |b, _| {
            same("stored", held(b, t, COBBLE)?, 64)?;
            let l = wire_line(b, t, COBBLE)?;
            same("count", l.count, 64)?;
            same("routed", l.routed, false)
        })
}
