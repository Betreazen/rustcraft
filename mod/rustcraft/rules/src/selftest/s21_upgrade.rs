//! 0.21 (owner 2026-11-19): the node window counts the line's housings by tier, and its button
//! replaces every housing of the line's lowest tier with the next tier's in one press.
//!
//! - A tier-1 node, six housings of tier 1 and two of tier 2 on its line: the window counts
//!   eight, six of tier 1 and two of tier 2, and asks for six housings of tier 2. A press makes
//!   the six of tier 2 (the two already of tier 2 untouched), and the window then asks for eight
//!   of tier 3; a second press makes all eight tier 3.

use super::magistral::{line, place_housing};
use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::net::node_window::{self as nw, NodeView};
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [line_upgrades_in_one_press];

const LIMIT: u64 = 200;

fn view(b: &mut dyn Bench, node: Pos) -> Result<NodeView, String> {
    let json = b.pipe("node_view:0", node)?;
    nw::decode(json.as_bytes()).ok_or_else(|| "the view does not read".to_owned())
}

fn line_upgrades_in_one_press(o: Pos, _t: &Tuning) -> Scenario {
    Scenario::new("upgrade21_line_in_one_press", plot_area(o))
        .then("a tier-1 node, six housings of tier 1 and two of tier 2", move |b, _| {
            line(b, o, 1, 6, 1)?;
            place_housing(b, at(o, [7, 0, 0]), 2)?;
            place_housing(b, at(o, [8, 0, 0]), 2)
        })
        .until("the window counts eight, six of tier 1", LIMIT, move |b, _| {
            Ok(view(b, o)?.tiers == [6, 2, 0, 0])
        })
        .check("it asks for six housings of tier 2", move |b, _| {
            let v = view(b, o)?;
            same("housings", v.housings, 8)?;
            let up = v.upgrade.ok_or("no upgrade offered")?;
            same("the upgrade", (up.from, up.to, up.need), (1, 2, 6))
        })
        .then("the button pressed", move |b, _| {
            same("what it did", b.pipe("upgrade_line:0", o)?, "1:2:6".to_owned())
        })
        .until("all eight are of tier 2", LIMIT, move |b, _| {
            Ok(view(b, o)?.tiers == [0, 8, 0, 0])
        })
        .check("it now asks for eight of tier 3", move |b, _| {
            let up = view(b, o)?.upgrade.ok_or("no upgrade offered")?;
            same("the upgrade", (up.from, up.to, up.need), (2, 3, 8))
        })
        .then("the button pressed again", move |b, _| {
            same("what it did", b.pipe("upgrade_line:0", o)?, "2:3:8".to_owned())
        })
        .until("all eight are of tier 3", LIMIT, move |b, _| {
            Ok(view(b, o)?.tiers == [0, 0, 8, 0])
        })
}
