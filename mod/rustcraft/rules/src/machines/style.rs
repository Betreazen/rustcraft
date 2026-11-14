//! The «Кузня душ» window style (docs/design/04-window-style.md): its colours and the rectangles
//! every part of a block window is drawn from. The game draws in immediate mode with filled
//! rectangles (`native_fill`), so each part here is a list of [`Fill`]s in window pixels; the
//! client only adds the window's origin and draws them in order. Sizes are GUI pixels (gp).
//!
//! Section 2 of the style document is [`colour`]; sections 1 and 3 are the functions below.

use crate::machines::panel::Rect;

/// The window tokens of `docs/showcase/ui.css` as the game draws them: fills as 0xAARRGGBB, text
/// as 0xRRGGBB (the game's text takes no alpha).
pub mod colour {
    /// `--w-outline`: the window's outer edge.
    pub const OUTLINE: u32 = 0xFF0B_0908;
    /// `--w-frame`: the brass line of the frame, the active tab's edge, the tooltip's frame.
    pub const FRAME: u32 = 0xFF6E_5526;
    /// `--w-bevel-hi`: the frame's light top and left, the buttons' light edge.
    pub const BEVEL_HI: u32 = 0xFF4A_3C31;
    /// `--w-bevel-lo`: the frame's shade bottom and right, the title's rule, table lines.
    pub const BEVEL_LO: u32 = 0xFF14_0F0C;
    /// `--w-face`: the window's iron face.
    pub const FACE: u32 = 0xFF2A_221D;
    /// `--w-well`: slots, bars, fields, tanks.
    pub const WELL: u32 = 0xFF11_0D0B;
    /// `--w-well-lo`: a well's dark top and left edge.
    pub const WELL_LO: u32 = 0xFF06_0504;
    /// `--w-well-hi`: a well's light bottom and right edge, an empty socket's plus, a dark arrow
    /// head.
    pub const WELL_HI: u32 = 0xFF4A_3C31;
    /// `--w-mark`: a module socket's corner.
    pub const MARK: u32 = 0xFFC9_A24A;
    /// `--w-on`: a chosen segment, a switched-on toggle.
    pub const ON: u32 = 0xFFC9_A24A;
    /// The brass face's light top and left (35% white on brass) and dark bottom and right (30%
    /// black on brass).
    pub const ON_LIGHT: u32 = 0xFFDC_C289;
    pub const ON_SHADE: u32 = 0xFF8D_7134;
    /// `--w-hot`: hover and focus outline, the caret.
    pub const HOT: u32 = 0xFFFF_8A3D;
    /// `--w-progress`: the progress arrow's fill.
    pub const PROGRESS: u32 = 0xFFEF_E6D6;
    /// `--w-energy`: energy, the resource colour of the guide's scenes.
    pub const ENERGY: u32 = 0xFFFF_E14A;
    /// `--spark`: a fuel's flame.
    pub const FLAME: u32 = 0xFFFF_9A3C;
    /// `--w-ok`, `--w-warn`, `--w-bad`: works, works worse, stands.
    pub const OK: u32 = 0xFF5F_E3D0;
    pub const WARN: u32 = 0xFFF2_C14E;
    pub const BAD: u32 = 0xFFE5_483A;
    /// The status line's background by severity: 14% of its colour in the well.
    pub const OK_BACK: u32 = 0xFF1C_2B27;
    pub const WARN_BACK: u32 = 0xFF31_2614;
    pub const BAD_BACK: u32 = 0xFF2F_1512;
    /// The rule between key and value lines: 28% black on the face.
    pub const KV_RULE: u32 = 0xFF1E_1815;
    /// A button that is not on (not brass).
    pub const BUTTON: u32 = 0xFF3A_2F27;
    /// The turret's role plates: background and stripe.
    pub const ATTACK_BACK: u32 = 0xFF4A_1F1A;
    pub const ATTACK_STRIPE: u32 = 0xFFE5_483A;
    pub const SUPPORT_BACK: u32 = 0xFF12_403A;
    pub const SUPPORT_STRIPE: u32 = 0xFF5F_E3D0;
    /// The schemes of stations: parchment, ink, the line of the way, the chosen station.
    pub const PAPER: u32 = 0xFFE6_D8BB;
    pub const PAPER_INK: u32 = 0xFF22_180F;
    pub const PAPER_LINE: u32 = 0xFFA3_762C;
    pub const PAPER_CHOSEN: u32 = 0xFF3F_B6C8;
    /// The light line on top of a bar's fill: white at 35%.
    pub const SHEEN: u32 = 0x59FF_FFFF;
    /// The light line on a fluid's top: white at 40%.
    pub const FLUID_TOP: u32 = 0x66FF_FFFF;
    /// The energy bar's quarter ticks: black at 45%; a tank's ticks: black at 50%.
    pub const TICK: u32 = 0x73_000000;
    pub const TANK_TICK: u32 = 0x80_000000;
    /// A sample item's veil (the item is not there, it is a pattern): the well at 65%.
    pub const GHOST: u32 = 0xA611_0D0B;

    // Text, 0xRRGGBB.
    /// `--w-text`: ordinary text.
    pub const TEXT: u32 = 0xEF_E6D6;
    /// `--w-value`: values, the title, the status line's text.
    pub const VALUE: u32 = 0xFB_F5EA;
    /// `--w-label`: parts' labels, units, keys.
    pub const LABEL: u32 = 0xA8_9A86;
    /// `--w-on-text`: text on brass.
    pub const ON_TEXT: u32 = 0x1A_0D05;
    /// Text on the parchment of a scheme.
    pub const PAPER_TEXT: u32 = 0x22_180F;
    /// A broken way's line in a list: the bad colour, readable on the face.
    pub const BAD_TEXT: u32 = 0xE5_483A;
}

/// A filled rectangle in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    pub rect: Rect,
    pub argb: u32,
}

const fn fill(x: f32, y: f32, w: f32, h: f32, argb: u32) -> Fill {
    Fill {
        rect: [x, y, w, h],
        argb,
    }
}

/// Section 1: the sizes of the grid.
pub const PADDING: f32 = 7.0;
/// Outline, brass line, bevel: three pixels from the outside to the face.
pub const FRAME_WIDTH: f32 = 3.0;
pub const SLOT: f32 = 18.0;
pub const SOCKET_GAP: f32 = 2.0;
pub const BAR_HEIGHT: f32 = 8.0;
pub const ARROW_SHAFT: [f32; 2] = [22.0, 6.0];
pub const ARROW_HEAD: [f32; 2] = [6.0, 12.0];
pub const TANK: [f32; 2] = [16.0, 36.0];
pub const GLYPH: f32 = 7.0;
/// The status line's stripe on its left.
pub const STRIPE: f32 = 2.0;
/// From the status line's left to its glyph, and from the glyph to the text.
pub const STATUS_PAD: f32 = 4.0;

/// The window's frame (section 3, «Рамка»): outline, a brass line one pixel in, light top and
/// left, shade bottom and right, the face.
pub fn frame(size: [f32; 2]) -> Vec<Fill> {
    let [w, h] = size;
    vec![
        fill(0.0, 0.0, w, h, colour::OUTLINE),
        fill(1.0, 1.0, w - 2.0, h - 2.0, colour::FRAME),
        fill(2.0, 2.0, w - 4.0, h - 4.0, colour::BEVEL_LO),
        fill(2.0, 2.0, w - 5.0, h - 5.0, colour::BEVEL_HI),
        fill(3.0, 3.0, w - 6.0, h - 6.0, colour::FACE),
    ]
}

/// A well (slot, bar, field, tank): dark top and left edge, light bottom and right edge, the
/// well inside.
pub fn well(r: Rect) -> Vec<Fill> {
    let [x, y, w, h] = r;
    vec![
        fill(x, y, w, h, colour::WELL_HI),
        fill(x, y, w - 1.0, h - 1.0, colour::WELL_LO),
        fill(x + 1.0, y + 1.0, w - 2.0, h - 2.0, colour::WELL),
    ]
}

/// A slot's well around an item drawn at `item` (the 16 × 16 icon sits one pixel in).
pub fn slot_rect(item: [f32; 2]) -> Rect {
    [item[0] - 1.0, item[1] - 1.0, SLOT, SLOT]
}

pub fn slot(item: [f32; 2]) -> Vec<Fill> {
    well(slot_rect(item))
}

/// A module socket: a slot with a brass corner 3 × 3 at its top left, two pixels in; an empty one
/// shows a plus 6 × 6 in its middle (drawn always: an item's icon covers it).
pub fn socket(item: [f32; 2], empty: bool) -> Vec<Fill> {
    let [x, y, _, _] = slot_rect(item);
    let mut out = slot(item);
    out.push(fill(x + 2.0, y + 2.0, 3.0, 1.0, colour::MARK));
    out.push(fill(x + 2.0, y + 2.0, 1.0, 3.0, colour::MARK));
    if empty {
        out.push(fill(x + 6.0, y + 8.0, 6.0, 1.0, colour::WELL_HI));
        out.push(fill(x + 8.0, y + 6.0, 1.0, 6.0, colour::WELL_HI));
    }
    out
}

/// A 1-pixel outline just inside `r`: hover and focus.
pub fn outline(r: Rect, argb: u32) -> Vec<Fill> {
    let [x, y, w, h] = r;
    vec![
        fill(x, y, w, 1.0, argb),
        fill(x, y + h - 1.0, w, 1.0, argb),
        fill(x, y + 1.0, 1.0, h - 2.0, argb),
        fill(x + w - 1.0, y + 1.0, 1.0, h - 2.0, argb),
    ]
}

/// The width of a fill of `share` inside a well `inner` pixels wide, whole pixels.
pub fn filled(inner: f32, share: f64) -> f32 {
    (inner * share.clamp(0.0, 1.0) as f32).round()
}

/// A bar (section 3, «Полосы»): a well, the fill one pixel in with a light line on its top, and on
/// the energy bar black ticks at 25, 50 and 75%.
pub fn bar(r: Rect, share: f64, argb: u32, ticks: bool) -> Vec<Fill> {
    let [x, y, w, h] = r;
    let mut out = well(r);
    let (iw, ih) = (w - 2.0, h - 2.0);
    let fw = filled(iw, share);
    if fw > 0.0 {
        out.push(fill(x + 1.0, y + 1.0, fw, ih, argb));
        out.push(fill(x + 1.0, y + 1.0, fw, 1.0, colour::SHEEN));
    }
    if ticks {
        for q in 1..4 {
            let tx = x + 1.0 + (iw * q as f32 / 4.0).round() - 1.0;
            out.push(fill(tx, y + 1.0, 1.0, ih, colour::TICK));
        }
    }
    out
}

/// The heights of the arrow head's six columns, from the shaft out.
pub const ARROW_COLUMNS: [f32; 6] = [12.0, 10.0, 8.0, 6.0, 4.0, 2.0];

/// The progress arrow (section 3, «Ход работы»): the shaft fills left to right, the head right of
/// it, centred on the shaft, lit when the work has started, else the well's light colour.
pub fn arrow(shaft: Rect, progress: f64) -> Vec<Fill> {
    let mut out = bar(shaft, progress, colour::PROGRESS, false);
    let head = if progress > 0.0 {
        colour::PROGRESS
    } else {
        colour::WELL_HI
    };
    let middle = shaft[1] + shaft[3] / 2.0;
    for (i, h) in ARROW_COLUMNS.iter().enumerate() {
        out.push(fill(
            shaft[0] + shaft[2] + i as f32,
            middle - h / 2.0,
            1.0,
            *h,
            head,
        ));
    }
    out
}

/// The right end of an arrow drawn from `shaft`, head included.
pub fn arrow_right(shaft: Rect) -> f32 {
    shaft[0] + shaft[2] + ARROW_HEAD[0]
}

/// A tank (section 3, «Баки»): a well, the fluid from the bottom for its share with a light top
/// line, ticks on the right 4 pixels at every quarter.
pub fn tank(r: Rect, share: f64, argb: u32) -> Vec<Fill> {
    let [x, y, w, h] = r;
    let mut out = well(r);
    let (iw, ih) = (w - 2.0, h - 2.0);
    let fh = filled(ih, share);
    if fh > 0.0 {
        let top = y + 1.0 + ih - fh;
        out.push(fill(x + 1.0, top, iw, fh, argb));
        out.push(fill(x + 1.0, top, iw, 1.0, colour::FLUID_TOP));
    }
    for q in 1..4 {
        let ty = y + 1.0 + (ih * q as f32 / 4.0).round() - 1.0;
        out.push(fill(x + w - 5.0, ty, 4.0, 1.0, colour::TANK_TICK));
    }
    out
}

/// The title's rule under it: one pixel of shade, one of light, across the face.
pub fn title_rule(width: f32, y: f32) -> Vec<Fill> {
    let inner = width - 2.0 * PADDING;
    vec![
        fill(PADDING, y, inner, 1.0, colour::BEVEL_LO),
        fill(PADDING, y + 1.0, inner, 1.0, colour::BEVEL_HI),
    ]
}

/// The rule under a key and value line.
pub fn kv_rule(x: f32, right: f32, y: f32) -> Fill {
    fill(x, y, right - x, 1.0, colour::KV_RULE)
}

/// A raised brass face (a chosen segment, a switched-on toggle, an open tab's number): brass,
/// light top and left, dark bottom and right.
pub fn raised(r: Rect) -> Vec<Fill> {
    let [x, y, w, h] = r;
    vec![
        fill(x, y, w, h, colour::ON_SHADE),
        fill(x, y, w - 1.0, h - 1.0, colour::ON_LIGHT),
        fill(x + 1.0, y + 1.0, w - 2.0, h - 2.0, colour::ON),
    ]
}

/// A button that is not on (`.w-btn`): its face with the bevel's light and shade.
pub fn button(r: Rect) -> Vec<Fill> {
    let [x, y, w, h] = r;
    vec![
        fill(x, y, w, h, colour::BEVEL_LO),
        fill(x, y, w - 1.0, h - 1.0, colour::BEVEL_HI),
        fill(x + 1.0, y + 1.0, w - 2.0, h - 2.0, colour::BUTTON),
    ]
}

/// The mode switch (section 3, «Переключатель режима»): one well around `segments` (each the
/// rectangle of one segment, left to right), the chosen one raised in brass. `well_of` is the
/// well around them all: one pixel out.
pub fn segments(segments: &[Rect], chosen: Option<usize>) -> Vec<Fill> {
    let Some(first) = segments.first() else {
        return Vec::new();
    };
    let last = segments[segments.len() - 1];
    let well_rect = [
        first[0] - 1.0,
        first[1] - 1.0,
        last[0] + last[2] - first[0] + 2.0,
        first[3] + 2.0,
    ];
    let mut out = well(well_rect);
    if let Some(r) = chosen.and_then(|i| segments.get(i)) {
        out.extend(raised(*r));
    }
    out
}

/// Three segments in one well over `r` (its outer rectangle): the well takes one pixel each side,
/// the segments one pixel apart, the widths shared out evenly.
pub fn segment_rects(r: Rect, n: usize) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    let inner = r[2] - 2.0 - (n as f32 - 1.0);
    let each = (inner / n as f32).floor();
    let spare = inner - each * n as f32;
    let mut x = r[0] + 1.0;
    (0..n)
        .map(|i| {
            let w = each + if (i as f32) < spare { 1.0 } else { 0.0 };
            let seg = [x, r[1] + 1.0, w, r[3] - 2.0];
            x += w + 1.0;
            seg
        })
        .collect()
}

/// How bad a status is; colour, glyph and stripe go together (colour is never the only sign).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Ok,
    Warn,
    Bad,
}

impl Severity {
    pub fn colour(self) -> u32 {
        match self {
            Severity::Ok => colour::OK,
            Severity::Warn => colour::WARN,
            Severity::Bad => colour::BAD,
        }
    }

    pub fn back(self) -> u32 {
        match self {
            Severity::Ok => colour::OK_BACK,
            Severity::Warn => colour::WARN_BACK,
            Severity::Bad => colour::BAD_BACK,
        }
    }

    pub fn glyph(self) -> Glyph {
        match self {
            Severity::Ok => Glyph::Check,
            Severity::Warn => Glyph::Bang,
            Severity::Bad => Glyph::Cross,
        }
    }
}

/// The 7 × 7 pixel signs of section 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Check,
    Bang,
    Cross,
    /// The turret's attack role: a sight.
    Sight,
    /// The turret's support role: a cross of healing.
    Plus,
    /// The search field's magnifier.
    Lens,
}

impl Glyph {
    /// The rows, `#` a pixel drawn.
    pub fn rows(self) -> [&'static str; 7] {
        match self {
            Glyph::Check => [
                ".......", "......#", ".....##", "#...##.", "##.##..", ".###...", "..#....",
            ],
            Glyph::Bang => [
                "...#...", "..###..", "..###..", "..###..", "...#...", ".......", "...#...",
            ],
            Glyph::Cross => [
                "##...##", "###.###", ".#####.", "..###..", ".#####.", "###.###", "##...##",
            ],
            Glyph::Sight => [
                "...#...", ".#####.", ".#...#.", "###.###", ".#...#.", ".#####.", "...#...",
            ],
            Glyph::Plus => [
                "..###..", "..###..", "#######", "#######", "#######", "..###..", "..###..",
            ],
            Glyph::Lens => [
                ".###...", "#...#..", "#...#..", "#...#..", ".####..", ".....#.", "......#",
            ],
        }
    }

    /// The glyph's pixels at `at` (its top left), one fill per run of drawn pixels in a row.
    pub fn fills(self, at: [f32; 2], argb: u32) -> Vec<Fill> {
        let mut out = Vec::new();
        for (row, line) in self.rows().iter().enumerate() {
            let bytes = line.as_bytes();
            let mut col = 0;
            while col < bytes.len() {
                if bytes[col] != b'#' {
                    col += 1;
                    continue;
                }
                let start = col;
                while col < bytes.len() && bytes[col] == b'#' {
                    col += 1;
                }
                out.push(fill(
                    at[0] + start as f32,
                    at[1] + row as f32,
                    (col - start) as f32,
                    1.0,
                    argb,
                ));
            }
        }
        out
    }
}

/// The status line (section 3, «Строка состояния»): its background and stripe by severity and
/// its glyph; the text goes at [`status_text_x`].
pub fn status(r: Rect, severity: Severity) -> Vec<Fill> {
    let [x, y, w, h] = r;
    let mut out = vec![
        fill(x, y, w, h, severity.back()),
        fill(x, y, STRIPE, h, severity.colour()),
    ];
    let gy = y + ((h - GLYPH) / 2.0).floor();
    out.extend(
        severity
            .glyph()
            .fills([x + STRIPE + STATUS_PAD, gy], severity.colour()),
    );
    out
}

/// Where a status line's text starts.
pub fn status_text_x(r: Rect) -> f32 {
    r[0] + STRIPE + STATUS_PAD + GLYPH + STATUS_PAD
}

/// The turret's role plate: its background, stripe and glyph, as a status line.
pub fn role_plate(r: Rect, support: bool) -> Vec<Fill> {
    let [x, y, w, h] = r;
    let (back, stripe, glyph) = if support {
        (colour::SUPPORT_BACK, colour::SUPPORT_STRIPE, Glyph::Plus)
    } else {
        (colour::ATTACK_BACK, colour::ATTACK_STRIPE, Glyph::Sight)
    };
    let mut out = vec![fill(x, y, w, h, back), fill(x, y, STRIPE, h, stripe)];
    let gy = y + ((h - GLYPH) / 2.0).floor();
    out.extend(glyph.fills([x + STRIPE + STATUS_PAD, gy], colour::VALUE | 0xFF00_0000));
    out
}

/// Tabs over a brass line (section 3, «Вкладки»): each tab a well, the open one the face in a brass
/// frame with a brass line on top whose bottom merges with the face. `line_y` is the brass line
/// under the row; `right` its end.
pub fn tabs(tabs: &[Rect], open: Option<usize>, line_y: f32, right: f32) -> Vec<Fill> {
    let mut out = Vec::new();
    if let Some(first) = tabs.first() {
        out.push(fill(first[0], line_y, right - first[0], 1.0, colour::FRAME));
    }
    for (i, r) in tabs.iter().enumerate() {
        let [x, y, w, h] = *r;
        if Some(i) == open {
            out.push(fill(x, y, w, h + 1.0, colour::FRAME));
            out.push(fill(x + 1.0, y + 1.0, w - 2.0, h, colour::FACE));
            out.push(fill(x + 1.0, y + 1.0, w - 2.0, 1.0, colour::MARK));
        } else {
            out.push(fill(x, y, w, h, colour::WELL));
        }
    }
    out
}

/// A text field (section 3, «Поля»): a well, the magnifier for a search field, and the caret at
/// `caret_x` when it has the focus.
pub fn field(r: Rect, search: bool, caret_x: Option<f32>) -> Vec<Fill> {
    let mut out = well(r);
    if search {
        let gy = r[1] + ((r[3] - GLYPH) / 2.0).floor();
        out.extend(Glyph::Lens.fills([r[0] + 4.0, gy], 0xFF00_0000 | colour::LABEL));
    }
    if let Some(x) = caret_x {
        out.push(fill(x, r[1] + 3.0, 1.0, r[3] - 6.0, colour::HOT));
    }
    out
}

/// Where a field's text starts: after the magnifier for a search field.
pub fn field_text_x(r: Rect, search: bool) -> f32 {
    r[0] + 4.0 + if search { GLYPH + 3.0 } else { 0.0 }
}

/// A scheme's parchment inset: a well's edges round the paper.
pub fn paper(r: Rect) -> Vec<Fill> {
    let [x, y, w, h] = r;
    vec![
        fill(x, y, w, h, colour::WELL_HI),
        fill(x, y, w - 1.0, h - 1.0, colour::WELL_LO),
        fill(x + 1.0, y + 1.0, w - 2.0, h - 2.0, colour::PAPER),
    ]
}

/// The relative luminance of a colour (its low 24 bits), as WCAG counts it.
pub fn luminance(rgb: u32) -> f64 {
    let channel = |shift: u32| {
        let c = f64::from((rgb >> shift) & 0xFF) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

/// The contrast ratio between two colours, 1 to 21.
pub fn contrast(a: u32, b: u32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn covers(fills: &[Fill], at: [f32; 2]) -> Option<u32> {
        fills
            .iter()
            .rev()
            .find(|f| {
                let [x, y, w, h] = f.rect;
                at[0] >= x && at[0] < x + w && at[1] >= y && at[1] < y + h
            })
            .map(|f| f.argb)
    }

    #[test]
    fn the_frame_goes_outline_brass_bevel_face() {
        let f = frame([100.0, 50.0]);
        assert_eq!(covers(&f, [0.0, 0.0]), Some(colour::OUTLINE));
        assert_eq!(covers(&f, [1.0, 20.0]), Some(colour::FRAME));
        assert_eq!(covers(&f, [98.0, 20.0]), Some(colour::FRAME));
        assert_eq!(covers(&f, [2.0, 20.0]), Some(colour::BEVEL_HI));
        assert_eq!(covers(&f, [50.0, 2.0]), Some(colour::BEVEL_HI));
        assert_eq!(covers(&f, [97.0, 20.0]), Some(colour::BEVEL_LO));
        assert_eq!(covers(&f, [50.0, 47.0]), Some(colour::BEVEL_LO));
        assert_eq!(covers(&f, [3.0, 3.0]), Some(colour::FACE));
        assert_eq!(covers(&f, [96.0, 46.0]), Some(colour::FACE));
    }

    #[test]
    fn a_well_is_dark_top_left_and_light_bottom_right() {
        let w = slot([10.0, 10.0]);
        assert_eq!(covers(&w, [9.0, 9.0]), Some(colour::WELL_LO));
        assert_eq!(covers(&w, [9.0, 20.0]), Some(colour::WELL_LO));
        assert_eq!(covers(&w, [26.0, 20.0]), Some(colour::WELL_HI));
        assert_eq!(covers(&w, [20.0, 26.0]), Some(colour::WELL_HI));
        assert_eq!(covers(&w, [10.0, 10.0]), Some(colour::WELL));
        assert_eq!(covers(&w, [25.0, 25.0]), Some(colour::WELL));
        assert_eq!(slot_rect([10.0, 10.0]), [9.0, 9.0, SLOT, SLOT]);
    }

    #[test]
    fn a_socket_has_its_corner_and_an_empty_one_its_plus() {
        let s = socket([1.0, 1.0], true);
        // The frame is at 0, the corner two pixels in: 2..5 across and down.
        assert_eq!(covers(&s, [2.0, 2.0]), Some(colour::MARK));
        assert_eq!(covers(&s, [4.0, 2.0]), Some(colour::MARK));
        assert_eq!(covers(&s, [2.0, 4.0]), Some(colour::MARK));
        assert_eq!(covers(&s, [4.0, 4.0]), Some(colour::WELL));
        assert_eq!(covers(&s, [8.0, 8.0]), Some(colour::WELL_HI));
        assert_eq!(covers(&s, [6.0, 8.0]), Some(colour::WELL_HI));
        assert_eq!(covers(&s, [8.0, 11.0]), Some(colour::WELL_HI));
        let full = socket([1.0, 1.0], false);
        assert_eq!(covers(&full, [8.0, 8.0]), Some(colour::WELL));
    }

    #[test]
    fn a_bar_fills_its_share_with_a_sheen_and_energy_ticks() {
        let b = bar([0.0, 0.0, 66.0, 8.0], 0.5, colour::ENERGY, true);
        assert_eq!(covers(&b, [1.0, 1.0]), Some(colour::SHEEN));
        assert_eq!(covers(&b, [1.0, 3.0]), Some(colour::ENERGY));
        assert_eq!(covers(&b, [32.0, 3.0]), Some(colour::TICK));
        assert_eq!(covers(&b, [40.0, 3.0]), Some(colour::WELL));
        assert_eq!(filled(64.0, 0.5), 32.0);
        assert_eq!(filled(64.0, 2.0), 64.0);
        assert_eq!(filled(64.0, -1.0), 0.0);
        let empty = bar([0.0, 0.0, 66.0, 8.0], 0.0, colour::ENERGY, false);
        assert_eq!(covers(&empty, [1.0, 3.0]), Some(colour::WELL));
    }

    #[test]
    fn the_arrow_head_lights_with_the_work() {
        let shaft = [0.0, 3.0, 22.0, 6.0];
        let idle = arrow(shaft, 0.0);
        let going = arrow(shaft, 0.3);
        // The head's first column, 12 high, centred on the shaft's middle (y 6).
        assert_eq!(covers(&idle, [22.0, 0.0]), Some(colour::WELL_HI));
        assert_eq!(covers(&going, [22.0, 0.0]), Some(colour::PROGRESS));
        assert_eq!(covers(&going, [22.0, 11.0]), Some(colour::PROGRESS));
        assert_eq!(covers(&going, [27.0, 5.0]), Some(colour::PROGRESS));
        assert_eq!(covers(&going, [27.0, 4.0]), None);
        assert_eq!(arrow_right(shaft), 28.0);
    }

    #[test]
    fn a_tank_fills_from_the_bottom() {
        let t = tank([0.0, 0.0, 16.0, 36.0], 0.5, 0xFF00_00FF);
        assert_eq!(covers(&t, [5.0, 33.0]), Some(0xFF00_00FF));
        assert_eq!(covers(&t, [5.0, 18.0]), Some(colour::FLUID_TOP));
        assert_eq!(covers(&t, [5.0, 10.0]), Some(colour::WELL));
        assert_eq!(covers(&t, [12.0, 9.0]), Some(colour::TANK_TICK));
    }

    #[test]
    fn segments_share_the_well_and_the_chosen_is_brass() {
        let r = segment_rects([0.0, 0.0, 200.0, 14.0], 3);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0][0], 1.0);
        for pair in r.windows(2) {
            assert_eq!(pair[0][0] + pair[0][2] + 1.0, pair[1][0]);
        }
        let last = r[2];
        assert_eq!(last[0] + last[2], 199.0);
        let f = segments(&r, Some(1));
        let mid = [r[1][0] + 5.0, 7.0];
        assert_eq!(covers(&f, mid), Some(colour::ON));
        assert_eq!(covers(&f, [r[0][0] + 5.0, 7.0]), Some(colour::WELL));
    }

    #[test]
    fn the_glyphs_are_seven_by_seven() {
        for g in [
            Glyph::Check,
            Glyph::Bang,
            Glyph::Cross,
            Glyph::Sight,
            Glyph::Plus,
            Glyph::Lens,
        ] {
            for row in g.rows() {
                assert_eq!(row.len(), 7, "{g:?}");
            }
            let pixels: usize = g.rows().iter().map(|r| r.matches('#').count()).sum();
            let drawn: f32 = g.fills([0.0, 0.0], 0).iter().map(|f| f.rect[2]).sum();
            assert_eq!(drawn as usize, pixels, "{g:?}");
        }
    }

    #[test]
    fn the_status_line_has_its_stripe_glyph_and_background() {
        let r = [0.0, 0.0, 100.0, 13.0];
        let f = status(r, Severity::Bad);
        assert_eq!(covers(&f, [0.0, 5.0]), Some(colour::BAD));
        assert_eq!(covers(&f, [50.0, 5.0]), Some(colour::BAD_BACK));
        // The cross's top left pixel, at the glyph's corner.
        assert_eq!(covers(&f, [6.0, 3.0]), Some(colour::BAD));
        assert_eq!(status_text_x(r), 17.0);
    }

    #[test]
    fn the_open_tab_merges_with_the_face() {
        let t = tabs(
            &[[0.0, 0.0, 20.0, 12.0], [21.0, 0.0, 20.0, 12.0]],
            Some(0),
            12.0,
            100.0,
        );
        assert_eq!(covers(&t, [10.0, 12.0]), Some(colour::FACE));
        assert_eq!(covers(&t, [10.0, 1.0]), Some(colour::MARK));
        assert_eq!(covers(&t, [30.0, 12.0]), Some(colour::FRAME));
        assert_eq!(covers(&t, [30.0, 5.0]), Some(colour::WELL));
    }

    #[test]
    fn the_text_reads_on_its_ground() {
        // The document's figures: about 14:1, 5.7:1 and 8:1; WCAG asks 4.5:1 for text.
        assert!(contrast(colour::VALUE, colour::FACE) > 13.0);
        assert!(contrast(colour::LABEL, colour::FACE) > 5.0);
        assert!(contrast(colour::ON_TEXT, colour::ON) > 7.5);
        for back in [colour::OK_BACK, colour::WARN_BACK, colour::BAD_BACK] {
            assert!(contrast(colour::VALUE, back) > 4.5);
        }
        assert!(contrast(colour::PAPER_TEXT, colour::PAPER) > 4.5);
        assert!(contrast(colour::TEXT, colour::BUTTON) > 4.5);
        assert!(contrast(colour::BAD_TEXT, colour::FACE) > 3.0);
    }
}
