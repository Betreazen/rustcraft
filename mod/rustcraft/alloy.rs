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
    Game,
    rustcraft_content::{self as content, Machine},
    rustcraft_machines::{
        self as machines, Fitted, MachineRecipes, QUEUE_KEY, RESERVE_KEY, burn_ticks, max_stack,
        read_f64, read_int, roll, slot_of, write_f64, write_int,
    },
    rustcraft_utility::{read_json, write_json},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use rustcraft_rules::machines::catalyst::Tank as CatalystTank;
use rustcraft_rules::{
    geom::Pos,
    machines::{
        alloy::{AlloyRecipe, AlloySmelter, AlloySpec, INPUTS, OUTPUTS},
        queue,
        processor::{Shown, State},
        slots::Slot,
        windows::alloy::{self as window, AlloyPanel},
    },
    modules,
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

/// The smelter's numbers on its tier and modules (A8, A7): a higher tier and speed modules work
/// faster, economy modules cost less; a stack module makes its cells 256.
fn spec(tuning: &Tuning, fitted: &Fitted, catalyst: &CatalystTank) -> AlloySpec {
    let tier = fitted.speed(Machine::Alloy, tuning);
    let (work, energy) = modules::factors(&fitted.modules, tuning);
    // Slice 18 (cross, guide `fluids`): its catalyst's speed and energy too.
    let (cat_work, cat_energy) = machines::catalyst_factors(content::ALLOY, catalyst, tuning);
    let (work, energy) = (work * cat_work, energy * cat_energy);
    // 0.20: its owner's tree shortens the work, not the draw (rustcraft_perks).
    let work = work * fitted.perk;
    AlloySpec {
        ticks_on_fuel: (tuning.alloy_ticks_on_fuel / (tier * work)).max(1.0),
        ticks_on_energy: (tuning.alloy_ticks_on_energy / (tier * work)).max(1.0),
        energy: tuning.alloy_energy * tier * energy,
        slot_capacity: modules::machine_stack(&fitted.modules, SLOT_CAPACITY),
    }
}

pub(super) struct Held {
    smelter: AlloySmelter,
    /// Its slots as the block entity holds them (inputs, fuel, outputs), before the queue took
    /// any: what the write compares with.
    raw: Vec<Slot>,
    queue_before: Vec<Slot>,
    reserve_before: Vec<Slot>,
    open_cells: usize,
    fitted: Fitted,
    burn_total: f64,
    /// E a second it asks for this tick.
    pub request: f64,
    data: Vec<i32>,
    /// Slice 18 (cross): its catalyst tank.
    catalyst: CatalystTank,
}

fn smelter_of(entity: &ModBlockEntity) -> AlloySmelter {
    AlloySmelter {
        inputs: std::array::from_fn(|k| slot_of(entity, content::ALLOY_INPUTS.start + k)),
        outputs: std::array::from_fn(|k| slot_of(entity, content::ALLOY_OUTPUTS.start + k)),
        fuel: slot_of(entity, content::ALLOY_FUEL),
        burn_left: read_f64(&entity.nbt, BURN_KEY).max(0.0),
        progress: read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0),
        take_own: read_int(&entity.nbt, TAKE_OWN_KEY) != Some(0),
        queue: Vec::new(),
        lucky: false,
    }
}

/// The inputs, the fuel and the outputs in their slots' order.
fn all(s: &AlloySmelter) -> Vec<Slot> {
    s.inputs
        .iter()
        .cloned()
        .chain([s.fuel.clone()])
        .chain(s.outputs.iter().cloned())
        .collect()
}

/// Slice 15: whether the smelter at `pos` can run on fuel: a fuel item in its fuel slot or a fuel
/// still burning (rustcraft_orders gives a step to a machine with energy or fuel first).
pub(super) fn has_fuel(game: &Game<'_>, pos: Pos) -> bool {
    crate::game::rustcraft_machines::entity(game, pos).is_some_and(|entity| {
        slot_of(&entity, content::ALLOY_FUEL).is_some() || read_f64(&entity.nbt, BURN_KEY) > 0.0
    })
}

/// The smelter at `pos`. Slice 18: its inputs move into its queue cells, and each output counts
/// with its reserve.
pub(super) fn read(
    game: &Game<'_>,
    pos: Pos,
    recipes: &MachineRecipes,
    tuning: &Tuning,
) -> Option<Held> {
    let entity = crate::game::rustcraft_machines::entity(game, pos)?;
    let fitted = machines::fitted(game, pos, entity);
    let mut smelter = smelter_of(entity);
    let raw = all(&smelter);
    let queue_before: Vec<Slot> = read_json(&entity.nbt, QUEUE_KEY);
    let mut reserve_before: Vec<Slot> = read_json(&entity.nbt, RESERVE_KEY);
    reserve_before.resize(OUTPUTS, None);
    let open_cells = fitted.open_cells();
    smelter.queue = queue_before.clone();
    for input in smelter.inputs.iter_mut() {
        if let Some((item, _)) = input.clone() {
            let size = modules::machine_stack(&fitted.modules, max_stack(game, &item));
            queue::intake(input, &mut smelter.queue, open_cells, size);
        }
    }
    let mut spare: Slot = None;
    queue::spill(&mut spare, &mut smelter.queue, open_cells, |i| max_stack(game, i));
    if let Some(back) = spare {
        // A closed cell's items go back into an input that has room for them.
        if let Some(free) = smelter.inputs.iter_mut().find(|s| s.is_none()) {
            *free = Some(back);
        } else {
            smelter.queue.push(Some(back));
        }
    }
    for (k, out) in smelter.outputs.iter_mut().enumerate() {
        *out = queue::join(out, &reserve_before[k]);
    }
    let catalyst = machines::read_catalyst(&entity.nbt);
    let request = smelter.request(&recipes.alloying, &spec(tuning, &fitted, &catalyst));
    Some(Held {
        smelter,
        raw,
        queue_before,
        reserve_before,
        open_cells,
        fitted,
        burn_total: read_f64(&entity.nbt, BURN_TOTAL_KEY).max(0.0),
        request,
        data: entity.data.clone(),
        catalyst,
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
        raw,
        queue_before,
        reserve_before,
        open_cells,
        fitted,
        mut burn_total,
        request,
        data: shown_data,
        mut catalyst,
    } = held;
    let spec = spec(tuning, &fitted, &catalyst);
    let before = smelter.clone();
    let catalyst_before = catalyst.clone();
    // Slice 18 (cross): the enricher's chance of one more product joins the yield module's.
    smelter.lucky = modules::yield_lucky(&fitted.modules, roll())
        || machines::catalyst_lucky(content::ALLOY, &catalyst, roll(), tuning);
    let state = {
        let game = &*game;
        smelter.tick(&recipes.alloying, &spec, share, &|item| {
            burn_ticks(game, item)
        })
    };
    smelter.lucky = false;
    if state == State::Working {
        // On fuel it works at full pace without energy.
        let pace = if share > 0.0 { share } else { 1.0 };
        catalyst.spend(1.0 / tuning.ticks_per_second, pace, tuning);
    }
    // Slice 18 (cross): what it gave goes to Path-of-craft-tree (`smelt`); PCT gives nothing for
    // alloys today but takes the report (docs/design/path-of-craft-tree-contract.md).
    fn outs(o: &[Option<(String, u32)>]) -> Vec<(&str, u32)> {
        o.iter().flatten().map(|(i, n)| (i.as_str(), *n)).collect()
    }
    let gained = rustcraft_rules::reports::gained(outs(&before.outputs), outs(&smelter.outputs));
    if !gained.is_empty()
        && let Some(owner) = crate::game::rustcraft_base::owner_at(game, pos)
    {
        for (item, count) in gained {
            crate::game::rustcraft_base::report(rustcraft_rules::reports::Report::Smelt { owner, item, count });
        }
    }
    let lit_item = lit(&before.fuel, &smelter.fuel);
    if let Some(item) = &lit_item {
        burn_total = burn_ticks(game, item).unwrap_or(0.0);
    }
    // A burnt-out lava bucket's empty bucket goes to the outputs, where the magistral takes it,
    // so new fuel can come in; with no room it stays in the fuel slot, as in a furnace.
    let remainder = lit_item
        .filter(|_| smelter.fuel.is_none())
        .and_then(|item| machines::fuel_remainder(game, &item));
    let remainder = remainder.filter(|rest| !smelter.put_remainder(&rest.item));
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
    // Each output back into its slot and its reserve.
    let mut reserve = vec![None; OUTPUTS];
    let mut shown_slots = smelter.clone();
    for (k, out) in shown_slots.outputs.iter_mut().enumerate() {
        let max = out.as_ref().map_or(SLOT_CAPACITY, |(i, _)| max_stack(game, i));
        let (slot, rest) = queue::split(out, max);
        *out = slot;
        reserve[k] = rest;
    }
    let mut data = window::encode(&AlloyPanel {
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
    let reserved: Slot = reserve
        .iter()
        .flatten()
        .next()
        .map(|(i, _)| (i.clone(), reserve.iter().flatten().map(|(_, n)| n).sum()));
    data.extend(fitted.extras(game, &smelter.queue, open_cells, &reserved));
    let after = all(&shown_slots);
    let hidden_moved = smelter.queue != queue_before || reserve != reserve_before;
    if after == raw
        && !hidden_moved
        && smelter.progress == before.progress
        && smelter.burn_left == before.burn_left
        && data == shown_data
        && remainder.is_none()
        && fitted.tier_saved()
        && catalyst == catalyst_before
    {
        return Some(shown);
    }
    let queue_now = smelter.queue.clone();
    game.native_with_mod_block_entity(pos, move |entity| {
        machines::write_slots(
            entity,
            content::ALLOY_INPUTS
                .chain([content::ALLOY_FUEL])
                .chain(content::ALLOY_OUTPUTS),
            &raw,
            &after,
            &[],
        );
        if let Some(rest) = remainder
            && let Some(slot @ None) = entity.items.get_mut(content::ALLOY_FUEL)
        {
            *slot = Some(rest);
        }
        if hidden_moved {
            write_json(&mut entity.nbt, QUEUE_KEY, &queue_now);
            write_json(&mut entity.nbt, RESERVE_KEY, &reserve);
        }
        fitted.save_tier(&mut entity.nbt);
        if catalyst != catalyst_before {
            machines::write_catalyst(&mut entity.nbt, &catalyst);
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
    machines::replace_front(&mut entity.data, window::encode(&shown));
}
