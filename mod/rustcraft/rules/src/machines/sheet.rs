//! The drawn windows of the blocks that used to open server menus (slices 7 and 8): the turret,
//! the base anchor, the docks, the battery, the windmill, the lava generator, the miner, the block
//! placer and the block breaker. Each is now its content mod's menu type, drawn by the client in
//! the «Кузня душ» style (docs/design/04-window-style.md) like the machines' windows.
//!
//! What a window shows is a [`Sheet`]: the server builds it from the same lines its server menu
//! showed (texts as the game's text components, so each client reads them in its own language)
//! and sends it, while the window is open, as a mod message. The window's parts sit where
//! [`layout`] puts them for the block's [`Shape`]; a press on a module socket, a work mode or an
//! ore toggle is a menu button ([`Press`]), and [`press_socket`] is what a socket press does with
//! the cursor, as a slot would.

use crate::geom::Pos;
use crate::machines::panel::{Rect, inside, slot_frame};
use crate::machines::processor::WorkMode;
use crate::machines::style::{Severity, segment_rects};
use crate::modules::{Holder, Module};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The mod message a sheet goes in, server to the players who have its window open.
pub const CHANNEL: &str = "rustcraft:sheet";
/// The largest message a mod may send.
pub const MAX_BYTES: usize = 30_000;
/// Module sockets of a slice-7 or slice-8 block of the first tiers; the window has room for
/// [`MAX_SOCKETS`] (slice 18: tiers 3 and 4 have four, rustcraft_rules::tiers::sockets).
pub const SOCKETS: usize = 3;
pub const MAX_SOCKETS: usize = crate::tiers::MAX_SOCKETS;

/// The blocks with a sheet window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Turret,
    Anchor,
    DockUnload,
    DockCharge,
    Battery,
    Windmill,
    LavaGenerator,
    Miner,
    Placer,
    Breaker,
    /// Slice 10: the mixer, the load dock and the hydro turbine.
    Mixer,
    DockLoad,
    Turbine,
    /// Slice 11: the butcher.
    Butcher,
    /// Slice 12: the last blocks of the guide.
    Util(crate::utility::Util),
    /// Slice 18: a crystal or crystal structure (guide `structure`, ui.js).
    Structure,
}

/// Slice 18, wave 2: the windows the guide draws part by part (`ui.js` trash, disintegrator,
/// charger), on top of a slice-12 window's shape.
fn guide_shape(u: crate::utility::Util, s: Shape) -> Shape {
    use crate::utility::Util;
    match u {
        // Its joint's filter, «самый низкий» priority, the minute's count and the throughput; the
        // fluid and energy bars; «Сгорает сейчас»: the nine cells it burns from and the arrow of
        // its batch (its old filter cells 0..9 hidden). The throughput is a line, not a gauge,
        // and the cells' name is their tooltip: the window keeps to 240 pixels.
        Util::Trash => Shape { rows: 4, bars: 2, shown: Some((9, 18)), lead: 9, work: true, ..s },
        // The share (and «чары теряются»), the kept fractions; durability and energy bars; its
        // input, the arrow and «Разборка», what comes out; the nine output cells.
        Util::Disintegrator => Shape { rows: 2, bars: 2, lead: 1, work: true, preview: 4, ..s },
        // «Броня»: four cells (the output, which the net takes, hidden); the charge and energy
        // bars; no work mode in the guide's window.
        Util::Charger => Shape { rows: 0, bars: 2, mode: false, shown: Some((0, 4)), lead: 4, label: true, ..s },
        // 0.19.2 (guide `fishery`, the owner: «вынеси в отдельную ячейку удочку»): the rod on a row
        // of its own over the catch, its wear and the wait beside it; the rows the water and where
        // the catch goes.
        Util::Fishery => Shape { rows: 2, lead: 1, aside: true, ..s },
        // 0.21 (spec 48): no text rows; its parts are the work area's (EW1).
        Util::Enchanter => Shape { rows: 0, toggles: crate::utility::enchanting::BUTTONS, enchant: true, ..s },
        _ => s,
    }
}

/// Which parts a window has, and how many of each it has room for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub sockets: bool,
    pub mode: bool,
    /// The turret's role plate, beside the sockets.
    pub role: bool,
    /// Lines of text («ключ — значение» lines in the guide).
    pub rows: usize,
    pub bars: usize,
    /// The miner's ore toggles.
    pub toggles: usize,
    /// The block's own slots, in rows of nine.
    pub slots: usize,
    /// Slice 18, wave 2: the menu's slots the window shows (the rest are hidden: the trash's old
    /// filter cells, the charger's output); all when none.
    pub shown: Option<(usize, usize)>,
    /// Slice 18, wave 2: the first shown slots on a row of their own from the left (the mixer's
    /// ingredients, the disintegrator's input, the trash's cells, the charger's «Броня»), the
    /// work arrow after them (`ui.js` `work`) and ghost cells of what comes out after it.
    pub lead: usize,
    pub work: bool,
    pub preview: usize,
    /// A label line over the lead row.
    pub label: bool,
    /// 0.19.2: the sheet's label on the lead row itself, right of its cells, in plain text (the
    /// fishery's rod: its wear and the wait to the next catch beside the rod's own cell).
    pub aside: bool,
    /// 0.21 (spec 46, LP7): the shown slots on the toggles' row, right of them, instead of rows of
    /// their own (the turret's loot beside its target chips: the window keeps to 240 pixels).
    pub beside: bool,
    /// 0.21 (spec 39, RT1): the toggles on the lead row after the work arrow, not on a row of
    /// their own: the mixer's recipes, its ingredients → the arrow → what it makes.
    pub toggles_aside: bool,
    /// 0.21 (spec 48, EW1): the enchanting machine's work area under the energy bar: its three
    /// slots one under another with their labels and the condenser's levels on the left; the
    /// view switch, the level row and four option rows on the right ([`enchant_area`]).
    pub enchant: bool,
}

/// 0.21 (spec 39): the mixer's recipe toggles the window has room for after its arrow.
pub const MIXER_TOGGLES: usize = 5;

impl Shape {
    /// The menu slots shown, in order.
    pub fn shown_range(&self) -> std::ops::Range<usize> {
        match self.shown {
            Some((a, b)) => a.min(self.slots)..b.min(self.slots),
            None => 0..self.slots,
        }
    }
}

impl Kind {
    pub const ALL: [Kind; 15] = [
        Kind::Turret,
        Kind::Anchor,
        Kind::DockUnload,
        Kind::DockCharge,
        Kind::Battery,
        Kind::Windmill,
        Kind::LavaGenerator,
        Kind::Miner,
        Kind::Placer,
        Kind::Breaker,
        Kind::Mixer,
        Kind::DockLoad,
        Kind::Turbine,
        Kind::Butcher,
        Kind::Structure,
    ];

    pub fn block(self) -> &'static str {
        match self {
            Kind::Turret => "rustcraft:turret",
            Kind::Anchor => "rustcraft:anchor",
            Kind::DockUnload => "rustcraft:dock_unload",
            Kind::DockCharge => "rustcraft:dock_charge",
            Kind::Battery => "rustcraft:battery",
            Kind::Windmill => "rustcraft:windmill",
            Kind::LavaGenerator => "rustcraft:gen_lava",
            Kind::Miner => "rustcraft:miner",
            Kind::Placer => "rustcraft:placer",
            Kind::Breaker => "rustcraft:breaker",
            Kind::Mixer => "rustcraft:mixer",
            Kind::DockLoad => "rustcraft:dock_load",
            Kind::Turbine => "rustcraft:turbine",
            Kind::Structure => "rustcraft:crystal",
            Kind::Butcher => crate::butcher::BLOCK,
            Kind::Util(u) => u.spec().block,
        }
    }

    pub fn of(block: &str) -> Option<Kind> {
        Kind::ALL
            .into_iter()
            .find(|k| k.block() == block)
            .or_else(|| crate::utility::Util::of(block).map(Kind::Util))
    }

    /// Whose modules its sockets take.
    pub fn holder(self) -> Option<Holder> {
        match self {
            Kind::Turret => Some(Holder::Turret),
            Kind::Anchor => Some(Holder::Anchor),
            Kind::Battery => Some(Holder::Battery),
            Kind::Miner => Some(Holder::Miner),
            Kind::Placer | Kind::Breaker => Some(Holder::Placer),
            Kind::Mixer => Some(Holder::Mixer),
            Kind::Butcher => Some(Holder::Butcher),
            Kind::Util(u) => Some(Holder::Util(u)),
            _ => None,
        }
    }

    /// Its window's parts: what its server menu showed, part by part.
    pub fn shape(self) -> Shape {
        let sockets = self.holder().is_some_and(|h| h.has_sockets());
        match self {
            // Slice 18 (base and gear): the targets as chips; the support window's players in
            // range, the one healed and the role's working modules. 0.21 (spec 46): its loot slots
            // beside the chips.
            Kind::Turret => Shape {
                sockets,
                role: true,
                rows: 5,
                bars: 1,
                toggles: 4,
                slots: crate::turret_loot::LOOT_SLOTS,
                beside: true,
                ..Shape::default()
            },
            // Slice 18: its owner and area, then the energy bar.
            Kind::Anchor => Shape {
                sockets,
                rows: 2,
                bars: 1,
                ..Shape::default()
            },
            Kind::DockUnload => Shape {
                slots: 9,
                ..Shape::default()
            },
            Kind::DockCharge => Shape {
                rows: 1,
                bars: 1,
                ..Shape::default()
            },
            Kind::Battery => Shape {
                sockets,
                rows: 3,
                bars: 1,
                ..Shape::default()
            },
            Kind::Windmill => Shape {
                rows: 2,
                ..Shape::default()
            },
            // Slice 18 (ui.js `gen_lava`): the lava and output bars, the hint and the circuit's
            // balance.
            Kind::LavaGenerator => Shape {
                rows: 1 + BALANCE_ROWS,
                bars: 2,
                ..Shape::default()
            },
            Kind::Miner => Shape {
                sockets,
                mode: true,
                rows: 2,
                toggles: 9,
                slots: 27,
                ..Shape::default()
            },
            Kind::Placer | Kind::Breaker => Shape {
                sockets,
                mode: true,
                rows: 2,
                slots: 9,
                ..Shape::default()
            },
            // Energy, water and product bars, a line of progress (slice 18: and one of its queue
            // cells), its three input slots.
            // Slice 18, wave 2 (`ui.js` mixer): its three ingredient slots and the work arrow
            // after them; one line, its pace (and its queue cells).
            // 0.21 (spec 39, RT1): its recipes' toggles after the arrow, one a recipe.
            Kind::Mixer => Shape {
                sockets,
                mode: true,
                rows: 1,
                bars: 3,
                slots: 3,
                lead: 3,
                work: true,
                toggles: MIXER_TOGGLES,
                toggles_aside: true,
                ..Shape::default()
            },
            Kind::DockLoad => Shape {
                rows: 1,
                slots: 9,
                ..Shape::default()
            },
            // Slice 18 (ui.js `structure`): crystals and form, the output and solution bars, the
            // circuit's balance.
            Kind::Structure => Shape {
                rows: 2 + BALANCE_ROWS,
                bars: 2,
                ..Shape::default()
            },
            Kind::Turbine => Shape {
                rows: 3,
                ..Shape::default()
            },
            // Its pen's line and the sword's, one toggle an animal (the number to keep), its nine
            // drop slots and the sword slot.
            Kind::Butcher => Shape {
                sockets,
                rows: 2,
                toggles: crate::butcher::ANIMALS.len(),
                slots: crate::butcher::SLOTS,
                ..Shape::default()
            },
            // Slice 12: its lines, its toggles, its slots; a block with only the menu's one hidden
            // slot shows none.
            Kind::Util(u) => {
                let spec = u.spec();
                let count = u.slots().count;
                // Slice 18: the living vein has the work modes and a progress bar (ui.js `vein`).
                let vein = u == crate::utility::Util::Vein;
                guide_shape(u, Shape {
                    sockets,
                    // Slice 18 (UT7): every machine that works in steps has the three modes;
                    // the ones whose pace is the animals' or the water's do not. The living vein
                    // and the craft machines the guide lists (stream 5) have them too.
                    mode: vein
                        || u.has_modes()
                        || crate::modules::Holder::Util(u).takes(crate::modules::Module::Speed),
                    // Slice 18: the guide's windows' rows and bars (`ui.js` trash, disintegrator,
                    // charger, vein, and each craft machine's own bars).
                    rows: spec.rows,
                    bars: if vein {
                        2
                    } else {
                        u.bars().max(1 + usize::from(u == crate::utility::Util::Charger))
                    },
                    toggles: spec.toggles,
                    slots: if count > 1 { count } else { 0 },
                    ..Shape::default()
                })
            }
        }
    }
}

/// A bar: its label and value line over it, its share, and whether it is energy (the energy
/// colour and quarter ticks; otherwise the fluid's or the progress colour).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub label: Value,
    pub value: Value,
    pub share: f64,
    pub energy: bool,
    /// A fluid bar's own colour (ARGB): water blue, the solution green; lava's when none.
    #[serde(default)]
    pub colour: Option<u32>,
}

/// One of the miner's ore toggles: its icon, its name and whether the ore is taken.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Toggle {
    pub icon: String,
    pub name: Value,
    pub on: bool,
    /// 0.21 (spec 39, RT4): the recipe the machine makes now, outlined in the «работает» colour.
    #[serde(default)]
    pub active: bool,
    pub hint: Value,
}

/// The status line: its severity (0 works, 1 works worse, 2 stands) and its text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub severity: u8,
    pub text: Value,
}

impl Status {
    pub fn severity(&self) -> Severity {
        match self.severity {
            0 => Severity::Ok,
            1 => Severity::Warn,
            _ => Severity::Bad,
        }
    }
}

/// The severity's number on the wire.
pub fn severity_code(s: Severity) -> u8 {
    match s {
        Severity::Ok => 0,
        Severity::Warn => 1,
        Severity::Bad => 2,
    }
}

/// What a sheet window shows. Texts are the game's text components (JSON), as the server menus
/// had them in their items' names and lore.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub pos: Pos,
    /// The modules in the sockets (item ids), as many as the block has sockets.
    pub modules: Vec<Option<String>>,
    /// The turret's role: whether it supports, and the plate's text and hint.
    pub support: Option<bool>,
    pub role: Vec<Value>,
    /// The work mode's code.
    pub mode: Option<u8>,
    /// Each row's text, and what its tooltip adds.
    pub rows: Vec<Value>,
    pub hints: Vec<Vec<Value>>,
    pub bars: Vec<Bar>,
    pub toggles: Vec<Toggle>,
    /// How many of the block's slots it uses now (the miner's grow with storage modules).
    pub usable: Option<u8>,
    /// Slice 18, wave 2: the work arrow's share done, the ghost cells (item, count text) and the
    /// lead row's label.
    #[serde(default)]
    pub work: Option<f64>,
    #[serde(default)]
    pub preview: Vec<(String, String)>,
    #[serde(default)]
    pub label: Option<Value>,
    pub status: Option<Status>,
    /// 0.21 (spec 48, EW1): a toggle drawn as a text button: its text (`left\tright` puts the
    /// right part at the button's right end), by the toggle's index; none keeps its icon.
    #[serde(default)]
    pub toggle_texts: Vec<Option<Value>>,
    /// A toggle that does nothing now (an enchantment at its most), drawn sunken and grey.
    #[serde(default)]
    pub toggle_off: Vec<bool>,
    /// Short texts at the layout's [`Layout::notes`]: the slots' labels, the level, the
    /// condenser's levels; and what each one's tooltip adds.
    #[serde(default)]
    pub notes: Vec<Value>,
    #[serde(default)]
    pub note_tips: Vec<Vec<Value>>,
    /// A veiled icon in each shown slot that is empty (what goes there), by the slot's index.
    #[serde(default)]
    pub ghosts: Vec<Option<String>>,
    /// What an empty shown slot's tooltip says, by the slot's index.
    #[serde(default)]
    pub slot_tips: Vec<Vec<Value>>,
}

pub fn encode(sheet: &Sheet) -> Vec<u8> {
    let mut s = sheet.clone();
    let mut bytes = serde_json::to_vec(&s).unwrap_or_default();
    // A sheet never comes near the limit; one that did loses its hints, then its rows.
    while bytes.len() > MAX_BYTES && (!s.hints.is_empty() || !s.rows.is_empty()) {
        if s.hints.pop().is_none() {
            s.rows.pop();
        }
        bytes = serde_json::to_vec(&s).unwrap_or_default();
    }
    bytes
}

pub fn decode(bytes: &[u8]) -> Option<Sheet> {
    serde_json::from_slice(bytes).ok()
}

// ---- Layout, in GUI pixels from the window's top left ----

pub const WIDTH: f32 = 214.0;
/// A window must fit 240 GUI pixels: a 1280 × 720 screen at GUI scale 3.
pub const MAX_HEIGHT: f32 = 240.0;

/// The circuit balance of ui.js `balance`: Производство, Потребление, В запасе, Хватает, one row
/// each, a label on the left and its value on the right (a row of `key	value`).
pub const BALANCE_ROWS: usize = 4;
/// Where the parts start: under the title's rule.
const TOP: f32 = 16.0;
const LEFT: f32 = 8.0;
const RIGHT: f32 = WIDTH - 8.0;
const ROW: f32 = 10.0;
const BAR_STEP: f32 = 18.0;
const TOGGLE: f32 = 20.0;
const TOGGLE_GAP: f32 = 2.0;
const STATUS_H: f32 = 12.0;
/// Slice 18, wave 2: the work arrow's width.
const WORK_W: f32 = 22.0;
/// The player's inventory: three rows, a gap and the hotbar.
const INVENTORY_H: f32 = 76.0;
const INVENTORY_X: f32 = 26.0;

/// Where a window's parts are.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Where each socket's item goes.
    pub sockets: Vec<[f32; 2]>,
    /// The work mode switch's segments, in `WorkMode::ALL` order.
    pub mode: Vec<Rect>,
    pub role: Option<Rect>,
    /// Each row's text line.
    pub rows: Vec<f32>,
    /// Each bar's label line and its well.
    pub bars: Vec<(f32, Rect)>,
    pub toggles: Vec<Rect>,
    /// Where each of the block's shown slots has its item.
    pub slots: Vec<[f32; 2]>,
    /// Slice 18, wave 2: every menu slot's place, none for a hidden one.
    pub menu_slots: Vec<Option<[f32; 2]>>,
    pub work: Option<Rect>,
    pub preview: Vec<[f32; 2]>,
    pub label_y: Option<f32>,
    /// 0.19.2 (`Shape::aside`): where the label's text starts on the lead row.
    pub label_at: Option<[f32; 2]>,
    /// 0.21 (`Shape::enchant`): where the sheet's notes go and how each is written.
    pub notes: Vec<(Rect, Note)>,
    pub status: Rect,
    pub inventory: [f32; 2],
    pub inventory_label_y: f32,
    pub height: f32,
}

/// How a note is written: a label in the guide's capitals, plain text from the left, or a value
/// in the middle of its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    Label,
    Text,
    Value,
}

/// The enchanting machine's notes, in [`Sheet::notes`] order.
pub mod enchant_notes {
    /// The labels beside the item, lapis and result slots.
    pub const ITEM: usize = 0;
    pub const LAPIS: usize = 1;
    pub const RESULT: usize = 2;
    /// «Конденсатор» and its levels under the slots.
    pub const TANK_LABEL: usize = 3;
    pub const TANK: usize = 4;
    /// The level between its −/+ buttons.
    pub const LEVEL: usize = 5;
    pub const COUNT: usize = 6;
}

/// The enchanting machine's work area from `y` (EW1): left, the item, lapis and result slots one
/// under another, each with its label, and the condenser's levels; right, the view switch
/// («Новые чары» / «Повысить»), the level row (−10, −1, the level, +1, +10) and four option rows.
/// Its height.
fn enchant_area(out: &mut Layout, y: f32) -> f32 {
    use crate::utility::enchanting::OPTION_ROWS;
    const RIGHT_X: f32 = 88.0;
    const SLOT_STEP: f32 = 19.0;
    const ROW_H: f32 = 12.0;
    const OPTION_H: f32 = 11.0;
    const OPTION_STEP: f32 = 12.0;
    let w = RIGHT - RIGHT_X;
    out.slots = (0..3).map(|k| [LEFT + 1.0, y + 1.0 + SLOT_STEP * k as f32]).collect();
    let label_x = LEFT + 20.0;
    let mut notes: Vec<(Rect, Note)> = (0..3)
        .map(|k| ([label_x, y + 5.0 + SLOT_STEP * k as f32, RIGHT_X - 2.0 - label_x, 9.0], Note::Text))
        .collect();
    notes.push(([LEFT, y + 58.0, RIGHT_X - 2.0 - LEFT, 9.0], Note::Label));
    notes.push(([LEFT, y + 67.0, RIGHT_X - 2.0 - LEFT, 9.0], Note::Text));
    // The level row: −10, −1, the level, +1, +10.
    let r = y + ROW_H + 2.0;
    let widths = [20.0, 15.0, 0.0, 15.0, 20.0];
    let middle = w - widths.iter().sum::<f32>() - 2.0 * 4.0;
    let mut x = RIGHT_X;
    let mut level = Vec::new();
    for (i, wi) in widths.into_iter().enumerate() {
        let wi = if i == 2 { middle } else { wi };
        if i == 2 {
            notes.push(([x, r, wi, ROW_H], Note::Value));
        } else {
            level.push([x, r, wi, ROW_H]);
        }
        x += wi + 2.0;
    }
    let options = (0..OPTION_ROWS).map(|k| [RIGHT_X, r + ROW_H + 2.0 + OPTION_STEP * k as f32, w, OPTION_H]);
    let half = (w - 2.0) / 2.0;
    let views = [[RIGHT_X, y, half, ROW_H], [RIGHT_X + half + 2.0, y, half, ROW_H]];
    out.toggles = level.into_iter().chain(options).chain(views).collect();
    out.notes = notes;
    let right_end = r + ROW_H + 2.0 + OPTION_STEP * (OPTION_ROWS - 1) as f32 + OPTION_H;
    (right_end - y).max(76.0) + 1.0
}

pub fn layout(shape: Shape) -> Layout {
    let mut y = TOP;
    let mut out = Layout {
        sockets: Vec::new(),
        mode: Vec::new(),
        role: None,
        rows: Vec::new(),
        bars: Vec::new(),
        toggles: Vec::new(),
        slots: Vec::new(),
        menu_slots: Vec::new(),
        work: None,
        preview: Vec::new(),
        label_y: None,
        label_at: None,
        notes: Vec::new(),
        status: [0.0; 4],
        inventory: [0.0; 2],
        inventory_label_y: 0.0,
        height: 0.0,
    };
    if shape.sockets || shape.mode || shape.role {
        if shape.sockets {
            out.sockets = (0..MAX_SOCKETS)
                .map(|k| [LEFT + 1.0 + 20.0 * k as f32, y + 1.0])
                .collect();
        }
        let x = if shape.sockets { LEFT + 84.0 } else { LEFT };
        if shape.mode {
            out.mode = segment_rects([x, y + 2.0, RIGHT - x, 14.0], WorkMode::ALL.len());
        } else if shape.role {
            out.role = Some([x, y + 3.0, RIGHT - x, 12.0]);
        }
        y += 18.0 + 3.0;
    } else {
        // Text right under the title's rule reads cramped: two pixels more.
        y += 2.0;
    }
    if shape.rows > 0 {
        out.rows = (0..shape.rows).map(|i| y + ROW * i as f32).collect();
        y += ROW * shape.rows as f32 + 1.0;
    }
    for _ in 0..shape.bars {
        out.bars.push((y, [LEFT, y + 10.0, RIGHT - LEFT, 6.0]));
        y += BAR_STEP;
    }
    if shape.enchant {
        y += enchant_area(&mut out, y);
        out.menu_slots = (0..shape.slots).map(|i| out.slots.get(i).copied()).collect();
        return finish(out, y);
    }
    let shown = shape.shown_range();
    // 0.21 (spec 46): the slots right of the toggles, on their row, centred on it.
    let beside = shape.beside && shape.toggles > 0;
    if shape.toggles > 0 && !shape.toggles_aside {
        out.toggles = (0..shape.toggles)
            .map(|i| [LEFT + (TOGGLE + TOGGLE_GAP) * i as f32, y, TOGGLE, TOGGLE])
            .collect();
        if beside {
            let frame = LEFT + (TOGGLE + TOGGLE_GAP) * shape.toggles as f32 + 2.0;
            out.slots = (0..shown.len())
                .map(|k| [frame + 1.0 + 18.0 * k as f32, y + 2.0])
                .collect();
        }
        y += TOGGLE + TOGGLE_GAP;
    }
    let lead = if beside { 0 } else { shape.lead.min(shown.len()) };
    if shape.label {
        out.label_y = Some(y);
        y += ROW;
    }
    if lead > 0 || shape.work {
        out.slots.extend((0..lead).map(|k| [LEFT + 1.0 + 18.0 * k as f32, y + 1.0]));
        let mut x = LEFT + 18.0 * lead as f32 + 4.0;
        if shape.aside {
            out.label_at = Some([x, y + 5.0]);
        }
        if shape.work {
            out.work = Some([x, y + 3.0, WORK_W, 12.0]);
            x += WORK_W + 4.0;
        }
        out.preview = (0..shape.preview).map(|k| [x + 1.0 + 18.0 * k as f32, y + 1.0]).collect();
        if shape.toggles_aside {
            // 0.21 (spec 39): the recipes after the arrow, each toggle a pixel over and under
            // the cells beside it.
            out.toggles = (0..shape.toggles)
                .map(|i| [x + (TOGGLE + TOGGLE_GAP) * i as f32, y, TOGGLE, TOGGLE])
                .collect();
            y += TOGGLE + TOGGLE_GAP;
        } else {
            y += 18.0 + 2.0;
        }
    }
    let rest = if beside { 0 } else { shown.len() - lead };
    if rest > 0 {
        out.slots.extend((0..rest).map(|i| {
            [
                INVENTORY_X + 18.0 * (i % 9) as f32,
                y + 1.0 + 18.0 * (i / 9) as f32,
            ]
        }));
        y += 18.0 * rest.div_ceil(9) as f32 + 2.0;
    }
    out.menu_slots = (0..shape.slots)
        .map(|i| shown.contains(&i).then(|| out.slots[i - shown.start]))
        .collect();
    finish(out, y)
}

/// The status line from `y`, then the inventory.
fn finish(mut out: Layout, mut y: f32) -> Layout {
    out.status = [LEFT, y, RIGHT - LEFT, STATUS_H];
    y += STATUS_H + 2.0;
    out.inventory_label_y = y;
    out.inventory = [INVENTORY_X, y + 10.0];
    out.height = out.inventory[1] + INVENTORY_H + 4.0;
    out
}

// ---- Presses ----

const SOCKET_BUTTON: i32 = 40;
const MODE_BUTTON: i32 = 50;
const TOGGLE_BUTTON: i32 = 60;

/// A press on a sheet window, sent as a menu button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Socket(usize),
    Mode(WorkMode),
    Toggle(usize),
}

impl Press {
    pub fn id(self) -> i32 {
        match self {
            Press::Socket(k) => SOCKET_BUTTON + k as i32,
            Press::Mode(m) => MODE_BUTTON + i32::from(m.code()),
            Press::Toggle(i) => TOGGLE_BUTTON + i as i32,
        }
    }

    pub fn of(id: i32) -> Option<Press> {
        match id {
            SOCKET_BUTTON..MODE_BUTTON => Some(Press::Socket((id - SOCKET_BUTTON) as usize))
                .filter(|p| matches!(p, Press::Socket(k) if *k < MAX_SOCKETS)),
            MODE_BUTTON..TOGGLE_BUTTON => u8::try_from(id - MODE_BUTTON)
                .ok()
                .and_then(WorkMode::from_code)
                .map(Press::Mode),
            TOGGLE_BUTTON..100 => Some(Press::Toggle((id - TOGGLE_BUTTON) as usize)),
            _ => None,
        }
    }
}

/// What a press at `at` on a window of `layout` is.
pub fn press_at(layout: &Layout, at: [f32; 2]) -> Option<Press> {
    if let Some(k) = socket_at(layout, at) {
        return Some(Press::Socket(k));
    }
    if let Some(i) = layout.mode.iter().position(|r| inside(*r, at)) {
        return Some(Press::Mode(WorkMode::ALL[i]));
    }
    layout
        .toggles
        .iter()
        .position(|r| inside(*r, at))
        .map(Press::Toggle)
}

pub fn socket_at(layout: &Layout, at: [f32; 2]) -> Option<usize> {
    layout
        .sockets
        .iter()
        .position(|p| inside(slot_frame(*p), at))
}

/// A stack on the cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub item: String,
    pub count: u32,
}

/// A press on socket `k` of a holder's `slots` with `carried` on the cursor, as a slot of one
/// item takes it: a module the holder takes goes in (one of the stack); an empty cursor takes the
/// module out; a single module swaps with the one there. The cursor after it, when anything
/// changed.
pub fn press_socket(
    slots: &mut Vec<Option<Module>>,
    k: usize,
    holder: Holder,
    carried: Option<&Cursor>,
) -> Option<Option<Cursor>> {
    press_socket_in(slots, k, holder, SOCKETS, carried)
}

/// [`press_socket`] on a block with `open` sockets (slice 18: by its tier); a socket past them
/// takes nothing, but a module left there (a block taken down a tier) still comes out.
pub fn press_socket_in(
    slots: &mut Vec<Option<Module>>,
    k: usize,
    holder: Holder,
    open: usize,
    carried: Option<&Cursor>,
) -> Option<Option<Cursor>> {
    if k >= MAX_SOCKETS || (k >= open && carried.is_some()) {
        return None;
    }
    if slots.len() < MAX_SOCKETS {
        slots.resize(MAX_SOCKETS, None);
    }
    let one = |m: Module| Cursor {
        item: m.item().to_owned(),
        count: 1,
    };
    match (carried, slots[k]) {
        (None, Some(old)) => {
            slots[k] = None;
            Some(Some(one(old)))
        }
        (None, None) => None,
        (Some(c), there) => {
            let m = Module::of(&c.item).filter(|m| holder.takes(*m))?;
            match there {
                None => {
                    slots[k] = Some(m);
                    Some((c.count > 1).then(|| Cursor {
                        item: c.item.clone(),
                        count: c.count - 1,
                    }))
                }
                Some(old) if c.count == 1 && old != m => {
                    slots[k] = Some(m);
                    Some(Some(one(old)))
                }
                Some(_) => None,
            }
        }
    }
}

/// A shift-click on a module in the inventory: into the first free socket, if the holder takes
/// it; whether it went in.
pub fn put_free(slots: &mut Vec<Option<Module>>, holder: Holder, item: &str) -> bool {
    put_free_in(slots, holder, SOCKETS, item)
}

/// [`put_free`] on a block with `open` sockets.
pub fn put_free_in(slots: &mut Vec<Option<Module>>, holder: Holder, open: usize, item: &str) -> bool {
    let Some(m) = Module::of(item).filter(|m| holder.takes(*m)) else {
        return false;
    };
    let open = open.min(MAX_SOCKETS);
    if slots.len() < open {
        slots.resize(open, None);
    }
    match slots.iter().take(open).position(Option::is_none) {
        Some(free) => {
            slots[free] = Some(m);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rects(l: &Layout) -> Vec<Rect> {
        let mut out: Vec<Rect> = l.sockets.iter().map(|p| slot_frame(*p)).collect();
        out.extend(l.mode.iter().copied());
        out.extend(l.role);
        out.extend(l.rows.iter().map(|y| [LEFT, *y, RIGHT - LEFT, ROW - 1.0]));
        for (y, r) in &l.bars {
            out.push([LEFT, *y, RIGHT - LEFT, 9.0]);
            out.push(*r);
        }
        out.extend(l.toggles.iter().copied());
        out.extend(l.slots.iter().map(|p| slot_frame(*p)));
        out.extend(l.work);
        out.extend(l.preview.iter().map(|p| slot_frame(*p)));
        out.extend(l.label_y.map(|y| [LEFT, y, RIGHT - LEFT, ROW - 1.0]));
        out.extend(l.label_at.map(|[x, y]| [x, y, RIGHT - x, ROW - 1.0]));
        out.extend(l.notes.iter().map(|(r, _)| *r));
        out.push(l.status);
        out
    }

    #[test]
    fn the_enchanters_window_has_labelled_slots_a_level_row_and_four_option_rows() {
        use crate::utility::{Util, enchanting};
        let shape = Kind::Util(Util::Enchanter).shape();
        assert!(shape.enchant && shape.sockets && shape.mode && shape.rows == 0);
        let l = layout(shape);
        assert!(l.height <= MAX_HEIGHT, "{}", l.height);
        assert_eq!(l.slots.len(), 3);
        assert!(l.menu_slots.iter().all(Option::is_some));
        assert_eq!(l.toggles.len(), enchanting::BUTTONS);
        assert_eq!(l.notes.len(), enchant_notes::COUNT);
        // The slots one under another, each with its label to the right on its own line.
        for k in 0..3 {
            let slot = slot_frame(l.slots[k]);
            let (note, _) = l.notes[k];
            assert!(note[0] >= slot[0] + slot[2] && note[1] >= slot[1] && note[1] + note[3] <= slot[1] + slot[3]);
        }
        assert!(l.slots[1][1] > l.slots[0][1] + 16.0 && l.slots[2][1] > l.slots[1][1] + 16.0);
        // The level between −1 and +1, on one row with the four steps.
        let (level, how) = l.notes[enchant_notes::LEVEL];
        assert_eq!(how, Note::Value);
        assert!(l.toggles[1][0] < level[0] && l.toggles[2][0] > level[0] + level[2]);
        assert!(level[2] >= 36.0, "«ур. 30» fits: {}", level[2]);
        // The option rows wide enough for «Лучшее: Острота IV», under the level row.
        for r in &l.toggles[enchanting::OPTION_BUTTONS] {
            assert!(r[2] >= 110.0 && r[1] > level[1]);
        }
        // A click names the button.
        let o = l.toggles[enchanting::OPTION_BUTTONS.start + 2];
        assert_eq!(press_at(&l, [o[0] + 50.0, o[1] + 5.0]), Some(Press::Toggle(6)));
        let v = l.toggles[enchanting::VIEW_RAISE];
        assert_eq!(press_at(&l, [v[0] + 2.0, v[1] + 2.0]), Some(Press::Toggle(enchanting::VIEW_RAISE)));
    }

    fn overlap(a: Rect, b: Rect) -> bool {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    }

    #[test]
    fn the_structure_and_the_lava_generator_have_the_guides_four_balance_rows() {
        // ui.js `balance`: Производство, Потребление, В запасе, Хватает, one row each (the
        // in-game check found them squeezed into two cut lines).
        assert_eq!(Kind::Structure.shape().rows, 2 + BALANCE_ROWS);
        assert_eq!(Kind::LavaGenerator.shape().rows, 1 + BALANCE_ROWS);
    }

    #[test]
    fn every_window_fits_the_screen_and_its_parts_do_not_overlap() {
        let utils = crate::utility::Util::ALL.into_iter().map(Kind::Util);
        for kind in Kind::ALL.into_iter().chain(utils) {
            let l = layout(kind.shape());
            assert!(l.height <= MAX_HEIGHT, "{kind:?} is {} high", l.height);
            let parts = rects(&l);
            for r in &parts {
                assert!(r[0] >= 3.0 && r[0] + r[2] <= WIDTH - 3.0, "{kind:?} {r:?}");
                assert!(
                    r[1] >= TOP && r[1] + r[3] <= l.inventory_label_y,
                    "{kind:?} {r:?}"
                );
            }
            for (i, a) in parts.iter().enumerate() {
                for b in &parts[i + 1..] {
                    assert!(!overlap(*a, *b), "{kind:?}: {a:?} and {b:?}");
                }
            }
            // The inventory, nine slots wide, inside the window.
            assert!(l.inventory[0] + 9.0 * 18.0 <= WIDTH);
            assert_eq!(l.slots.len(), kind.shape().shown_range().len());
            assert_eq!(l.menu_slots.len(), kind.shape().slots);
        }
    }

    #[test]
    fn the_miner_has_everything_its_server_menu_had() {
        let l = layout(Kind::Miner.shape());
        assert_eq!(l.sockets.len(), MAX_SOCKETS);
        assert_eq!(l.mode.len(), 3);
        assert_eq!(l.toggles.len(), 9);
        assert_eq!(l.slots.len(), 27);
        assert_eq!(l.rows.len(), 2);
        let turret = layout(Kind::Turret.shape());
        assert!(turret.role.is_some() && turret.mode.is_empty());
    }

    #[test]
    fn the_turrets_loot_slots_sit_beside_its_chips_and_cost_no_height() {
        // 0.21 (spec 46, LP7).
        let shape = Kind::Turret.shape();
        let l = layout(shape);
        let without = layout(Shape { slots: 0, beside: false, ..shape });
        assert_eq!(l.height, without.height, "no row of its own");
        assert_eq!(l.slots.len(), crate::turret_loot::LOOT_SLOTS);
        assert_eq!(l.menu_slots.iter().flatten().count(), crate::turret_loot::LOOT_SLOTS);
        let chips = l.toggles.last().copied().unwrap();
        for s in &l.slots {
            let f = slot_frame(*s);
            assert!(f[0] >= chips[0] + chips[2], "right of the chips: {f:?}");
            assert!(f[1] >= chips[1] && f[1] + f[3] <= chips[1] + chips[3], "on their row: {f:?}");
        }
    }

    #[test]
    fn the_mixers_recipes_follow_its_arrow_and_the_kitchens_dishes_are_one_row() {
        // 0.21 (spec 39, RT1): ingredients → the arrow → the recipes; the kitchen's seven dishes
        // side by side where its four pages were.
        let l = layout(Kind::Mixer.shape());
        let arrow = l.work.expect("the arrow");
        assert_eq!(l.toggles.len(), MIXER_TOGGLES);
        for t in &l.toggles {
            assert!(t[0] >= arrow[0] + arrow[2], "{t:?}");
            assert!(t[1] <= arrow[1] && t[1] + t[3] >= arrow[1] + arrow[3], "{t:?}");
        }
        assert_eq!(press_at(&l, [l.toggles[1][0] + 5.0, l.toggles[1][1] + 5.0]), Some(Press::Toggle(1)));
        let kitchen = layout(Kind::Util(crate::utility::Util::Kitchen).shape());
        assert_eq!(kitchen.toggles.len(), crate::utility::KITCHEN_RECIPES.len());
        assert!(kitchen.toggles.iter().all(|t| t[1] == kitchen.toggles[0][1]));
    }

    #[test]
    fn a_socket_past_the_tiers_count_takes_nothing_but_gives_back() {
        let speed = Cursor { item: Module::Speed.item().into(), count: 1 };
        let mut slots = vec![None; MAX_SOCKETS];
        assert_eq!(press_socket_in(&mut slots, 3, Holder::Placer, 3, Some(&speed)), None);
        assert!(press_socket_in(&mut slots, 3, Holder::Placer, 4, Some(&speed)).is_some());
        assert_eq!(slots[3], Some(Module::Speed));
        // Down a tier: the fourth module still comes out.
        assert_eq!(press_socket_in(&mut slots, 3, Holder::Placer, 3, None), Some(Some(speed.clone())));
        let mut slots = vec![Some(Module::Speed), Some(Module::Speed), Some(Module::Speed)];
        assert!(!put_free_in(&mut slots, Holder::Placer, 3, Module::Economy.item()));
        assert!(put_free_in(&mut slots, Holder::Placer, 4, Module::Economy.item()));
        assert_eq!(slots[3], Some(Module::Economy));
    }

    #[test]
    fn a_press_names_its_part_and_survives_its_id() {
        let l = layout(Kind::Miner.shape());
        let s = l.sockets[1];
        assert_eq!(
            press_at(&l, [s[0] + 3.0, s[1] + 3.0]),
            Some(Press::Socket(1))
        );
        let m = l.mode[2];
        assert_eq!(
            press_at(&l, [m[0] + 2.0, m[1] + 2.0]),
            Some(Press::Mode(WorkMode::Boost))
        );
        let t = l.toggles[8];
        assert_eq!(
            press_at(&l, [t[0] + 1.0, t[1] + 1.0]),
            Some(Press::Toggle(8))
        );
        assert_eq!(press_at(&l, [1.0, 1.0]), None);
        for p in [
            Press::Socket(0),
            Press::Socket(2),
            Press::Mode(WorkMode::Economy),
            Press::Mode(WorkMode::Boost),
            Press::Toggle(0),
            Press::Toggle(8),
        ] {
            assert_eq!(Press::of(p.id()), Some(p));
        }
        assert_eq!(Press::of(SOCKET_BUTTON + 3), Some(Press::Socket(3)));
        assert_eq!(Press::of(SOCKET_BUTTON + 4), None);
        assert_eq!(Press::of(0), None);
        assert_eq!(Press::of(MODE_BUTTON + 7), None);
    }

    #[test]
    fn a_socket_takes_and_gives_as_a_slot_does() {
        let poison = Module::Poison.item();
        let heal = Module::Heal.item();
        let mut slots = Vec::new();
        let three = Cursor {
            item: poison.to_owned(),
            count: 3,
        };
        // One of the stack goes in.
        let left = press_socket(&mut slots, 0, Holder::Turret, Some(&three));
        assert_eq!(
            left,
            Some(Some(Cursor {
                item: poison.to_owned(),
                count: 2
            }))
        );
        assert_eq!(slots[0], Some(Module::Poison));
        // A filled socket and a stack: nothing.
        assert_eq!(
            press_socket(&mut slots, 0, Holder::Turret, Some(&three)),
            None
        );
        // A single other module swaps.
        let one_heal = Cursor {
            item: heal.to_owned(),
            count: 1,
        };
        let back = press_socket(&mut slots, 0, Holder::Turret, Some(&one_heal));
        assert_eq!(
            back,
            Some(Some(Cursor {
                item: poison.to_owned(),
                count: 1
            }))
        );
        assert_eq!(slots[0], Some(Module::Heal));
        // An empty cursor takes it out.
        let out = press_socket(&mut slots, 0, Holder::Turret, None);
        assert_eq!(
            out,
            Some(Some(Cursor {
                item: heal.to_owned(),
                count: 1
            }))
        );
        assert_eq!(slots[0], None);
        assert_eq!(press_socket(&mut slots, 0, Holder::Turret, None), None);
        // What the holder does not take, and what is no module, stay on the cursor.
        let area = Cursor {
            item: Module::Area.item().to_owned(),
            count: 1,
        };
        assert_eq!(
            press_socket(&mut slots, 1, Holder::Turret, Some(&area)),
            None
        );
        let dirt = Cursor {
            item: "minecraft:dirt".to_owned(),
            count: 1,
        };
        assert_eq!(
            press_socket(&mut slots, 1, Holder::Turret, Some(&dirt)),
            None
        );
        assert_eq!(
            press_socket(&mut slots, 3, Holder::Turret, Some(&three)),
            None
        );
    }

    #[test]
    fn a_shift_click_fills_the_first_free_socket() {
        let mut slots = vec![Some(Module::Speed), None, None];
        assert!(put_free(&mut slots, Holder::Miner, Module::Area.item()));
        assert_eq!(slots[1], Some(Module::Area));
        assert!(!put_free(&mut slots, Holder::Miner, Module::Poison.item()));
        assert!(put_free(&mut slots, Holder::Miner, Module::Storage.item()));
        assert!(!put_free(&mut slots, Holder::Miner, Module::Speed.item()));
    }

    #[test]
    fn a_lead_row_has_its_slots_the_arrow_and_the_preview_in_turn() {
        let shape = Shape { slots: 10, lead: 1, work: true, preview: 4, label: true, ..Shape::default() };
        let l = layout(shape);
        let w = l.work.expect("an arrow");
        assert!(w[0] > l.slots[0][0] + 16.0);
        assert!(l.preview[0][0] > w[0] + w[2]);
        assert_eq!(l.preview.len(), 4);
        assert_eq!(l.slots.len(), 10);
        // The rest of the slots go under the lead row.
        assert!(l.slots[1][1] > l.slots[0][1] + 16.0);
        assert!(l.label_y.unwrap() < l.slots[0][1]);
        let hidden = layout(Shape { slots: 8, shown: Some((0, 4)), lead: 4, ..Shape::default() });
        assert_eq!(hidden.slots.len(), 4);
        assert!(hidden.menu_slots[..4].iter().all(Option::is_some));
        assert!(hidden.menu_slots[4..].iter().all(Option::is_none));
    }

    #[test]
    fn the_guides_trash_disintegrator_and_charger_windows() {
        use crate::utility::Util;
        let trash = layout(Kind::Util(Util::Trash).shape());
        assert_eq!(trash.slots.len(), 9);
        assert!(trash.menu_slots[..9].iter().all(Option::is_none));
        assert!(trash.work.is_some() && trash.rows.len() == 4 && trash.bars.len() == 2);
        assert!(!trash.mode.is_empty());
        let dis = layout(Kind::Util(Util::Disintegrator).shape());
        assert_eq!((dis.preview.len(), dis.slots.len()), (4, 10));
        assert!(dis.work.is_some());
        let mixer = layout(Kind::Mixer.shape());
        assert!(mixer.work.is_some_and(|w| w[0] > mixer.slots[2][0] + 16.0));
        let charger = Kind::Util(Util::Charger).shape();
        assert!(!charger.mode && charger.label);
        let l = layout(charger);
        assert_eq!(l.slots.len(), 4);
        assert!(l.menu_slots[4..].iter().all(Option::is_none));
        // 0.19.2: the fishing station's rod alone on its row with its line beside it; the catch in
        // rows of nine under it.
        let fishery = Kind::Util(Util::Fishery).shape();
        assert!(fishery.aside && !fishery.label && fishery.lead == 1 && fishery.rows == 2);
        let l = layout(fishery);
        let catch = &l.slots[1..];
        assert_eq!(l.slots.len(), Util::Fishery.slots().count);
        let [x, y] = l.label_at.expect("the rod's line");
        assert!(x > l.slots[0][0] + 16.0 && (l.slots[0][1]..l.slots[0][1] + 16.0).contains(&y));
        assert!(catch.iter().all(|c| c[1] > l.slots[0][1] + 16.0), "the rod has its row");
        assert_eq!(catch[0][0], catch[9][0], "nine to a row");
    }

    #[test]
    fn blocks_without_modules_draw_no_sockets() {
        use crate::utility::Util;
        for u in [Util::ChargePlate, Util::XpTank, Util::Projector, Util::PortalDock] {
            assert!(!Kind::Util(u).shape().sockets, "{u:?}");
            assert!(layout(Kind::Util(u).shape()).sockets.is_empty(), "{u:?}");
        }
        assert!(Kind::Util(Util::Trash).shape().sockets);
        assert!(Kind::Miner.shape().sockets);
    }

    #[test]
    fn a_sheet_survives_its_message() {
        let sheet = Sheet {
            pos: [1, 64, -3],
            modules: vec![Some(Module::Poison.item().to_owned()), None, None],
            support: Some(false),
            role: vec![json!({"translate": "rustcraft.turret.mode_attack"})],
            rows: vec![json!({"text": "Range: 12 blocks"})],
            hints: vec![vec![json!({"text": "a hint"})]],
            bars: vec![Bar {
                label: json!({"text": "Energy"}),
                value: json!({"text": "400 / 800 E"}),
                share: 0.5,
                energy: true,
                colour: None,
            }],
            status: Some(Status {
                severity: severity_code(Severity::Warn),
                text: json!({"text": "No target"}),
            }),
            ..Sheet::default()
        };
        let bytes = encode(&sheet);
        assert!(bytes.len() <= MAX_BYTES);
        assert_eq!(decode(&bytes), Some(sheet.clone()));
        assert_eq!(sheet.status.unwrap().severity(), Severity::Warn);
        assert_eq!(decode(b"not json"), None);
        for kind in Kind::ALL {
            assert_eq!(Kind::of(kind.block()), Some(kind));
        }
    }
}
