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
    rustcraft_fluid_panels as fluid_panels, rustcraft_panels as panels,
    rustcraft_station as station, rustcraft_terminal as terminal,
};
use horizon_client::{hud::HudBatch, mod_api::NativeScreen};
use rustcraft_rules::machines::{
    panel::{self, Panel, Rect},
    processor::{Shown, WorkMode},
};

// The guide's window colours (docs/showcase/ui.css), as 0xAARRGGBB.
pub(super) const PANEL: u32 = 0xFFC9CCD4;
pub(super) const PANEL_EDGE: u32 = 0xFF0D0F14;
pub(super) const PANEL_LIGHT: u32 = 0xFFF1F3F7;
pub(super) const PANEL_SHADE: u32 = 0xFF8A919D;
pub(super) const SLOT: u32 = 0xFF8A919D;
pub(super) const SLOT_EDGE: u32 = 0xFF5D6470;
pub(super) const BAR: u32 = 0xFF5D6470;
pub(super) const BAR_EDGE: u32 = 0xFF3A3F49;
pub(super) const PROGRESS: u32 = 0xFFF1F3F7;
pub(super) const ENERGY: u32 = 0xFFFFE14A;
pub(super) const FLAME: u32 = 0xFFFF9A3C;
pub(super) const BUTTON_ON: u32 = 0xFF1A1D25;
pub(super) const STATUS_OK: u32 = 0xFF59D38C;
pub(super) const STATUS_WARN: u32 = 0xFFF0B33C;
pub(super) const STATUS_BAD: u32 = 0xFFE0564A;
// Text colours, 0xRRGGBB.
pub(super) const LABEL: u32 = 0x3A3F49;
pub(super) const TEXT: u32 = 0x1A1D25;
pub(super) const TEXT_ON: u32 = 0xF1F3F7;
pub(super) const STATUS_TEXT: u32 = 0x14161B;
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
    let title = format!(
        "{} · {}",
        tr(
            &format!("block.{}", machine.block().replace(':', ".")),
            fallback_name(machine)
        ),
        tr_with("rustcraft.window.tier", "tier %s", &[tier.to_string()])
    );
    text(game, batch, &title, o, panel::TITLE, TEXT);
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

/// The panel with the guide's bevel: a dark edge, light top and left, shade bottom and right.
pub(super) fn background(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], size: [f32; 2]) {
    let [w, h] = size;
    game.native_fill(batch, [o[0], o[1], w, h], PANEL_EDGE);
    game.native_fill(
        batch,
        [o[0] + 2.0, o[1] + 2.0, w - 4.0, h - 4.0],
        PANEL_SHADE,
    );
    game.native_fill(
        batch,
        [o[0] + 2.0, o[1] + 2.0, w - 6.0, h - 6.0],
        PANEL_LIGHT,
    );
    game.native_fill(batch, [o[0] + 4.0, o[1] + 4.0, w - 8.0, h - 8.0], PANEL);
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

/// A slot's frame around an item at `item` (`.w-s`).
pub(super) fn slot(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], item: [f32; 2]) {
    let f = at(o, panel::slot_frame(item));
    game.native_fill(batch, f, SLOT_EDGE);
    game.native_fill(batch, [f[0] + 1.0, f[1] + 1.0, 16.0, 16.0], SLOT);
}

/// A bar's track and its fill to `share` (`.w-prog`).
pub(super) fn bar(game: &Game<'_>, batch: &mut HudBatch, r: Rect, share: f64, fill: u32) {
    game.native_fill(batch, r, BAR_EDGE);
    game.native_fill(batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], BAR);
    let w = ((r[2] - 2.0) * share.clamp(0.0, 1.0) as f32).round();
    if w > 0.0 {
        game.native_fill(batch, [r[0] + 1.0, r[1] + 1.0, w, r[3] - 2.0], fill);
    }
}

/// The progress arrow: its shaft (`shaft`, in window pixels) fills with the progress, its head
/// lights when there is any.
pub(super) fn arrow(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    shaft: Rect,
    progress: f64,
) {
    let shaft = at(o, shaft);
    bar(game, batch, shaft, progress, PROGRESS);
    let (x, y, h) = (
        shaft[0] + shaft[2],
        shaft[1] + shaft[3] / 2.0,
        shaft[3] + 6.0,
    );
    let head = |grow: f32| {
        [
            [x - 1.0, y - h / 2.0 - grow],
            [x + h / 2.0 + grow, y],
            [x - 1.0, y + h / 2.0 + grow],
            [x - 1.0, y + h / 2.0 + grow],
        ]
    };
    game.native_quad(batch, head(1.0), BAR_EDGE);
    let lit = if progress > 0.0 { PROGRESS } else { BAR };
    game.native_quad(batch, head(0.0), lit);
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
        slot(game, batch, o, m);
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
    let line = tr_with(
        "rustcraft.window.energy_line",
        "gets %s of %s E/s",
        &[panel::energy_text(got), panel::energy_text(asked)],
    );
    text_right(
        game,
        batch,
        &line,
        o,
        bar_rect[0] + bar_rect[2],
        label_y,
        TEXT,
    );
    let share = if asked > 0.0 { got / asked } else { 0.0 };
    bar(game, batch, at(o, bar_rect), share, ENERGY);
}

pub(super) fn mode_name(mode: WorkMode) -> String {
    match mode {
        WorkMode::Economy => tr("rustcraft.work.economy", "Economy"),
        WorkMode::Normal => tr("rustcraft.work.normal", "Normal"),
        WorkMode::Boost => tr("rustcraft.work.boost", "Boost"),
    }
}

/// A button (`.w-btn`, lit when `on`) with its text in the middle.
pub(super) fn button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    r: Rect,
    name: &str,
    on: bool,
) {
    let r = at(o, r);
    game.native_fill(batch, r, if on { PANEL_EDGE } else { SLOT_EDGE });
    let face = [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0];
    game.native_fill(batch, face, if on { BUTTON_ON } else { SLOT });
    let w = game.native_text_width(name);
    let at_text = [
        r[0] + (r[2] - w) / 2.0,
        r[1] + (r[3] - FONT_HEIGHT) / 2.0 + 1.0,
    ];
    game.native_text(batch, name, at_text, if on { TEXT_ON } else { TEXT }, false);
}

/// The three work modes (`.w-btn`, the active one `.on`), with their label above when given.
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
            [buttons[0][0], y],
        );
    }
    for (mode, r) in WorkMode::ALL.into_iter().zip(buttons) {
        button(game, batch, o, *r, &mode_name(mode), mode == active);
    }
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

/// The status line in its colour: green at work, amber when waiting, red without power.
pub(super) fn status_part(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    rect: Rect,
    machine: Machine,
    shown: Shown,
) {
    let colour = match shown {
        Shown::Working | Shown::OnFuel => STATUS_OK,
        Shown::Slowed { .. } | Shown::OutputFull | Shown::Idle | Shown::NoDemand => STATUS_WARN,
        Shown::NoEnergy | Shown::NoFuel => STATUS_BAD,
    };
    let r = at(o, rect);
    game.native_fill(batch, r, colour);
    let s = status_text(machine, shown);
    let at_text = [r[0] + 5.0, r[1] + (r[3] - FONT_HEIGHT) / 2.0 + 1.0];
    game.native_text(batch, &s, at_text, STATUS_TEXT, false);
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
