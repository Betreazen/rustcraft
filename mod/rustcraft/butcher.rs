// Slice 11's butcher on the server (spec 23, round 26; rustcraft_rules::butcher decides). Every
// `Tuning::butcher_ticks` each butcher strikes one adult farm animal of a kind that has more
// adults in its pen than its window keeps (K2, K3): a blow no farm animal survives, through
// rustcraft_base::strike with no attacker, so the base game and Path-of-craft-tree do not take
// the kill for a player's own. Its death is told to Path-of-craft-tree as `slaughter` the same
// tick (rustcraft_base::deaths), never `kill`.
//
// - The sword: the last slot takes a sword (rustcraft_machines::slot_takes); a plain sword loses
//   a point of durability a strike and breaks at its end (K5), an energy blade spends its charge.
//   Slice 18: its enchantments act: Fire Aspect sets the animal alight before the blow, Looting
//   adds drops by the animal's table (rustcraft_rules::butcher::looting_bonus).
// - The drops: once a second the butcher picks up what lies in its pen into its nine slots
//   (K4), which the net and hoppers take from; what does not fit stays on the ground.
// - The number to keep, one an animal of rustcraft_rules::butcher::ANIMALS: JSON in its saved
//   compound (`keep`); a press on an animal's toggle in its window steps it (2 to 16, K3).
// - The radius module doubles its pen (4 → 8 blocks, K1). Slice 18: the kill's experience goes
//   into an experience condenser it touches, and is lost without one (K6).
// - Wave 2: tiers 2–4 (rustcraft_rules::tiers): a strike every 10 s on tier 2, faster a tier
//   above, ×0.85 slower an economy module; 4 E/s from its buffer.
use crate::game::{
    Game, rustcraft_base as base, rustcraft_content::Machine, rustcraft_machines as machines,
    rustcraft_mining as mining, rustcraft_server::{LevelNet, line},
};
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    butcher::{self as rules, ANIMALS, Animal, Keep, Kill, LOOT_SLOTS, SWORD_SLOT, Shown},
    geom::Pos,
    machines::{
        sheet::{Sheet, Status as SheetStatus, Toggle, severity_code},
        style::Severity,
    },
    modules::{self, Module},
    tuning::Tuning,
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const KEEP_KEY: &str = "keep";
/// The strike that kills: more than any farm animal's health.
const BLOW: f32 = 1000.0;
/// Drops are picked up this often, ticks.
const GATHER_EVERY: u64 = 20;

#[derive(Default)]
struct State {
    ticks: HashMap<String, u64>,
    /// Wave 2: the tick of each butcher's next strike (its pace is its tier's and its modules').
    due: HashMap<(String, Pos), u64>,
    shown: HashMap<(String, Pos), Shown>,
    /// 0.19.3: each butcher's recent kills, whose drops alone it picks up.
    kills: HashMap<(String, Pos), Vec<Kill>>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn read_keep(nbt: &Compound, t: &Tuning) -> Keep {
    match nbt.get(&JavaString::from(KEEP_KEY)) {
        Some(Tag::String(s)) => {
            serde_json::from_str(&s.to_string_lossy()).unwrap_or_else(|_| Keep::new(t))
        }
        _ => Keep::new(t),
    }
}

fn write_keep(nbt: &mut Compound, keep: &Keep) {
    let json = serde_json::to_string(keep).unwrap_or_default();
    nbt.insert(
        JavaString::from(KEEP_KEY),
        Tag::String(JavaString::from(json.as_str())),
    );
}

/// A press on animal `i`'s toggle (rustcraft_base::on_sheet_button): one more to keep.
pub(super) fn step_keep(nbt: &mut Compound, i: usize) {
    let t = Tuning::default();
    let mut keep = read_keep(nbt, &t);
    keep.step(i, &t);
    write_keep(nbt, &keep);
}

/// Its pen's radius, blocks: doubled by a radius module.
fn radius(game: &Game<'_>, pos: Pos, t: &Tuning) -> f64 {
    let slots = base::modules_at(game, pos);
    if modules::count(&slots, Module::Radius) > 0 {
        t.butcher_radius_module
    } else {
        t.butcher_radius
    }
}

fn centre(pos: Pos) -> [f64; 3] {
    [
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + 0.5,
        f64::from(pos[2]) + 0.5,
    ]
}

/// The animals in its pen: within `r` across and a few blocks up and down.
fn animals(game: &Game<'_>, pos: Pos, r: f64) -> Vec<Animal> {
    let c = centre(pos);
    game.mobs
        .iter()
        .filter(|m| !m.removed && m.alive())
        .filter(|m| {
            (m.position.x - c[0]).abs() <= r
                && (m.position.z - c[2]).abs() <= r
                && (m.position.y - c[1]).abs() <= 3.0
        })
        .map(|m| Animal {
            id: m.id,
            kind: m.kind.id.to_string(),
            baby: m.is_baby(),
            tamed: m.extra.tame,
            named: m.custom_name.is_some(),
            leashed: m.extra.leash.is_some(),
        })
        .collect()
}

/// Each butcher of the level: its drops picked up once a second, a strike every
/// `butcher_ticks`.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &LevelNet, t: &Tuning) {
    let butchers: Vec<Pos> = level
        .machines
        .iter()
        .filter(|(_, m)| **m == Machine::Butcher)
        .map(|(p, _)| *p)
        .collect();
    if butchers.is_empty() {
        return;
    }
    let now = {
        let mut s = state();
        let n = s.ticks.entry(dimension.to_owned()).or_default();
        *n += 1;
        *n
    };
    for pos in butchers {
        if now.is_multiple_of(GATHER_EVERY) {
            gather(game, dimension, pos, t, now);
        }
        // Wave 2: its block entity keeps its tier for the socket presses (A5), and it strikes at
        // its own pace.
        keep_tier(game, pos);
        let key = (dimension.to_owned(), pos);
        let due = *state().due.entry(key.clone()).or_insert(now + strike_every(game, pos, t));
        if now >= due {
            state().due.insert(key, now + strike_every(game, pos, t));
            let shown = strike(game, dimension, level, pos, t, now);
            // Slice 12: its arm chops while it works.
            crate::game::rustcraft_anim::show_on(game, pos, matches!(shown, Shown::Working(_)));
            state().shown.insert((dimension.to_owned(), pos), shown);
        }
    }
}

/// Wave 2: ticks between its strikes on its tier with its modules.
fn strike_every(game: &Game<'_>, pos: Pos, t: &Tuning) -> u64 {
    let tier = crate::game::rustcraft_tiers::tier_at(game, pos);
    let speed = rustcraft_rules::tiers::family_of_block(rules::BLOCK)
        .map_or(1.0, |f| rustcraft_rules::tiers::speed(f, tier, t.tier_speed_step));
    let economy = modules::count(&base::modules_at(game, pos), Module::Economy);
    // 0.20: its owner's tree strikes sooner (rustcraft_perks).
    let speed = speed * crate::game::rustcraft_perks::machine_speed(game, pos, Machine::Butcher);
    rules::tier_strike_ticks(rules::strike_ticks(t.butcher_ticks, economy, t.economy_work), speed)
}

/// Wave 2: the block entity's copy of its tier (rustcraft_rules::machines::sockets::TIER_KEY),
/// which the socket presses read.
fn keep_tier(game: &mut Game<'_>, pos: Pos) {
    let tier = i32::from(crate::game::rustcraft_tiers::tier_at(game, pos));
    let key = rustcraft_rules::machines::sockets::TIER_KEY;
    let saved = game.native_mod_block_entity(pos).map(|e| machines::read_int(&e.nbt, key));
    if saved.is_some_and(|s| s != Some(tier)) {
        game.native_with_mod_block_entity(pos, |e| machines::write_int(&mut e.nbt, key, tier));
    }
}

/// Picks up the drops of its recent kills in its pen into its drop slots. 0.19.3 (the owner:
/// «забойщик тоже забирает чужие предметы»): before, it took everything that lay in its pen.
fn gather(game: &mut Game<'_>, dimension: &str, pos: Pos, t: &Tuning, now: u64) {
    let kills = {
        let mut s = state();
        let Some(kills) = s.kills.get_mut(&(dimension.to_owned(), pos)) else {
            return;
        };
        kills.retain(|k| now.saturating_sub(k.tick) <= rules::KILL_TICKS);
        kills.clone()
    };
    if kills.is_empty() {
        return;
    }
    let r = radius(game, pos, t);
    let c = centre(pos);
    let lo = [c[0] - r, c[1] - 3.0, c[2] - r];
    let hi = [c[0] + r, c[1] + 3.0, c[2] + r];
    for e in game.native_item_entities(dimension, lo, hi) {
        if !rules::is_its_drop(&kills, now, e.position, &e.stack.item) {
            continue;
        }
        let max = machines::max_stack(game, &e.stack.item);
        let stack = e.stack.clone();
        let left = game
            .native_with_mod_block_entity(pos, move |b| {
                if b.items.len() < rules::SLOTS {
                    b.items.resize(rules::SLOTS, None);
                }
                mining::store(&mut b.items[..LOOT_SLOTS], stack, max)
            })
            .flatten();
        let left_count = left.as_ref().map_or(0, |l| l.count);
        if left_count == e.stack.count {
            continue;
        }
        let mut rest = e.stack.clone();
        rest.count = left_count;
        game.native_set_item_entity(dimension, e.id, rest);
    }
}

/// One strike, if it has a sword, room for the drops and an animal over its number.
fn strike(game: &mut Game<'_>, dimension: &str, level: &LevelNet, pos: Pos, t: &Tuning, now: u64) -> Shown {
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return Shown::NoSword;
    };
    let sword = entity
        .items
        .get(SWORD_SLOT)
        .cloned()
        .flatten()
        .filter(|s| s.count > 0 && rules::is_sword(&s.item));
    let full = entity.items.len() >= LOOT_SLOTS
        && entity.items[..LOOT_SLOTS]
            .iter()
            .all(|x| x.as_ref().is_some_and(|x| x.count > 0));
    let keep = read_keep(&entity.nbt, t);
    let r = radius(game, pos, t);
    let herd = animals(game, pos, r);
    let away = crate::game::rustcraft_trips::away(&entity);
    let (id, kind) = match rules::decide(sword.is_some(), full, &herd, &keep, t) {
        Ok(a) => (a.id, a.kind.clone()),
        // 0.20 (round 31): its sword is at the repair stand (rustcraft_trips).
        Err(Shown::NoSword) if away => return Shown::SwordAtRepair,
        Err(shown) => return shown,
    };
    // Slice 18: 4 E/s, from its buffer (×0.6 an economy module); a blade needs its charge.
    let economy = modules::count(&base::modules_at(game, pos), Module::Economy);
    // 0.20: a strike of an owner's tree costs less as it comes sooner (rustcraft_perks).
    let cost = rules::strike_energy(t.butcher_ticks, economy)
        / crate::game::rustcraft_perks::machine_speed(game, pos, Machine::Butcher);
    let energy = machines::buffer_energy(game, pos);
    if energy + 1e-9 < cost {
        return Shown::NoEnergy;
    }
    let blade = sword.as_ref().filter(|s| rules::is_blade(&s.item)).cloned();
    if let Some(b) = &blade
        && crate::game::rustcraft_gear::battery(game, b).is_none_or(|bat| bat.charge < rules::BLADE_CHARGE)
    {
        return Shown::NoEnergy;
    }
    machines::set_buffer_energy(game, pos, energy - cost);
    // Its sword's enchantments: Fire Aspect sets the animal alight so its meat drops cooked, and
    // Looting adds to the drops as the game's tables do (the strike itself has no attacker).
    let fire = sword.as_ref().map_or(0, |s| s.enchantment_level("minecraft:fire_aspect"));
    let looting = sword.as_ref().map_or(0, |s| s.enchantment_level("minecraft:looting"));
    if fire > 0 {
        for m in game.mobs.iter_mut().filter(|m| m.id == id) {
            m.fire_ticks = m.fire_ticks.max(80);
        }
    }
    // 0.19.3: where the animal stands, so the drops round it are known for the kill's.
    let stood = game
        .mobs
        .iter()
        .find(|m| m.id == id)
        .map(|m| [m.position.x, m.position.y, m.position.z]);
    let owner = base::owner_at(game, pos);
    let struck = match owner {
        Some(owner) => base::strike(game, dimension, id, BLOW, owner, base::Striker::Butcher),
        None => game.native_hurt_entity(dimension, id, BLOW, "minecraft:generic", None),
    };
    if struck && let Some(at) = stood {
        state()
            .kills
            .entry((dimension.to_owned(), pos))
            .or_default()
            .push(Kill { at, kind: kind.clone(), tick: now });
    }
    if struck {
        let mut random = crate::game::rustcraft_utility::random;
        for (item, n) in rules::looting_bonus(&kind, looting, fire > 0, &mut random) {
            let max = machines::max_stack(game, &item);
            let stack = horizon_client::items::ItemStack::new(item.as_str(), n);
            game.native_with_mod_block_entity(pos, move |b| {
                if b.items.len() < rules::SLOTS {
                    b.items.resize(rules::SLOTS, None);
                }
                mining::store(&mut b.items[..LOOT_SLOTS], stack, max)
            });
        }
        // K6 (slice 18): the kill's experience into a condenser beside it; 0.21: or on its line.
        xp_to_condenser(game, level, pos, &kind);
    }
    if struck && let Some(b) = blade {
        if let Some(bat) = crate::game::rustcraft_gear::battery(game, &b) {
            crate::game::rustcraft_gear::charge(&b, (bat.charge - rules::BLADE_CHARGE).max(0.0));
        }
    } else if struck && let Some(sword) = sword {
        let max_damage = game
            .items()
            .get(&sword.item)
            .map_or(0, |info| info.max_damage);
        let after = rules::worn(sword.damage, max_damage).map(|damage| {
            let mut s = sword.clone();
            s.damage = damage;
            s
        });
        game.native_with_mod_block_entity(pos, move |b| {
            if b.items.len() > SWORD_SLOT {
                b.items[SWORD_SLOT] = after;
            }
        });
    }
    Shown::Working(kind)
}

/// K6 (guide `butcher`): «опыт за убийство уходит в конденсатор опыта вплотную, без него
/// пропадает»: the points the animal is worth (the game's 1–3 for a farm animal) into a condenser
/// touching the butcher (a kill with no player behind it drops no orbs). 0.21 (K6a): without one,
/// into the nearest condenser with room beside the magistral line the butcher stands on.
fn xp_to_condenser(game: &mut Game<'_>, level: &LevelNet, pos: Pos, kind: &str) {
    use crate::game::rustcraft_utility as utility;
    let points = if kind == "minecraft:chicken" || kind == "minecraft:rabbit" { 1 } else { 2 };
    let block = rustcraft_rules::utility::Util::XpTank.spec().block;
    let room = |game: &Game<'_>, p: Pos| utility::xp_of(game, p) < rustcraft_rules::utility::xp::CAPACITY;
    let tank = utility::neighbour_of(game, pos, block).or_else(|| {
        rules::line_neighbours(&level.topology, &level.layout, pos)
            .into_iter()
            .find(|p| game.native_block(*p) == block && room(game, *p))
    });
    if let Some(tank) = tank {
        utility::add_xp(game, tank, points);
    }
}

fn animal_name(kind: &str) -> Text {
    Text::translate(&format!("entity.{}", kind.replace(':', ".")), &[])
}

fn shown_text(s: &Shown) -> Text {
    match s {
        Shown::Working(kind) => line(
            "rustcraft.butcher.working",
            "Works · last: %s",
            &[animal_name(kind)],
        ),
        Shown::NoSword => line("rustcraft.butcher.no_sword", "No sword", &[]),
        Shown::SwordAtRepair => line("rustcraft.butcher.sword_at_repair", "The sword is at the repair stand", &[]),
        Shown::NothingToCull => line(
            "rustcraft.butcher.nothing",
            "Nothing to cull: no more adults than it keeps",
            &[],
        ),
        Shown::OutputFull => line(
            "rustcraft.butcher.full",
            "The drops have nowhere to go",
            &[],
        ),
        Shown::NoEnergy => line("rustcraft.butcher.no_energy", "No energy", &[]),
    }
}

fn severity(s: &Shown) -> Severity {
    match s {
        Shown::Working(_) => Severity::Ok,
        Shown::NothingToCull | Shown::SwordAtRepair => Severity::Warn,
        Shown::NoSword | Shown::OutputFull | Shown::NoEnergy => Severity::Bad,
    }
}

/// What a player looking at it reads (A4).
pub(super) fn diagnosis(dimension: &str, pos: Pos) -> Text {
    state()
        .shown
        .get(&(dimension.to_owned(), pos))
        .map_or_else(|| line("rustcraft.butcher.idle", "Butcher", &[]), shown_text)
}

/// Its window: its pen and its sword, a toggle an animal with the number to keep, its slots.
pub(super) fn sheet(game: &Game<'_>, dimension: &str, pos: Pos, t: &Tuning, s: &mut Sheet) {
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return;
    };
    let r = radius(game, pos, t);
    s.rows.push(base::v(&line(
        "rustcraft.butcher.pen",
        "Pen: %s blocks around · a strike every %s s",
        &[
            Text::literal(format!("{r:.0}")),
            Text::literal(format!(
                "{:.1}",
                strike_every(game, pos, t) as f64 / t.ticks_per_second
            )),
        ],
    )));
    let sword = entity.items.get(SWORD_SLOT).cloned().flatten();
    s.rows.push(base::v(&match sword {
        Some(sw) if rules::is_sword(&sw.item) => {
            let max = game.items().get(&sw.item).map_or(0, |i| i.max_damage);
            line(
                "rustcraft.butcher.sword",
                "Sword in the last slot · durability %s of %s",
                &[
                    Text::literal(max.saturating_sub(sw.damage).to_string()),
                    Text::literal(max.to_string()),
                ],
            )
        }
        // 0.20 (round 31): away at the repair stand (rustcraft_trips).
        _ if crate::game::rustcraft_trips::away(&entity) => line(
            "rustcraft.butcher.sword_at_repair",
            "The sword is at the repair stand",
            &[],
        ),
        _ => line(
            "rustcraft.butcher.sword_none",
            "Put a sword in the last slot",
            &[],
        ),
    }));
    s.hints = vec![
        vec![base::v(&line(
            "rustcraft.butcher.pen_hint",
            "Culls adult farm animals over the number to keep; never babies, tamed, named or leashed ones",
            &[],
        ))],
        vec![base::v(&line(
            "rustcraft.butcher.sword_hint",
            "A plain sword loses durability with each strike; with a repair stand on its magistral it goes there before it breaks",
            &[],
        ))],
    ];
    let keep = read_keep(&entity.nbt, t);
    let counts = rules::adults(&animals(game, pos, r));
    s.toggles = ANIMALS
        .iter()
        .enumerate()
        .map(|(i, kind)| {
            let k = keep.of(i, t);
            Toggle {
                icon: rules::icon(kind),
                name: base::v(&line(
                    "rustcraft.butcher.animal",
                    "%s: %s adults · keeps %s",
                    &[
                        animal_name(kind),
                        Text::literal(counts[i].to_string()),
                        Text::literal(k.to_string()),
                    ],
                )),
                on: counts[i] > k,
                hint: base::v(&line(
                    "rustcraft.butcher.animal_hint",
                    "Click: keep one more (%s to %s)",
                    &[
                        Text::literal(t.butcher_keep_min.to_string()),
                        Text::literal(t.butcher_keep_max.to_string()),
                    ],
                )),
            }
        })
        .collect();
    let shown = state()
        .shown
        .get(&(dimension.to_owned(), pos))
        .cloned()
        .unwrap_or(Shown::NothingToCull);
    s.status = Some(SheetStatus {
        severity: severity_code(severity(&shown)),
        text: base::v(&shown_text(&shown)),
    });
}

/// The self-test's plot cleared: what its butchers showed goes.
pub(super) fn forget_area(dimension: &str, min: Pos, max: Pos) {
    let inside = |p: Pos| (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
    let mut s = state();
    s.shown.retain(|(d, p), _| d != dimension || !inside(*p));
    s.kills.retain(|(d, p), _| d != dimension || !inside(*p));
}

/// What the self-test reads of a butcher: what it showed last.
pub(super) fn shown(dimension: &str, pos: Pos) -> Option<Shown> {
    state().shown.get(&(dimension.to_owned(), pos)).cloned()
}
