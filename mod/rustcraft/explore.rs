// Slice 13: the cartographer and the scout compass on the server (rustcraft_rules::explore).
//
// - A worker with the map tool, in each rail cell it comes into, marks the band of chunks round it
//   (its chunk and one each way, two with a radius module) and the biome there for its owner;
//   each new one costs 10 E of its battery and is told to Path-of-craft-tree once (EX1: `chunk`,
//   `biome`). The atlas is a file of the world (`data/rustcraft_explore.json`).
// - Twice a second each player's biome is noted as visited; a player holding the scout compass
//   reads over the hotbar the nearest found biome of the dimension they have not been in, how far
//   and which way.
use crate::game::{
    Game, rustcraft_base as base,
    rustcraft_server::{line, tell},
};
use horizon_client::text::Text;
use rustcraft_rules::{
    explore::{self as rules, Atlas},
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

/// The cartographer in a rail cell.
pub(super) fn map(game: &mut Game<'_>, dimension: &str, w: &mut Worker, cell: Pos, t: &Tuning) {
    let side = if w.count(Module::Radius) > 0 { 2 } else { 1 };
    let biome = game.native_biome(cell);
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
    }
    for r in reports {
        base::report(r);
    }
}

/// Twice a second for a player of this level: their biome is visited; the compass speaks.
pub(super) fn player(game: &mut Game<'_>, p: &horizon_client::mod_api::NativeServerPlayer, holds_compass: bool) {
    let at = p.position;
    let cell = at.map(|v| v.floor() as i32);
    let Some(biome) = game.native_biome(cell) else { return };
    let target = {
        let mut s = state();
        load(&mut s, game);
        if !s.atlas.visited.get(&p.uuid).is_some_and(|v| v.contains(&biome)) {
            s.atlas.visit(p.uuid, &biome);
            s.dirty = true;
        }
        if holds_compass { s.atlas.target(p.uuid, &p.dimension, at) } else { None }
    };
    if !holds_compass {
        return;
    }
    let text = match target {
        Some((b, to, d)) => {
            let way = rules::direction(at, to);
            line(
                "rustcraft.compass.target",
                "%s · %s blocks %s",
                &[
                    Text::translate(&format!("biome.{}", b.replace(':', ".")), &[]),
                    Text::literal(format!("{d:.0}")),
                    line(&format!("rustcraft.compass.{way}"), way, &[]),
                ],
            )
        }
        None => line("rustcraft.compass.none", "No new places on the map", &[]),
    };
    tell(game, p.uuid, text);
}
