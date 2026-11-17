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
        self as kit, ENERGY, HOT, LABEL, TEXT, TEXT_ON, VALUE, button, text, tr, tr_with,
    },
    rustcraft_workers::NOTES as WORKERS,
};
use horizon_client::{
    hud::HudBatch,
    mod_api::{NativeScreen, NativeScreenView},
};
use rustcraft_rules::{
    geom::{Face, Pos},
    machines::style::{self, Severity, colour},
    monorail::{
        capsule,
        route::Point,
        window::{self as win, Part, Projection},
        wire::{self, CapsuleNote, Destination, Listing, Reach, Rename},
    },
    worker::{pose as worker_pose, wire as worker_wire},
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const ESCAPE: &str = "key.keyboard.escape";
const ENTER: [&str; 2] = ["key.keyboard.enter", "key.keyboard.keypad.enter"];
const BACKSPACE: &str = "key.keyboard.backspace";
/// The scheme's colours (docs/showcase/ui.css `.w-map`, style «Схемы»): parchment, ink, the brass
/// line of the way; this station an inked dot, the chosen one blue.
const MAP_LINE: u32 = colour::PAPER_LINE;
const DOT_EDGE: u32 = colour::PAPER_INK;
const DOT: u32 = colour::PAPER;
const DOT_HERE: u32 = colour::PAPER_INK;
const DOT_CHOSEN: u32 = colour::PAPER_CHOSEN;
const MAP_TEXT: u32 = colour::PAPER_TEXT;
/// Slice 18: the cartographers' finds on the scheme: a structure brown, a biome green.
const MARK_STRUCTURE: u32 = 0xFF8A_5A2B;
const MARK_BIOME: u32 = 0xFF4F_8A3A;

const BROKEN_TEXT: u32 = colour::BAD_TEXT;
/// The capsule's parts as standalone models.
const BODY: &str = "rustcraft:block/capsule_body";
const BELOW: &str = "rustcraft:block/capsule_below";
const ABOVE: &str = "rustcraft:block/capsule_above";
const SIDE: &str = "rustcraft:block/capsule_side";
/// Slice 14: a zipline's rope, one block long along +z, a texel thick round the axis.
const ROPE: &str = "rustcraft:block/zipline_rope";
/// The harpoon's tip at the hook's end of its rope, pointing along +z.
const HARPOON_TIP: &str = "rustcraft:block/harpoon_tip";
/// The rider's hands over its feet, where the rope runs from.
const ROPE_HANDS: f64 = rustcraft_rules::traversal::ROPE_HANDS;
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
/// Slice 14: zipline and harpoon seats: the harpoon rider's rise and its rope's end; none for a
/// zipline's, whose rope its anchors draw.
static ROPES: LazyLock<Mutex<HashMap<u32, Option<(f64, [f64; 3])>>>> = LazyLock::new(Mutex::default);
/// The workers the server told of, by seat mob: their heading and tool (0 none, 1 saw, 2 reaper).
static WORKER_SEATS: LazyLock<Mutex<HashMap<u32, worker_wire::Seat>>> = LazyLock::new(Mutex::default);
/// The worker's parts as standalone models (assets/models/transport.py `worker`).
/// The open chassis window's panel, as its server sent it last.
static CHASSIS_PANEL: Mutex<Option<worker_wire::Panel>> = Mutex::new(None);
const WORKER_BODY: &str = "rustcraft:block/worker_body";

const WORKER_PAINT: &str = "rustcraft:block/worker_paint";
/// Every part model of the worker (assets/models/export_mc.py `WORKER_PARTS`).
const WORKER_PARTS: [&str; 26] = [
    "body", "paint", "below", "above", "side", "saw", "aim_yaw_saw", "reach_saw", "saw_disc",
    "reaper", "reaper_reel", "hold", "grab", "grab_left", "grab_right", "bore", "bore_bit", "gun",
    "aim_yaw_gun", "aim_pitch_gun", "scoop", "aim_yaw_scoop", "reach_scoop", "scoop_bucket", "map",
    "map_drum",
];

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

fn ropes() -> MutexGuard<'static, HashMap<u32, Option<(f64, [f64; 3])>>> {
    ROPES.lock().unwrap_or_else(PoisonError::into_inner)
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
        let Some(list) = worker_wire::decode(data) else {
            log::warn!("rustcraft: a workers note that does not read");
            return;
        };
        let mut seats = WORKER_SEATS.lock().unwrap_or_else(PoisonError::into_inner);
        seats.clear();
        seats.extend(list.into_iter().map(|s| (s.seat, s)));
    } else if channel == worker_wire::PANEL {
        *CHASSIS_PANEL.lock().unwrap_or_else(PoisonError::into_inner) = worker_wire::Panel::decode(data);
    } else if channel == CAPSULE {
        match wire::decode_capsule(data) {
            Some(CapsuleNote::Ride { seat, rise, points }) => {
                capsules().insert(seat, (rise, points));
            }
            Some(CapsuleNote::Gone { seat }) => {
                capsules().remove(&seat);
                ropes().remove(&seat);
            }
            Some(CapsuleNote::Rope { seat, rise, to }) => {
                ropes().insert(seat, Some((rise, to)));
            }
            Some(CapsuleNote::Seat { seat }) => {
                ropes().insert(seat, None);
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
fn status(listing: &Listing, chosen: Option<&Destination>) -> (String, Severity) {
    if !listing.on_rail {
        return (
            tr("rustcraft.station.no_rail", "The station stands by no rail"),
            Severity::Bad,
        );
    }
    let Some(d) = chosen else {
        return if listing.destinations.is_empty() {
            (
                tr(
                    "rustcraft.station.alone",
                    "No other stations on the network",
                ),
                Severity::Warn,
            )
        } else {
            (
                tr(
                    "rustcraft.station.choose",
                    "Choose a station on the scheme or in the list",
                ),
                Severity::Warn,
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
            Severity::Ok,
        ),
        Reach::Short(short) => (
            tr_with(
                "rustcraft.station.short",
                "Not enough energy: %s E more",
                &[format!("{}", short.ceil())],
            ),
            Severity::Bad,
        ),
        Reach::Broken(at) => (
            tr_with(
                "rustcraft.station.broken",
                "The way is broken at %s",
                &[pos_text(at)],
            ),
            Severity::Bad,
        ),
        Reach::NoRoom(at) => (
            tr_with(
                "rustcraft.station.no_room",
                "No room for the capsule at %s",
                &[pos_text(at)],
            ),
            Severity::Bad,
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
            LABEL,
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
        let rect = win::row_rect(i);
        let r = kit::at(o, rect);
        let on = Some(d.station) == v.chosen;
        if on {
            kit::fills(game, batch, o, &style::raised(rect));
        } else {
            kit::hover(game, batch, o, rect);
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
    let (line, severity) = status(&listing, chosen);
    kit::status_line(game, batch, o, win::STATUS, severity, &line);
    kit::inventory(game, batch, o, win::INVENTORY, win::INVENTORY_LABEL_Y);
}

/// The name field (style «Поля»): a well, «Название:» in the label colour, the name in the value
/// colour, the caret while typing, the focus outlined.
fn name_field(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], listing: &Listing, v: &View) {
    let r = win::NAME;
    let x = style::field_text_x(r, false);
    let key = tr("rustcraft.station.name", "Name: %s");
    let key = key.split("%s").next().unwrap_or_default().to_owned();
    let key_w = game.native_text_width(&key);
    let value = match &v.editing {
        Some(typed) => typed.clone(),
        None => station_name(&listing.name, listing.station),
    };
    let value = fit(game, &value, r[0] + r[2] - x - key_w - 4.0);
    let caret = v
        .editing
        .is_some()
        .then(|| x + key_w + game.native_text_width(&value));
    kit::fills(game, batch, o, &style::field(r, false, caret));
    if v.editing.is_some() {
        kit::fills(game, batch, o, &style::outline(r, HOT));
    } else {
        kit::hover(game, batch, o, r);
    }
    let y = r[1] + ((r[3] - kit::FONT_HEIGHT) / 2.0).round() + 1.0;
    text(game, batch, &key, o, [x, y], LABEL);
    text(game, batch, &value, o, [x + key_w, y], VALUE);
}

/// The scheme (T7): the rails as lines from above, the stations as dots with their names.
fn scheme(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    listing: &Listing,
    chosen: Option<Pos>,
) {
    kit::fills(game, batch, o, &style::paper(win::SCHEME));
    let r = kit::at(o, win::SCHEME);
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
    marks(game, batch, o, r, &p, listing);
    let mut dots: Vec<(Pos, &str, u32)> = vec![(listing.station, &listing.name, DOT_HERE)];
    dots.extend(reachable.iter().map(|d| {
        let fill = if Some(d.station) == chosen {
            DOT_CHOSEN
        } else {
            DOT
        };
        (d.station, d.name.as_str(), fill)
    }));
    // Slice 18: the floors of a lift share a dot; their names (and near stations' names) are
    // set apart, the lowest floor first (rustcraft_rules::monorail::window::place_labels).
    dots.sort_by_key(|d| d.0[1]);
    let mut labels = Vec::with_capacity(dots.len());
    for &(pos, name, fill) in &dots {
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
        labels.push((label, at, w));
    }
    let spots: Vec<([f32; 2], f32)> = labels.iter().map(|(_, at, w)| (*at, *w)).collect();
    for ((label, _, _), xy) in labels.iter().zip(win::place_labels(&spots, r)) {
        game.native_text(batch, label, xy, MAP_TEXT, false);
    }

}

/// Slice 18 (guide `tool_map`): the cartographers' finds near the network, a square for a
/// structure and a smaller one for a biome, each with its name, and the chunks charted.
fn marks(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    r: [f32; 4],
    p: &Projection,
    listing: &Listing,
) {
    for m in &listing.marks {
        let c = p.at(m.at);
        let at = [o[0] + c[0], o[1] + c[1]];
        if !win::inside(r, at) {
            continue;
        }
        let (size, fill) = if m.structure { (5.0, MARK_STRUCTURE) } else { (3.0, MARK_BIOME) };
        let h = (size / 2.0f32).floor();
        game.native_fill(batch, [at[0] - h - 1.0, at[1] - h - 1.0, size + 2.0, size + 2.0], DOT_EDGE);
        game.native_fill(batch, [at[0] - h, at[1] - h, size, size], fill);
        let key = if m.structure {
            format!("rustcraft.structure.{}", m.id.replace(':', "."))
        } else {
            format!("biome.{}", m.id.replace(':', "."))
        };
        let name: String = tr(&key, m.id.rsplit(':').next().unwrap_or(&m.id))
            .chars()
            .take(MAP_LABEL)
            .collect();
        let w = game.native_text_width(&name);
        let x = (at[0] - w / 2.0).clamp(r[0] + 2.0, r[0] + r[2] - w - 2.0);
        let y = (at[1] + 4.0).min(r[1] + r[3] - 10.0);
        game.native_text(batch, &name, [x, y], MAP_TEXT, false);
    }
    if listing.charted > 0 {
        let s = tr_with(
            "rustcraft.station.charted",
            "Charted: %s chunks",
            &[listing.charted.to_string()],
        );
        game.native_text(batch, &s, [r[0] + 3.0, r[1] + r[3] - 10.0], MAP_TEXT, false);
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
        ROPE,
        HARPOON_TIP,
    ] {
        models.push(m.to_owned());
    }
    for part in WORKER_PARTS {
        models.push(format!("rustcraft:block/worker_{part}"));
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
/// before it, then after it, that is not straight up or down; on a stretch straight up or down a
/// wall, along the wall (capsule::along_wall), so the cabin does not ride with its nose in it.
fn heading(points: &[Point], i: usize) -> f32 {
    let yaw_of = |j: usize| -> Option<f32> {
        let (a, b) = (points.get(j)?.at, points.get(j + 1)?.at);
        let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
        (dx.hypot(dz) > 1e-6).then(|| (-dx).atan2(dz).to_degrees() as f32)
    };
    let nearest = (0..=i)
        .rev()
        .find_map(yaw_of)
        .or_else(|| (i..points.len()).find_map(yaw_of))
        .unwrap_or(0.0);
    match points.get(i).map(|p| p.mount) {
        Some(wall @ (Face::XNeg | Face::XPos | Face::ZNeg | Face::ZPos)) if yaw_of(i).is_none() => {
            capsule::along_wall(wall, nearest)
        }
        _ => nearest,
    }
}

/// `native_mob_submit`: a harpoon's seat drawn as its rope, from the rider's hands to the hook
/// over the block it went to with the harpoon's tip there, and the hook's line down to that
/// block; a zipline's seat is drawn as nothing (its anchors draw its rope). The seat itself is
/// never drawn.
pub(super) fn draw_rope(game: &Game<'_>, mob: &NativeMobRender<'_>, draw: &mut BlockEntityDraw, drawn: &mut bool) {
    if *drawn {
        return;
    }
    let Some(seen) = ropes().get(&mob.id).copied() else {
        return;
    };
    let Some((rise, to)) = seen else {
        *drawn = true;
        return;
    };
    let Some(m) = game.mobs.iter().find(|m| m.id == mob.id) else {
        return;
    };
    *drawn = true;
    let t = f64::from(mob.partial);
    let lerp = |a: f64, b: f64| a + (b - a) * t;
    let feet = [lerp(m.previous.x, m.position.x), lerp(m.previous.y, m.position.y), lerp(m.previous.z, m.position.z)];
    let hands = [0.0, rise + ROPE_HANDS, 0.0];
    let hook = [to[0] - feet[0], to[1] + ROPE_HANDS - feet[1], to[2] - feet[2]];
    let ground = [hook[0], to[1] - feet[1], hook[2]];
    draw.push();
    // From the mob's frame (turned with its body) to the world's axes at its feet.
    draw.rotate_about([0.0, 1.0, 0.0], mob.body_yaw, [0.0; 3]);
    rope_bar(game, draw, hands, hook);
    rope_bar(game, draw, hook, ground);
    draw_tip(game, draw, hands, hook);
    draw.pop();
}

/// The harpoon's tip at `b`, pointing on from `a` to `b`.
fn draw_tip(game: &Game<'_>, draw: &mut BlockEntityDraw, a: [f64; 3], b: [f64; 3]) {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    if d.iter().map(|c| c * c).sum::<f64>() < 0.0025 {
        return;
    }
    draw.push();
    turn_to(draw, b, d);
    draw.translate([-0.5, -0.5, 0.0]);
    game.native_draw_model(draw, HARPOON_TIP, 0xFFFFFF);
    draw.pop();
}

/// The pose moved to `at` and turned so +z points along `d`: about y to its heading, then about
/// x up or down to it.
fn turn_to(draw: &mut BlockEntityDraw, at: [f64; 3], d: [f64; 3]) {
    let flat = d[0].hypot(d[2]);
    draw.translate([at[0] as f32, at[1] as f32, at[2] as f32]);
    draw.rotate_about([0.0, 1.0, 0.0], d[0].atan2(d[2]).to_degrees() as f32, [0.0; 3]);
    draw.rotate_about([1.0, 0.0, 0.0], (-d[1].atan2(flat)).to_degrees() as f32, [0.0; 3]);
}

/// One stretch of rope from `a` to `b` (blocks from the pose's origin, world axes); the
/// zipline's anchors draw their rope with it too (rustcraft_zipline).
pub(super) fn rope_bar(game: &Game<'_>, draw: &mut BlockEntityDraw, a: [f64; 3], b: [f64; 3]) {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len = d[0].hypot(d[2]).hypot(d[1]);
    if len < 0.05 {
        return;
    }
    draw.push();
    turn_to(draw, a, d);
    draw.scale([1.0, 1.0, len as f32]);
    draw.translate([-0.5, -0.5, 0.0]);
    game.native_draw_model(draw, ROPE, 0xFFFFFF);
    draw.pop();
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
    // A camera looking out from inside the cabin sees the world, not the cabin's inside (block
    // models have no inner faces to cull): neither the capsule nor the seat is drawn for it. The
    // camera is the player's own eye unless the studio's free camera has the view
    // (`studio_freecam`, game.rs:3100; the engine's own body-in-view rule, game/perspective.rs:
    // 710-712), which LetsCam's director drives in a replay while the replay rides the recorded
    // player in first person (game/studio.rs:762, game/remote_vehicles.rs:212-220).
    let camera = if game.studio_freecam.active {
        let c = game.studio_freecam.position;
        capsule::Camera::Free([c.x, c.y, c.z])
    } else {
        capsule::Camera::Own {
            rides: game.riding == Some(mob.id) && game.controls_camera(),
            first_person: game.camera_type.first_person(),
        }
    };
    if capsule::hidden_from(camera, feet) {
        *drawn = true;
        return;
    }
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
/// `worker`, docs/showcase/js/models.js `M.worker`, poses.js `P.worker`): the chassis painted in
/// its tool's colour, the bracket of the rail it holds (below, above, or beside it turned to the
/// wall) and its tool, whose parts turn to what it works on, slide out and spin while it works
/// (rustcraft_rules::worker::pose), the size of a block, turned to its heading.
pub(super) fn draw_worker(
    game: &Game<'_>,
    mob: &NativeMobRender<'_>,
    draw: &mut BlockEntityDraw,
    drawn: &mut bool,
) {
    if *drawn {
        return;
    }
    let Some(seat) = WORKER_SEATS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&mob.id)
        .copied()
    else {
        return;
    };
    *drawn = true;
    let yaw = seat.yaw;
    // What its tool works on, in the model's frame: from the seat's feet, turned to its heading.
    let aim = seat.aim.and_then(|a| {
        let m = game.mobs.iter().find(|m| m.id == mob.id)?;
        let t = f64::from(mob.partial);
        let feet = [
            m.previous.x + (m.position.x - m.previous.x) * t,
            m.previous.y + (m.position.y - m.previous.y) * t,
            m.previous.z + (m.position.z - m.previous.z) * t,
        ];
        let rel = [
            f64::from(a[0]) - feet[0],
            f64::from(a[1]) - feet[1],
            f64::from(a[2]) - feet[2],
        ];
        Some(worker_pose::point_in_model(rel, f64::from(yaw)))
    });
    let seconds = f64::from(mob.age) / 20.0;
    // Wave 2 (poses.js `glow`): a working worker glows with its work's pulse, an idle one faintly.
    let light = draw.light;
    draw.light = worker_pose::glow_light(light, seat.busy, seconds);
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], mob.body_yaw - yaw, [0.0; 3]);
    draw.rotate_about([0.0, 1.0, 0.0], -90.0, [0.0; 3]);
    draw.translate([-0.5, 0.0, -0.5]);
    game.native_draw_model(draw, WORKER_BODY, 0xFFFFFF);
    game.native_draw_model(draw, WORKER_PAINT, worker_pose::colour(seat.tool));
    let bracket = worker_pose::bracket(seat.mount);
    if matches!(seat.mount, Face::YNeg | Face::YPos) {
        game.native_draw_model(draw, bracket, 0xFFFFFF);
    } else {
        let turn = worker_pose::bracket_turn(seat.mount, f64::from(yaw)) as f32;
        draw.push();
        draw.rotate_about([0.0, 1.0, 0.0], turn, [0.5, 0.5, 0.5]);
        game.native_draw_model(draw, bracket, 0xFFFFFF);
        draw.pop();
    }
    if let Some(tool) = seat.tool {
        for part in worker_pose::tool_parts(tool, seat.busy, seconds, aim) {
            draw.push();
            for op in part.ops {
                match op {
                    worker_pose::Op::Turn { axis, deg, pivot } => {
                        let mut a = [0.0f32; 3];
                        a[usize::from(axis)] = 1.0;
                        draw.rotate_about(a, deg as f32, pivot.map(|v| v as f32));
                    }
                    worker_pose::Op::Move(v) => draw.translate(v.map(|c| c as f32)),
                }
            }
            game.native_draw_model(draw, part.model, 0xFFFFFF);
            draw.pop();
        }
    }
    draw.pop();
    draw.light = light;
}

// ---- The chassis window ----

/// The panel's size and the gap to the menu's slots.
const PANEL_W: f32 = 128.0;
const PANEL_H: f32 = 132.0;
const PANEL_GAP: f32 = 4.0;

/// `native_screen_render_post` (API 78): beside the chassis's server menu, whose slots hold its
/// tool, sockets, state and hold, the rest of the guide's window (`ui.js` `chassis`): the tool's
/// name, «Улучшения и модули» with the sockets in use, the bars «Заряд» and «Трюм», the filter
/// and the status line. Told apart from other menus by its title, one of the chassis's three.
pub(super) fn draw_chassis(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    let Some(title) = screen.title.map(|t| t.plain()) else {
        return;
    };
    let ours = (1..=3u8).any(|tier| {
        let (key, fallback) = worker_wire::window_key(tier);
        tr(key, fallback) == title
    });
    if !ours {
        return;
    }
    let Some(p) = CHASSIS_PANEL
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    else {
        return;
    };
    let [x, y, w, _] = screen.rect;
    let o = [(x + w + PANEL_GAP).round(), y.round()];
    kit::background(game, batch, o, [PANEL_W, PANEL_H]);
    let inner = PANEL_W - 12.0;
    let mut row = 6.0;
    let (tool_key, tool_fallback) = worker_wire::tool_key(p.tool);
    kit::label(game, batch, &tr("rustcraft.worker.tool_slot", "Tool"), o, [6.0, row]);
    row += 10.0;
    text(game, batch, &fit(game, &tr(tool_key, tool_fallback), inner), o, [6.0, row], VALUE);
    row += 13.0;
    kit::label(
        game,
        batch,
        &fit(game, &tr("rustcraft.worker.sockets_title", "Upgrades and modules"), inner),
        o,
        [6.0, row],
    );
    row += 10.0;
    let sockets = tr_with(
        "rustcraft.worker.sockets",
        "%s of %s sockets",
        &[p.sockets.0.to_string(), p.sockets.1.to_string()],
    );
    text(game, batch, &sockets, o, [6.0, row], VALUE);
    row += 13.0;
    let charge = format!("{:.0}%", 100.0 * p.charge);
    gauge(game, batch, o, row, &tr("rustcraft.worker.charge_title", "Charge"), &charge, p.charge, ENERGY);
    row += 22.0;
    let hold_share = p.hold.0 as f64 / (p.hold.1.max(1)) as f64;
    let hold = tr_with(
        "rustcraft.worker.hold_slots",
        "%s of %s slots",
        &[p.hold.0.to_string(), p.hold.1.to_string()],
    );
    gauge(game, batch, o, row, &tr("rustcraft.worker.hold_title", "Hold"), &hold, hold_share, HOT);
    row += 22.0;
    let filter = if p.filter.is_empty() {
        tr("rustcraft.worker.filter_all", "Filter: takes everything")
    } else {
        let names: Vec<String> = p.filter.iter().map(|f| item_name(f)).collect();
        tr_with("rustcraft.worker.filter", "Filter: %s", &[names.join(", ")])
    };
    text(game, batch, &fit(game, &filter, inner), o, [6.0, row], TEXT);
    row += 12.0;
    let (key, fallback, mood) = worker_wire::note_key(p.note);
    let severity = match mood {
        worker_wire::Mood::Fine => Severity::Ok,
        worker_wire::Mood::Waiting => Severity::Warn,
        worker_wire::Mood::Stopped => Severity::Bad,
    };
    kit::status_line(game, batch, o, [4.0, row, PANEL_W - 8.0, 14.0], severity, &tr(key, fallback));
}

/// A labelled bar: the label left, the value right, the bar under them.
#[allow(clippy::too_many_arguments)]
fn gauge(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    row: f32,
    label: &str,
    value: &str,
    share: f64,
    fill: u32,
) {
    kit::label(game, batch, label, o, [6.0, row]);
    kit::text_right(game, batch, value, o, PANEL_W - 6.0, row, VALUE);
    kit::bar(
        game,
        batch,
        [o[0] + 6.0, o[1] + row + 10.0, PANEL_W - 12.0, 6.0],
        share,
        fill,
    );
}

/// An item's or block's name in the player's language.
fn item_name(id: &str) -> String {
    let path = id.replace(':', ".");
    let block = format!("block.{path}");
    let named = tr(&block, "");
    if named.is_empty() || named == block {
        tr(&format!("item.{path}"), id.rsplit(':').next().unwrap_or(id))
    } else {
        named
    }
}
