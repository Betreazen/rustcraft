//! Slice 18, last leftovers: the backpack's store with the stack module (guide `backpack`,
//! `mod_stack`). Opening the window, wearing and clicking need a player and are the bots' checks;
//! the store is the server's own and is checked here with the game's own stacks and NBT: 600
//! cobblestone onto a slot of 1000 with the crystallite stack module (1024 and 576), the module
//! kept in while a slot holds more than a plain stack, the store saved and read back with its
//! counts past 99, its module and a worn tool, a slice-10 file (the count inside the stack) read,
//! and the window's message bringing the tool's wear.

use super::{Build, Scenario, plot_area};
use crate::geom::Pos;
use crate::tuning::Tuning;

pub const ALL: [Build; 1] = [backpack_store_holds_past_99];

fn backpack_store_holds_past_99(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("s18_backpack_store_holds_past_99", plot_area(o))
        .check("the backpack's store keeps 1024 a slot with the stack module and reads back; the key opens a worn one", |b, _| b.backpack_store())
}
