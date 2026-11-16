//! The engineer's handbook (spec H2, H5, H6; guide `guide`): an item the player gets the first
//! time they carry an alloy, and its window, a page for every block and item of the guide.
//!
//! - `pages` (written by assets/models/handbook_mc.py from docs/showcase/js/data.js and ui.js)
//!   lists the sections and the pages; their texts are lang keys (`key`), so every client reads
//!   them in its own language, and English, being shorter, has fewer of them.
//! - The grant (H2): `due` says whether a player gets the handbook now, `Given` remembers who has
//!   had it, as the world's file keeps it.
//! - The window: the contents (the sections on the left, the chosen section's pages on the
//!   right) and a page. A page's texts become `Piece`s (`pieces`), are wrapped by the client's own
//!   text width into `Row`s (`lay_out`) and cut into sheets that fit the window (`sheets`);
//!   `part_at` and `press` turn a click into the next `View`.
//! - `recipe_grid`: a crafting recipe's cells as the 3 × 3 grid a page draws; `recipes`: every
//!   recipe a page shows (the crafting table's, the transmuting ones, the machines'), two to a row.
//! - `scene` (H3, H4): a page whose guide entry has an animated scene has one sheet more, its
//!   last, which plays the scene (`scenes`, written by assets/models/scenes_mc.py from the
//!   guide's scenes.js): `scene_sheet` says which sheet that is.

pub mod pages;
pub mod recipes;
pub mod scene;
pub mod scenes;

use crate::machines::panel::{Rect, inside};
use std::collections::BTreeSet;
use std::ops::Range;

pub use pages::{PAGES, SECTIONS};

/// The handbook's item.
pub const ITEM: &str = "rustcraft:handbook";
/// The alloys, one a tier (guide `alloys`): carrying any of them is having made an alloy.
pub const ALLOYS: [&str; 4] = [
    "rustcraft:coppersteel",
    "rustcraft:sparksteel",
    "rustcraft:crystallite",
    "rustcraft:starsteel",
];
/// The mod message that opens the window on the player's client.
pub const CHANNEL: &str = "rustcraft:handbook";
/// Every lang key of the handbook starts so (handbook_mc.py owns them).
pub const PREFIX: &str = "rustcraft.handbook.";

/// One page of the guide, as handbook_mc.py writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    /// The guide's id of the entry: its lang keys are `rustcraft.handbook.page.<id>.*`.
    pub id: &'static str,
    /// Its section, an index into SECTIONS.
    pub section: usize,
    /// Its tier, 0 for none.
    pub tier: u8,
    /// The item whose icon and crafting recipe the page shows.
    pub item: Option<&'static str>,
    /// How many paragraphs (`.text.<n>`), stats (`.stat.<n>.key`, `.value`), diagnoses
    /// (`.diag.<n>`, `.fix`) and window parts (`.part.<n>`, `.label`) its Russian lang has.
    pub text: usize,
    pub stats: usize,
    pub diagnoses: usize,
    pub parts: usize,
    /// What the guide entry's recipes make (game ids; `recipes::ANY_FLUID` for every fluid):
    /// the page shows the game's recipes of these, every tier of each (`recipes::for_page`).
    pub makes: &'static [&'static str],
    /// The guide entry's tables, each its columns and rows (`.table.<t>.title`, `.note`,
    /// `.head.<c>`, `.row.<r>.<c>`).
    pub tables: &'static [[usize; 2]],
    /// The pages of its «см. также», by their ids.
    pub see: &'static [&'static str],
}

impl Page {
    /// The page's lang key `rest` (`name`, `sum`, `text.0`…).
    pub fn key(&self, rest: &str) -> String {
        format!("{PREFIX}page.{}.{rest}", self.id)
    }

    /// The item whose icon the page and the contents show: its own, else the handbook.
    pub fn icon(&self) -> &'static str {
        self.item.unwrap_or(ITEM)
    }
}

/// A window word's lang key: `title`, `contents`, `recipe`….
pub fn key(rest: &str) -> String {
    format!("{PREFIX}{rest}")
}

/// A section's name's lang key.
pub fn section_key(section: usize) -> String {
    key(&format!(
        "section.{}",
        SECTIONS.get(section).copied().unwrap_or("start")
    ))
}

/// The pages of a section, as indices into PAGES, in the guide's order.
pub fn pages_of(section: usize) -> Vec<usize> {
    (0..PAGES.len())
        .filter(|&i| PAGES[i].section == section)
        .collect()
}

// ---- The grant (H2) ----

pub fn is_alloy(item: &str) -> bool {
    ALLOYS.contains(&item)
}

/// Whether a player who has not had the handbook yet and carries `carried` gets it now.
pub fn due<'a>(had: bool, carried: impl IntoIterator<Item = &'a str>) -> bool {
    !had && carried.into_iter().any(is_alloy)
}

/// The players who have had the handbook, as the world's file keeps them (UUIDs in hex).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Given {
    #[serde(default)]
    pub players: BTreeSet<String>,
}

impl Given {
    fn name(player: u128) -> String {
        format!("{player:032x}")
    }

    pub fn has(&self, player: u128) -> bool {
        self.players.contains(&Self::name(player))
    }

    /// Notes that `player` had it; false when they already had.
    pub fn add(&mut self, player: u128) -> bool {
        self.players.insert(Self::name(player))
    }

    pub fn remove(&mut self, player: u128) -> bool {
        self.players.remove(&Self::name(player))
    }
}

// ---- The window ----

pub const WIDTH: f32 = 320.0;
pub const HEIGHT: f32 = 210.0;
pub const TITLE: [f32; 2] = [8.0, 6.0];
/// The bottom row: back a sheet, the contents, the sheet's number, on a sheet.
pub const PREV: Rect = [8.0, 191.0, 16.0, 12.0];
pub const CONTENTS: Rect = [28.0, 191.0, 84.0, 12.0];
pub const SHEET: Rect = [116.0, 191.0, 176.0, 12.0];
pub const NEXT: Rect = [296.0, 191.0, 16.0, 12.0];
/// The contents: the sections down the left, the chosen section's pages on the right.
pub const SECTION_LIST: [f32; 2] = [8.0, 20.0];
pub const SECTION_SIZE: [f32; 2] = [112.0, 12.0];
pub const ENTRY_LIST: [f32; 2] = [126.0, 20.0];
pub const ENTRY_SIZE: [f32; 2] = [186.0, 18.0];
/// Pages a sheet of the contents lists.
pub const ENTRIES: usize = 9;
/// A page's head: its icon's slot, its name and the line under it.
pub const ICON: [f32; 2] = [9.0, 22.0];
pub const NAME: [f32; 2] = [30.0, 22.0];
pub const UNDER_NAME: [f32; 2] = [30.0, 32.0];
/// A page's lines: from the left, as wide as the window's face.
pub const BODY_X: f32 = 8.0;
pub const BODY_WIDTH: f32 = WIDTH - 2.0 * BODY_X;
/// The first sheet's lines start under the head, the others under the title.
pub const FIRST_TOP: f32 = 44.0;
pub const REST_TOP: f32 = 20.0;
pub const BODY_BOTTOM: f32 = 187.0;
pub const LINE: f32 = 10.0;
/// What a key and value line keeps between them, and what a line under a heading is set in.
pub const PAIR_GAP: f32 = 8.0;
pub const INDENT: f32 = 12.0;
/// A part's number: a brass square at the line's left.
pub const NUMBER: [f32; 2] = [9.0, 9.0];
/// A sheet asked for by its end: the window takes the last one.
pub const LAST: usize = usize::MAX;

/// Where the window's corner goes on a screen of `size` GUI pixels: in the middle, whole pixels.
pub fn origin(size: [f32; 2]) -> [f32; 2] {
    [
        ((size[0] - WIDTH) / 2.0).floor().max(0.0),
        ((size[1] - HEIGHT) / 2.0).floor().max(0.0),
    ]
}

pub fn section_rect(i: usize) -> Rect {
    [
        SECTION_LIST[0],
        SECTION_LIST[1] + SECTION_SIZE[1] * i as f32,
        SECTION_SIZE[0],
        SECTION_SIZE[1],
    ]
}

pub fn entry_rect(row: usize) -> Rect {
    [
        ENTRY_LIST[0],
        ENTRY_LIST[1] + ENTRY_SIZE[1] * row as f32,
        ENTRY_SIZE[0],
        ENTRY_SIZE[1],
    ]
}

/// Where the icon of the contents' row `row` goes.
pub fn entry_icon(row: usize) -> [f32; 2] {
    let r = entry_rect(row);
    [r[0] + 1.0, r[1] + 1.0]
}

/// What the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// The contents with `section` chosen, its pages from `sheet` * ENTRIES on.
    Contents { section: usize, sheet: usize },
    /// The page `page` (an index into PAGES) at its sheet `sheet` (LAST for its last).
    Page { page: usize, sheet: usize },
}

impl Default for View {
    fn default() -> Self {
        View::Contents {
            section: 0,
            sheet: 0,
        }
    }
}

impl View {
    /// The view with its page and sheet inside what there is: `sheets` sheets of a page.
    pub fn clamped(self, sheets: usize) -> View {
        let last = sheets.max(1) - 1;
        match self {
            View::Contents { section, sheet } => {
                let section = section.min(SECTIONS.len().saturating_sub(1));
                View::Contents {
                    section,
                    sheet: sheet.min(contents_sheets(section) - 1),
                }
            }
            View::Page { page, sheet } => View::Page {
                page: page.min(PAGES.len().saturating_sub(1)),
                sheet: sheet.min(last),
            },
        }
    }
}

/// The page's scene, if its guide entry has one.
pub fn scene_of(page: &Page) -> Option<&'static scene::Scene> {
    scene::find(page.id)
}

/// The sheets of a page whose texts take `text_sheets`: those, and its scene's after them.
pub fn sheet_count(page: &Page, text_sheets: usize) -> usize {
    text_sheets.max(1) + usize::from(scene_of(page).is_some())
}

/// Which of a page's sheets is its scene's: the one after its texts' `text_sheets`.
pub fn scene_sheet(page: &Page, text_sheets: usize) -> Option<usize> {
    scene_of(page).map(|_| text_sheets.max(1))
}

/// Sheets of the contents with `section` chosen.
pub fn contents_sheets(section: usize) -> usize {
    pages_of(section).len().div_ceil(ENTRIES).max(1)
}

/// The pages listed on the contents' sheet, by row.
pub fn listed(section: usize, sheet: usize) -> Vec<usize> {
    pages_of(section)
        .into_iter()
        .skip(sheet * ENTRIES)
        .take(ENTRIES)
        .collect()
}

/// A part of the window a click can be on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Prev,
    Next,
    Contents,
    Section(usize),
    /// A row of the contents' pages.
    Entry(usize),
    /// A scene's step buttons, on its sheet (`scene::part_at`).
    StepBack,
    StepOn,
}

/// The part at `at` (window pixels) of the window showing `view`.
pub fn part_at(view: View, at: [f32; 2]) -> Option<Part> {
    let bottom = [
        (PREV, Part::Prev),
        (NEXT, Part::Next),
        (CONTENTS, Part::Contents),
    ];
    if let Some(p) = bottom.iter().find_map(|&(r, p)| inside(r, at).then_some(p)) {
        return Some(p);
    }
    let View::Contents { section, sheet } = view else {
        return None;
    };
    if let Some(i) = (0..SECTIONS.len()).find(|&i| inside(section_rect(i), at)) {
        return Some(Part::Section(i));
    }
    let rows = listed(section, sheet).len();
    (0..rows)
        .find(|&r| inside(entry_rect(r), at))
        .map(Part::Entry)
}

/// The view after a click on `part`; `sheets` is how many sheets the page shown has.
pub fn press(view: View, part: Part, sheets: usize) -> View {
    let view = view.clamped(sheets);
    match (view, part) {
        (_, Part::Prev) => turn(view, false, sheets),
        (_, Part::Next) => turn(view, true, sheets),
        (View::Page { page, .. }, Part::Contents) => {
            let section = PAGES[page].section;
            let place = pages_of(section)
                .iter()
                .position(|&p| p == page)
                .unwrap_or(0);
            View::Contents {
                section,
                sheet: place / ENTRIES,
            }
        }
        (View::Contents { .. }, Part::Section(section)) if section < SECTIONS.len() => {
            View::Contents { section, sheet: 0 }
        }
        (View::Contents { section, sheet }, Part::Entry(row)) => {
            match listed(section, sheet).get(row) {
                Some(&page) => View::Page { page, sheet: 0 },
                None => view,
            }
        }
        _ => view,
    }
}

/// A sheet on (`forward`) or back: past a page's last sheet to the next page, before its first to
/// the previous page's last; the contents page through the section's list.
pub fn turn(view: View, forward: bool, sheets: usize) -> View {
    match view.clamped(sheets) {
        View::Contents { section, sheet } => {
            let last = contents_sheets(section) - 1;
            let sheet = if forward {
                (sheet + 1).min(last)
            } else {
                sheet.saturating_sub(1)
            };
            View::Contents { section, sheet }
        }
        View::Page { page, sheet } => {
            let last = sheets.max(1) - 1;
            match (forward, sheet) {
                (true, s) if s < last => View::Page { page, sheet: s + 1 },
                (true, _) if page + 1 < PAGES.len() => View::Page {
                    page: page + 1,
                    sheet: 0,
                },
                (false, s) if s > 0 => View::Page { page, sheet: s - 1 },
                (false, _) if page > 0 => View::Page {
                    page: page - 1,
                    sheet: LAST,
                },
                _ => View::Page { page, sheet },
            }
        }
    }
}

// ---- A page's lines ----

/// What a page says, in the order the window shows it, its texts already in the reader's
/// language.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    /// The summary under the head.
    Lead(String),
    /// A part's heading: «Рецепт», «Диагнозы», «Окно блока».
    Heading(String),
    /// The recipes, by the height of each one's block (`recipes::Shown::height`).
    Recipes(Vec<f32>),
    Paragraph(String),
    Stat {
        key: String,
        value: String,
    },
    Diagnosis {
        name: String,
        fix: String,
    },
    /// A numbered part of the block's window (H6), from 1.
    Part {
        n: usize,
        label: String,
        note: String,
    },
    /// A table's head and rows (its title and note are a heading and a paragraph before it).
    Table {
        head: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// «См. также»: the pages it links, as indices into PAGES.
    See(Vec<usize>),
}

/// A page's pieces: the summary, the recipes (`recipes`: each one's block height, as the game's
/// recipes give them), the paragraphs, the stats, the diagnoses and the window's parts. `lookup`
/// gives a lang key's line, None (or empty) where the reader's language has none, which leaves
/// that piece out.
pub fn pieces(page: &Page, recipes: &[f32], lookup: &dyn Fn(&str) -> Option<String>) -> Vec<Piece> {
    let get = |rest: &str| lookup(&page.key(rest)).filter(|s| !s.trim().is_empty());
    let word = |rest: &str| lookup(&key(rest)).filter(|s| !s.trim().is_empty());
    let mut out = Vec::new();
    if let Some(sum) = get("sum") {
        out.push(Piece::Lead(sum));
    }
    if !recipes.is_empty() {
        let (w, fallback) = if recipes.len() > 1 {
            ("recipes", "Recipes")
        } else {
            ("recipe", "Recipe")
        };
        out.push(Piece::Heading(word(w).unwrap_or_else(|| fallback.into())));
        out.push(Piece::Recipes(recipes.to_vec()));
    }
    for i in 0..page.text {
        if let Some(t) = get(&format!("text.{i}")) {
            out.push(Piece::Paragraph(t));
        }
    }
    for i in 0..page.stats {
        if let (Some(key), Some(value)) = (
            get(&format!("stat.{i}.key")),
            get(&format!("stat.{i}.value")),
        ) {
            out.push(Piece::Stat { key, value });
        }
    }
    tables(page, &get, &mut out);
    let diagnoses: Vec<Piece> = (0..page.diagnoses)
        .filter_map(|i| {
            let name = get(&format!("diag.{i}"))?;
            let fix = get(&format!("diag.{i}.fix")).unwrap_or_default();
            Some(Piece::Diagnosis { name, fix })
        })
        .collect();
    if !diagnoses.is_empty() {
        out.push(Piece::Heading(
            word("diagnoses").unwrap_or_else(|| "Diagnoses".into()),
        ));
        out.extend(diagnoses);
    }
    let parts: Vec<Piece> = (0..page.parts)
        .filter_map(|i| {
            let note = get(&format!("part.{i}"))?;
            let label = get(&format!("part.{i}.label")).unwrap_or_default();
            Some(Piece::Part {
                n: i + 1,
                label,
                note,
            })
        })
        .collect();
    if !parts.is_empty() {
        out.push(Piece::Heading(
            word("window").unwrap_or_else(|| "Window".into()),
        ));
        out.extend(parts);
    }
    let see: Vec<usize> = page.see.iter().filter_map(|id| page_index(id)).collect();
    if !see.is_empty() {
        out.push(Piece::Heading(
            word("see").unwrap_or_else(|| "See also".into()),
        ));
        out.push(Piece::See(see));
    }
    out
}

/// The page's tables as pieces: each its title, its note and itself; one whose title the
/// reader's language lacks is left out.
fn tables(page: &Page, get: &dyn Fn(&str) -> Option<String>, out: &mut Vec<Piece>) {
    for (t, &[cols, rows]) in page.tables.iter().enumerate() {
        let Some(title) = get(&format!("table.{t}.title")) else {
            continue;
        };
        out.push(Piece::Heading(title));
        if let Some(note) = get(&format!("table.{t}.note")) {
            out.push(Piece::Paragraph(note));
        }
        let cell = |rest: String| get(&rest).unwrap_or_default();
        let head = (0..cols)
            .map(|c| cell(format!("table.{t}.head.{c}")))
            .collect();
        let rows = (0..rows)
            .map(|r| {
                (0..cols)
                    .map(|c| cell(format!("table.{t}.row.{r}.{c}")))
                    .collect()
            })
            .collect();
        out.push(Piece::Table { head, rows });
    }
}

/// How a line of text is coloured (the window picks the colours of the style).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The summary: the value colour.
    Lead,
    Text,
    /// A heading or a key: the label colour (a heading in capitals).
    Heading,
    Key,
    Value,
    /// A diagnosis' name: the status colour of a stopped block.
    Bad,
}

/// One line of a page as the window draws it, `x` from the body's left.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Text {
        x: f32,
        text: String,
        role: Role,
    },
    /// A stat that fits on one line: its key on the left, its value on the right.
    Pair {
        key: String,
        value: String,
    },
    /// A part's first line: its number in the brass square, then `text`.
    Number {
        n: usize,
        text: String,
        role: Role,
    },
    /// A row of recipe blocks: `n` (1 or 2) of the page's recipes from `first`, side by side.
    Recipes {
        first: usize,
        n: usize,
        height: f32,
    },
    /// A little space between pieces.
    Gap,
    /// One line of a table: each cell's text at its column's `x`.
    Cells {
        cells: Vec<(f32, String)>,
        role: Role,
    },
    /// A row of «см. также» links: up to two pages (indices into PAGES).
    Links([Option<usize>; 2]),
}

impl Row {
    pub fn height(&self) -> f32 {
        match self {
            Row::Recipes { height, .. } => *height,
            Row::Gap => 4.0,
            Row::Links(_) => LINK_HEIGHT,
            _ => LINE,
        }
    }
}

/// `text` cut into lines no wider than `width` by `measure` (a line's width in GUI pixels): at
/// spaces, and a word wider than a line at its characters.
pub fn wrap(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let joined = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if measure(&joined) <= width {
            line = joined;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if measure(word) <= width {
            line = word.to_owned();
            continue;
        }
        for c in word.chars() {
            let longer = format!("{line}{c}");
            if !line.is_empty() && measure(&longer) > width {
                lines.push(std::mem::replace(&mut line, c.to_string()));
            } else {
                line = longer;
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// `text` as it fits in `width`: whole, or cut with «…» at its end.
pub fn clip(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> String {
    if measure(text) <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    for c in text.chars() {
        let longer = format!("{out}{c}…");
        if measure(&longer) > width {
            break;
        }
        out.push(c);
    }
    format!("{}…", out.trim_end())
}

fn lines(
    out: &mut Vec<Row>,
    text: &str,
    x: f32,
    width: f32,
    role: Role,
    measure: &dyn Fn(&str) -> f32,
) {
    for t in wrap(text, width - x, measure) {
        out.push(Row::Text { x, text: t, role });
    }
}

/// The page's pieces as the lines of a body `width` wide.
pub fn lay_out(pieces: &[Piece], width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let mut last: Option<&Piece> = None;
    for piece in pieces {
        let kind = std::mem::discriminant(piece);
        let same = last.is_some_and(|l| std::mem::discriminant(l) == kind);
        if !out.is_empty() && !same && !matches!(last, Some(Piece::Heading(_))) {
            out.push(Row::Gap);
        }
        match piece {
            Piece::Lead(t) => lines(&mut out, t, 0.0, width, Role::Lead, measure),
            Piece::Heading(t) => out.push(Row::Text {
                x: 0.0,
                text: t.to_uppercase(),
                role: Role::Heading,
            }),
            Piece::Table { head, rows } => table(&mut out, head, rows, width, measure),
            Piece::See(pages) => {
                for pair in pages.chunks(2) {
                    out.push(Row::Links([pair.first().copied(), pair.get(1).copied()]));
                }
            }
            Piece::Recipes(heights) => {
                for (row, pair) in heights.chunks(2).enumerate() {
                    out.push(Row::Recipes {
                        first: row * 2,
                        n: pair.len(),
                        height: pair.iter().copied().fold(0.0, f32::max),
                    });
                }
            }
            Piece::Paragraph(t) => lines(&mut out, t, 0.0, width, Role::Text, measure),
            Piece::Stat { key, value } => {
                if measure(key) + PAIR_GAP + measure(value) <= width {
                    out.push(Row::Pair {
                        key: key.clone(),
                        value: value.clone(),
                    });
                } else {
                    lines(&mut out, key, 0.0, width, Role::Key, measure);
                    lines(&mut out, value, INDENT, width, Role::Value, measure);
                }
            }
            Piece::Diagnosis { name, fix } => {
                lines(&mut out, name, 0.0, width, Role::Bad, measure);
                lines(&mut out, fix, INDENT, width, Role::Text, measure);
            }
            Piece::Part { n, label, note } => {
                let (first, rest) = if label.is_empty() {
                    let mut l = wrap(note, width - INDENT, measure);
                    let first = if l.is_empty() {
                        String::new()
                    } else {
                        l.remove(0)
                    };
                    (
                        Row::Number {
                            n: *n,
                            text: first,
                            role: Role::Text,
                        },
                        l.join(" "),
                    )
                } else {
                    let row = Row::Number {
                        n: *n,
                        text: label.to_uppercase(),
                        role: Role::Key,
                    };
                    (row, note.clone())
                };
                out.push(first);
                lines(&mut out, &rest, INDENT, width, Role::Text, measure);
            }
        }
        last = Some(piece);
    }
    out
}

/// The rows cut into sheets: the first starts under the page's head, the others under the title.
/// A heading never ends a sheet, and no sheet starts with a gap.
pub fn sheets(rows: &[Row]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < rows.len() {
        while start < rows.len() && rows[start] == Row::Gap {
            start += 1;
        }
        if start >= rows.len() {
            break;
        }
        let top = if out.is_empty() { FIRST_TOP } else { REST_TOP };
        let room = BODY_BOTTOM - top;
        let mut used = 0.0;
        let mut end = start;
        while end < rows.len() && used + rows[end].height() <= room {
            used += rows[end].height();
            end += 1;
        }
        // A heading at the end goes on with what it heads (unless it is all the sheet holds).
        while end > start + 1 && end < rows.len() && is_heading(&rows[end - 1]) {
            end -= 1;
        }
        let end = end.max(start + 1);
        out.push(start..end);
        start = end;
    }
    if out.is_empty() {
        out.push(0..0);
    }
    out
}

fn is_heading(row: &Row) -> bool {
    matches!(
        row,
        Row::Text {
            role: Role::Heading,
            ..
        }
    ) || *row == Row::Gap
}

/// The top of the sheet `sheet`'s first line, in window pixels.
pub fn sheet_top(sheet: usize) -> f32 {
    if sheet == 0 { FIRST_TOP } else { REST_TOP }
}

// ---- Tables and links (guide: an entry's `tables`, its «См. также») ----

/// What a table keeps between its columns.
pub const COLUMN_GAP: f32 = 6.0;
/// A «см. также» link: a page's icon and name, two to a row.
pub const LINK_WIDTH: f32 = 152.0;
pub const LINK_HEIGHT: f32 = 18.0;

/// The widths of a table's columns in `width`: each its `natural` width (its widest cell on one
/// line) when that fits its fair share; what the narrow ones leave is shared by the wide ones.
pub fn columns(natural: &[f32], width: f32) -> Vec<f32> {
    let n = natural.len();
    if n == 0 {
        return Vec::new();
    }
    let mut room = (width - COLUMN_GAP * (n - 1) as f32).max(0.0);
    let mut out = vec![0.0; n];
    let mut open: Vec<usize> = (0..n).collect();
    loop {
        let fair = room / open.len() as f32;
        let narrow: Vec<usize> = open
            .iter()
            .copied()
            .filter(|&i| natural[i] <= fair)
            .collect();
        if narrow.is_empty() {
            for &i in &open {
                out[i] = fair;
            }
            return out;
        }
        for &i in &narrow {
            out[i] = natural[i];
            room -= natural[i];
        }
        open.retain(|i| !narrow.contains(i));
        if open.is_empty() {
            // Everything fits: the last column takes what is left over.
            out[n - 1] += room;
            return out;
        }
    }
}

/// The lines of one table row: each cell wrapped in its column, side by side.
fn table_row(
    out: &mut Vec<Row>,
    cells: &[String],
    widths: &[f32],
    role: Role,
    measure: &dyn Fn(&str) -> f32,
) {
    let wrapped: Vec<Vec<String>> = cells
        .iter()
        .zip(widths)
        .map(|(c, &w)| wrap(c, w, measure))
        .collect();
    let height = wrapped.iter().map(Vec::len).max().unwrap_or(0);
    for line in 0..height {
        let mut x = 0.0;
        let mut row = Vec::new();
        for (col, w) in wrapped.iter().zip(widths) {
            if let Some(t) = col.get(line) {
                row.push((x, t.clone()));
            }
            x += w + COLUMN_GAP;
        }
        out.push(Row::Cells { cells: row, role });
    }
}

/// A table laid out in `width`: its head, then each row, a little space between rows.
fn table(
    out: &mut Vec<Row>,
    head: &[String],
    rows: &[Vec<String>],
    width: f32,
    measure: &dyn Fn(&str) -> f32,
) {
    let n = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    let mut natural = vec![0.0f32; n];
    for r in std::iter::once(head).chain(rows.iter().map(Vec::as_slice)) {
        for (i, c) in r.iter().enumerate() {
            natural[i] = natural[i].max(measure(c));
        }
    }
    let widths = columns(&natural, width);
    table_row(out, head, &widths, Role::Key, measure);
    for r in rows {
        out.push(Row::Gap);
        table_row(out, r, &widths, Role::Text, measure);
    }
}

/// The page a click at `at` (window pixels) on the sheet `sheet` (its rows `range`) lands on: a
/// «см. также» link's.
pub fn link_at(rows: &[Row], range: Range<usize>, sheet: usize, at: [f32; 2]) -> Option<usize> {
    let mut y = sheet_top(sheet);
    for row in rows.get(range)? {
        if let Row::Links(pages) = row
            && at[1] >= y
            && at[1] < y + LINK_HEIGHT
            && at[0] >= BODY_X
        {
            let col = ((at[0] - BODY_X) / LINK_WIDTH) as usize;
            return pages.get(col).copied().flatten();
        }
        y += row.height();
    }
    None
}

/// The rectangle (window pixels) of link `col` of a links row whose top is `y`.
pub fn link_rect(y: f32, col: usize) -> Rect {
    [
        BODY_X + LINK_WIDTH * col as f32,
        y,
        LINK_WIDTH - 4.0,
        LINK_HEIGHT,
    ]
}

/// The page whose guide id is `id`.
pub fn page_index(id: &str) -> Option<usize> {
    PAGES.iter().position(|p| p.id == id)
}

// ---- The recipe ----

/// A crafting recipe's cells (row by row, `width` to a row; a shapeless recipe's ingredients with
/// a width of 3), each the item it shows, as the handbook's 3 × 3 grid; None for a recipe that
/// does not fit it.
pub fn recipe_grid(width: usize, cells: &[Option<String>]) -> Option<[Option<String>; 9]> {
    if width == 0 || width > 3 || cells.len() > width * 3 {
        return None;
    }
    let mut grid: [Option<String>; 9] = Default::default();
    for (i, c) in cells.iter().enumerate() {
        grid[(i / width) * 3 + i % width] = c.clone();
    }
    grid.iter().any(Option::is_some).then_some(grid)
}

#[cfg(test)]
mod tests;
