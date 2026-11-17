//! Slice 18, wave 2: the chassis window as the guide draws it (ui.js `chassis`), a screen of the
//! mod's own on the player's client (API 111) as the backpack's and the tablet's: «Инструмент»
//! and «Улучшения и модули» with their slots, the bars «Заряд» and «Трюм», «Груз» with the hold's
//! slots, the filter, the bore's height field, the status line and the player's inventory under
//! them. The server sends what it shows ([`View`]), the client sends presses back ([`Press`]).
//!
//! A press names one of the window's parts by the cell the server's click rules know
//! (rustcraft_workers::click): the tool `TOOL`, the sockets from `SOCKETS`, the filter `FILTER`,
//! the hold from `HOLD`, the player's inventory from `INVENTORY_FROM` (its 27 main slots, then
//! the hotbar's 9, as a chest menu counts them).

use serde::{Deserialize, Serialize};

use super::{Note, Tool, chassis};

/// The server's view of the window, and its opening or closing.
pub const VIEW_CHANNEL: &str = "rustcraft:chassis_view";
/// The client's presses.
pub const PRESS_CHANNEL: &str = "rustcraft:chassis_press";
/// The client's screen id (`native_mod_screen`).
pub const SCREEN: &str = "rustcraft:chassis";

/// The click rules' cells.
pub const TOOL: usize = 0;
pub const SOCKETS: usize = 1;
pub const FILTER: usize = 8;
pub const HOLD: usize = 9;
/// The hold's slots the window shows; a bigger hold keeps the rest hidden and counted.
pub const SHOWN: usize = 45;
pub const INVENTORY_FROM: usize = HOLD + SHOWN;
const MAIN: usize = 27;
pub const INVENTORY: usize = 36;

/// The lowest and highest height the bore's field takes (the world's build limits).
pub const HEIGHT_MIN: i32 = -64;
pub const HEIGHT_MAX: i32 = 319;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct View {
    pub tier: u8,
    pub tool: Option<Tool>,
    /// Each socket's module item (as many as the chassis has, U4).
    pub sockets: Vec<Option<String>>,
    /// Charge 0..1 and the battery, E.
    pub charge: f64,
    pub battery: f64,
    /// Hold slots used and in all.
    pub hold: (usize, usize),
    /// The shown hold slots: item id and count.
    pub cargo: Vec<Option<(String, u32)>>,
    /// Items in the hold past the shown slots.
    pub hidden: u32,
    pub filter: Vec<String>,
    pub note: Note,
    /// The bore's height field (guide `mod_height`), when it has the module: the height set,
    /// none for «держит уровень пути», and the rail's height now.
    pub height: Option<(Option<i32>, i32)>,
    pub open: bool,
    pub close: bool,
}

impl View {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn decode(data: &[u8]) -> Option<View> {
        serde_json::from_slice(data).ok()
    }
}

/// A press in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Press {
    /// A click on cell `cell` of the click rules; `right` puts an inventory item in the filter.
    Cell { cell: usize, right: bool },
    /// The height field: up or down by `by`; 0 back to «держит уровень пути».
    Height { by: i32 },
    Closed,
}

pub fn encode_press(p: &Press) -> Vec<u8> {
    serde_json::to_vec(p).unwrap_or_default()
}

pub fn decode_press(data: &[u8]) -> Option<Press> {
    serde_json::from_slice(data).ok()
}

/// The click rules' cell of the client's inventory slot `i` (0..9 the hotbar, 9..36 the rest).
pub fn inventory_cell(i: usize) -> Option<usize> {
    match i {
        0..9 => Some(INVENTORY_FROM + MAIN + i),
        9..INVENTORY => Some(INVENTORY_FROM + i - 9),
        _ => None,
    }
}

/// The height field after a press of `by` from `set`, the rail being at `y`: the first press
/// starts from the rail's height.
pub fn adjust_height(set: Option<i32>, y: i32, by: i32) -> Option<i32> {
    if by == 0 {
        return None;
    }
    Some((set.unwrap_or(y) + by).clamp(HEIGHT_MIN, HEIGHT_MAX))
}

// ---- The layout (GUI pixels from the window's corner; items' top left corners) ----

const PITCH: f32 = 18.0;
const COLUMNS: usize = 9;
pub const WIDTH: f32 = 176.0;
pub const TOOL_AT: [f32; 2] = [8.0, 28.0];
pub const SOCKETS_AT: [f32; 2] = [44.0, 28.0];
pub const LABELS_Y: f32 = 17.0;
pub const GAUGES_Y: f32 = 50.0;
pub const CARGO_LABEL_Y: f32 = 72.0;
pub const CARGO_AT: [f32; 2] = [8.0, 83.0];
/// The −/+ buttons of the height field, from its row's top.
pub const BUTTON: [f32; 2] = [14.0, 12.0];

/// A part of the window under the mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Tool,
    Socket(usize),
    Cargo(usize),
    Inventory(usize),
    Filter,
    HeightDown,
    HeightUp,
    Height,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub sockets: usize,
    pub cargo: usize,
    /// The rows under the hold: the filter's, the height field's (if any), the status line's.
    pub filter_y: f32,
    pub height_y: Option<f32>,
    pub status_y: f32,
    pub inventory_label_y: f32,
    pub inventory: [f32; 2],
    pub size: [f32; 2],
}

/// The window for `sockets` sockets, `cargo` shown hold slots and the height field or not.
pub fn layout(sockets: usize, cargo: usize, height: bool) -> Layout {
    let sockets = sockets.min(chassis::MAX_SOCKETS);
    let cargo = cargo.min(SHOWN);
    let rows = cargo.div_ceil(COLUMNS).max(1);
    let grid_bottom = CARGO_AT[1] + rows as f32 * PITCH - 1.0;
    let filter_y = grid_bottom + 4.0;
    let mut y = filter_y + 11.0;
    let height_y = height.then(|| {
        let at = y;
        y += 14.0;
        at
    });
    let status_y = y;
    let inventory_label_y = status_y + 18.0;
    let inventory = [8.0, inventory_label_y + 11.0];
    let bottom = inventory[1] + 58.0 + 17.0 + 7.0;
    Layout { sockets, cargo, filter_y, height_y, status_y, inventory_label_y, inventory, size: [WIDTH, bottom] }
}

/// Whether `at` is on the well of the slot whose item is at `item`.
fn on(item: [f32; 2], at: [f32; 2]) -> bool {
    at[0] >= item[0] - 1.0 && at[0] < item[0] + 17.0 && at[1] >= item[1] - 1.0 && at[1] < item[1] + 17.0
}

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

impl Layout {
    pub fn socket(&self, i: usize) -> [f32; 2] {
        [SOCKETS_AT[0] + i as f32 * PITCH, SOCKETS_AT[1]]
    }

    pub fn cargo_slot(&self, i: usize) -> [f32; 2] {
        [CARGO_AT[0] + (i % COLUMNS) as f32 * PITCH, CARGO_AT[1] + (i / COLUMNS) as f32 * PITCH]
    }

    pub fn inventory_slot(&self, i: usize) -> [f32; 2] {
        let [x, y] = self.inventory;
        if i < COLUMNS {
            [x + i as f32 * PITCH, y + 58.0]
        } else {
            let k = i - COLUMNS;
            [x + (k % COLUMNS) as f32 * PITCH, y + (k / COLUMNS) as f32 * PITCH]
        }
    }

    /// The filter's row: a click there clears it.
    pub fn filter_rect(&self) -> [f32; 4] {
        [8.0, self.filter_y - 1.0, WIDTH - 16.0, 10.0]
    }

    /// The height field's −, value and + boxes.
    pub fn height_rects(&self) -> Option<[[f32; 4]; 3]> {
        let y = self.height_y?;
        let right = WIDTH - 8.0;
        let up = [right - BUTTON[0], y, BUTTON[0], BUTTON[1]];
        let value = [up[0] - 40.0, y, 38.0, BUTTON[1]];
        let down = [value[0] - BUTTON[0] - 2.0, y, BUTTON[0], BUTTON[1]];
        Some([down, value, up])
    }

    pub fn at(&self, at: [f32; 2]) -> Option<Part> {
        if on(TOOL_AT, at) {
            return Some(Part::Tool);
        }
        if let Some(i) = (0..self.sockets).find(|&i| on(self.socket(i), at)) {
            return Some(Part::Socket(i));
        }
        if let Some(i) = (0..self.cargo).find(|&i| on(self.cargo_slot(i), at)) {
            return Some(Part::Cargo(i));
        }
        if let Some(i) = (0..INVENTORY).find(|&i| on(self.inventory_slot(i), at)) {
            return Some(Part::Inventory(i));
        }
        if let Some([down, value, up]) = self.height_rects() {
            if inside(down, at) {
                return Some(Part::HeightDown);
            }
            if inside(up, at) {
                return Some(Part::HeightUp);
            }
            if inside(value, at) {
                return Some(Part::Height);
            }
        }
        inside(self.filter_rect(), at).then_some(Part::Filter)
    }
}

/// The press a click on `part` makes, `right` for the right button; none for a part that does
/// nothing so.
pub fn press_of(part: Part, right: bool) -> Option<Press> {
    let step = if right { 10 } else { 1 };
    Some(match part {
        Part::Tool => Press::Cell { cell: TOOL, right },
        Part::Socket(i) => Press::Cell { cell: SOCKETS + i, right },
        Part::Cargo(i) => Press::Cell { cell: HOLD + i, right },
        Part::Inventory(i) => Press::Cell { cell: inventory_cell(i)?, right },
        Part::Filter => Press::Cell { cell: FILTER, right },
        Part::HeightDown => Press::Height { by: -step },
        Part::HeightUp => Press::Height { by: step },
        Part::Height => Press::Height { by: 0 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_names_the_cell_the_click_rules_know() {
        let l = layout(5, 27, true);
        let tool = l.at([TOOL_AT[0] + 4.0, TOOL_AT[1] + 4.0]);
        assert_eq!(tool.and_then(|p| press_of(p, false)), Some(Press::Cell { cell: TOOL, right: false }));
        let s = l.socket(4);
        assert_eq!(l.at([s[0] + 2.0, s[1] + 2.0]), Some(Part::Socket(4)));
        let past = l.socket(5);
        assert_eq!(l.at([past[0] + 2.0, past[1] + 2.0]), None, "a sixth socket is not there");
        let c = l.cargo_slot(26);
        assert_eq!(press_of(l.at([c[0] + 1.0, c[1] + 1.0]).unwrap(), false), Some(Press::Cell { cell: HOLD + 26, right: false }));
        let hotbar = l.inventory_slot(0);
        assert_eq!(press_of(l.at(hotbar).unwrap(), true), Some(Press::Cell { cell: INVENTORY_FROM + 27, right: true }));
        let main = l.inventory_slot(9);
        assert_eq!(press_of(l.at(main).unwrap(), false), Some(Press::Cell { cell: INVENTORY_FROM, right: false }));
        let [down, value, up] = l.height_rects().unwrap();
        assert_eq!(l.at([down[0] + 1.0, down[1] + 1.0]), Some(Part::HeightDown));
        assert_eq!(l.at([up[0] + 1.0, up[1] + 1.0]), Some(Part::HeightUp));
        assert_eq!(press_of(l.at([value[0] + 1.0, value[1] + 1.0]).unwrap(), false), Some(Press::Height { by: 0 }));
        assert_eq!(l.at([20.0, l.filter_y + 2.0]), Some(Part::Filter));
        assert!(layout(3, 9, false).height_rects().is_none());
    }

    #[test]
    fn the_window_fits_a_hold_of_45_and_the_inventory() {
        let l = layout(7, 45, true);
        assert!(l.size[1] <= 330.0, "{l:?}");
        let last = l.cargo_slot(44);
        assert!(last[1] + 17.0 < l.filter_y);
        assert!(l.inventory_slot(0)[1] + 17.0 < l.size[1]);
    }

    #[test]
    fn the_height_field_starts_from_the_rail_and_keeps_to_the_world() {
        assert_eq!(adjust_height(None, 40, 1), Some(41));
        assert_eq!(adjust_height(Some(41), 40, -10), Some(31));
        assert_eq!(adjust_height(Some(310), 40, 10), Some(HEIGHT_MAX));
        assert_eq!(adjust_height(Some(12), 40, 0), None);
    }

    #[test]
    fn views_and_presses_go_over_the_wire_whole() {
        let v = View { tier: 2, sockets: vec![None, Some("rustcraft:module_height".into())], height: Some((Some(12), 40)), ..View::default() };
        assert_eq!(View::decode(&v.encode()), Some(v));
        for p in [Press::Cell { cell: 3, right: true }, Press::Height { by: -10 }, Press::Closed] {
            assert_eq!(decode_press(&encode_press(&p)), Some(p));
        }
    }
}

impl View {
    /// What the window of worker `w` shows.
    pub fn of(w: &super::Worker, t: &crate::tuning::Tuning) -> View {
        use crate::modules::Module;
        let battery = w.capacity(t);
        let name = |s: &(String, u32)| (crate::utility::grown::from_hold(&s.0).0.to_owned(), s.1);
        let cargo: Vec<Option<(String, u32)>> = w.hold.slots.iter().take(SHOWN).map(|s| s.as_ref().map(name)).collect();
        let hidden = w.hold.slots.iter().skip(SHOWN).flatten().map(|s| s.1).sum();
        let height = (w.tool == Some(Tool::Bore) && w.count(Module::Height) > 0).then_some((w.height, w.cell[1]));
        View {
            tier: w.tier,
            tool: w.tool,
            sockets: w.upgrades.iter().map(|m| m.map(|m| m.item().to_owned())).collect(),
            charge: (w.charge / battery.max(1.0)).clamp(0.0, 1.0),
            battery,
            hold: (w.hold.used(), w.hold.slots.len()),
            cargo,
            hidden,
            filter: w.filter.clone(),
            note: w.note,
            height,
            open: false,
            close: false,
        }
    }
}

#[cfg(test)]
mod view_tests {
    use super::*;
    use crate::modules::Module;
    use crate::tuning::Tuning;

    #[test]
    fn a_bores_window_has_the_height_field_with_the_module_only() {
        let t = Tuning::default();
        let mut w = super::super::Worker::new(1, 7, [0, 40, 0], &t);
        w.tool = Some(Tool::Bore);
        assert_eq!(View::of(&w, &t).height, None);
        w.upgrades[0] = Some(Module::Height);
        w.height = Some(30);
        let v = View::of(&w, &t);
        assert_eq!(v.height, Some((Some(30), 40)));
        assert_eq!(v.sockets[0].as_deref(), Some(Module::Height.item()));
        assert_eq!(v.cargo.len(), w.hold.slots.len().min(SHOWN));
    }
}
