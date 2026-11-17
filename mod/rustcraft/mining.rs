// Slice 8's miner, block placer and block breaker on the server (spec 4.8 D1–D5, 18.4 MN1, 4.3;
// guide `miner`, `placer`, `breaker`). rustcraft_rules::mining decides what to scan, which ore to
// dig, which rock takes its place and when a block is due; this module reads the world, breaks and
// places, keeps the drops in the block's slots and hands them to a container beside it.
//
// - Breaking: `native_machine_destroy_block` (game/native_machines.rs:36) with the drops rolled for
//   Tuning's tool and collected; the miner then sets the rock (`native_set_block`).
// - Placing: `native_use_as_player` (game/native_machines.rs:102), a sneaking fake player
//   clicking the placer's front face with one block of its stack, so a block goes in as a player's would (its
//   facing, a sapling's soil, a crop's farmland).
// - Reports to Path-of-craft-tree (contract «break», «place»): the miner's `break … pickaxe` and
//   `place` of its rock, the placer's `place`; the breaker reports nothing (X7).
// - Owners: the block's `owner` (rustcraft_base::on_place_block, X2).
use crate::game::{
    Game, rustcraft_base as base,
    rustcraft_content::{self as content, Machine},
    rustcraft_machines::{self as machines, read_f64, read_int, write_f64, write_int},
    rustcraft_server::line,
    rustcraft_transport as transport,
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack, mod_api::NativeUseTarget, text::Text};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    machines::{
        sheet::{
            Bar as SheetBar, Kind as SheetKind, Sheet, Status as SheetStatus,
            Toggle as SheetToggle, severity_code,
        },
        style::Severity,
    },
    machines::slots::Slot,
    mining::{self as rules, Cycle, Miner, Ore, Outlet, Shown, Step, Survey},
    reports::Report,
    tuning::Tuning,
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const CURSOR_KEY: &str = "cursor";
const TARGET_KEY: &str = "target";
const DIG_KEY: &str = "dig";
const CARRY_KEY: &str = "carry";
const OFF_KEY: &str = "ores_off";
const PROGRESS_KEY: &str = "progress";
/// Slice 18: the ores a miner has dug («Найдено руды»).
const FOUND_KEY: &str = "found";
/// Cells of its square a miner's open window counts ahead each time it is looked at.
const SURVEY_BUDGET: u64 = 2048;
/// MN1: what a miner or a breaker holds goes to a container beside it this often, this many.
const PUSH_EVERY: u64 = 8;
const PUSH: u32 = 16;
/// Slice 18: into a chest beside it, as much as fits.
const PUSH_TO_CHEST: u32 = 27 * 64;

#[derive(Default)]
struct State {
    ticks: u64,
    shown: HashMap<(String, Pos), Shown>,
    /// Slice 18: where each miner and breaker puts what it dug (MN1), what it got and asked, E/s,
    /// and each miner's count of the ores ahead (its window's «Осталось»).
    outlets: HashMap<(String, Pos), Outlet>,
    energy: HashMap<(String, Pos), (f64, f64)>,
    surveys: HashMap<(String, Pos), Survey>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a miner, a placer or a breaker showed last, for its look, its window and the self-test.
pub(super) fn shown_at(dimension: &str, pos: Pos) -> Option<Shown> {
    state().shown.get(&(dimension.to_owned(), pos)).copied()
}

// ---- The saved compound ----

fn read_long(nbt: &Compound, key: &str) -> u64 {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Long(v)) => (*v).max(0) as u64,
        _ => 0,
    }
}

fn read_miner(nbt: &Compound) -> Miner {
    let target = match nbt.get(&JavaString::from(TARGET_KEY)) {
        Some(Tag::IntArray(p)) if p.len() == 3 => Some([p[0], p[1], p[2]]),
        _ => None,
    };
    Miner {
        cursor: read_long(nbt, CURSOR_KEY),
        target,
        dig: read_f64(nbt, DIG_KEY),
        carry: read_f64(nbt, CARRY_KEY),
    }
}

fn write_miner(nbt: &mut Compound, m: &Miner) {
    nbt.insert(JavaString::from(CURSOR_KEY), Tag::Long(m.cursor as i64));
    // No target: an empty array, which reads back as none.
    let target = m.target.map_or_else(Vec::new, |p| p.to_vec());
    nbt.insert(JavaString::from(TARGET_KEY), Tag::IntArray(target));
    write_f64(nbt, DIG_KEY, m.dig);
    write_f64(nbt, CARRY_KEY, m.carry);
}

pub(super) fn set_ores_off(game: &mut Game<'_>, pos: Pos, off: u16) -> bool {
    game.native_with_mod_block_entity(pos, |e| write_int(&mut e.nbt, OFF_KEY, i32::from(off)))
        .is_some()
}

fn front_of(game: &Game<'_>, pos: Pos) -> Option<(Face, Pos)> {
    let facing = game.native_block_property(pos, content::FACING)?;
    let face = rules::front(&facing)?;
    Some((face, step(pos, face)))
}

/// Slots `0..usable` of the block entity's items: whether one more stack of anything fits.
fn has_room(items: &[Option<ItemStack>], usable: usize) -> bool {
    (0..usable).any(|i| items.get(i).cloned().flatten().is_none_or(|s| s.count == 0))
}

// ---- The tick, from rustcraft_machines ----

/// A miner, a placer or a breaker as the machines' tick reads it.
pub struct Held {
    pub request: f64,
    work: f64,
    what: What,
}

enum What {
    Miner {
        m: Miner,
        total: u64,
        side: i32,
        min_y: i32,
        off: u16,
        scout: f64,
        usable: usize,
        hidden: usize,
        room: bool,
    },
    Placer {
        cycle: Cycle,
        slot: Option<usize>,
        free: bool,
        front: Option<(Face, Pos)>,
    },
    Breaker {
        cycle: Cycle,
        target: bool,
        room: bool,
        front: Option<Pos>,
    },
}

pub(super) fn read(game: &Game<'_>, pos: Pos, machine: Machine, t: &Tuning) -> Option<Held> {
    let entity = crate::game::rustcraft_machines::entity(game, pos)?;
    let slots = base::modules_at(game, pos);
    let mode = machines::read_work(&entity.nbt);
    let (work, energy) = rules::factors(mode, &slots, t);
    // Slice 18 (cross, A8): the placer and the breaker work faster on a higher tier, for as much
    // more energy, as the pump does.
    let tier = tier_speed(game, pos, machine, entity, t);
    let (work, energy) = (work * tier, energy * tier);
    let cycle = Cycle {
        progress: read_f64(&entity.nbt, PROGRESS_KEY),
    };
    let held = match machine {
        Machine::Miner => {
            let min_y = game.native_world_height().0;
            let side = rules::side(&slots, t);
            let total = rules::cells(pos, side, min_y);
            let m = read_miner(&entity.nbt);
            // Slice 18: past the window's 27 slots the miner keeps hidden cells.
            let (usable, hidden) = rules::split_slots(rules::slots_of(&slots, t));
            let room = has_room(&entity.items, usable)
                || rules::hidden_room(&read_cells(&entity.nbt), hidden);
            Held {
                request: m.request(total, room, energy, t),
                work,
                what: What::Miner {
                    m,
                    total,
                    side,
                    min_y,
                    off: read_int(&entity.nbt, OFF_KEY).map_or(0, |v| v as u16),
                    scout: rules::scout(&slots),
                    usable,
                    hidden,
                    room,
                },
            }
        }
        Machine::Placer => {
            let front = front_of(game, pos);
            let free = front.is_some_and(|(_, f)| rules::free(&game.native_block(f)));
            let slot = (0..content::BLOCK_SLOTS).find(|&i| {
                entity
                    .items
                    .get(i)
                    .cloned()
                    .flatten()
                    .is_some_and(|s| s.count > 0)
            });
            let busy = free && slot.is_some();
            Held {
                request: if busy { t.placer_energy * energy } else { 0.0 },
                work,
                what: What::Placer {
                    cycle,
                    slot,
                    free,
                    front,
                },
            }
        }
        _ => {
            let front = front_of(game, pos).map(|(_, f)| f);
            let target = front.is_some_and(|f| rules::breakable(&game.native_block(f)));
            let room = has_room(&entity.items, content::BLOCK_SLOTS);
            Held {
                request: if target && room {
                    t.breaker_energy * energy
                } else {
                    0.0
                },
                work,
                what: What::Breaker {
                    cycle,
                    target,
                    room,
                    front,
                },
            }
        }
    };
    Some(held)
}

/// Slice 18 (cross, A8): how much faster the placer or the breaker at `pos` works on its tier
/// than on its first; 1 for the miner, which has no tiers.
fn tier_speed(game: &Game<'_>, pos: Pos, machine: Machine, entity: &ModBlockEntity, t: &Tuning) -> f64 {
    match machine {
        Machine::Placer | Machine::Breaker => machines::fitted(game, pos, entity).speed(machine, t),
        _ => 1.0,
    }
}

/// Runs the block on `share` of its request.
pub(super) fn run(
    game: &mut Game<'_>,
    pos: Pos,
    machine: Machine,
    held: Held,
    share: f64,
    t: &Tuning,
) {
    let dimension = game.native_dimension().to_owned();
    let Held {
        request,
        work,
        what,
    } = held;
    let shown = match what {
        What::Miner {
            mut m,
            total,
            side,
            min_y,
            off,
            scout,
            usable,
            hidden,
            room,
        } => {
            let before = m.clone();
            let step = if request > 0.0 {
                let g: &Game<'_> = game;
                m.tick(
                    share,
                    work,
                    scout,
                    total,
                    t,
                    |i| rules::cell(pos, side, min_y, i),
                    |p| rules::wanted(&g.native_block(p), off),
                )
            } else {
                Step::Nothing
            };
            if let Step::Dig(p) = step {
                dig(game, &dimension, pos, p, off, (usable, hidden), t);
            }
            if m != before {
                game.native_with_mod_block_entity(pos, |e| write_miner(&mut e.nbt, &m));
            }
            if m.exhausted(total) {
                Shown::Exhausted
            } else if !room {
                Shown::Full
            } else {
                Shown::working(share)
            }
        }
        What::Placer {
            mut cycle,
            slot,
            free,
            front,
        } => match (slot, front) {
            (None, _) => Shown::NoBlocks,
            (_, None) => Shown::Taken,
            _ if !free => Shown::Taken,
            (Some(slot), Some((face, cell))) => {
                let before = cycle;
                if cycle.tick(share, work, t.placer_ticks) {
                    place(game, &dimension, pos, slot, face, cell);
                }
                save_progress(game, pos, before, cycle);
                Shown::working(share)
            }
        },
        What::Breaker {
            mut cycle,
            target,
            room,
            front,
        } => match front {
            Some(cell) if target && room => {
                let before = cycle;
                if cycle.tick(share, work, t.breaker_ticks) {
                    take(
                        game,
                        &dimension,
                        pos,
                        cell,
                        (content::BLOCK_SLOTS, 0),
                        t.breaker_tool.as_str(),
                    );
                }
                save_progress(game, pos, before, cycle);
                Shown::working(share)
            }
            _ if !room => Shown::Full,
            _ => Shown::Nothing,
        },
    };
    let push = {
        let mut s = state();
        s.ticks += 1;
        s.shown.insert((dimension.clone(), pos), shown);
        s.energy.insert((dimension.clone(), pos), (share * request, request));
        s.ticks.is_multiple_of(PUSH_EVERY)
    };
    if push && machine != Machine::Placer {
        let outlet = push_out(game, &dimension, pos);
        state().outlets.insert((dimension, pos), outlet);
        if machine == Machine::Miner {
            settle_cells(game, pos, t);
        }
    }
}

// ---- Slice 18: the miner's hidden cells (guide `mod_storage`: ×2 a module, past 27) ----

/// The miner's cells past its window's slots, its saved compound's key: (item, count) each.
pub(super) const CELLS_KEY: &str = "miner_cells";

fn read_cells(nbt: &Compound) -> Vec<Slot> {
    crate::game::rustcraft_utility::read_json(nbt, CELLS_KEY)
}

/// What the miner's hidden cells hold: stacks and items, for its window.
pub(super) fn hidden_counts(nbt: &Compound) -> (usize, u64) {
    read_cells(nbt)
        .iter()
        .flatten()
        .fold((0, 0), |(k, n), (_, c)| (k + 1, n + u64::from(*c)))
}

/// The hidden cells give into the window's slots as far as they take (rustcraft_rules::mining::
/// settle): what goes out (MN1) is taken from the slots, the cells refill them.
fn settle_cells(game: &mut Game<'_>, pos: Pos, t: &Tuning) {
    let Some(entity) = crate::game::rustcraft_machines::entity(game, pos) else {
        return;
    };
    let before = read_cells(&entity.nbt);
    if before.iter().all(Option::is_none) {
        return;
    }
    let (usable, hidden) = rules::split_slots(rules::slots_of(&base::modules_at(game, pos), t));
    let modules = base::module_items(game, pos);
    let plain = |s: &ItemStack| s.damage == 0 && s.extra.is_none() && s.count > 0;
    let visible_before: Vec<Slot> = (0..usable)
        .map(|i| {
            let st = entity.items.get(i).cloned().flatten();
            match st {
                Some(st) if plain(&st) => Some((st.item.to_string(), st.count)),
                // A stack with components (or anything odd) is not the cells' to merge onto: its
                // slot reads as taken by an item no cell holds.
                Some(st) if st.count > 0 => Some((String::from("!components"), st.count)),
                _ => None,
            }
        })
        .collect();
    let mut visible = visible_before.clone();
    let mut cells = before.clone();
    rules::settle(&mut visible, &mut cells, hidden, |i| rules::slot_max(&modules, machines::max_stack(game, i)));
    if cells == before {
        return;
    }
    game.native_with_mod_block_entity(pos, move |e| {
        if e.items.len() < usable {
            e.items.resize(usable, None);
        }
        for (i, (now, was)) in visible.iter().zip(&visible_before).enumerate() {
            if now != was && let Some((item, n)) = now {
                match e.items[i].as_mut() {
                    Some(st) if st.count > 0 && &*st.item == item.as_str() => st.count = *n,
                    _ => e.items[i] = Some(ItemStack::new(item, *n)),
                }
            }
        }
        crate::game::rustcraft_utility::write_json(&mut e.nbt, CELLS_KEY, &cells);
    });
}

fn save_progress(game: &mut Game<'_>, pos: Pos, before: Cycle, after: Cycle) {
    if before != after {
        game.native_with_mod_block_entity(pos, |e| {
            write_f64(&mut e.nbt, PROGRESS_KEY, after.progress)
        });
    }
}

/// D2: the ore at `p` comes out with its drops into the miner's slots, the rock round it in its
/// place; Path-of-craft-tree hears both.
fn dig(
    game: &mut Game<'_>,
    dimension: &str,
    miner: Pos,
    p: Pos,
    off: u16,
    usable: (usize, usize),
    t: &Tuning,
) {
    let ore = game.native_block(p);
    // A player may have taken it meanwhile.
    if !rules::wanted(&ore, off) {
        return;
    }
    let around: Vec<String> = Face::ALL
        .iter()
        .map(|&f| game.native_block(step(p, f)))
        .collect();
    let rock = rules::rock_for(&ore, &around);
    if !take(game, dimension, miner, p, usable, &t.miner_tool) {
        return;
    }
    if let Some(state) = game.native_default_state(rock) {
        game.native_set_block(dimension, p, state, 3);
    }
    // Slice 18: one more found, one fewer ahead.
    game.native_with_mod_block_entity(miner, |e| {
        let n = read_long(&e.nbt, FOUND_KEY) + 1;
        e.nbt.insert(JavaString::from(FOUND_KEY), Tag::Long(n as i64));
    });
    if let Some(s) = self::state().surveys.get_mut(&(dimension.to_owned(), miner)) {
        s.dug();
    }
    if let Some(owner) = base::owner_at(game, miner) {
        base::report(Report::Break {
            owner,
            dimension: dimension.to_owned(),
            pos: p,
            block: ore,
            tool: "pickaxe",
        });
    }
    base::report(Report::Place {
        dimension: dimension.to_owned(),
        pos: p,
        block: rock.to_owned(),
    });
}

/// Breaks `cell` with `tool`, the drops into slots `0..usable.0` of the block at `at`, then (slice
/// 18, the miner) into its first `usable.1` hidden cells (what does not fit drops on top of it);
/// whether it broke.
fn take(
    game: &mut Game<'_>,
    dimension: &str,
    at: Pos,
    cell: Pos,
    usable: (usize, usize),
    tool: &str,
) -> bool {
    let (usable, hidden) = usable;
    let tool = ItemStack::new(tool, 1);
    let Some(drops) = game.native_machine_destroy_block(cell, Some(&tool), true) else {
        return false;
    };
    crate::game::rustcraft_utility::forget_grown(game, dimension, cell);
    // Slice 18: a stack module makes the slots hold four times as much (guide `mod_stack`).
    let modules = base::module_items(game, at);
    let maxes: Vec<u32> = drops
        .iter()
        .map(|d| rules::slot_max(&modules, machines::max_stack(game, &d.item)))
        .collect();
    let left: Vec<ItemStack> = game
        .native_with_mod_block_entity(at, |e| {
            if e.items.len() < usable {
                e.items.resize(usable, None);
            }
            drops
                .into_iter()
                .zip(maxes)
                .filter_map(|(d, max)| store(&mut e.items[..usable], d, max))
                .collect()
        })
        .unwrap_or_default();
    // Slice 18: what the slots do not take goes into the hidden cells (plain stacks only).
    let left = if hidden > 0 && !left.is_empty() {
        let sizes: Vec<u32> = left.iter().map(|l| rules::slot_max(&modules, machines::max_stack(game, &l.item))).collect();
        game.native_with_mod_block_entity(at, |e| {
            let mut cells = read_cells(&e.nbt);
            let rest: Vec<ItemStack> = left
                .into_iter()
                .zip(sizes)
                .filter_map(|(mut l, size)| {
                    if l.damage == 0 && l.extra.is_none() {
                        l.count = rules::store_hidden(&mut cells, hidden, &l.item, l.count, size);
                    }
                    (l.count > 0).then_some(l)
                })
                .collect();
            crate::game::rustcraft_utility::write_json(&mut e.nbt, CELLS_KEY, &cells);
            rest
        })
        .unwrap_or_default()
    } else {
        left
    };
    for l in left {
        drop_stack(game, dimension, [at[0], at[1] + 1, at[2]], &l);
    }
    true
}

/// Puts `stack` into `slots`, merging first; what is left.
pub(super) fn store(slots: &mut [Option<ItemStack>], mut stack: ItemStack, max: u32) -> Option<ItemStack> {
    if stack.count == 0 || &*stack.item == "minecraft:air" {
        return None;
    }
    for s in slots.iter_mut().flatten() {
        if s.count > 0 && s.item == stack.item && s.count < max {
            let n = (max - s.count).min(stack.count);
            s.count += n;
            stack.count -= n;
            if stack.count == 0 {
                return None;
            }
        }
    }
    for s in slots.iter_mut() {
        if s.as_ref().is_none_or(|x| x.count == 0) {
            let n = stack.count.min(max);
            let mut put = stack.clone();
            put.count = n;
            *s = Some(put);
            stack.count -= n;
            if stack.count == 0 {
                return None;
            }
        }
    }
    Some(stack)
}

fn drop_stack(game: &mut Game<'_>, dimension: &str, pos: Pos, stack: &ItemStack) {
    let item = Tag::compound([
        ("id", Tag::String(JavaString::from(&*stack.item))),
        ("count", Tag::Int(stack.count.min(99) as i32)),
    ]);
    let nbt = Compound::from([(JavaString::from("Item"), item)]);
    let at = [
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + 0.2,
        f64::from(pos[2]) + 0.5,
    ];
    if game
        .native_summon(dimension, "minecraft:item", at, nbt)
        .is_none()
    {
        log::warn!("rustcraft: {} {} lost at {pos:?}", stack.count, stack.item);
    }
}

/// The placer puts one block of slot `slot` into `cell`, as a player clicking its front face.
fn place(game: &mut Game<'_>, dimension: &str, pos: Pos, slot: usize, face: Face, cell: Pos) {
    let Some(stack) = crate::game::rustcraft_machines::entity(game, pos)
        .and_then(|e| e.items.get(slot).cloned().flatten())
    else {
        return;
    };
    let o = face.offset();
    let center = pos.map(|c| f64::from(c) + 0.5);
    let point = [
        center[0] + 0.5 * f64::from(o[0]),
        center[1],
        center[2] + 0.5 * f64::from(o[2]),
    ];
    // The player looks along the front: yaw 0 south, 90 west, 180 north, -90 east.
    let yaw = match face {
        Face::ZPos => 0.0,
        Face::XNeg => 90.0,
        Face::ZNeg => 180.0,
        _ => -90.0,
    };
    let one = {
        let mut s = stack.clone();
        s.count = 1;
        s
    };
    let target = NativeUseTarget::Block {
        pos,
        face: transport::direction(face),
        point,
    };
    // Sneaking: since slice 9 the placer copies the furnace, whose use opens its window; a
    // sneaking player's click with a block in hand skips the clicked block's use and places.
    let Some(used) = game.native_use_as_player(one, center, yaw, 0.0, target, true) else {
        return;
    };
    let placed = game.native_block(cell);
    if !used.consumed || rules::free(&placed) {
        return;
    }
    game.native_with_mod_block_entity(pos, |e| {
        if let Some(Some(s)) = e.items.get_mut(slot) {
            s.count = s.count.saturating_sub(1);
            if s.count == 0 {
                e.items[slot] = None;
            }
        }
    });
    base::report(Report::Place {
        dimension: dimension.to_owned(),
        pos: cell,
        block: placed,
    });
}

/// MN1 (slice 18: the guide's order): a miner's or a breaker's slots go to a chest beside it; with
/// none, the magistral takes from them as from any machine's products (a housing beside it); with
/// neither, they go into a load dock beside it for a cargo worker. Where they go now.
fn push_out(game: &mut Game<'_>, dimension: &str, pos: Pos) -> Outlet {
    let mut chests = Vec::new();
    let mut docks = Vec::new();
    let mut magistral = false;
    for f in Face::ALL {
        let block = game.native_block(step(pos, f));
        if rules::holds_items(&block) {
            chests.push(f);
        } else if block == content::DOCK_LOAD {
            docks.push(f);
        } else if block == content::HOUSING {
            magistral = true;
        }
    }
    let outlet = Outlet::of(!chests.is_empty(), magistral, !docks.is_empty());
    let (faces, max) = match outlet {
        // The chest first: it takes all it can, before the magistral's next pull.
        Outlet::Chest => (chests, PUSH_TO_CHEST),
        Outlet::LoadDock => (docks, PUSH),
        _ => (Vec::new(), 0),
    };
    for f in faces {
        game.native_move_items(
            dimension,
            pos,
            transport::direction(f),
            step(pos, f),
            transport::direction(f.opposite()),
            None,
            max,
        );
    }
    outlet
}

/// Where the miner or breaker at `pos` put what it dug last (MN1), for its window.
pub(super) fn outlet_at(dimension: &str, pos: Pos) -> Option<Outlet> {
    state().outlets.get(&(dimension.to_owned(), pos)).copied()
}

// ---- What they show ----

fn shown_text(s: Shown) -> Text {
    match s {
        Shown::Working => line("rustcraft.machine.working", "Working", &[]),
        Shown::Slowed(p) => line(
            "rustcraft.machine.slowed",
            "Not enough energy: %s",
            &[Text::literal(format!("{p}%"))],
        ),
        Shown::NoEnergy => line("rustcraft.machine.no_energy", "No energy", &[]),
        Shown::Exhausted => line("rustcraft.miner.exhausted", "The area is worked out", &[]),
        Shown::Full => line("rustcraft.miner.full", "Nowhere to put what it dug", &[]),
        Shown::NoBlocks => line("rustcraft.placer.no_blocks", "No blocks", &[]),
        Shown::Taken => line("rustcraft.placer.taken", "The cell is taken", &[]),
        Shown::Nothing => line("rustcraft.breaker.nothing", "Nothing to break", &[]),
    }
}

/// The look of a miner, a placer or a breaker.
pub(super) fn diagnosis(dimension: &str, pos: Pos, block: &str) -> Option<Text> {
    let m = Machine::of(block)?;
    if !matches!(m, Machine::Miner | Machine::Placer | Machine::Breaker) {
        return None;
    }
    Some(shown_at(dimension, pos).map_or_else(
        || Text::translate(&format!("block.{}", block.replace(':', ".")), &[]),
        shown_text,
    ))
}

// ---- Windows: sheets (rustcraft_sheets, rustcraft_base::sheet) ----

/// A press on ore toggle `i` of the miner's window (D5): that ore taken or left.
pub(super) fn toggle_ore(nbt: &mut Compound, i: usize) {
    let Some(ore) = Ore::LISTED.get(i) else {
        return;
    };
    let off = read_int(nbt, OFF_KEY).map_or(0, |v| v as u16) ^ ore.bit();
    write_int(nbt, OFF_KEY, i32::from(off));
}

/// What the window of a miner, a placer or a breaker shows: its server menu's cells, part by
/// part. The modules and the mode are the sheet's own parts; the slots are the menu's.
pub(super) fn sheet(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    kind: SheetKind,
    t: &Tuning,
    s: &mut Sheet,
) {
    let Some(entity) = crate::game::rustcraft_machines::entity(game, pos) else {
        return;
    };
    let slots = base::modules_at(game, pos);
    s.mode = Some(machines::read_work(&entity.nbt).code());
    if kind == SheetKind::Miner {
        let min_y = game.native_world_height().0;
        let side = rules::side(&slots, t);
        let total = rules::cells(pos, side, min_y);
        let m = read_miner(&entity.nbt);
        let layer = m.cursor / (side as u64).pow(2);
        let size = Text::literal(format!("{side}×{side}"));
        let off = read_int(&entity.nbt, OFF_KEY).map_or(0, |v| v as u16);
        // Slice 18 (ui.js `miner`): «Найдено руды» and «Осталось», counted ahead a little each
        // time the window is looked at.
        let left = survey(game, dimension, pos, &m, side, min_y, off, total);
        let found = read_long(&entity.nbt, FOUND_KEY);
        let left_text = match left {
            Some(n) => Text::literal(n.to_string()),
            None => line("rustcraft.miner.counting", "counting…", &[]),
        };
        s.rows.push(base::v(&line(
            "rustcraft.miner.found_left",
            "Ore found %s · left %s",
            &[Text::literal(found.to_string()), left_text],
        )));
        // «Куда складывает»: the guide's three places, the one it uses now marked.
        let now = outlet_at(dimension, pos);
        let mut places = line("rustcraft.miner.outlet", "Puts it: ", &[]);
        for (i, o) in Outlet::ORDER.into_iter().enumerate() {
            if i > 0 {
                places.push(Text::literal(" · "));
            }
            let name = line(&format!("rustcraft.miner.outlet.{}", o.key()), o.key(), &[]);
            if now == Some(o) {
                places.push(Text::literal("["));
                places.push(name);
                places.push(Text::literal("]"));
            } else {
                places.push(name);
            }
        }
        s.rows.push(base::v(&places));
        s.hints = vec![
            vec![
                base::v(&line(
                    "rustcraft.miner.area",
                    "Area %s down to y %s",
                    &[size, Text::literal(min_y.to_string())],
                )),
                base::v(&line(
                    "rustcraft.miner.scanned",
                    "Scanned %s of %s cells, layer y %s",
                    &[
                        Text::literal(m.cursor.min(total).to_string()),
                        Text::literal(total.to_string()),
                        Text::literal((pos[1] - 1 - layer as i32).max(min_y).to_string()),
                    ],
                )),
                base::v(&line(
                    "rustcraft.miner.area_hint",
                    "An area module: 22×22, two: 33×33",
                    &[],
                )),
            ],
            vec![base::v(&line(
                "rustcraft.miner.outlet_hint",
                "A chest beside it first, else the magistral, else a load dock",
                &[],
            ))],
        ];
        s.toggles = Ore::LISTED
            .iter()
            .map(|ore| {
                let on = ore.bit() & off == 0;
                SheetToggle {
                    icon: ore.icon().to_owned(),
                    name: base::v(&line(
                        &format!("rustcraft.ore.{}", ore.key()),
                        ore.key(),
                        &[],
                    )),
                    on,
                    hint: base::v(&if on {
                        line("rustcraft.miner.ore_on", "Taken · click: leave it", &[])
                    } else {
                        line(
                            "rustcraft.miner.ore_off",
                            "Left in the ground · click: take it",
                            &[],
                        )
                    }),
                }
            })
            .collect();
        // Slice 18: the window's slots, and the hidden cells past them counted.
        let (shown_slots, hidden) = rules::split_slots(rules::slots_of(&slots, t));
        s.usable = u8::try_from(shown_slots).ok();
        if hidden > 0 {
            let (stacks, items) = hidden_counts(&entity.nbt);
            s.rows.push(base::v(&line(
                "rustcraft.miner.hidden",
                "In reserve past the window: %s items in %s of %s cells",
                &[
                    Text::literal(items.to_string()),
                    Text::literal(stacks.to_string()),
                    Text::literal(hidden.to_string()),
                ],
            )));
        }
    } else {
        let (title, energy, ticks) = if kind == SheetKind::Placer {
            (
                line(
                    "rustcraft.placer.info",
                    "Places the block in front of it",
                    &[],
                ),
                t.placer_energy,
                t.placer_ticks,
            )
        } else {
            (
                line(
                    "rustcraft.breaker.info",
                    "Breaks the block in front of it",
                    &[],
                ),
                t.breaker_energy,
                t.breaker_ticks,
            )
        };
        // Slice 18 (cross): the pace on its tier, mode and modules, as it works.
        let machine = if kind == SheetKind::Placer { Machine::Placer } else { Machine::Breaker };
        let mode = machines::read_work(&entity.nbt);
        let (work, draw) = rules::factors(mode, &slots, t);
        let tier = tier_speed(game, pos, machine, entity, t);
        let secs = ticks / (work * tier).max(0.01) / t.ticks_per_second;
        let energy = energy * draw * tier;
        let secs = format!("{secs:.1}").trim_end_matches(".0").replace('.', ",");
        s.rows.push(base::v(&title));
        s.rows.push(base::v(&line(
            "rustcraft.placer.pace",
            "A block in %s s, %s E/s",
            &[
                Text::literal(secs),
                Text::literal(format!("{energy:.0}")),
            ],
        )));
    }
    let shown = shown_at(dimension, pos);
    let severity = match shown {
        Some(Shown::Working) => Severity::Ok,
        Some(Shown::NoEnergy) => Severity::Bad,
        _ => Severity::Warn,
    };
    let mut text = shown.map_or_else(|| Text::literal("…"), shown_text);
    // Slice 18 (ui.js `energy`): what it gets of what it asks.
    if let Some((got, asked)) = state().energy.get(&(dimension.to_owned(), pos)).copied()
        && asked > 0.0
    {
        text.push(Text::literal(" · "));
        text.push(line(
            "rustcraft.miner.energy",
            "%s of %s E/s",
            &[
                Text::literal(format!("{got:.0}")),
                Text::literal(format!("{asked:.0}")),
            ],
        ));
    }
    s.status = Some(SheetStatus {
        severity: severity_code(severity),
        text: base::v(&text),
    });
}

/// The miner's count of the wanted ores ahead of its scan, `SURVEY_BUDGET` more cells a call;
/// none while it still counts.
#[allow(clippy::too_many_arguments)]
fn survey(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    m: &Miner,
    side: i32,
    min_y: i32,
    off: u16,
    total: u64,
) -> Option<u64> {
    let key = (dimension.to_owned(), pos);
    let mut s = state()
        .surveys
        .get(&key)
        .copied()
        .filter(|s| s.fits(off, total))
        .unwrap_or_else(|| Survey::new(m.cursor, off, total));
    if !s.done {
        s.step(SURVEY_BUDGET, |i| {
            rules::cell(pos, side, min_y, i).is_some_and(|p| rules::wanted(&game.native_block(p), off))
        });
    }
    state().surveys.insert(key, s);
    s.done.then_some(s.ores)
}

// ---- Slice 18: the living vein's mode and window (guide `vein`, ui.js `vein`) ----

const GROWN_KEY: &str = "grown";

/// The vein's step and cost under its work mode.
pub(super) fn vein_paced(game: &Game<'_>, pos: Pos, ticks: u64, cost: f64) -> (u64, f64) {
    let mode = crate::game::rustcraft_machines::entity(game, pos)
        .map(|e| machines::read_work(&e.nbt))
        .unwrap_or_default();
    rules::vein_paced(mode, ticks, cost)
}

/// The vein grew a block: one more for «Выращено всего».
pub(super) fn vein_grew(game: &mut Game<'_>, pos: Pos) {
    game.native_with_mod_block_entity(pos, |e| {
        let n = read_long(&e.nbt, GROWN_KEY) + 1;
        e.nbt.insert(JavaString::from(GROWN_KEY), Tag::Long(n as i64));
    });
}

/// ui.js `vein`: its mode, «Ход работы», «Свободных клеток вокруг» and «Выращено всего» (what it
/// does and its pace go to the rows' hints).
pub(super) fn vein_sheet(game: &Game<'_>, pos: Pos, now: u64, every: u64, s: &mut Sheet) {
    let Some(entity) = crate::game::rustcraft_machines::entity(game, pos) else {
        return;
    };
    s.mode = Some(machines::read_work(&entity.nbt).code());
    let free = rustcraft_rules::utility::vein::cells()
        .into_iter()
        .map(|c| [pos[0] + c[0], pos[1] + c[1], pos[2] + c[2]])
        .filter(|p| matches!(game.native_block(*p).as_str(), "minecraft:air" | "minecraft:cave_air"))
        .count();
    let old_rows = std::mem::take(&mut s.rows);
    s.rows.push(base::v(&line(
        "rustcraft.vein.free",
        "Free cells around: %s",
        &[Text::literal(free.to_string())],
    )));
    s.rows.push(base::v(&line(
        "rustcraft.vein.grown",
        "Grown in all: %s blocks",
        &[Text::literal(read_long(&entity.nbt, GROWN_KEY).to_string())],
    )));
    s.hints = vec![old_rows.clone(), old_rows];
    s.bars.push(SheetBar {
        label: base::v(&line("rustcraft.vein.progress", "Progress", &[])),
        value: base::v(&Text::literal(format!(
            "{:.0}%",
            rules::vein_progress(now, every) * 100.0
        ))),
        share: rules::vein_progress(now, every),
        energy: false,
        colour: None,
    });
}

/// The self-test's plot cleared: what it showed goes.
pub(super) fn forget_area(dimension: &str, min: Pos, max: Pos) {
    let inside = |p: Pos| (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
    let mut s = state();
    s.shown.retain(|(d, p), _| d != dimension || !inside(*p));
    s.outlets.retain(|(d, p), _| d != dimension || !inside(*p));
    s.energy.retain(|(d, p), _| d != dimension || !inside(*p));
    s.surveys.retain(|(d, p), _| d != dimension || !inside(*p));
}
