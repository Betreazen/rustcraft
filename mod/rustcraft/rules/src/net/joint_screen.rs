//! s18/pipe: the joint window drawn as the guide's table (ui.js `wrench`, «Стык: Электропечь»):
//! a row per slot of the housing with «Гнездо / Канал / Направление / Фильтр / Приоритет» and
//! «Скопировать настройки / Вставить» under it. It is a screen of the mod's own, as the node's
//! window is (`node_window`): the server sends the view of one face (`JointView`), the client
//! draws it and sends back what was pressed (`Press`), the server turns a press into the same
//! `joint_window::Action` a click on the chest page gives and applies it.
//!
//! A housing with several joints shows the faces as tabs over the table (the guide's «Страницы»).
//! A filter item comes from the player's off hand: the wrench is in the main hand, and «+» in a
//! row's filter adds what the other hand holds (a bucket names its fluid on a fluid row).

use serde::{Deserialize, Serialize};

use crate::geom::{Face, Pos};
use crate::net::items::Mode;
use crate::net::joint_window::{Action, Page, pick_for};
use crate::net::settings::{Edit, FILTER_LIMIT};
use crate::net::topology::{ChannelKind, SLOTS};

/// The mod message the view goes in, server to the player whose window it is.
pub const CHANNEL: &str = "rustcraft:joint";
/// The mod message of a press, client to server.
pub const PRESS: &str = "rustcraft:joint_press";

/// One row of the table, numbers and ids only: the client writes the words.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewRow {
    /// The channel in the slot: 0 items, 1 fluids, 2 energy; none for an empty slot.
    pub kind: Option<u8>,
    /// The block on the face works with this channel.
    pub here: bool,
    /// `Mode::code`.
    pub mode: u8,
    pub allow_only: bool,
    pub items: Vec<String>,
    pub priority: i32,
}

impl ViewRow {
    pub fn channel(&self) -> Option<ChannelKind> {
        self.kind.and_then(kind_of_code)
    }

    /// Whether the row's direction can be set.
    pub fn directed(&self) -> bool {
        self.kind.is_some() && self.here
    }

    /// Whether the row has a filter and a priority (items and fluids, not energy).
    pub fn filtered(&self) -> bool {
        self.directed() && self.channel() != Some(ChannelKind::Energy)
    }
}

/// What the window shows of one face of the housing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JointView {
    /// The housing.
    pub pos: Pos,
    /// The face shown (`Face::index`) and the block on it, which names the window.
    pub face: u8,
    pub block: String,
    /// Which faces are joints, in `Face::ALL` order.
    pub joints: [bool; 6],
    pub rows: [ViewRow; SLOTS],
    /// Something was copied.
    pub clipboard: bool,
    /// The first message: open the window.
    pub open: bool,
    /// The last one: the player walked away or the housing is gone.
    pub close: bool,
}

pub fn kind_code(kind: ChannelKind) -> u8 {
    match kind {
        ChannelKind::Item => 0,
        ChannelKind::Fluid => 1,
        ChannelKind::Energy => 2,
    }
}

pub fn kind_of_code(code: u8) -> Option<ChannelKind> {
    match code {
        0 => Some(ChannelKind::Item),
        1 => Some(ChannelKind::Fluid),
        2 => Some(ChannelKind::Energy),
        _ => None,
    }
}

/// The view of `page` for the housing at `pos`, whose selected face has `block` on it.
pub fn view(page: &Page, pos: Pos, block: &str) -> JointView {
    JointView {
        pos,
        face: page.selected.index(),
        block: block.to_owned(),
        joints: page.joints,
        rows: std::array::from_fn(|slot| {
            let row = page.rows[slot];
            let s = &page.settings[slot];
            ViewRow {
                kind: row.kind.map(kind_code),
                here: row.here,
                mode: s.mode.code(),
                allow_only: s.filter.allow_only,
                items: s.filter.items.iter().cloned().collect(),
                priority: s.priority,
            }
        }),
        clipboard: page.clipboard,
        open: false,
        close: false,
    }
}

/// A press in the window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Press {
    /// Show face `Face::index`.
    Face(u8),
    /// The slot's direction, the next one or (`back`) the one before.
    Mode { slot: usize, back: bool },
    /// White list ↔ black list.
    List { slot: usize },
    /// Take `item` off the slot's filter.
    Remove { slot: usize, item: String },
    /// Put the item of the player's off hand on the slot's filter.
    Add { slot: usize },
    Priority { slot: usize, delta: i32 },
    Copy,
    Paste,
    /// The player closed the window.
    Close,
}

/// What `press` asks of the joint `page`, when the player's off hand holds `held`. A press the
/// page cannot take (a face that is no joint, a filter on an energy row, a slot past the four)
/// asks nothing: a client is never trusted further than the page it was sent.
pub fn action(page: &Page, press: &Press, held: Option<&str>) -> Action {
    let row = |slot: usize| page.rows.get(slot).copied();
    let directed = |slot| row(slot).is_some_and(|r| r.kind.is_some() && r.here);
    let filtered = |slot| directed(slot) && row(slot).and_then(|r| r.kind) != Some(ChannelKind::Energy);
    let edit = |slot, edit| Action::Edit { slot, edit };
    match press {
        Press::Face(i) => match Face::from_index(*i) {
            Some(f) if page.joints[usize::from(*i)] => Action::Select(f),
            _ => Action::None,
        },
        Press::Mode { slot, back } if directed(*slot) => {
            edit(*slot, if *back { Edit::PreviousMode } else { Edit::NextMode })
        }
        Press::List { slot } if filtered(*slot) => edit(*slot, Edit::ToggleList),
        Press::Remove { slot, item } if filtered(*slot) && page.settings[*slot].filter.items.contains(item) => {
            edit(*slot, Edit::RemoveItem(item.clone()))
        }
        Press::Add { slot } if filtered(*slot) && page.settings[*slot].filter.items.len() < FILTER_LIMIT => {
            let Some(item) = held else {
                return Action::None;
            };
            let armed = Page { armed: Some(*slot), ..page.clone() };
            pick_for(&armed, item)
        }
        Press::Priority { slot, delta } if filtered(*slot) => edit(*slot, Edit::Priority(delta.signum())),
        Press::Copy => Action::Copy,
        Press::Paste if page.clipboard => Action::Paste,
        _ => Action::None,
    }
}

#[derive(Serialize, Deserialize)]
struct Wire {
    pos: Pos,
    press: Press,
}

pub fn encode(view: &JointView) -> Vec<u8> {
    serde_json::to_vec(view).unwrap_or_default()
}

pub fn decode(bytes: &[u8]) -> Option<JointView> {
    serde_json::from_slice(bytes).ok()
}

pub fn encode_press(pos: Pos, press: &Press) -> Vec<u8> {
    serde_json::to_vec(&Wire { pos, press: press.clone() }).unwrap_or_default()
}

pub fn decode_press(bytes: &[u8]) -> Option<(Pos, Press)> {
    serde_json::from_slice::<Wire>(bytes).ok().map(|w| (w.pos, w.press))
}

/// The mode of a row as the window names it.
pub fn mode_of(row: &ViewRow) -> Mode {
    Mode::from_code(row.mode).unwrap_or(Mode::Both)
}

// ---- Layout, in GUI pixels from the window's top left (docs/design/04-window-style.md) ----

/// A rectangle: x, y, width, height.
pub type Rect = [f32; 4];

pub const WIDTH: f32 = 354.0;
pub const HEIGHT: f32 = 172.0;
const LEFT: f32 = 8.0;
/// The face tabs under the title.
pub const TABS_Y: f32 = 18.0;
const TAB_H: f32 = 12.0;
const TAB_GAP: f32 = 2.0;
/// The table's head and its four rows.
pub const HEAD_Y: f32 = 36.0;
pub const ROWS_Y: f32 = 47.0;
pub const ROW_H: f32 = 20.0;
/// The columns' left edges: «Гнездо», «Канал», «Направление», «Фильтр», «Приоритет».
pub const COLUMNS: [f32; 5] = [LEFT, 42.0, 100.0, 182.0, 292.0];
const MODE_W: f32 = 78.0;
const LIST_W: f32 = 36.0;
const BUTTON_H: f32 = 14.0;
/// The filter's item cells after the list button, then «+».
const FILTER_ITEMS_X: f32 = COLUMNS[3] + LIST_W + 3.0;
const ITEM_STEP: f32 = 18.0;
const SMALL: f32 = 12.0;
/// «Скопировать настройки» and «Вставить».
pub const BUTTONS_Y: f32 = ROWS_Y + ROW_H * SLOTS as f32 + 4.0;
pub const COPY: Rect = [LEFT, BUTTONS_Y, 130.0, BUTTON_H];
pub const PASTE: Rect = [LEFT + 134.0, BUTTONS_Y, 70.0, BUTTON_H];
/// The hint line at the bottom.
pub const HINT_Y: f32 = BUTTONS_Y + BUTTON_H + 6.0;

fn row_y(slot: usize) -> f32 {
    ROWS_Y + ROW_H * slot as f32
}

/// The six face tabs, in `Face::ALL` order.
pub fn tab_rects() -> [Rect; 6] {
    let w = ((WIDTH - 2.0 * LEFT - 5.0 * TAB_GAP) / 6.0).floor();
    std::array::from_fn(|i| [LEFT + (w + TAB_GAP) * i as f32, TABS_Y, w, TAB_H])
}

/// The direction button of a slot's row.
pub fn mode_rect(slot: usize) -> Rect {
    [COLUMNS[2], row_y(slot) + 3.0, MODE_W, BUTTON_H]
}

/// The white/black list button of a slot's row.
pub fn list_rect(slot: usize) -> Rect {
    [COLUMNS[3], row_y(slot) + 3.0, LIST_W, BUTTON_H]
}

/// Where the filter's `i`-th item is drawn (its well is a pixel round it).
pub fn item_at(slot: usize, i: usize) -> [f32; 2] {
    [FILTER_ITEMS_X + 1.0 + ITEM_STEP * i as f32, row_y(slot) + 2.0]
}

/// «+» after the filter's items.
pub fn add_rect(slot: usize) -> Rect {
    [FILTER_ITEMS_X + ITEM_STEP * FILTER_LIMIT as f32 + 1.0, row_y(slot) + 4.0, SMALL, SMALL]
}

/// «−» and «+» round the priority.
pub fn priority_rects(slot: usize) -> [Rect; 2] {
    let y = row_y(slot) + 4.0;
    [[COLUMNS[4], y, SMALL, SMALL], [COLUMNS[4] + 30.0, y, SMALL, SMALL]]
}

fn inside(r: Rect, at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

/// What a click (`right`: the right button) at `at` (window pixels) presses in `view`.
pub fn press_at(view: &JointView, at: [f32; 2], right: bool) -> Option<Press> {
    if let Some(i) = tab_rects().iter().position(|r| inside(*r, at)) {
        return view.joints[i].then_some(Press::Face(i as u8));
    }
    if inside(COPY, at) {
        return Some(Press::Copy);
    }
    if inside(PASTE, at) {
        return view.clipboard.then_some(Press::Paste);
    }
    for (slot, row) in view.rows.iter().enumerate() {
        if row.directed() && inside(mode_rect(slot), at) {
            return Some(Press::Mode { slot, back: right });
        }
        if !row.filtered() {
            continue;
        }
        if inside(list_rect(slot), at) {
            return Some(Press::List { slot });
        }
        for (i, item) in row.items.iter().enumerate() {
            let p = item_at(slot, i);
            if inside([p[0] - 1.0, p[1] - 1.0, ITEM_STEP, ITEM_STEP], at) {
                return Some(Press::Remove { slot, item: item.clone() });
            }
        }
        if row.items.len() < FILTER_LIMIT && inside(add_rect(slot), at) {
            return Some(Press::Add { slot });
        }
        let [down, up] = priority_rects(slot);
        if inside(down, at) {
            return Some(Press::Priority { slot, delta: -1 });
        }
        if inside(up, at) {
            return Some(Press::Priority { slot, delta: 1 });
        }
    }
    None
}

/// The window's top left on a screen `size` GUI pixels big: in the middle.
pub fn origin(size: [f32; 2]) -> [f32; 2] {
    [((size[0] - WIDTH) / 2.0).floor().max(0.0), ((size[1] - HEIGHT) / 2.0).floor().max(0.0)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::joint_window::Row;
    use crate::net::settings::JointSetting;

    /// The guide's example: «Стык: Электропечь», items into the machine, energy, items out, empty.
    fn page() -> Page {
        let mut joints = [false; 6];
        joints[usize::from(Face::YPos.index())] = true;
        joints[usize::from(Face::XPos.index())] = true;
        let row = |kind, here| Row { kind, here };
        let mut settings: [JointSetting; SLOTS] = std::array::from_fn(|_| JointSetting::of_mode(Mode::Into));
        settings[2] = JointSetting::of_mode(Mode::From);
        settings[0] = settings[0].apply(&Edit::ToggleList).apply(&Edit::AddItem("minecraft:raw_iron".into()));
        Page {
            joints,
            selected: Face::YPos,
            rows: [
                row(Some(ChannelKind::Item), true),
                row(Some(ChannelKind::Energy), true),
                row(Some(ChannelKind::Item), true),
                row(None, false),
            ],
            settings,
            armed: None,
            clipboard: false,
        }
    }

    #[test]
    fn the_view_is_the_guides_table() {
        let v = view(&page(), [1, 2, 3], "rustcraft:furnace");
        assert_eq!(v.face, Face::YPos.index());
        assert_eq!(v.block, "rustcraft:furnace");
        assert_eq!(v.rows[0].channel(), Some(ChannelKind::Item));
        assert!(v.rows[0].allow_only, "white list");
        assert_eq!(v.rows[0].items, vec!["minecraft:raw_iron".to_owned()]);
        assert_eq!(mode_of(&v.rows[0]), Mode::Into);
        assert_eq!(mode_of(&v.rows[2]), Mode::From);
        assert!(v.rows[1].directed() && !v.rows[1].filtered(), "energy: a direction, no filter");
        assert!(!v.rows[3].directed(), "an empty slot: only dashes");
    }

    #[test]
    fn a_press_asks_what_the_chest_page_would() {
        let p = page();
        assert_eq!(action(&p, &Press::Face(Face::XPos.index()), None), Action::Select(Face::XPos));
        assert_eq!(action(&p, &Press::Face(Face::XNeg.index()), None), Action::None, "no joint there");
        assert_eq!(action(&p, &Press::Face(9), None), Action::None);
        assert_eq!(
            action(&p, &Press::Mode { slot: 1, back: true }, None),
            Action::Edit { slot: 1, edit: Edit::PreviousMode }
        );
        assert_eq!(action(&p, &Press::Mode { slot: 3, back: false }, None), Action::None);
        assert_eq!(action(&p, &Press::List { slot: 1 }, None), Action::None, "energy has no filter");
        assert_eq!(
            action(&p, &Press::Priority { slot: 0, delta: 5 }, None),
            Action::Edit { slot: 0, edit: Edit::Priority(1) },
            "a step at a time"
        );
        assert_eq!(action(&p, &Press::Priority { slot: 7, delta: 1 }, None), Action::None);
        assert_eq!(action(&p, &Press::Paste, None), Action::None, "nothing copied");
        assert_eq!(action(&p, &Press::Copy, None), Action::Copy);
    }

    #[test]
    fn the_filter_takes_the_off_hands_item() {
        let p = page();
        assert_eq!(
            action(&p, &Press::Add { slot: 0 }, Some("minecraft:raw_copper")),
            Action::Edit { slot: 0, edit: Edit::AddItem("minecraft:raw_copper".into()) }
        );
        assert_eq!(action(&p, &Press::Add { slot: 0 }, None), Action::None, "an empty off hand");
        assert_eq!(
            action(&p, &Press::Remove { slot: 0, item: "minecraft:raw_iron".into() }, None),
            Action::Edit { slot: 0, edit: Edit::RemoveItem("minecraft:raw_iron".into()) }
        );
        assert_eq!(
            action(&p, &Press::Remove { slot: 0, item: "minecraft:dirt".into() }, None),
            Action::None,
            "not on the filter"
        );
        let mut fluid = page();
        fluid.rows[2].kind = Some(ChannelKind::Fluid);
        assert_eq!(
            action(&fluid, &Press::Add { slot: 2 }, Some("minecraft:lava_bucket")),
            Action::Edit { slot: 2, edit: Edit::AddItem("minecraft:lava".into()) }
        );
        let mut full = page();
        for i in ["a:1", "a:2"] {
            full.settings[0] = full.settings[0].apply(&Edit::AddItem(i.into()));
        }
        assert_eq!(action(&full, &Press::Add { slot: 0 }, Some("a:3")), Action::None, "three items at most");
    }

    #[test]
    fn a_click_finds_its_cell() {
        let mut v = view(&page(), [0, 0, 0], "rustcraft:furnace");
        let mid = |r: Rect| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
        assert_eq!(press_at(&v, mid(tab_rects()[usize::from(Face::XPos.index())]), false), Some(Press::Face(Face::XPos.index())));
        assert_eq!(press_at(&v, mid(tab_rects()[usize::from(Face::XNeg.index())]), false), None);
        assert_eq!(press_at(&v, mid(mode_rect(0)), true), Some(Press::Mode { slot: 0, back: true }));
        assert_eq!(press_at(&v, mid(list_rect(1)), false), None, "energy has no list");
        assert_eq!(press_at(&v, mid(list_rect(0)), false), Some(Press::List { slot: 0 }));
        let first = item_at(0, 0);
        assert_eq!(
            press_at(&v, [first[0] + 8.0, first[1] + 8.0], false),
            Some(Press::Remove { slot: 0, item: "minecraft:raw_iron".into() })
        );
        assert_eq!(press_at(&v, mid(add_rect(0)), false), Some(Press::Add { slot: 0 }));
        let [down, up] = priority_rects(2);
        assert_eq!(press_at(&v, mid(down), false), Some(Press::Priority { slot: 2, delta: -1 }));
        assert_eq!(press_at(&v, mid(up), false), Some(Press::Priority { slot: 2, delta: 1 }));
        assert_eq!(press_at(&v, mid(mode_rect(3)), false), None, "an empty slot");
        assert_eq!(press_at(&v, mid(COPY), false), Some(Press::Copy));
        assert_eq!(press_at(&v, mid(PASTE), false), None);
        v.clipboard = true;
        assert_eq!(press_at(&v, mid(PASTE), false), Some(Press::Paste));
    }

    #[test]
    fn the_table_fits_its_window() {
        let [_, up] = priority_rects(0);
        assert!(up[0] + up[2] <= WIDTH - LEFT);
        assert!(add_rect(0)[0] + add_rect(0)[2] < COLUMNS[4]);
        assert!(mode_rect(0)[0] + mode_rect(0)[2] < COLUMNS[3]);
        assert!(HINT_Y + 8.0 <= HEIGHT);
        let tabs = tab_rects();
        assert!(tabs[5][0] + tabs[5][2] <= WIDTH - LEFT);
    }

    #[test]
    fn a_view_and_a_press_go_over_the_wire() {
        let v = JointView { open: true, ..view(&page(), [4, 5, 6], "minecraft:chest") };
        assert_eq!(decode(&encode(&v)), Some(v));
        let p = Press::Remove { slot: 2, item: "minecraft:lava".into() };
        assert_eq!(decode_press(&encode_press([1, 2, 3], &p)), Some(([1, 2, 3], p)));
    }
}
