// The alloy smelter `rustcraft:alloy` (guide `alloy`, spec 4.3, AL1): four inputs, a fuel slot
// and three outputs. It runs the content mod's `rustcraft:alloying` recipes on any furnace fuel,
// or faster on energy, burning no fuel then (rustcraft_rules::machines::alloy::AlloySmelter). An
// alloy in its output that a recipe needs, with the rest of that recipe in its inputs, it takes
// back itself (sparksteel straight on to crystallite) unless the «брать свой сплав» switch of its
// window is off. The switch is a menu button (rustcraft_panels) kept in its saved compound.
//
// A recipe file: {"type": "rustcraft:alloying", "ingredients": [{"item": "rustcraft:coppersteel",
// "count": 2}, {"item": "minecraft:gold_ingot"}, ...], "result": {"id": "rustcraft:sparksteel",
// "count": 2}}, at most four ingredients, each one item.
use crate::game::{
    Game, rustcraft_content as content,
    rustcraft_machines::{
        self as machines, MachineRecipes, burn_ticks, read_f64, read_int, slot_of, write_f64,
        write_int,
    },
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use rustcraft_rules::{
    geom::Pos,
    machines::{
        alloy::{AlloyRecipe, AlloySmelter, AlloySpec, INPUTS},
        processor::{Shown, State},
        slots::Slot,
        windows::alloy::{self as window, AlloyPanel},
    },
    tuning::Tuning,
};

/// Saved compound keys.
const PROGRESS_KEY: &str = "progress";
const BURN_KEY: &str = "burn";
const BURN_TOTAL_KEY: &str = "burn_total";
const TAKE_OWN_KEY: &str = "take_own";
/// Alloys stack to 64.
const SLOT_CAPACITY: u32 = 64;

/// One alloying recipe file; none when an ingredient or the result is not one item the game knows.
pub(super) fn parse_recipe(game: &Game<'_>, json: &serde_json::Value) -> Option<AlloyRecipe> {
    let list = json["ingredients"].as_array()?;
    if list.is_empty() || list.len() > INPUTS {
        return None;
    }
    let mut inputs = Vec::new();
    for entry in list {
        let items = game.native_recipe_ingredient(&entry["item"])?.items;
        let mut items = items.into_iter();
        let (Some(item), None) = (items.next(), items.next()) else {
            return None;
        };
        let count = entry["count"].as_u64().unwrap_or(1).clamp(1, 64) as u32;
        inputs.push((item, count));
    }
    let result = game.native_recipe_result(&json["result"])?;
    Some(AlloyRecipe {
        inputs,
        output: (result.item.to_string(), result.count.max(1)),
    })
}

fn spec(tuning: &Tuning) -> AlloySpec {
    AlloySpec {
        ticks_on_fuel: tuning.alloy_ticks_on_fuel,
        ticks_on_energy: tuning.alloy_ticks_on_energy,
        energy: tuning.alloy_energy,
        slot_capacity: SLOT_CAPACITY,
    }
}

pub(super) struct Held {
    smelter: AlloySmelter,
    burn_total: f64,
    /// E a second it asks for this tick.
    pub request: f64,
    data: Vec<i32>,
}

fn smelter_of(entity: &ModBlockEntity) -> AlloySmelter {
    AlloySmelter {
        inputs: std::array::from_fn(|k| slot_of(entity, content::ALLOY_INPUTS.start + k)),
        outputs: std::array::from_fn(|k| slot_of(entity, content::ALLOY_OUTPUTS.start + k)),
        fuel: slot_of(entity, content::ALLOY_FUEL),
        burn_left: read_f64(&entity.nbt, BURN_KEY).max(0.0),
        progress: read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0),
        take_own: read_int(&entity.nbt, TAKE_OWN_KEY) != Some(0),
    }
}

pub(super) fn read(
    game: &Game<'_>,
    pos: Pos,
    recipes: &MachineRecipes,
    tuning: &Tuning,
) -> Option<Held> {
    let entity = game.native_mod_block_entity(pos)?;
    let smelter = smelter_of(&entity);
    let request = smelter.request(&recipes.alloying, &spec(tuning));
    Some(Held {
        smelter,
        burn_total: read_f64(&entity.nbt, BURN_TOTAL_KEY).max(0.0),
        request,
        data: entity.data.clone(),
    })
}

/// One tick on `share` of its request; writes back what changed. What it shows.
pub(super) fn run(
    game: &mut Game<'_>,
    pos: Pos,
    held: Held,
    share: f64,
    recipes: &MachineRecipes,
    tuning: &Tuning,
) -> Option<Shown> {
    let Held {
        mut smelter,
        mut burn_total,
        request,
        data: shown_data,
    } = held;
    let spec = spec(tuning);
    let before = smelter.clone();
    let state = {
        let game = &*game;
        smelter.tick(&recipes.alloying, &spec, share, &|item| {
            burn_ticks(game, item)
        })
    };
    let lit_item = lit(&before.fuel, &smelter.fuel);
    if let Some(item) = &lit_item {
        burn_total = burn_ticks(game, item).unwrap_or(0.0);
    }
    let on_energy = share > 0.0;
    let shown = match state {
        State::Working if on_energy => Shown::of(state, share),
        State::Working => Shown::OnFuel,
        State::NoEnergy => Shown::NoFuel,
        State::OutputFull => Shown::OutputFull,
        State::Idle => Shown::Idle,
    };
    let flame = if burn_total > 0.0 {
        smelter.burn_left / burn_total
    } else {
        0.0
    };
    let data = window::encode(&AlloyPanel {
        progress: smelter.progress,
        flame,
        got: if on_energy && state == State::Working {
            share * request
        } else {
            0.0
        },
        asked: spec.energy,
        take_own: smelter.take_own,
        shown: Some(shown),
    });
    if smelter == before && data == shown_data {
        return Some(shown);
    }
    let remainder: Vec<ItemStack> = lit_item
        .filter(|_| smelter.fuel.is_none())
        .and_then(|item| machines::fuel_remainder(game, &item))
        .into_iter()
        .collect();
    game.native_with_mod_block_entity(pos, move |entity| {
        let all = |s: &AlloySmelter| -> Vec<Slot> {
            s.inputs
                .iter()
                .cloned()
                .chain([s.fuel.clone()])
                .chain(s.outputs.iter().cloned())
                .collect()
        };
        machines::write_slots(
            entity,
            content::ALLOY_INPUTS
                .chain([content::ALLOY_FUEL])
                .chain(content::ALLOY_OUTPUTS),
            &all(&before),
            &all(&smelter),
            &[],
        );
        if let Some(rest) = remainder.into_iter().next()
            && let Some(slot @ None) = entity.items.get_mut(content::ALLOY_FUEL)
        {
            *slot = Some(rest);
        }
        if smelter.progress != before.progress {
            write_f64(&mut entity.nbt, PROGRESS_KEY, smelter.progress);
        }
        if smelter.burn_left != before.burn_left {
            write_f64(&mut entity.nbt, BURN_KEY, smelter.burn_left);
            write_f64(&mut entity.nbt, BURN_TOTAL_KEY, burn_total);
        }
        if entity.data != data {
            entity.data = data;
        }
        shown
    })
}

/// The fuel item lit this tick: the fuel slot lost one.
fn lit(before: &Slot, after: &Slot) -> Option<String> {
    let (item, n) = before.as_ref()?;
    let left = after.as_ref().map_or(0, |(_, m)| *m);
    (left < *n).then(|| item.clone())
}

/// The «брать свой сплав» switch (AL1): on or off, shown with the next data slots.
pub(super) fn on_menu_button(entity: &mut ModBlockEntity, id: i32) {
    let on = match id {
        window::TAKE_OWN_ON => true,
        window::TAKE_OWN_OFF => false,
        _ => return,
    };
    write_int(&mut entity.nbt, TAKE_OWN_KEY, i32::from(on));
    let mut shown = window::decode(&entity.data);
    shown.take_own = on;
    entity.data = window::encode(&shown);
}
