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
/// Module sockets of a slice-7 or slice-8 block.
pub const SOCKETS: usize = 3;

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
}

impl Kind {
    pub const ALL: [Kind; 10] = [
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
        }
    }

    pub fn of(block: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.block() == block)
    }

    /// Whose modules its sockets take.
    pub fn holder(self) -> Option<Holder> {
        match self {
            Kind::Turret => Some(Holder::Turret),
            Kind::Anchor => Some(Holder::Anchor),
            Kind::Battery => Some(Holder::Battery),
            Kind::Miner => Some(Holder::Miner),
            Kind::Placer | Kind::Breaker => Some(Holder::Placer),
            _ => None,
        }
    }

    /// Its window's parts: what its server menu showed, part by part.
    pub fn shape(self) -> Shape {
        let sockets = self.holder().is_some();
        match self {
            Kind::Turret => Shape {
                sockets,
                role: true,
                rows: 4,
                bars: 1,
                ..Shape::default()
            },
            Kind::Anchor => Shape {
                sockets,
                rows: 3,
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
            Kind::LavaGenerator => Shape {
                rows: 1,
                bars: 1,
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
}

/// One of the miner's ore toggles: its icon, its name and whether the ore is taken.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Toggle {
    pub icon: String,
    pub name: Value,
    pub on: bool,
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
    pub status: Option<Status>,
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
/// Where the parts start: under the title's rule.
const TOP: f32 = 16.0;
const LEFT: f32 = 8.0;
const RIGHT: f32 = WIDTH - 8.0;
const ROW: f32 = 10.0;
const BAR_STEP: f32 = 18.0;
const TOGGLE: f32 = 20.0;
const TOGGLE_GAP: f32 = 2.0;
const STATUS_H: f32 = 12.0;
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
    /// Where each of the block's slots has its item.
    pub slots: Vec<[f32; 2]>,
    pub status: Rect,
    pub inventory: [f32; 2],
    pub inventory_label_y: f32,
    pub height: f32,
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
        status: [0.0; 4],
        inventory: [0.0; 2],
        inventory_label_y: 0.0,
        height: 0.0,
    };
    if shape.sockets || shape.mode || shape.role {
        if shape.sockets {
            out.sockets = (0..SOCKETS)
                .map(|k| [LEFT + 1.0 + 20.0 * k as f32, y + 1.0])
                .collect();
        }
        let x = if shape.sockets { LEFT + 64.0 } else { LEFT };
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
    if shape.toggles > 0 {
        out.toggles = (0..shape.toggles)
            .map(|i| [LEFT + (TOGGLE + TOGGLE_GAP) * i as f32, y, TOGGLE, TOGGLE])
            .collect();
        y += TOGGLE + TOGGLE_GAP;
    }
    if shape.slots > 0 {
        out.slots = (0..shape.slots)
            .map(|i| {
                [
                    INVENTORY_X + 18.0 * (i % 9) as f32,
                    y + 1.0 + 18.0 * (i / 9) as f32,
                ]
            })
            .collect();
        y += 18.0 * shape.slots.div_ceil(9) as f32 + 2.0;
    }
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
                .filter(|p| matches!(p, Press::Socket(k) if *k < SOCKETS)),
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
    if k >= SOCKETS {
        return None;
    }
    if slots.len() < SOCKETS {
        slots.resize(SOCKETS, None);
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
    let Some(m) = Module::of(item).filter(|m| holder.takes(*m)) else {
        return false;
    };
    if slots.len() < SOCKETS {
        slots.resize(SOCKETS, None);
    }
    match slots.iter().take(SOCKETS).position(Option::is_none) {
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
        out.push(l.status);
        out
    }

    fn overlap(a: Rect, b: Rect) -> bool {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    }

    #[test]
    fn every_window_fits_the_screen_and_its_parts_do_not_overlap() {
        for kind in Kind::ALL {
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
            assert_eq!(l.slots.len(), kind.shape().slots);
        }
    }

    #[test]
    fn the_miner_has_everything_its_server_menu_had() {
        let l = layout(Kind::Miner.shape());
        assert_eq!(l.sockets.len(), SOCKETS);
        assert_eq!(l.mode.len(), 3);
        assert_eq!(l.toggles.len(), 9);
        assert_eq!(l.slots.len(), 27);
        assert_eq!(l.rows.len(), 2);
        let turret = layout(Kind::Turret.shape());
        assert!(turret.role.is_some() && turret.mode.is_empty());
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
        assert_eq!(Press::of(SOCKET_BUTTON + 3), None);
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
