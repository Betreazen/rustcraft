//! Slice 7: the base anchor (B1, B2, B5, B6). With its owner on the server and its draw met it
//! holds its own chunk; a radius module asks 3×3 chunks' draw, which one crystal does not meet;
//! its owner away, it holds nothing; a second anchor of the same owner switches the first off.
//! The self-test's owners are pretend players the anchors take as on the server.

use super::{Build, Scenario, at, plot_area, same};
use crate::anchor::State;
use crate::geom::Pos;
use crate::modules::Module;
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [
    holds_its_chunk,
    not_enough_energy,
    owner_away,
    second_one_replaces,
];

const ANCHOR: &str = "rustcraft:anchor";
const CRYSTAL: &str = "rustcraft:crystal";
/// Each scenario has its own owner: one anchor a player (B6) would set them against each other.
const ONLINE: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_00a1;
const ONLINE_2: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_00a3;
const ONLINE_3: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_00a4;
const AWAY: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_00a2;
/// The anchors decide once a second.
const SECONDS: u64 = 80;

fn chunk(p: Pos) -> [i32; 2] {
    [p[0] >> 4, p[2] >> 4]
}

/// An anchor of `owner` with `modules` and a crystal beside it.
fn anchor(name: &'static str, o: Pos, owner: u128, modules: Vec<&'static str>) -> Scenario {
    let a = at(o, [4, 0, 0]);
    Scenario::new(name, plot_area(o)).then("an anchor beside a crystal", move |b, _| {
        for p in [ONLINE, ONLINE_2, ONLINE_3] {
            b.pretend_online(p, true);
        }
        b.place(a, ANCHOR, &[])?;
        b.set_owner(a, owner, 10)?;
        b.set_modules(a, &modules)?;
        b.place(at(a, [1, 0, 0]), CRYSTAL, &[])
    })
}

/// The anchor and its chunk go when its scenario ends.
fn end(s: Scenario, o: Pos) -> Scenario {
    let a = at(o, [4, 0, 0]);
    s.then("the anchor taken away", move |b, _| {
        b.place(a, "minecraft:air", &[])
    })
    .until("its chunk let go", SECONDS, move |b, _| {
        Ok(!b.held_chunks().contains(&chunk(a)))
    })
}

fn holds_its_chunk(o: Pos, _: &Tuning) -> Scenario {
    let a = at(o, [4, 0, 0]);
    let s = anchor("anchor_holds_its_chunk", o, ONLINE, Vec::new()).until(
        "B5: one chunk held on 2 E/s",
        SECONDS,
        move |b, _| Ok(b.anchor(a) == Some(State::Holding) && b.held_chunks().contains(&chunk(a))),
    );
    end(s, o)
}

fn not_enough_energy(o: Pos, t: &Tuning) -> Scenario {
    let a = at(o, [4, 0, 0]);
    let draw = t.anchor_energy_per_chunk * 9.0;
    let crystal = t.crystal_output;
    let s = anchor(
        "anchor_without_its_draw",
        o,
        ONLINE_2,
        vec![Module::Radius.item()],
    )
    .check("3×3 asks more than a crystal gives", move |_, _| {
        if draw > crystal {
            Ok(())
        } else {
            Err(format!("{draw} E/s fits in {crystal}"))
        }
    })
    .until("B2: it holds nothing", SECONDS, move |b, _| {
        Ok(b.anchor(a) == Some(State::NoEnergy))
    });
    end(s, o)
}

fn owner_away(o: Pos, _: &Tuning) -> Scenario {
    let a = at(o, [4, 0, 0]);
    anchor("anchor_owner_away", o, AWAY, Vec::new()).until(
        "B1: off while its owner is away",
        SECONDS,
        move |b, _| Ok(b.anchor(a) == Some(State::OwnerAway)),
    )
}

fn second_one_replaces(o: Pos, _: &Tuning) -> Scenario {
    let a = at(o, [4, 0, 0]);
    let newer = at(o, [30, 0, 0]);
    let s = anchor("anchor_second_one_replaces", o, ONLINE_3, Vec::new())
        .then("a newer anchor of the same owner", move |b, _| {
            b.place(newer, ANCHOR, &[])?;
            b.set_owner(newer, ONLINE_3, 20)?;
            b.place(at(newer, [1, 0, 0]), CRYSTAL, &[])
        })
        .until(
            "B6: the first waits, the newer holds",
            SECONDS,
            move |b, _| {
                Ok(b.anchor(a) == Some(State::Replaced) && b.anchor(newer) == Some(State::Holding))
            },
        )
        .then("the newer taken away", move |b, _| {
            b.place(newer, "minecraft:air", &[])
        })
        .until("the first works again", SECONDS * 2, move |b, _| {
            Ok(b.anchor(a) == Some(State::Holding))
        })
        .check("both are known by their positions", move |_, _| {
            same("apart", a == newer, false)
        });
    end(s, o)
}
