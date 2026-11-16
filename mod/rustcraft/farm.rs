// Slice 12: the farm machines of spec 22 on the server (rustcraft_rules::utility::farm decides).
// rustcraft_utility runs their steps and draws their windows.
//
// - The fishing station: every 30–60 s, with a rod in its first slot and open water under it
//   (5 × 5, 2 deep), it rolls the game's fishing loot table with that rod (its luck enchantments
//   count, `sequence_loot` as the dispenser rolls shearing), the rod wears, the catch goes to its
//   slots and Path-of-craft-tree hears `fish`.
// - The feeder: two adults of a kind under 16 heads that take food get its food and fall in love,
//   as the game's own feeding sets them (`takes_food`, `in_love` 600); the game's breeding does the
//   rest. Path-of-craft-tree hears `breed`.
// - The shearer-milker: shears an adult unsheared sheep with its shears (the game's shearing loot
//   table by colour, as a dispenser shears) and milks an adult cow into an empty bucket (a cow
//   rests 5 minutes): `shear`, `milk`.
// - The apiary collector: takes a full hive's or nest's honey (`honey_level` 5 → 0) into a honey
//   bottle for a glass bottle, else its combs: `honey`.
use crate::game::{
    Game, rustcraft_base as base,
    rustcraft_server::line,
    rustcraft_utility::{self as util, fits, items, put, spend, take_one, with_items},
};
use horizon_client::{items::ItemStack, text::Text};
use rustcraft_rules::{
    butcher::Animal,
    geom::Pos,
    modules::{self, Module},
    reports::Report,
    tuning::Tuning,
    utility::{Shown, Util, farm as rules},
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

#[derive(Default)]
struct State {
    /// The tick each station may fish next.
    fish_due: HashMap<(String, Pos), u64>,
    /// The tick each cow was last milked.
    milked: HashMap<u32, u64>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The radius of its work: 4 blocks, 8 with a radius module.
fn radius(game: &Game<'_>, pos: Pos) -> f64 {
    if modules::count(&base::modules_at(game, pos), Module::Radius) > 0 { 8.0 } else { 4.0 }
}

fn near(pos: Pos, r: f64, at: [f64; 3]) -> bool {
    let c = pos.map(|v| f64::from(v) + 0.5);
    (at[0] - c[0]).abs() <= r && (at[2] - c[2]).abs() <= r && (at[1] - c[1]).abs() <= 3.0
}

pub(super) fn step(game: &mut Game<'_>, dimension: &str, pos: Pos, u: Util, cost: f64, now: u64, _t: &Tuning) -> Shown {
    match u {
        Util::Fishery => fish(game, dimension, pos, cost, now),
        Util::Feeder => feed(game, pos, cost),
        Util::Shearer => shear(game, pos, cost, now),
        _ => honey(game, pos, cost),
    }
}

fn owner(game: &Game<'_>, pos: Pos) -> Option<u128> {
    base::owner_at(game, pos)
}

// ---- The fishing station ----

/// Water blocks straight down from each cell of the 5 × 5 under it.
fn depths(game: &Game<'_>, pos: Pos) -> Vec<i32> {
    let half = rules::WATER_SIDE / 2;
    let mut out = Vec::new();
    for dx in -half..=half {
        for dz in -half..=half {
            let mut d = 0;
            while d < 4 && game.native_block([pos[0] + dx, pos[1] - 1 - d, pos[2] + dz]) == "minecraft:water" {
                d += 1;
            }
            out.push(d);
        }
    }
    out
}

/// Slice 18: whether any cell over the 5 × 5 water is filled: the row beside the station and the
/// row over it.
fn sky_closed(game: &Game<'_>, pos: Pos) -> bool {
    let half = rules::WATER_SIDE / 2;
    let mut filled = Vec::new();
    for dy in 0..=1 {
        for dx in -half..=half {
            for dz in -half..=half {
                if dy == 0 && dx == 0 && dz == 0 {
                    continue;
                }
                let b = game.native_block([pos[0] + dx, pos[1] + dy, pos[2] + dz]);
                filled.push(!matches!(b.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"));
            }
        }
    }
    !rules::open_sky(&filled)
}

/// The catch slots it uses: 9, 18 with a storage module.
fn catch_slots(game: &Game<'_>, pos: Pos) -> std::ops::Range<usize> {
    let storage = util::module_count(game, pos, rustcraft_rules::storage::modules::STORAGE)
        + modules::count(&base::modules_at(game, pos), Module::Storage);
    1..1 + rules::fish_buffer(storage)
}

fn fish(game: &mut Game<'_>, dimension: &str, pos: Pos, cost: f64, now: u64) -> Shown {
    let Some(rod) = items(game, pos).first().cloned().flatten() else {
        return Shown::Blocked("fish_no_rod");
    };
    if !rules::open_water(&depths(game, pos)) || sky_closed(game, pos) {
        return Shown::Blocked("fish_no_water");
    }
    let max = game.items().get(&rod.item).map_or(0, |i| i.max_damage);
    if rules::rod_worn(rod.damage, max) {
        return Shown::Blocked("fish_rod_worn");
    }
    let lure = rod.enchantment_level("minecraft:lure");
    let key = (dimension.to_owned(), pos);
    let due = *state().fish_due.entry(key.clone()).or_insert_with(|| now + rules::fish_wait(util::random(), lure));
    if now < due {
        return Shown::Working;
    }
    let slots = catch_slots(game, pos);
    if !fits(game, pos, slots.clone(), &[ItemStack::new("minecraft:cod", 1)]) {
        return Shown::Blocked("output_full");
    }
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    state().fish_due.insert(key, now + rules::fish_wait(util::random(), lure));
    let catch = game.sequence_loot("minecraft:gameplay/fishing", "minecraft:gameplay/fishing", Some(&rod));
    with_items(game, pos, |items| {
        if let Some(Some(r)) = items.get_mut(0) {
            r.damage += 1;
        }
    });
    let who = owner(game, pos);
    for c in catch {
        if let Some(o) = who {
            base::report(Report::Fish { owner: o, item: c.item.to_string(), count: c.count });
        }
        put(game, pos, slots.clone(), c);
    }
    Shown::Working
}

/// Slice 18 (guide `ui.js`, window `fishery`): the station's rows: the water, the rod (its
/// enchantments and durability), the wait to the next catch.
pub(super) fn fishery_rows(game: &Game<'_>, dimension: &str, pos: Pos, now: u64) -> Vec<Text> {
    let open = rules::open_water(&depths(game, pos)) && !sky_closed(game, pos);
    let water = if open {
        line("rustcraft.util.fishery.water_ok", "Open water 5×5, deeper than 2", &[])
    } else {
        line("rustcraft.util.fishery.water_bad", "Not enough open water: 5×5, deeper than 2, nothing over it", &[])
    };
    let rod = match items(game, pos).first().cloned().flatten() {
        Some(r) => {
            let max = game.items().get(&r.item).map_or(0, |i| i.max_damage);
            let ench: Vec<String> = r
                .enchantments()
                .iter()
                .map(|(e, l)| format!("{} {l}", e.rsplit(':').next().unwrap_or(e)))
                .collect();
            line(
                "rustcraft.util.fishery.rod",
                "Rod: %s of %s %s",
                &[
                    Text::literal(max.saturating_sub(r.damage).to_string()),
                    Text::literal(max.to_string()),
                    Text::literal(ench.join(", ")),
                ],
            )
        }
        None => line("rustcraft.util.fish_no_rod", "No rod", &[]),
    };
    let due = state().fish_due.get(&(dimension.to_owned(), pos)).copied();
    let wait = match due {
        Some(d) if d > now => line(
            "rustcraft.util.fishery.wait",
            "Next catch in %s s",
            &[Text::literal(((d - now) / 20).to_string())],
        ),
        _ => line("rustcraft.util.fishery.ready", "The line is out", &[]),
    };
    let rod_and_wait = line("rustcraft.util.fishery.rod_wait", "%s · %s", &[rod, wait]);
    vec![water, rod_and_wait]
}

// ---- The feeder ----

fn feed(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let slots = items(game, pos);
    let foods: Vec<String> = slots.iter().flatten().filter(|s| s.count > 0).map(|s| s.item.to_string()).collect();
    if foods.is_empty() {
        return Shown::Idle("feed_empty");
    }
    let r = radius(game, pos);
    let animals: Vec<rules::Fed> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.alive() && near(pos, r, [m.position.x, m.position.y, m.position.z]))
        .map(|m| rules::Fed {
            animal: Animal {
                id: m.id,
                kind: m.kind.id.to_string(),
                baby: m.is_baby(),
                tamed: m.extra.tame,
                named: m.custom_name.is_some(),
                leashed: m.extra.leash.is_some(),
            },
            takes_food: horizon_client::mobs::ai::takes_food(m) && m.extra.age == 0,
        })
        .collect();
    let Some((a, b, food)) = rules::pair_to_feed(&animals, &foods) else {
        return Shown::Idle("feed_nobody");
    };
    let (a, b) = (a.animal.id, b.animal.id);
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    for m in game.mobs.iter_mut().filter(|m| m.id == a || m.id == b) {
        m.extra.in_love = rules::LOVE_TICKS;
        m.extra.love_player = false;
    }
    with_items(game, pos, |items| {
        for _ in 0..2 {
            if let Some(slot) = items.iter().position(|s| s.as_ref().is_some_and(|s| &*s.item == food)) {
                take_one(items, slot);
            }
        }
    });
    if let Some(o) = owner(game, pos) {
        base::report(Report::Counted { kind: "breed", owner: o, count: 1 });
    }
    Shown::Working
}

// ---- The shearer-milker ----

fn shear(game: &mut Game<'_>, pos: Pos, cost: f64, now: u64) -> Shown {
    let slots = items(game, pos);
    let shears = slots.first().cloned().flatten();
    let has_bucket = slots.get(1).cloned().flatten().is_some_and(|s| s.count > 0);
    if shears.is_none() && !has_bucket {
        return Shown::Blocked("shear_no_tools");
    }
    let r = radius(game, pos);
    let in_reach = |m: &horizon_client::mobs::Mob| {
        !m.removed && m.alive() && !m.is_baby() && near(pos, r, [m.position.x, m.position.y, m.position.z])
    };
    let sheep = shears.as_ref().and_then(|_| {
        game.mobs
            .iter()
            .position(|m| in_reach(m) && &*m.kind.id == "minecraft:sheep" && !m.extra.sheared)
    });
    let rest = rules::MILK_REST_TICKS;
    let cow = has_bucket
        .then(|| {
            let milked = &state().milked;
            game.mobs.iter().position(|m| {
                in_reach(m)
                    && matches!(&*m.kind.id, "minecraft:cow" | "minecraft:mooshroom")
                    && milked.get(&m.id).is_none_or(|t| now >= t + rest)
            })
        })
        .flatten();
    if sheep.is_none() && cow.is_none() {
        return Shown::Idle("shear_nobody");
    }
    if !fits(game, pos, 2..11, &[ItemStack::new("minecraft:white_wool", 3), ItemStack::new("minecraft:milk_bucket", 1)]) {
        return Shown::Blocked("output_full");
    }
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    let who = owner(game, pos);
    if let (Some(i), Some(tool)) = (sheep, shears) {
        let color = horizon_client::mobs::render::DYE_NAMES[game.mobs[i].extra.color as usize & 15];
        let wool = game.sequence_loot("minecraft:shearing/sheep", &format!("minecraft:shearing/sheep/{color}"), Some(&tool));
        game.mobs[i].extra.sheared = true;
        let max = game.items().get(&tool.item).map_or(0, |i| i.max_damage);
        with_items(game, pos, |items| {
            if let Some(Some(s)) = items.get_mut(0) {
                s.damage += 1;
                if max > 0 && s.damage >= max {
                    items[0] = None;
                }
            }
        });
        for w in wool {
            put(game, pos, 2..11, w);
        }
        if let Some(o) = who {
            base::report(Report::Counted { kind: "shear", owner: o, count: 1 });
        }
    }
    if let Some(i) = cow {
        let id = game.mobs[i].id;
        state().milked.insert(id, now);
        with_items(game, pos, |items| take_one(items, 1));
        put(game, pos, 2..11, ItemStack::new("minecraft:milk_bucket", 1));
        if let Some(o) = who {
            base::report(Report::Milk { owner: o, entity: id });
        }
    }
    Shown::Working
}

// ---- The apiary collector ----

fn honey(game: &mut Game<'_>, pos: Pos, cost: f64) -> Shown {
    let r = radius(game, pos) as i32;
    let mut hive = None;
    'find: for dx in -r..=r {
        for dz in -r..=r {
            for dy in -2..=3 {
                let p = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
                let b = game.native_block(p);
                if (b == "minecraft:beehive" || b == "minecraft:bee_nest")
                    && game.native_block_property(p, "honey_level").as_deref() == Some("5")
                {
                    hive = Some(p);
                    break 'find;
                }
            }
        }
    }
    let Some(hive) = hive else {
        return Shown::Idle("honey_none");
    };
    let bottle = items(game, pos).first().cloned().flatten().is_some_and(|s| s.count > 0);
    let (item, count) = rules::harvest(bottle);
    if !fits(game, pos, 1..10, &[ItemStack::new(item, count)]) {
        return Shown::Blocked("output_full");
    }
    if !spend(game, pos, cost) {
        return Shown::NoEnergy;
    }
    game.native_set_block_property(hive, "honey_level", "0");
    if bottle {
        with_items(game, pos, |items| take_one(items, 0));
    }
    put(game, pos, 1..10, ItemStack::new(item, count));
    if let Some(o) = owner(game, pos) {
        base::report(Report::Counted { kind: "honey", owner: o, count: 1 });
    }
    Shown::Working
}
