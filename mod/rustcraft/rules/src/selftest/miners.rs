//! Slice 8: the miner, the block placer and the block breaker. The miner finds an ore in its
//! square, digs it with its owner's `break … pickaxe` and puts back the rock round it with a
//! `place` (D1, D2, X1, X6), leaves a switched-off ore and an ore outside its square (D5), stops
//! when it has nowhere to put what it digs, and works its whole square out (D4). A placer and a
//! breaker facing each other turn cobblestone over into a chest, the placer reporting each block
//! and the breaker nothing (X6, X7); a placer waits on a taken cell, a breaker spares bedrock.

use super::{Bench, Build, Scenario, at, count_of, plot_area, same};
use crate::geom::Pos;
use crate::mining::{self as rules, Ore, Shown};
use crate::tuning::Tuning;

pub const ALL: [Build; 6] = [
    miner_digs_an_ore,
    miner_leaves_what_it_should,
    miner_full,
    miner_works_out_its_square,
    placer_and_breaker,
    placer_waits_and_breaker_spares,
];

const MINER: &str = "rustcraft:miner";
const PLACER: &str = "rustcraft:placer";
const BREAKER: &str = "rustcraft:breaker";
const CRYSTAL: &str = "rustcraft:crystal";
const CHEST: &str = "minecraft:chest";
const STONE: &str = "minecraft:stone";
const COBBLE: &str = "minecraft:cobblestone";
const IRON_ORE: &str = "minecraft:iron_ore";
const DIM: &str = "minecraft:overworld";
/// The scenarios' owner: its miners report as theirs.
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_00b1;
/// Ticks past the rules' count a dig may be seen late: the rescan after building, the tick it
/// is read in, the chest's push every 8 ticks.
const SLACK: u64 = 60;
const LIMIT: u64 = 400;

/// The miner of a scenario: 6 above the plot's floor, two crystals beside it (8 E/s, its draw).
fn miner_at(o: Pos) -> Pos {
    at(o, [5, 6, 3])
}

fn build_miner(b: &mut dyn Bench, o: Pos, chest: bool) -> Result<Pos, String> {
    let m = miner_at(o);
    b.place(m, MINER, &[])?;
    b.set_owner(m, OWNER, 1)?;
    b.place(at(m, [1, 0, 0]), CRYSTAL, &[])?;
    b.place(at(m, [-1, 0, 0]), CRYSTAL, &[])?;
    if chest {
        b.place(at(m, [0, 1, 0]), CHEST, &[])?;
    }
    Ok(m)
}

/// An ore at `pos` in a shell of stone.
fn ore(b: &mut dyn Bench, pos: Pos, block: &str) -> Result<(), String> {
    for d in [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ] {
        let p = at(pos, d);
        if b.block(p) == "minecraft:air" {
            b.place(p, STONE, &[])?;
        }
    }
    b.place(pos, block, &[])
}

/// The ticks the rules give a miner at full energy to reach and dig `cell` from a fresh start.
fn dig_ticks(b: &dyn Bench, m: Pos, cell: Pos, t: &Tuning) -> Result<u64, String> {
    let side = t.miner_sides[0];
    let total = rules::cells(m, side, b.min_y());
    let index = (0..total)
        .find(|&i| rules::cell(m, side, b.min_y(), i) == Some(cell))
        .ok_or_else(|| format!("{cell:?} is not in the square"))?;
    Ok(rules::ticks_to_dig(index, 1.0, 1.0, 1.0, t))
}

fn reported(b: &dyn Bench, line: &str) -> bool {
    b.reports().iter().any(|r| r.ends_with(line))
}

fn miner_digs_an_ore(o: Pos, t: &Tuning) -> Scenario {
    let m = miner_at(o);
    let iron = at(m, [2, -6, -1]);
    let far = at(m, [6, -4, 0]);
    let t = t.clone();
    Scenario::new("mining_miner_digs_an_ore_in_its_square", plot_area(o))
        .then(
            "a miner with a chest, an iron ore below, another outside its square",
            move |b, m2| {
                build_miner(b, o, true)?;
                ore(b, iron, IRON_ORE)?;
                ore(b, far, IRON_ORE)?;
                m2.mark("ticks", dig_ticks(b, m, iron, &t)? as f64);
                Ok(())
            },
        )
        .until(
            "D1, D2: the ore is stone again within the rules' time",
            LIMIT,
            move |b, memo| {
                let done = b.block(iron) == STONE;
                if done {
                    memo.mark("at", memo.elapsed as f64);
                }
                Ok(done)
            },
        )
        .check("it took the rules' ticks", move |_, memo| {
            let (at_tick, ticks) = (memo.get("at"), memo.get("ticks"));
            if at_tick <= ticks + SLACK as f64 {
                Ok(())
            } else {
                Err(format!("dug at {at_tick}, the rules say {ticks}"))
            }
        })
        .until("MN1: its raw iron went to the chest", LIMIT, move |b, _| {
            Ok(count_of(&b.items(at(m, [0, 1, 0])), "minecraft:raw_iron") >= 1)
        })
        .check("the ore outside its square stays", move |b, _| {
            same("the far ore", b.block(far), IRON_ORE.to_owned())
        })
        .until(
            "X1, X6: Path-of-craft-tree heard the break and the rock",
            40,
            move |b, _| {
                let brk = format!(
                    "break {OWNER:032x} {DIM} {} {} {} {IRON_ORE} pickaxe",
                    iron[0], iron[1], iron[2]
                );
                let place = format!("place {DIM} {} {} {} {STONE}", iron[0], iron[1], iron[2]);
                Ok(reported(b, &brk) && reported(b, &place))
            },
        )
}

fn miner_leaves_what_it_should(o: Pos, _: &Tuning) -> Scenario {
    let m = miner_at(o);
    let coal = at(m, [1, -2, 1]);
    let iron = at(m, [1, -4, 1]);
    Scenario::new("mining_miner_leaves_a_switched_off_ore", plot_area(o))
        .then("coal switched off above iron", move |b, _| {
            build_miner(b, o, true)?;
            b.set_ores_off(m, Ore::Coal.bit())?;
            ore(b, coal, "minecraft:coal_ore")?;
            ore(b, iron, IRON_ORE)
        })
        .until("the iron below is dug", LIMIT, move |b, _| {
            Ok(b.block(iron) == STONE)
        })
        .check("D5: the coal above it stays", move |b, _| {
            same("coal", b.block(coal), "minecraft:coal_ore".to_owned())
        })
}

fn miner_full(o: Pos, t: &Tuning) -> Scenario {
    let m = miner_at(o);
    let iron = at(m, [0, -2, 0]);
    let slots = t.miner_slots;
    Scenario::new("mining_miner_stops_when_full", plot_area(o))
        .then("a miner without a chest, its slots full", move |b, _| {
            build_miner(b, o, false)?;
            for i in 0..slots {
                b.put(m, i, COBBLE, 64)?;
            }
            ore(b, iron, IRON_ORE)
        })
        .until(
            "«Некуда складывать добытое»",
            LIMIT,
            move |b, _| Ok(b.mining(m) == Some(Shown::Full)),
        )
        .wait(60)
        .check("the ore stays in the ground", move |b, _| {
            same("ore", b.block(iron), IRON_ORE.to_owned())
        })
}

fn miner_works_out_its_square(o: Pos, t: &Tuning) -> Scenario {
    let m = miner_at(o);
    let side = t.miner_sides[0];
    let per_tick = t.miner_scan_per_tick;
    Scenario::new("mining_miner_works_out_its_square", plot_area(o))
        .then("a miner over nothing", move |b, memo| {
            build_miner(b, o, true)?;
            let cells = rules::cells(m, side, b.min_y()) as f64;
            memo.mark("ticks", (cells / per_tick).ceil());
            Ok(())
        })
        .until(
            "D4: «Область выработана» after the rules' scan",
            2000,
            move |b, memo| {
                let done = b.mining(m) == Some(Shown::Exhausted);
                if done {
                    memo.mark("at", memo.elapsed as f64);
                }
                Ok(done)
            },
        )
        .check("it scanned at the rules' pace", move |_, memo| {
            if memo.get("at") <= memo.get("ticks") + SLACK as f64 {
                Ok(())
            } else {
                Err(format!(
                    "{} ticks, the rules say {}",
                    memo.get("at"),
                    memo.get("ticks")
                ))
            }
        })
}

/// A placer facing east at `p`, the cell in front, a breaker facing west behind it and a chest
/// behind the breaker; a crystal under each.
fn loop_line(o: Pos) -> (Pos, Pos, Pos, Pos) {
    (
        at(o, [2, 1, 2]),
        at(o, [3, 1, 2]),
        at(o, [4, 1, 2]),
        at(o, [5, 1, 2]),
    )
}

fn placer_and_breaker(o: Pos, t: &Tuning) -> Scenario {
    let (placer, cell, breaker, chest) = loop_line(o);
    let blocks = 3;
    let pace = (t.placer_ticks + t.breaker_ticks) as u64;
    Scenario::new("mining_placer_and_breaker_turn_cobblestone", plot_area(o))
        .then(
            "a placer with cobblestone facing a breaker, a chest behind it",
            move |b, _| {
                b.place(placer, PLACER, &[("facing", "east")])?;
                b.set_owner(placer, OWNER, 1)?;
                b.place(breaker, BREAKER, &[("facing", "west")])?;
                b.place(chest, CHEST, &[])?;
                b.place(at(placer, [0, -1, 0]), CRYSTAL, &[])?;
                b.place(at(breaker, [0, -1, 0]), CRYSTAL, &[])?;
                b.put(placer, 0, COBBLE, blocks)
            },
        )
        .until(
            "the cobblestone went round into the chest",
            pace * 3 + LIMIT,
            move |b, _| Ok(count_of(&b.items(chest), COBBLE) >= blocks),
        )
        .check("X6: a place for each block, X7: no break", move |b, _| {
            let here = format!("{} {} {}", cell[0], cell[1], cell[2]);
            let lines = b.reports();
            let places = lines
                .iter()
                .filter(|r| r.ends_with(&format!("place {DIM} {here} {COBBLE}")))
                .count();
            let breaks = lines
                .iter()
                .filter(|r| r.contains(" break ") && r.contains(&here))
                .count();
            same("places, breaks", (places, breaks), (blocks as usize, 0))
        })
}

fn placer_waits_and_breaker_spares(o: Pos, _: &Tuning) -> Scenario {
    let placer = at(o, [2, 1, 2]);
    let breaker = at(o, [8, 1, 2]);
    Scenario::new(
        "mining_placer_waits_and_breaker_spares_bedrock",
        plot_area(o),
    )
    .then(
        "a placer facing stone, a breaker facing bedrock",
        move |b, _| {
            b.place(placer, PLACER, &[("facing", "east")])?;
            b.place(at(placer, [1, 0, 0]), STONE, &[])?;
            b.place(at(placer, [0, -1, 0]), CRYSTAL, &[])?;
            b.put(placer, 0, COBBLE, 4)?;
            b.place(breaker, BREAKER, &[("facing", "east")])?;
            b.place(at(breaker, [1, 0, 0]), "minecraft:bedrock", &[])?;
            b.place(at(breaker, [0, -1, 0]), CRYSTAL, &[])
        },
    )
    .until(
        "«Место занято», «Нечего ломать»",
        LIMIT,
        move |b, _| {
            Ok(b.mining(placer) == Some(Shown::Taken) && b.mining(breaker) == Some(Shown::Nothing))
        },
    )
    .wait(60)
    .check("nothing placed, the bedrock stays", move |b, _| {
        same(
            "the placer's blocks and the bedrock",
            (
                count_of(&b.items(placer), COBBLE),
                b.block(at(breaker, [1, 0, 0])),
            ),
            (4, "minecraft:bedrock".to_owned()),
        )
    })
}
