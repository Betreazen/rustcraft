//! The windows of the tank and the pump (guide `ui.js`, windows `tank` and `pump`): what the
//! server sends each through its menu's data slots and where its parts are. Data slots are 16-bit
//! on the network, so a tank's contents go in tenths of a bucket split over two slots, a flow in
//! hundredths of a bucket a second, energy in tenths of E a second.

use crate::machines::panel::{ENERGY_SCALE, PROGRESS_SCALE, Rect, scaled, shown_code, shown_of};
use crate::machines::processor::Shown;
use crate::machines::pump::Source;
use crate::net::fluids::BUCKET;

pub const WIDTH: f32 = 214.0;
pub const TITLE: [f32; 2] = [8.0, 6.0];
/// The largest value one data slot carries.
const SLOT_MAX: u64 = 1 << 15;
/// Millibuckets in the tenth of a bucket a data slot counts in.
const TENTH: u64 = BUCKET / 10;
const FLOW_SCALE: f64 = 100.0;

fn at(data: &[i32], i: usize) -> i32 {
    data.get(i).copied().unwrap_or(0)
}

/// A value of up to 2^30 over two data slots.
fn split(value: u64) -> [i32; 2] {
    let v = value.min(SLOT_MAX * SLOT_MAX - 1);
    [(v / SLOT_MAX) as i32, (v % SLOT_MAX) as i32]
}

fn joined(data: &[i32], i: usize) -> u64 {
    let part = |k| u64::try_from(at(data, k)).unwrap_or(0);
    part(i) * SLOT_MAX + part(i + 1)
}

/// A fluid as a window names it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FluidShown {
    #[default]
    None,
    Water,
    Lava,
    /// A mod's fluid: the window says «жидкость».
    Other,
}

impl FluidShown {
    pub fn of(fluid: Option<&str>) -> FluidShown {
        match fluid {
            None => FluidShown::None,
            Some(crate::machines::pump::WATER) => FluidShown::Water,
            Some(crate::machines::pump::LAVA) => FluidShown::Lava,
            Some(_) => FluidShown::Other,
        }
    }

    fn code(self) -> i32 {
        match self {
            FluidShown::None => 0,
            FluidShown::Water => 1,
            FluidShown::Lava => 2,
            FluidShown::Other => 3,
        }
    }

    fn from_code(code: i32) -> FluidShown {
        match code {
            1 => FluidShown::Water,
            2 => FluidShown::Lava,
            3 => FluidShown::Other,
            _ => FluidShown::None,
        }
    }
}

pub mod tank {
    use super::*;

    pub const DATA_SLOTS: usize = 10;

    /// What else the tank's window says.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Note {
        #[default]
        None,
        /// A tank with another fluid touches it: they stay apart.
        Mixed,
        /// «В канале нет места для новой жидкости»: the tank's fluid found no lane.
        NoLane,
    }

    impl Note {
        fn code(self) -> i32 {
            match self {
                Note::None => 0,
                Note::Mixed => 1,
                Note::NoLane => 2,
            }
        }
        fn from_code(code: i32) -> Note {
            match code {
                1 => Note::Mixed,
                2 => Note::NoLane,
                _ => Note::None,
            }
        }
    }

    /// What the tank's window shows: the whole group of touching tanks.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct TankPanel {
        pub fluid: FluidShown,
        /// In millibuckets, to a tenth of a bucket.
        pub amount: u64,
        pub capacity: u64,
        pub blocks: u32,
        /// Buckets a second coming in and going out, over the last second.
        pub coming: f64,
        pub going: f64,
        pub note: Note,
    }

    pub fn encode(p: &TankPanel) -> Vec<i32> {
        let [ah, al] = split(p.amount / TENTH);
        let [ch, cl] = split(p.capacity / TENTH);
        vec![
            p.fluid.code(),
            ah,
            al,
            ch,
            cl,
            p.blocks.min(SLOT_MAX as u32 - 1) as i32,
            scaled(p.coming, FLOW_SCALE),
            scaled(p.going, FLOW_SCALE),
            p.note.code(),
            0,
        ]
    }

    pub fn decode(data: &[i32]) -> TankPanel {
        TankPanel {
            fluid: FluidShown::from_code(at(data, 0)),
            amount: joined(data, 1) * TENTH,
            capacity: joined(data, 3) * TENTH,
            blocks: u32::try_from(at(data, 5)).unwrap_or(0),
            coming: f64::from(at(data, 6)) / FLOW_SCALE,
            going: f64::from(at(data, 7)) / FLOW_SCALE,
            note: Note::from_code(at(data, 8)),
        }
    }

    pub const GAUGE: Rect = [8.0, 18.0, 22.0, 58.0];
    pub const NAME: [f32; 2] = [36.0, 18.0];
    pub const AMOUNT: [f32; 2] = [36.0, 30.0];
    pub const ROWS: [f32; 2] = [46.0, 58.0];
    pub const LABEL_X: f32 = 36.0;
    /// Two lines high: «В канале нет места для новой жидкости» is longer than one.
    pub const STATUS: Rect = [8.0, 80.0, 198.0, 22.0];
    pub const INVENTORY: [f32; 2] = [26.0, 116.0];
    pub const INVENTORY_LABEL_Y: f32 = INVENTORY[1] - 10.0;
    pub const HEIGHT: f32 = INVENTORY[1] + 80.0;
}

pub mod pump {
    use super::*;

    pub const DATA_SLOTS: usize = 8;

    /// What the pump's window shows.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct PumpPanel {
        pub under: Option<Source>,
        pub endless: bool,
        pub progress: f64,
        pub got: f64,
        pub asked: f64,
        pub shown: Shown,
        /// The fluid its channel had no lane for (M13), none when it found one.
        pub no_lane: bool,
    }

    impl Default for PumpPanel {
        fn default() -> Self {
            PumpPanel {
                under: None,
                endless: false,
                progress: 0.0,
                got: 0.0,
                asked: 0.0,
                shown: Shown::Idle,
                no_lane: false,
            }
        }
    }

    fn source_code(s: Option<Source>) -> i32 {
        match s {
            None => 0,
            Some(Source::Water) => 1,
            Some(Source::Lava) => 2,
        }
    }

    pub fn encode(p: &PumpPanel) -> Vec<i32> {
        let (shown, percent) = shown_code(p.shown);
        vec![
            source_code(p.under),
            i32::from(p.endless),
            scaled(p.progress.clamp(0.0, 1.0), PROGRESS_SCALE),
            scaled(p.got, ENERGY_SCALE),
            scaled(p.asked, ENERGY_SCALE),
            shown,
            percent,
            i32::from(p.no_lane),
        ]
    }

    pub fn decode(data: &[i32]) -> PumpPanel {
        PumpPanel {
            under: match at(data, 0) {
                1 => Some(Source::Water),
                2 => Some(Source::Lava),
                _ => None,
            },
            endless: at(data, 1) != 0,
            progress: f64::from(at(data, 2)) / PROGRESS_SCALE,
            got: f64::from(at(data, 3)) / ENERGY_SCALE,
            asked: f64::from(at(data, 4)) / ENERGY_SCALE,
            shown: shown_of(at(data, 5), at(data, 6)),
            no_lane: at(data, 7) != 0,
        }
    }

    pub const ROWS: [f32; 2] = [18.0, 30.0];
    pub const PROGRESS: Rect = [8.0, 41.0, 198.0, 4.0];
    pub const LABEL_ENERGY_Y: f32 = 50.0;
    pub const ENERGY_BAR: Rect = [8.0, 59.0, 198.0, 5.0];
    pub const LABEL_MODULES_Y: f32 = 68.0;
    pub const MODULE_SLOTS: [[f32; 2]; 3] = [[9.0, 78.0], [27.0, 78.0], [45.0, 78.0]];
    pub const STATUS: Rect = [8.0, 100.0, 198.0, 22.0];
    pub const INVENTORY: [f32; 2] = [26.0, 136.0];
    pub const INVENTORY_LABEL_Y: f32 = INVENTORY[1] - 10.0;
    pub const HEIGHT: f32 = INVENTORY[1] + 80.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tank_panel_comes_back_from_its_data_slots() {
        let p = tank::TankPanel {
            fluid: FluidShown::Water,
            amount: 20_000,
            capacity: 4_000_000,
            blocks: 250,
            coming: 0.1,
            going: 1.0,
            note: tank::Note::NoLane,
        };
        let data = tank::encode(&p);
        assert_eq!(data.len(), tank::DATA_SLOTS);
        assert!(data.iter().all(|&v| (0..=i16::MAX as i32).contains(&v)));
        assert_eq!(tank::decode(&data), p);
    }

    #[test]
    fn a_pump_panel_comes_back_from_its_data_slots() {
        let p = pump::PumpPanel {
            under: Some(Source::Lava),
            endless: false,
            progress: 0.25,
            got: 2.0,
            asked: 4.0,
            shown: Shown::Slowed { percent: 50 },
            no_lane: true,
        };
        let data = pump::encode(&p);
        assert_eq!(data.len(), pump::DATA_SLOTS);
        assert_eq!(pump::decode(&data), p);
    }

    #[test]
    fn the_windows_fit_the_screen() {
        const { assert!(tank::HEIGHT <= 240.0 && pump::HEIGHT <= 240.0) };
    }
}
