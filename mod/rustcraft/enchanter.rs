// 0.21 (spec 48, EW1–EW12): the enchanting machine's choice, its window and its lapis.
//
// The owner: «окно у автомата вообще не понятное, как тут выбирать чары и так далее или хотя бы
// получать их»; «в станции зачарования так же и книги должно быть можно зачаровать, чтобы собрать
// нужный набор, а так же за доп плату повысить на один уже имеющееся зачарование»; «станция
// зачарования не брала сама лазурит из системы». rustcraft_rules::utility::enchanting decides; this
// module asks the game for the enchanting table's lists and keeps the choice in the block entity:
//
// - `job`: what the next step makes of the item in the first slot: one of the three offers the
//   table would give at the chosen level and the machine's seed (the chosen row, or «Лучшее»), or
//   one level more of an enchantment it has. An item or book that already has enchantments is
//   offered only those it lacks and that go with them (EW6). rustcraft_utility::enchant spends.
// - `done`: a new seed after each enchanting (EW5), and what was made, for the status line.
// - `press`: the window's buttons (the level, the option rows, the view).
// - `restock`: the lapis the next enchanting lacks, from the storage on its item net (EW10).
// - `sheet`: the window (EW1–EW3): labelled slots with ghost icons, the condenser's levels, the
//   level row, the offers or the raises, and a status that says what to do.
// - `keeps`: 0.21.2 (EW17), the result slot's toggle: the item net leaves what is done in it.
// - «Особые»: 0.21.2 (EW18–EW21; the owner: «я хочу чтобы в зачаровательном столе можно было и
//   уникальные зачарования получить, например починка»), the treasure enchantments the item or
//   book can take, at their most, the chosen one kept as `enchant_raise` with the pick 6.
use crate::game::{
    Game, rustcraft_base as base,
    rustcraft_server::{LevelNet, line},
    rustcraft_storage as storage,
    rustcraft_utility::{item_key, items, random, read_i64, shown_at, with_items, write_i64, xp_reached},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack, text::Text};
use horizon_core::random::{LegacyRandom, RandomSource};
use horizon_world::nbt::{Compound, JavaString, Tag};
use serde_json::Value;
use rustcraft_rules::{
    geom::Pos,
    machines::{
        sheet::{Sheet, Status as SheetStatus, Toggle, enchant_notes as notes, severity_code},
        style::Severity,
    },
    utility::{
        Shown,
        enchanting::{self, Action, Levels, OPTION_ROWS, Pick, RaisePrice, RaiseRefusal, TreasureRefusal, View},
        xp,
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex, PoisonError},
    time::{Duration, Instant},
};

/// The level the window chose (30 until one is chosen), as slice 18 kept it.
pub(super) const LEVEL_KEY: &str = "enchant_level";
const PICK_KEY: &str = "enchant_pick";
const RAISE_KEY: &str = "enchant_raise";
const VIEW_KEY: &str = "enchant_view";
const SEED_KEY: &str = "enchant_seed";
/// 0.21.2 (EW17): 1 while what is done stays in the result slot for the player.
const KEEP_KEY: &str = "enchant_keep";
/// 0.21.2 (EW18): the page of «Особые» shown, by four rows.
const PAGE_KEY: &str = "enchant_page";
const POOL: &str = "minecraft:in_enchanting_table";
const BOOK: &str = "minecraft:book";
const ENCHANTED_BOOK: &str = "minecraft:enchanted_book";
const LAPIS: &str = "minecraft:lapis_lazuli";
const LAPIS_STACK: u32 = 64;
/// How long the status line says what the last step made.
const DONE_SHOWN: Duration = Duration::from_secs(30);

/// What the last enchanting of each machine made, for «Зачаровано: … → склад».
static DONE: LazyLock<Mutex<HashMap<(String, Pos), (String, Instant)>>> = LazyLock::new(Mutex::default);

/// The choice the block entity keeps.
struct Settings {
    level: u32,
    pick: Pick,
    raise: Option<String>,
    view: View,
    seed: i32,
    keep: bool,
    page: usize,
}

fn read_string(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn settings_of(nbt: &Compound, pos: Pos) -> Settings {
    let level = read_i64(nbt, LEVEL_KEY);
    let seed = read_i64(nbt, SEED_KEY) as i32;
    Settings {
        level: if level > 0 { (level as u32).clamp(xp::LEVEL_MIN, xp::LEVEL_MAX) } else { xp::LEVEL_MAX },
        pick: Pick::of(read_i64(nbt, PICK_KEY)),
        raise: read_string(nbt, RAISE_KEY),
        view: View::of(read_i64(nbt, VIEW_KEY)),
        seed: if seed == 0 { enchanting::first_seed(pos) } else { seed },
        keep: read_i64(nbt, KEEP_KEY) == 1,
        page: usize::try_from(read_i64(nbt, PAGE_KEY)).unwrap_or(0),
    }
}

/// EW17: whether the item net leaves the result slot of the enchanting machine `entity` alone
/// (rustcraft_transport's snapshot).
pub(super) fn keeps(entity: &ModBlockEntity) -> bool {
    !enchanting::net_takes_result(read_i64(&entity.nbt, KEEP_KEY) == 1)
}

fn settings(game: &Game<'_>, pos: Pos) -> Settings {
    game.native_mod_block_entity(pos)
        .map_or_else(|| settings_of(&Compound::default(), pos), |e| settings_of(&e.nbt, pos))
}

/// The enchantments an item or book has (a book's stored ones).
fn have(stack: &ItemStack) -> Levels {
    stack.crafting_enchantments().iter().map(|(i, l)| (i.to_string(), *l)).collect()
}

/// The item the table's rules read: an enchanted book is a book.
fn kind_of(stack: &ItemStack) -> &str {
    if &*stack.item == ENCHANTED_BOOK { BOOK } else { &*stack.item }
}

/// `EnchantmentMenu.getEnchantmentList` for row `slot` at `cost`, with the pool the item has room
/// for (EW6): the same list as the table's for an item with no enchantments.
fn offer(game: &Game<'_>, seed: i32, slot: usize, cost: u32, stack: &ItemStack) -> Vec<(Arc<str>, u32)> {
    let e = &game.stations().enchantments;
    let kind = kind_of(stack);
    if e.enchantable(kind) == 0 {
        return Vec::new();
    }
    let all: Vec<String> = e.tag(POOL).iter().map(|s| s.to_string()).collect();
    let pool: Vec<Arc<str>> = enchanting::open_pool(&all, &have(stack), |a, b| e.compatible(a, b))
        .into_iter()
        .map(|s| Arc::from(s.as_str()))
        .collect();
    let mut r = LegacyRandom::new(i64::from(seed.wrapping_add(slot as i32)));
    let mut list = e.select(game.items(), &mut r, kind, cost as i32, &pool);
    if kind == BOOK && list.len() > 1 {
        let i = r.next_int_bound(list.len() as i32).unwrap_or(0) as usize;
        list.remove(i.min(list.len() - 1));
    }
    list
}

fn offers(game: &Game<'_>, st: &Settings, stack: &ItemStack) -> [Vec<(Arc<str>, u32)>; 3] {
    [0, 1, 2].map(|k| offer(game, st.seed, k, st.level, stack))
}

fn as_levels(list: &[(Arc<str>, u32)]) -> Levels {
    list.iter().map(|(i, l)| (i.to_string(), *l)).collect()
}

fn max_of(game: &Game<'_>, id: &str) -> u32 {
    game.stations().enchantments.get(id).map_or(1, |e| e.max_level)
}

fn anvil_of(game: &Game<'_>, id: &str) -> u32 {
    game.stations().enchantments.get(id).map_or(1, |e| e.anvil_cost)
}

/// EW18: the treasure enchantments `stack` can take, at their most, in «Особые»'s order: those
/// whose `supported_items` hold it (anything for a book), that go with what it has and that it
/// lacks. Unbreaking's and Mending's refusal on the mod's gear stays (`gear::enchant`).
fn treasure_list(game: &Game<'_>, stack: &ItemStack) -> Levels {
    let e = &game.stations().enchantments;
    let kind = kind_of(stack);
    let tag: Vec<String> = e.tag(enchanting::TREASURE_TAG).iter().map(|s| s.to_string()).collect();
    let order = enchanting::treasure_order(&tag, |id| e.is_curse(id));
    let gear = rustcraft_rules::gear::Gear::of(kind).is_some();
    let fits = |id: &str| {
        let refused = gear && rustcraft_rules::gear::enchant::refused_on_gear(id);
        !refused && (kind == BOOK || e.can_enchant(game.items(), id, kind))
    };
    enchanting::treasures(&order, &have(stack), fits, |a, b| e.compatible(a, b), |id| max_of(game, id))
}

/// What one step makes of an item and what it costs.
pub(super) struct Job {
    pub result: ItemStack,
    pub lapis: u32,
    /// The condenser must hold this many points (EW3)…
    pub need_points: u32,
    /// …and gives this many.
    pub points: u32,
    /// The «enchant» report's count (1–3, as the table's rows).
    pub report: u32,
}

/// The next step's job for `item` (one of it), or why there is none (a status key's tail).
pub(super) fn job(game: &Game<'_>, pos: Pos, item: &ItemStack) -> Result<Job, &'static str> {
    let st = settings(game, pos);
    // EW19: the chosen treasure, or the machine waits and says why (never «Лучшее» instead).
    if st.pick == Pick::Treasure
        && let Some(id) = &st.raise
    {
        return match enchanting::treasure(&treasure_list(game, item), &have(item), id) {
            Ok(level) => Ok(step_job(item, id, level, enchanting::treasure_price(anvil_of(game, id), level))),
            Err(TreasureRefusal::Has) => Err("enchant_treasure_has"),
            Err(TreasureRefusal::Unfit) => Err("enchant_treasure_unfit"),
        };
    }
    if st.pick == Pick::Raise
        && let Some(id) = &st.raise
    {
        match enchanting::raise(&have(item), id, max_of(game, id)) {
            Ok(level) => return Ok(step_job(item, id, level, enchanting::raise_price(anvil_of(game, id), level))),
            Err(RaiseRefusal::AtMax) => return Err("enchant_at_max"),
            // EW9: an item without it is enchanted as «Лучшее».
            Err(RaiseRefusal::Missing) => {}
        }
    }
    let lists = offers(game, &st, item);
    let levels = lists.clone().map(|l| as_levels(&l));
    if st.pick == Pick::Remove
        && let Some(id) = &st.raise
        && let Some(rest) = enchanting::remove(&have(item), id)
    {
        if game.stations().enchantments.is_curse(id) {
            return Err("enchant_curse_stays");
        }
        return Ok(remove_job(item, rest));
    }
    // EW8, EW15: a raise or a removal the item has nothing for: «Лучшее» (a treasure with no
    // enchantment named too).
    let pick = if matches!(st.pick, Pick::Raise | Pick::Remove | Pick::Treasure) { Pick::Best } else { st.pick };
    let k = enchanting::chosen(pick, &levels).ok_or("enchant_not_enchantable")?;
    let mut result = if &*item.item == BOOK { item.transmute(ENCHANTED_BOOK, 1) } else { item.clone() };
    for (id, l) in &lists[k] {
        result.enchant(id, *l);
    }
    let need = xp::enchant_cost(st.level);
    Ok(Job {
        result,
        lapis: need,
        need_points: xp::total(st.level),
        points: xp::enchant_points(st.level, need),
        report: need,
    })
}

/// A raise (EW7) or a treasure (EW20): `id` at `level` for `price`; a plain book becomes an
/// enchanted book that stores it (EW21).
fn step_job(item: &ItemStack, id: &str, level: u32, price: RaisePrice) -> Job {
    let points = enchanting::raise_points(price);
    let mut result = if &*item.item == BOOK { item.transmute(ENCHANTED_BOOK, 1) } else { item.clone() };
    result.enchant(id, level);
    Job {
        result,
        lapis: price.lapis,
        need_points: points.max(xp::total(price.levels)),
        points,
        report: xp::enchant_cost(price.levels.min(xp::LEVEL_MAX)),
    }
}

/// EW15: the item without one enchantment, for a lapis and no experience; a book left with none
/// is a plain book again.
fn remove_job(item: &ItemStack, rest: Levels) -> Job {
    let rest: Vec<(Arc<str>, u32)> = rest.into_iter().map(|(id, l)| (Arc::from(id.as_str()), l)).collect();
    let result = if &*item.item == ENCHANTED_BOOK && rest.is_empty() {
        let mut plain = item.clone();
        plain.set_crafting_enchantments(Vec::new());
        plain.transmute(BOOK, 1)
    } else {
        let mut out = item.clone();
        out.set_crafting_enchantments(rest);
        out
    };
    Job { result, lapis: enchanting::REMOVE_LAPIS, need_points: 0, points: 0, report: 0 }
}

/// After an enchanting: a new seed (EW5), and what was made.
pub(super) fn done(game: &mut Game<'_>, pos: Pos, made: &ItemStack) {
    let old = settings(game, pos).seed;
    let seed = enchanting::next_seed(old, random());
    game.native_with_mod_block_entity(pos, |e| write_i64(&mut e.nbt, SEED_KEY, i64::from(seed)));
    let key = (game.native_dimension().to_owned(), pos);
    DONE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, (made.item.to_string(), Instant::now()));
}

/// A press on the machine's button `i` (the block entity is out of the world: written to it).
pub(super) fn press(game: &Game<'_>, entity: &mut ModBlockEntity, i: usize) {
    let view = View::of(read_i64(&entity.nbt, VIEW_KEY));
    match enchanting::press(i, view) {
        Some(Action::Step(by)) => {
            let now = read_i64(&entity.nbt, LEVEL_KEY);
            let now = if now > 0 { now as u32 } else { xp::LEVEL_MAX };
            write_i64(&mut entity.nbt, LEVEL_KEY, i64::from(xp::step_level(now, by)));
        }
        Some(Action::Pick(p)) => write_i64(&mut entity.nbt, PICK_KEY, p.code()),
        Some(Action::RaiseRow(k)) => {
            let Some(item) = entity.items.first().cloned().flatten() else { return };
            let rows = enchanting::raise_rows(&have(&item), |id| max_of(game, id));
            if let Some(row) = rows.get(k).filter(|r| !r.at_max()) {
                write_i64(&mut entity.nbt, PICK_KEY, Pick::Raise.code());
                entity.nbt.insert(JavaString::from(RAISE_KEY), Tag::String(JavaString::from(row.id.as_str())));
            }
        }
        Some(Action::RemoveRow(k)) => {
            let Some(item) = entity.items.first().cloned().flatten() else { return };
            let rows = enchanting::raise_rows(&have(&item), |id| max_of(game, id));
            if let Some(row) = rows.get(k).filter(|r| !game.stations().enchantments.is_curse(&r.id)) {
                write_i64(&mut entity.nbt, PICK_KEY, Pick::Remove.code());
                entity.nbt.insert(JavaString::from(RAISE_KEY), Tag::String(JavaString::from(row.id.as_str())));
            }
        }
        // EW19: the row's treasure itself is kept, as a raise's (EW8).
        Some(Action::TreasureRow(k)) => {
            let Some(item) = entity.items.first().cloned().flatten() else { return };
            let page = usize::try_from(read_i64(&entity.nbt, PAGE_KEY)).unwrap_or(0);
            let list = treasure_list(game, &item);
            if let Some((id, _)) = enchanting::treasure_page(&list, page).get(k) {
                write_i64(&mut entity.nbt, PICK_KEY, Pick::Treasure.code());
                entity.nbt.insert(JavaString::from(RAISE_KEY), Tag::String(JavaString::from(id.as_str())));
            }
        }
        Some(Action::NextPage) => {
            let n = entity.items.first().cloned().flatten().map_or(0, |i| treasure_list(game, &i).len());
            let page = usize::try_from(read_i64(&entity.nbt, PAGE_KEY)).unwrap_or(0);
            write_i64(&mut entity.nbt, PAGE_KEY, enchanting::next_page(page, n) as i64);
        }
        Some(Action::Show(v)) => {
            write_i64(&mut entity.nbt, VIEW_KEY, v.code());
            write_i64(&mut entity.nbt, PAGE_KEY, 0);
        }
        Some(Action::Keep) => {
            let keep = read_i64(&entity.nbt, KEEP_KEY) == 1;
            write_i64(&mut entity.nbt, KEEP_KEY, i64::from(!keep));
        }
        None => {}
    }
}

/// EW10: the lapis the next enchanting lacks, from the storage on the machine's item net.
pub(super) fn restock(game: &mut Game<'_>, dimension: &str, level: &LevelNet, pos: Pos) {
    let slots = items(game, pos);
    let Some(mut item) = slots.first().cloned().flatten() else { return };
    item.count = 1;
    let Ok(job) = job(game, pos, &item) else { return };
    let lapis = slots.get(1).cloned().flatten();
    if lapis.as_ref().is_some_and(|s| &*s.item != LAPIS) {
        return;
    }
    let held = lapis.map_or(0, |s| s.count);
    let n = enchanting::lapis_to_take(true, held, job.lapis, LAPIS_STACK);
    if n == 0 {
        return;
    }
    let Some(id) = storage::storage_by(dimension, pos, &level.topology) else { return };
    let took = storage::take_from(dimension, id, LAPIS, u64::from(n)) as u32;
    if took == 0 {
        return;
    }
    let placed = with_items(game, pos, |cells| {
        if cells.len() < 2 {
            cells.resize(2, None);
        }
        if cells[1].is_none() {
            cells[1] = Some(ItemStack::new(LAPIS, took));
            true
        } else if let Some(s) = cells[1].as_mut().filter(|s| &*s.item == LAPIS) {
            s.count += took;
            true
        } else {
            false
        }
    }) == Some(true);
    if !placed {
        storage::restore_into(dimension, id, LAPIS, u64::from(took));
        return;
    }
    let route = format!("storage#{id} -> rustcraft:enchanter({})", rustcraft_rules::debug::pos_text(pos));
    crate::game::rustcraft_debug::flow(dimension, &route, LAPIS, u64::from(took));
}

// ---- The window ----

fn lit(s: impl ToString) -> Text {
    Text::literal(s.to_string())
}

fn roman(level: u32) -> String {
    const NAMES: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    NAMES.get(level.wrapping_sub(1) as usize).map_or_else(|| level.to_string(), |s| (*s).to_owned())
}

/// «Острота IV», as the game names an enchantment at a level.
fn ench_name(game: &Game<'_>, id: &str, level: u32) -> Text {
    let e = game.stations().enchantments.get(id);
    let name = match e {
        Some(e) => line(&e.description_key, &e.description, &[]),
        None => lit(id),
    };
    if level == 1 && e.is_some_and(|e| e.max_level == 1) {
        return name;
    }
    let lvl = line(&format!("enchantment.level.{level}"), &roman(level), &[]);
    line("rustcraft.util.enchanter.name_level", "%s %s", &[name, lvl])
}

/// «Острота IV, Добыча II».
fn list_text(game: &Game<'_>, list: &[(Arc<str>, u32)]) -> Text {
    let mut parts = list.iter().map(|(i, l)| ench_name(game, i, *l));
    let Some(first) = parts.next() else {
        return line("rustcraft.util.enchanter.nothing", "nothing for this item", &[]);
    };
    parts.fold(first, |a, b| line("rustcraft.util.enchanter.and", "%s, %s", &[a, b]))
}

/// The first enchantment of a list, «…» when more follow.
fn list_short(game: &Game<'_>, list: &[(Arc<str>, u32)]) -> Text {
    match list {
        [] => lit("—"),
        [(i, l)] => ench_name(game, i, *l),
        [(i, l), ..] => line("rustcraft.util.enchanter.more", "%s …", &[ench_name(game, i, *l)]),
    }
}

fn item_name(id: &str) -> Text {
    line(&item_key(id), id, &[])
}

struct Button {
    text: Text,
    name: Text,
    hint: Text,
    on: bool,
    off: bool,
    /// Not drawn, and not pressed (a «×» in «Новые», or with no enchantment beside it).
    hidden: bool,
}

fn button(text: Text, name: Text, hint: Text, on: bool, off: bool) -> Button {
    Button { text, name, hint, on, off, hidden: false }
}

/// What the window's sheet shows for the enchanting machine at `pos` (EW1–EW3, EW8, EW12).
pub(super) fn sheet(game: &Game<'_>, dimension: &str, level: &LevelNet, pos: Pos, s: &mut Sheet) {
    let st = settings(game, pos);
    let slots = items(game, pos);
    let item = slots.first().cloned().flatten().map(|mut i| {
        i.count = 1;
        i
    });
    let to_storage = storage::storage_by(dimension, pos, &level.topology).is_some();
    // 0.21 (spec 44, XC10): the condensers it reaches, beside it and over its experience channel.
    let tuning = crate::game::rustcraft_server::tuning();
    let (tanks, held) = xp_reached(game, level, tuning, pos, rustcraft_rules::net::xp::Way::Draw);
    let tank = (tanks > 0).then(|| u32::try_from(held).unwrap_or(u32::MAX));
    let job = item.as_ref().map(|i| job(game, pos, i));
    let ok_job = job.as_ref().and_then(|j| j.as_ref().ok());
    let lapis = ok_job.map_or(xp::enchant_cost(st.level), |j| j.lapis);
    let need_points = ok_job.map_or(xp::total(st.level), |j| j.need_points);
    s.rows.clear();
    s.hints.clear();
    s.notes = vec![Value::Null; notes::COUNT];
    s.note_tips = vec![Vec::new(); notes::COUNT];
    let mut note = |k: usize, text: Text, tips: Vec<Text>| {
        s.notes[k] = base::v(&text);
        s.note_tips[k] = tips.iter().map(base::v).collect();
    };
    let item_tips = vec![
        line("rustcraft.util.enchanter.item_tip", "A book or a tool: one a step", &[]),
        line(
            "rustcraft.util.enchanter.item_tip2",
            "An enchanted one gets enchantments it lacks, or +1 to one it has",
            &[],
        ),
    ];
    note(notes::ITEM, line("rustcraft.util.enchanter.item", "Item", &[]), item_tips.clone());
    let lapis_tip = if to_storage {
        line("rustcraft.util.enchanter.lapis_storage", "Taken from the storage of its net by itself", &[])
    } else {
        line("rustcraft.util.enchanter.lapis_tip", "Put lapis here, or bring it by the magistral", &[])
    };
    note(notes::LAPIS, line("rustcraft.util.enchanter.lapis", "Lapis ×%s", &[lit(lapis)]), vec![lapis_tip.clone()]);
    // EW17: what is done goes to the storage unless the window keeps it here.
    let result_tip = if to_storage && !st.keep {
        line("rustcraft.util.enchanter.result_storage", "The magistral takes what is done to the storage", &[])
    } else {
        line("rustcraft.util.enchanter.result_tip", "Take what is done from here", &[])
    };
    note(notes::TANK_LABEL, line("rustcraft.util.enchanter.tank", "Condenser", &[]), Vec::new());
    let need_levels = xp::levels_from(0, need_points);
    let (tank_text, tank_tip) = match tank {
        Some(points) => {
            let held = lit(xp::levels_from(0, points));
            let text = if points >= need_points {
                line("rustcraft.util.enchanter.tank_levels", "%s levels", &[held.clone()])
            } else {
                line("rustcraft.util.enchanter.tank_short", "%s lv. — too few", &[held.clone()])
            };
            let tip = line(
                "rustcraft.util.enchanter.tank_tip",
                "The condensers it reaches hold %s levels; it needs at least %s",
                &[held, lit(need_levels)],
            );
            (text, tip)
        }
        None => (
            line("rustcraft.util.enchanter.no_tank", "none found", &[]),
            line("rustcraft.util.enchanter.no_tank_tip", "Put an experience condenser beside the machine or on its magistral line", &[]),
        ),
    };
    note(notes::TANK, tank_text, vec![tank_tip]);
    let cost = xp::enchant_cost(st.level);
    note(
        notes::LEVEL,
        line("rustcraft.util.enchanter.level_short", "%s", &[lit(st.level)]),
        vec![line(
            "rustcraft.util.enchanter.level_tip",
            "Enchanting level %s of 1–30, as the table's row; costs %s levels and %s lapis",
            &[lit(st.level), lit(cost), lit(cost)],
        )],
    );
    // The slots' ghosts and tooltips (EW2).
    let result_icon = ok_job.map_or_else(|| ENCHANTED_BOOK.to_owned(), |j| j.result.item.to_string());
    let icons = [BOOK.to_owned(), LAPIS.to_owned(), result_icon];
    s.ghosts = icons
        .into_iter()
        .enumerate()
        .map(|(k, icon)| slots.get(k).cloned().flatten().is_none().then_some(icon))
        .collect();
    s.slot_tips = [item_tips, vec![lapis_tip], vec![result_tip]]
        .iter()
        .map(|t| t.iter().map(base::v).collect())
        .collect();
    let mut buttons = buttons(game, &st, item.as_ref());
    buttons.push(keep_button(st.keep, to_storage));
    buttons.push(treasure_view_button(game, &st, item.as_ref()));
    s.toggles = buttons
        .iter()
        .map(|b| Toggle { icon: String::new(), name: base::v(&b.name), on: b.on, active: false, hint: base::v(&b.hint) })
        .collect();
    s.toggle_texts = buttons.iter().map(|b| Some(if b.hidden { Value::Null } else { base::v(&b.text) })).collect();
    s.toggle_off = buttons.iter().map(|b| b.off).collect();
    s.status = Some(status(game, dimension, pos, &st, item.as_ref(), tank, need_points, lapis, to_storage));
}

/// EW17 (the owner: «нужно сделать тумблер уносить сразу на склад или нет»): the result slot's
/// label as a toggle: «→ на склад» (or «Готово» off a storage's line) while the magistral takes
/// what is done, «← здесь», lit, while it stays in the slot for the player.
fn keep_button(keep: bool, to_storage: bool) -> Button {
    let name = line("rustcraft.util.enchanter.keep_name", "Done: to the storage or here", &[]);
    if keep {
        return button(
            line("rustcraft.util.enchanter.keep_here", "← here", &[]),
            name,
            line(
                "rustcraft.util.enchanter.keep_hint_on",
                "What is done stays in this slot, the magistral leaves it. Click: to the storage",
                &[],
            ),
            true,
            false,
        );
    }
    let (text, hint) = if to_storage {
        (
            line("rustcraft.util.enchanter.to_storage", "→ storage", &[]),
            line(
                "rustcraft.util.enchanter.keep_hint_storage",
                "The magistral takes what is done to the storage. Click: keep it here",
                &[],
            ),
        )
    } else {
        (
            line("rustcraft.util.enchanter.result", "Done", &[]),
            line(
                "rustcraft.util.enchanter.keep_hint_off",
                "Take what is done from here. Click: the magistral will leave it here",
                &[],
            ),
        )
    };
    button(text, name, hint, false, false)
}

/// The fourteen buttons: the level steps, the option rows, the views «Новые» and «Свои», the «×»
/// of the rows (the result slot's toggle and «Особые» follow them, [`keep_button`],
/// [`treasure_view_button`]).
fn buttons(game: &Game<'_>, st: &Settings, item: Option<&ItemStack>) -> Vec<Button> {
    let raise_view = st.view == View::Raise;
    let no_level = st.view != View::New;
    let mut out: Vec<Button> = ["-10", "-1", "+1", "+10"]
        .iter()
        .zip(["minus10", "minus1", "plus1", "plus10"])
        .map(|(t, k)| {
            let hint = if no_level {
                line("rustcraft.util.enchanter.level_unused", "Only «New» needs a level", &[])
            } else {
                line("rustcraft.util.enchanter.hint", "Click: the enchanting level (1–30)", &[])
            };
            button(lit(t), line(&format!("rustcraft.util.enchanter.toggle.{k}"), k, &[]), hint, false, no_level)
        })
        .collect();
    match st.view {
        View::Raise => out.extend(raise_buttons(game, st, item)),
        View::Treasure => out.extend(treasure_buttons(game, st, item)),
        View::New => out.extend(offer_buttons(game, st, item)),
    }
    out.push(button(
        line("rustcraft.util.enchanter.view_new", "New", &[]),
        line("rustcraft.util.enchanter.view_new_name", "New enchantments", &[]),
        line("rustcraft.util.enchanter.view_new_hint", "The table's three offers for the item", &[]),
        st.view == View::New,
        false,
    ));
    out.push(button(
        line("rustcraft.util.enchanter.view_raise", "Own", &[]),
        line("rustcraft.util.enchanter.view_raise_name", "Its own enchantments", &[]),
        line(
            "rustcraft.util.enchanter.view_raise_hint",
            "+1 level to one the item has for more lapis and levels, or «×» to take one off",
            &[],
        ),
        raise_view,
        false,
    ));
    out.extend(remove_buttons(game, st, item, raise_view));
    out
}

/// EW15: the «×» beside each row of «Свои чары»: takes that enchantment off for a lapis.
fn remove_buttons(game: &Game<'_>, st: &Settings, item: Option<&ItemStack>, shown: bool) -> Vec<Button> {
    let rows = item.map_or_else(Vec::new, |i| enchanting::raise_rows(&have(i), |id| max_of(game, id)));
    (0..OPTION_ROWS)
        .map(|k| {
            let Some(r) = rows.get(k).filter(|_| shown) else {
                return Button { hidden: true, ..button(lit(""), lit(""), lit(""), false, true) };
            };
            let name = ench_name(game, &r.id, r.level);
            let curse = game.stations().enchantments.is_curse(&r.id);
            let hint = if curse {
                line("rustcraft.util.enchanter.remove_curse", "A curse does not come off", &[])
            } else {
                line(
                    "rustcraft.util.enchanter.remove_hint",
                    "Takes %s off for %s lapis, no experience; a book with none left is a plain book",
                    &[name.clone(), lit(enchanting::REMOVE_LAPIS)],
                )
            };
            let chosen = st.pick == Pick::Remove && st.raise.as_deref() == Some(r.id.as_str());
            button(
                lit("×"),
                line("rustcraft.util.enchanter.remove_name", "Take off %s", &[name]),
                hint,
                chosen,
                curse,
            )
        })
        .collect()
}

fn offer_buttons(game: &Game<'_>, st: &Settings, item: Option<&ItemStack>) -> Vec<Button> {
    let lists = item.map(|i| offers(game, st, i));
    let best = lists.as_ref().and_then(|l| enchanting::best(&l.clone().map(|x| as_levels(&x))));
    let best_text = match (&lists, best) {
        (Some(l), Some(k)) => line("rustcraft.util.enchanter.best_is", "Best\t%s", &[list_short(game, &l[k])]),
        _ => line("rustcraft.util.enchanter.best", "Best", &[]),
    };
    let mut out = vec![button(
        best_text,
        line("rustcraft.util.enchanter.best_name", "Best (hands-off)", &[]),
        line(
            "rustcraft.util.enchanter.best_hint",
            "The richest of the three; what comes by the magistral is enchanted so",
            &[],
        ),
        st.pick == Pick::Best,
        false,
    )];
    for k in 0..3 {
        let name = line("rustcraft.util.enchanter.offer", "Offer %s", &[lit(k + 1)]);
        let b = match &lists {
            Some(l) if !l[k].is_empty() => button(list_short(game, &l[k]), name, list_text(game, &l[k]), st.pick == Pick::Offer(k), false),
            Some(_) => button(
                lit("—"),
                name,
                line("rustcraft.util.enchanter.offer_none", "Nothing in this row for the item", &[]),
                st.pick == Pick::Offer(k),
                true,
            ),
            None => button(
                line("rustcraft.util.enchanter.offer_empty", "%s · put an item", &[lit(k + 1)]),
                name,
                line("rustcraft.util.enchanter.offer_wait", "Put an item to see the offers", &[]),
                st.pick == Pick::Offer(k),
                false,
            ),
        };
        out.push(b);
    }
    out
}

fn raise_buttons(game: &Game<'_>, st: &Settings, item: Option<&ItemStack>) -> Vec<Button> {
    let rows = item.map_or_else(Vec::new, |i| enchanting::raise_rows(&have(i), |id| max_of(game, id)));
    let anvil = |id: &str| game.stations().enchantments.get(id).map_or(1, |e| e.anvil_cost);
    let mut out: Vec<Button> = rows
        .iter()
        .map(|r| {
            let name = ench_name(game, &r.id, r.level);
            let chosen = st.pick == Pick::Raise && st.raise.as_deref() == Some(r.id.as_str());
            if r.at_max() {
                return button(
                    line("rustcraft.util.enchanter.raise_max", "%s\tmax.", &[name.clone()]),
                    name,
                    line("rustcraft.util.enchanter.raise_max_hint", "Already at its highest level", &[]),
                    chosen,
                    true,
                );
            }
            let price = enchanting::raise_price(anvil(&r.id), r.level + 1);
            button(
                line("rustcraft.util.enchanter.raise_row", "%s\t+1 · %s lv.", &[name.clone(), lit(price.levels)]),
                name,
                line(
                    "rustcraft.util.enchanter.raise_hint",
                    "Up to %s: %s levels and %s lapis",
                    &[ench_name(game, &r.id, r.level + 1), lit(price.levels), lit(price.lapis)],
                ),
                chosen,
                false,
            )
        })
        .collect();
    while out.len() < OPTION_ROWS {
        let hint = if item.is_some() {
            line("rustcraft.util.enchanter.raise_none", "The item has no more enchantments", &[])
        } else {
            line("rustcraft.util.enchanter.raise_wait", "Put an enchanted item or book", &[])
        };
        out.push(button(lit("—"), lit("—"), hint, false, true));
    }
    out
}

/// EW18: «Особые», the middle segment of the view switch; its hint says how many treasures the
/// item can take and, past four, that a press again shows the next four.
fn treasure_view_button(game: &Game<'_>, st: &Settings, item: Option<&ItemStack>) -> Button {
    let shown = st.view == View::Treasure;
    let n = item.map_or(0, |i| treasure_list(game, i).len());
    let pages = enchanting::treasure_pages(n);
    let hint = if shown && pages > 1 {
        line(
            "rustcraft.util.enchanter.view_treasure_page",
            "Page %s of %s (%s in all): click again for the next four",
            &[lit(st.page % pages + 1), lit(pages), lit(n)],
        )
    } else {
        line(
            "rustcraft.util.enchanter.view_treasure_hint",
            "Treasure enchantments the table never gives (Mending, Frost Walker…) at their highest, for more levels and %s lapis",
            &[lit(enchanting::TREASURE_LAPIS)],
        )
    };
    button(
        line("rustcraft.util.enchanter.view_treasure", "Special", &[]),
        line("rustcraft.util.enchanter.view_treasure_name", "Special enchantments", &[]),
        hint,
        shown,
        false,
    )
}

/// EW18–EW20: the rows of «Особые»: the treasures the item can take on the shown page, each at
/// its most with its price; the chosen one lit.
fn treasure_buttons(game: &Game<'_>, st: &Settings, item: Option<&ItemStack>) -> Vec<Button> {
    let list = item.map_or_else(Vec::new, |i| treasure_list(game, i));
    let pages = enchanting::treasure_pages(list.len());
    let mut out: Vec<Button> = enchanting::treasure_page(&list, st.page)
        .iter()
        .map(|(id, level)| {
            let name = ench_name(game, id, *level);
            let price = enchanting::treasure_price(anvil_of(game, id), *level);
            let hint = if pages > 1 {
                line(
                    "rustcraft.util.enchanter.treasure_hint_more",
                    "%s: %s levels and %s lapis. More on the next page: click «Special» again",
                    &[name.clone(), lit(price.levels), lit(price.lapis)],
                )
            } else {
                line(
                    "rustcraft.util.enchanter.treasure_hint",
                    "%s: %s levels and %s lapis; the table never gives it",
                    &[name.clone(), lit(price.levels), lit(price.lapis)],
                )
            };
            let chosen = st.pick == Pick::Treasure && st.raise.as_deref() == Some(id.as_str());
            button(
                line("rustcraft.util.enchanter.treasure_row", "%s\t%s lv.", &[name.clone(), lit(price.levels)]),
                name,
                hint,
                chosen,
                false,
            )
        })
        .collect();
    while out.len() < OPTION_ROWS {
        let hint = match item {
            None => line("rustcraft.util.enchanter.treasure_wait", "Put an item or a book", &[]),
            Some(_) if list.is_empty() => line(
                "rustcraft.util.enchanter.treasure_none",
                "No special enchantment goes on this item, or it has them all",
                &[],
            ),
            Some(_) => line("rustcraft.util.enchanter.treasure_no_more", "No more for this item", &[]),
        };
        out.push(button(lit("—"), lit("—"), hint, false, true));
    }
    out
}

/// The status line: what to do, in words (EW12), and what the last step made.
#[allow(clippy::too_many_arguments)]
fn status(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    st: &Settings,
    item: Option<&ItemStack>,
    tank: Option<u32>,
    need_points: u32,
    lapis: u32,
    to_storage: bool,
) -> SheetStatus {
    let shown = shown_at(dimension, pos).unwrap_or(Shown::Idle("enchant_empty"));
    let (severity, text) = match (&shown, item) {
        (_, None) => {
            let last = DONE
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&(dimension.to_owned(), pos))
                .filter(|(_, at)| at.elapsed() < DONE_SHOWN)
                .map(|(id, _)| id.clone());
            match last {
                // EW17: kept here, «Зачаровано: … — заберите внизу».
                Some(id) if to_storage && !st.keep => {
                    (Severity::Ok, line("rustcraft.util.enchanter.done_storage", "Enchanted: %s → storage", &[item_name(&id)]))
                }
                Some(id) => (Severity::Ok, line("rustcraft.util.enchanter.done", "Enchanted: %s — take it below", &[item_name(&id)])),
                None => (Severity::Warn, line("rustcraft.util.enchant_empty", "Put an item in the top slot", &[])),
            }
        }
        (Shown::Working, Some(i)) => (Severity::Ok, line("rustcraft.util.enchanter.working", "Enchanting: %s", &[item_name(&i.item)])),
        // An item put in since its last step: it starts on the next one.
        (Shown::Idle("enchant_empty"), Some(i)) => {
            (Severity::Ok, line("rustcraft.util.enchanter.waiting", "Starts on its next step: %s", &[item_name(&i.item)]))
        }
        (Shown::Blocked("enchant_no_lapis"), _) if to_storage => (
            Severity::Bad,
            line("rustcraft.util.enchanter.no_lapis_storage", "Needs %s lapis: none in the storage", &[lit(lapis)]),
        ),
        (Shown::Blocked("enchant_no_lapis"), _) => {
            (Severity::Bad, line("rustcraft.util.enchanter.no_lapis", "Needs %s lapis in the middle slot", &[lit(lapis)]))
        }
        (Shown::Blocked("enchant_no_xp"), _) => (
            Severity::Bad,
            line(
                "rustcraft.util.enchanter.no_xp",
                "Too few levels: %s of %s in the condenser",
                &[lit(xp::levels_from(0, tank.unwrap_or(0))), lit(xp::levels_from(0, need_points))],
            ),
        ),
        (Shown::Blocked("enchant_at_max"), _) => {
            let id = st.raise.clone().unwrap_or_default();
            let level = max_of(game, &id);
            (
                Severity::Bad,
                line("rustcraft.util.enchanter.at_max", "%s is at its highest: choose another", &[ench_name(game, &id, level)]),
            )
        }
        // EW19: the chosen treasure does not go on this item.
        (Shown::Blocked(k @ ("enchant_treasure_has" | "enchant_treasure_unfit")), _) => {
            let id = st.raise.clone().unwrap_or_default();
            let name = ench_name(game, &id, max_of(game, &id));
            let text = if *k == "enchant_treasure_has" {
                line("rustcraft.util.enchanter.treasure_has", "%s is on the item already: choose another", &[name])
            } else {
                line("rustcraft.util.enchanter.treasure_unfit", "%s does not go on this item: choose another in «Special»", &[name])
            };
            (Severity::Bad, text)
        }
        (Shown::Blocked(k), _) => (Severity::Bad, line(&format!("rustcraft.util.{k}"), k, &[])),
        (Shown::Idle(k), _) => (Severity::Warn, line(&format!("rustcraft.util.{k}"), k, &[])),
        (Shown::NoEnergy, _) => (Severity::Bad, line("rustcraft.util.no_energy", "No energy", &[])),
    };
    SheetStatus { severity: severity_code(severity), text: base::v(&text) }
}
