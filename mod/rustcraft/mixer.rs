// Slice 10's mixer on the server (spec 4.3, 4.12 K1; guide `mixer`, `fluids`). It makes fluids
// out of water and items by the content mod's `rustcraft:mixing` recipes
// (rustcraft_rules::machines::mixer): the nutrient solution out of water and bone meal.
//
// - Items: its three input slots, one ingredient a slot; hoppers put only ingredients there
//   (rustcraft_machines::slot_takes), the magistral only those of the recipes turned on.
// - 0.21 (round 32, spec 39): its window has a toggle a recipe after its arrow; it makes the recipe
//   turned on it works on now while it has a run of it, then the next one on, and takes their
//   ingredients from the storage on its item net itself, a few runs at a time
//   (rustcraft_rules::machines::recipes). An order's recipe runs whatever the toggles say.
// - Fluids: its water tank (`water`, mB) and its product tank (`product_fluid`, `product`, mB) in its
//   saved compound. rustcraft_fluids fills the water from a water tank it touches and from a fluid
//   channel, and takes the product into the tank on top of it and along a fluid channel (the
//   mixer's fluid joint works both ways by default).
// - Energy, the work mode and the speed and economy modules as the slice-8 machines have them.
//
// A recipe file: {"type": "rustcraft:mixing", "ingredients": [{"item": "minecraft:bone_meal"},
// ...], "water": 1000, "result": {"fluid": "rustcraft:solution", "amount": 1000}, "time": 100}.
use crate::game::{
    Game, rustcraft_base as base, rustcraft_content as content, rustcraft_fluids as fluids,
    rustcraft_machines::{
        self as machines, MachineRecipes, read_f64, read_int, slot_of, write_f64, write_int,
    },
    rustcraft_server::line,
    rustcraft_storage as storage,
    rustcraft_utility::{needs_text, read_json, saved_now, saved_on, write_json},
};
use horizon_client::{containers::ModBlockEntity, text::Text};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    machines::{
        mixer::{INPUTS, MixRecipe, Mixer, MixerView, Shown, Spec, ingredient_sets},
        recipes::{self, Pick},
        sheet::{Bar as SheetBar, Sheet, Status as SheetStatus, Toggle, severity_code},
        slots::{Slot, count_in},
        style::Severity,
    },
    mining,
    net::{fluids::BUCKET, topology::Topology},
    power::SOLUTION,
    tuning::Tuning,
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const WATER_KEY: &str = "water";
const PRODUCT_FLUID_KEY: &str = "product_fluid";
const PRODUCT_KEY: &str = "product";
const PROGRESS_KEY: &str = "progress";
pub const WATER: &str = "minecraft:water";
/// Colours of the window's fluid bars (ARGB): water and the solution, as the guide draws them.
const WATER_COLOUR: u32 = 0xFF3F_76E4;
const SOLUTION_COLOUR: u32 = 0xFF59_D38C;
/// 0.21 (spec 39, RT3): how often, in level ticks, a mixer takes from the storage what its recipe
/// needs (it tops up below two runs, so a batch of 100 ticks never waits on it).
const FETCH_EVERY: u64 = 10;

/// What a mixer showed on its last tick, the energy it got and the recipe it named (the one it
/// makes, or the first turned on when it lacks ingredients).
#[derive(Clone, Debug)]
struct Seen {
    shown: Shown,
    got: f64,
    recipe: Option<String>,
}

/// What each mixer showed last, by level and place.
#[derive(Default)]
struct State {
    shown: HashMap<(String, Pos), Seen>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 0.21 (spec 39): the mixer's recipes in the window's order
/// (rustcraft_rules::machines::mixer::ordered) as the last rescan read them, for its window and
/// its presses, which have no level at hand.
static RECIPES: LazyLock<Mutex<Vec<MixRecipe>>> = LazyLock::new(Mutex::default);

fn recipes_now() -> Vec<MixRecipe> {
    RECIPES.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// The recipes rustcraft_machines::load_recipes read, in the window's order.
pub(super) fn remember(recipes: &[MixRecipe]) {
    *RECIPES.lock().unwrap_or_else(PoisonError::into_inner) = recipes.to_vec();
}

fn products(recipes: &[MixRecipe]) -> Vec<&str> {
    recipes.iter().map(|r| r.fluid.as_str()).collect()
}

/// The recipes a mixer's block entity turns on, a flag a recipe of `recipes`.
fn on_of(nbt: &Compound, recipes: &[MixRecipe]) -> Vec<bool> {
    recipes::flags(&products(recipes), &saved_on(nbt).unwrap_or_default())
}

/// A press on the toggle of recipe `i` in its window (rustcraft_base::on_sheet_button).
pub(super) fn press(entity: &mut ModBlockEntity, i: usize) {
    let all = recipes_now();
    let Some(r) = all.get(i) else {
        return;
    };
    let on = saved_on(&entity.nbt).unwrap_or_default();
    write_json(&mut entity.nbt, recipes::KEY, &recipes::press(&on, &r.fluid));
}

/// 0.21 (spec 39, RT5): whether the magistral brings the mixer `entity` `item`: an ingredient of a
/// recipe turned on. Before, a joint's allow list (0.21's IN5) chose; the world's update turns
/// its recipes on (RT7).
pub(super) fn net_takes(entity: &ModBlockEntity, item: &str) -> bool {
    let all = recipes_now();
    recipes::net_takes(&on_of(&entity.nbt, &all), &ingredient_sets(&all), item)
}

/// One mixing recipe file; none when an ingredient is not one item the game knows.
pub(super) fn parse_recipe(game: &Game<'_>, json: &serde_json::Value) -> Option<MixRecipe> {
    let list = json["ingredients"].as_array()?;
    if list.is_empty() || list.len() > INPUTS {
        return None;
    }
    let mut ingredients = Vec::new();
    for entry in list {
        let items = game.native_recipe_ingredient(&entry["item"])?.items;
        let mut items = items.into_iter();
        let (Some(item), None) = (items.next(), items.next()) else {
            return None;
        };
        let count = entry["count"].as_u64().unwrap_or(1).clamp(1, 64) as u32;
        ingredients.push((item, count));
    }
    let fluid = json["result"]["fluid"].as_str()?.to_owned();
    Some(MixRecipe {
        ingredients,
        water: json["water"].as_u64().unwrap_or(BUCKET),
        fluid,
        amount: json["result"]["amount"].as_u64().unwrap_or(BUCKET).max(1),
        ticks: json["time"].as_f64().filter(|t| *t >= 1.0).unwrap_or(100.0),
    })
}

/// A mixer's tanks, as the fluid tick reads and writes them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tanks {
    pub water: u64,
    pub product: Option<(String, u64)>,
}

fn read_tanks_of(nbt: &Compound) -> Tanks {
    let water = read_int(nbt, WATER_KEY).map_or(0, |v| v.max(0) as u64);
    let amount = read_int(nbt, PRODUCT_KEY).map_or(0, |v| v.max(0) as u64);
    let fluid = match nbt.get(&JavaString::from(PRODUCT_FLUID_KEY)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    };
    Tanks {
        water,
        product: fluid.filter(|_| amount > 0).map(|f| (f, amount)),
    }
}

fn write_tanks_to(nbt: &mut Compound, t: &Tanks) {
    write_int(nbt, WATER_KEY, t.water.min(i32::MAX as u64) as i32);
    let (fluid, amount) = t
        .product
        .as_ref()
        .map_or(("", 0), |(f, a)| (f.as_str(), *a));
    nbt.insert(
        JavaString::from(PRODUCT_FLUID_KEY),
        Tag::String(JavaString::from(fluid)),
    );
    write_int(nbt, PRODUCT_KEY, amount.min(i32::MAX as u64) as i32);
}

pub(super) fn read_tanks(game: &Game<'_>, pos: Pos) -> Option<Tanks> {
    crate::game::rustcraft_machines::entity(game, pos).map(|e| read_tanks_of(&e.nbt))
}

pub(super) fn write_tanks(game: &mut Game<'_>, pos: Pos, t: &Tanks) {
    game.native_with_mod_block_entity(pos, |e| write_tanks_to(&mut e.nbt, t));
}

/// The size of each of its tanks, mB.
pub(super) fn tank_size(t: &Tuning) -> u64 {
    t.mixer_buckets * BUCKET
}

// ---- The tick, from rustcraft_machines ----

pub struct Held {
    mixer: Mixer,
    /// Its real input slots, then its queue cells (slice 18).
    inputs: Vec<Slot>,
    /// The real slots as the block entity holds them, and the queue as saved.
    raw: Vec<Slot>,
    queue_before: Vec<Slot>,
    spec: Spec,
    pub request: f64,
    /// 0.21 (spec 39): the recipe it works on this tick, by the window's order.
    pick: Pick,
    /// An order's job runs on it: its recipe, whatever the toggles say (RT6), and nothing from
    /// the storage of its own.
    job: bool,
    /// The recipes turned on, none saved yet (a mixer of 0.20: RT7 turns them on at its first
    /// fetch), and the recipe it worked on as saved.
    on: Vec<bool>,
    fresh: bool,
    now: Option<usize>,
}

fn spec_of(game: &Game<'_>, pos: Pos, nbt: &Compound, t: &Tuning) -> Spec {
    let slots = base::modules_at(game, pos);
    let (work, energy_factor) = mining::factors(machines::read_work(nbt), &slots, t);
    // Slice 18 (A8): a higher tier mixes faster for the same energy a bucket.
    let tier = rustcraft_rules::tiers::family_of_block(content::MIXER).map_or(1.0, |f| {
        rustcraft_rules::tiers::speed(f, crate::game::rustcraft_tiers::tier_of(game, pos, content::MIXER), t.tier_speed_step)
    });
    // 0.20: its owner's tree shortens the work, not the draw (rustcraft_perks).
    let perk = crate::game::rustcraft_perks::machine_speed(game, pos, crate::game::rustcraft_content::Machine::Mixer);
    Spec {
        tank: tank_size(t),
        energy: t.mixer_energy * tier,
        work: work * tier * perk,
        energy_factor,
    }
}

/// Slice 18: the capacity and stack modules' queue cells behind the input slots
/// (rustcraft_rules::machines::queue): the real slots move into them, the mixer counts both.
fn queued(game: &Game<'_>, pos: Pos, raw: &[Slot], saved: &[Slot]) -> Vec<Slot> {
    let slots = base::modules_at(game, pos);
    // Slice 18, wave 2 (guide `mod_stack`): a stack module gives each input a cell of 256.
    let open = rustcraft_rules::machines::queue::open_cells_per_input(
        rustcraft_rules::modules::capacity_slots(&slots),
        rustcraft_rules::modules::stack_tier(&slots) > 0,
        raw.len(),
    );
    let mut visible = raw.to_vec();
    let mut cells = saved.to_vec();
    for slot in visible.iter_mut() {
        if let Some((item, _)) = slot.clone() {
            let size = rustcraft_rules::modules::machine_stack(&slots, machines::max_stack(game, &item));
            rustcraft_rules::machines::queue::intake(slot, &mut cells, open, size);
        }
    }
    let mut spare: Slot = None;
    rustcraft_rules::machines::queue::spill(&mut spare, &mut cells, open, |i| machines::max_stack(game, i));
    if let Some(back) = spare {
        match visible.iter_mut().find(|s| s.is_none()) {
            Some(free) => *free = Some(back),
            None => cells.push(Some(back)),
        }
    }
    visible.into_iter().chain(cells).collect()
}

/// Whether `inputs` hold a whole run of `recipe`.
fn has_run(recipe: &MixRecipe, inputs: &[Slot]) -> bool {
    recipes::runs_in(&recipe.ingredients, |item| u64::from(count_in(inputs, item))) > 0
}

pub(super) fn read(
    game: &Game<'_>,
    pos: Pos,
    recipes: &MachineRecipes,
    t: &Tuning,
) -> Option<Held> {
    let entity = crate::game::rustcraft_machines::entity(game, pos)?;
    let tanks = read_tanks_of(&entity.nbt);
    let mixer = Mixer {
        water: tanks.water,
        product: tanks.product,
        progress: read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0),
    };
    let raw: Vec<Slot> = (0..content::MIXER_SLOTS)
        .map(|i| slot_of(&entity, i))
        .collect();
    let queue_before: Vec<Slot> = read_json(&entity.nbt, machines::QUEUE_KEY);
    let inputs = queued(game, pos, &raw, &queue_before);
    let spec = spec_of(game, pos, &entity.nbt, t);
    let all = &recipes.mixing;
    let names = products(all);
    let on = on_of(&entity.nbt, all);
    let fresh = saved_on(&entity.nbt).is_none();
    let now = saved_now(&entity.nbt).and_then(|n| names.iter().position(|p| *p == n));
    // RT6: an order's recipe, by the fluid its step makes.
    let ordered = crate::game::rustcraft_orders::job_output_at(&game.native_dimension().to_owned(), pos)
        .and_then(|k| rustcraft_rules::orders::fluid_of(&k).map(str::to_owned))
        .and_then(|f| names.iter().position(|p| *p == f));
    let can = |i: usize| has_run(&all[i], &inputs);
    let pick = match ordered {
        Some(i) if can(i) => Pick::Run(i),
        Some(i) => Pick::Missing(i),
        None => recipes::pick(&on, now, can),
    };
    let request = pick
        .run()
        .map_or(0.0, |i| mixer.request(std::slice::from_ref(&all[i]), &inputs, &spec));
    Some(Held {
        mixer,
        inputs,
        raw,
        queue_before,
        spec,
        request,
        pick,
        job: ordered.is_some(),
        on,
        fresh,
        now,
    })
}

/// What a mixer's fetch changed: the recipes turned on by the world's update (RT7), and what went
/// back to the storage to free a slot.
struct Fetched {
    migrated: Option<Vec<String>>,
}

/// 0.21 (spec 39, RT3): the mixer at `pos` takes from the storage on its item net what a few runs
/// of its recipe need, into its visible input slots (`inputs`' first ones), one kind a slot; a
/// kind no slot is left for frees one, its items back to the storage. A mixer of 0.20 first turns
/// on the recipes its joints' allow lists cover (RT7).
#[allow(clippy::too_many_arguments)]
fn fetch(
    game: &Game<'_>,
    dimension: &str,
    topology: &Topology,
    pos: Pos,
    all: &[MixRecipe],
    held: &Held,
    now: Option<usize>,
    inputs: &mut [Slot],
) -> Fetched {
    let mut on = held.on.clone();
    let migrated = held.fresh.then(|| {
        let filters = crate::game::rustcraft_utility::joint_filters(game, pos);
        on = recipes::mixer_from_filters(&ingredient_sets(all), &filters);
        recipes::saved(&products(all), &on)
    });
    let out = Fetched { migrated };
    let Some(id) = storage::storage_by(dimension, pos, topology) else {
        return out;
    };
    let stored = |item: &str| storage::stored_count(dimension, id, item);
    let reach = |r: &MixRecipe| {
        recipes::runs_in(&r.ingredients, |item| u64::from(count_in(&inputs[..], item)) + stored(item)) > 0
    };
    let Some(r) = recipes::pick(&on, now, |i| reach(&all[i])).run().map(|i| &all[i]) else {
        return out;
    };
    let route = format!("storage#{id} -> rustcraft:mixer({})", rustcraft_rules::debug::pos_text(pos));
    let visible = content::MIXER_SLOTS.min(inputs.len());
    let needed = |item: &str| r.ingredients.iter().any(|(i, _)| i == item);
    let holds = |slots: &[Slot], item: &str| slots.iter().any(|s| s.as_ref().is_some_and(|(i, _)| i == item));
    let missing = r.ingredients.iter().filter(|(i, _)| !holds(&inputs[..visible], i)).count();
    let mut empty = inputs[..visible].iter().filter(|s| s.is_none()).count();
    for slot in inputs[..visible].iter_mut() {
        if empty >= missing {
            break;
        }
        if let Some((item, n)) = slot.clone()
            && !needed(&item)
        {
            storage::restore_into(dimension, id, &item, u64::from(n));
            crate::game::rustcraft_debug::flow(dimension, &format!("rustcraft:mixer({}) -> storage#{id}", rustcraft_rules::debug::pos_text(pos)), &item, u64::from(n));
            *slot = None;
            empty += 1;
        }
    }
    let have = |item: &str| u64::from(count_in(&inputs[..], item));
    let room = |item: &str| -> u64 {
        let max = u64::from(machines::max_stack(game, item));
        let same: u64 = inputs[..visible]
            .iter()
            .flatten()
            .filter(|(i, _)| i == item)
            .map(|(_, n)| max.saturating_sub(u64::from(*n)))
            .sum();
        if holds(&inputs[..visible], item) {
            same
        } else if empty > 0 {
            max
        } else {
            0
        }
    };
    let takes = recipes::fetch(&r.ingredients, have, stored, room, recipes::FETCH_RUNS);
    for (item, n) in takes {
        let took = storage::take_from(dimension, id, &item, u64::from(n)) as u32;
        if took == 0 {
            continue;
        }
        let max = machines::max_stack(game, &item);
        let mut left = took;
        for slot in inputs[..visible].iter_mut() {
            match slot {
                Some((i, c)) if *i == item => {
                    let k = left.min(max.saturating_sub(*c));
                    *c += k;
                    left -= k;
                }
                _ => {}
            }
        }
        if left > 0
            && let Some(free) = inputs[..visible].iter_mut().find(|s| s.is_none())
        {
            let k = left.min(max);
            *free = Some((item.clone(), k));
            left -= k;
        }
        if left > 0 {
            storage::restore_into(dimension, id, &item, u64::from(left));
        }
        crate::game::rustcraft_debug::flow(dimension, &route, &item, u64::from(took - left));
    }
    out
}

/// Runs the mixer on `share` of its request and writes back what changed; every FETCH_EVERY
/// ticks it takes from the storage what its recipe needs (RT3).
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    game: &mut Game<'_>,
    pos: Pos,
    held: Held,
    share: f64,
    recipes: &MachineRecipes,
    topology: &Topology,
    ticks: u64,
    _t: &Tuning,
) {
    let dimension = game.native_dimension().to_owned();
    let all = &recipes.mixing;
    let before = held.mixer.clone();
    let mut mixer = held.mixer.clone();
    let mut after = held.inputs.clone();
    // A recipe it turns to starts its batch afresh.
    if held.pick.run().is_some_and(|i| Some(i) != held.now) {
        mixer.progress = 0.0;
    }
    let shown = match held.pick {
        Pick::Run(i) => mixer.tick(std::slice::from_ref(&all[i]), &mut after, &held.spec, share),
        Pick::Missing(_) => {
            mixer.progress = 0.0;
            Shown::NoIngredients
        }
        Pick::Off => {
            mixer.progress = 0.0;
            Shown::Off
        }
    };
    let got = if held.request > 0.0 { share * held.request } else { 0.0 };
    let named = held.pick.named().and_then(|i| all.get(i)).map(|r| r.fluid.clone());
    state().shown.insert(
        (dimension.clone(), pos),
        Seen { shown, got, recipe: named },
    );
    let now = held.pick.run().or(held.now);
    let fetched = (!held.job && ticks.is_multiple_of(FETCH_EVERY))
        .then(|| fetch(game, &dimension, topology, pos, all, &held, now, &mut after));
    let migrated = fetched.and_then(|f| f.migrated);
    // The recipe it works on now, an order's too: else each tick of the job would start its batch afresh.
    let now_name = held.pick.run().filter(|i| Some(*i) != held.now).map(|i| all[i].fluid.clone());
    let cells = after.split_off(content::MIXER_SLOTS.min(after.len()));
    if mixer == before && after == held.raw && cells == held.queue_before && migrated.is_none() && now_name.is_none() {
        return;
    }
    let (raw, queue_before) = (held.raw, held.queue_before);
    game.native_with_mod_block_entity(pos, |e| {
        machines::write_slots(e, 0..content::MIXER_SLOTS, &raw, &after, &[]);
        if cells != queue_before {
            write_json(&mut e.nbt, machines::QUEUE_KEY, &cells);
        }
        if mixer.progress != before.progress {
            write_f64(&mut e.nbt, PROGRESS_KEY, mixer.progress);
        }
        if mixer.water != before.water || mixer.product != before.product {
            write_tanks_to(
                &mut e.nbt,
                &Tanks {
                    water: mixer.water,
                    product: mixer.product.clone(),
                },
            );
        }
        if let Some(list) = &migrated {
            write_json(&mut e.nbt, recipes::KEY, list);
        }
        if let Some(name) = &now_name {
            write_json(&mut e.nbt, recipes::NOW_KEY, &Some(name));
        }
    });
}

// ---- What it shows ----

fn seen_at(dimension: &str, pos: Pos) -> Option<Seen> {
    state().shown.get(&(dimension.to_owned(), pos)).cloned()
}

/// What it shows; 0.21 (spec 39, RT4) names its recipe (`recipe`, a fluid): «Делает:
/// питательный раствор», «Нет ингредиентов: ускоритель».
fn shown_text(s: Shown, recipe: Option<&str>) -> Text {
    match (s, recipe) {
        (Shown::Working, Some(f)) => line("rustcraft.mixer.making", "Making: %s", &[fluids::fluid_name(f)]),
        (Shown::NoIngredients, Some(f)) => line(
            "rustcraft.mixer.no_ingredients_for",
            "No ingredients: %s",
            &[fluids::fluid_name(f)],
        ),
        (Shown::Working, None) => line("rustcraft.machine.working", "Working", &[]),
        (Shown::Slowed(p), _) => line(
            "rustcraft.machine.slowed",
            "Not enough energy: %s",
            &[Text::literal(format!("{p}%"))],
        ),
        (Shown::NoEnergy, _) => line("rustcraft.machine.no_energy", "No energy", &[]),
        (Shown::NoIngredients, None) => line("rustcraft.mixer.no_ingredients", "No ingredients", &[]),
        (Shown::NoWater, _) => line("rustcraft.mixer.no_water", "No water", &[]),
        (Shown::Full, _) => line(
            "rustcraft.machine.output_full",
            "Nowhere to put the product",
            &[],
        ),
        (Shown::Off, _) => line("rustcraft.mixer.off", "Turn a recipe on in the window", &[]),
    }
}

fn severity(s: Shown) -> Severity {
    match s {
        Shown::Working => Severity::Ok,
        Shown::Slowed(_) | Shown::Off => Severity::Warn,
        _ => Severity::Bad,
    }
}

/// «Раствор: 2 из 4 вёдер».
fn tank_line(game: &Game<'_>, pos: Pos, t: &Tuning) -> Text {
    let tanks = read_tanks(game, pos).unwrap_or_default();
    let size = Text::literal(fluids::bucket_text(tank_size(t)));
    let mut text = line(
        "rustcraft.mixer.water_line",
        "water %s of %s buckets",
        &[
            Text::literal(fluids::bucket_text(tanks.water)),
            size.clone(),
        ],
    );
    if let Some((f, n)) = &tanks.product {
        text.push(Text::literal(" · "));
        text.push(line(
            "rustcraft.mixer.product_line",
            "%s %s of %s buckets",
            &[
                fluids::fluid_name(f),
                Text::literal(fluids::bucket_text(*n)),
                size,
            ],
        ));
    }
    text
}

/// The look of a mixer: its state, the recipe it works on and its tanks.
pub(super) fn diagnosis(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    block: &str,
    t: &Tuning,
) -> Option<Text> {
    if block != content::MIXER {
        return None;
    }
    let mut text = seen_at(dimension, pos).map_or_else(
        || Text::translate("block.rustcraft.mixer", &[]),
        |s| {
            let mut text = shown_text(s.shown, s.recipe.as_deref());
            // RT4: a mixer that stands names the recipe it stands on too.
            if let Some(f) = s.recipe.as_deref()
                && !matches!(s.shown, Shown::Working | Shown::NoIngredients | Shown::Off)
            {
                text.push(Text::literal(" · "));
                text.push(fluids::fluid_name(f));
            }
            text
        },
    );
    text.push(Text::literal(" · "));
    text.push(tank_line(game, pos, t));
    Some(text)
}

fn fluid_bar(label: Text, amount: u64, size: u64, colour: u32) -> SheetBar {
    SheetBar {
        label: base::v(&label),
        value: base::v(&line(
            "rustcraft.mixer.buckets",
            "%s of %s buckets",
            &[
                Text::literal(fluids::bucket_text(amount)),
                Text::literal(fluids::bucket_text(size)),
            ],
        )),
        share: if size > 0 {
            amount as f64 / size as f64
        } else {
            0.0
        },
        energy: false,
        colour: Some(colour),
    }
}

/// 0.21 (spec 39, RT1): a recipe's toggle: its icon the ingredient that tells it from the others
/// (the last of its file: bone meal, glowstone dust, copper dust, gold dust), its name the fluid,
/// its tooltip what it takes and the click.
fn recipe_toggle(game: &Game<'_>, r: &MixRecipe, on: bool, active: bool) -> Toggle {
    let mut hint = needs_text(game, &r.ingredients);
    hint.push(line("rustcraft.mixer.plus_water", " + water", &[]));
    hint.push(Text::literal(" · "));
    hint.push(if on {
        line("rustcraft.recipes.click_off", "click: turn off", &[])
    } else {
        line("rustcraft.recipes.click_on", "click: turn on", &[])
    });
    Toggle {
        icon: r.ingredients.last().map_or_else(|| "minecraft:water_bucket".to_owned(), |(i, _)| i.clone()),
        name: base::v(&fluids::fluid_name(&r.fluid)),
        on,
        active,
        hint: base::v(&hint),
    }
}

/// The mixer's window: energy, water and product bars, its progress, its recipes, its state.
pub(super) fn sheet(game: &Game<'_>, dimension: &str, pos: Pos, t: &Tuning, s: &mut Sheet) {
    let Some(entity) = crate::game::rustcraft_machines::entity(game, pos) else {
        return;
    };
    s.mode = Some(machines::read_work(&entity.nbt).code());
    let spec = spec_of(game, pos, &entity.nbt, t);
    let asked = spec.energy * spec.energy_factor;
    let seen = seen_at(dimension, pos).unwrap_or(Seen { shown: Shown::Off, got: 0.0, recipe: None });
    let (shown, got) = (seen.shown, seen.got);
    let e = |n: f64| Text::literal(format!("{n:.1}").replace(".0", "").replace('.', ","));
    s.bars.push(SheetBar {
        label: base::v(&line("rustcraft.window.energy", "Energy", &[])),
        value: base::v(&line(
            "rustcraft.window.energy_line",
            "gets %s of %s E/s",
            &[e(got), e(asked)],
        )),
        share: if asked > 0.0 {
            (got / asked).min(1.0)
        } else {
            0.0
        },
        energy: true,
        colour: None,
    });
    let tanks = read_tanks_of(&entity.nbt);
    let size = tank_size(t);
    s.bars.push(fluid_bar(
        line("rustcraft.mixer.water", "Water", &[]),
        tanks.water,
        size,
        WATER_COLOUR,
    ));
    let (label, amount, colour) = match &tanks.product {
        Some((f, n)) => (
            line(
                "rustcraft.mixer.product",
                "Product: %s",
                &[fluids::fluid_name(f)],
            ),
            *n,
            fluid_colour(f),
        ),
        None => (
            line("rustcraft.mixer.product_none", "Product: none yet", &[]),
            0,
            SOLUTION_COLOUR,
        ),
    };
    s.bars.push(fluid_bar(label, amount, size, colour));
    // Slice 18: the queue cells' items, when it has any.
    let cells: Vec<Slot> = read_json(&entity.nbt, machines::QUEUE_KEY);
    let queued: u32 = cells.iter().flatten().map(|(_, n)| n).sum();
    // Slice 18, wave 2 (`ui.js` mixer): the progress is the arrow after its slots; one line says
    // its pace, and its queue cells when it has any.
    let progress = read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0);
    s.work = Some(progress);
    let secs = t.mixer_ticks / spec.work.max(1e-9) / t.ticks_per_second;
    let mut row = line(
        "rustcraft.mixer.pace",
        "A bucket in %s s",
        &[Text::literal(format!("{secs:.1}").replace(".0", "").replace('.', ","))],
    );
    if !cells.is_empty() {
        row.push(Text::literal(" · "));
        row.push(line(
            "rustcraft.mixer.queue",
            "Queue: %s items in %s cells",
            &[Text::literal(queued.to_string()), Text::literal(cells.len().to_string())],
        ));
    }
    s.rows.push(base::v(&row));
    s.hints = vec![vec![base::v(&line(
        "rustcraft.mixer.recipes_hint",
        "The icons after the arrow are its recipes: a click turns one on or off. It makes the first one on while its ingredients last, then the next, and takes them from the storage itself",
        &[],
    ))]];
    // 0.21 (spec 39, RT1, RT4): a toggle a recipe, the one it makes now outlined.
    let all = recipes_now();
    let on = on_of(&entity.nbt, &all);
    let making = seen.recipe.as_deref().filter(|_| matches!(shown, Shown::Working | Shown::Slowed(_)));
    s.toggles = all
        .iter()
        .zip(&on)
        .take(rustcraft_rules::machines::sheet::MIXER_TOGGLES)
        .map(|(r, on)| recipe_toggle(game, r, *on, making == Some(r.fluid.as_str())))
        .collect();
    s.status = Some(SheetStatus {
        severity: severity_code(severity(shown)),
        text: base::v(&shown_text(shown, seen.recipe.as_deref())),
    });
}

/// A product's bar colour: the solution green, the catalysts their own (rustcraft_fluid_panels).
fn fluid_colour(fluid: &str) -> u32 {
    match fluid {
        SOLUTION => SOLUTION_COLOUR,
        rustcraft_rules::machines::catalyst::ACCELERATOR => 0xFFF5_D547,
        rustcraft_rules::machines::catalyst::ECONOMIZER => 0xFFE3_9A2E,
        rustcraft_rules::machines::catalyst::ENRICHER => 0xFFB6_5FD8,
        _ => WATER_COLOUR,
    }
}

/// What the self-test reads of a mixer.
pub(super) fn view(game: &Game<'_>, dimension: &str, pos: Pos) -> Option<MixerView> {
    let tanks = read_tanks(game, pos)?;
    Some(MixerView {
        water: tanks.water,
        product: tanks.product,
        shown: seen_at(dimension, pos).map(|s| s.shown),
    })
}

/// The self-test's plot cleared: what its mixers showed goes.
pub(super) fn forget_area(dimension: &str, min: Pos, max: Pos) {
    let inside = |p: Pos| (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
    state()
        .shown
        .retain(|(d, p), _| d != dimension || !inside(*p));
}
