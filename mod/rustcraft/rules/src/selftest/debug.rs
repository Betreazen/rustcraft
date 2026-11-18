//! The diagnostics console command and the event journal (crate::debug): `rustcraft_debug at`
//! names a placed furnace and its status key, and the furnace's status changing (nothing to work
//! on, then raw iron and no energy) lands in the journal, which `rustcraft_debug log` reads back;
//! `rustcraft_debug storage` lists what a storage took from the net, and the journal sums the move
//! into it as one `flow` line.

use super::transport::{COBBLE, chest, fill, item_line, joint};
use super::{Bench, Build, Scenario, at, plot_area};
use crate::debug::pos_text;
use crate::geom::Pos;
use crate::net::items::Mode;
use crate::tuning::Tuning;

pub const ALL: [Build; 2] = [
    debug_reads_a_machine_and_journals_its_status,
    debug_storage_lists_its_items_and_journals_the_flow,
];

const FURNACE: &str = "rustcraft:furnace";
const RAW_IRON: &str = "minecraft:raw_iron";
const IDLE: &str = "rustcraft.machine.idle";

fn joined(b: &mut dyn Bench, args: &str) -> Result<String, String> {
    Ok(b.debug(args)?.join("\n"))
}

fn debug_reads_a_machine_and_journals_its_status(o: Pos, _: &Tuning) -> Scenario {
    let at = format!("at {}", pos_text(o));
    // The journal's lines of this furnace: `<time> <tick> <dimension> <x y z> status ...`.
    let log = format!("log 20 {} status", pos_text(o));
    let sample_at = format!("at {}", pos_text(o));
    Scenario::new("debug_reads_a_machine_and_journals_its_status", plot_area(o))
        .then("an empty electric furnace, no energy", move |b, _| b.place(o, FURNACE, &[]))
        .until("`at` names the block and its status key", 200, move |b, _| {
            let text = joined(b, &at)?;
            Ok(text.contains(FURNACE) && text.contains(IDLE))
        })
        .then("raw iron in its input: it now has work and no energy", move |b, _| {
            let slot = b
                .inputs(o)
                .first()
                .copied()
                .ok_or("the furnace has no input slot")?;
            b.put(o, slot, RAW_IRON, 1)
        })
        .until("the journal has its status change from idle", 600, move |b, _| {
            let text = joined(b, &log)?;
            Ok(text.contains(&format!("{IDLE} ->")))
        })
        .then("samples of the answers for the server log", move |b, _| {
            b.debug(&format!("sample {sample_at}"))?;
            b.debug("sample workers")?;
            b.debug("sample near nobody")?;
            b.debug("sample log 12").map(|_| ())
        })
}

fn debug_storage_lists_its_items_and_journals_the_flow(o: Pos, _: &Tuning) -> Scenario {
    let source = chest(o, 1);
    let cells = [at(o, [3, 0, 1]), at(o, [4, 0, 1]), at(o, [3, 0, 2]), at(o, [4, 0, 2])];
    let terminal = at(o, [5, 0, 1]);
    let storage = format!("storage {}", pos_text(terminal));
    Scenario::new("debug_storage_lists_its_items_and_journals_the_flow", plot_area(o))
        .then("a chest «из блока» on an item net, a storage of four cells", move |b, _| {
            item_line(b, o, 2, 4, &[1])?;
            joint(b, o, 1, Mode::From, 0)?;
            for c in cells {
                b.place(c, "rustcraft:cell", &[])?;
            }
            b.place(terminal, "rustcraft:terminal", &[])
        })
        .then("cobblestone in the chest", move |b, _| fill(b, source, COBBLE, 40))
        .until("`storage` lists the cobblestone it took", 200, move |b, _| {
            let text = joined(b, &storage)?;
            Ok(text.contains("storage#") && text.contains(&format!("{COBBLE} x40")))
        })
        .until("the journal sums the move from the chest into it", 200, move |b, _| {
            let from = format!("minecraft:chest({}) -> storage#", pos_text(source));
            let text = joined(b, &format!("log 50 {from}"))?;
            Ok(text.contains(" flow ") && text.contains(COBBLE))
        })
        .then("a sample of the answer for the server log", move |b, _| {
            b.debug(&format!("sample storage {}", pos_text(terminal))).map(|_| ())
        })
}
