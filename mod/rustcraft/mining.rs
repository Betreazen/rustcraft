// Slice 8's miner, block placer and block breaker on the server (spec 4.8 D1–D5, 18.4 MN1, 4.3;
// guide `miner`, `placer`, `breaker`). rustcraft_rules::mining decides what to scan, which ore to
// dig, which rock takes its place and when a block is due; this module reads the world, breaks and
// places, keeps the drops in the block's slots and hands them to a container beside it.
//
// - Breaking: `native_machine_destroy_block` (game/native_machines.rs:36) with the drops rolled for
//   Tuning's tool and collected; the miner then sets the rock (`native_set_block`).
// - Placing: `native_use_as_player` (game/native_machines.rs:102), a fake player clicking the
//   placer's front face with one block of its stack, so a block goes in as a player's would (its
//   facing, a sapling's soil, a crop's farmland).
// - Reports to Path-of-craft-tree (contract «break», «place»): the miner's `break … pickaxe` and
//   `place` of its rock, the placer's `place`; the breaker reports nothing (X7).
// - Owners: the block's `owner` (rustcraft_base::on_place_block, X2).
use crate::game::{
    Game,
    native_inventories::NativeInventoryKind,
    rustcraft_base as base,
    rustcraft_content::{self as content, Machine},
    rustcraft_machines::{self as machines, read_f64, read_int, write_f64, write_int},
    rustcraft_server::{self as server, line},
    rustcraft_transport as transport,
    rustcraft_window::named,
};
use horizon_client::{items::ItemStack, mod_api::NativeUseTarget, text::Text};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    machines::processor::WorkMode,
    mining::{self as rules, Cycle, Miner, Ore, Shown, Step},
    modules::Holder,
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
/// MN1: what a miner or a breaker holds goes to a container beside it this often, this many.
const PUSH_EVERY: u64 = 8;
const PUSH: u32 = 16;
const MINER_ROWS: u8 = 5;
const BLOCK_ROWS: u8 = 2;
/// The cells of a window's first row.
const MODE_CELL: usize = 3;
const INFO_CELL: usize = 4;
const STATUS_CELL: usize = 8;

#[derive(Default)]
struct State {
    ticks: u64,
    shown: HashMap<(String, Pos), Shown>,
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

/// D5: the ores switched off in the miner's window.
pub(super) fn ores_off(game: &Game<'_>, pos: Pos) -> u16 {
    game.native_mod_block_entity(pos)
        .and_then(|e| read_int(&e.nbt, OFF_KEY))
        .map_or(0, |v| v as u16)
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
    let entity = game.native_mod_block_entity(pos)?;
    let slots = base::modules_at(game, pos);
    let mode = machines::read_work(&entity.nbt);
    let (work, energy) = rules::factors(mode, &slots, t);
    let cycle = Cycle {
        progress: read_f64(&entity.nbt, PROGRESS_KEY),
    };
    let held = match machine {
        Machine::Miner => {
            let min_y = game.native_world_height().0;
            let side = rules::side(&slots, t);
            let total = rules::cells(pos, side, min_y);
            let m = read_miner(&entity.nbt);
            let usable = rules::slots_of(&slots, t);
            let room = has_room(&entity.items, usable);
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
                dig(game, &dimension, pos, p, off, usable, t);
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
                        content::BLOCK_SLOTS,
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
        s.ticks.is_multiple_of(PUSH_EVERY)
    };
    if push && machine != Machine::Placer {
        push_out(game, &dimension, pos);
    }
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
    usable: usize,
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

/// Breaks `cell` with `tool`, the drops into slots `0..usable` of the block at `at` (what does
/// not fit drops on top of it); whether it broke.
fn take(
    game: &mut Game<'_>,
    dimension: &str,
    at: Pos,
    cell: Pos,
    usable: usize,
    tool: &str,
) -> bool {
    let tool = ItemStack::new(tool, 1);
    let Some(drops) = game.native_machine_destroy_block(cell, Some(&tool), true) else {
        return false;
    };
    let maxes: Vec<u32> = drops
        .iter()
        .map(|d| machines::max_stack(game, &d.item))
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
    for l in left {
        drop_stack(game, dimension, [at[0], at[1] + 1, at[2]], &l);
    }
    true
}

/// Puts `stack` into `slots`, merging first; what is left.
fn store(slots: &mut [Option<ItemStack>], mut stack: ItemStack, max: u32) -> Option<ItemStack> {
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
    let Some(stack) = game
        .native_mod_block_entity(pos)
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
    let Some(used) = game.native_use_as_player(one, center, yaw, 0.0, target, false) else {
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

/// MN1: a miner's or a breaker's slots go to a container beside it; the magistral takes from them
/// as from any machine's products.
fn push_out(game: &mut Game<'_>, dimension: &str, pos: Pos) {
    for f in Face::ALL {
        let n = step(pos, f);
        let block = game.native_block(n);
        if block == "minecraft:air" || server::is_ours(&block) {
            continue;
        }
        game.native_move_items(
            dimension,
            pos,
            transport::direction(f),
            n,
            transport::direction(f.opposite()),
            None,
            PUSH,
        );
    }
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

// ---- Windows (rustcraft_base keeps the sessions) ----

pub(super) fn rows(machine: Machine) -> u8 {
    if machine == Machine::Miner {
        MINER_ROWS
    } else {
        BLOCK_ROWS
    }
}

fn mode_name(mode: WorkMode) -> Text {
    match mode {
        WorkMode::Economy => line("rustcraft.work.economy", "Economy", &[]),
        WorkMode::Normal => line("rustcraft.work.normal", "Normal", &[]),
        WorkMode::Boost => line("rustcraft.work.boost", "Boost", &[]),
    }
}

pub(super) fn cells(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    machine: Machine,
    t: &Tuning,
) -> Vec<Option<ItemStack>> {
    let mut out: Vec<Option<ItemStack>> = vec![None; usize::from(rows(machine)) * 9];
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return out;
    };
    let slots = base::modules_at(game, pos);
    base::module_cells(&slots, &mut out);
    let mode = machines::read_work(&entity.nbt);
    out[MODE_CELL] = Some(named(
        "minecraft:clock",
        1,
        line("rustcraft.machine.mode_now", "Mode: %s", &[mode_name(mode)]),
        &[line(
            "rustcraft.machine.mode_next",
            "Click: the next mode",
            &[],
        )],
        false,
    ));
    let first = 9;
    match machine {
        Machine::Miner => {
            let min_y = game.native_world_height().0;
            let side = rules::side(&slots, t);
            let total = rules::cells(pos, side, min_y);
            let m = read_miner(&entity.nbt);
            let layer = m.cursor / (side as u64).pow(2);
            let size = Text::literal(format!("{side}×{side}"));
            out[INFO_CELL] = Some(named(
                "minecraft:filled_map",
                1,
                line(
                    "rustcraft.miner.area",
                    "Area %s down to y %s",
                    &[size, Text::literal(min_y.to_string())],
                ),
                &[
                    line(
                        "rustcraft.miner.scanned",
                        "Scanned %s of %s cells, layer y %s",
                        &[
                            Text::literal(m.cursor.min(total).to_string()),
                            Text::literal(total.to_string()),
                            Text::literal((pos[1] - 1 - layer as i32).max(min_y).to_string()),
                        ],
                    ),
                    line(
                        "rustcraft.miner.area_hint",
                        "An area module: 22×22, two: 33×33",
                        &[],
                    ),
                ],
                false,
            ));
            let off = read_int(&entity.nbt, OFF_KEY).map_or(0, |v| v as u16);
            for (i, ore) in Ore::LISTED.iter().enumerate() {
                let on = ore.bit() & off == 0;
                out[first + i] = Some(named(
                    ore.icon(),
                    1,
                    line(&format!("rustcraft.ore.{}", ore.key()), ore.key(), &[]),
                    &[if on {
                        line("rustcraft.miner.ore_on", "Taken · click: leave it", &[])
                    } else {
                        line(
                            "rustcraft.miner.ore_off",
                            "Left in the ground · click: take it",
                            &[],
                        )
                    }],
                    on,
                ));
            }
            let usable = rules::slots_of(&slots, t);
            for i in 0..content::MINER_SLOTS {
                out[first + 9 + i] = if i < usable {
                    entity
                        .items
                        .get(i)
                        .cloned()
                        .flatten()
                        .filter(|s| s.count > 0)
                } else {
                    Some(named(
                        "minecraft:gray_stained_glass_pane",
                        1,
                        line(
                            "rustcraft.miner.more_slots",
                            "More slots with a storage module",
                            &[],
                        ),
                        &[],
                        false,
                    ))
                };
            }
        }
        _ => {
            let (item, title, energy, ticks) = if machine == Machine::Placer {
                (
                    "minecraft:dropper",
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
                    "minecraft:iron_pickaxe",
                    line(
                        "rustcraft.breaker.info",
                        "Breaks the block in front of it",
                        &[],
                    ),
                    t.breaker_energy,
                    t.breaker_ticks,
                )
            };
            let secs = ticks / t.ticks_per_second;
            out[INFO_CELL] = Some(named(
                item,
                1,
                title,
                &[line(
                    "rustcraft.placer.pace",
                    "A block in %s s, %s E/s",
                    &[
                        Text::literal(format!("{secs:.0}")),
                        Text::literal(format!("{energy:.0}")),
                    ],
                )],
                false,
            ));
            for i in 0..content::BLOCK_SLOTS {
                out[first + i] = entity
                    .items
                    .get(i)
                    .cloned()
                    .flatten()
                    .filter(|s| s.count > 0);
            }
        }
    }
    let shown = shown_at(dimension, pos);
    let good = matches!(shown, Some(Shown::Working | Shown::Slowed(_)));
    out[STATUS_CELL] = Some(named(
        if good {
            "minecraft:lime_dye"
        } else {
            "minecraft:red_dye"
        },
        1,
        shown.map_or_else(|| Text::literal("…"), shown_text),
        &[],
        false,
    ));
    out
}

/// A click on a cell of the window (`slot` in the menu, the player's inventory after its cells).
pub(super) fn click(game: &mut Game<'_>, uuid: u128, pos: Pos, machine: Machine, slot: usize) {
    let size = usize::from(rows(machine)) * 9;
    if slot == MODE_CELL {
        game.native_with_mod_block_entity(pos, |e| {
            let next = match machines::read_work(&e.nbt) {
                WorkMode::Economy => WorkMode::Normal,
                WorkMode::Normal => WorkMode::Boost,
                WorkMode::Boost => WorkMode::Economy,
            };
            machines::write_work(&mut e.nbt, next);
        });
        return;
    }
    if slot < 3 {
        base::take_module(game, uuid, pos, slot);
        return;
    }
    if slot >= size {
        let n = slot - size;
        let inventory = match n {
            0..27 => n + 9,
            27..36 => n - 27,
            _ => return,
        };
        let holder = if machine == Machine::Miner {
            Holder::Miner
        } else {
            Holder::Placer
        };
        if base::put_module(game, uuid, pos, inventory, holder) || machine != Machine::Placer {
            return;
        }
        put_blocks(game, uuid, pos, inventory);
        return;
    }
    let first = 9;
    let store_from = if machine == Machine::Miner {
        if (first..first + 9).contains(&slot) {
            let ore = Ore::LISTED[slot - first];
            let off = ores_off(game, pos) ^ ore.bit();
            set_ores_off(game, pos, off);
            return;
        }
        first + 9
    } else {
        first
    };
    let Some(i) = slot.checked_sub(store_from) else {
        return;
    };
    let stack = game
        .native_mod_block_entity(pos)
        .and_then(|e| e.items.get(i).cloned().flatten());
    if let Some(stack) = stack.filter(|s| s.count > 0) {
        let left = game.native_give_stack(uuid, stack);
        game.native_with_mod_block_entity(pos, |e| {
            if let Some(s) = e.items.get_mut(i) {
                *s = left;
            }
        });
    }
}

/// A block stack of the player's inventory goes into the placer.
fn put_blocks(game: &mut Game<'_>, uuid: u128, pos: Pos, inventory: usize) {
    let Some(stack) = game
        .native_player_stacks(uuid)
        .and_then(|s| s.get(inventory).cloned().flatten())
    else {
        return;
    };
    let placeable = game
        .items()
        .get(&stack.item)
        .is_some_and(|i| i.block.is_some());
    if !placeable {
        return;
    }
    let max = machines::max_stack(game, &stack.item);
    let Some(left) = game.native_with_mod_block_entity(pos, |e| {
        if e.items.len() < content::BLOCK_SLOTS {
            e.items.resize(content::BLOCK_SLOTS, None);
        }
        store(&mut e.items[..content::BLOCK_SLOTS], stack.clone(), max)
    }) else {
        return;
    };
    if left.as_ref().map(|l| l.count) == Some(stack.count) {
        return;
    }
    if game
        .native_set_inventory_slot(
            uuid,
            NativeInventoryKind::Player,
            inventory,
            Some(&stack),
            left,
        )
        .is_err()
    {
        log::warn!("rustcraft: a placer's blocks from {uuid:032x} not taken");
    }
}

/// The self-test's plot cleared: what it showed goes.
pub(super) fn forget_area(dimension: &str, min: Pos, max: Pos) {
    let inside = |p: Pos| (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
    state()
        .shown
        .retain(|(d, p), _| d != dimension || !inside(*p));
}

/// The module helper for the bench: a placer's or a breaker's modules count as the placer's.
pub(super) fn holder(machine: Machine) -> Option<Holder> {
    match machine {
        Machine::Miner => Some(Holder::Miner),
        Machine::Placer | Machine::Breaker => Some(Holder::Placer),
        Machine::Battery => Some(Holder::Battery),
        _ => None,
    }
}
