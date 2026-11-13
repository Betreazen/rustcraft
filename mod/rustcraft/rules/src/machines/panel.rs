//! A one-input machine's window (the guide's `furnace` and `crusher` windows, spec 12.6): what
//! the server sends it through the menu's data slots, and where its parts are. The window shows
//! the input, the progress arrow, the output, the energy bar with «получает X из Y Е/с», the
//! three work modes (A1) as buttons, the module slots (A5; empty until modules exist) and the
//! status line with the machine's diagnosis (A4).
//!
//! Data slots are 16-bit on the network, so every number goes as a small integer: the progress
//! in thousandths, energy in tenths of E a second.

use crate::machines::processor::{Shown, WorkMode};

/// How many data slots the machine's menu type declares (its file in the content mod).
pub const DATA_SLOTS: usize = 6;
pub const PROGRESS_SCALE: f64 = 1000.0;
pub const ENERGY_SCALE: f64 = 10.0;
/// The largest value a data slot carries.
const MAX_DATA: f64 = i16::MAX as f64;

/// What the window shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    /// Work done on the current recipe, 0 to 1.
    pub progress: f64,
    /// E a second the machine gets now.
    pub got: f64,
    /// E a second it asks for in its mode (when idle, what it would ask).
    pub asked: f64,
    pub mode: WorkMode,
    pub shown: Shown,
}

impl Default for Panel {
    fn default() -> Self {
        Panel {
            progress: 0.0,
            got: 0.0,
            asked: 0.0,
            mode: WorkMode::Normal,
            shown: Shown::Idle,
        }
    }
}

pub fn shown_code(shown: Shown) -> (i32, i32) {
    match shown {
        Shown::Working => (0, 0),
        Shown::Slowed { percent } => (1, percent as i32),
        Shown::NoEnergy => (2, 0),
        Shown::OutputFull => (3, 0),
        Shown::Idle => (4, 0),
        Shown::NoFuel => (5, 0),
        Shown::OnFuel => (6, 0),
        Shown::NoDemand => (7, 0),
    }
}

pub fn shown_of(code: i32, percent: i32) -> Shown {
    match code {
        0 => Shown::Working,
        1 => Shown::Slowed {
            percent: percent.clamp(1, 99) as u32,
        },
        2 => Shown::NoEnergy,
        3 => Shown::OutputFull,
        5 => Shown::NoFuel,
        6 => Shown::OnFuel,
        7 => Shown::NoDemand,
        _ => Shown::Idle,
    }
}

pub fn scaled(value: f64, scale: f64) -> i32 {
    (value * scale).round().clamp(0.0, MAX_DATA) as i32
}

/// The data slots of `panel`, in the order `decode` reads them.
pub fn encode(panel: &Panel) -> Vec<i32> {
    let (shown, percent) = shown_code(panel.shown);
    vec![
        scaled(panel.progress.clamp(0.0, 1.0), PROGRESS_SCALE),
        scaled(panel.got, ENERGY_SCALE),
        scaled(panel.asked, ENERGY_SCALE),
        i32::from(panel.mode.code()),
        shown,
        percent,
    ]
}

/// The panel the data slots say; the client has none until the server's first values come, and
/// a slot it has not had yet reads as the default.
pub fn decode(data: &[i32]) -> Panel {
    let at = |i: usize| data.get(i).copied();
    let d = Panel::default();
    Panel {
        progress: at(0).map_or(d.progress, |v| f64::from(v) / PROGRESS_SCALE),
        got: at(1).map_or(d.got, |v| f64::from(v) / ENERGY_SCALE),
        asked: at(2).map_or(d.asked, |v| f64::from(v) / ENERGY_SCALE),
        mode: at(3)
            .and_then(|v| u8::try_from(v).ok())
            .and_then(WorkMode::from_code)
            .unwrap_or(d.mode),
        shown: at(4).map_or(d.shown, |v| shown_of(v, at(5).unwrap_or(0))),
    }
}

/// An energy number as the window writes it: whole when it is, else one decimal after a comma
/// (Russian style): «8», «2,8».
pub fn energy_text(value: f64) -> String {
    let tenths = (value * 10.0).round() as i64;
    if tenths % 10 == 0 {
        (tenths / 10).to_string()
    } else {
        format!("{},{}", tenths / 10, (tenths % 10).abs())
    }
}

// ---- Layout, in GUI pixels from the window's top left ----

/// A rectangle: x, y, width, height.
pub type Rect = [f32; 4];

pub const WIDTH: f32 = 214.0;
pub const HEIGHT: f32 = 214.0;
/// The player's inventory: its first row and left column (the hotbar is 58 below).
pub const INVENTORY: [f32; 2] = [26.0, 131.0];
/// Where the input and output slots' items go (a slot's frame is one pixel around them).
pub const INPUT: [f32; 2] = [12.0, 28.0];
pub const OUTPUT: [f32; 2] = [110.0, 28.0];
/// The progress arrow's shaft; its head follows it.
pub const ARROW: Rect = [34.0, 33.0, 62.0, 8.0];
pub const ENERGY_BAR: Rect = [8.0, 60.0, 198.0, 6.0];
/// The three work modes' buttons, in `WorkMode::ALL` order.
pub const MODE_BUTTONS: [Rect; 3] = [
    [8.0, 82.0, 64.0, 14.0],
    [75.0, 82.0, 64.0, 14.0],
    [142.0, 82.0, 64.0, 14.0],
];
/// The module slots (A5: three for a machine of tiers 1–2), where their items would go.
pub const MODULE_SLOTS: [[f32; 2]; 3] = [[152.0, 28.0], [171.0, 28.0], [190.0, 28.0]];
pub const STATUS: Rect = [8.0, 102.0, 198.0, 13.0];
/// Label lines: the slots' labels, the energy line, the mode line.
pub const LABEL_SLOTS_Y: f32 = 18.0;
pub const LABEL_ENERGY_Y: f32 = 50.0;
pub const LABEL_MODE_Y: f32 = 72.0;
pub const TITLE: [f32; 2] = [8.0, 6.0];
pub const INVENTORY_LABEL_Y: f32 = INVENTORY[1] - 10.0;
/// A slot's frame around an item drawn at `at`.
pub fn slot_frame(at: [f32; 2]) -> Rect {
    [at[0] - 1.0, at[1] - 1.0, 18.0, 18.0]
}

pub fn inside(rect: Rect, at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

/// The work mode whose button is at `at`.
pub fn mode_button_at(at: [f32; 2]) -> Option<WorkMode> {
    MODE_BUTTONS
        .iter()
        .position(|&r| inside(r, at))
        .map(|i| WorkMode::ALL[i])
}

/// The module slot at `at`.
pub fn module_slot_at(at: [f32; 2]) -> Option<usize> {
    MODULE_SLOTS.iter().position(|&p| inside(slot_frame(p), at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_survives_the_data_slots() {
        let panel = Panel {
            progress: 0.6,
            got: 2.0,
            asked: 8.0,
            mode: WorkMode::Boost,
            shown: Shown::Slowed { percent: 25 },
        };
        let data = encode(&panel);
        assert_eq!(data.len(), DATA_SLOTS);
        assert!(data.iter().all(|&v| (0..=i32::from(i16::MAX)).contains(&v)));
        assert_eq!(decode(&data), panel);
        for shown in [
            Shown::Working,
            Shown::NoEnergy,
            Shown::OutputFull,
            Shown::Idle,
            Shown::NoFuel,
            Shown::OnFuel,
            Shown::NoDemand,
        ] {
            let p = Panel { shown, ..panel };
            assert_eq!(decode(&encode(&p)).shown, shown);
        }
    }

    #[test]
    fn a_window_without_data_yet_shows_the_defaults() {
        assert_eq!(decode(&[]), Panel::default());
        assert_eq!(decode(&[500]).progress, 0.5);
        // A mode the client does not know reads as normal.
        assert_eq!(decode(&[0, 0, 0, 7]).mode, WorkMode::Normal);
    }

    #[test]
    fn big_numbers_are_kept_inside_a_data_slot() {
        let p = Panel {
            got: 1e9,
            asked: -5.0,
            ..Panel::default()
        };
        let data = encode(&p);
        assert_eq!(data[1], i32::from(i16::MAX));
        assert_eq!(data[2], 0);
    }

    #[test]
    fn energy_reads_whole_or_with_one_decimal() {
        assert_eq!(energy_text(8.0), "8");
        assert_eq!(energy_text(2.8), "2,8");
        assert_eq!(energy_text(24.0), "24");
        assert_eq!(energy_text(0.0), "0");
        assert_eq!(energy_text(1.04), "1");
    }

    #[test]
    fn each_mode_button_is_its_own_rectangle_inside_the_window() {
        assert_eq!(mode_button_at([10.0, 85.0]), Some(WorkMode::Economy));
        assert_eq!(mode_button_at([100.0, 90.0]), Some(WorkMode::Normal));
        assert_eq!(mode_button_at([205.0, 95.0]), Some(WorkMode::Boost));
        assert_eq!(mode_button_at([73.0, 85.0]), None);
        assert_eq!(mode_button_at([100.0, 60.0]), None);
        for r in MODE_BUTTONS.iter().chain([&ENERGY_BAR, &STATUS, &ARROW]) {
            assert!(r[0] >= 2.0 && r[0] + r[2] <= WIDTH - 2.0);
            assert!(r[1] + r[3] < INVENTORY_LABEL_Y);
        }
        assert_eq!(module_slot_at([153.0, 30.0]), Some(0));
        assert_eq!(module_slot_at([200.0, 40.0]), Some(2));
        assert_eq!(module_slot_at([140.0, 30.0]), None);
        // The inventory, nine slots wide and the hotbar 58 below, fits inside.
        const { assert!(INVENTORY[0] + 9.0 * 18.0 <= WIDTH) };
        const { assert!(INVENTORY[1] + 58.0 + 18.0 <= HEIGHT) };
    }
}
