//! Round 32 (spec 33): the terminal's «Сеть» page.
//!
//! - A storage on a line of six housings (five of tier 2, one of tier 1) with a water tank, an
//!   electric furnace, a crusher and a chest on it: the page the server builds for its terminal
//!   counts the six housings at the weakest tier 1, lists both machines and the chest, and lists
//!   the water tank with its fluid and amount, its joint counted as the storage's stock. A lava
//!   tank on a second line of its own, not joined to the first, is not on the page.

use super::magistral::{line, place_housing};
use super::{Bench, Build, Scenario, at, near, plot_area, same};
use crate::geom::Pos;
use crate::net::fluids::BUCKET;
use crate::storage::network::{JointUse, NetPage};
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [network_page_lists_the_line];

const CELL: &str = "rustcraft:cell";
const TERMINAL: &str = "rustcraft:terminal";
const TANK: &str = "rustcraft:tank";
const FURNACE: &str = "rustcraft:furnace";
const CRUSHER: &str = "rustcraft:crusher";
const CHEST: &str = "minecraft:chest";
const WATER: &str = "minecraft:water";
const LAVA: &str = "minecraft:lava";
const LIMIT: u64 = 400;

fn page(b: &dyn Bench, terminal: Pos) -> Result<NetPage, String> {
    b.net_page(terminal)
        .ok_or_else(|| format!("no network page for {terminal:?}"))
}

fn network_page_lists_the_line(o: Pos, t: &Tuning) -> Scenario {
    let cells = [at(o, [2, 0, 1]), at(o, [3, 0, 1])];
    let terminal = at(o, [4, 0, 1]);
    let water = at(o, [1, 0, -1]);
    let furnace = at(o, [3, 0, -1]);
    let crusher = at(o, [4, 0, -1]);
    let chest = at(o, [5, 0, -1]);
    // The second line, seven blocks apart: its own node, three housings, a lava tank.
    let apart = at(o, [0, 0, 7]);
    let lava = at(o, [2, 0, 8]);
    let items = t.items_of(1);
    Scenario::new("terminal21_network_page_lists_the_line", plot_area(o))
        .then(
            "a storage, a water tank, a furnace, a crusher and a chest on one line; a lava tank on another",
            move |b, _| {
                line(b, o, 1, 5, 2)?;
                place_housing(b, at(o, [6, 0, 0]), 1)?;
                for c in cells {
                    b.place(c, CELL, &[])?;
                }
                b.place(terminal, TERMINAL, &[])?;
                b.place(water, TANK, &[])?;
                b.set_tank(water, WATER, 5 * BUCKET)?;
                b.place(furnace, FURNACE, &[])?;
                b.place(crusher, CRUSHER, &[])?;
                b.place(chest, CHEST, &[])?;
                line(b, apart, 1, 3, 1)?;
                b.place(lava, TANK, &[])?;
                b.set_tank(lava, LAVA, 3 * BUCKET)
            },
        )
        .until("the page lists the tank and both machines", LIMIT, move |b, _| {
            let Some(p) = b.net_page(terminal) else {
                return Ok(false);
            };
            Ok(p.tank_count >= 1 && p.machines.len() >= 2 && p.storage.is_some())
        })
        .check("the magistral: six housings, the weakest of tier 1", move |b, _| {
            let p = page(b, terminal)?;
            same("branches", p.magistral.branches, 1)?;
            same("housings", p.magistral.housings, 6)?;
            same("weakest tier", p.magistral.min_tier, 1)?;
            same("housings out of the node's range", p.magistral.out_of_reach, 0)?;
            near("items a second", f64::from(p.magistral.items), items, 0.01)?;
            same("cells", p.storage.map(|s| s.cells), Some(2))
        })
        .check("the furnace, the crusher and the chest on its joints", move |b, _| {
            let p = page(b, terminal)?;
            let count = |block: &str| {
                p.machines
                    .iter()
                    .find(|g| g.block == block)
                    .map(|g| g.count)
            };
            same("furnaces", count(FURNACE), Some(1))?;
            same("crushers", count(CRUSHER), Some(1))?;
            same("machine kinds", p.machines.len(), 2)?;
            same("chests", p.containers, 1)?;
            same("other storages", p.storages, 0)
        })
        .check("the water tank with its water, and not the lava tank", move |b, _| {
            let p = page(b, terminal)?;
            same("tanks", p.tank_count, 1)?;
            let tank = p.tanks.first().ok_or("no tank listed")?;
            same("the tank's place", tank.pos, water)?;
            same("its fluid", tank.fluid.as_deref(), Some(WATER))?;
            let held = b.tank(water).map(|g| g.amount).ok_or("the water tank is gone")?;
            same("its amount, as the tank holds it", tank.amount, held)?;
            same("some water in it", tank.amount > 0, true)?;
            same("its capacity", tank.capacity, b.tuning().tank_buckets * BUCKET)?;
            same("its joint", tank.joint, JointUse::Both)?;
            same("the storage counts it", tank.joint.counted(), true)?;
            same("the lava tank listed", p.tanks.iter().any(|x| x.pos == lava), false)
        })
}
