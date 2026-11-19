// 0.20 (round 31): a machine's worn tool goes to a repair stand on its item net and comes back
// mended (rustcraft_rules::utility::repair decides: wants_repair, units_for, top_up, pick_stand).
//
// The owner: «рыболовный стенд, у него портится удочка, у нас там есть ремонтный стенд, чтобы он
// корректно забирал, отдавал удочку… забирал для её починки необходимые ресурсы… она целая, потом
// возвращалась». Once a second:
//
// - A fishing station whose rod, a butcher whose plain sword, or an archaeology table whose brush
//   has one point left, with a repair stand on one of its item nets that has a free cell for it,
//   hands the tool to the nearest such stand. The tool carries its machine's cell (custom data
//   `rustcraft:tool_home`) and the machine keeps the stand's (`tool_trip` in its saved compound):
//   one tool of a machine is away at a time, its tool cell takes nothing from the net or a hopper
//   meanwhile (rustcraft_machines::slot_takes), and its window says the tool is at the repair.
// - The storages on the stand's item nets give the stand the material its tools away for repair
//   still need (string for a rod, iron for an iron sword, copper for a brush), as far as its
//   material cells hold it; the material that waits in the storage comes the second it is there.
// - The stand mends the tool as any (rustcraft_utility::repair_one); a whole tool that carries a
//   home goes straight back into its machine's tool cell (`send_home`), its mark taken off. A tool
//   whose machine is gone, or whose cell holds another tool, goes to the stand's output as before.
// - A machine with no stand on its nets works as before: the station stops at «Удочка изношена»,
//   the butcher and the table spend the last point.
// The move is straight from block to block, as an order's items move where no route is: no tool
// is ever out of a block, so none is lost to a stop of the server.
use crate::game::{
    Game,
    rustcraft_content::Machine,
    rustcraft_server::LevelNet,
    rustcraft_storage as storage,
    rustcraft_utility::{self as util, CUSTOM_DATA},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    net::items::Mode,
    storage::modules::CAPACITY,
    utility::{REPAIR_MATERIAL, REPAIR_TOOLS, Util, repair, repair_material_end},
};

/// The cell of a machine's tool: the station's rod, the table's brush, the butcher's sword.
pub(super) fn tool_slot(machine: Machine) -> Option<usize> {
    match machine {
        Machine::Util(Util::Fishery) => Some(0),
        Machine::Util(Util::Archeo) => Some(1),
        Machine::Butcher => Some(rustcraft_rules::butcher::SWORD_SLOT),
        _ => None,
    }
}

fn read_pos(nbt: &Compound, key: &str) -> Option<Pos> {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        _ => None,
    }
}

/// Whether the machine `entity` has its tool away at a repair stand.
pub(super) fn away(entity: &ModBlockEntity) -> bool {
    read_pos(&entity.nbt, repair::TRIP_KEY).is_some()
}

/// Whether the machine at `pos` has its tool away at a repair stand.
pub(super) fn away_at(game: &Game<'_>, pos: Pos) -> bool {
    game.native_mod_block_entity(pos).is_some_and(|e| away(&e))
}

fn set_trip(game: &mut Game<'_>, machine: Pos, stand: Option<Pos>) {
    game.native_with_mod_block_entity(machine, |e| match stand {
        Some(s) => {
            e.nbt.insert(JavaString::from(repair::TRIP_KEY), Tag::IntArray(s.to_vec()));
        }
        None => {
            e.nbt.remove(&JavaString::from(repair::TRIP_KEY));
        }
    });
}

/// The machine a tool away for repair belongs to.
pub(super) fn home_of(stack: &ItemStack) -> Option<Pos> {
    match stack.custom_data()?.get(&JavaString::from(repair::HOME_KEY))? {
        Tag::IntArray(v) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        _ => None,
    }
}

/// `stack` marked as away from `home`, or with the mark taken off (none).
fn with_home(mut stack: ItemStack, home: Option<Pos>) -> ItemStack {
    let mut data = stack.custom_data().cloned().unwrap_or_default();
    match home {
        Some(p) => {
            data.insert(JavaString::from(repair::HOME_KEY), Tag::IntArray(p.to_vec()));
        }
        None => {
            data.remove(&JavaString::from(repair::HOME_KEY));
        }
    }
    let tag = (!data.is_empty()).then_some(Tag::Compound(data));
    stack.data_mut().set_other(CUSTOM_DATA, tag);
    stack
}

fn max_damage(game: &Game<'_>, stack: &ItemStack) -> u32 {
    game.items().get(&stack.item).map_or(0, |i| i.max_damage)
}

fn is_stand(level: &LevelNet, pos: Pos) -> bool {
    level.machines.get(&pos) == Some(&Machine::Util(Util::Repair))
}

/// The stands on the item nets of `pos`, each with whether a tool cell of it is free.
fn stands_near(game: &Game<'_>, level: &LevelNet, pos: Pos) -> Vec<(Pos, bool)> {
    let Some(peers) = level.transport.peers(pos) else {
        return Vec::new();
    };
    let mut out: Vec<(Pos, bool)> = Vec::new();
    // A stand whose joint the player switched off is out of the line.
    for (p, _, _) in peers.into_iter().filter(|(_, mode, _)| *mode != Mode::Off) {
        if !is_stand(level, p) || out.iter().any(|(q, _)| *q == p) {
            continue;
        }
        let slots = util::items(game, p);
        let free = REPAIR_TOOLS.clone().any(|i| slots.get(i).is_none_or(Option::is_none));
        out.push((p, free));
    }
    out
}

/// 0.21 (spec 38, FR2): whether a repair stand stands on the item nets of the machine at `pos`: a
/// fishing station whose storage has no whole rod leaves its worn one to the stand.
pub(super) fn stand_near(game: &Game<'_>, level: &LevelNet, pos: Pos) -> bool {
    !stands_near(game, level, pos).is_empty()
}

/// The storages on the item nets of the stand at `stand`, by id.
fn storages_near(dimension: &str, level: &LevelNet, stand: Pos) -> Vec<u32> {
    let mut ids: Vec<u32> = level
        .transport
        .peers(stand)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, mode, _)| *mode != Mode::Off)
        .filter_map(|(p, _, _)| storage::storage_at(dimension, p))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Once a second, every machine with a tool on the level (see the head of this file).
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &LevelNet) {
    let tooled: Vec<(Pos, usize)> = level
        .machines
        .iter()
        .filter_map(|(p, m)| tool_slot(*m).map(|s| (*p, s)))
        .collect();
    for (pos, slot) in tooled {
        follow(game, dimension, level, pos, slot);
    }
}

fn follow(game: &mut Game<'_>, dimension: &str, level: &LevelNet, pos: Pos, slot: usize) {
    let Some(entity) = game.native_mod_block_entity(pos) else {
        return;
    };
    let trip = read_pos(&entity.nbt, repair::TRIP_KEY);
    let tool = entity.items.get(slot).cloned().flatten().filter(|s| s.count > 0);
    match (trip, tool) {
        // A tool in its cell: a mark a hand brought back comes off, a trip is over.
        (trip, Some(tool)) => {
            if home_of(&tool).is_some() {
                util::with_items(game, pos, |items| {
                    if let Some(cell) = items.get_mut(slot) {
                        *cell = cell.take().map(|t| with_home(t, None));
                    }
                });
            }
            if trip.is_some() {
                set_trip(game, pos, None);
                return;
            }
            let max = max_damage(game, &tool);
            if repair::wants_repair(tool.damage, max) && repair::material_of(&tool.item).is_some() {
                send(game, dimension, level, pos, slot, tool);
            }
        }
        // Away: the stand gets its material; a stand that is gone, or a tool a player took off
        // it, ends the trip (the cell takes a tool from a hand again).
        (Some(stand), None) => {
            if !is_stand(level, stand) || !holds_home(game, stand, pos) {
                set_trip(game, pos, None);
                return;
            }
            feed(game, dimension, level, stand);
        }
        (None, None) => {}
    }
}

/// Whether the stand at `stand` holds a tool marked for `home`, in a cell to mend or done.
fn holds_home(game: &Game<'_>, stand: Pos, home: Pos) -> bool {
    util::items(game, stand)
        .iter()
        .flatten()
        .any(|s| home_of(s) == Some(home))
}

/// The worn `tool` of the machine at `pos` into the nearest stand with a free cell, if any.
fn send(game: &mut Game<'_>, dimension: &str, level: &LevelNet, pos: Pos, slot: usize, tool: ItemStack) {
    let Some(stand) = repair::pick_stand(pos, &stands_near(game, level, pos)) else {
        return;
    };
    let marked = with_home(tool, Some(pos));
    let placed = util::with_items(game, stand, move |items| {
        if items.len() < REPAIR_TOOLS.end {
            items.resize(REPAIR_TOOLS.end, None);
        }
        let free = REPAIR_TOOLS.clone().find(|&i| items[i].is_none())?;
        items[free] = Some(marked);
        Some(free)
    })
    .flatten();
    if placed.is_none() {
        return;
    }
    util::with_items(game, pos, |items| {
        if let Some(cell) = items.get_mut(slot) {
            *cell = None;
        }
    });
    set_trip(game, pos, Some(stand));
    log::info!("rustcraft: tool of {pos:?} to the repair stand at {stand:?}");
    feed(game, dimension, level, stand);
}

/// The storages of the stand's nets give it what its tools away for repair still need.
fn feed(game: &mut Game<'_>, dimension: &str, level: &LevelNet, stand: Pos) {
    let slots = util::items(game, stand);
    let end = repair_material_end(util::module_count(game, stand, CAPACITY));
    let materials = REPAIR_MATERIAL.start..end;
    // Each material the stand's travelling tools need, in units.
    let mut need: Vec<(&'static [&'static str], u32)> = Vec::new();
    for tool in REPAIR_TOOLS.clone().filter_map(|i| slots.get(i).cloned().flatten()) {
        if home_of(&tool).is_none() {
            continue;
        }
        let Some(material) = repair::material_of(&tool.item) else {
            continue;
        };
        let units = repair::units_for(tool.damage, max_damage(game, &tool));
        match need.iter_mut().find(|(m, _)| *m == material) {
            Some((_, n)) => *n += units,
            None => need.push((material, units)),
        }
    }
    if need.is_empty() {
        return;
    }
    let ids = storages_near(dimension, level, stand);
    for (material, units) in need {
        let have: u32 = materials
            .clone()
            .filter_map(|i| slots.get(i).cloned().flatten())
            .filter(|s| material.contains(&&*s.item))
            .map(|s| s.count)
            .sum();
        // What does not fit in its cells goes back to the storage (`put` gives it back).
        let mut want = repair::top_up(units, have, u32::MAX);
        for id in &ids {
            for item in material {
                if want == 0 {
                    break;
                }
                let took = storage::take_from(dimension, *id, item, u64::from(want)) as u32;
                if took == 0 {
                    continue;
                }
                let rest = util::put(game, stand, materials.clone(), ItemStack::new(item, took))
                    .map_or(0, |r| r.count.min(took));
                if rest > 0 {
                    storage::restore_into(dimension, *id, item, u64::from(rest));
                }
                want -= took - rest;
                if rest > 0 {
                    want = 0;
                }
            }
        }
    }
}

/// The stand at `stand` mended the tool in its cell `cell` whole: one that carries a home goes
/// back into its machine's tool cell, its mark off. False when it has no home or its machine
/// cannot take it now (the stand puts it out as any).
pub(super) fn send_home(game: &mut Game<'_>, level: &LevelNet, stand: Pos, cell: usize, tool: &ItemStack) -> bool {
    let Some(home) = home_of(tool) else {
        return false;
    };
    let Some(slot) = level.machines.get(&home).copied().and_then(tool_slot) else {
        return false;
    };
    let back = with_home(tool.clone(), None);
    let placed = util::with_items(game, home, move |items| {
        if items.len() <= slot {
            items.resize(slot + 1, None);
        }
        if items[slot].is_some() {
            return false;
        }
        items[slot] = Some(back);
        true
    }) == Some(true);
    if !placed {
        return false;
    }
    util::with_items(game, stand, |items| {
        if let Some(c) = items.get_mut(cell) {
            *c = None;
        }
    });
    set_trip(game, home, None);
    log::info!("rustcraft: mended tool back from {stand:?} into {home:?}");
    true
}

/// The tool marked for `home` comes off its mark wherever it lies (`stack`), for the stand's
/// output when its machine cannot take it.
pub(super) fn unmarked(stack: ItemStack) -> ItemStack {
    if home_of(&stack).is_some() { with_home(stack, None) } else { stack }
}
