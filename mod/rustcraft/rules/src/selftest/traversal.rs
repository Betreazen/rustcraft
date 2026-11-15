//! The zipline's ropes and the harpoon on mobs (12.5 Z2, G5). A ride needs a real player and is
//! the bots' to check.
//!
//! - A rope tied between two anchors puts both anchor blocks in the world and saves the rope.
//! - An anchor gone (blown up, pushed, cleared) unties its rope and the other anchor goes; a
//!   player's break goes the same way (rustcraft_zipline::on_anchor_broken), the broken anchor
//!   dropping by its loot table.
//! - A player's fifth rope takes the place of their oldest, whose anchors go.
//! - The harpoon pulls a small mob on its line to the player.

use super::{Bench, Build, Scenario, at, plot_area};
use crate::geom::Pos;
use crate::traversal::{ANCHOR, MAX_ROPES, owner_key};
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [rope_is_tied_and_saved, lost_anchor_unties_the_rope, fifth_rope_unties_the_oldest, harpoon_pulls_a_mob];

const STONE: &str = "minecraft:stone";
const AIR: &str = "minecraft:air";
/// One owner a scenario: the plots run at once, and a player's limit counts all their ropes.
const TIES: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a16;
const LOSES: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a17;
const KEEPS: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_0a18;

/// A stone floor at the origin's level, x 0..=12 and z -2..=8, and a stone pillar three high at
/// x 10 for the far anchors.
fn ground(b: &mut dyn Bench, o: Pos) -> Result<(), String> {
    for x in 0..=12 {
        for z in -2..=8 {
            b.place(at(o, [x, 0, z]), STONE, &[])?;
        }
    }
    for z in -2..=8 {
        for y in 1..=2 {
            b.place(at(o, [10, y, z]), STONE, &[])?;
        }
    }
    Ok(())
}

/// The start and far anchor of row `z`.
fn ends(o: Pos, z: i32) -> (Pos, Pos) {
    (at(o, [1, 1, z]), at(o, [10, 3, z]))
}

fn saved(b: &dyn Bench, owner: u128, a: Pos, far: Pos) -> bool {
    b.ropes().iter().any(|r| r.a == a && r.b == far && r.owner == owner_key(owner))
}

fn rope_is_tied_and_saved(o: Pos, _: &Tuning) -> Scenario {
    let (a, far) = ends(o, 0);
    Scenario::new("zipline_ties_a_saved_rope", plot_area(o))
        .then("a floor and a pillar, a rope tied from the floor to the pillar's top", move |b, _| {
            ground(b, o)?;
            b.tie_rope(TIES, a, far).map(|_| ())
        })
        .wait(20)
        .check("both anchors stand and the rope is saved", move |b, _| {
            for p in [a, far] {
                if b.block(p) != ANCHOR {
                    return Err(format!("{} at {p:?}, not an anchor", b.block(p)));
                }
            }
            if !saved(b, TIES, a, far) {
                return Err(format!("no saved rope from {a:?} to {far:?}: {:?}", b.ropes()));
            }
            Ok(())
        })
}

fn lost_anchor_unties_the_rope(o: Pos, _: &Tuning) -> Scenario {
    let (a, far) = ends(o, 0);
    Scenario::new("zipline_lost_anchor_unties_the_rope", plot_area(o))
        .then("a rope tied from the floor to the pillar's top", move |b, _| {
            ground(b, o)?;
            b.tie_rope(LOSES, a, far).map(|_| ())
        })
        .then("the far anchor is gone", move |b, _| b.place(far, AIR, &[]))
        .until("the rope is untied and the start anchor gone", 100, move |b, _| {
            Ok(!saved(b, LOSES, a, far) && b.block(a) == AIR)
        })
}

fn fifth_rope_unties_the_oldest(o: Pos, _: &Tuning) -> Scenario {
    let rows: Vec<i32> = (0..=MAX_ROPES as i32).map(|i| i * 2 - 2).collect();
    let tied = rows.clone();
    Scenario::new("zipline_fifth_rope_unties_the_oldest", plot_area(o))
        .then("five ropes of one player, one a row", move |b, _| {
            ground(b, o)?;
            for z in &tied {
                let (a, far) = ends(o, *z);
                b.tie_rope(KEEPS, a, far)?;
            }
            Ok(())
        })
        .wait(20)
        .check("the first rope and its anchors are gone, the other four stay", move |b, _| {
            let (a0, far0) = ends(o, rows[0]);
            if saved(b, KEEPS, a0, far0) || b.block(a0) == ANCHOR || b.block(far0) == ANCHOR {
                return Err("the oldest rope is still there".into());
            }
            for z in &rows[1..] {
                let (a, far) = ends(o, *z);
                if !saved(b, KEEPS, a, far) || b.block(a) != ANCHOR {
                    return Err(format!("the rope of row {z} is gone"));
                }
            }
            Ok(())
        })
}

fn harpoon_pulls_a_mob(o: Pos, _: &Tuning) -> Scenario {
    let me = at(o, [1, 1, 3]).map(f64::from);
    let me = [me[0] + 0.5, me[1], me[2] + 0.5];
    Scenario::new("harpoon_pulls_a_mob", plot_area(o))
        .then("a floor, a pig 8 blocks from a player's place", move |b, m| {
            ground(b, o)?;
            let id = b.spawn_mob("minecraft:pig", [me[0] + 8.0, me[1], me[2]], None)?;
            m.mark("pig", f64::from(id));
            Ok(())
        })
        .then("the harpoon hooks it", move |b, m| b.harpoon_mob(me, m.get("pig") as u32))
        .until("the pig is pulled in front of the player", 60, move |b, m| {
            let pig = b.mob(m.get("pig") as u32).ok_or("the pig is gone")?;
            let d = (pig.pos[0] - me[0]).hypot(pig.pos[2] - me[2]);
            Ok(d < 2.5)
        })
}
