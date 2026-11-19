//! 0.21 (spec 45; the owner: «Вырежи из игры вообще каналы … если уже что-то скрафчено, то пускай
//! разберутся на блоки, из которых делаются»): the channel items of 0.19 are gone.
//!
//! - A storage on a line, a chest beside it with old channels of three kinds and tiers and some
//!   cobblestone. The channels the storage takes in become their alloy within a second (CR2): none
//!   is left, and each alloy is at least half its channels, rounded up, and no more than them.
//! - The handbook's alloys page shows its alloys' recipes and none that takes channels (CR5).

use super::slice15::{FILL, build, held, holds};
use super::{Build, Scenario, plot_area};
use crate::geom::Pos;
use crate::net::old_channels::{OLD_CHANNELS, ingots, is_old_channel};
use crate::tuning::Tuning;

pub const ALL: [Build; 2] = [a_storage_turns_old_channels_into_alloy, the_handbook_shows_no_channels];

const COBBLE: (&str, u32) = ("minecraft:cobblestone", 4);
/// The old channels the chest holds and the alloy each becomes.
const OLD: [(&str, u32, &str); 3] = [
    ("rustcraft:item_channel", 6, "rustcraft:coppersteel"),
    ("rustcraft:fluid_channel_3", 3, "rustcraft:crystallite"),
    ("rustcraft:energy_channel_4", 1, "rustcraft:starsteel"),
];
/// The conversion's pass (20 ticks) after the last channel came in, and some.
const CONVERT: u64 = 60;

fn a_storage_turns_old_channels_into_alloy(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("channels21_a_storage_turns_old_channels_into_alloy", plot_area(o))
        .then("a storage on a line, a chest of old channels and cobblestone beside it", move |b, _| {
            let mut parts: Vec<(&str, u32)> = OLD.iter().map(|(c, n, _)| (*c, *n)).collect();
            parts.push(COBBLE);
            build(b, o, &parts)
        })
        .until("the storage holds the cobblestone", FILL, move |b, _| holds(b, o, &[COBBLE]))
        .until("no old channel is left in the storage", FILL + CONVERT, move |b, _| {
            let v = b.storage(super::slice15::terminal_at(o)).ok_or("the terminal shows no storage")?;
            Ok(v.store.items.keys().all(|k| !is_old_channel(k))
                && OLD.iter().all(|(_, n, alloy)| v.store.count(alloy) >= u64::from(ingots(*n))))
        })
        .check("each alloy is half its channels, rounded up, at most their count", move |b, _| {
            for (channel, n, alloy) in OLD {
                let got = held(b, o, alloy)?;
                if got < u64::from(ingots(n)) || got > u64::from(n) {
                    return Err(format!("{n} × {channel} gave {got} × {alloy}"));
                }
            }
            if !b.items(super::transport::chest(o, 1)).iter().flatten().all(|(i, _)| !is_old_channel(i)) {
                return Err("channels left in the chest".into());
            }
            Ok(())
        })
}

fn the_handbook_shows_no_channels(o: Pos, _: &Tuning) -> Scenario {
    Scenario::new("channels21_the_handbook_shows_no_channels", plot_area(o)).check(
        "the alloys page shows recipes, none of them taking old channels",
        |b, _| {
            let page = crate::handbook::PAGES
                .iter()
                .position(|p| p.id == "alloys")
                .ok_or("no alloys page")?;
            let shown = b.handbook_recipes(page);
            if shown.is_empty() {
                return Err("the alloys page shows no recipe".into());
            }
            if let Some(s) = shown.iter().find(|s| s.inputs().into_iter().any(is_old_channel)) {
                return Err(format!("the alloys page shows {s:?}"));
            }
            for (id, _, _) in OLD_CHANNELS {
                if !b.item_known(id) {
                    return Err(format!("{id} is no longer registered: saved stacks would be lost"));
                }
            }
            Ok(())
        },
    )
}
