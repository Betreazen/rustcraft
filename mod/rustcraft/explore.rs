// Slice 13: the cartographer and the scout compass on the server (rustcraft_rules::explore).
//
// - A worker with the map tool, in each rail cell it comes into, marks the band of chunks round it
//   (its chunk and one each way, two with a radius module), the biome there and the structures
//   near its way for its owner; each new one costs 10 E of its battery and is told to
//   Path-of-craft-tree once (EX1: `chunk`, `biome`, `structure`). The atlas is a file of the world
//   (`data/rustcraft_explore.json`).
// - Structures: `Game::structures_reaching` (SRC/crates/horizon-client/src/game/structures.rs:9,
//   the starts whose whole box holds a cell, from the chunk's structure references and the
//   generator's cached starts) at SAMPLE_BUDGET cells of a grid round the rail cell
//   (rustcraft_rules::explore::structure_samples), the grid taken in turns from one cell to the
//   next. A chunk with no structure reference answers at once.
// - Twice a second each player's biome and the structures whose pieces hold their feet
//   (`Game::structures_at`, structures.rs:47, as Path-of-craft-tree reads a player's) are noted as
//   visited; a player holding the scout compass reads over the hotbar the nearest found biome or
//   structure of the dimension they have not been in, how far and which way.
use crate::game::{
    Game, rustcraft_base as base,
    rustcraft_server::{line, tell},
};
use horizon_client::text::Text;
use rustcraft_rules::{
    explore::{self as rules, Atlas, Place},
    geom::Pos,
    modules::Module,
    reports::Report,
    tuning::Tuning,
    worker::Worker,
};
use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

pub(super) const COMPASS: &str = "rustcraft:scout_compass";
/// E a new chunk or biome costs the cartographer's battery.
const MARK_ENERGY: f64 = 10.0;

#[derive(Default)]
struct State {
    path: Option<PathBuf>,
    atlas: Atlas,
    dirty: bool,
    /// Where the next structure samples start in the grid (rules::structure_samples).
    turn: usize,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn load(s: &mut State, game: &Game<'_>) {
    if s.path.is_some() {
        return;
    }
    let Some(world) = game.native_world_dir() else { return };
    let path = world.join("data").join("rustcraft_explore.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        match serde_json::from_str(&text) {
            Ok(a) => s.atlas = a,
            Err(e) => log::warn!("rustcraft: {} does not read: {e}", path.display()),
        }
    }
    s.path = Some(path);
}

/// Writes the atlas when it changed, whole under another name first.
pub(super) fn save(game: &Game<'_>) {
    let mut s = state();
    load(&mut s, game);
    if !s.dirty {
        return;
    }
    let Some(path) = s.path.clone() else { return };
    let Ok(json) = serde_json::to_string(&s.atlas) else { return };
    s.dirty = false;
    drop(s);
    let fresh = path.with_extension("json.tmp");
    let done = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&fresh, json))
        .and_then(|()| std::fs::rename(&fresh, &path));
    if let Err(e) = done {
        log::warn!("rustcraft: the atlas was not saved: {e}");
    }
}

/// The structures whose boxes reach this turn's sample cells round `cell`: each id once, with the
/// first cell it reached.
fn structures_near(game: &Game<'_>, cell: Pos, side: i32) -> Vec<(String, Pos)> {
    let turn = {
        let mut s = state();
        let turn = s.turn;
        s.turn = s.turn.wrapping_add(rules::SAMPLE_BUDGET);
        turn
    };
    let mut out: Vec<(String, Pos)> = Vec::new();
    for p in rules::structure_samples(cell, side, turn, rules::SAMPLE_BUDGET) {
        for (id, _) in game.structures_reaching(p) {
            if !out.iter().any(|(o, _)| *o == id) {
                out.push((id, p));
            }
        }
    }
    out
}

/// The cartographer in a rail cell.
pub(super) fn map(game: &mut Game<'_>, dimension: &str, w: &mut Worker, cell: Pos, t: &Tuning) {
    let side = if w.count(Module::Radius) > 0 { 2 } else { 1 };
    let biome = game.native_biome(cell);
    let structures = if w.charge >= MARK_ENERGY {
        structures_near(game, cell, side)
    } else {
        Vec::new()
    };
    let mut reports = Vec::new();
    {
        let mut s = state();
        load(&mut s, game);
        for (x, z) in rules::band(cell, side) {
            if w.charge < MARK_ENERGY {
                break;
            }
            if s.atlas.mark_chunk(w.owner, (dimension.to_owned(), x, z)) {
                w.spend(0.0, MARK_ENERGY, t);
                reports.push(Report::Chunk { owner: w.owner, dimension: dimension.to_owned(), x, z });
                s.dirty = true;
            }
        }
        if let Some(b) = biome
            && w.charge >= MARK_ENERGY
            && s.atlas.mark_biome(w.owner, dimension, &b, cell)
        {
            w.spend(0.0, MARK_ENERGY, t);
            reports.push(Report::Biome { owner: w.owner, biome: b });
            s.dirty = true;
        }
        for (id, at) in structures {
            if w.charge < MARK_ENERGY {
                break;
            }
            let sites = s.atlas.sites.get(dimension).map_or(0, Vec::len);
            if s.atlas.mark_structure(w.owner, dimension, &id, at) {
                w.spend(0.0, MARK_ENERGY, t);
                reports.push(Report::Structure { owner: w.owner, id });
                s.dirty = true;
            } else if s.atlas.sites.get(dimension).map_or(0, Vec::len) != sites {
                s.dirty = true;
            }
        }
    }
    // Slice 18: after a whole stretch with nothing new it says so («Вокруг пути всё нанесено»).
    w.found_as(reports.len(), MAP_IDLE_CELLS, rustcraft_rules::worker::Note::AllMapped);
    for r in reports {
        base::report(r);
    }
}

/// Rail cells in a row with nothing new before the cartographer says all is mapped: a loop of
/// a few chunks.
const MAP_IDLE_CELLS: u32 = 64;


/// Twice a second for a player of this level: their biome and the structures round their feet
/// are visited; the compass speaks. Slice 18: `compass` is the held compass's mode (structures or
/// biomes, rustcraft_rules::gear::compass), and its target comes back for its needle.
pub(super) fn player(
    game: &mut Game<'_>,
    p: &horizon_client::mod_api::NativeServerPlayer,
    compass: Option<rustcraft_rules::gear::compass::Mode>,
) -> Option<[i32; 3]> {
    let holds_compass = compass.is_some();
    let at = p.position;
    let cell = at.map(|v| v.floor() as i32);
    let Some(biome) = game.native_biome(cell) else { return None };
    let inside = game.structures_at(cell);
    let target = {
        let mut s = state();
        load(&mut s, game);
        if !s.atlas.visited.get(&p.uuid).is_some_and(|v| v.contains(&biome)) {
            s.atlas.visit(p.uuid, &biome);
            s.dirty = true;
        }
        for id in &inside {
            if s.atlas.enter(p.uuid, id) {
                s.dirty = true;
            }
        }
        compass.and_then(|mode| rustcraft_rules::gear::compass::target(&s.atlas, mode, p.uuid, &p.dimension, at))
    };
    if !holds_compass {
        return None;
    }
    let needle = target.as_ref().map(|t| t.at);
    let text = match target {
        Some(target) => {
            let way = rules::direction(at, target.at);
            let name = match &target.place {
                Place::Biome(b) => Text::translate(&format!("biome.{}", b.replace(':', ".")), &[]),
                Place::Structure(id) => line(
                    &format!("rustcraft.structure.{}", id.replace(':', ".")),
                    &rules::structure_words(id),
                    &[],
                ),
            };
            line(
                "rustcraft.compass.target",
                "%s · %s blocks %s",
                &[
                    name,
                    Text::literal(format!("{:.0}", target.distance)),
                    line(&format!("rustcraft.compass.{way}"), way, &[]),
                ],
            )
        }
        None => line("rustcraft.compass.none", "No new places on the map", &[]),
    };
    tell(game, p.uuid, text);
    needle
}

/// Slice 18 (guide `tool_map`: «Найденные биомы и постройки … появляются на схеме в окне любой
/// станции»): the biomes and the structures found in `dimension`, each `(id, block)`, and how many
/// chunks of it the cartographers charted.
pub(super) fn found_in(game: &Game<'_>, dimension: &str) -> (Vec<(String, Pos)>, Vec<(String, Pos)>, u32) {
    let mut s = state();
    load(&mut s, game);
    let biomes = s.atlas.found.get(dimension).cloned().unwrap_or_default();
    let sites = s.atlas.sites.get(dimension).cloned().unwrap_or_default();
    let charted: std::collections::BTreeSet<(i32, i32)> = s
        .atlas
        .chunks
        .values()
        .flatten()
        .filter(|c| c.0 == dimension)
        .map(|c| (c.1, c.2))
        .collect();
    (biomes, sites, charted.len() as u32)
}
