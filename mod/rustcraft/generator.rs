// The fuel generator `rustcraft:gen_fuel` (guide `gen_fuel`): it burns any furnace fuel from its
// one slot and offers 16 E/s while it burns, to the machines it touches (E10) and to the energy
// nets it is a source of. It lights a fuel item only when its circuit asks for energy and burns
// as fast as the circuit takes (rustcraft_rules::machines::generator), so a coal always gives its
// 1280 E. Its window (rustcraft_panels) shows the fuel, how much of the lit item is left, what it
// gives and the balance of its whole circuit (E17): production, demand, storage and whether it is
// enough.
use crate::game::{
    Game, rustcraft_content as content,
    rustcraft_machines::{self as machines, burn_ticks, read_f64, slot_of, write_f64},
};
use horizon_client::items::ItemStack;
use rustcraft_rules::{
    geom::Pos,
    machines::{
        generator::{GenState, Generator},
        processor::Shown,
        windows::generator::{GenPanel, encode},
    },
    net::energy::GroupBalance,
    tuning::Tuning,
};

/// Saved compound keys: ticks the lit item still burns at full output, and from the start.
const BURN_KEY: &str = "burn";
const BURN_TOTAL_KEY: &str = "burn_total";

pub(super) struct Held {
    generator: Generator,
    data: Vec<i32>,
}

pub(super) fn read(game: &Game<'_>, pos: Pos) -> Option<Held> {
    let entity = game.native_mod_block_entity(pos)?;
    Some(Held {
        generator: Generator {
            fuel: slot_of(&entity, content::GEN_FUEL),
            burn_left: read_f64(&entity.nbt, BURN_KEY).max(0.0),
            burn_total: read_f64(&entity.nbt, BURN_TOTAL_KEY).max(0.0),
        },
        data: entity.data.clone(),
    })
}

/// The E a second it offers its circuit this tick.
pub(super) fn offer(game: &Game<'_>, held: &Held, tuning: &Tuning) -> f64 {
    held.generator
        .offer(tuning.generator_output, &|item| burn_ticks(game, item))
}

/// The tick after its circuit took `used` E a second from it: burns, lights the next item when
/// needed and writes back the fuel slot, the burn and the window. What it shows.
pub(super) fn run(
    game: &mut Game<'_>,
    pos: Pos,
    held: Held,
    used: f64,
    balance: GroupBalance,
    tuning: &Tuning,
) -> Option<Shown> {
    let Held {
        mut generator,
        data: shown_data,
    } = held;
    let before = generator.clone();
    let output = tuning.generator_output;
    let (state, lit) = {
        let game = &*game;
        generator.burn(used, output, &|item| burn_ticks(game, item))
    };
    let shown = match state {
        GenState::Burning => Shown::Working,
        GenState::NoFuel => Shown::NoFuel,
        GenState::NoDemand => Shown::NoDemand,
    };
    let data = encode(&GenPanel {
        flame: generator.flame(),
        gives: used,
        output,
        produced: balance.produced,
        requested: balance.requested,
        supplied: balance.supplied,
        shown: Some(shown),
    });
    if generator == before && data == shown_data {
        return Some(shown);
    }
    // A lava bucket burnt to the end leaves its bucket in the slot.
    let remainder: Vec<ItemStack> = lit
        .filter(|_| generator.fuel.is_none())
        .and_then(|item| machines::fuel_remainder(game, &item))
        .into_iter()
        .collect();
    game.native_with_mod_block_entity(pos, move |entity| {
        machines::write_slots(
            entity,
            [content::GEN_FUEL],
            &[before.fuel.clone()],
            &[generator.fuel.clone()],
            &[],
        );
        if let Some(rest) = remainder.into_iter().next()
            && let Some(slot @ None) = entity.items.get_mut(content::GEN_FUEL)
        {
            *slot = Some(rest);
        }
        if generator.burn_left != before.burn_left {
            write_f64(&mut entity.nbt, BURN_KEY, generator.burn_left);
        }
        if generator.burn_total != before.burn_total {
            write_f64(&mut entity.nbt, BURN_TOTAL_KEY, generator.burn_total);
        }
        if entity.data != data {
            entity.data = data;
        }
        shown
    })
}
