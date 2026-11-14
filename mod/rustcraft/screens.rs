// The machines' own windows on a Horizon client: each machine's menu type
// (rustcraft_content::Machine::menu) shown through `native_screen` (its layout, asked on the
// client and the server alike: a slot it leaves out is not in the menu, native_hooks.rs:
// 3742-3748) and `native_screen_draw` (native_hooks.rs:3750-3764), drawn in the guide's style
// (docs/showcase/js/ui.js, docs/showcase/ui.css `.win`). This module draws the one-input
// machines' window (ui.js `machine`: input, progress arrow, output, the energy bar with
// «получает X из Y Е/с», the three work modes as buttons, the empty module slots and the status
// line) and holds the drawing kit; rustcraft_panels draws the generator's, the alloy smelter's
// and the autocrafter's, rustcraft_fluid_panels the tank's and the pump's, rustcraft_terminal the
// storage terminal's. Everything a window shows comes from the menu's data slots
// (rustcraft_rules::machines::panel and ::windows), 16-bit as vanilla's. A press on a button is
// a menu button (`native_screen_click`, native_hooks.rs:4246-4258), which the server hears as
// `native_menu_button` (rustcraft_machines::on_menu_button). The client predicts nothing: a
// button lights when the server's next data slots say so (SRC/docs/mods.md:392).
//
// Texts come from the content mod's lang files through `horizon_client::lang::get_or`, the
// table the client's own screens read (game/stations.rs:542-544); the English fallbacks are for
// a client without the content mod.
use crate::game::{
    Game,
    rustcraft_content::{self as content, Kind, Machine},
    rustcraft_fluid_panels as fluid_panels, rustcraft_panels as panels, rustcraft_sheets as sheets,
    rustcraft_station as station, rustcraft_terminal as terminal,
};
use horizon_client::{hud::HudBatch, mod_api::NativeScreen};
use rustcraft_rules::machines::{
    panel::{self, Panel, Rect},
    processor::{Shown, WorkMode},
    style::{self, Fill, Severity, colour as C},
};

// The «Кузня душ» window colours the windows name (docs/design/04-window-style.md, section 2);
// rustcraft_rules::machines::style::colour holds every token, and the fills take theirs there.
pub(super) const HOT: u32 = C::HOT;
pub(super) const PROGRESS: u32 = C::PROGRESS;
pub(super) const ENERGY: u32 = C::ENERGY;
pub(super) const FLAME: u32 = C::FLAME;
pub(super) const STATUS_OK: u32 = C::OK;
// Text colours, 0xRRGGBB.
pub(super) const LABEL: u32 = C::LABEL;
pub(super) const TEXT: u32 = C::TEXT;
pub(super) const VALUE: u32 = C::VALUE;
pub(super) const TEXT_ON: u32 = C::ON_TEXT;
pub(super) const STATUS_TEXT: u32 = C::VALUE;
/// The font's line height, for centring text in a box.
pub(super) const FONT_HEIGHT: f32 = 8.0;

pub(super) fn tr(key: &str, fallback: &str) -> String {
    horizon_client::lang::get_or(key, fallback)
}

/// `tr` with its `%s` filled in order.
pub(super) fn tr_with(key: &str, fallback: &str, args: &[String]) -> String {
    let mut text = tr(key, fallback);
    for arg in args {
        text = text.replacen("%s", arg, 1);
    }
    text
}

/// A count with its noun in the form the language asks for: `key` with `.one`, `.few` or `.many`
/// (Russian: 1, 21 «блок»; 2–4, 22 «блока»; 5–20 «блоков»), `%s` the count; `fallbacks` are the
/// English forms in that order.
pub(super) fn plural(n: u64, key: &str, fallbacks: (&str, &str, &str)) -> String {
    let (form, fallback) = if n % 10 == 1 && n % 100 != 11 {
        ("one", fallbacks.0)
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        ("few", fallbacks.1)
    } else {
        ("many", fallbacks.2)
    };
    tr_with(&format!("{key}.{form}"), fallback, &[n.to_string()])
}

/// `native_screen`: the layout of a machine's window. A one-input machine's slot 1 (no face
/// reaches it) is hidden, so it is not in the menu.
pub(super) fn layout(block: &str, screen: &mut Option<NativeScreen>) {
    if screen.is_some() {
        return;
    }
    if sheets::is_sheet(block) {
        *screen = sheets::layout(block);
        return;
    }
    if block == content::TERMINAL {
        *screen = Some(terminal::layout());
        return;
    }
    if block == content::TANK {
        *screen = Some(fluid_panels::tank_layout());
        return;
    }
    if block == content::STATION {
        *screen = Some(station::layout());
        return;
    }
    let Some(machine) = Machine::of(block) else {
        return;
    };
    match machine.kind() {
        Kind::Processor => {}
        Kind::Pump => {
            *screen = Some(fluid_panels::pump_layout());
            return;
        }
        _ => {
            *screen = Some(panels::layout(machine));
            return;
        }
    }
    *screen = Some(NativeScreen {
        width: panel::WIDTH,
        height: panel::HEIGHT,
        slots: vec![Some(panel::INPUT), None, Some(panel::OUTPUT)],
        inventory_y: panel::INVENTORY[1],
        inventory_x: panel::INVENTORY[0],
        own_labels: true,
        own_background: true,
        ..NativeScreen::default()
    });
}

pub(super) fn at(origin: [f32; 2], r: Rect) -> Rect {
    [origin[0] + r[0], origin[1] + r[1], r[2], r[3]]
}

/// `native_screen_draw`: the whole window, under the items Horizon draws in its slots.
pub(super) fn draw(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut HudBatch,
    origin: [f32; 2],
) {
    if sheets::is_sheet(block) {
        sheets::draw(game, block, batch, origin);
        return;
    }
    if block == content::TERMINAL {
        terminal::draw(game, batch, origin);
        return;
    }
    if block == content::STATION {
        station::draw(game, batch, origin);
        return;
    }
    if block == content::TANK {
        fluid_panels::draw_tank(game, data, batch, origin);
        return;
    }
    let Some(machine) = Machine::of(block) else {
        return;
    };
    match machine.kind() {
        Kind::Processor => {}
        Kind::Pump => {
            fluid_panels::draw_pump(game, data, batch, origin);
            return;
        }
        _ => {
            panels::draw(game, machine, data, batch, origin);
            return;
        }
    }
    let p = panel::decode(data);
    let o = origin;
    background(game, batch, o, [panel::WIDTH, panel::HEIGHT]);
    title(game, batch, o, machine);
    slots_part(game, batch, o, machine, &p);
    energy_part(
        game,
        batch,
        o,
        p.got,
        p.asked,
        panel::ENERGY_BAR,
        panel::LABEL_ENERGY_Y,
    );
    modes_part(
        game,
        batch,
        o,
        p.mode,
        &panel::MODE_BUTTONS,
        Some(panel::LABEL_MODE_Y),
    );
    status_part(game, batch, o, panel::STATUS, machine, p.shown);
    inventory(game, batch, o, panel::INVENTORY, panel::INVENTORY_LABEL_Y);
}

/// The block's name and its tier («Электропечь · ступень 1»).
pub(super) fn title(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], machine: Machine) {
    let tier = match machine {
        Machine::Crafter => 2,
        _ => 1,
    };
    let name = tr(
        &format!("block.{}", machine.block().replace(':', ".")),
        fallback_name(machine),
    );
    let tier = tr_with("rustcraft.window.tier", "tier %s", &[tier.to_string()]);
    title_line(game, batch, o, &name, Some(&tier), panel::WIDTH);
}

/// The title line (style section 3, «Заголовок»): the name in the value colour, after it in the
/// label colour what the guide writes after « · », and the rule under it across the window.
pub(super) fn title_line(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    name: &str,
    rest: Option<&str>,
    width: f32,
) {
    text(game, batch, name, o, panel::TITLE, VALUE);
    if let Some(rest) = rest {
        let x = panel::TITLE[0] + game.native_text_width(name);
        text(
            game,
            batch,
            &format!(" · {rest}"),
            o,
            [x, panel::TITLE[1]],
            LABEL,
        );
    }
    fills(game, batch, o, &style::title_rule(width, TITLE_RULE_Y));
}

/// The title's rule: under the font's line, above the first part (every window's parts start at
/// 16 or lower).
pub(super) const TITLE_RULE_Y: f32 = 14.0;

/// Fills from rustcraft_rules' style, in window pixels, at the window's origin `o`.
pub(super) fn fills(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], fills: &[Fill]) {
    for f in fills {
        game.native_fill(batch, at(o, f.rect), f.argb);
    }
}

/// Whether the mouse is over `r` (window pixels) of the window at `o`.
pub(super) fn hovered(game: &Game<'_>, o: [f32; 2], r: Rect) -> bool {
    let [x, y] = game.native_cursor();
    panel::inside(r, [x - o[0], y - o[1]])
}

/// The hover outline (style section 4): one pixel of `HOT` round `r` while the mouse is on it.
pub(super) fn hover(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], r: Rect) {
    if hovered(game, o, r) {
        fills(game, batch, o, &style::outline(r, HOT));
    }
}

/// The player's inventory: its label and its 36 slots' frames.
pub(super) fn inventory(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    at: [f32; 2],
    label_y: f32,
) {
    let inventory = tr("container.inventory", "Inventory");
    text(game, batch, &inventory, o, [at[0], label_y], LABEL);
    for i in 0..36 {
        let (column, row) = (i % 9, i / 9);
        let y = if row < 3 {
            at[1] + row as f32 * 18.0
        } else {
            at[1] + 58.0
        };
        slot(game, batch, o, [at[0] + column as f32 * 18.0, y]);
    }
}

fn fallback_name(machine: Machine) -> &'static str {
    match machine {
        Machine::Furnace => "Electric Furnace",
        Machine::Crusher => "Crusher",
        Machine::Generator => "Fuel Generator",
        Machine::Alloy => "Alloy Smelter",
        Machine::Crafter => "Autocrafter",
        Machine::Pump => "Pump",
        Machine::Station => "Station",
        Machine::DockUnload => "Unload Dock",
        Machine::DockCharge => "Charge Dock",
        Machine::Turret => "Turret",
        Machine::Anchor => "Base Anchor",
        Machine::Solar => "Solar Panel",
        Machine::Windmill => "Windmill",
        Machine::LavaGenerator => "Lava Generator",
        Machine::Battery => "Battery",
        Machine::Miner => "Miner",
        Machine::Placer => "Block Placer",
        Machine::Breaker => "Block Breaker",
    }
}

/// The window's frame (style step 2): outline, brass line, light top and left, shade bottom and
/// right, the iron face.
pub(super) fn background(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], size: [f32; 2]) {
    fills(game, batch, o, &style::frame(size));
}

pub(super) fn text(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &str,
    o: [f32; 2],
    at: [f32; 2],
    rgb: u32,
) {
    game.native_text(batch, s, [o[0] + at[0], o[1] + at[1]], rgb, false);
}

/// Text whose right end is at `right`.
pub(super) fn text_right(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &str,
    o: [f32; 2],
    right: f32,
    y: f32,
    rgb: u32,
) {
    let w = game.native_text_width(s);
    text(game, batch, s, o, [right - w, y], rgb);
}

/// A label in the guide's capitals.
pub(super) fn label(game: &Game<'_>, batch: &mut HudBatch, s: &str, o: [f32; 2], at: [f32; 2]) {
    text(game, batch, &s.to_uppercase(), o, at, LABEL);
}

/// A label whose right end is at `right`.
pub(super) fn label_right(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &str,
    o: [f32; 2],
    right: f32,
    y: f32,
) {
    text_right(game, batch, &s.to_uppercase(), o, right, y, LABEL);
}

/// A slot's well around an item at `item` (style step 3): dark top and left, light bottom and
/// right.
pub(super) fn slot(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], item: [f32; 2]) {
    fills(game, batch, o, &style::slot(item));
}

/// A module socket (style step 7): a slot with the brass corner, a plus while `empty`, the hover
/// outline.
pub(super) fn socket(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    item: [f32; 2],
    empty: bool,
) {
    fills(game, batch, o, &style::socket(item, empty));
    hover(game, batch, o, style::slot_rect(item));
}

/// A bar's well and its fill to `share` (style step 4), `r` already at the window's origin; the
/// energy bar has its quarter ticks.
pub(super) fn bar(game: &Game<'_>, batch: &mut HudBatch, r: Rect, share: f64, fill: u32) {
    fills(
        game,
        batch,
        [0.0, 0.0],
        &style::bar(r, share, fill, fill == ENERGY),
    );
}

/// The progress arrow: its shaft (`shaft`, in window pixels) fills with the progress, its head of
/// six columns lights when there is any.
pub(super) fn arrow(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    shaft: Rect,
    progress: f64,
) {
    fills(game, batch, o, &style::arrow(shaft, progress));
}

fn slots_part(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], machine: Machine, p: &Panel) {
    let (input, output) = match machine {
        Machine::Crusher => (
            tr("rustcraft.window.input.crusher", "Ore"),
            tr("rustcraft.window.output.crusher", "Dust"),
        ),
        _ => (
            tr("rustcraft.window.input", "Input"),
            tr("rustcraft.window.output", "Output"),
        ),
    };
    label(
        game,
        batch,
        &input,
        o,
        [panel::INPUT[0] - 4.0, panel::LABEL_SLOTS_Y],
    );
    let output_right = panel::OUTPUT[0] + 21.0;
    label_right(game, batch, &output, o, output_right, panel::LABEL_SLOTS_Y);
    let modules = tr("rustcraft.window.modules", "Modules");
    label(
        game,
        batch,
        &modules,
        o,
        [panel::MODULE_SLOTS[0][0] - 1.0, panel::LABEL_SLOTS_Y],
    );
    slot(game, batch, o, panel::INPUT);
    slot(game, batch, o, panel::OUTPUT);
    for m in panel::MODULE_SLOTS {
        socket(game, batch, o, m, true);
    }
    arrow(game, batch, o, panel::ARROW, p.progress);
}

/// The energy bar with «получает X из Y Е/с» over its right end.
pub(super) fn energy_part(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    got: f64,
    asked: f64,
    bar_rect: Rect,
    label_y: f32,
) {
    label(
        game,
        batch,
        &tr("rustcraft.window.energy", "Energy"),
        o,
        [bar_rect[0], label_y],
    );
    let line = tr("rustcraft.window.energy_line", "gets %s of %s E/s");
    values_right(
        game,
        batch,
        o,
        bar_rect[0] + bar_rect[2],
        label_y,
        &line,
        &[panel::energy_text(got), panel::energy_text(asked)],
    );
    let share = if asked > 0.0 { got / asked } else { 0.0 };
    bar(game, batch, at(o, bar_rect), share, ENERGY);
}

/// A line whose `%s` are values: the words in the label colour, the values in the value colour
/// (style: «получает **8** из **8** Е/с»), its right end at `right`.
pub(super) fn values_right(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    right: f32,
    y: f32,
    template: &str,
    values: &[String],
) {
    let mut parts: Vec<(String, u32)> = Vec::new();
    let mut rest = template;
    for v in values {
        let Some(i) = rest.find("%s") else {
            break;
        };
        parts.push((rest[..i].to_owned(), LABEL));
        parts.push((v.clone(), VALUE));
        rest = &rest[i + 2..];
    }
    parts.push((rest.to_owned(), LABEL));
    let width: f32 = parts.iter().map(|(t, _)| game.native_text_width(t)).sum();
    let mut x = right - width;
    for (t, rgb) in parts {
        if !t.is_empty() {
            text(game, batch, &t, o, [x, y], rgb);
            x += game.native_text_width(&t);
        }
    }
}

/// The modes' names for segments `width` pixels wide: their short names where a full one does
/// not fit (the miner's switch shares its row with the module sockets).
pub(super) fn mode_names(game: &Game<'_>, width: f32) -> Vec<String> {
    let full: Vec<String> = WorkMode::ALL.into_iter().map(mode_name).collect();
    if full
        .iter()
        .all(|n| game.native_text_width(n) <= width - 4.0)
    {
        return full;
    }
    WorkMode::ALL
        .into_iter()
        .map(|m| match m {
            WorkMode::Economy => tr("rustcraft.work.economy.short", "Eco"),
            WorkMode::Normal => tr("rustcraft.work.normal.short", "Normal"),
            WorkMode::Boost => tr("rustcraft.work.boost.short", "Boost"),
        })
        .collect()
}

pub(super) fn mode_name(mode: WorkMode) -> String {
    match mode {
        WorkMode::Economy => tr("rustcraft.work.economy", "Economy"),
        WorkMode::Normal => tr("rustcraft.work.normal", "Normal"),
        WorkMode::Boost => tr("rustcraft.work.boost", "Boost"),
    }
}

/// Text centred in `r` (window pixels).
pub(super) fn centred(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    r: Rect,
    s: &str,
    rgb: u32,
) {
    let w = game.native_text_width(s);
    let at_text = [
        o[0] + r[0] + ((r[2] - w) / 2.0).round(),
        o[1] + r[1] + ((r[3] - FONT_HEIGHT) / 2.0).round() + 1.0,
    ];
    game.native_text(batch, s, at_text, rgb, false);
}

/// A button (`.w-btn`, brass when `on`) with its text in the middle and the hover outline.
pub(super) fn button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    r: Rect,
    name: &str,
    on: bool,
) {
    let face = if on {
        style::raised(r)
    } else {
        style::button(r)
    };
    fills(game, batch, o, &face);
    let hot = hovered(game, o, r);
    let colour = match (on, hot) {
        (true, _) => TEXT_ON,
        (false, true) => VALUE,
        (false, false) => TEXT,
    };
    centred(game, batch, o, r, name, colour);
    if hot && !on {
        fills(game, batch, o, &style::outline(r, HOT));
    }
}

/// A switch (style step 5, `.w-seg`): its segments in one well, the chosen one raised in brass
/// with dark text, the others the label colour on the well; the one under the mouse lights.
pub(super) fn switch(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    segments: &[Rect],
    names: &[String],
    chosen: Option<usize>,
) {
    fills(game, batch, o, &style::segments(segments, chosen));
    for (i, (r, name)) in segments.iter().zip(names).enumerate() {
        let hot = hovered(game, o, *r);
        let colour = if Some(i) == chosen {
            TEXT_ON
        } else if hot {
            VALUE
        } else {
            LABEL
        };
        centred(game, batch, o, *r, name, colour);
        if hot && Some(i) != chosen {
            fills(game, batch, o, &style::outline(*r, HOT));
        }
    }
}

/// The three work modes as one switch, with their label above when given.
pub(super) fn modes_part(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    active: WorkMode,
    buttons: &[Rect; 3],
    label_y: Option<f32>,
) {
    if let Some(y) = label_y {
        label(
            game,
            batch,
            &tr("rustcraft.window.mode", "Work mode"),
            o,
            [buttons[0][0] - 1.0, y],
        );
    }
    let names = mode_names(game, buttons[2][2]);
    let chosen = WorkMode::ALL.iter().position(|m| *m == active);
    switch(game, batch, o, buttons, &names, chosen);
}

/// The machine's diagnosis (A4) as its status line says it.
pub(super) fn status_text(machine: Machine, shown: Shown) -> String {
    let kind = machine.kind();
    match shown {
        Shown::Working if kind == Kind::Generator => tr("rustcraft.generator.working", "Burning"),
        Shown::Idle if kind == Kind::Pump => {
            tr("rustcraft.pump.no_source", "No fluid under the pump")
        }
        Shown::OutputFull if kind == Kind::Pump => {
            tr("rustcraft.pump.full", "Nowhere to give the fluid")
        }
        Shown::Working => tr("rustcraft.machine.working", "Working"),
        Shown::Slowed { percent } => tr_with(
            "rustcraft.machine.slowed",
            "Not enough energy: %s",
            &[format!("{percent}%")],
        ),
        Shown::NoEnergy => tr("rustcraft.machine.no_energy", "No energy"),
        Shown::OutputFull => tr("rustcraft.machine.output_full", "No room for the product"),
        Shown::Idle if matches!(kind, Kind::Alloy | Kind::Crafter) => {
            tr("rustcraft.machine.no_ingredients", "No ingredients")
        }
        Shown::Idle => tr("rustcraft.machine.idle", "Nothing to work on"),
        Shown::NoFuel => tr("rustcraft.machine.no_fuel", "No fuel"),
        Shown::OnFuel => tr("rustcraft.machine.on_fuel", "Working on fuel"),
        Shown::NoDemand => tr(
            "rustcraft.generator.no_demand",
            "Idle: no machine needs energy",
        ),
    }
}

/// How bad a machine's diagnosis is: works, works worse (or waits), stands.
pub(super) fn severity(shown: Shown) -> Severity {
    match shown {
        Shown::Working | Shown::OnFuel => Severity::Ok,
        Shown::Slowed { .. } | Shown::OutputFull | Shown::Idle | Shown::NoDemand => Severity::Warn,
        Shown::NoEnergy | Shown::NoFuel => Severity::Bad,
    }
}

/// The status line of a machine's diagnosis.
pub(super) fn status_part(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    rect: Rect,
    machine: Machine,
    shown: Shown,
) {
    let s = status_text(machine, shown);
    status_line(game, batch, o, rect, severity(shown), &s);
}

/// A status line (style step 6): background and a stripe on the left by severity, its 7 × 7 glyph,
/// the text in the value colour, cut with «…» when it is wider than the line.
pub(super) fn status_line(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    rect: Rect,
    severity: Severity,
    s: &str,
) {
    fills(game, batch, o, &style::status(rect, severity));
    let x = style::status_text_x(rect);
    let room = rect[0] + rect[2] - x - 2.0;
    let shown = fit(game, s, room);
    let y = rect[1] + ((rect[3] - FONT_HEIGHT) / 2.0).round() + 1.0;
    text(game, batch, &shown, o, [x, y], STATUS_TEXT);
}

/// `s`, cut with «…» to fit `room` pixels.
pub(super) fn fit(game: &Game<'_>, s: &str, room: f32) -> String {
    if game.native_text_width(s) <= room {
        return s.to_owned();
    }
    let mut out: String = s.to_owned();
    while !out.is_empty() && game.native_text_width(&format!("{out}…")) > room {
        out.pop();
    }
    format!("{}…", out.trim_end())
}

/// `native_screen_click`: a press on one of the window's buttons is its menu button.
pub(super) fn on_click(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    at: [f32; 2],
    right: bool,
    button: &mut Option<i32>,
) {
    if button.is_some() {
        return;
    }
    if sheets::is_sheet(block) {
        *button = sheets::on_click(block, at);
        return;
    }
    if block == content::TERMINAL {
        *button = terminal::on_click(game, at, right);
        return;
    }
    if block == content::STATION {
        *button = station::on_click(game, at);
        return;
    }
    let Some(machine) = Machine::of(block) else {
        return;
    };
    match machine.kind() {
        Kind::Processor => {}
        Kind::Pump => return,
        _ => {
            *button = panels::button_at(machine, data, at);
            return;
        }
    }
    if let Some(mode) = panel::mode_button_at(at) {
        *button = Some(i32::from(mode.code()));
    }
}

/// The lines over a module slot that no module fills yet.
pub(super) fn module_slot_tooltip(lines: &mut Vec<String>) {
    lines.push(tr("rustcraft.window.module_slot", "Module slot"));
    lines.push(format!(
        "§7{}",
        tr("rustcraft.window.module_slot.none", "No modules exist yet")
    ));
}

/// The lines over a work mode's button.
pub(super) fn mode_tooltip(mode: WorkMode, lines: &mut Vec<String>) {
    let (speed, energy) = mode.factors();
    lines.push(mode_name(mode));
    lines.push(format!(
        "§7{}",
        tr_with(
            "rustcraft.window.mode_hint",
            "speed ×%s, energy ×%s",
            &[factor(speed), factor(energy)]
        )
    ));
}

pub(super) fn energy_tooltip(lines: &mut Vec<String>) {
    lines.push(tr(
        "rustcraft.window.energy_hint",
        "Energy the machine gets and asks for; with less it works slower",
    ));
}

pub(super) fn progress_tooltip(progress: f64, lines: &mut Vec<String>) {
    lines.push(tr_with(
        "rustcraft.window.progress",
        "Progress: %s",
        &[format!("{}%", (progress * 100.0).round())],
    ));
}

/// `native_screen_tooltip`: what the parts without an item mean.
pub(super) fn tooltip(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    at: [f32; 2],
    lines: &mut Vec<String>,
) {
    if sheets::is_sheet(block) {
        sheets::tooltip(game, block, at, lines);
        return;
    }
    if block == content::TERMINAL {
        terminal::tooltip(game, at, lines);
        return;
    }
    if block == content::STATION {
        station::tooltip(game, at, lines);
        return;
    }
    if block == content::TANK {
        fluid_panels::tank_tooltip(data, at, lines);
        return;
    }
    let Some(machine) = Machine::of(block) else {
        return;
    };
    match machine.kind() {
        Kind::Processor => {}
        Kind::Pump => {
            fluid_panels::pump_tooltip(data, at, lines);
            return;
        }
        _ => {
            panels::tooltip(game, machine, data, at, lines);
            return;
        }
    }
    let p = panel::decode(data);
    if panel::module_slot_at(at).is_some() {
        module_slot_tooltip(lines);
    } else if panel::inside(panel::ARROW, at) {
        progress_tooltip(p.progress, lines);
    } else if panel::inside(panel::ENERGY_BAR, at) {
        energy_tooltip(lines);
    } else if let Some(mode) = panel::mode_button_at(at) {
        mode_tooltip(mode, lines);
    }
}

/// A multiplier as the guide writes it: «0,5», «2».
fn factor(value: f64) -> String {
    let s = format!("{value}");
    s.replace('.', ",")
}
