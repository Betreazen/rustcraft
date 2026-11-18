// The diagnostics console command and the event journal (rustcraft_rules::debug). The owner's
// server is reached through its console alone, so whatever a report needs is a console answer:
// `rustcraft_debug at|near|workers|storage|log` (registered with `native_register_commands`, heard
// through `native_perform_console_command`, API 99, as rustcraft_selftest's), each line of its
// answer a `native_feedback` line.
//
// The journal is `rustcraft-debug.log` in the world folder: a machine's status changing (its
// looked-at diagnosis, rustcraft_server::diagnosis_of, compared by its translation keys), a
// worker's errand or note changing, a worker placed or gone, a RustCraft block a player placed or
// broke, an order started, waiting on something else, finished, cancelled or gone, a job handed to
// a machine or ended, items leaving or entering a storage (for an order, by the net's own routing,
// to a player; summed a second by route and item, `flow`), and the mod's warnings
// (`journal_warn!`). Machines are looked at a batch a second, round the level's list; lines wait
// in a buffer that goes to the file once a second.
use crate::game::{
    Game, rustcraft_base as base,
    rustcraft_content::Machine,
    rustcraft_farm as farm, rustcraft_machines as machines,
    rustcraft_server::{self as server, LevelNet},
    rustcraft_orders as orders, rustcraft_storage as storage, rustcraft_tiers as tiers,
    rustcraft_workers as workers,
};
use crate::mod_api::NativeCommand;
use horizon_client::text::Text;
use horizon_world::nbt::JavaString;
use rustcraft_rules::{
    debug::{
        self, COMMAND, Command, TOP_ITEMS, WorkerRow,
        flow::Flows,
        journal::{self, Buffer},
        pos_text, text,
        track::{OrderSeen, Seen, Tracker, next_batch, order_events, set_diff},
    },
    geom::{Face, Pos, step},
    utility::Util,
    worker::{Worker, wire::note_key},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// Machines whose status is looked at each second, per level.
const BATCH: usize = 64;

/// `log::warn!` that also goes to the journal: `crate::game::rustcraft_debug::journal_warn!(...)`.
macro_rules! journal_warn {
    ($($arg:tt)*) => {{
        let line = format!($($arg)*);
        log::warn!("{line}");
        $crate::game::rustcraft_debug::note_warning(&line);
    }};
}
pub(crate) use journal_warn;

// ---- State ----

#[derive(Default)]
struct Journal {
    /// The file, once the world's folder is known.
    path: Option<PathBuf>,
    buffer: Buffer,
}

#[derive(Default)]
struct Track {
    machines: Tracker<(String, Pos)>,
    /// Where each level's round of machines goes on from.
    cursors: HashMap<String, Option<Pos>>,
    workers: Tracker<(String, u64)>,
    /// The levels whose fleet was seen once: the first sight is the baseline, not «placed».
    fleets_seen: BTreeSet<String>,
    /// Players' names as last seen online, for the owners of workers and machines.
    names: HashMap<u128, String>,
    /// Each level's orders and jobs as last seen; a level not seen yet has its baseline taken.
    orders: HashMap<String, (BTreeMap<(u32, u16), OrderSeen>, BTreeSet<JobSeen>)>,
    /// Orders whose end was journalled where it happened (finished, cancelled): the next look
    /// does not call them «gone».
    ended: BTreeSet<(String, u32, u16)>,
}

/// A job as the journal compares it: storage, order, machine, what it makes, runs.
type JobSeen = (u32, u16, Pos, String, u32);

static JOURNAL: LazyLock<Mutex<Journal>> = LazyLock::new(Mutex::default);
static TRACK: LazyLock<Mutex<Track>> = LazyLock::new(Mutex::default);
/// Items on the move out of and into storages, by level, summed until the level's next second.
static FLOWS: LazyLock<Mutex<HashMap<String, Flows>>> = LazyLock::new(Mutex::default);
static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);
/// The game tick the server last ran, for lines recorded without the game at hand.
static TICK: AtomicU64 = AtomicU64::new(0);

fn journal_state() -> MutexGuard<'static, Journal> {
    JOURNAL.lock().unwrap_or_else(PoisonError::into_inner)
}

fn track() -> MutexGuard<'static, Track> {
    TRACK.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> u128 {
    EPOCH.elapsed().as_millis()
}

fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    journal::utc(secs)
}

// ---- The journal ----

/// One event into the journal.
pub(super) fn record(dimension: Option<&str>, pos: Option<Pos>, kind: &str, details: &str) {
    let line = journal::line(
        &utc_now(),
        TICK.load(Ordering::Relaxed),
        dimension,
        pos,
        kind,
        details,
    );
    let mut j = journal_state();
    j.buffer.push(line);
    if j.buffer.due(now_ms()) {
        flush(&mut j);
    }
}

/// A warning the mod logged (`journal_warn!`), into the journal.
pub(crate) fn note_warning(line: &str) {
    record(None, None, "warn", line.strip_prefix("rustcraft: ").unwrap_or(line));
}

/// The buffer to the file, the file moved to `.1` first when it grew past its limit. Nothing
/// happens while the file is not known (the lines wait, the oldest dropped).
fn flush(j: &mut Journal) {
    let Some(path) = j.path.clone() else {
        return;
    };
    let text = j.buffer.take(now_ms());
    if text.is_empty() {
        return;
    }
    // Not journal_warn!: that would come back here.
    if let Err(e) = append(&path, &text) {
        log::warn!("rustcraft: the debug journal {} not written: {e}", path.display());
    }
}

fn backup(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".1");
    PathBuf::from(name)
}

fn append(path: &Path, text: &str) -> std::io::Result<()> {
    let size = std::fs::metadata(path).map_or(0, |m| m.len());
    if journal::should_rotate(size, text.len() as u64) {
        let old = backup(path);
        let _ = std::fs::remove_file(&old);
        std::fs::rename(path, &old)?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(text.as_bytes())
}

/// `native_server_stopping`: what waits goes to the file; a world opened next starts afresh.
pub(super) fn on_stopping() {
    {
        let mut j = journal_state();
        flush(&mut j);
        j.path = None;
    }
    let mut t = track();
    let names = std::mem::take(&mut t.names);
    *t = Track {
        names,
        ..Track::default()
    };
}

// ---- Once a second, per level ----

/// `native_level_tick_post`, once a second (rustcraft_server::on_level_tick): the file found,
/// the players' names noted, a batch of machines and the fleet looked at, the buffer written.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    if let Some(t) = game.native_level(dimension).and_then(|l| game.native_level_game_time(l)) {
        TICK.store(u64::try_from(t).unwrap_or(0), Ordering::Relaxed);
    }
    {
        let mut j = journal_state();
        if j.path.is_none() {
            j.path = game.native_world_dir().map(|w| w.join(journal::FILE));
        }
        if j.buffer.due(now_ms()) {
            flush(&mut j);
        }
    }
    remember_names(game);
    machines_tick(game, dimension);
    workers_tick(dimension);
    orders_tick(dimension);
    let moved = FLOWS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_mut(dimension)
        .map(Flows::drain)
        .unwrap_or_default();
    for line in moved {
        record(Some(dimension), None, "flow", &line);
    }
}

/// `n` of `item` moved along `route` (`storage#3 -> rustcraft:crusher(1 2 3) net`): summed, and
/// one journal line a route and item at the level's next second.
pub(super) fn flow(dimension: &str, route: &str, item: &str, n: u64) {
    FLOWS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(dimension.to_owned())
        .or_default()
        .add(route, &debug::short_key(item), n);
}

/// `flow` of items a player took out of the storage `storage`.
pub(super) fn flow_to_player(dimension: &str, storage: u32, item: &str, player: u128, n: u64) {
    let route = format!("storage#{storage} -> player {}", name_of(player));
    flow(dimension, &route, item, n);
}

/// An order of the storage `storage` ended where it happened (`order_finished` from the queue's
/// round, `order_cancelled` from the terminal's «x»): journalled now, so the next look's «gone»
/// does not repeat it, and an order made within one second still shows.
pub(super) fn order_ended(dimension: &str, storage: u32, o: &rustcraft_rules::orders::queue::Order, kind: &str) {
    track()
        .ended
        .insert((dimension.to_owned(), storage, o.id));
    record(
        Some(dimension),
        None,
        kind,
        &format!("storage#{storage} order#{} {} {}/{}", o.id, o.item, o.done, o.count),
    );
}

fn orders_tick(dimension: &str) {
    let (queues, jobs) = orders::debug_orders(dimension);
    let now: BTreeMap<(u32, u16), OrderSeen> = queues
        .iter()
        .flat_map(|(s, q)| {
            q.iter().map(move |o| {
                let seen = OrderSeen {
                    item: o.item.clone(),
                    count: o.count,
                    done: o.done,
                    wait: format!("{:?}", o.wait),
                };
                ((*s, o.id), seen)
            })
        })
        .collect();
    let now_jobs: BTreeSet<JobSeen> = jobs
        .iter()
        .map(|(s, j)| (*s, j.order, j.machine, j.step.output.0.clone(), j.runs))
        .collect();
    let (events, started, ended) = {
        let mut t = track();
        let Some((before, before_jobs)) = t
            .orders
            .insert(dimension.to_owned(), (now.clone(), now_jobs.clone()))
        else {
            // The first look is the baseline.
            return;
        };
        let (started, ended) = set_diff(&before_jobs, &now_jobs);
        let mut events = order_events(&before, &now);
        // An end journalled where it happened is not «gone» again; the marks serve this look only.
        let marked: BTreeSet<(u32, u16)> = t
            .ended
            .iter()
            .filter(|(d, _, _)| d == dimension)
            .map(|(_, s, o)| (*s, *o))
            .collect();
        t.ended.retain(|(d, _, _)| d != dimension);
        events.retain(|(k, kind, _)| *kind != "order_gone" || !marked.contains(k));
        (events, started, ended)
    };
    for (_, kind, details) in events {
        record(Some(dimension), None, kind, &details);
    }
    for (kind, list) in [("job_started", started), ("job_ended", ended)] {
        for (s, o, machine, output, runs) in list {
            record(
                Some(dimension),
                Some(machine),
                kind,
                &format!("storage#{s} order#{o} makes {output} runs {runs}"),
            );
        }
    }
}

fn remember_names(game: &Game<'_>) {
    let players = game.native_server_players();
    let mut t = track();
    for p in players {
        t.names.insert(p.uuid, p.name);
    }
}

fn name_of(uuid: u128) -> String {
    track()
        .names
        .get(&uuid)
        .cloned()
        .unwrap_or_else(|| format!("{uuid:032x}"))
}

/// The status of the block at `pos` in the level in place, as the player looking at it reads it.
fn status_of(game: &Game<'_>, dimension: &str, l: &LevelNet, pos: Pos, block: &str) -> Value {
    server::diagnosis_of(game, dimension, l, pos, block)
        .map_or(Value::String("-".into()), |t| t.component())
}

/// A status seen: noted, and a change into the journal.
fn observe(dimension: &str, pos: Pos, block: &str, status: &Value) {
    let sig = text::signature(status);
    let seen = track().machines.observe((dimension.to_owned(), pos), &sig);
    if let Seen::Changed(old) = seen {
        record(
            Some(dimension),
            Some(pos),
            "status",
            &format!("{block} {old} -> {sig} | {}", text::plain(status)),
        );
    }
}

fn machines_tick(game: &Game<'_>, dimension: &str) {
    let cursor = track().cursors.get(dimension).copied().flatten();
    let Some((seen, next, all)) = server::with_level(dimension, |l| {
        let all: BTreeSet<Pos> = l.machines.keys().copied().collect();
        let (batch, next) = next_batch(&all, cursor.as_ref(), BATCH);
        let seen: Vec<(Pos, String, Value)> = batch
            .into_iter()
            .map(|p| {
                let block = game.native_block(p);
                let status = status_of(game, dimension, l, p, &block);
                (p, block, status)
            })
            .collect();
        (seen, next, all)
    }) else {
        return;
    };
    {
        let mut t = track();
        t.cursors.insert(dimension.to_owned(), next);
        // A machine gone from the level's list leaves the tracker.
        t.machines
            .retain(|(d, p)| d != dimension || all.contains(p));
    }
    for (p, block, status) in seen {
        observe(dimension, p, &block, &status);
    }
}

fn worker_status(w: &Worker) -> String {
    format!("{:?}/{}", w.errand, note_key(w.note).0)
}

fn workers_tick(dimension: &str) {
    let Some(fleet) = workers::fleets()
        .into_iter()
        .find(|(d, _)| d == dimension)
        .map(|(_, f)| f)
    else {
        return;
    };
    let mut events = Vec::new();
    {
        let mut t = track();
        let first = t.fleets_seen.insert(dimension.to_owned());
        let ids: BTreeSet<u64> = fleet.iter().map(|w| w.id).collect();
        for (_, id) in t.workers.retain(|(d, id)| d != dimension || ids.contains(id)) {
            events.push((None, "worker_removed", format!("worker #{id}")));
        }
        for w in &fleet {
            let status = worker_status(w);
            match t.workers.observe((dimension.to_owned(), w.id), &status) {
                Seen::New if !first => events.push((
                    Some(w.cell),
                    "worker_placed",
                    format!("worker #{} owner {:032x} {status}", w.id, w.owner),
                )),
                Seen::Changed(old) => events.push((
                    Some(w.cell),
                    "worker",
                    format!("worker #{} {old} -> {status}", w.id),
                )),
                _ => {}
            }
        }
    }
    for (pos, kind, details) in events {
        record(Some(dimension), pos, kind, &details);
    }
}

// ---- The console command ----

/// `native_register_commands`: the command, for operators of level 4 only.
pub(super) fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op(&format!("{COMMAND} <args:text>"), 4));
}

/// `native_perform_console_command`: `rustcraft_debug …`, each line of the answer a feedback line.
pub(super) fn on_console_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let Some(args) = command
        .strip_prefix(COMMAND)
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
    else {
        return;
    };
    *ran = true;
    let lines = answer(game, args);
    log::info!("rustcraft: {COMMAND} {}: {} lines", args.trim(), lines.len());
    for line in lines {
        game.native_feedback(Text::literal(line));
    }
}

/// The answer to `rustcraft_debug <args>`, its lines (the self-test's bench asks it too).
pub(super) fn answer(game: &mut Game<'_>, args: &str) -> Vec<String> {
    remember_names(game);
    let lines = match debug::parse(args) {
        Ok(Command::At { pos, dimension }) => at(game, pos, dimension),
        Ok(Command::Near { player, radius }) => near(game, &player, radius),
        Ok(Command::Workers { owner }) => workers_list(game, owner.as_deref()),
        Ok(Command::Storage { pos, dimension }) => storage_view(game, pos, dimension),
        Ok(Command::Log { n, filter }) => log_tail(n, filter.as_deref()),
        Err(usage) => vec![usage],
    };
    debug::clip(lines)
}

fn loaded(game: &Game<'_>, dimension: &str, p: Pos) -> bool {
    game.native_level(dimension)
        .is_some_and(|l| game.native_level_is_loaded(l, p))
}

// ---- at ----

fn at(game: &mut Game<'_>, pos: Pos, dimension: Option<String>) -> Vec<String> {
    let game = &*game;
    let here = game.native_dimension();
    let dim = dimension.unwrap_or_else(|| here.to_owned());
    let Some(level) = game.native_level(&dim) else {
        return vec![format!("no level {dim} on this server")];
    };
    if !game.native_level_is_loaded(level, pos) {
        return vec![format!("{dim} {}: its chunk is not loaded", pos_text(pos))];
    }
    let block = game.native_level_block(level, pos).unwrap_or_default();
    let state = game.native_level_block_state(level, pos).unwrap_or_default();
    let mut out = vec![format!("{dim} {}: {state}", pos_text(pos))];
    let machine = Machine::of(&block);
    if dim != here {
        // The mod's helpers read the level in place; another level gives what the server keeps.
        out.push(format!(
            "machine {machine:?}; details only in the level in place ({here}); last status seen: {}",
            track()
                .machines
                .last(&(dim.clone(), pos))
                .unwrap_or("none")
        ));
        if let Some(lines) = server::with_level(&dim, |l| magistral(l, pos)) {
            out.extend(lines);
        }
        return out;
    }
    let Some((status, mut lines)) = server::with_level(&dim, |l| {
        let status = status_of(game, &dim, l, pos, &block);
        let mut lines = vec![format!(
            "machine {machine:?}, in the level's list: {}, power status: {}",
            l.machines.contains_key(&pos),
            l.power
                .status
                .get(&pos)
                .map_or_else(|| "none".to_owned(), |s| format!("{:?}", s.shown))
        )];
        lines.extend(sheet_lines(game, &dim, l, pos, &block));
        lines.extend(magistral(l, pos));
        (status, lines)
    }) else {
        return vec![format!("the server has not read {dim} yet")];
    };
    out.push(format!("status: {}", text::status_line(&status)));
    if machine.is_some() {
        observe(&dim, pos, &block, &status);
    }
    out.append(&mut lines);
    out.extend(entity_lines(game, pos, &block));
    out.extend(extras(game, &dim, pos, machine));
    out
}

/// The window's sheet of a slice-7 or later block: its status, rows and bars.
fn sheet_lines(game: &Game<'_>, dim: &str, l: &LevelNet, pos: Pos, block: &str) -> Vec<String> {
    let Some(s) = base::sheet(game, dim, l, pos, block, server::tuning()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(st) = &s.status {
        out.push(format!(
            "window status (severity {}): {}",
            st.severity,
            text::status_line(&st.text)
        ));
    }
    for row in &s.rows {
        out.push(format!("  row: {}", text::status_line(row)));
    }
    for bar in &s.bars {
        out.push(format!(
            "  bar: {} = {} ({:.0}%)",
            text::plain(&bar.label),
            text::plain(&bar.value),
            bar.share * 100.0
        ));
    }
    out
}

/// The owner, tier, buffer, sockets and slots the block entity holds.
fn entity_lines(game: &Game<'_>, pos: Pos, block: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(e) = machines::entity(game, pos) else {
        out.push("no mod block entity".to_owned());
        return out;
    };
    let owner = base::owner_at(game, pos).map_or_else(|| "none".to_owned(), name_of);
    let tier = rustcraft_rules::tiers::family_of_block(block)
        .map_or_else(|| "-".to_owned(), |_| tiers::tier_of(game, pos, block).to_string());
    let mut head = format!("owner {owner}, tier {tier}");
    if e.nbt.get(&JavaString::from(machines::BUFFER_KEY)).is_some() {
        let spec = Machine::of(block).map(|m| machines::buffer_spec(m, server::tuning()));
        head.push_str(&format!(
            ", energy buffer {:.1}{}",
            machines::buffer_energy(game, pos),
            spec.map_or_else(String::new, |(cap, draw)| format!(" of {cap} (draw {draw} E/s)"))
        ));
    }
    out.push(head);
    let modules: Vec<String> = base::module_items(game, pos)
        .into_iter()
        .map(|m| m.unwrap_or_else(|| "-".to_owned()))
        .collect();
    if !modules.is_empty() {
        out.push(format!("modules: {}", modules.join(", ")));
    }
    let slots: Vec<String> = (0..e.items.len())
        .filter_map(|i| machines::slot_of(e, i).map(|(item, n)| format!("{i}:{item} x{n}")))
        .collect();
    out.push(format!(
        "slots ({}): {}",
        e.items.len(),
        if slots.is_empty() { "empty".to_owned() } else { slots.join(", ") }
    ));
    out
}

/// What a kind of machine adds; more kinds go here.
fn extras(game: &Game<'_>, dim: &str, pos: Pos, machine: Option<Machine>) -> Vec<String> {
    match machine {
        Some(Machine::Util(Util::Fishery)) => farm::fishery_debug(game, dim, pos),
        _ => Vec::new(),
    }
}

/// The magistral round the block: the housings touching it, their lines and channel nets, the
/// energy circuits it is a member of and what it got.
fn magistral(l: &LevelNet, pos: Pos) -> Vec<String> {
    let touching = |p: Pos, set: &BTreeSet<Pos>| Face::ALL.iter().any(|f| set.contains(&step(p, *f)));
    let mut housings: Vec<Pos> = Face::ALL
        .iter()
        .map(|f| step(pos, *f))
        .filter(|p| l.layout.housings.contains_key(p))
        .collect();
    if l.layout.housings.contains_key(&pos) {
        housings.push(pos);
    }
    let mut out = Vec::new();
    if housings.is_empty() {
        out.push("magistral: no housing touches it".to_owned());
    }
    let lines: BTreeSet<usize> = housings
        .iter()
        .filter_map(|h| l.topology.line_of(*h))
        .collect();
    for i in lines {
        let hs = &l.topology.lines[i];
        let nodes = l.layout.nodes.keys().filter(|n| touching(**n, hs)).count();
        let on_line = l.machines.keys().filter(|m| touching(**m, hs)).count();
        out.push(format!(
            "magistral line #{i}: {} housings, {nodes} nodes, {on_line} machines touching it",
            hs.len()
        ));
    }
    for net in l
        .topology
        .nets
        .iter()
        .filter(|n| housings.iter().any(|h| n.housings.contains(h)))
    {
        out.push(format!(
            "  net {:?} slot {}: {} housings, weakest tier {}",
            net.kind,
            net.slot + 1,
            net.housings.len(),
            net.min_tier
        ));
    }
    for c in l.power.circuits.iter().filter(|c| c.members.contains(&pos)) {
        out.push(format!(
            "energy circuit: {} members, carries up to {:.1} E/s",
            c.members.len(),
            c.cap
        ));
    }
    if let Some((_, got)) = &l.pipe18_energy
        && let Some(g) = got.get(&pos)
    {
        out.push(format!("got {g:.2} E/s from the magistral at its last count"));
    }
    out
}

// ---- near ----

fn worker_row(game: &Game<'_>, dim: &str, w: &Worker) -> WorkerRow {
    let t = server::tuning();
    WorkerRow {
        id: w.id,
        owner: name_of(w.owner),
        cell: w.cell,
        tool: w.tool.map_or_else(|| "none".to_owned(), |t| format!("{t:?}")),
        charge_percent: 100.0 * w.charge / w.capacity(t).max(1.0),
        hold_used: w.hold.used(),
        hold_size: w.hold.slots.len(),
        errand: format!("{:?}", w.errand),
        note_key: note_key(w.note).0,
        loaded: loaded(game, dim, w.cell),
    }
}

fn near(game: &mut Game<'_>, player: &str, radius: i32) -> Vec<String> {
    let game = &*game;
    let Some(p) = game
        .native_server_players()
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(player))
    else {
        return vec![format!("no player {player} online")];
    };
    let dim = p.dimension.clone();
    let centre = p.position;
    let mut out = vec![format!(
        "{} in {dim} at {:.1} {:.1} {:.1}, radius {radius}",
        p.name, centre[0], centre[1], centre[2]
    )];
    let in_place = dim == game.native_dimension();
    let found = server::with_level(&dim, |l| {
        l.machines
            .iter()
            .filter(|(pos, _)| debug::within(centre, **pos, radius))
            .map(|(pos, m)| {
                let status = in_place.then(|| {
                    let block = game.native_block(*pos);
                    text::status_line(&status_of(game, &dim, l, *pos, &block))
                });
                (*pos, *m, status)
            })
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    out.push(format!("machines: {}", found.len()));
    for (pos, m, status) in found {
        let status = status.unwrap_or_else(|| {
            let last = track().machines.last(&(dim.clone(), pos)).map(str::to_owned);
            format!("last seen {}", last.unwrap_or_else(|| "none".to_owned()))
        });
        out.push(format!("  {} {} {status}", pos_text(pos), m.block()));
    }
    let fleet: Vec<Worker> = workers::fleets()
        .into_iter()
        .filter(|(d, _)| *d == dim)
        .flat_map(|(_, f)| f)
        .filter(|w| debug::within(centre, w.cell, radius))
        .collect();
    out.push(format!("workers: {}", fleet.len()));
    out.extend(fleet.iter().map(|w| format!("  {}", worker_row(game, &dim, w).line())));
    out
}

// ---- workers ----

/// The uuid a word names: a player's name (online or seen this run) or a uuid, dashes or not.
fn owner_of(word: &str) -> Option<u128> {
    let hex: String = word.chars().filter(|c| *c != '-').collect();
    if hex.len() == 32
        && let Ok(u) = u128::from_str_radix(&hex, 16)
    {
        return Some(u);
    }
    track()
        .names
        .iter()
        .find(|(_, n)| n.eq_ignore_ascii_case(word))
        .map(|(u, _)| *u)
}

fn workers_list(game: &mut Game<'_>, owner: Option<&str>) -> Vec<String> {
    let game = &*game;
    let wanted = match owner {
        Some(word) => match owner_of(word) {
            Some(u) => Some(u),
            None => return vec![format!("no player {word} seen this run (give a uuid)")],
        },
        None => None,
    };
    let mut out = Vec::new();
    let mut count = 0;
    for (dim, fleet) in workers::fleets() {
        for w in fleet.iter().filter(|w| wanted.is_none_or(|u| w.owner == u)) {
            count += 1;
            out.push(format!("{dim} {}", worker_row(game, &dim, w).line()));
        }
    }
    out.insert(0, format!("workers: {count}"));
    out
}

// ---- storage ----

/// How wide the packed lists of items run, characters.
const PACK_WIDTH: usize = 180;

fn counted(items: &[(String, u64)]) -> Vec<String> {
    items.iter().map(|(k, n)| format!("{} x{n}", debug::short_key(k))).collect()
}

fn storage_view(game: &mut Game<'_>, pos: Pos, dimension: Option<String>) -> Vec<String> {
    let game = &*game;
    let here = game.native_dimension();
    let dim = dimension.unwrap_or_else(|| here.to_owned());
    // The cell's or terminal's storage, else the one on the item net beside the block (S6).
    let id = storage::storage_at(&dim, pos).or_else(|| {
        server::with_level(&dim, |l| storage::storage_by(&dim, pos, &l.topology)).flatten()
    });
    let Some(id) = id else {
        return vec![format!("{dim} {}: no storage there or on its line", pos_text(pos))];
    };
    let Some((anchor, cells, works, store)) = storage::debug_info(&dim, id) else {
        return vec![format!("storage#{id}: not found on the level")];
    };
    let total: u64 = store.items.values().sum();
    let mut out = vec![format!(
        "storage#{id} anchor {} in {dim}: {cells} cells, works {works}, {} kinds, {total} items, stack module {}",
        pos_text(anchor),
        store.items.len(),
        store.stack_module
    )];
    out.extend(debug::pack(
        "  ",
        &counted(&debug::top_items(&store.items, TOP_ITEMS)),
        PACK_WIDTH,
    ));
    if !store.keep.is_empty() {
        let keep: Vec<(String, u64)> = store.keep.iter().map(|(k, n)| (k.clone(), *n)).collect();
        out.extend(debug::pack("  keep N: ", &counted(&keep), PACK_WIDTH));
    }
    out.extend(orders_lines(&dim, id));
    let line = server::with_level(&dim, |l| orders::debug_line(l, anchor));
    let Some((smart, on_line)) = line else {
        return out;
    };
    out.push(format!("line: orders {smart:?}, {} machines behind its joints", on_line.len()));
    if dim != here {
        out.push(format!("(machine slots are read only in the level in place, {here})"));
        return out;
    }
    for (p, m) in on_line {
        let slots: Vec<String> = machines::entity(game, p)
            .map(|e| {
                (0..e.items.len())
                    .filter_map(|i| machines::slot_of(e, i).map(|(item, n)| format!("{i}:{item} x{n}")))
                    .collect()
            })
            .unwrap_or_default();
        let held = if slots.is_empty() { "empty".to_owned() } else { slots.join(", ") };
        out.push(format!("  {} {}: {held}", pos_text(p), m.block()));
    }
    out
}

/// The storage's orders: each with what it waits for, its reserve and its jobs; then what the
/// orders hold at machines and what rides for them.
fn orders_lines(dim: &str, id: u32) -> Vec<String> {
    let (queues, jobs) = orders::debug_orders(dim);
    let queue: Vec<_> = queues
        .into_iter()
        .filter(|(s, _)| *s == id)
        .flat_map(|(_, q)| q)
        .collect();
    let mut out = vec![format!("orders: {}", queue.len())];
    let mut reserved: BTreeMap<String, u64> = BTreeMap::new();
    for o in &queue {
        out.push(format!(
            "  order#{} {} {}/{}{} waits {:?}",
            o.id,
            o.item,
            o.done,
            o.count,
            if o.keep { " (keep N)" } else { "" },
            o.wait
        ));
        if !o.chain.is_empty() {
            let chain: Vec<String> = o
                .chain
                .iter()
                .map(|(kind, item, now)| format!("{kind}:{item}{}", if *now { "*" } else { "" }))
                .collect();
            out.push(format!("    steps (* runs now): {}", chain.join(" > ")));
        }
        for (k, n) in &o.reserved {
            *reserved.entry(k.clone()).or_insert(0) += n;
        }
        for (_, j) in jobs.iter().filter(|(s, j)| *s == id && j.order == o.id) {
            out.push(format!(
                "    job at {} on {}: {} x{} per run, {} runs, inputs {}",
                pos_text(j.machine),
                j.step.machine,
                j.step.output.0,
                j.step.output.1,
                j.runs,
                j.step
                    .inputs
                    .iter()
                    .map(|(i, n)| format!("{i} x{n}"))
                    .collect::<Vec<_>>()
                    .join(" + ")
            ));
        }
    }
    let reserved: Vec<(String, u64)> = reserved.into_iter().collect();
    out.extend(debug::pack("  reserved: ", &counted(&reserved), PACK_WIDTH));
    let (held, ships) = orders::debug_riding(dim);
    let held: Vec<String> = held
        .iter()
        .filter(|h| h.storage == id)
        .map(|h| format!("{} x{} at {} order#{}", debug::short_key(&h.item), h.count, pos_text(h.machine), h.order))
        .collect();
    out.extend(debug::pack("  held at machines: ", &held, PACK_WIDTH));
    let riding: Vec<String> = ships
        .iter()
        .filter(|s| s.storage == id)
        .flat_map(|s| {
            s.items
                .iter()
                .map(move |(item, n)| format!("{} x{n} order#{} {:?}", debug::short_key(item), s.order, s.leg))
        })
        .collect();
    out.extend(debug::pack("  riding: ", &riding, PACK_WIDTH));
    out
}

// ---- log ----

fn log_tail(n: usize, filter: Option<&str>) -> Vec<String> {
    let (path, pending) = {
        let j = journal_state();
        (j.path.clone(), j.buffer.pending.clone())
    };
    let mut text = path
        .as_deref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    for line in &pending {
        text.push_str(line);
        text.push('\n');
    }
    let mut lines: Vec<String> = journal::tail(text.lines(), n, filter)
        .into_iter()
        .map(str::to_owned)
        .collect();
    // Too few in the file since it rotated: the backup's end comes first.
    if lines.len() < n
        && let Some(older) = path.as_deref().and_then(|p| std::fs::read_to_string(backup(p)).ok())
    {
        let mut both = older;
        both.push_str(&text);
        lines = journal::tail(both.lines(), n, filter)
            .into_iter()
            .map(str::to_owned)
            .collect();
    }
    let place = path.map_or_else(|| "not known yet".to_owned(), |p| p.display().to_string());
    let mut out = vec![format!(
        "journal {place}: {} lines{}",
        lines.len(),
        filter.map_or_else(String::new, |f| format!(" containing \"{f}\""))
    )];
    out.extend(debug::clip_newest(lines));
    out
}
