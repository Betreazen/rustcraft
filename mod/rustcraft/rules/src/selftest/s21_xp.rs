//! 0.21 (spec 44, XC1–XC10; owner 2026-11-19: «четвертым каналом просто в модельку корпуса
//! добавить зеленый канал. Ну, типа опыт»): experience rides the fourth built-in channel.
//!
//! - A butcher by one end of a line, a condenser three housings down it, a second condenser on a
//!   line of its own above: the kill's experience arrives in the first, none in the second.
//! - The same with the joint of the butcher's housing set «в блок» on the experience row: the
//!   kill's experience has nowhere to ride and is lost.
//! - An enchanting machine by one end of a line, a full condenser three housings down it: a book is
//!   enchanted with the condenser's experience.

use super::magistral::{place_housing, place_node};
use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::butcher;
use crate::geom::{Face, Pos};
use crate::net::items::Mode;
use crate::net::settings::JointSetting;
use crate::net::topology::XP_SLOT;
use crate::net::xp::CONDENSER;
use crate::tuning::Tuning;

pub const ALL: [Build; 3] = [
    a_kills_experience_rides_to_the_condenser_on_its_line,
    a_joint_into_the_butcher_keeps_its_experience_off_the_line,
    the_enchanter_draws_over_the_line,
];

const XP_KEY: &str = "util_xp";
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0c21;
/// A kill, its experience's ride and a margin: the butcher strikes once in 10 s.
const KILL: u64 = 900;

/// A stone floor, a node and seven housings of tier 1 along x at height 1, z −2 (the line), and a
/// node and seven housings at height 3 (the other line).
fn lines(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in -1..=9 {
        for z in -3..=3 {
            b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
        }
    }
    for y in [1, 3] {
        place_node(b, at(o, [0, y, -2]), 1)?;
        for x in 1..=7 {
            place_housing(b, at(o, [x, y, -2]), 1)?;
        }
    }
    Ok(())
}

/// A butcher with a sword, an owner and a full buffer at `p`, three cows in front of it.
fn butcher_with_cows(b: &mut dyn Bench, o: Pos, p: Pos) -> Result<(), String> {
    b.place(p, butcher::BLOCK, &[])?;
    b.set_owner(p, OWNER, 1)?;
    b.put(p, butcher::SWORD_SLOT, "minecraft:iron_sword", 1)?;
    b.set_buffer(p, butcher::BUFFER)?;
    for i in 0..3 {
        let at = [f64::from(o[0]) + 1.5 + f64::from(i), f64::from(o[1] + 1), f64::from(o[2]) + 2.5];
        b.spawn_mob("minecraft:cow", at, None)?;
    }
    Ok(())
}

fn a_kills_experience_rides_to_the_condenser_on_its_line(o: Pos, _: &Tuning) -> Scenario {
    // The butcher touches housing 2; the condenser touches housing 5, three housings on, and not
    // the butcher; the other condenser sits on housing 5 of the upper line.
    let p = at(o, [2, 1, -1]);
    let tank = at(o, [5, 1, -1]);
    let other = at(o, [5, 4, -2]);
    Scenario::new("xp21_kills_experience_rides_to_the_condenser_on_its_line", plot_area(o))
        .then("two lines, a condenser on each, a butcher on the lower one, three cows", move |b, _| {
            lines(b, o)?;
            b.place(tank, CONDENSER, &[])?;
            b.place(other, CONDENSER, &[])?;
            butcher_with_cows(b, o, p)?;
            b.refresh();
            Ok(())
        })
        .until("the kill's experience is in the condenser on its line", KILL, move |b, _| {
            Ok(b.long(tank, XP_KEY) > 0)
        })
        .check("a cow's 2 points; the other line's condenser got none", move |b, _| {
            same("points in the line's condenser", b.long(tank, XP_KEY), 2)?;
            same("points in the other line's condenser", b.long(other, XP_KEY), 0)
        })
}

fn a_joint_into_the_butcher_keeps_its_experience_off_the_line(o: Pos, _: &Tuning) -> Scenario {
    // XC3: the butcher's experience joint «в блок» gives nothing to the net.
    let p = at(o, [2, 1, -1]);
    let tank = at(o, [5, 1, -1]);
    Scenario::new("xp21_a_joint_into_the_butcher_keeps_its_experience_off_the_line", plot_area(o))
        .then("the line with its condenser, the butcher's experience joint set «в блок»", move |b, _| {
            lines(b, o)?;
            b.place(tank, CONDENSER, &[])?;
            b.set_joint(at(o, [2, 1, -2]), Face::ZPos, XP_SLOT, JointSetting::of_mode(Mode::Into))?;
            butcher_with_cows(b, o, p)?;
            b.refresh();
            Ok(())
        })
        .until("the butcher has made its kill", KILL, move |b, _| {
            Ok(b.butcher(p).is_some_and(|s| matches!(s, butcher::Shown::Working(_))))
        })
        .wait(100)
        .check("the condenser got nothing", move |b, _| same("points in the condenser", b.long(tank, XP_KEY), 0))
}

fn the_enchanter_draws_over_the_line(o: Pos, _: &Tuning) -> Scenario {
    const BOOK: &str = "minecraft:book";
    const LAPIS: &str = "minecraft:lapis_lazuli";
    const ENCHANTED: &str = "minecraft:enchanted_book";
    const HELD: i64 = 3000;
    let p = at(o, [2, 1, -1]);
    let tank = at(o, [5, 1, -1]);
    Scenario::new("xp21_the_enchanter_draws_over_the_line", plot_area(o))
        .then(
            "an enchanting machine by housing 2 with a book and 3 lapis, a condenser of 3000 points by housing 5",
            move |b, _| {
                lines(b, o)?;
                b.place(tank, CONDENSER, &[])?;
                b.set_long(tank, XP_KEY, HELD)?;
                b.place(p, "rustcraft:enchanter", &[])?;
                b.set_buffer(p, 600.0)?;
                b.put(p, 0, BOOK, 1)?;
                b.put(p, 1, LAPIS, 3)?;
                b.refresh();
                Ok(())
            },
        )
        .until("an enchanted book is out", 800, move |b, _| Ok(count_of(&b.items(p), ENCHANTED) == 1))
        .check("the condenser three housings away paid for it", move |b, _| {
            let left = b.long(tank, XP_KEY);
            if left >= HELD || left <= 0 {
                return Err(format!("the condenser holds {left} of {HELD}: it paid nothing or all"));
            }
            Ok(())
        })
}
