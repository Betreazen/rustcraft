//! Slices 2–3: items along an item channel net from chest to chest: the rate of each tier (M10),
//! a joint's filter and priority (M7, M8), and a full receiver that leaves the items where they
//! are (M6).

use super::magistral::{channel, line};
use super::{Bench, Build, Scenario, at, count_of, expect, near, plot_area, same, total};
use crate::geom::{Face, Pos};
use crate::net::items::Mode;
use crate::net::settings::JointSetting;
use crate::net::topology::ChannelKind;
use crate::tuning::Tuning;

pub const ALL: [Build; 8] = [
    rate_1,
    rate_2,
    rate_3,
    rate_4,
    filter,
    full_receiver,
    priority_first,
    equal_priorities_take_turns,
];

pub const CHEST: &str = "minecraft:chest";
pub const COBBLE: &str = "minecraft:cobblestone";
const IRON: &str = "minecraft:iron_ingot";
const DIRT: &str = "minecraft:dirt";
/// A chest's room: 27 stacks of 64.
const CHEST_ROOM: u32 = 27 * 64;
/// The window a rate is measured over, in ticks: a whole number of the nets' moves.
const WINDOW: u64 = 40;
/// How long items may take to start moving after the net is built.
const START_LIMIT: u64 = 100;

/// The chest beside housing `k` of a line built from `o` (on the housing's z− side).
pub fn chest(o: Pos, k: i32) -> Pos {
    at(o, [k, 0, -1])
}

/// The housing `k` of a line built from `o`.
pub fn housing(o: Pos, k: i32) -> Pos {
    at(o, [k, 0, 0])
}

/// A node, `len` housings with an item channel of `tier` in slot 0, and a chest beside each
/// housing of `chests`.
pub fn item_line(
    b: &mut dyn Bench,
    o: Pos,
    tier: u8,
    len: i32,
    chests: &[i32],
) -> Result<(), String> {
    line(
        b,
        o,
        1,
        len,
        [channel(ChannelKind::Item, tier), None, None, None],
    )?;
    for &k in chests {
        b.place(chest(o, k), CHEST, &[])?;
    }
    Ok(())
}

/// The joint of housing `k` toward its chest, slot 0.
pub fn joint(b: &mut dyn Bench, o: Pos, k: i32, mode: Mode, priority: i32) -> Result<(), String> {
    let mut setting = JointSetting::of_mode(mode);
    setting.priority = priority;
    b.set_joint(housing(o, k), Face::ZNeg, 0, setting)
}

/// Fills the container at `pos` with `count` of `item`; a fail when it does not all fit.
pub fn fill(b: &mut dyn Bench, pos: Pos, item: &str, count: u32) -> Result<(), String> {
    let went = b.insert(pos, item, count)?;
    same("items put in", went, count)
}

fn rate_1(o: Pos, t: &Tuning) -> Scenario {
    rate(o, t, 1, "items_rate_tier_1")
}

fn rate_2(o: Pos, t: &Tuning) -> Scenario {
    rate(o, t, 2, "items_rate_tier_2")
}

fn rate_3(o: Pos, t: &Tuning) -> Scenario {
    rate(o, t, 3, "items_rate_tier_3")
}

fn rate_4(o: Pos, t: &Tuning) -> Scenario {
    rate(o, t, 4, "items_rate_tier_4")
}

/// M10: a net moves its tier's items a second, measured once it runs.
fn rate(o: Pos, t: &Tuning, tier: u8, name: &'static str) -> Scenario {
    let wanted = expect::items_in(t, tier, WINDOW);
    let supply = (expect::items_in(t, tier, 3 * WINDOW) as u32 + 64).min(CHEST_ROOM);
    // A move carries a fraction of a second's items: one move's worth either way, and one item.
    let tolerance = 1.0 + expect::items_in(t, tier, 4);
    let (a, bb) = (chest(o, 1), chest(o, 3));
    Scenario::new(name, plot_area(o))
        .then("build chest, net, chest", move |b, _| {
            item_line(b, o, tier, 3, &[1, 3])?;
            joint(b, o, 1, Mode::From, 0)?;
            fill(b, a, COBBLE, supply)
        })
        .until("the first items arrive", START_LIMIT, move |b, m| {
            let n = count_of(&b.items(bb), COBBLE);
            m.mark("start", f64::from(n));
            Ok(n > 0)
        })
        .wait(WINDOW)
        .check("the net moved its rate", move |b, m| {
            let moved = f64::from(count_of(&b.items(bb), COBBLE)) - m.get("start");
            near(
                &format!("tier {tier}: items in {WINDOW} ticks"),
                moved,
                wanted,
                tolerance,
            )
        })
}

/// M7: a white list on the receiving joint lets only its item through; the rest stays put.
fn filter(o: Pos, _: &Tuning) -> Scenario {
    let (a, bb) = (chest(o, 1), chest(o, 3));
    Scenario::new("items_filter", plot_area(o))
        .then("build with a white list on the receiver", move |b, _| {
            item_line(b, o, 2, 3, &[1, 3])?;
            joint(b, o, 1, Mode::From, 0)?;
            let mut only_iron = JointSetting::of_mode(Mode::Into);
            only_iron.filter.allow_only = true;
            only_iron.filter.items.insert(IRON.into());
            b.set_joint(housing(o, 3), Face::ZNeg, 0, only_iron)?;
            fill(b, a, COBBLE, 32)?;
            fill(b, a, IRON, 32)
        })
        .until("the iron arrives", START_LIMIT, move |b, _| {
            Ok(count_of(&b.items(bb), IRON) == 32)
        })
        .wait(20)
        .check("no cobblestone passed and none was lost", move |b, _| {
            same(
                "cobblestone in the receiver",
                count_of(&b.items(bb), COBBLE),
                0,
            )?;
            same(
                "cobblestone in the source",
                count_of(&b.items(a), COBBLE),
                32,
            )
        })
}

/// M6: items wait in their source while the receiver is full, and the receiver says so.
fn full_receiver(o: Pos, _: &Tuning) -> Scenario {
    let (a, bb) = (chest(o, 1), chest(o, 3));
    Scenario::new("items_full_receiver_keeps_them", plot_area(o))
        .then("build with a full receiver", move |b, _| {
            item_line(b, o, 2, 3, &[1, 3])?;
            joint(b, o, 1, Mode::From, 0)?;
            fill(b, bb, DIRT, CHEST_ROOM)?;
            fill(b, a, COBBLE, 16)
        })
        .wait(WINDOW)
        .check("nothing moved, nothing was lost", move |b, _| {
            same(
                "cobblestone in the source",
                count_of(&b.items(a), COBBLE),
                16,
            )?;
            same("items in the receiver", total(&b.items(bb)), CHEST_ROOM)
        })
        .check(
            "the receiver shows «Приёмник полон»",
            move |b, _| same("receiver full", b.receiver_full(bb), true),
        )
}

/// M8: the receiver of the higher priority fills first.
fn priority_first(o: Pos, _: &Tuning) -> Scenario {
    let (a, high, low) = (chest(o, 1), chest(o, 3), chest(o, 5));
    Scenario::new("items_priority_first", plot_area(o))
        .then("build two receivers of priorities 1 and 0", move |b, _| {
            item_line(b, o, 2, 5, &[1, 3, 5])?;
            joint(b, o, 1, Mode::From, 0)?;
            joint(b, o, 3, Mode::Into, 1)?;
            joint(b, o, 5, Mode::Into, 0)?;
            fill(b, a, COBBLE, 16)
        })
        .until("the source is empty", START_LIMIT, move |b, _| {
            Ok(total(&b.items(a)) == 0)
        })
        .check("all went to the higher priority", move |b, _| {
            same("priority 1", count_of(&b.items(high), COBBLE), 16)?;
            same("priority 0", count_of(&b.items(low), COBBLE), 0)
        })
}

/// M8: receivers of one priority take turns.
fn equal_priorities_take_turns(o: Pos, _: &Tuning) -> Scenario {
    let (a, first, second) = (chest(o, 1), chest(o, 3), chest(o, 5));
    Scenario::new("items_equal_priorities_take_turns", plot_area(o))
        .then("build two receivers of one priority", move |b, _| {
            item_line(b, o, 2, 5, &[1, 3, 5])?;
            joint(b, o, 1, Mode::From, 0)?;
            fill(b, a, COBBLE, 16)
        })
        .until("the source is empty", START_LIMIT, move |b, _| {
            Ok(total(&b.items(a)) == 0)
        })
        .check("they shared it", move |b, _| {
            let x = count_of(&b.items(first), COBBLE);
            let y = count_of(&b.items(second), COBBLE);
            same("both together", x + y, 16)?;
            near("the first receiver", f64::from(x), f64::from(y), 1.0)
        })
}
