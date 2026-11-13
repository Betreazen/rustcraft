// The monorail on a Horizon client: the station's window and the capsule.
//
// The window (docs/showcase/js/ui.js, window `station`; T7, T8.a, T8.b) draws the listing the
// server sends while it is open (rustcraft_monorail, `native_net_from_server`): the station's name
// in a field the player types into, the scheme of its rail network seen from above with the
// stations as dots (this one black, the chosen one blue), a list of the stations with distance
// and time, pages of three, the station's energy, «Ехать», and a status line with the chosen
// trip's cost or why there is none. Choosing a station (a dot or a row) and the pages are this
// window's own; «Ехать» is a menu button naming the station by its place in the listing
// (rustcraft_rules::monorail::window::go_button). The name is typed as the terminal's search is
// (a mod screen has no text box, game/native_text_field.rs:80-150): while the field is focused
// `native_screen_key_pressed_pre` and `_char_typed_pre` take the keys, Enter sends the name to
// the server (`native_net_send_to_server`, API 40) and Escape drops it.
//
// The capsule: the server spawns a pig as the seat and tells every client its id and the corners
// of its way (rustcraft_rules::monorail::wire::CapsuleNote). `native_mob_submit` (game/
// native_mob_draw.rs, API 110) then draws the guide's capsule (assets/models/transport.py
// `capsule`, exported as the standalone models capsule_body, _below, _above and _side, API 151)
// round that mob instead of the pig: MODEL_SCALE times the guide's cell, the cabin level and
// turned to where the way goes (T3), the bracket for the rail it holds (below a floor rail, above
// a ceiling rail, beside a wall rail and turned to the wall). Being the mob itself, it moves as
// smoothly as the server's mob and its rider do.
use crate::game::{
    Game,
    native_block_entity_draw::BlockEntityDraw,
    native_mob_draw::NativeMobRender,
    rustcraft_content as content,
    rustcraft_monorail::{CAPSULE, LISTING, RENAME},
    rustcraft_screens::{
        self as kit, BUTTON_ON, ENERGY, FONT_HEIGHT, LABEL, PANEL_LIGHT, SLOT, SLOT_EDGE,
        STATUS_BAD, STATUS_OK, STATUS_TEXT, STATUS_WARN, TEXT, TEXT_ON, button, text, tr, tr_with,
    },
    rustcraft_workers::NOTES as WORKERS,
};
use horizon_client::{
    hud::HudBatch,
    mod_api::{NativeScreen, NativeScreenView},
};
use rustcraft_rules::{
    geom::{Face, Pos},
    monorail::{
        capsule,
        route::Point,
        window::{self as win, Part, Projection},
        wire::{self, CapsuleNote, Destination, Listing, Reach, Rename},
    },
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const ESCAPE: &str = "key.keyboard.escape";
const ENTER: [&str; 2] = ["key.keyboard.enter", "key.keyboard.keypad.enter"];
const BACKSPACE: &str = "key.keyboard.backspace";
/// The scheme's colours (docs/showcase/ui.css `.w-map`).
const MAP_BACK: u32 = 0xFFF1F3F7;
const MAP_EDGE: u32 = 0xFF5D6470;
const MAP_LINE: u32 = 0xFFC57F52;
const DOT_EDGE: u32 = 0xFF1A1D25;
const DOT: u32 = 0xFFF1F3F7;
const DOT_HERE: u32 = 0xFF1A1D25;
const DOT_CHOSEN: u32 = 0xFF56DBE2;
const MAP_TEXT: u32 = 0x1A1D25;
const BROKEN_TEXT: u32 = 0xB0382E;
/// The capsule's parts as standalone models.
const BODY: &str = "rustcraft:block/capsule_body";
const BELOW: &str = "rustcraft:block/capsule_below";
const ABOVE: &str = "rustcraft:block/capsule_above";
const SIDE: &str = "rustcraft:block/capsule_side";
/// A label on the scheme is cut to this many characters.
const MAP_LABEL: usize = 10;

/// The window's own state for the station it is open on.
#[derive(Default)]
struct View {
    station: Option<Pos>,
    chosen: Option<Pos>,
    page: usize,
    /// The name being typed, while the field is focused.
    editing: Option<String>,
}

static VIEW: LazyLock<Mutex<View>> = LazyLock::new(Mutex::default);
static LISTING_NOW: Mutex<Option<Listing>> = Mutex::new(None);
/// A name typed and kept with Enter, until the client's next tick sends it: the key hook holds
/// the game only to read it, and sending takes it whole.
static RENAME_DUE: Mutex<Option<Rename>> = Mutex::new(None);
/// The capsules the server told of, by seat mob: the rider's rise and the way's corners.
static CAPSULES: LazyLock<Mutex<HashMap<u32, (f64, Vec<Point>)>>> = LazyLock::new(Mutex::default);
/// The workers the server told of, by seat mob: their heading and tool (0 none, 1 saw, 2 reaper).
static WORKER_SEATS: LazyLock<Mutex<HashMap<u32, (f32, u8)>>> = LazyLock::new(Mutex::default);
/// The worker's parts as standalone models (assets/models/transport.py `worker`).
const WORKER_BODY: &str = "rustcraft:block/worker_body";
const WORKER_BELOW: &str = "rustcraft:block/worker_below";
const WORKER_SAW: &str = "rustcraft:block/worker_saw";
const WORKER_REAPER: &str = "rustcraft:block/worker_reaper";

fn view_for(game: &Game<'_>) -> MutexGuard<'static, View> {
    let pos = game.native_screen_pos();
    let mut v = VIEW.lock().unwrap_or_else(PoisonError::into_inner);
    if v.station != pos {
        *v = View {
            station: pos,
            ..View::default()
        };
    }
    v
}

fn listing_of(station: Option<Pos>) -> Option<Listing> {
    let l = LISTING_NOW.lock().unwrap_or_else(PoisonError::into_inner);
    l.as_ref().filter(|l| Some(l.station) == station).cloned()
}

fn capsules() -> MutexGuard<'static, HashMap<u32, (f64, Vec<Point>)>> {
    CAPSULES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_net_from_server`: a station's listing or a capsule.
pub(super) fn on_net(channel: &str, data: &[u8]) {
    if channel == LISTING {
        match wire::decode_listing(data) {
            Some(l) => *LISTING_NOW.lock().unwrap_or_else(PoisonError::into_inner) = Some(l),
            None => log::warn!("rustcraft: a station listing that does not read"),
        }
    } else if channel == WORKERS {
        let mut seats = WORKER_SEATS.lock().unwrap_or_else(PoisonError::into_inner);
        seats.clear();
        for c in data.chunks_exact(9) {
            let seat = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            let yaw = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
            seats.insert(seat, (yaw, c[8]));
        }
    } else if channel == CAPSULE {
        match wire::decode_capsule(data) {
            Some(CapsuleNote::Ride { seat, rise, points }) => {
                capsules().insert(seat, (rise, points));
            }
            Some(CapsuleNote::Gone { seat }) => {
                capsules().remove(&seat);
            }
            None => log::warn!("rustcraft: a capsule note that does not read"),
        }
    }
}

pub(super) fn layout() -> NativeScreen {
    NativeScreen {
        width: win::WIDTH,
        height: win::HEIGHT,
        slots: vec![None],
        inventory_y: win::INVENTORY[1],
        inventory_x: win::INVENTORY[0],
        own_labels: true,
        own_background: true,
        ..NativeScreen::default()
    }
}

// ---- Texts ----

fn station_name(name: &str, pos: Pos) -> String {
    if name.is_empty() {
        tr_with(
            "rustcraft.station.unnamed",
            "Station %s, %s",
            &[pos[0].to_string(), pos[2].to_string()],
        )
    } else {
        name.to_owned()
    }
}

fn pos_text(p: Pos) -> String {
    format!("{} {} {}", p[0], p[1], p[2])
}

fn number(v: f64) -> String {
    let rounded = (v * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}").replace('.', ",")
    }
}

/// A row of the list.
fn row_text(d: &Destination) -> String {
    let name = station_name(&d.name, d.station);
    match d.reach {
        Reach::Broken(_) => tr_with(
            "rustcraft.station.broken_row",
            "%s · the way is broken",
            &[name],
        ),
        Reach::NoRoom(_) => tr_with("rustcraft.station.no_room_row", "%s · no room", &[name]),
        _ => tr_with(
            "rustcraft.station.row",
            "%s · %s · %s s",
            &[
                name,
                tr_with(
                    "rustcraft.window.blocks_short",
                    "%s bl",
                    &[d.length.to_string()],
                ),
                number(f64::from(d.ticks) / 20.0),
            ],
        ),
    }
}

/// The status line for the chosen destination, and its colour.
fn status(listing: &Listing, chosen: Option<&Destination>) -> (String, u32) {
    if !listing.on_rail {
        return (
            tr("rustcraft.station.no_rail", "The station stands by no rail"),
            STATUS_BAD,
        );
    }
    let Some(d) = chosen else {
        return if listing.destinations.is_empty() {
            (
                tr(
                    "rustcraft.station.alone",
                    "No other stations on the network",
                ),
                STATUS_WARN,
            )
        } else {
            (
                tr(
                    "rustcraft.station.choose",
                    "Choose a station on the scheme or in the list",
                ),
                STATUS_WARN,
            )
        };
    };
    match d.reach {
        Reach::Ready => (
            tr_with(
                "rustcraft.station.cost",
                "%s · the trip costs %s E",
                &[station_name(&d.name, d.station), number(d.cost)],
            ),
            STATUS_OK,
        ),
        Reach::Short(short) => (
            tr_with(
                "rustcraft.station.short",
                "Not enough energy: %s E more",
                &[format!("{}", short.ceil())],
            ),
            STATUS_BAD,
        ),
        Reach::Broken(at) => (
            tr_with(
                "rustcraft.station.broken",
                "The way is broken at %s",
                &[pos_text(at)],
            ),
            STATUS_BAD,
        ),
        Reach::NoRoom(at) => (
            tr_with(
                "rustcraft.station.no_room",
                "No room for the capsule at %s",
                &[pos_text(at)],
            ),
            STATUS_BAD,
        ),
    }
}

/// `s` cut with «…» to fit `width`.
fn fit(game: &Game<'_>, s: &str, width: f32) -> String {
    if game.native_text_width(s) <= width {
        return s.to_owned();
    }
    let mut out: String = s.to_owned();
    while !out.is_empty() && game.native_text_width(&format!("{out}…")) > width {
        out.pop();
    }
    format!("{out}…")
}

// ---- Drawing ----

/// `native_screen_draw` on a station.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2]) {
    let mut v = view_for(game);
    let listing = listing_of(v.station);
    kit::background(game, batch, o, [win::WIDTH, win::HEIGHT]);
    let Some(listing) = listing else {
        text(
            game,
            batch,
            &tr("rustcraft.station.loading", "Waiting for the station…"),
            o,
            [win::NAME[0], win::NAME[1] + 2.0],
            TEXT,
        );
        kit::inventory(game, batch, o, win::INVENTORY, win::INVENTORY_LABEL_Y);
        return;
    };
    name_field(game, batch, o, &listing, &v);
    if v.chosen
        .is_some_and(|c| !listing.destinations.iter().any(|d| d.station == c))
    {
        v.chosen = None;
    }
    let chosen = v
        .chosen
        .and_then(|c| listing.destinations.iter().find(|d| d.station == c));
    scheme(game, batch, o, &listing, v.chosen);
    let pages = win::pages(listing.destinations.len());
    v.page = v.page.min(pages - 1);
    for (i, d) in listing
        .destinations
        .iter()
        .skip(v.page * win::ROWS)
        .take(win::ROWS)
        .enumerate()
    {
        let r = kit::at(o, win::row_rect(i));
        let on = Some(d.station) == v.chosen;
        if on {
            game.native_fill(batch, r, BUTTON_ON);
        }
        let broken = matches!(d.reach, Reach::Broken(_) | Reach::NoRoom(_));
        let colour = if on {
            TEXT_ON
        } else if broken {
            BROKEN_TEXT
        } else {
            TEXT
        };
        let line = fit(game, &row_text(d), r[2] - 6.0);
        game.native_text(batch, &line, [r[0] + 3.0, r[1] + 1.0], colour, false);
    }
    button(game, batch, o, win::PREV, "<", false);
    button(game, batch, o, win::NEXT, ">", false);
    let energy = tr_with(
        "rustcraft.station.energy",
        "Energy stored: %s of %s E",
        &[number(listing.energy.floor()), number(listing.capacity)],
    );
    let energy = fit(game, &energy, win::GO[0] - 12.0);
    text(game, batch, &energy, o, [8.0, win::ENERGY_LABEL_Y], LABEL);
    kit::bar(
        game,
        batch,
        kit::at(o, win::ENERGY),
        if listing.capacity > 0.0 {
            listing.energy / listing.capacity
        } else {
            0.0
        },
        ENERGY,
    );
    let ready = chosen.is_some_and(|d| d.reach == Reach::Ready);
    button(
        game,
        batch,
        o,
        win::GO,
        &tr("rustcraft.station.go", "Go"),
        ready,
    );
    let (line, colour) = status(&listing, chosen);
    let r = kit::at(o, win::STATUS);
    game.native_fill(batch, r, colour);
    let line = fit(game, &line, r[2] - 8.0);
    let y = r[1] + (r[3] - FONT_HEIGHT) / 2.0 + 1.0;
    game.native_text(batch, &line, [r[0] + 4.0, y], STATUS_TEXT, false);
    kit::inventory(game, batch, o, win::INVENTORY, win::INVENTORY_LABEL_Y);
}

fn name_field(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], listing: &Listing, v: &View) {
    let r = kit::at(o, win::NAME);
    game.native_fill(batch, r, SLOT_EDGE);
    let face = if v.editing.is_some() {
        PANEL_LIGHT
    } else {
        SLOT
    };
    game.native_fill(
        batch,
        [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
        face,
    );
    let shown = match &v.editing {
        Some(typed) => format!("{typed}▏"),
        None => station_name(&listing.name, listing.station),
    };
    let line = tr_with("rustcraft.station.name", "Name: %s", &[shown]);
    let line = fit(game, &line, r[2] - 6.0);
    game.native_text(batch, &line, [r[0] + 3.0, r[1] + 2.0], TEXT, false);
}

/// The scheme (T7): the rails as lines from above, the stations as dots with their names.
fn scheme(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    listing: &Listing,
    chosen: Option<Pos>,
) {
    let r = kit::at(o, win::SCHEME);
    game.native_fill(batch, r, MAP_EDGE);
    game.native_fill(
        batch,
        [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
        MAP_BACK,
    );
    let reachable: Vec<&Destination> = listing
        .destinations
        .iter()
        .filter(|d| d.length > 0)
        .collect();
    let p = projection(listing);
    for s in &listing.segments {
        let a = p.at([s[0], s[1]]);
        let b = p.at([s[2], s[3]]);
        let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
        let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
        game.native_fill(
            batch,
            [
                o[0] + x0 - 1.0,
                o[1] + y0 - 1.0,
                x1 - x0 + 2.0,
                y1 - y0 + 2.0,
            ],
            MAP_LINE,
        );
    }
    let mut dots: Vec<(Pos, &str, u32)> = vec![(listing.station, &listing.name, DOT_HERE)];
    dots.extend(reachable.iter().map(|d| {
        let fill = if Some(d.station) == chosen {
            DOT_CHOSEN
        } else {
            DOT
        };
        (d.station, d.name.as_str(), fill)
    }));
    for (pos, name, fill) in dots {
        let c = p.at(win::column(pos));
        let at = [o[0] + c[0], o[1] + c[1]];
        game.native_fill(batch, [at[0] - 3.0, at[1] - 3.0, 7.0, 7.0], DOT_EDGE);
        game.native_fill(batch, [at[0] - 2.0, at[1] - 2.0, 5.0, 5.0], fill);
        let label: String = if name.is_empty() {
            format!("{}, {}", pos[0], pos[2])
        } else {
            name.chars().take(MAP_LABEL).collect()
        };
        let w = game.native_text_width(&label);
        let x = (at[0] - w / 2.0).clamp(r[0] + 2.0, r[0] + r[2] - w - 2.0);
        let y = (at[1] - 12.0).max(r[1] + 2.0);
        game.native_text(batch, &label, [x, y], MAP_TEXT, false);
    }
}

fn projection(listing: &Listing) -> Projection {
    let mut points: Vec<[i32; 2]> = listing
        .segments
        .iter()
        .flat_map(|s| [[s[0], s[1]], [s[2], s[3]]])
        .collect();
    points.push(win::column(listing.station));
    points.extend(
        listing
            .destinations
            .iter()
            .filter(|d| d.length > 0)
            .map(|d| win::column(d.station)),
    );
    Projection::fit(points)
}

// ---- Clicks and keys ----

/// `native_screen_click` on a station: «Ехать» is a menu button; the rest is the window's own.
pub(super) fn on_click(game: &Game<'_>, at: [f32; 2]) -> Option<i32> {
    let part = win::part_at(at);
    let mut v = view_for(game);
    let listing = listing_of(v.station)?;
    if part != Some(Part::Name) {
        v.editing = None;
    }
    match part? {
        Part::Name => {
            if v.editing.is_none() {
                v.editing = Some(listing.name.clone());
            }
            None
        }
        Part::Scheme => {
            let p = projection(&listing);
            let hit = listing
                .destinations
                .iter()
                .filter(|d| d.length > 0)
                .map(|d| {
                    let c = p.at(win::column(d.station));
                    (d.station, (c[0] - at[0]).hypot(c[1] - at[1]))
                })
                .filter(|(_, dist)| *dist <= win::DOT_REACH)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((station, _)) = hit {
                v.chosen = Some(station);
            }
            None
        }
        Part::Row(i) => {
            if let Some(d) = listing.destinations.get(v.page * win::ROWS + i) {
                v.chosen = Some(d.station);
            }
            None
        }
        Part::Prev => {
            v.page = v.page.saturating_sub(1);
            None
        }
        Part::Next => {
            v.page = (v.page + 1).min(win::pages(listing.destinations.len()) - 1);
            None
        }
        Part::Go => {
            let index = listing
                .destinations
                .iter()
                .position(|d| Some(d.station) == v.chosen && d.reach == Reach::Ready)?;
            Some(win::go_button(index))
        }
    }
}

fn is_station(screen: &NativeScreenView<'_>) -> bool {
    screen.menu == Some(content::STATION)
}

/// `native_screen_key_pressed_pre`: while the name is being typed it takes the keys.
pub(super) fn on_key(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    key: &str,
    canceled: &mut bool,
) {
    if *canceled || !is_station(screen) {
        return;
    }
    let mut v = view_for(game);
    let Some(typed) = v.editing.as_mut() else {
        return;
    };
    *canceled = true;
    if key == BACKSPACE {
        typed.pop();
    } else if ENTER.contains(&key) {
        let name = std::mem::take(typed);
        v.editing = None;
        let (Some(station), Some(name)) = (v.station, wire::clean_name(&name)) else {
            return;
        };
        *RENAME_DUE.lock().unwrap_or_else(PoisonError::into_inner) = Some(Rename { station, name });
    } else if key == ESCAPE {
        v.editing = None;
    }
}

/// `native_tick` on a client of a server: a name kept with Enter goes to the server.
pub(super) fn on_tick(game: &mut Game<'_>) {
    if game.remote.is_none() {
        return;
    }
    let Some(rename) = RENAME_DUE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
    else {
        return;
    };
    if let Err(e) = game.native_net_send_to_server(RENAME, &wire::encode_rename(&rename)) {
        log::warn!("rustcraft: the station's name not sent: {e}");
    }
}

/// `native_screen_char_typed_pre`: a character typed into the name.
pub(super) fn on_char(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if *canceled || !is_station(screen) {
        return;
    }
    let mut v = view_for(game);
    let Some(typed) = v.editing.as_mut() else {
        return;
    };
    *canceled = true;
    if typed.chars().count() < wire::NAME_MAX && !c.is_control() {
        typed.push(c);
    }
}

/// `native_screen_tooltip` on a station.
pub(super) fn tooltip(game: &Game<'_>, at: [f32; 2], lines: &mut Vec<String>) {
    let v = view_for(game);
    let Some(listing) = listing_of(v.station) else {
        return;
    };
    match win::part_at(at) {
        Some(Part::Name) => lines.push(tr(
            "rustcraft.station.name_hint",
            "Click and type; Enter keeps the name, Esc drops it",
        )),
        Some(Part::Scheme) => lines.push(tr(
            "rustcraft.station.scheme_hint",
            "The rail network from above",
        )),
        Some(Part::Row(i)) => {
            if let Some(d) = listing.destinations.get(v.page * win::ROWS + i) {
                lines.push(row_text(d));
                if d.reach != Reach::Ready {
                    lines.push(format!("§7{}", status(&listing, Some(d)).0));
                }
            }
        }
        _ if win::inside(win::ENERGY, at) || win::inside(win::STATUS, at) => {
            let chosen = v
                .chosen
                .and_then(|c| listing.destinations.iter().find(|d| d.station == c));
            if win::inside(win::STATUS, at) {
                lines.push(status(&listing, chosen).0);
            } else {
                lines.push(tr(
                    "rustcraft.station.energy_hint",
                    "Energy the station keeps for trips",
                ));
            }
        }
        _ => {}
    }
}

// ---- The capsule ----

/// `native_standalone_models` (API 151): the capsule's parts.
pub(super) fn standalone_models(models: &mut Vec<String>) {
    for m in [
        BODY,
        BELOW,
        ABOVE,
        SIDE,
        WORKER_BODY,
        WORKER_BELOW,
        WORKER_SAW,
        WORKER_REAPER,
    ] {
        models.push(m.to_owned());
    }
}

/// The way's point nearest to `feet`: the segment it lies on and how far along it.
fn segment_at(points: &[Point], feet: [f64; 3]) -> Option<usize> {
    if points.len() < 2 {
        return (!points.is_empty()).then_some(0);
    }
    let mut best = (0, f64::INFINITY);
    for (i, w) in points.windows(2).enumerate() {
        let (a, b) = (w[0].at, w[1].at);
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let len2 = ab.iter().map(|c| c * c).sum::<f64>();
        let t = if len2 > 0.0 {
            ((0..3).map(|k| (feet[k] - a[k]) * ab[k]).sum::<f64>() / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d2 = (0..3)
            .map(|k| (feet[k] - (a[k] + ab[k] * t)).powi(2))
            .sum::<f64>();
        if d2 < best.1 {
            best = (i, d2);
        }
    }
    Some(best.0)
}

/// The heading (vanilla's yaw: 0 south, 90 west) of segment `i`, or of the nearest segment
/// before it, then after it, that is not straight up or down.
fn heading(points: &[Point], i: usize) -> f32 {
    let yaw_of = |j: usize| -> Option<f32> {
        let (a, b) = (points.get(j)?.at, points.get(j + 1)?.at);
        let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
        (dx.hypot(dz) > 1e-6).then(|| (-dx).atan2(dz).to_degrees() as f32)
    };
    (0..=i)
        .rev()
        .find_map(yaw_of)
        .or_else(|| (i..points.len()).find_map(yaw_of))
        .unwrap_or(0.0)
}

/// `native_mob_submit`: a capsule's seat drawn as the guide's capsule.
pub(super) fn draw_capsule(
    game: &Game<'_>,
    mob: &NativeMobRender<'_>,
    draw: &mut BlockEntityDraw,
    drawn: &mut bool,
) {
    if *drawn {
        return;
    }
    let Some((rise, points)) = capsules().get(&mob.id).cloned() else {
        return;
    };
    // Its own rider looking out in first person sees the world, not the cabin's inside (block
    // models have no inner faces to cull): neither the capsule nor the seat is drawn for them.
    if game.riding == Some(mob.id) && game.camera_type.first_person() {
        *drawn = true;
        return;
    }
    let Some(m) = game.mobs.iter().find(|m| m.id == mob.id) else {
        return;
    };
    let t = f64::from(mob.partial);
    let lerp = |a: f64, b: f64| a + (b - a) * t;
    let feet = [
        lerp(m.previous.x, m.position.x),
        lerp(m.previous.y, m.position.y) + rise,
        lerp(m.previous.z, m.position.z),
    ];
    let Some(i) = segment_at(&points, feet) else {
        return;
    };
    let mount = points[i].mount;
    let yaw = heading(&points, i);
    *drawn = true;
    let k = capsule::block_per_pixel() * 16.0;
    draw.push();
    // From the mob's frame (its feet, turned with its body: +z forward, +x left) to one turned to
    // the heading, then to the model's: its floor FEET_OVER_FLOOR under the feet, its middle on
    // them, its front (+x) forward.
    draw.rotate_about([0.0, 1.0, 0.0], mob.body_yaw - yaw, [0.0; 3]);
    draw.translate([0.0, (rise - capsule::FEET_OVER_FLOOR) as f32, 0.0]);
    draw.rotate_about([0.0, 1.0, 0.0], -90.0, [0.0; 3]);
    draw.scale([k as f32; 3]);
    draw.translate([-0.5, 0.0, -0.5]);
    game.native_draw_model(draw, BODY, 0xFFFFFF);
    match mount {
        Face::YNeg => {
            game.native_draw_model(draw, BELOW, 0xFFFFFF);
        }
        Face::YPos => {
            game.native_draw_model(draw, ABOVE, 0xFFFFFF);
        }
        wall => side_bracket(game, draw, wall, yaw, k),
    }
    draw.pop();
}

/// The side bracket turned from the cabin's left to the wall, lowered and pushed in to reach the
/// rail (capsule::side_bracket_shift). Drawn in the model's frame: x forward, -z the cabin's left.
fn side_bracket(game: &Game<'_>, draw: &mut BlockEntityDraw, wall: Face, yaw: f32, k: f64) {
    let o = wall.offset();
    let (sin, cos) = f64::from(yaw).to_radians().sin_cos();
    let forward = [-sin, cos];
    let left = [cos, sin];
    let w = [f64::from(o[0]), f64::from(o[2])];
    let along = w[0] * forward[0] + w[1] * forward[1];
    let leftward = w[0] * left[0] + w[1] * left[1];
    // In the model's frame the wall is at (along, -leftward); the bracket points to -z.
    let angle = (-along).atan2(leftward).to_degrees() as f32;
    let (inward, down) = capsule::side_bracket_shift();
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], angle, [0.5, 0.0, 0.5]);
    draw.translate([0.0, -(down / k) as f32, (inward / k) as f32]);
    game.native_draw_model(draw, SIDE, 0xFFFFFF);
    draw.pop();
}

// ---- The worker ----

/// `native_mob_submit`: a worker's seat drawn as the guide's worker (assets/models/transport.py
/// `worker`): the chassis on its floor bracket and its tool in front, the size of a block, turned
/// to its heading. The tools stand still: the guide's turning disc and reel need a renderer of
/// their own.
pub(super) fn draw_worker(
    game: &Game<'_>,
    mob: &NativeMobRender<'_>,
    draw: &mut BlockEntityDraw,
    drawn: &mut bool,
) {
    if *drawn {
        return;
    }
    let Some((yaw, tool)) = WORKER_SEATS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&mob.id)
        .copied()
    else {
        return;
    };
    *drawn = true;
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], mob.body_yaw - yaw, [0.0; 3]);
    draw.rotate_about([0.0, 1.0, 0.0], -90.0, [0.0; 3]);
    draw.translate([-0.5, 0.0, -0.5]);
    game.native_draw_model(draw, WORKER_BODY, 0xFFFFFF);
    game.native_draw_model(draw, WORKER_BELOW, 0xFFFFFF);
    match tool {
        1 => {
            game.native_draw_model(draw, WORKER_SAW, 0xFFFFFF);
        }
        2 => {
            game.native_draw_model(draw, WORKER_REAPER, 0xFFFFFF);
        }
        _ => {}
    }
    draw.pop();
}
