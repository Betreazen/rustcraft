//! The windows of slice 4's blocks (guide `ui.js`: `gen_fuel`, `alloy`, `crafter`): what the
//! server sends each through its menu's data slots, where its parts are, and the ids of its
//! buttons. As in `panel`, data slots are 16-bit on the network: progress goes in thousandths,
//! energy in tenths of E a second, and an item as its number in the item table both sides share
//! (rustcraft_content::item_number in the bundle), 0 for none. Every window fits in 240 GUI pixels
//! of height, a 1280×720 screen at GUI scale 3, so a part sits a few pixels from the next.

use crate::machines::panel::{
    ENERGY_SCALE, PROGRESS_SCALE, Rect, inside, scaled, shown_code, shown_of, slot_frame,
};
use crate::machines::processor::{Shown, WorkMode};

/// Every window is as wide as the one-input machine's.
pub const WIDTH: f32 = 214.0;
pub const TITLE: [f32; 2] = [8.0, 6.0];

fn at(data: &[i32], i: usize) -> i32 {
    data.get(i).copied().unwrap_or(0)
}

fn fraction(data: &[i32], i: usize) -> f64 {
    f64::from(at(data, i)) / PROGRESS_SCALE
}

fn energy(data: &[i32], i: usize) -> f64 {
    f64::from(at(data, i)) / ENERGY_SCALE
}

/// An item's number as a data slot carries it: 0 for none, numbers past i16 dropped.
fn item_slot(n: Option<u32>) -> i32 {
    n.filter(|&n| n < i16::MAX as u32)
        .map_or(0, |n| n as i32 + 1)
}

fn item_of(data: &[i32], i: usize) -> Option<u32> {
    u32::try_from(at(data, i))
        .ok()
        .and_then(|v| v.checked_sub(1))
}

// ---- The fuel generator ----

pub mod generator {
    use super::*;

    pub const DATA_SLOTS: usize = 8;

    /// What the generator's window shows.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct GenPanel {
        /// How much of the lit item is left, 0 to 1.
        pub flame: f64,
        /// E a second it gives now, and its full output.
        pub gives: f64,
        pub output: f64,
        /// E17: its circuit's production, demand and what the demand got.
        pub produced: f64,
        pub requested: f64,
        pub supplied: f64,
        pub shown: Option<Shown>,
    }

    pub fn encode(p: &GenPanel) -> Vec<i32> {
        let (shown, percent) = shown_code(p.shown.unwrap_or(Shown::NoFuel));
        vec![
            scaled(p.flame.clamp(0.0, 1.0), PROGRESS_SCALE),
            scaled(p.gives, ENERGY_SCALE),
            scaled(p.output, ENERGY_SCALE),
            scaled(p.produced, ENERGY_SCALE),
            scaled(p.requested, ENERGY_SCALE),
            scaled(p.supplied, ENERGY_SCALE),
            shown,
            percent,
        ]
    }

    pub fn decode(data: &[i32]) -> GenPanel {
        GenPanel {
            flame: fraction(data, 0),
            gives: energy(data, 1),
            output: energy(data, 2),
            produced: energy(data, 3),
            requested: energy(data, 4),
            supplied: energy(data, 5),
            shown: (data.len() > 6).then(|| shown_of(at(data, 6), at(data, 7))),
        }
    }

    pub const FUEL: [f32; 2] = [12.0, 26.0];
    pub const FLAME: Rect = [34.0, 31.0, 80.0, 8.0];
    pub const LABEL_SLOTS_Y: f32 = 16.0;
    pub const LABEL_GIVES_Y: f32 = 47.0;
    pub const GIVES_BAR: Rect = [8.0, 56.0, 198.0, 5.0];
    /// The balance's four rows (E17), each a line of text.
    pub const BALANCE_ROWS: [f32; 4] = [65.0, 75.0, 85.0, 95.0];
    pub const BALANCE: Rect = [8.0, 64.0, 198.0, 40.0];
    pub const STATUS: Rect = [8.0, 106.0, 198.0, 12.0];
    pub const INVENTORY: [f32; 2] = [26.0, 132.0];
    pub const INVENTORY_LABEL_Y: f32 = INVENTORY[1] - 10.0;
    pub const HEIGHT: f32 = INVENTORY[1] + 80.0;
}

// ---- The alloy smelter ----

pub mod alloy {
    use super::*;

    pub const DATA_SLOTS: usize = 8;
    /// Menu button ids of the «брать свой сплав» switch (AL1).
    pub const TAKE_OWN_ON: i32 = 10;
    pub const TAKE_OWN_OFF: i32 = 11;

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct AlloyPanel {
        pub progress: f64,
        /// How much of the lit fuel item is left, 0 to 1.
        pub flame: f64,
        /// E a second it gets and asks for (energy is optional).
        pub got: f64,
        pub asked: f64,
        pub take_own: bool,
        pub shown: Option<Shown>,
    }

    pub fn encode(p: &AlloyPanel) -> Vec<i32> {
        let (shown, percent) = shown_code(p.shown.unwrap_or(Shown::Idle));
        vec![
            scaled(p.progress.clamp(0.0, 1.0), PROGRESS_SCALE),
            scaled(p.flame.clamp(0.0, 1.0), PROGRESS_SCALE),
            scaled(p.got, ENERGY_SCALE),
            scaled(p.asked, ENERGY_SCALE),
            i32::from(p.take_own),
            shown,
            percent,
            1,
        ]
    }

    pub fn decode(data: &[i32]) -> AlloyPanel {
        AlloyPanel {
            progress: fraction(data, 0),
            flame: fraction(data, 1),
            got: energy(data, 2),
            asked: energy(data, 3),
            // The switch is on until the server says otherwise (AL1's default).
            take_own: data.len() < 8 || at(data, 4) != 0,
            shown: (data.len() > 5).then(|| shown_of(at(data, 5), at(data, 6))),
        }
    }

    pub const INPUTS: [[f32; 2]; 4] = [[10.0, 26.0], [28.0, 26.0], [46.0, 26.0], [64.0, 26.0]];
    pub const ARROW: Rect = [86.0, 31.0, 44.0, 8.0];
    pub const OUTPUTS: [[f32; 2]; 3] = [[150.0, 26.0], [168.0, 26.0], [186.0, 26.0]];
    pub const FUEL: [f32; 2] = [10.0, 56.0];
    pub const FLAME: Rect = [32.0, 61.0, 60.0, 6.0];
    pub const MODULE_SLOTS: [[f32; 2]; 3] = [[150.0, 56.0], [168.0, 56.0], [186.0, 56.0]];
    pub const TAKE_OWN: [Rect; 2] = [[8.0, 87.0, 64.0, 13.0], [75.0, 87.0, 64.0, 13.0]];
    pub const ENERGY_BAR: Rect = [8.0, 113.0, 198.0, 5.0];
    pub const STATUS: Rect = [8.0, 121.0, 198.0, 12.0];
    pub const LABEL_SLOTS_Y: f32 = 16.0;
    pub const LABEL_FUEL_Y: f32 = 46.0;
    pub const LABEL_TAKE_Y: f32 = 77.0;
    pub const LABEL_ENERGY_Y: f32 = 104.0;
    pub const INVENTORY: [f32; 2] = [26.0, 147.0];
    pub const INVENTORY_LABEL_Y: f32 = INVENTORY[1] - 10.0;
    pub const HEIGHT: f32 = INVENTORY[1] + 80.0;

    /// The switch's button at `at`: on or off.
    pub fn button_at(at: [f32; 2]) -> Option<i32> {
        if inside(TAKE_OWN[0], at) {
            Some(TAKE_OWN_ON)
        } else if inside(TAKE_OWN[1], at) {
            Some(TAKE_OWN_OFF)
        } else {
            None
        }
    }

    pub fn module_slot_at(at: [f32; 2]) -> Option<usize> {
        MODULE_SLOTS.iter().position(|&p| inside(slot_frame(p), at))
    }
}

// ---- The autocrafter ----

pub mod crafter {
    use super::*;

    /// Pages a crafter can have: one, and one more for each of three capacity modules (C2).
    pub const MAX_PAGES: usize = 4;
    pub const CELLS: usize = 9;
    pub const DATA_SLOTS: usize = 24;
    /// Menu button ids: a page's tab, a pattern cell.
    pub const TAB_BUTTON: i32 = 20;
    pub const CELL_BUTTON: i32 = 30;

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct CrafterPanel {
        pub progress: f64,
        pub got: f64,
        pub asked: f64,
        pub mode: WorkMode,
        pub shown: Option<Shown>,
        /// The page the window shows, how many pages the crafter has now, the page it crafts.
        pub page: usize,
        pub pages: usize,
        pub crafting: Option<usize>,
        /// The shown page's pattern and what it makes, as item numbers.
        pub cells: [Option<u32>; CELLS],
        pub result: Option<u32>,
        pub result_count: u32,
        /// What each page makes, for its tab.
        pub tabs: [Option<u32>; MAX_PAGES],
    }

    pub fn encode(p: &CrafterPanel) -> Vec<i32> {
        let (shown, percent) = shown_code(p.shown.unwrap_or(Shown::Idle));
        let mut data = vec![
            scaled(p.progress.clamp(0.0, 1.0), PROGRESS_SCALE),
            scaled(p.got, ENERGY_SCALE),
            scaled(p.asked, ENERGY_SCALE),
            i32::from(p.mode.code()),
            shown,
            percent,
            p.page.min(MAX_PAGES) as i32,
            p.pages.min(MAX_PAGES) as i32,
            p.crafting.map_or(0, |c| c.min(MAX_PAGES) as i32 + 1),
        ];
        data.extend(p.cells.iter().map(|c| item_slot(*c)));
        data.push(item_slot(p.result));
        data.push(p.result_count.min(i16::MAX as u32) as i32);
        data.extend(p.tabs.iter().map(|c| item_slot(*c)));
        data
    }

    pub fn decode(data: &[i32]) -> CrafterPanel {
        CrafterPanel {
            progress: fraction(data, 0),
            got: energy(data, 1),
            asked: energy(data, 2),
            mode: u8::try_from(at(data, 3))
                .ok()
                .and_then(WorkMode::from_code)
                .unwrap_or_default(),
            shown: (data.len() > 4).then(|| shown_of(at(data, 4), at(data, 5))),
            page: usize::try_from(at(data, 6)).unwrap_or(0).min(MAX_PAGES - 1),
            pages: usize::try_from(at(data, 7))
                .unwrap_or(1)
                .clamp(1, MAX_PAGES),
            crafting: usize::try_from(at(data, 8))
                .ok()
                .and_then(|c| c.checked_sub(1)),
            cells: std::array::from_fn(|i| item_of(data, 9 + i)),
            result: item_of(data, 18),
            result_count: u32::try_from(at(data, 19)).unwrap_or(0),
            tabs: std::array::from_fn(|i| item_of(data, 20 + i)),
        }
    }

    /// Container slots of its menu: the buffer, the output, the module slots.
    pub const BUFFER_SLOTS: usize = 9;
    pub const OUTPUT_SLOTS: usize = 3;
    pub const MODULE_SLOT_COUNT: usize = 3;

    pub const TABS_Y: f32 = 16.0;
    pub const TAB_W: f32 = 28.0;
    pub const TAB_H: f32 = 16.0;
    pub const LABEL_GRID_Y: f32 = 36.0;
    const ROWS: [f32; 3] = [46.0, 64.0, 82.0];
    pub const ARROW: Rect = [66.0, 68.0, 10.0, 8.0];
    pub const RESULT: [f32; 2] = [86.0, 64.0];
    pub const MODULE_SLOTS: [[f32; 2]; 3] = [[152.0, 16.0], [171.0, 16.0], [190.0, 16.0]];
    pub const LABEL_ENERGY_Y: f32 = 102.0;
    pub const ENERGY_BAR: Rect = [8.0, 111.0, 198.0, 5.0];
    pub const MODE_BUTTONS: [Rect; 3] = [
        [8.0, 119.0, 64.0, 13.0],
        [75.0, 119.0, 64.0, 13.0],
        [142.0, 119.0, 64.0, 13.0],
    ];
    pub const STATUS: Rect = [8.0, 135.0, 198.0, 12.0];
    pub const INVENTORY: [f32; 2] = [26.0, 159.0];
    pub const INVENTORY_LABEL_Y: f32 = INVENTORY[1] - 10.0;
    pub const HEIGHT: f32 = INVENTORY[1] + 80.0;
    pub const BUFFER_X: f32 = 112.0;
    pub const OUTPUT_X: f32 = 188.0;

    pub fn tab(i: usize) -> Rect {
        [8.0 + i as f32 * (TAB_W + 2.0), TABS_Y, TAB_W, TAB_H]
    }

    /// Where pattern cell `i` (row by row) has its item.
    pub fn cell(i: usize) -> [f32; 2] {
        [10.0 + (i % 3) as f32 * 18.0, ROWS[i / 3]]
    }

    pub fn buffer(i: usize) -> [f32; 2] {
        [BUFFER_X + (i % 3) as f32 * 18.0, ROWS[i / 3]]
    }

    pub fn output(i: usize) -> [f32; 2] {
        [OUTPUT_X, ROWS[i]]
    }

    /// Where each container slot of the menu goes, in slot order.
    pub fn slot_positions() -> Vec<[f32; 2]> {
        (0..BUFFER_SLOTS)
            .map(buffer)
            .chain((0..OUTPUT_SLOTS).map(output))
            .chain(MODULE_SLOTS)
            .collect()
    }

    /// The button at `at`: a work mode, a tab of a page the crafter has, a pattern cell.
    pub fn button_at(at: [f32; 2], pages: usize) -> Option<i32> {
        if let Some(i) = MODE_BUTTONS.iter().position(|&r| inside(r, at)) {
            return Some(i32::from(WorkMode::ALL[i].code()));
        }
        if let Some(i) = (0..pages.min(MAX_PAGES)).find(|&i| inside(tab(i), at)) {
            return Some(TAB_BUTTON + i as i32);
        }
        cell_at(at).map(|i| CELL_BUTTON + i as i32)
    }

    pub fn cell_at(at: [f32; 2]) -> Option<usize> {
        (0..CELLS).find(|&i| inside(slot_frame(cell(i)), at))
    }

    pub fn module_slot_at(at: [f32; 2]) -> Option<usize> {
        MODULE_SLOTS.iter().position(|&p| inside(slot_frame(p), at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fits(r: Rect, height: f32, inventory_label_y: f32) {
        assert!(r[0] >= 2.0 && r[0] + r[2] <= WIDTH - 2.0, "{r:?}");
        assert!(r[1] >= 2.0 && r[1] + r[3] < inventory_label_y, "{r:?}");
        assert!(inventory_label_y < height);
    }

    #[test]
    fn the_generators_panel_survives_the_data_slots() {
        let p = generator::GenPanel {
            flame: 0.4,
            gives: 16.0,
            output: 16.0,
            produced: 20.0,
            requested: 16.0,
            supplied: 16.0,
            shown: Some(Shown::Working),
        };
        let data = generator::encode(&p);
        assert_eq!(data.len(), generator::DATA_SLOTS);
        assert_eq!(generator::decode(&data), p);
        let none = generator::GenPanel {
            shown: Some(Shown::NoFuel),
            ..Default::default()
        };
        assert_eq!(generator::decode(&generator::encode(&none)), none);
        assert_eq!(generator::decode(&[]).shown, None);
    }

    #[test]
    fn the_alloy_panel_survives_the_data_slots_and_its_switch_starts_on() {
        let p = alloy::AlloyPanel {
            progress: 0.35,
            flame: 0.5,
            got: 0.0,
            asked: 8.0,
            take_own: false,
            shown: Some(Shown::OnFuel),
        };
        let data = alloy::encode(&p);
        assert_eq!(data.len(), alloy::DATA_SLOTS);
        assert_eq!(alloy::decode(&data), p);
        assert!(alloy::decode(&[]).take_own);
        assert_eq!(alloy::button_at([10.0, 90.0]), Some(alloy::TAKE_OWN_ON));
        assert_eq!(alloy::button_at([100.0, 90.0]), Some(alloy::TAKE_OWN_OFF));
        assert_eq!(alloy::button_at([100.0, 60.0]), None);
    }

    #[test]
    fn the_crafter_panel_survives_the_data_slots() {
        let mut p = crafter::CrafterPanel {
            progress: 0.5,
            got: 8.0,
            asked: 8.0,
            mode: WorkMode::Boost,
            shown: Some(Shown::Slowed { percent: 40 }),
            page: 1,
            pages: 2,
            crafting: Some(0),
            result: Some(812),
            result_count: 3,
            ..Default::default()
        };
        p.cells[3] = Some(0);
        p.cells[4] = Some(1500);
        p.tabs = [Some(812), Some(77), None, None];
        let data = crafter::encode(&p);
        assert_eq!(data.len(), crafter::DATA_SLOTS);
        assert!(data.iter().all(|&v| (0..=i32::from(i16::MAX)).contains(&v)));
        assert_eq!(crafter::decode(&data), p);
        let empty = crafter::decode(&[]);
        assert_eq!((empty.page, empty.pages, empty.crafting), (0, 1, None));
    }

    #[test]
    fn the_crafters_buttons() {
        use crafter::*;
        assert_eq!(button_at([10.0, 121.0], 1), Some(0));
        assert_eq!(button_at([150.0, 125.0], 1), Some(2));
        assert_eq!(button_at(cell(0), 1), Some(CELL_BUTTON));
        assert_eq!(
            button_at([cell(8)[0] + 8.0, cell(8)[1] + 8.0], 1),
            Some(CELL_BUTTON + 8)
        );
        let second_tab = [tab(1)[0] + 2.0, tab(1)[1] + 2.0];
        assert_eq!(button_at(second_tab, 1), None);
        assert_eq!(button_at(second_tab, 2), Some(TAB_BUTTON + 1));
        assert_eq!(button_at(buffer(0), 4), None);
        assert_eq!(
            slot_positions().len(),
            BUFFER_SLOTS + OUTPUT_SLOTS + MODULE_SLOT_COUNT
        );
        assert_eq!(module_slot_at([153.0, 18.0]), Some(0));
    }

    #[test]
    fn every_part_fits_its_window() {
        use crafter as c;
        for r in c::MODE_BUTTONS
            .iter()
            .chain([&c::ENERGY_BAR, &c::STATUS, &c::ARROW])
            .copied()
            .chain((0..c::MAX_PAGES).map(c::tab))
            .chain(c::slot_positions().into_iter().map(slot_frame))
            .chain((0..c::CELLS).map(|i| slot_frame(c::cell(i))))
        {
            fits(r, c::HEIGHT, c::INVENTORY_LABEL_Y);
        }
        use alloy as a;
        for r in a::TAKE_OWN
            .iter()
            .chain([&a::ENERGY_BAR, &a::STATUS, &a::ARROW, &a::FLAME])
            .copied()
            .chain(
                a::INPUTS
                    .iter()
                    .chain(&a::OUTPUTS)
                    .chain(&a::MODULE_SLOTS)
                    .map(|p| slot_frame(*p)),
            )
            .chain([slot_frame(a::FUEL)])
        {
            fits(r, a::HEIGHT, a::INVENTORY_LABEL_Y);
        }
        use generator as g;
        for r in [
            g::FLAME,
            g::GIVES_BAR,
            g::BALANCE,
            g::STATUS,
            slot_frame(g::FUEL),
        ] {
            fits(r, g::HEIGHT, g::INVENTORY_LABEL_Y);
        }
        for (inv, h) in [
            (c::INVENTORY, c::HEIGHT),
            (a::INVENTORY, a::HEIGHT),
            (g::INVENTORY, g::HEIGHT),
        ] {
            assert!(inv[0] + 9.0 * 18.0 <= WIDTH);
            assert!(inv[1] + 58.0 + 18.0 <= h);
            // A 1280×720 screen at GUI scale 3 is 240 pixels high.
            assert!(h <= 240.0);
        }
    }
}
