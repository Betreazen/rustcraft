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
//
// Slice 15 (O1–O8, O14): the recipe book's socket left of the grid takes the book from the cursor
// and gives it back, as a slot would (a press with the cursor's stack). With the book, the search
// also finds what the book can make and the storage has none of, veiled (O2). In the «Заказы»
// mode a click chooses the item; the row under the grid orders 1, 8 or 64 of it (O3) and sets its
// «держать N» (O14); the line under the status shows one order of the queue: what, how many done,
// what it waits for (O6, O8, O13); a click on it shows the next, «×» cancels it (O7).
use crate::game::{
    Game,
    rustcraft_content::{self as content, item_of_number},
    rustcraft_screens::{
        self as kit, HOT, LABEL, TEXT, VALUE, at, button, plural, slot, text, tr, tr_with,
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
    machines::style::{self, Severity, colour},
    modules::Module,
    orders::{KINDS, fluid_of_number},
    storage::{
        Limit,
        terminal::{
            self as term, ItemRef, LimitChange, NO_PRESS, ORDER_COUNTS, Part, Press, Take, cell,
            part_in,
        },
        view::{self, Entry, Facts, PER_PAGE, Tab},
        wire::{self, Line, Listing, OrderLine, OrderWait, Smart, State},
    },
};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

/// The search keeps at most this many characters.
const SEARCH_MAX: usize = 32;
/// A sample item's veil: an item with a limit and none stored (style: the icon at 35%).
const GHOST: u32 = colour::GHOST;
/// The veil over the limit buttons while no item is chosen.
const DIMMED: u32 = 0x9911_0D0B;
const ESCAPE: &str = "key.keyboard.escape";
const ENTER: [&str; 2] = ["key.keyboard.enter", "key.keyboard.keypad.enter"];
const BACKSPACE: &str = "key.keyboard.backspace";
/// The block of each machine kind an order step names (rustcraft_rules::orders::KINDS).
const KIND_BLOCKS: [&str; 5] = [
    content::CRAFTER,
    content::FURNACE,
    content::CRUSHER,
    content::ALLOY,
    content::MIXER,
];

/// What the window shows of the terminal it is open on: its own state, apart from the server.
#[derive(Default)]
struct View {
    terminal: Option<Pos>,
    tab: Tab,
    page: usize,
    search: String,
    focused: bool,
    limits: bool,
    /// Slice 15: the «Заказы» mode, and the order of the queue its line shows.
    orders: bool,
    shown_order: usize,
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

/// An item's id; for a fluid's number (O12) the bucket item its icon is drawn with.
fn id_of(game: &Game<'_>, item: u16) -> Option<&'static str> {
    match fluid_of_number(item) {
        Some("minecraft:water") => Some("minecraft:water_bucket"),
        Some("minecraft:lava") => Some("minecraft:lava_bucket"),
        Some(_) => Some("minecraft:bucket"),
        None => item_of_number(game, u32::from(item)),
    }
}

/// An item's name in the player's language.
fn item_name(id: &str) -> String {
    let key = id.replace(':', ".");
    tr(&format!("item.{key}"), &tr(&format!("block.{key}"), id))
}

/// The name of an item the list names by number, or of a fluid (`rustcraft.fluid.<name>.name`).
fn name_of(game: &Game<'_>, item: u16) -> String {
    if let Some(fluid) = fluid_of_number(item) {
        let name = fluid.rsplit(':').next().unwrap_or(fluid);
        return tr(&format!("rustcraft.fluid.{name}.name"), fluid);
    }
    id_of(game, item).map_or_else(|| "?".to_owned(), item_name)
}

/// The name of a machine kind (rustcraft_rules::orders::KINDS) by its code.
fn kind_name(code: u8) -> String {
    KIND_BLOCKS
        .get(usize::from(code))
        .map_or_else(|| KINDS[0].to_owned(), |b| item_name(b))
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

/// The lines the tab and the search let through, in the window's order. With the book and a
/// search, the items the book can make and the storage has none of come last (O2).
fn shown(game: &Game<'_>, listing: &Listing, v: &View) -> Vec<Entry<ItemRef>> {
    let mut entries: Vec<Entry<ItemRef>> = listing
        .lines
        .iter()
        .map(|l: &Line| Entry {
            key: (l.item, l.damage),
            count: l.count,
            limit: l.limit,
        })
        .collect();
    if listing.book && !v.search.is_empty() {
        let listed: std::collections::BTreeSet<u16> = listing
            .lines
            .iter()
            .filter(|l| l.damage == 0)
            .map(|l| l.item)
            .collect();
        entries.extend(
            listing
                .orderable
                .iter()
                .filter(|n| !listed.contains(*n))
                .map(|&n| Entry {
                    key: (n, 0),
                    count: 0,
                    limit: None,
                }),
        );
    }
    let names = |k: &ItemRef| {
        id_of(game, k.0).map_or_else(Vec::new, |id| {
            let own = fluid_of_number(k.0).unwrap_or(id);
            vec![name_of(game, k.0), own.to_owned()]
        })
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
    let open = Tab::ALL.iter().position(|t| *t == v.tab);
    let line_y = term::TABS[0][1] + term::TABS[0][3];
    let right = term::WIDTH - 8.0;
    kit::fills(
        game,
        batch,
        o,
        &style::tabs(&term::TABS, open, line_y, right),
    );
    for (i, (tab, r)) in Tab::ALL.into_iter().zip(term::TABS).enumerate() {
        let is_open = Some(i) == open;
        if !is_open {
            kit::hover(game, batch, o, r);
        }
        let colour = if is_open { VALUE } else { LABEL };
        kit::centred(game, batch, o, r, &tab_name(tab), colour);
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
        VALUE,
    );
    let orders_name = tr("rustcraft.terminal.orders", "Orders");
    button(game, batch, o, term::ORDER_MODE, &orders_name, v.orders);
    button(
        game,
        batch,
        o,
        term::LIMIT_MODE,
        &tr("rustcraft.terminal.limits", "Limits"),
        v.limits,
    );
    book_socket(game, batch, o, listing.as_ref());
    grid(game, batch, o, view::page(&list, v.page), v.selected);
    if v.orders {
        order_row(game, batch, o, &v, listing.as_ref());
        orders_status(game, batch, o, &mut v, listing.as_ref());
    } else {
        limit_row(game, batch, o, &v, listing.as_ref());
        status(game, batch, o, listing.as_ref());
    }
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
    let (name, rest) = match title.split_once(" · ") {
        Some((n, r)) => (n.to_owned(), Some(r.to_owned())),
        None => (title, None),
    };
    kit::title_line(game, batch, o, &name, rest.as_deref(), term::WIDTH);
}

fn tab_name(tab: Tab) -> String {
    match tab {
        Tab::All => tr("rustcraft.terminal.tab.all", "All"),
        Tab::Armor => tr("rustcraft.terminal.tab.armor", "Armour"),
        Tab::Tools => tr("rustcraft.terminal.tab.tools", "Tools"),
        Tab::Blocks => tr("rustcraft.terminal.tab.blocks", "Blocks"),
    }
}

/// The search field (`.w-find`, style «Поля»): a well with the magnifier, the text and a caret
/// while focused, a hint while empty; the focus has the hover outline's colour round it.
fn search_field(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], v: &View) {
    let r = term::SEARCH;
    let x = style::field_text_x(r, true);
    let room = r[0] + r[2] - x - 3.0;
    let (shown, colour) = if v.search.is_empty() && !v.focused {
        (tr("rustcraft.terminal.search", "Search"), LABEL)
    } else {
        // The end of a long search, as a text box scrolls.
        let mut s: String = v.search.clone();
        while game.native_text_width(&s) > room && !s.is_empty() {
            s.remove(0);
        }
        (s, VALUE)
    };
    let caret = v.focused.then(|| x + game.native_text_width(&shown));
    kit::fills(game, batch, o, &style::field(r, true, caret));
    if v.focused {
        kit::fills(game, batch, o, &style::outline(r, HOT));
    } else {
        kit::hover(game, batch, o, r);
    }
    text(game, batch, &shown, o, [x, r[1] + 3.0], colour);
}

/// O1: the recipe book's socket, with the book in it or empty.
fn book_socket(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], listing: Option<&Listing>) {
    let held = listing.is_some_and(|l| l.book);
    kit::socket(game, batch, o, term::BOOK, !held);
    if held {
        let p = term::BOOK;
        let stack = ItemStack::new(Module::Recipes.item(), 1);
        game.native_gui_draw_item_with(batch, &stack, [o[0] + p[0], o[1] + p[1]], None);
    }
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
            kit::fills(game, batch, o, &style::outline(style::slot_rect(p), HOT));
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
            text(game, batch, &s, o, term::LIMIT_TEXT, VALUE);
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
            game.native_fill(batch, at(o, r), DIMMED);
        }
    }
}

/// «Держать: 64» or «Держать: —».
fn keep_text(keep: u64) -> String {
    if keep == 0 {
        tr("rustcraft.terminal.keep.none", "Keep: —")
    } else {
        tr_with("rustcraft.terminal.keep", "Keep: %s", &[keep.to_string()])
    }
}

/// The orders mode's row under the grid: the chosen item, «1», «8», «64» to order, its N.
fn order_row(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    v: &View,
    listing: Option<&Listing>,
) {
    let keep = v
        .selected
        .and_then(|key| listing.and_then(|l| l.lines.iter().find(|x| (x.item, x.damage) == key)))
        .map_or(0, |l| l.keep);
    if let Some(id) = v.selected.and_then(|key| id_of(game, key.0)) {
        let p = term::LIMIT_ICON;
        game.native_item(batch, id, [o[0] + p[0], o[1] + p[1]], 12.0);
    }
    for (r, n) in term::ORDER_BUTTONS.into_iter().zip(ORDER_COUNTS) {
        button(game, batch, o, r, &n.to_string(), false);
    }
    let colour = if v.selected.is_some() { VALUE } else { LABEL };
    text(game, batch, &keep_text(keep), o, term::KEEP_TEXT, colour);
    button(game, batch, o, term::KEEP_MINUS, "-", false);
    button(game, batch, o, term::KEEP_PLUS, "+", false);
    button(game, batch, o, term::KEEP_CLEAR, "x", false);
    let on = v.selected.is_some() && listing.is_some_and(|l| l.book);
    if !on {
        for r in term::ORDER_BUTTONS.into_iter().chain([
            term::KEEP_MINUS,
            term::KEEP_PLUS,
            term::KEEP_CLEAR,
        ]) {
            game.native_fill(batch, at(o, r), DIMMED);
        }
    }
}

/// What an order waits for, in words: «в работе», «не хватает: железный слиток ×2»…
fn wait_text(game: &Game<'_>, wait: &OrderWait) -> String {
    match wait {
        OrderWait::New => tr("rustcraft.orders.wait.new", "planning"),
        OrderWait::Running => tr("rustcraft.orders.wait.running", "running"),
        OrderWait::Items(items) => {
            let named: Vec<String> = items
                .iter()
                .map(|(item, n)| format!("{} ×{n}", name_of(game, *item)))
                .collect();
            tr_with(
                "rustcraft.orders.wait.items",
                "missing: %s",
                &[named.join(", ")],
            )
        }
        OrderWait::Machines(kinds) => {
            let named: Vec<String> = kinds.iter().map(|k| kind_name(*k)).collect();
            tr_with(
                "rustcraft.orders.wait.machine",
                "no machine on the line: %s",
                &[named.join(", ")],
            )
        }
        OrderWait::Busy(kind) => tr_with(
            "rustcraft.orders.wait.busy",
            "waits for a free %s",
            &[kind_name(*kind)],
        ),
        OrderWait::Energy(kind) => tr_with(
            "rustcraft.orders.wait.energy",
            "waits for energy: %s",
            &[kind_name(*kind)],
        ),
    }
}

/// «Поршень 2/4 · не хватает: железный слиток ×2», «держать» orders marked.
fn order_text(game: &Game<'_>, order: &OrderLine) -> String {
    let mut name = name_of(game, order.item);
    if order.keep {
        name = tr_with("rustcraft.orders.kept", "%s (keep)", &[name]);
    }
    format!(
        "{name} {}/{} · {}",
        order.done,
        order.count,
        wait_text(game, &order.wait)
    )
}

/// The orders mode's status line (M15, O1) and the line of one order of the queue (O8).
fn orders_status(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    v: &mut View,
    listing: Option<&Listing>,
) {
    let (severity, s) = match listing {
        None => (
            Severity::Warn,
            tr("rustcraft.terminal.loading", "Waiting for the storage…"),
        ),
        Some(l) if !l.book => (
            Severity::Warn,
            tr(
                "rustcraft.orders.status.no_book",
                "Put the recipe book into the socket on the left",
            ),
        ),
        Some(l) if l.smart == Smart::LowTier => (
            Severity::Bad,
            tr(
                "rustcraft.orders.low_tier",
                "Orders need a node of tier 3 on the storage's line",
            ),
        ),
        Some(l) if l.smart == Smart::NoNet => (
            Severity::Bad,
            tr(
                "rustcraft.orders.no_net",
                "The storage is on no item channel",
            ),
        ),
        Some(l) => (
            Severity::Ok,
            plural(
                l.orders.len() as u64,
                "rustcraft.orders.status.queue",
                ("%s order", "%s orders", "%s orders"),
            ),
        ),
    };
    kit::status_line(game, batch, o, term::STATUS, severity, &s);
    let orders = listing.map_or(&[][..], |l| &l.orders[..]);
    if orders.is_empty() {
        let s = tr("rustcraft.orders.empty", "No orders");
        text(game, batch, &s, o, term::DETAIL, LABEL);
        return;
    }
    v.shown_order %= orders.len();
    let order = &orders[v.shown_order];
    let line = format!(
        "{}/{} {}",
        v.shown_order + 1,
        orders.len(),
        order_text(game, order)
    );
    let r = term::QUEUE;
    kit::hover(game, batch, o, r);
    let shown = kit::fit(game, &line, r[2] - 2.0);
    text(game, batch, &shown, o, term::DETAIL, TEXT);
    button(game, batch, o, term::CANCEL, "x", false);
}

/// The status line and the line under it: the storage's state, its slots or its missing cells.
fn status(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], listing: Option<&Listing>) {
    let (severity, s, detail) = match listing {
        None => (
            Severity::Warn,
            tr("rustcraft.terminal.loading", "Waiting for the storage…"),
            String::new(),
        ),
        Some(l) => match l.state {
            State::Works => (
                Severity::Ok,
                tr("rustcraft.machine.working", "Working"),
                tr_with(
                    "rustcraft.terminal.used",
                    "%s of %s slots used",
                    &[l.used.to_string(), l.total.to_string()],
                ),
            ),
            State::Holes => (
                Severity::Bad,
                tr("rustcraft.terminal.holes", "The storage is no solid box"),
                holes_line(l),
            ),
            State::NoCells => (
                Severity::Bad,
                tr(
                    "rustcraft.storage.no_cells",
                    "The terminal touches no storage cell",
                ),
                String::new(),
            ),
        },
    };
    kit::status_line(game, batch, o, term::STATUS, severity, &s);
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
/// NO_PRESS (the press goes no further, native_hooks.rs:12160-12187); a place, a limit, the book's
/// socket, an order or a «держать» button is a press for the server.
pub(super) fn on_click(game: &Game<'_>, at: [f32; 2], right: bool) -> Option<i32> {
    let orders_mode = view_for(game).orders;
    let part = part_in(at, orders_mode)?;
    let listing = {
        let v = view_for(game);
        listing_of(v.terminal)
    };
    let mut v = view_for(game);
    v.focused = part == Part::Search;
    let step = if right { 1 } else { 64 };
    match part {
        Part::Tab(tab) => {
            v.tab = tab;
            v.page = 0;
        }
        Part::Search | Part::Status => {}
        Part::Prev => v.page = v.page.saturating_sub(1),
        Part::Next => v.page += 1,
        Part::LimitMode => {
            v.limits = !v.limits;
            v.orders &= !v.limits;
        }
        Part::OrderMode => {
            v.orders = !v.orders;
            v.limits &= !v.orders;
        }
        Part::Book => return term::encode(Press::Book).or(Some(NO_PRESS)),
        Part::Cell(i) => {
            let list = listing
                .as_ref()
                .map(|l| shown(game, l, &v))
                .unwrap_or_default();
            let item = view::page(&list, v.page).get(i).map(|e| e.key);
            if v.limits || v.orders {
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
            let change = match part {
                Part::LimitMinus => LimitChange::Add(-step),
                Part::LimitPlus => LimitChange::Add(step),
                Part::LimitClear => LimitChange::Clear,
                _ => LimitChange::ToggleDestroy,
            };
            return term::encode(Press::Limit { item, change }).or(Some(NO_PRESS));
        }
        Part::Order(k) => {
            let (item, _) = v.selected?;
            let count = *ORDER_COUNTS.get(k)?;
            return term::encode(Press::Order { item, count }).or(Some(NO_PRESS));
        }
        Part::KeepMinus | Part::KeepPlus | Part::KeepClear => {
            let (item, _) = v.selected?;
            let change = match part {
                Part::KeepMinus => LimitChange::Add(-step),
                Part::KeepPlus => LimitChange::Add(step),
                _ => LimitChange::Clear,
            };
            return term::encode(Press::Keep { item, change }).or(Some(NO_PRESS));
        }
        Part::Queue => v.shown_order += 1,
        Part::Cancel => {
            let orders = listing.as_ref().map_or(&[][..], |l| &l.orders[..]);
            let order = orders.get(v.shown_order % orders.len().max(1))?.id;
            return term::encode(Press::Cancel { order }).or(Some(NO_PRESS));
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

fn hint(lines: &mut Vec<String>, key: &str, fallback: &str) {
    lines.push(format!("§7{}", tr(key, fallback)));
}

/// `native_screen_tooltip` on the terminal.
pub(super) fn tooltip(game: &Game<'_>, at: [f32; 2], lines: &mut Vec<String>) {
    let orders_mode = view_for(game).orders;
    let Some(part) = part_in(at, orders_mode) else {
        return;
    };
    match part {
        Part::Cell(i) => cell_tooltip(game, i, orders_mode, lines),
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
        Part::OrderMode => hint(
            lines,
            "rustcraft.terminal.orders_hint",
            "On: a click on an item picks it to order; the queue under the status",
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
        Part::Book => {
            lines.push(item_name(Module::Recipes.item()));
            hint(
                lines,
                "rustcraft.terminal.book_hint",
                "The recipe book's socket: with it the storage takes orders",
            );
        }
        Part::Order(k) => {
            let n = ORDER_COUNTS.get(k).copied().unwrap_or(1);
            lines.push(tr_with(
                "rustcraft.terminal.order_hint",
                "Order %s",
                &[n.to_string()],
            ));
        }
        Part::KeepMinus | Part::KeepPlus | Part::KeepClear => {
            hint(
                lines,
                "rustcraft.terminal.keep_hint",
                "Keep N: below N the storage orders the difference by itself",
            );
            hint(
                lines,
                "rustcraft.terminal.step_hint",
                "Left click: by 64 · right click: by 1",
            );
        }
        Part::Queue => queue_tooltip(game, lines),
        Part::Cancel => hint(lines, "rustcraft.terminal.cancel_hint", "Cancel the order"),
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

fn cell_tooltip(game: &Game<'_>, i: usize, orders_mode: bool, lines: &mut Vec<String>) {
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
    if id_of(game, e.key.0).is_some() {
        lines.push(name_of(game, e.key.0));
    }
    lines.push(tr_with(
        "rustcraft.terminal.count",
        "In storage: %s",
        &[e.count.to_string()],
    ));
    let orderable = listing
        .as_ref()
        .is_some_and(|l| l.book && e.key.1 == 0 && l.orderable.contains(&e.key.0));
    if e.count == 0 && orderable {
        hint(lines, "rustcraft.terminal.orderable", "Can be ordered");
    }
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
    if orders_mode {
        hint(
            lines,
            "rustcraft.terminal.pick_hint",
            "Click: pick it to order",
        );
    } else {
        hint(
            lines,
            "rustcraft.terminal.cell_hint",
            "Left click: a stack · right: one · shift: into the inventory",
        );
    }
}

/// The whole queue (O8), one order a line.
fn queue_tooltip(game: &Game<'_>, lines: &mut Vec<String>) {
    let v = view_for(game);
    let Some(listing) = listing_of(v.terminal) else {
        return;
    };
    drop(v);
    for (i, order) in listing.orders.iter().enumerate() {
        lines.push(format!("{}. {}", i + 1, order_text(game, order)));
    }
    hint(
        lines,
        "rustcraft.terminal.queue_hint",
        "Click: the next order · ×: cancel it",
    );
}
