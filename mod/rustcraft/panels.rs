// The windows of the fuel generator, the alloy smelter and the autocrafter on a Horizon client,
// drawn with rustcraft_screens' kit after the guide's sketches (docs/showcase/js/ui.js `gen_fuel`,
// `alloy`, `crafter`). Their layouts and data slots are rustcraft_rules::machines::windows.
//
// The autocrafter's pattern cells are no slots: the window draws each cell's item, which the data
// slots name by number (rustcraft_content::item_of_number), with `native_item`
// (native_hooks.rs:10951-10967). A press on a cell is a menu button; the server sets the cell to
// the item on the player's cursor (rustcraft_crafter::on_cell_button).
use crate::game::{
    Game,
    rustcraft_content::{Machine, item_of_number},
    rustcraft_screens::{
        self as kit, FLAME, LABEL, PANEL_EDGE, SLOT, SLOT_EDGE, STATUS_OK, TEXT, TEXT_ON, at,
        label, label_right, slot, text, text_right, tr, tr_with,
    },
};
use horizon_client::{hud::HudBatch, mod_api::NativeScreen};
use rustcraft_rules::{
    machines::{
        panel::{Rect, energy_text, inside, slot_frame},
        processor::WorkMode,
        windows::{
            WIDTH,
            alloy::{self as aw},
            crafter::{self as cw},
            generator::{self as gw},
        },
    },
    net::energy::GroupBalance,
};

/// The right edge of a window's parts.
const RIGHT: f32 = WIDTH - 8.0;
/// A ghost item's veil: the pattern's items are a sample, not items.
const GHOST: u32 = 0x66C9CCD4;

/// `native_screen` for the three windows.
pub(super) fn layout(machine: Machine) -> NativeScreen {
    let (height, slots, inventory) = match machine {
        Machine::Generator => (gw::HEIGHT, vec![gw::FUEL], gw::INVENTORY),
        Machine::Alloy => (
            aw::HEIGHT,
            aw::INPUTS
                .iter()
                .copied()
                .chain([aw::FUEL])
                .chain(aw::OUTPUTS)
                .collect(),
            aw::INVENTORY,
        ),
        _ => (cw::HEIGHT, cw::slot_positions(), cw::INVENTORY),
    };
    NativeScreen {
        width: WIDTH,
        height,
        slots: slots.into_iter().map(Some).collect(),
        inventory_y: inventory[1],
        inventory_x: inventory[0],
        own_labels: true,
        own_background: true,
        ..NativeScreen::default()
    }
}

pub(super) fn draw(
    game: &Game<'_>,
    machine: Machine,
    data: &[i32],
    batch: &mut HudBatch,
    o: [f32; 2],
) {
    match machine {
        Machine::Generator => draw_generator(game, data, batch, o),
        Machine::Alloy => draw_alloy(game, data, batch, o),
        Machine::Crafter => draw_crafter(game, data, batch, o),
        _ => {}
    }
}

// ---- The fuel generator ----

fn draw_generator(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, o: [f32; 2]) {
    let p = gw::decode(data);
    kit::background(game, batch, o, [WIDTH, gw::HEIGHT]);
    kit::title(game, batch, o, Machine::Generator);
    let fuel = tr("rustcraft.window.fuel", "Fuel");
    label(
        game,
        batch,
        &fuel,
        o,
        [gw::FUEL[0] - 4.0, gw::LABEL_SLOTS_Y],
    );
    slot(game, batch, o, gw::FUEL);
    let burning = tr("rustcraft.window.burning", "Burns");
    // Over the bar's right end: «ТОПЛИВО» over the slot is wider than the gap before the bar.
    let flame_right = gw::FLAME[0] + gw::FLAME[2];
    label_right(game, batch, &burning, o, flame_right, gw::LABEL_SLOTS_Y);
    kit::bar(game, batch, at(o, gw::FLAME), p.flame, FLAME);
    let gives = tr("rustcraft.window.gives", "Output");
    label(
        game,
        batch,
        &gives,
        o,
        [gw::GIVES_BAR[0], gw::LABEL_GIVES_Y],
    );
    let line = tr_with(
        "rustcraft.window.gives_line",
        "gives %s E/s",
        &[energy_text(p.gives)],
    );
    text_right(game, batch, &line, o, RIGHT, gw::LABEL_GIVES_Y, TEXT);
    let share = if p.output > 0.0 {
        p.gives / p.output
    } else {
        0.0
    };
    kit::bar(game, batch, at(o, gw::GIVES_BAR), share, kit::ENERGY);
    let balance = GroupBalance {
        produced: p.produced,
        requested: p.requested,
        supplied: p.supplied,
    };
    let per_second = |v: f64| tr_with("rustcraft.window.per_second", "%s E/s", &[energy_text(v)]);
    let enough = if balance.enough() {
        tr("rustcraft.window.enough.yes", "yes")
    } else {
        tr_with(
            "rustcraft.window.enough.no",
            "no, machines slowed by %s",
            &[format!("{}%", balance.slowdown_percent())],
        )
    };
    let rows = [
        (
            tr("rustcraft.window.produced", "Production"),
            per_second(p.produced),
        ),
        (
            tr("rustcraft.window.used", "Consumption"),
            per_second(p.requested),
        ),
        (
            tr("rustcraft.window.stored", "Stored"),
            tr("rustcraft.window.stored.none", "no batteries"),
        ),
        (tr("rustcraft.window.enough", "Enough"), enough),
    ];
    for ((key, value), y) in rows.iter().zip(gw::BALANCE_ROWS) {
        text(game, batch, key, o, [gw::BALANCE[0], y], LABEL);
        text_right(game, batch, value, o, RIGHT, y, TEXT);
    }
    if let Some(shown) = p.shown {
        kit::status_part(game, batch, o, gw::STATUS, Machine::Generator, shown);
    }
    kit::inventory(game, batch, o, gw::INVENTORY, gw::INVENTORY_LABEL_Y);
}

// ---- The alloy smelter ----

fn draw_alloy(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, o: [f32; 2]) {
    let p = aw::decode(data);
    kit::background(game, batch, o, [WIDTH, aw::HEIGHT]);
    kit::title(game, batch, o, Machine::Alloy);
    let input = tr("rustcraft.window.inputs", "Input");
    label(
        game,
        batch,
        &input,
        o,
        [aw::INPUTS[0][0] - 2.0, aw::LABEL_SLOTS_Y],
    );
    let output = tr("rustcraft.window.output", "Output");
    label_right(game, batch, &output, o, RIGHT, aw::LABEL_SLOTS_Y);
    for s in aw::INPUTS
        .iter()
        .chain(&aw::OUTPUTS)
        .chain(&aw::MODULE_SLOTS)
    {
        slot(game, batch, o, *s);
    }
    kit::arrow(game, batch, o, aw::ARROW, p.progress);
    let fuel = tr("rustcraft.window.fuel", "Fuel");
    label(game, batch, &fuel, o, [aw::FUEL[0] - 2.0, aw::LABEL_FUEL_Y]);
    slot(game, batch, o, aw::FUEL);
    kit::bar(game, batch, at(o, aw::FLAME), p.flame, FLAME);
    let modules = tr("rustcraft.window.modules", "Modules");
    label(
        game,
        batch,
        &modules,
        o,
        [aw::MODULE_SLOTS[0][0] - 1.0, aw::LABEL_FUEL_Y],
    );
    let take = tr("rustcraft.window.take_own", "Take own alloy");
    label(
        game,
        batch,
        &take,
        o,
        [aw::TAKE_OWN[0][0], aw::LABEL_TAKE_Y],
    );
    let on = tr("rustcraft.window.on", "on");
    let off = tr("rustcraft.window.off", "off");
    kit::button(game, batch, o, aw::TAKE_OWN[0], &on, p.take_own);
    kit::button(game, batch, o, aw::TAKE_OWN[1], &off, !p.take_own);
    kit::energy_part(
        game,
        batch,
        o,
        p.got,
        p.asked,
        aw::ENERGY_BAR,
        aw::LABEL_ENERGY_Y,
    );
    if let Some(shown) = p.shown {
        kit::status_part(game, batch, o, aw::STATUS, Machine::Alloy, shown);
    }
    kit::inventory(game, batch, o, aw::INVENTORY, aw::INVENTORY_LABEL_Y);
}

// ---- The autocrafter ----

/// An item's icon at `item` (where a slot's item goes), veiled as a sample.
fn ghost(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], item: [f32; 2], number: Option<u32>) {
    let Some(id) = number.and_then(|n| item_of_number(game, n)) else {
        return;
    };
    let p = [o[0] + item[0], o[1] + item[1]];
    if game.native_item(batch, id, p, 16.0) {
        game.native_fill(batch, [p[0], p[1], 16.0, 16.0], GHOST);
    }
}

fn draw_crafter(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, o: [f32; 2]) {
    let p = cw::decode(data);
    kit::background(game, batch, o, [WIDTH, cw::HEIGHT]);
    kit::title(game, batch, o, Machine::Crafter);
    for i in 0..p.pages {
        tab(game, batch, o, i, &p);
    }
    for m in cw::MODULE_SLOTS {
        slot(game, batch, o, m);
    }
    let pattern = tr("rustcraft.window.pattern", "Pattern");
    label(game, batch, &pattern, o, [8.0, cw::LABEL_GRID_Y]);
    let buffer = tr("rustcraft.window.buffer", "Stock");
    label(
        game,
        batch,
        &buffer,
        o,
        [cw::BUFFER_X - 1.0, cw::LABEL_GRID_Y],
    );
    let output = tr("rustcraft.window.output", "Output");
    label_right(game, batch, &output, o, RIGHT, cw::LABEL_GRID_Y);
    for i in 0..cw::CELLS {
        slot(game, batch, o, cw::cell(i));
        ghost(game, batch, o, cw::cell(i), p.cells[i]);
    }
    kit::arrow(game, batch, o, cw::ARROW, p.progress);
    slot(game, batch, o, cw::RESULT);
    ghost(game, batch, o, cw::RESULT, p.result);
    if p.result.is_some() && p.result_count > 1 {
        let count = p.result_count.to_string();
        let w = game.native_text_width(&count);
        let at_text = [o[0] + cw::RESULT[0] + 17.0 - w, o[1] + cw::RESULT[1] + 9.0];
        game.native_text(batch, &count, at_text, 0xFFFFFF, true);
    }
    for i in 0..cw::BUFFER_SLOTS {
        slot(game, batch, o, cw::buffer(i));
    }
    for i in 0..cw::OUTPUT_SLOTS {
        slot(game, batch, o, cw::output(i));
    }
    kit::energy_part(
        game,
        batch,
        o,
        p.got,
        p.asked,
        cw::ENERGY_BAR,
        cw::LABEL_ENERGY_Y,
    );
    kit::modes_part(game, batch, o, p.mode, &cw::MODE_BUTTONS, None);
    if let Some(shown) = p.shown {
        kit::status_part(game, batch, o, cw::STATUS, Machine::Crafter, shown);
    }
    kit::inventory(game, batch, o, cw::INVENTORY, cw::INVENTORY_LABEL_Y);
}

/// A page's tab: its number and what it makes, lit when open, marked while it is crafted.
fn tab(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], i: usize, p: &cw::CrafterPanel) {
    let r = at(o, cw::tab(i));
    let open = i == p.page;
    game.native_fill(batch, r, if open { PANEL_EDGE } else { SLOT_EDGE });
    let face = [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0];
    game.native_fill(batch, face, if open { kit::BUTTON_ON } else { SLOT });
    let number = (i + 1).to_string();
    let colour = if open { TEXT_ON } else { TEXT };
    game.native_text(batch, &number, [r[0] + 3.0, r[1] + 5.0], colour, false);
    if let Some(id) = p.tabs[i].and_then(|n| item_of_number(game, n)) {
        game.native_item(batch, id, [r[0] + 12.0, r[1] + 2.0], 14.0);
    }
    if p.crafting == Some(i) {
        game.native_fill(
            batch,
            [r[0] + 1.0, r[1] + r[3] - 3.0, r[2] - 2.0, 2.0],
            STATUS_OK,
        );
    }
}

// ---- Presses and tooltips ----

/// `native_screen_click` for the three windows: the button under `at`.
pub(super) fn button_at(machine: Machine, data: &[i32], at: [f32; 2]) -> Option<i32> {
    match machine {
        Machine::Alloy => aw::button_at(at),
        Machine::Crafter => cw::button_at(at, cw::decode(data).pages),
        _ => None,
    }
}

/// An item's name in the player's language.
fn item_name(id: &str) -> String {
    let key = id.replace(':', ".");
    tr(&format!("item.{key}"), &tr(&format!("block.{key}"), id))
}

fn hint(lines: &mut Vec<String>, key: &str, fallback: &str) {
    lines.push(format!("§7{}", tr(key, fallback)));
}

fn over(r: Rect, at: [f32; 2]) -> bool {
    inside(r, at)
}

pub(super) fn tooltip(
    game: &Game<'_>,
    machine: Machine,
    data: &[i32],
    at: [f32; 2],
    lines: &mut Vec<String>,
) {
    match machine {
        Machine::Generator => generator_tooltip(data, at, lines),
        Machine::Alloy => alloy_tooltip(data, at, lines),
        Machine::Crafter => crafter_tooltip(game, data, at, lines),
        _ => {}
    }
}

fn generator_tooltip(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let p = gw::decode(data);
    if over(gw::FLAME, at) {
        lines.push(tr_with(
            "rustcraft.window.flame",
            "Fuel left: %s",
            &[format!("{}%", (p.flame * 100.0).round())],
        ));
        hint(
            lines,
            "rustcraft.window.flame.hint",
            "One coal gives 1280 E; it burns only while the circuit takes energy",
        );
    } else if over(gw::GIVES_BAR, at) {
        hint(
            lines,
            "rustcraft.window.gives.hint",
            "What the generator gives the machines now, of 16 E/s",
        );
    } else if over(gw::BALANCE, at) {
        hint(
            lines,
            "rustcraft.window.balance.hint.1",
            "The balance of the whole circuit:",
        );
        hint(
            lines,
            "rustcraft.window.balance.hint.2",
            "what it makes and uses, what is stored, whether it is enough",
        );
    }
}

fn alloy_tooltip(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let p = aw::decode(data);
    if over(aw::ARROW, at) {
        kit::progress_tooltip(p.progress, lines);
    } else if over(aw::FLAME, at) {
        lines.push(tr_with(
            "rustcraft.window.flame",
            "Fuel left: %s",
            &[format!("{}%", (p.flame * 100.0).round())],
        ));
    } else if aw::module_slot_at(at).is_some() {
        kit::module_slot_tooltip(lines);
    } else if aw::button_at(at).is_some() {
        lines.push(tr("rustcraft.window.take_own", "Take own alloy"));
        hint(
            lines,
            "rustcraft.window.take_own.hint.1",
            "An alloy in the output that the next recipe needs",
        );
        hint(
            lines,
            "rustcraft.window.take_own.hint.2",
            "goes on by itself: sparksteel to crystallite",
        );
    } else if over(aw::ENERGY_BAR, at) {
        hint(
            lines,
            "rustcraft.window.alloy_energy.hint",
            "Energy is optional: with it the smelter works faster and burns no fuel",
        );
    }
}

fn crafter_tooltip(game: &Game<'_>, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let p = cw::decode(data);
    let name = |n: Option<u32>| n.and_then(|n| item_of_number(game, n)).map(item_name);
    if let Some(i) = (0..p.pages).find(|&i| over(cw::tab(i), at)) {
        let made =
            name(p.tabs[i]).unwrap_or_else(|| tr("rustcraft.window.page.empty", "nothing yet"));
        lines.push(tr_with(
            "rustcraft.window.page",
            "Page %s: %s",
            &[(i + 1).to_string(), made],
        ));
        hint(lines, "rustcraft.window.page.hint", "Click: open this page");
    } else if let Some(i) = cw::cell_at(at) {
        if let Some(item) = name(p.cells[i]) {
            lines.push(item);
        }
        hint(
            lines,
            "rustcraft.window.cell.hint.1",
            "Click with an item on the cursor: into the pattern",
        );
        hint(
            lines,
            "rustcraft.window.cell.hint.2",
            "Click with an empty cursor: clear the cell",
        );
    } else if inside(slot_frame(cw::RESULT), at) {
        match name(p.result) {
            Some(item) => lines.push(tr_with(
                "rustcraft.window.result",
                "Makes: %s ×%s",
                &[item, p.result_count.to_string()],
            )),
            None => lines.push(tr(
                "rustcraft.window.result.none",
                "The pattern makes nothing",
            )),
        }
    } else if cw::module_slot_at(at).is_some() {
        lines.push(tr("rustcraft.window.module_slot", "Module slot"));
        hint(
            lines,
            "rustcraft.window.module_slot.capacity",
            "A capacity module gives one more page",
        );
    } else if over(cw::ARROW, at) {
        kit::progress_tooltip(p.progress, lines);
    } else if over(cw::ENERGY_BAR, at) {
        kit::energy_tooltip(lines);
    } else if let Some(i) = cw::MODE_BUTTONS.iter().position(|&r| over(r, at)) {
        kit::mode_tooltip(WorkMode::ALL[i], lines);
    }
}
