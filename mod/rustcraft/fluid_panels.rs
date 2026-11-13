// The tank's and the pump's windows on a Horizon client (docs/showcase/js/ui.js, windows `tank`
// and `pump`), drawn with rustcraft_screens' kit from their menus' data slots
// (rustcraft_rules::machines::fluid_windows). Neither has a slot of its own: the menu's one slot
// is hidden (`NativeScreen.slots`, a `None` leaves it out of the menu, native_hooks.rs:3742-3748).
// The tank shows the whole group of touching tanks: its fluid, how much of how much, what comes
// in and goes out a second. The pump shows what is under it, how fast it pumps, its energy, its
// empty module slots and its diagnosis.
use crate::game::{
    Game,
    rustcraft_content::Machine,
    rustcraft_screens::{
        self as kit, BAR, BAR_EDGE, LABEL, STATUS_BAD, STATUS_TEXT, STATUS_WARN, TEXT, at, label,
        plural, slot, text, text_right, tr, tr_with,
    },
};
use horizon_client::{hud::HudBatch, mod_api::NativeScreen};
use rustcraft_rules::machines::{
    fluid_windows::{
        FluidShown, TITLE, WIDTH,
        pump::{self as pw, PumpPanel},
        tank::{self as tw, Note, TankPanel},
    },
    panel::{inside, slot_frame},
    pump::Source,
};

const RIGHT: f32 = WIDTH - 8.0;
/// The guide's fluid colours (ui.js: water #3f76e4, lava #f2731c, the solution's #59d38c).
const WATER: u32 = 0xFF3F76E4;
const LAVA: u32 = 0xFFF2731C;
const OTHER: u32 = 0xFF59D38C;

fn screen(height: f32, inventory: [f32; 2]) -> NativeScreen {
    NativeScreen {
        width: WIDTH,
        height,
        slots: vec![None],
        inventory_y: inventory[1],
        inventory_x: inventory[0],
        own_labels: true,
        own_background: true,
        ..NativeScreen::default()
    }
}

pub(super) fn tank_layout() -> NativeScreen {
    screen(tw::HEIGHT, tw::INVENTORY)
}

pub(super) fn pump_layout() -> NativeScreen {
    screen(pw::HEIGHT, pw::INVENTORY)
}

/// Buckets as the guide writes them: «20», «12,8».
fn buckets(mb: u64) -> String {
    let tenths = (mb + 50) / 100;
    if tenths.is_multiple_of(10) {
        (tenths / 10).to_string()
    } else {
        format!("{},{}", tenths / 10, tenths % 10)
    }
}

/// A flow in buckets a second: «0,1», «1», «0».
fn flow(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

fn fluid_name(f: FluidShown) -> String {
    match f {
        FluidShown::None => tr("rustcraft.tank.nothing", "Empty"),
        FluidShown::Water => tr("rustcraft.fluid.water.name", "Water"),
        FluidShown::Lava => tr("rustcraft.fluid.lava.name", "Lava"),
        FluidShown::Other => tr("rustcraft.fluid.other.name", "Fluid"),
    }
}

fn fluid_colour(f: FluidShown) -> u32 {
    match f {
        FluidShown::Lava => LAVA,
        FluidShown::Other => OTHER,
        _ => WATER,
    }
}

/// A status line in its colour, `s` in it, broken into lines between words where it is wider
/// than the box.
fn status_line(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    r: [f32; 4],
    colour: u32,
    s: &str,
) {
    let r = at(o, r);
    game.native_fill(batch, r, colour);
    let width = r[2] - 10.0;
    let mut lines: Vec<String> = Vec::new();
    for word in s.split(' ') {
        match lines.last_mut() {
            Some(line) if game.native_text_width(&format!("{line} {word}")) <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    let line_height = kit::FONT_HEIGHT + 2.0;
    let top = r[1] + (r[3] - line_height * lines.len() as f32) / 2.0 + 1.0;
    for (i, line) in lines.iter().enumerate() {
        let at_text = [r[0] + 5.0, top + line_height * i as f32];
        game.native_text(batch, line, at_text, STATUS_TEXT, false);
    }
}

// ---- The tank ----

pub(super) fn draw_tank(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, o: [f32; 2]) {
    let p = tw::decode(data);
    kit::background(game, batch, o, [WIDTH, tw::HEIGHT]);
    let blocks = plural(
        p.blocks.max(1) as u64,
        "rustcraft.window.blocks",
        ("%s block", "%s blocks", "%s blocks"),
    );
    let title = format!("{} · {}", tr("block.rustcraft.tank", "Tank"), blocks);
    text(game, batch, &title, o, TITLE, TEXT);
    gauge(game, batch, o, &p);
    label(game, batch, &fluid_name(p.fluid), o, tw::NAME);
    let amount = tr_with(
        "rustcraft.tank.amount",
        "%s of %s buckets",
        &[buckets(p.amount), buckets(p.capacity)],
    );
    text(game, batch, &amount, o, tw::AMOUNT, TEXT);
    let rows = [
        (tr("rustcraft.tank.coming", "Coming in"), p.coming),
        (tr("rustcraft.tank.going", "Going out"), p.going),
    ];
    for ((key, value), y) in rows.iter().zip(tw::ROWS) {
        text(game, batch, key, o, [tw::LABEL_X, y], LABEL);
        let v = tr_with("rustcraft.tank.per_second", "%s buckets/s", &[flow(*value)]);
        text_right(game, batch, &v, o, RIGHT, y, TEXT);
    }
    match p.note {
        Note::NoLane => {
            let s = tr(
                "rustcraft.fluid.no_lane.short",
                "No room in the channel for another fluid",
            );
            status_line(game, batch, o, tw::STATUS, STATUS_BAD, &s);
        }
        Note::Mixed => {
            let s = tr(
                "rustcraft.tank.mixed.short",
                "A tank with another fluid touches it",
            );
            status_line(game, batch, o, tw::STATUS, STATUS_WARN, &s);
        }
        Note::None => {}
    }
    kit::inventory(game, batch, o, tw::INVENTORY, tw::INVENTORY_LABEL_Y);
}

/// The tank's level, filled from the bottom (`.w-tank`).
fn gauge(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], p: &TankPanel) {
    let r = at(o, tw::GAUGE);
    game.native_fill(batch, r, BAR_EDGE);
    game.native_fill(batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], BAR);
    if p.capacity == 0 || p.amount == 0 {
        return;
    }
    let share = (p.amount as f32 / p.capacity as f32).clamp(0.0, 1.0);
    let h = ((r[3] - 2.0) * share).round().max(1.0);
    let fill = [r[0] + 1.0, r[1] + r[3] - 1.0 - h, r[2] - 2.0, h];
    game.native_fill(batch, fill, fluid_colour(p.fluid));
}

pub(super) fn tank_tooltip(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let p = tw::decode(data);
    if inside(tw::GAUGE, at) {
        lines.push(fluid_name(p.fluid));
        lines.push(format!(
            "§7{}",
            tr(
                "rustcraft.tank.gauge_hint",
                "The fluid level: which fluid, how much and how much fits. Tanks that touch show their shared volume."
            )
        ));
    }
}

// ---- The pump ----

pub(super) fn draw_pump(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, o: [f32; 2]) {
    let p = pw::decode(data);
    kit::background(game, batch, o, [WIDTH, pw::HEIGHT]);
    kit::title(game, batch, o, Machine::Pump);
    let under = match p.under {
        None => tr("rustcraft.pump.under.none", "nothing"),
        Some(Source::Water) if p.endless => {
            tr("rustcraft.pump.under.water_endless", "water (endless)")
        }
        Some(Source::Water) => tr("rustcraft.pump.under.water", "water"),
        Some(Source::Lava) => tr("rustcraft.pump.under.lava", "lava"),
    };
    let rate = pump_rate(&p);
    let rows = [
        (tr("rustcraft.pump.under", "Under the pump"), under),
        (tr("rustcraft.pump.rate", "Pumps"), rate),
    ];
    for ((key, value), y) in rows.iter().zip(pw::ROWS) {
        text(game, batch, key, o, [8.0, y], LABEL);
        text_right(game, batch, value, o, RIGHT, y, TEXT);
    }
    kit::bar(game, batch, at(o, pw::PROGRESS), p.progress, kit::PROGRESS);
    kit::energy_part(
        game,
        batch,
        o,
        p.got,
        p.asked,
        pw::ENERGY_BAR,
        pw::LABEL_ENERGY_Y,
    );
    label(
        game,
        batch,
        &tr("rustcraft.window.modules", "Modules"),
        o,
        [8.0, pw::LABEL_MODULES_Y],
    );
    for m in pw::MODULE_SLOTS {
        slot(game, batch, o, m);
    }
    if p.no_lane {
        let s = tr(
            "rustcraft.fluid.no_lane.short",
            "No room in the channel for another fluid",
        );
        status_line(game, batch, o, pw::STATUS, STATUS_BAD, &s);
    } else {
        kit::status_part(game, batch, o, pw::STATUS, Machine::Pump, p.shown);
    }
    kit::inventory(game, batch, o, pw::INVENTORY, pw::INVENTORY_LABEL_Y);
}

/// «1 ведро за 10 с» at the speed it gets energy for now; «—» while it does not pump.
fn pump_rate(p: &PumpPanel) -> String {
    if p.under.is_none() || p.asked <= 0.0 || p.got <= 0.0 {
        return "—".to_owned();
    }
    let share = (p.got / p.asked).clamp(0.01, 1.0);
    let seconds = (PUMP_SECONDS / share).round() as u64;
    tr_with(
        "rustcraft.pump.rate_value",
        "1 bucket in %s s",
        &[seconds.to_string()],
    )
}

/// Seconds a bucket takes on full energy, as rustcraft_rules::tuning says (200 ticks).
const PUMP_SECONDS: f64 = 10.0;

pub(super) fn pump_tooltip(_data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if pw::MODULE_SLOTS.iter().any(|&m| inside(slot_frame(m), at)) {
        kit::module_slot_tooltip(lines);
    } else if inside(pw::ENERGY_BAR, at) {
        kit::energy_tooltip(lines);
    } else if inside([8.0, pw::ROWS[0], RIGHT - 8.0, 22.0], at) {
        lines.push(tr(
            "rustcraft.pump.rows_hint",
            "Which fluid the pump found under itself and how fast it gives it",
        ));
        lines.push(format!(
            "§7{}",
            tr(
                "rustcraft.pump.endless_hint",
                "Water with two sources beside it does not run out; lava does"
            )
        ));
    }
}
