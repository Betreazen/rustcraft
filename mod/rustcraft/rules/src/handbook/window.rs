//! A block's window as a picture (guide «Окно блока»: ui.js `RC.UI`), drawn on its page above
//! the numbered legend of its parts.
//!
//! `windows` (written by assets/models/windows_mc.py) holds each window's parts as widgets, the
//! ui.js builder that made each: slots, the work arrow, a bar, the mode switch, module sockets,
//! a tank, the status line, rows of key and value, chips, a craft, a map, a text. The words a
//! widget shows are lang keys (`Page::key("part.<i>.w.<k>")`). The picture stacks the parts
//! down the page, each with its number and label over its widget, on a dark panel under the
//! window's title; this module says how tall each part is and where its pieces go.

use super::{INDENT, LINE, Rect, wrap};

pub use super::windows::WINDOWS;

/// A slot of the mock-up: its item's colour (RGB, 0 for an empty slot), the count it shows and
/// whether it is pale (an item that is not there yet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub rgb: u32,
    pub count: &'static str,
    pub ghost: bool,
}

/// One part of a window, as ui.js builds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widget {
    /// Slots, `cols` to a row.
    Slots { cols: u8, cells: &'static [Cell] },
    /// The work arrow, `pct` filled.
    Work { pct: u8 },
    /// A bar `pct` full of `rgb` (energy's yellow for an energy bar); its line is word 0.
    Bar { pct: u8, rgb: u32 },
    /// The mode switch: words 0–2, `active` lit.
    Mode { active: u8 },
    /// Module sockets: each its module's colour, 0 for an empty one.
    Sockets { filled: &'static [u32] },
    /// A tank `pct` full of `rgb`; its line is word 0.
    Tank { pct: u8, rgb: u32 },
    /// The status line: 0 works, 1 works worse, 2 stands; its text is word 0.
    Status { tone: u8 },
    /// `n` rows of key (word 2k) and value (word 2k + 1).
    Rows { n: u8 },
    /// Chips (words), each lit or not; with `note`, the last word is a line under them.
    Chips { on: &'static [bool], note: bool },
    /// What goes in, an arrow, what comes out.
    Craft {
        from: &'static [Cell],
        to: &'static [Cell],
    },
    /// A map's names (words), listed three to a row.
    Map { n: u8 },
    /// A line of text (word 0).
    Text,
}

/// The window of page `id`, if its guide entry has one.
pub fn window_of(id: &str) -> Option<&'static [Widget]> {
    WINDOWS.iter().find(|(p, _)| *p == id).map(|(_, w)| *w)
}

pub const SLOT: f32 = 18.0;
/// The title strip over the picture.
pub const TITLE_HEIGHT: f32 = 13.0;
/// A part keeps this much space under it.
pub const PART_GAP: f32 = 4.0;
/// A bar's length and thickness, a chip's height and the gaps between chips.
pub const BAR: [f32; 2] = [120.0, 6.0];
pub const CHIP_HEIGHT: f32 = 11.0;
pub const CHIP_GAP: f32 = 3.0;
pub const CHIP_PAD: f32 = 3.0;
pub const MODE_HEIGHT: f32 = 12.0;
pub const STATUS_HEIGHT: f32 = 12.0;
/// The craft's arrow, between what goes in and what comes out.
pub const CRAFT_ARROW: f32 = 22.0;

/// How many slots a row of the picture holds in `width`.
pub fn slot_cols(cols: u8, width: f32) -> usize {
    let room = ((width / SLOT).floor() as usize).max(1);
    usize::from(cols.max(1)).min(room)
}

/// Where the chips go (from the widget's corner) in `width`: each as wide as its word, wrapped
/// to the next line when the line is full.
pub fn chip_rects(
    words: &[String],
    n: usize,
    width: f32,
    measure: &dyn Fn(&str) -> f32,
) -> Vec<Rect> {
    let mut out = Vec::new();
    let (mut x, mut y) = (0.0, 0.0);
    for w in words.iter().take(n) {
        let cw = (measure(w) + 2.0 * CHIP_PAD).min(width);
        if x > 0.0 && x + cw > width {
            x = 0.0;
            y += CHIP_HEIGHT + CHIP_GAP;
        }
        out.push([x, y, cw, CHIP_HEIGHT]);
        x += cw + CHIP_GAP;
    }
    out
}

/// The height of a widget in `width`, its words `words`.
pub fn widget_height(
    w: &Widget,
    words: &[String],
    width: f32,
    measure: &dyn Fn(&str) -> f32,
) -> f32 {
    match *w {
        Widget::Slots { cols, cells } => {
            let per = slot_cols(cols, width);
            SLOT * cells.len().div_ceil(per).max(1) as f32
        }
        Widget::Work { .. } => 8.0,
        Widget::Bar { .. } | Widget::Tank { .. } => LINE,
        Widget::Mode { .. } => MODE_HEIGHT,
        Widget::Sockets { .. } | Widget::Craft { .. } => SLOT,
        Widget::Status { .. } => STATUS_HEIGHT,
        Widget::Rows { n } => LINE * f32::from(n),
        Widget::Chips { on, note } => {
            let chips = chip_rects(words, on.len(), width, measure);
            let h = chips.last().map_or(0.0, |r| r[1] + r[3]);
            let note_lines = if note {
                words.last().map_or(0, |t| wrap(t, width, measure).len())
            } else {
                0
            };
            h + LINE * note_lines as f32 + if note { 2.0 } else { 0.0 }
        }
        Widget::Map { n } => LINE * usize::from(n).div_ceil(3).max(1) as f32 + 4.0,
        Widget::Text => {
            LINE * words
                .first()
                .map_or(1, |t| wrap(t, width, measure).len().max(1)) as f32
        }
    }
}

/// A part's whole height on the page: its label's line (when it has one), its widget, a gap.
pub fn part_height(
    w: &Widget,
    label: bool,
    words: &[String],
    width: f32,
    measure: &dyn Fn(&str) -> f32,
) -> f32 {
    let head = if label { LINE } else { 0.0 };
    head + widget_height(w, words, width - INDENT, measure) + PART_GAP
}

/// Where a part's widget starts, from the part's top left.
pub fn widget_at(label: bool) -> [f32; 2] {
    [INDENT, if label { LINE } else { 0.0 } + 1.0]
}

/// The left of what comes out of a craft of `from` slots.
pub fn craft_out_x(from: usize) -> f32 {
    SLOT * from as f32 + 4.0 + CRAFT_ARROW + 4.0
}

#[cfg(test)]
mod tests {
    use super::super::{BODY_WIDTH, PAGES, Piece, Row, lay_out, pieces};
    use super::*;

    fn measure(s: &str) -> f32 {
        6.0 * s.chars().count() as f32
    }

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn chips_wrap_to_the_next_line() {
        let w = words(&["aaaa", "bbbb", "cccc"]);
        // 4 letters: 24 + 6 = 30 wide; two fit in 70 with the gap, the third wraps.
        let r = chip_rects(&w, 3, 70.0, &measure);
        assert_eq!(r[0], [0.0, 0.0, 30.0, CHIP_HEIGHT]);
        assert_eq!(r[1], [33.0, 0.0, 30.0, CHIP_HEIGHT]);
        assert_eq!(r[2], [0.0, CHIP_HEIGHT + CHIP_GAP, 30.0, CHIP_HEIGHT]);
        let h = widget_height(
            &Widget::Chips {
                on: &[true, false, false],
                note: false,
            },
            &w,
            70.0,
            &measure,
        );
        assert_eq!(h, 2.0 * CHIP_HEIGHT + CHIP_GAP);
    }

    #[test]
    fn slots_fold_into_the_width() {
        assert_eq!(slot_cols(9, 300.0), 9);
        assert_eq!(slot_cols(20, 100.0), 5);
        const CELLS: [Cell; 7] = [Cell {
            rgb: 0,
            count: "",
            ghost: false,
        }; 7];
        let h = widget_height(
            &Widget::Slots {
                cols: 3,
                cells: &CELLS,
            },
            &[],
            300.0,
            &measure,
        );
        assert_eq!(h, 3.0 * SLOT);
        assert_eq!(
            part_height(&Widget::Work { pct: 5 }, true, &[], 300.0, &measure),
            LINE + 8.0 + PART_GAP
        );
        assert_eq!(widget_at(false), [INDENT, 1.0]);
    }

    #[test]
    fn every_window_has_a_widget_for_each_numbered_part() {
        for (id, widgets) in WINDOWS {
            let page = PAGES
                .iter()
                .find(|p| p.id == *id)
                .unwrap_or_else(|| panic!("{id}"));
            assert_eq!(widgets.len(), page.parts, "{id}");
        }
        assert!(window_of("furnace").is_some() && window_of("overview").is_none());
    }

    #[test]
    fn the_furnace_page_draws_its_window_before_the_legend() {
        let page = PAGES.iter().find(|p| p.id == "furnace").unwrap();
        let lines = |k: &str| -> Option<String> {
            let rest = k.strip_prefix(&page.key(""))?;
            Some(match rest {
                "window" => "Электропечь".into(),
                "part.4.w.1" => "Обычный".into(),
                r if r.starts_with("part.") => "x".into(),
                _ => return None,
            })
        };
        let got = pieces(page, &[], &lines);
        let at = got
            .iter()
            .position(|p| matches!(p, Piece::Window { .. }))
            .expect("a window");
        assert!(matches!(got[at + 1], Piece::Part { n: 1, .. }));
        let Piece::Window { title, parts } = &got[at] else {
            unreachable!()
        };
        assert_eq!(title, "Электропечь");
        assert_eq!(parts.len(), page.parts);
        assert_eq!(parts[4].words[1], "Обычный");
        let rows = lay_out(&got, BODY_WIDTH, &measure);
        let n = rows
            .iter()
            .filter(|r| matches!(r, Row::Widget { .. }))
            .count();
        assert_eq!(n, page.parts);
        // Without the window's title in the reader's language there is no picture.
        let none = pieces(page, &[], &|k| {
            (k != page.key("window")).then(|| "x".to_owned())
        });
        assert!(!none.iter().any(|p| matches!(p, Piece::Window { .. })));
    }
}
