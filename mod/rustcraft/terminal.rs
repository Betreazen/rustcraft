// The storage terminal's window on a Horizon client (docs/showcase/js/ui.js, window `terminal`;
// S4, S5, S8, TM1). The server sends the whole storage as a mod message whenever it changes
// (rustcraft_storage, `native_net_from_server`, native_hooks.rs:8400-8404); this window filters
// it by the quick filter's tab and the search, most numerous first, and shows it in pages of 27
// (rustcraft_rules::storage::view). A tab, a page or a letter of the search costs the server
// nothing: they are this window's own.
//
// The search: a mod's screen has no text box (`native_text_field` reaches only the game's own
// focused boxes, game/native_text_field.rs:80-150), so the window keeps the text itself. A click
// on the search field focuses it; while it is, `native_screen_key_pressed_pre` takes every key
// press but Escape and Enter (which only unfocus it) and Backspace (which deletes), so the
// inventory key does not close the window, and `native_screen_char_typed_pre` takes the typed
// characters (native_hooks.rs:3956-4033). The control socket's `key` and `type` go the same way
// (game/control.rs:684, 1050).
//
// A press on a stored item is a menu button naming it (rustcraft_rules::storage::terminal::Press):
// a left click takes a stack onto the cursor, a right click one item, shift and a click a stack
// into the inventory; with a stack on the cursor any place of the grid puts it in. In the «Лимиты»
// mode a click chooses the item whose limit the row under the grid changes: «−» and «+» by 64 (by
// 1 with the right button), «×» clears it, «Уничтожать» switches «уничтожать излишек».
use crate::game::{
    Game,
    rustcraft_content::{self as content, item_of_number},
    rustcraft_screens::{
        self as kit, LABEL, PANEL_EDGE, PANEL_LIGHT, SLOT_EDGE, STATUS_BAD, STATUS_OK, STATUS_TEXT,
        STATUS_WARN, TEXT, at, button, plural, slot, text, tr, tr_with,
    },
    rustcraft_storage::CHANNEL,
};
use horizon_client::{
    hud::HudBatch,
    items::ItemStack,
    mod_api::{NativeScreen, NativeScreenView},
};
use rustcraft_rules::{
    geom::Pos,
    storage::{
        Limit,
        terminal::{
            self as term, ItemRef, LimitChange, NO_PRESS, Part, Press, Take, cell, part_at,
        },
        view::{self, Entry, Facts, PER_PAGE, Tab},
        wire::{self, Line, Listing, State},
    },
};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

/// The search keeps at most this many characters.
const SEARCH_MAX: usize = 32;
/// A sample item's veil: an item with a limit and none stored.
const GHOST: u32 = 0x99C9CCD4;
const SELECTED: u32 = 0xFFF0B33C;
const ESCAPE: &str = "key.keyboard.escape";
const ENTER: [&str; 2] = ["key.keyboard.enter", "key.keyboard.keypad.enter"];
const BACKSPACE: &str = "key.keyboard.backspace";

/// What the window shows of the terminal it is open on: its own state, apart from the server.
#[derive(Default)]
struct View {
    terminal: Option<Pos>,
    tab: Tab,
    page: usize,
    search: String,
    focused: bool,
    limits: bool,
    selected: Option<ItemRef>,
}

static VIEW: LazyLock<Mutex<View>> = LazyLock::new(Mutex::default);
/// The last list the server sent.
static LISTING: Mutex<Option<Listing>> = Mutex::new(None);

fn view() -> MutexGuard<'static, View> {
    VIEW.lock().unwrap_or_else(PoisonError::into_inner)
}

fn listing_of(terminal: Option<Pos>) -> Option<Listing> {
    let l = LISTING.lock().unwrap_or_else(PoisonError::into_inner);
    l.as_ref().filter(|l| Some(l.terminal) == terminal).cloned()
}

/// The view of the terminal open now; a fresh one for a terminal it was not for.
fn view_for(game: &Game<'_>) -> MutexGuard<'static, View> {
    let pos = game.native_screen_pos();
    let mut v = view();
    if v.terminal != pos {
        *v = View {
            terminal: pos,
            ..View::default()
        };
    }
    v
}

/// `native_net_from_server`: the terminal's list.
pub(super) fn on_net(channel: &str, data: &[u8]) {
    if channel != CHANNEL {
        return;
    }
    match wire::decode(data) {
        Some(l) => *LISTING.lock().unwrap_or_else(PoisonError::into_inner) = Some(l),
        None => log::warn!("rustcraft: a terminal list that does not read"),
    }
}

pub(super) fn layout() -> NativeScreen {
    NativeScreen {
        width: term::WIDTH,
        height: term::HEIGHT,
        slots: vec![None],
        inventory_y: term::INVENTORY[1],
        inventory_x: term::INVENTORY[0],
        own_labels: true,
        own_background: true,
        ..NativeScreen::default()
    }
}

// ---- What is shown ----

/// An item's id.
fn id_of(game: &Game<'_>, item: u16) -> Option<&'static str> {
    item_of_number(game, u32::from(item))
}

/// An item's name in the player's language.
fn item_name(id: &str) -> String {
    let key = id.replace(':', ".");
    tr(&format!("item.{key}"), &tr(&format!("block.{key}"), id))
}

fn facts(game: &Game<'_>, key: &ItemRef) -> Facts {
    id_of(game, key.0)
        .and_then(|id| game.items().get(id))
        .map_or_else(Facts::default, |info| {
            Facts::of(
                info.equip_slot.as_deref(),
                info.max_damage,
                info.block.is_some(),
            )
        })
}

/// The lines the tab and the search let through, in the window's order.
fn shown(game: &Game<'_>, listing: &Listing, v: &View) -> Vec<Entry<ItemRef>> {
    let entries: Vec<Entry<ItemRef>> = listing
        .lines
        .iter()
        .map(|l: &Line| Entry {
            key: (l.item, l.damage),
            count: l.count,
            limit: l.limit,
        })
        .collect();
    let names = |k: &ItemRef| {
        id_of(game, k.0).map_or_else(Vec::new, |id| vec![item_name(id), id.to_owned()])
    };
    view::visible(&entries, v.tab, &v.search, |k| facts(game, k), names)
        .into_iter()
        .cloned()
        .collect()
}

/// A count in a slot's corner: «256», «1,2k», «35k», «1,5M».
fn short(n: u64) -> String {
    let scaled = |v: u64, unit: u64, mark: &str| {
        if v < 10 * unit {
            let tenths = v * 10 / unit;
            if tenths % 10 == 0 {
                format!("{}{mark}", tenths / 10)
            } else {
                format!("{},{}{mark}", tenths / 10, tenths % 10)
            }
        } else {
            format!("{}{mark}", v / unit)
        }
    };
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => scaled(n, 1_000, "k"),
        _ => scaled(n, 1_000_000, "M"),
    }
}

fn stack_of(game: &Game<'_>, key: ItemRef, count: u64) -> Option<ItemStack> {
    let id = id_of(game, key.0)?;
    let mut stack = ItemStack::new(id, count.clamp(1, u64::from(u32::MAX)) as u32);
    stack.damage = u32::from(key.1);
    Some(stack)
}

fn limit_text(limit: Option<Limit>) -> String {
    match limit {
        Some(l) => tr_with(
            "rustcraft.terminal.limit",
            "Limit: %s",
            &[l.max.to_string()],
        ),
        None => tr("rustcraft.terminal.limit.none", "Limit: none"),
    }
}

/// `native_screen_draw` for the terminal.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2]) {
    let mut v = view_for(game);
    let listing = listing_of(v.terminal);
    kit::background(game, batch, o, [term::WIDTH, term::HEIGHT]);
    title(game, batch, o, listing.as_ref());
    for (tab, r) in Tab::ALL.into_iter().zip(term::TABS) {
        button(game, batch, o, r, &tab_name(tab), tab == v.tab);
    }
    search_field(game, batch, o, &v);
    let list = listing
        .as_ref()
        .map(|l| shown(game, l, &v))
        .unwrap_or_default();
    let pages = view::pages(list.len());
    v.page = v.page.min(pages - 1);
    button(game, batch, o, term::PREV, "<", false);
    button(game, batch, o, term::NEXT, ">", false);
    let page = format!("{}/{}", v.page + 1, pages);
    let w = game.native_text_width(&page);
    let r = term::PAGE_TEXT;
    text(
        game,
        batch,
        &page,
        o,
        [r[0] + (r[2] - w) / 2.0, r[1] + 3.0],
        TEXT,
    );
    button(
        game,
        batch,
        o,
        term::LIMIT_MODE,
        &tr("rustcraft.terminal.limits", "Limits"),
        v.limits,
    );
    grid(game, batch, o, view::page(&list, v.page), v.selected);
    limit_row(game, batch, o, &v, listing.as_ref());
    status(game, batch, o, listing.as_ref());
    kit::inventory(game, batch, o, term::INVENTORY, term::INVENTORY_LABEL_Y);
}

/// «Склад · 4 ячейки · 108 слотов».
fn title(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], listing: Option<&Listing>) {
    let mut title = tr("rustcraft.terminal.title", "Storage");
    if let Some(l) = listing.filter(|l| l.state != State::NoCells) {
        let cells = plural(
            u64::from(l.cells),
            "rustcraft.window.cells",
            ("%s cell", "%s cells", "%s cells"),
        );
        let slots = plural(
            l.total,
            "rustcraft.window.slots",
            ("%s slot", "%s slots", "%s slots"),
        );
        title = format!("{title} · {cells} · {slots}");
    }
    text(game, batch, &title, o, term::TITLE, TEXT);
}

fn tab_name(tab: Tab) -> String {
    match tab {
        Tab::All => tr("rustcraft.terminal.tab.all", "All"),
        Tab::Armor => tr("rustcraft.terminal.tab.armor", "Armour"),
        Tab::Tools => tr("rustcraft.terminal.tab.tools", "Tools"),
        Tab::Blocks => tr("rustcraft.terminal.tab.blocks", "Blocks"),
    }
}

/// The search field (`.w-find`): its text and a caret while focused, a hint while empty.
fn search_field(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], v: &View) {
    let r = at(o, term::SEARCH);
    game.native_fill(batch, r, if v.focused { PANEL_EDGE } else { SLOT_EDGE });
    game.native_fill(
        batch,
        [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
        PANEL_LIGHT,
    );
    let shown = if v.search.is_empty() && !v.focused {
        (tr("rustcraft.terminal.search", "Search"), LABEL)
    } else {
        let caret = if v.focused { "_" } else { "" };
        // The end of a long search, as a text box scrolls.
        let mut s: String = v.search.clone();
        while game.native_text_width(&format!("{s}{caret}")) > r[2] - 6.0 && !s.is_empty() {
            s.remove(0);
        }
        (format!("{s}{caret}"), TEXT)
    };
    game.native_text(batch, &shown.0, [r[0] + 3.0, r[1] + 3.0], shown.1, false);
}

/// The page's items: icon, count and wear; a limit without items veiled; the chosen one framed.
fn grid(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    page: &[Entry<ItemRef>],
    selected: Option<ItemRef>,
) {
    for i in 0..PER_PAGE {
        slot(game, batch, o, cell(i));
    }
    for (i, e) in page.iter().enumerate() {
        let p = cell(i);
        let at_item = [o[0] + p[0], o[1] + p[1]];
        if selected == Some(e.key) {
            game.native_fill(
                batch,
                [at_item[0] - 1.0, at_item[1] - 1.0, 18.0, 18.0],
                SELECTED,
            );
            game.native_fill(batch, [at_item[0], at_item[1], 16.0, 16.0], kit::SLOT);
        }
        let Some(stack) = stack_of(game, e.key, e.count) else {
            continue;
        };
        let count = (e.count != 1).then(|| short(e.count));
        game.native_gui_draw_item_with(batch, &stack, at_item, count.as_deref());
        if e.count == 0 {
            game.native_fill(batch, [at_item[0], at_item[1], 16.0, 16.0], GHOST);
        }
    }
}

/// The row under the grid: the chosen item's limit and its buttons.
fn limit_row(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    v: &View,
    listing: Option<&Listing>,
) {
    let line = v
        .selected
        .and_then(|key| listing.and_then(|l| l.lines.iter().find(|x| (x.item, x.damage) == key)));
    let destroy = line
        .and_then(|l| l.limit)
        .is_some_and(|l| l.destroy_surplus);
    match v.selected {
        Some(key) => {
            if let Some(id) = id_of(game, key.0) {
                let p = term::LIMIT_ICON;
                game.native_item(batch, id, [o[0] + p[0], o[1] + p[1]], 12.0);
            }
            let s = limit_text(line.and_then(|l| l.limit));
            text(game, batch, &s, o, term::LIMIT_TEXT, TEXT);
        }
        None => {
            let s = tr("rustcraft.terminal.limit.choose", "Limit: —");
            text(
                game,
                batch,
                &s,
                o,
                [term::LIMIT_ICON[0], term::LIMIT_TEXT[1]],
                LABEL,
            );
        }
    }
    let on = v.selected.is_some();
    button(game, batch, o, term::LIMIT_MINUS, "-", false);
    button(game, batch, o, term::LIMIT_PLUS, "+", false);
    button(game, batch, o, term::LIMIT_CLEAR, "x", false);
    let name = tr("rustcraft.terminal.destroy", "Destroy");
    button(game, batch, o, term::LIMIT_DESTROY, &name, on && destroy);
    if !on {
        // Dimmed while no item is chosen.
        for r in [
            term::LIMIT_MINUS,
            term::LIMIT_PLUS,
            term::LIMIT_CLEAR,
            term::LIMIT_DESTROY,
        ] {
            game.native_fill(batch, at(o, r), 0x66C9CCD4);
        }
    }
}

/// The status line and the line under it: the storage's state, its slots or its missing cells.
fn status(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], listing: Option<&Listing>) {
    let (colour, s, detail) = match listing {
        None => (
            STATUS_WARN,
            tr("rustcraft.terminal.loading", "Waiting for the storage…"),
            String::new(),
        ),
        Some(l) => match l.state {
            State::Works => (
                STATUS_OK,
                tr("rustcraft.machine.working", "Working"),
                tr_with(
                    "rustcraft.terminal.used",
                    "%s of %s slots used",
                    &[l.used.to_string(), l.total.to_string()],
                ),
            ),
            State::Holes => (
                STATUS_BAD,
                tr("rustcraft.terminal.holes", "The storage is no solid box"),
                holes_line(l),
            ),
            State::NoCells => (
                STATUS_BAD,
                tr(
                    "rustcraft.storage.no_cells",
                    "The terminal touches no storage cell",
                ),
                String::new(),
            ),
        },
    };
    let r = at(o, term::STATUS);
    game.native_fill(batch, r, colour);
    let at_text = [r[0] + 5.0, r[1] + (r[3] - kit::FONT_HEIGHT) / 2.0 + 1.0];
    game.native_text(batch, &s, at_text, STATUS_TEXT, false);
    text(game, batch, &detail, o, term::DETAIL, TEXT);
}

/// «Нет ячейки: 10 64 -3; 11 64 -3 и ещё 2».
fn holes_line(l: &Listing) -> String {
    let shown: Vec<String> = l
        .holes
        .iter()
        .take(2)
        .map(|h| format!("{} {} {}", h[0], h[1], h[2]))
        .collect();
    let mut s = tr_with(
        "rustcraft.terminal.holes_at",
        "No cell at %s",
        &[shown.join("; ")],
    );
    let rest = l.hole_count as usize - shown.len().min(l.hole_count as usize);
    if rest > 0 {
        s.push_str(&tr_with(
            "rustcraft.terminal.holes_more",
            " and %s more",
            &[rest.to_string()],
        ));
    }
    s
}

// ---- Presses ----

/// `native_screen_click` on the terminal: the window's own parts change its view and send
/// NO_PRESS (the press goes no further, native_hooks.rs:12160-12187); a place or a limit button is
/// a press for the server.
pub(super) fn on_click(game: &Game<'_>, at: [f32; 2], right: bool) -> Option<i32> {
    let part = part_at(at)?;
    let listing = {
        let v = view_for(game);
        listing_of(v.terminal)
    };
    let mut v = view_for(game);
    v.focused = part == Part::Search;
    match part {
        Part::Tab(tab) => {
            v.tab = tab;
            v.page = 0;
        }
        Part::Search | Part::Status => {}
        Part::Prev => v.page = v.page.saturating_sub(1),
        Part::Next => v.page += 1,
        Part::LimitMode => v.limits = !v.limits,
        Part::Cell(i) => {
            let list = listing
                .as_ref()
                .map(|l| shown(game, l, &v))
                .unwrap_or_default();
            let item = view::page(&list, v.page).get(i).map(|e| e.key);
            if v.limits {
                if item.is_some() {
                    v.selected = item;
                }
                return Some(NO_PRESS);
            }
            let take = if game.native_has_shift_down() {
                Take::ToInventory
            } else if right {
                Take::One
            } else {
                Take::Stack
            };
            return term::encode(Press::Place { item, take }).or(Some(NO_PRESS));
        }
        Part::LimitMinus | Part::LimitPlus | Part::LimitClear | Part::LimitDestroy => {
            let item = v.selected?;
            let step = if right { 1 } else { 64 };
            let change = match part {
                Part::LimitMinus => LimitChange::Add(-step),
                Part::LimitPlus => LimitChange::Add(step),
                Part::LimitClear => LimitChange::Clear,
                _ => LimitChange::ToggleDestroy,
            };
            return term::encode(Press::Limit { item, change }).or(Some(NO_PRESS));
        }
    }
    Some(NO_PRESS)
}

/// Whether the open screen is a terminal's.
fn is_terminal(screen: &NativeScreenView<'_>) -> bool {
    screen.menu == Some(content::TERMINAL)
}

/// `native_screen_key_pressed_pre`: while the search is focused it takes the keys.
pub(super) fn on_key(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    key: &str,
    canceled: &mut bool,
) {
    if *canceled || !is_terminal(screen) {
        return;
    }
    let mut v = view_for(game);
    if !v.focused {
        return;
    }
    *canceled = true;
    if key == ESCAPE || ENTER.contains(&key) {
        v.focused = false;
    } else if key == BACKSPACE {
        v.search.pop();
        v.page = 0;
    }
}

/// `native_screen_char_typed_pre`: a character typed into the focused search.
pub(super) fn on_char(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if *canceled || !is_terminal(screen) {
        return;
    }
    let mut v = view_for(game);
    if !v.focused {
        return;
    }
    *canceled = true;
    if v.search.chars().count() < SEARCH_MAX && !c.is_control() {
        v.search.push(c);
        v.page = 0;
    }
}

// ---- Tooltips ----

/// `native_screen_tooltip` on the terminal.
pub(super) fn tooltip(game: &Game<'_>, at: [f32; 2], lines: &mut Vec<String>) {
    let Some(part) = part_at(at) else {
        return;
    };
    let hint = |lines: &mut Vec<String>, key: &str, fallback: &str| {
        lines.push(format!("§7{}", tr(key, fallback)));
    };
    match part {
        Part::Cell(i) => {
            let v = view_for(game);
            let listing = listing_of(v.terminal);
            let list = listing
                .as_ref()
                .map(|l| shown(game, l, &v))
                .unwrap_or_default();
            let Some(e) = view::page(&list, v.page).get(i).cloned() else {
                return;
            };
            drop(v);
            if let Some(id) = id_of(game, e.key.0) {
                lines.push(item_name(id));
            }
            lines.push(tr_with(
                "rustcraft.terminal.count",
                "In storage: %s",
                &[e.count.to_string()],
            ));
            if e.key.1 > 0 {
                lines.push(tr_with(
                    "rustcraft.terminal.worn",
                    "Worn: %s",
                    &[e.key.1.to_string()],
                ));
            }
            if let Some(l) = e.limit {
                let what = if l.destroy_surplus {
                    tr(
                        "rustcraft.terminal.over.destroy",
                        "over the limit: destroyed",
                    )
                } else {
                    tr("rustcraft.terminal.over.keep", "over the limit: not taken")
                };
                lines.push(format!("{} · {what}", limit_text(Some(l))));
            }
            hint(
                lines,
                "rustcraft.terminal.cell_hint",
                "Left click: a stack · right: one · shift: into the inventory",
            );
        }
        Part::Search => hint(
            lines,
            "rustcraft.terminal.search_hint",
            "Click and type: finds by name",
        ),
        Part::Tab(_) => hint(lines, "rustcraft.terminal.tab_hint", "Quick filter"),
        Part::LimitMode => hint(
            lines,
            "rustcraft.terminal.limits_hint",
            "On: a click on an item picks it for its limit",
        ),
        Part::LimitMinus | Part::LimitPlus => hint(
            lines,
            "rustcraft.terminal.step_hint",
            "Left click: by 64 · right click: by 1",
        ),
        Part::LimitClear => hint(lines, "rustcraft.terminal.clear_hint", "No limit"),
        Part::LimitDestroy => hint(
            lines,
            "rustcraft.terminal.destroy_hint",
            "Destroy the surplus: what comes over the limit vanishes instead of staying in the line",
        ),
        Part::Status => {
            let v = view_for(game);
            if let Some(l) = listing_of(v.terminal).filter(|l| l.state == State::Holes) {
                for h in &l.holes {
                    lines.push(format!("{} {} {}", h[0], h[1], h[2]));
                }
            }
        }
        Part::Prev | Part::Next => {}
    }
}
