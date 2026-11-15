// Slice 10's mixer on the server (spec 4.3, 4.12 K1; guide `mixer`, `fluids`). It makes fluids
// out of water and items by the content mod's `rustcraft:mixing` recipes
// (rustcraft_rules::machines::mixer): the nutrient solution out of water, bone meal and redstone.
//
// - Items: its three input slots, one ingredient a slot; the item net and hoppers put only
//   ingredients there (rustcraft_machines::slot_takes).
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
};
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    machines::{
        mixer::{INPUTS, MixRecipe, Mixer, MixerView, Shown, Spec},
        sheet::{Bar as SheetBar, Sheet, Status as SheetStatus, severity_code},
        slots::Slot,
        style::Severity,
    },
    mining,
    net::fluids::BUCKET,
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

/// What each mixer showed last and the energy it got, by level and place.
#[derive(Default)]
struct State {
    shown: HashMap<(String, Pos), (Shown, f64)>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
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
    game.native_mod_block_entity(pos)
        .map(|e| read_tanks_of(&e.nbt))
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
    inputs: Vec<Slot>,
    spec: Spec,
    pub request: f64,
}

fn spec_of(game: &Game<'_>, pos: Pos, nbt: &Compound, t: &Tuning) -> Spec {
    let slots = base::modules_at(game, pos);
    let (work, energy_factor) = mining::factors(machines::read_work(nbt), &slots, t);
    Spec {
        tank: tank_size(t),
        energy: t.mixer_energy,
        work,
        energy_factor,
    }
}

pub(super) fn read(
    game: &Game<'_>,
    pos: Pos,
    recipes: &MachineRecipes,
    t: &Tuning,
) -> Option<Held> {
    let entity = game.native_mod_block_entity(pos)?;
    let tanks = read_tanks_of(&entity.nbt);
    let mixer = Mixer {
        water: tanks.water,
        product: tanks.product,
        progress: read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0),
    };
    let inputs: Vec<Slot> = (0..content::MIXER_SLOTS)
        .map(|i| slot_of(&entity, i))
        .collect();
    let spec = spec_of(game, pos, &entity.nbt, t);
    let request = mixer.request(&recipes.mixing, &inputs, &spec);
    Some(Held {
        mixer,
        inputs,
        spec,
        request,
    })
}

/// Runs the mixer on `share` of its request and writes back what changed.
pub(super) fn run(
    game: &mut Game<'_>,
    pos: Pos,
    held: Held,
    share: f64,
    recipes: &MachineRecipes,
    _t: &Tuning,
) {
    let Held {
        mut mixer,
        inputs,
        spec,
        request,
    } = held;
    let before = mixer.clone();
    let mut after = inputs.clone();
    let shown = mixer.tick(&recipes.mixing, &mut after, &spec, share);
    let got = if request > 0.0 { share * request } else { 0.0 };
    state()
        .shown
        .insert((game.native_dimension().to_owned(), pos), (shown, got));
    if mixer == before && after == inputs {
        return;
    }
    game.native_with_mod_block_entity(pos, |e| {
        machines::write_slots(e, 0..content::MIXER_SLOTS, &inputs, &after, &[]);
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
    });
}

// ---- What it shows ----

fn shown_at(dimension: &str, pos: Pos) -> Option<(Shown, f64)> {
    state().shown.get(&(dimension.to_owned(), pos)).copied()
}

fn shown_text(s: Shown) -> Text {
    match s {
        Shown::Working => line("rustcraft.machine.working", "Working", &[]),
        Shown::Slowed(p) => line(
            "rustcraft.machine.slowed",
            "Not enough energy: %s",
            &[Text::literal(format!("{p}%"))],
        ),
        Shown::NoEnergy => line("rustcraft.machine.no_energy", "No energy", &[]),
        Shown::NoIngredients => line("rustcraft.mixer.no_ingredients", "No ingredients", &[]),
        Shown::NoWater => line("rustcraft.mixer.no_water", "No water", &[]),
        Shown::Full => line(
            "rustcraft.machine.output_full",
            "Nowhere to put the product",
            &[],
        ),
    }
}

fn severity(s: Shown) -> Severity {
    match s {
        Shown::Working => Severity::Ok,
        Shown::Slowed(_) => Severity::Warn,
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

/// The look of a mixer: its state and its tanks.
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
    let mut text = shown_at(dimension, pos).map_or_else(
        || Text::translate("block.rustcraft.mixer", &[]),
        |(s, _)| shown_text(s),
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

/// The mixer's window: energy, water and product bars, its progress, its state.
pub(super) fn sheet(game: &Game<'_>, dimension: &str, pos: Pos, t: &Tuning, s: &mut Sheet) {
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return;
    };
    s.mode = Some(machines::read_work(&entity.nbt).code());
    let spec = spec_of(game, pos, &entity.nbt, t);
    let asked = spec.energy * spec.energy_factor;
    let (shown, got) = shown_at(dimension, pos).unwrap_or((Shown::NoIngredients, 0.0));
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
    let progress = read_f64(&entity.nbt, PROGRESS_KEY).clamp(0.0, 1.0);
    let secs = t.mixer_ticks / spec.work.max(1e-9) / t.ticks_per_second;
    s.rows.push(base::v(&line(
        "rustcraft.mixer.progress",
        "Progress %s · a bucket in %s s",
        &[
            Text::literal(format!("{:.0}%", progress * 100.0)),
            Text::literal(format!("{secs:.1}").replace(".0", "").replace('.', ",")),
        ],
    )));
    s.hints = vec![vec![base::v(&line(
        "rustcraft.mixer.hint",
        "Water with bone meal and redstone: nutrient solution; sugar and glowstone: accelerator; honey and copper dust: economizer; nether wart and gold dust: enricher",
        &[],
    ))]];
    s.status = Some(SheetStatus {
        severity: severity_code(severity(shown)),
        text: base::v(&shown_text(shown)),
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
        shown: shown_at(dimension, pos).map(|(s, _)| s),
    })
}

/// The self-test's plot cleared: what its mixers showed goes.
pub(super) fn forget_area(dimension: &str, min: Pos, max: Pos) {
    let inside = |p: Pos| (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
    state()
        .shown
        .retain(|(d, p), _| d != dimension || !inside(*p));
}
