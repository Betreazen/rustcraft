// The storage terminal's window on a Horizon client (docs/showcase/js/ui.js, window `terminal`;
// S4, S5, S8, TM1). The server sends the whole storage as a mod message whenever it changes
// (rustcraft_storage, `native_net_from_server`, native_hooks.rs:8400-8404); this window filters
// it by the quick filter's tab and the search, most numerous first, and shows it in pages of 27
// (rustcraft_rules::storage::view). A tab, a page or a letter of the search costs the server
// nothing: they are this window's own.
//
// 0.21.2 (spec 49, rustcraft_rules::storage::typed): the chosen item's limit, the order's count
// and «Держать: N» are fields too. A click on the number opens it with the number selected; digits
// replace it, Backspace deletes, Enter applies, Escape or a click elsewhere leaves the old number.
// Only one field (the search or a number) is focused at a time, and while one is, it takes every
// key as the search does. The order's count is the window's own; a limit or an N goes to the server
// as a message on its own channel (`client_tick`), the tablet's as its own press.
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
//
// 0.21.1 (OT12): what the book can make comes on its own channel, once a window opens and again
// when the book is built anew (rustcraft_rules::storage::orderable), and takes the place of what
// the list had room for: on a big storage with many enchanted stacks it had none.
// The component stacks too (OT13): the terminal's and the tablet's each come whole on their own
// channel when they change, and a list takes them when it names their stamp.
use crate::game::{
    Game,
    rustcraft_content::{self as content, item_of_number},
    rustcraft_screens::{
        self as kit, HOT, LABEL, TEXT, VALUE, at, button, plural, slot, text, tr, tr_with,
    },
    rustcraft_storage::{self as storage, CHANNEL},
    rustcraft_tablet as tablet_window,
    rustcraft_terminal_net as terminal_net,
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
    orders::{KINDS, fluid_of_number, groups::group_of_number},
    storage::{
        Limit,
        modules as sockets,
        network::{self as net, Ask},
        orderable as book_items,
        variants as stacks,
        terminal::{
            self as term, ItemRef, LimitChange, NO_PRESS, Part, Press, Take, cell,
            part_in,
        },
        typed::{self, Field, Setting, Typing},
        view::{self, Entry, Facts, Over, PER_PAGE, Tab},
        wire::{self, Line, Listing, OrderLine, OrderWait, Smart, State},
    },
};
use horizon_client::save;
use horizon_world::nbt;
use std::collections::HashMap;
use std::sync::{
    LazyLock, Mutex, MutexGuard, PoisonError,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// The search keeps at most this many characters.
const SEARCH_MAX: usize = 32;
/// 0.21.2 (spec 49): a window not drawn for this long was closed: its number field is left.
const CLOSED_AFTER: Duration = Duration::from_millis(500);
/// The selected number's highlight in a field.
const SELECTION: u32 = 0xAA33_5A8C;
/// A sample item's veil: an item with a limit and none stored (style: the icon at 35%).
const GHOST: u32 = colour::GHOST;
/// The veil over the limit buttons while no item is chosen.
const DIMMED: u32 = 0x9911_0D0B;
const ESCAPE: &str = "key.keyboard.escape";
const ENTER: [&str; 2] = ["key.keyboard.enter", "key.keyboard.keypad.enter"];
const BACKSPACE: &str = "key.keyboard.backspace";
/// The block of each machine kind an order step names (rustcraft_rules::orders::KINDS).
const KIND_BLOCKS: [&str; 6] = [
    content::CRAFTER,
    content::FURNACE,
    content::CRUSHER,
    content::ALLOY,
    content::MIXER,
    "rustcraft:kitchen",
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
    /// Slice 18: the count the orders row orders (0 reads as 1).
    order_count: u16,
    /// Round 32 (spec 33): the «Сеть» page in the grid's place, and its page.
    network: bool,
    net_page: usize,
    /// 0.21.2 (spec 49): the number field being typed into; the search is not focused then.
    typing: Option<Typing>,
    /// When the window was last drawn: one not drawn since was closed.
    drawn: Option<Instant>,
}

static VIEW: LazyLock<Mutex<View>> = LazyLock::new(Mutex::default);
/// The last list the server sent.
static LISTING: Mutex<Option<Listing>> = Mutex::new(None);

// ---- Slice 18: the tablet's window is this window in its tablet context ----

/// Drawing, clicking or typing for the tablet's screen now (rustcraft_tablet): the view and the
/// list are the tablet's, the sockets and the orders are not shown.
static TABLET: AtomicBool = AtomicBool::new(false);
static TABLET_VIEW: LazyLock<Mutex<View>> = LazyLock::new(Mutex::default);
static TABLET_LISTING: Mutex<Option<Listing>> = Mutex::new(None);

fn tablet() -> bool {
    TABLET.load(Ordering::Relaxed)
}

/// Runs `f` in the tablet's context.
pub(super) fn in_tablet<R>(f: impl FnOnce() -> R) -> R {
    TABLET.store(true, Ordering::Relaxed);
    let r = f();
    TABLET.store(false, Ordering::Relaxed);
    r
}

/// The list the server sent the tablet's window.
pub(super) fn set_tablet_listing(mut l: Listing) {
    complete_list(&mut l, true);
    remember_variants(&l.variants);
    *TABLET_LISTING.lock().unwrap_or_else(PoisonError::into_inner) = Some(l);
}

/// 0.21.1 (OT12): the recipe book's items as their own channel last brought them whole
/// (rustcraft_rules::storage::orderable); the terminal's and the tablet's windows share them.
static ORDERABLE: LazyLock<Mutex<book_items::Assembler>> = LazyLock::new(Mutex::default);
/// 0.21.1 (OT13): the component stacks of the terminal's and of the tablet's storage as their own
/// channels last brought them whole (rustcraft_rules::storage::variants).
static TERMINAL_STACKS: LazyLock<Mutex<stacks::Assembler>> = LazyLock::new(Mutex::default);
static TABLET_STACKS: LazyLock<Mutex<stacks::Assembler>> = LazyLock::new(Mutex::default);

fn stacks_of(tablet: bool) -> MutexGuard<'static, stacks::Assembler> {
    let s = if tablet { &TABLET_STACKS } else { &TERMINAL_STACKS };
    s.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A list with the whole lists of the book's items and of its component stacks in place of what
/// its message had room for (the stacks only when the list names their stamp).
fn complete_list(l: &mut Listing, tablet: bool) {
    stacks::complete(l, &stacks_of(tablet));
    let a = ORDERABLE.lock().unwrap_or_else(PoisonError::into_inner);
    book_items::complete(l, a.list());
}

/// A part of the book's items; once the list is whole, the windows' lists take it.
fn on_orderable(data: &[u8]) {
    let mut a = ORDERABLE.lock().unwrap_or_else(PoisonError::into_inner);
    if !a.take(data) {
        return;
    }
    let whole = a.list().map(<[u16]>::to_vec);
    drop(a);
    for slot in [&LISTING, &TABLET_LISTING] {
        if let Some(l) = slot.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            book_items::complete(l, whole.as_deref());
        }
    }
}

/// A part of a window's component stacks; once they are whole, its list takes them if it names
/// their stamp.
fn on_stacks(data: &[u8], tablet: bool) {
    if !stacks_of(tablet).take(data) {
        return;
    }
    let slot = if tablet { &TABLET_LISTING } else { &LISTING };
    let mut l = slot.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(l) = l.as_mut() {
        stacks::complete(l, &stacks_of(tablet));
        remember_variants(&l.variants);
    }
}

/// A fresh view for a tablet's window that opens.
pub(super) fn reset_tablet_view() {
    *TABLET_VIEW.lock().unwrap_or_else(PoisonError::into_inner) = View::default();
    terminal_net::forget_tablet();
}

fn view() -> MutexGuard<'static, View> {
    if tablet() {
        return TABLET_VIEW.lock().unwrap_or_else(PoisonError::into_inner);
    }
    VIEW.lock().unwrap_or_else(PoisonError::into_inner)
}

fn listing_of(terminal: Option<Pos>) -> Option<Listing> {
    if tablet() {
        return TABLET_LISTING.lock().unwrap_or_else(PoisonError::into_inner).clone();
    }
    let l = LISTING.lock().unwrap_or_else(PoisonError::into_inner);
    l.as_ref().filter(|l| Some(l.terminal) == terminal).cloned()
}

/// The view of the terminal open now; a fresh one for a terminal it was not for.
fn view_for(game: &Game<'_>) -> MutexGuard<'static, View> {
    if tablet() {
        let mut v = view();
        v.orders = false;
        return v;
    }
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
    if channel == book_items::CHANNEL {
        on_orderable(data);
        return;
    }
    if channel == stacks::CHANNEL || channel == stacks::TABLET_CHANNEL {
        on_stacks(data, channel == stacks::TABLET_CHANNEL);
        return;
    }
    if channel != CHANNEL {
        return;
    }
    match wire::decode(data) {
        Some(mut l) => {
            complete_list(&mut l, false);
            remember_variants(&l.variants);
            *LISTING.lock().unwrap_or_else(PoisonError::into_inner) = Some(l);
        }
        None => log::warn!("rustcraft: a terminal list that does not read"),
    }
}

/// Slice 18: the component stacks of the last list, by the place a line names them with.
static VARIANTS: LazyLock<Mutex<HashMap<ItemRef, ItemStack>>> = LazyLock::new(Mutex::default);

/// Reads the saved stacks of a list's component lines (the tablet's window shares them).
pub(super) fn remember_variants(variants: &[wire::Variant]) {
    let stacks: HashMap<ItemRef, ItemStack> = variants
        .iter()
        .filter_map(|v| {
            let bytes = rustcraft_rules::storage::keys::unhex(&v.payload)?;
            let (tag, _) = nbt::read_network(&bytes, nbt::Limits::saved()).ok()?;
            Some(((v.item, term::VARIANT | v.index), save::item_from_nbt(&tag)?))
        })
        .collect();
    *VARIANTS.lock().unwrap_or_else(PoisonError::into_inner) = stacks;
}

/// The component stack a line names; none for a plain line.
fn variant(key: ItemRef) -> Option<ItemStack> {
    (key.1 & term::VARIANT != 0)
        .then(|| VARIANTS.lock().unwrap_or_else(PoisonError::into_inner).get(&key).cloned())
        .flatten()
}

/// A level as the game writes it in a tooltip: I to X, past it the number.
fn roman(level: u32) -> String {
    const R: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    R.get(level.wrapping_sub(1) as usize)
        .map_or_else(|| level.to_string(), |r| (*r).to_owned())
}

/// What a component stack's tooltip says beyond its item's name: its own name, its
/// enchantments and a potion's contents.
pub(super) fn variant_lines(stack: &ItemStack) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(name) = stack.custom_name() {
        out.push(name);
    }
    for (id, level) in stack.enchantments().iter().chain(stack.stored_enchantments()) {
        let key = format!("enchantment.{}", id.replace(':', "."));
        out.push(format!("{} {}", tr(&key, id), roman(*level)));
    }
    if let Some(potion) = stack.potion() {
        let name = potion.rsplit(':').next().unwrap_or(potion);
        let item = stack.item.rsplit(':').next().unwrap_or("potion");
        let key = format!("item.minecraft.{item}.effect.{name}");
        out.push(tr(&key, potion));
    }
    out
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
    // The game's bucket for water, lava and milk (round 31: the storage's milk too), the mod's
    // display bucket for the solution and the catalysts (0.21.2), the empty bucket for any other.
    match fluid_of_number(item) {
        Some(fluid) => Some(rustcraft_rules::fluid_icons::icon_of(fluid)),
        None => item_of_number(game, u32::from(item)),
    }
}

/// An item's name in the player's language.
pub(super) fn item_name(id: &str) -> String {
    let key = id.replace(':', ".");
    tr(&format!("item.{key}"), &tr(&format!("block.{key}"), id))
}

/// The name of an item the list names by number, of a fluid (`rustcraft.fluid.<name>.name`) or,
/// 0.21 (OT5), of a group an order waits for (`rustcraft.group.<name>.name`: «доски любого
/// дерева»).
fn name_of(game: &Game<'_>, item: u16) -> String {
    if let Some(group) = group_of_number(item) {
        return tr(&format!("rustcraft.group.{group}.name"), group);
    }
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

fn facts(game: &Game<'_>, key: &ItemRef, own: &[u16]) -> Facts {
    let mut f = id_of(game, key.0)
        .and_then(|id| game.items().get(id))
        .map_or_else(Facts::default, |info| {
            Facts::of(
                info.equip_slot.as_deref(),
                info.max_damage,
                info.block.is_some(),
            )
        });
    // Slice 18: «своя» shows what the filter module marks.
    f.own = own.contains(&key.0);
    f
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
            routed: l.routed,
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
                    routed: false,
                }),
        );
    }
    let names = |k: &ItemRef| {
        id_of(game, k.0).map_or_else(Vec::new, |id| {
            let own = fluid_of_number(k.0).unwrap_or(id);
            let mut names = vec![name_of(game, k.0), own.to_owned()];
            // Slice 18: a component stack is found by its name, enchantments and potion too.
            names.extend(variant(*k).map(|s| variant_lines(&s)).unwrap_or_default());
            names
        })
    };
    view::visible(&entries, v.tab, &v.search, |k| facts(game, k, &listing.own), names)
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
    if let Some(mut stack) = variant(key) {
        stack.count = count.clamp(1, u64::from(u32::MAX)) as u32;
        return Some(stack);
    }
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
    if v.drawn.is_none_or(|t| t.elapsed() > CLOSED_AFTER) {
        v.typing = None;
    }
    v.drawn = Some(Instant::now());
    let listing = listing_of(v.terminal);
    kit::background(game, batch, o, [term::WIDTH, term::HEIGHT]);
    title(game, batch, o, listing.as_ref());
    // Round 32 (spec 33): the «Сеть» button, right of the title.
    let net_name = tr("rustcraft.netpage.button", "Network");
    button(game, batch, o, net::NET_MODE, &net_name, v.network);
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
    if v.network {
        network_page(game, batch, o, &mut v);
        return;
    }
    let list = listing
        .as_ref()
        .map(|l| shown(game, l, &v))
        .unwrap_or_default();
    // Slice 18: the «Лимиты» mode lists every item as a row, those with a limit first.
    let list = if v.limits { view::limits_first(list) } else { list };
    let pages = if v.limits {
        list.len().div_ceil(term::LIMIT_ROWS).max(1)
    } else {
        view::pages(list.len())
    };
    v.page = v.page.min(pages - 1);
    pager(game, batch, o, v.page, pages);
    mode_buttons(game, batch, o, &v);
    if !tablet() {
        sockets(game, batch, o, listing.as_ref());
    }
    if v.limits {
        limit_rows(game, batch, o, limit_page(&list, v.page), v.selected);
    } else {
        grid(game, batch, o, view::page(&list, v.page), v.selected);
    }
    if v.orders {
        order_row(game, batch, o, &v, listing.as_ref());
        orders_status(game, batch, o, &mut v, listing.as_ref());
    } else {
        limit_row(game, batch, o, &v, listing.as_ref());
        status(game, batch, o, listing.as_ref());
    }
    kit::inventory(game, batch, o, term::INVENTORY, term::INVENTORY_LABEL_Y);
}

/// The page buttons of the top row and «2/5» between them.
fn pager(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], page: usize, pages: usize) {
    button(game, batch, o, term::PREV, "<", false);
    button(game, batch, o, term::NEXT, ">", false);
    let page = format!("{}/{}", page + 1, pages);
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
}

/// «Заказы» (not in the tablet) and «Лимиты».
fn mode_buttons(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], v: &View) {
    if !tablet() {
        let orders_name = tr("rustcraft.terminal.orders", "Orders");
        button(game, batch, o, term::ORDER_MODE, &orders_name, v.orders);
    }
    button(
        game,
        batch,
        o,
        term::LIMIT_MODE,
        &tr("rustcraft.terminal.limits", "Limits"),
        v.limits,
    );
}

/// Round 32 (spec 33): the «Сеть» page in the place of the grid, the row under it and the status
/// (rustcraft_terminal_net draws its rows); the tabs and the search veiled, the page buttons
/// turning its pages. Drawing it is what keeps the client asking the server for it.
fn network_page(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], v: &mut View) {
    for r in term::TABS.into_iter().chain([term::SEARCH]) {
        game.native_fill(batch, at(o, r), DIMMED);
    }
    let page = terminal_net::page_for(tablet(), v.terminal);
    let pages = terminal_net::page_count(page.as_ref());
    v.net_page = v.net_page.min(pages - 1);
    pager(game, batch, o, v.net_page, pages);
    mode_buttons(game, batch, o, v);
    terminal_net::draw(game, batch, o, page.as_ref(), v.net_page);
    if tablet() {
        terminal_net::shown(Ask::Tablet);
    } else if let Some(t) = v.terminal {
        terminal_net::shown(Ask::Terminal(t));
    }
    kit::inventory(game, batch, o, term::INVENTORY, term::INVENTORY_LABEL_Y);
}

/// «Склад · 4 ячейки · 108 слотов», kept clear of the «Сеть» button.
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
    let room = net::NET_MODE[0] - 4.0 - term::TITLE[0] - game.native_text_width(&name) - game.native_text_width(" · ");
    let rest = rest.map(|r| kit::fit(game, &r, room.max(0.0)));
    kit::title_line(game, batch, o, &name, rest.as_deref(), term::WIDTH);
}

fn tab_name(tab: Tab) -> String {
    match tab {
        Tab::All => tr("rustcraft.terminal.tab.all", "All"),
        Tab::Armor => tr("rustcraft.terminal.tab.armor", "Armour"),
        Tab::Tools => tr("rustcraft.terminal.tab.tools", "Tools"),
        Tab::Blocks => tr("rustcraft.terminal.tab.blocks", "Blocks"),
        Tab::Own => tr("rustcraft.terminal.tab.own", "Own"),
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

/// The module in socket `k` as the list names it; the book from the header when the list has no
/// sockets section.
fn socket_item(game: &Game<'_>, listing: Option<&Listing>, k: usize) -> Option<&'static str> {
    let l = listing?;
    match l.sockets.get(k) {
        Some(&n) if n != wire::NO_ITEM => item_of_number(game, u32::from(n)),
        Some(_) => None,
        None => (k == sockets::SOCKET_BOOK && l.book).then(|| Module::Recipes.item()),
    }
}

/// O1, slice 18: the stack module's, the recipe book's and the filter's sockets, full or empty.
fn sockets(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], listing: Option<&Listing>) {
    for (k, p) in term::SOCKETS.into_iter().enumerate() {
        let held = socket_item(game, listing, k);
        kit::socket(game, batch, o, p, held.is_none());
        if let Some(item) = held {
            let stack = ItemStack::new(item, 1);
            game.native_gui_draw_item_with(batch, &stack, [o[0] + p[0], o[1] + p[1]], None);
        }
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

/// Slice 18: the «Лимиты» mode's page `page` of rows.
fn limit_page<T>(list: &[T], page: usize) -> &[T] {
    let start = (page * term::LIMIT_ROWS).min(list.len());
    &list[start..(start + term::LIMIT_ROWS).min(list.len())]
}

/// «лимит 512 · сверх лимита: в переработку», «лимит 64 · сверх лимита 6: некуда девать», or
/// «без лимита» (0.21.2, SP15).
fn limit_words<K>(e: &Entry<K>) -> String {
    let Some(over) = e.over() else {
        return tr("rustcraft.terminal.limit.none", "Limit: none");
    };
    let words = match over {
        Over::Destroyed => tr("rustcraft.terminal.over.destroy", "over the limit: destroyed"),
        Over::Recycled => tr("rustcraft.terminal.over.out", "over the limit: recycled"),
        Over::Stays => tr("rustcraft.terminal.over.keep", "over the limit: stays"),
        Over::Leaving(n) => tr_with("rustcraft.terminal.over.leaving", "%s over the limit: recycled", &[n.to_string()]),
        Over::Stuck(n) => tr_with("rustcraft.terminal.over.stuck", "%s over the limit: nowhere to go", &[n.to_string()]),
    };
    format!("{} · {words}", limit_text(e.limit))
}

/// Slice 18 (guide `terminal`: limits as a list): each item a row with its icon, its name and
/// its limit; the chosen one framed.
fn limit_rows(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    page: &[Entry<ItemRef>],
    selected: Option<ItemRef>,
) {
    for (i, e) in page.iter().enumerate() {
        let r = term::limit_row(i);
        kit::hover(game, batch, o, r);
        if selected == Some(e.key) {
            kit::fills(game, batch, o, &style::outline(r, HOT));
        }
        if let Some(stack) = stack_of(game, e.key, 1) {
            game.native_item(batch, &stack.item, [o[0] + r[0] + 1.0, o[1] + r[1] + 1.0], 8.0);
        }
        let name = kit::fit(game, &name_of(game, e.key.0), 70.0);
        text(game, batch, &name, o, [r[0] + 12.0, r[1] + 1.0], VALUE);
        let words = kit::fit(game, &limit_words(e), r[2] - 86.0);
        let colour = if e.limit.is_some() { TEXT } else { LABEL };
        text(game, batch, &words, o, [r[0] + 84.0, r[1] + 1.0], colour);
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
            match v.typing.as_ref().filter(|t| t.field == Field::Limit) {
                Some(t) => {
                    let label = tr_with("rustcraft.terminal.limit", "Limit: %s", &[t.text.clone()]);
                    number_field(game, batch, o, term::LIMIT_VALUE, term::LIMIT_TEXT[0], &label, t);
                }
                None => {
                    kit::hover(game, batch, o, term::LIMIT_VALUE);
                    let s = limit_text(line.and_then(|l| l.limit));
                    text(game, batch, &s, o, term::LIMIT_TEXT, VALUE);
                }
            }
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

/// 0.21.2 (spec 49): a number field being typed into: a well with the hover outline's colour
/// round it, `label` from `x` (its digits last, the selected number highlighted) and the caret.
fn number_field(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    r: rustcraft_rules::machines::panel::Rect,
    x: f32,
    label: &str,
    t: &Typing,
) {
    let end = x + game.native_text_width(label);
    kit::fills(game, batch, o, &style::field(r, false, Some(end)));
    kit::fills(game, batch, o, &style::outline(r, HOT));
    if t.selected() {
        let digits = game.native_text_width(&t.text);
        game.native_fill(batch, [o[0] + end - digits, o[1] + r[1] + 2.0, digits, r[3] - 4.0], SELECTION);
    }
    text(game, batch, label, o, [x, r[1] + 2.0], VALUE);
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
    // Slice 18: «−», the count, «+», «Заказ».
    button(game, batch, o, term::ORDER_BUTTONS[term::ORDER_MINUS], "-", false);
    button(game, batch, o, term::ORDER_BUTTONS[term::ORDER_PLUS], "+", false);
    let go = tr("rustcraft.terminal.order_go", "Order");
    button(game, batch, o, term::ORDER_BUTTONS[term::ORDER_GO], &go, false);
    match v.typing.as_ref().filter(|t| t.field == Field::OrderCount) {
        Some(t) => {
            let r = term::ORDER_COUNT;
            let x = r[0] + ((r[2] - game.native_text_width(&t.text)) / 2.0).round().max(1.0);
            number_field(game, batch, o, r, x, &t.text, t);
        }
        None => {
            kit::hover(game, batch, o, term::ORDER_COUNT);
            let count = v.order_count.max(1).to_string();
            kit::centred(game, batch, o, term::ORDER_COUNT, &count, VALUE);
        }
    }
    match v.typing.as_ref().filter(|t| t.field == Field::Keep) {
        Some(t) => {
            let label = tr_with("rustcraft.terminal.keep", "Keep: %s", &[t.text.clone()]);
            number_field(game, batch, o, term::KEEP_VALUE, term::KEEP_TEXT[0], &label, t);
        }
        None => {
            if v.selected.is_some() {
                kit::hover(game, batch, o, term::KEEP_VALUE);
            }
            let colour = if v.selected.is_some() { VALUE } else { LABEL };
            text(game, batch, &keep_text(keep), o, term::KEEP_TEXT, colour);
        }
    }
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
    } else if !v.selected.zip(listing).is_none_or(|(key, l)| orderable(l, key)) {
        // OT10: an item no recipe makes takes no order and no «держать N»; a «держать» set
        // before 0.21 can still be lowered and cleared.
        for r in term::ORDER_BUTTONS.into_iter().chain([term::KEEP_PLUS]) {
            game.native_fill(batch, at(o, r), DIMMED);
        }
    }
}

/// OT10: the window can order `key`: the book makes it, or the list of what it makes was cut
/// (the server then answers).
fn orderable(l: &Listing, key: ItemRef) -> bool {
    l.orderable_cut || (key.1 == 0 && l.orderable.contains(&key.0))
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
        // Slice 18: «нет свободного автокрафтера», in each kind's own words.
        OrderWait::Busy(kind) => {
            let generic = tr_with("rustcraft.orders.wait.busy", "waits for a free %s", &[kind_name(*kind)]);
            KINDS
                .get(usize::from(*kind))
                .map_or(generic.clone(), |k| tr(&format!("rustcraft.orders.wait.busy.{k}"), &generic))
        }
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

/// Slice 18: a step of an order's chain in words, «печь: железный слиток».
fn step_text(game: &Game<'_>, step: &wire::OrderStep) -> String {
    format!("{}: {}", kind_name(step.kind), name_of(game, step.item))
}

/// Slice 18: an order's chain, «печь: слиток → [автокрафтер: поршень] → на склад», the running
/// step in brackets.
fn chain_text(game: &Game<'_>, order: &OrderLine) -> String {
    let mut parts: Vec<String> = order
        .steps
        .iter()
        .map(|s| {
            let t = step_text(game, s);
            if s.current { format!("[{t}]") } else { t }
        })
        .collect();
    parts.push(tr("rustcraft.orders.to_storage", "to the storage"));
    parts.join(" → ")
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
    // Wave 2 (ui.js `terminal`): the queue as a list, QUEUE_ROWS orders a page, each its chain of
    // steps with the running one in brackets, and its own cancel button.
    for (k, i) in term::queue_page(orders.len(), v.shown_order).into_iter().enumerate() {
        let order = &orders[i];
        let what = if order.steps.is_empty() || order.wait != OrderWait::Running {
            order_text(game, order)
        } else {
            format!(
                "{} {}/{}: {}",
                name_of(game, order.item),
                order.done,
                order.count,
                chain_text(game, order)
            )
        };
        let line = format!("{}/{} {what}", i + 1, orders.len());
        let (r, cancel) = term::queue_row(k);
        kit::hover(game, batch, o, r);
        let shown = kit::fit(game, &line, r[2] - 2.0);
        text(game, batch, &shown, o, [r[0], r[1] + 1.0], TEXT);
        button(game, batch, o, cancel, "x", false);
    }
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
            // Slice 18: «Склад полон».
            State::Works if l.total > 0 && l.used >= l.total => (
                Severity::Bad,
                tr("rustcraft.storage.full", "The storage is full"),
                tr_with(
                    "rustcraft.terminal.used",
                    "%s of %s slots used",
                    &[l.used.to_string(), l.total.to_string()],
                ),
            ),
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
            // Wave 2: a protrusion names the cells to take away.
            State::Extra => (
                Severity::Bad,
                tr("rustcraft.terminal.holes", "The storage is no solid box"),
                extra_line(l),
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

/// Wave 2: «Лишняя ячейка: 10 64 -3; 11 64 -3 и ещё 2».
fn extra_line(l: &Listing) -> String {
    let shown: Vec<String> = l.holes.iter().take(2).map(|h| format!("{} {} {}", h[0], h[1], h[2])).collect();
    let mut s = tr_with("rustcraft.terminal.extra_at", "A cell sticks out at %s", &[shown.join("; ")]);
    let rest = l.hole_count as usize - shown.len().min(l.hole_count as usize);
    if rest > 0 {
        s.push_str(&tr_with("rustcraft.terminal.holes_more", " and %s more", &[rest.to_string()]));
    }
    s
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
    let (orders_mode, network) = {
        let v = view_for(game);
        (v.orders, v.network)
    };
    // Round 32: the «Сеть» page has parts of its own.
    let part = if network {
        term::part_net(at)
    } else {
        part_in(at, orders_mode)
    };
    let listing = {
        let mut v = view_for(game);
        // 0.21.2 (spec 49): a click anywhere but on the field being typed into leaves it.
        let field = part.and_then(field_of);
        if v.typing.as_ref().map(|t| t.field) != field {
            v.typing = None;
        }
        listing_of(v.terminal)
    };
    let part = part?;
    let mut v = view_for(game);
    v.focused = part == Part::Search;
    let step = if right { 1 } else { 64 };
    // Slice 18: the «Лимиты» mode's rows stand where the grid does.
    if v.limits
        && let Some(row) = term::limit_row_at(at)
    {
        let list = listing
            .as_ref()
            .map(|l| view::limits_first(shown(game, l, &v)))
            .unwrap_or_default();
        if let Some(e) = limit_page(&list, v.page).get(row) {
            v.selected = Some(e.key);
        }
        return Some(NO_PRESS);
    }
    match part {
        Part::Tab(tab) => {
            v.tab = tab;
            v.page = 0;
        }
        Part::Search | Part::Status | Part::NetRow(_) => {}
        Part::LimitValue | Part::KeepValue | Part::OrderCount => {
            start_typing(&mut v, part, listing.as_ref());
        }
        // Round 32 (spec 33): the «Сеть» page instead of the grid, and its own pages.
        Part::NetMode => {
            v.network = !v.network;
            v.net_page = 0;
            v.limits &= !v.network;
            v.orders &= !v.network;
        }
        Part::Prev if v.network => v.net_page = v.net_page.saturating_sub(1),
        Part::Next if v.network => v.net_page += 1,
        Part::Prev => v.page = v.page.saturating_sub(1),
        Part::Next => v.page += 1,
        Part::LimitMode => {
            v.limits = !v.limits;
            v.orders &= !v.limits;
            v.network = false;
        }
        Part::OrderMode | Part::Socket(_) if tablet() => {}
        Part::OrderMode => {
            v.orders = !v.orders;
            v.limits &= !v.orders;
            v.network = false;
        }
        Part::Socket(k) => {
            return u8::try_from(k)
                .ok()
                .and_then(|k| term::encode(Press::Socket(k)))
                .or(Some(NO_PRESS));
        }
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
            let by = if game.native_has_shift_down() {
                64
            } else if right {
                8
            } else {
                1
            };
            let count = v.order_count.max(1);
            match k {
                term::ORDER_MINUS => v.order_count = term::step_count(count, -by),
                term::ORDER_PLUS => v.order_count = term::step_count(count, by),
                _ => {
                    let (item, _) = v.selected?;
                    return term::encode(Press::Order { item, count }).or(Some(NO_PRESS));
                }
            }
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
        // Wave 2: a click on a row shows the next page of the queue; × cancels that row's order.
        Part::Queue(_) => v.shown_order += term::QUEUE_ROWS,
        Part::Cancel(k) => {
            let orders = listing.as_ref().map_or(&[][..], |l| &l.orders[..]);
            let i = *term::queue_page(orders.len(), v.shown_order).get(k)?;
            let order = orders.get(i)?.id;
            return term::encode(Press::Cancel { order }).or(Some(NO_PRESS));
        }
    }
    Some(NO_PRESS)
}

/// 0.21.2 (spec 49): the number field a part is.
fn field_of(part: Part) -> Option<Field> {
    match part {
        Part::LimitValue => Some(Field::Limit),
        Part::KeepValue => Some(Field::Keep),
        Part::OrderCount => Some(Field::OrderCount),
        _ => None,
    }
}

/// A click on a number field opens it on its number, selected: the limit and «держать» only for
/// a chosen item (and «держать» only with the book), the order's count always.
fn start_typing(v: &mut View, part: Part, listing: Option<&Listing>) {
    let Some(field) = field_of(part) else {
        return;
    };
    if v.typing.as_ref().is_some_and(|t| t.field == field) {
        return;
    }
    let line = v
        .selected
        .and_then(|key| listing.and_then(|l| l.lines.iter().find(|x| (x.item, x.damage) == key)));
    let now = match field {
        Field::OrderCount => Some(u64::from(v.order_count.max(1))),
        _ if v.selected.is_none() => return,
        Field::Limit => line.and_then(|l| l.limit).map(|l| l.max),
        Field::Keep if !listing.is_some_and(|l| l.book) => return,
        Field::Keep => line.map(|l| l.keep).filter(|n| *n > 0),
    };
    v.focused = false;
    v.typing = Some(Typing::start(field, now));
}

/// Enter in a number field: the order's count changes in the window; a limit or an N goes to the
/// server. An empty field changes nothing.
fn apply_typing(v: &mut View) {
    let Some(t) = v.typing.take() else {
        return;
    };
    let Some(value) = t.value() else {
        return;
    };
    let (setting, item) = match (t.field, v.selected) {
        (Field::OrderCount, _) => {
            v.order_count = u16::try_from(value).unwrap_or(term::MAX_ORDER);
            return;
        }
        (_, None) => return,
        (Field::Limit, Some(key)) => (Setting::Limit, key),
        (Field::Keep, Some((item, _))) => (Setting::Keep, (item, 0)),
    };
    let set = typed::Set { setting, item, value };
    if tablet() {
        tablet_window::queue(rustcraft_rules::storage::tablet::Press::Set(set));
    } else if let Some(terminal) = v.terminal {
        TYPED.lock().unwrap_or_else(PoisonError::into_inner).push((terminal, set));
    }
}

/// Numbers typed into the terminal's window since the client's last tick (a key hook has no
/// `&mut Game` to send with).
static TYPED: Mutex<Vec<(Pos, typed::Set)>> = Mutex::new(Vec::new());

/// The client's tick: the typed numbers to the server; in single player straight into its
/// queue, the local player being the server's own.
pub(super) fn client_tick(game: &mut Game<'_>) {
    let due = std::mem::take(&mut *TYPED.lock().unwrap_or_else(PoisonError::into_inner));
    for (terminal, set) in due {
        if game.remote.is_none() {
            storage::queue_typed(game.native_acting_id(), terminal, set);
            continue;
        }
        let bytes = typed::encode_terminal(terminal, set);
        if let Err(e) = game.native_net_send_to_server(typed::CHANNEL, &bytes) {
            log::debug!("rustcraft: a typed number not sent: {e}");
        }
    }
}

/// Whether the open screen is a terminal's.
fn is_terminal(screen: &NativeScreenView<'_>) -> bool {
    if tablet() {
        return true;
    }
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
    // 0.21.2 (spec 49): a number field takes every key, as the search does.
    if v.typing.is_some() {
        *canceled = true;
        if key == ESCAPE {
            v.typing = None;
        } else if ENTER.contains(&key) {
            apply_typing(&mut v);
        } else if key == BACKSPACE
            && let Some(t) = v.typing.as_mut()
        {
            t.backspace();
        }
        return;
    }
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
    // 0.21.2 (spec 49): digits into the number field; anything else goes nowhere.
    if let Some(t) = v.typing.as_mut() {
        *canceled = true;
        t.type_char(c);
        return;
    }
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
    let (orders_mode, network, net_page, terminal) = {
        let v = view_for(game);
        (v.orders, v.network, v.net_page, v.terminal)
    };
    let part = if network {
        term::part_net(at)
    } else {
        part_in(at, orders_mode)
    };
    let Some(part) = part else {
        return;
    };
    match part {
        Part::NetMode => hint(
            lines,
            "rustcraft.netpage.button_hint",
            "What the storage sees on its magistral: housings, machines, tanks",
        ),
        Part::NetRow(i) => {
            let page = terminal_net::page_for(tablet(), terminal);
            terminal_net::row_tooltip(page.as_ref(), net_page, i, lines);
        }
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
        Part::LimitValue | Part::KeepValue | Part::OrderCount => {
            if part == Part::KeepValue {
                hint(
                    lines,
                    "rustcraft.terminal.keep_hint",
                    "Keep N: below N the storage orders the difference by itself",
                );
            }
            hint(
                lines,
                "rustcraft.terminal.type_hint",
                "Click and type a number · Enter: set · Esc: leave",
            );
        }
        Part::LimitDestroy => hint(
            lines,
            "rustcraft.terminal.destroy_hint",
            "Destroy the surplus: what comes over the limit vanishes instead of staying in the line",
        ),
        Part::Socket(k) => {
            let v = view_for(game);
            let listing = listing_of(v.terminal);
            drop(v);
            if let Some(item) = socket_item(game, listing.as_ref(), k) {
                lines.push(item_name(item));
            }
            let (key, fallback) = match k {
                sockets::SOCKET_STACK => (
                    "rustcraft.terminal.stack_hint",
                    "The stack module's socket: a slot of the storage holds 256, 1024 or 4096",
                ),
                sockets::SOCKET_FILTER => (
                    "rustcraft.terminal.filter_hint",
                    "The filter module's socket: the «Own» tab shows what it marks",
                ),
                _ => (
                    "rustcraft.terminal.book_hint",
                    "The recipe book's socket: with it the storage takes orders",
                ),
            };
            hint(lines, key, fallback);
        }
        Part::Order(k) => {
            if k == term::ORDER_GO {
                let n = view_for(game).order_count.max(1);
                lines.push(tr_with("rustcraft.terminal.order_hint", "Order %s", &[n.to_string()]));
            } else {
                hint(
                    lines,
                    "rustcraft.terminal.count_hint",
                    "Left click: by 1 · right click: by 8 · shift: by 64",
                );
            }
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
        Part::Queue(_) => queue_tooltip(game, lines),
        Part::Cancel(_) => hint(lines, "rustcraft.terminal.cancel_hint", "Cancel the order"),
        Part::Status => {
            let v = view_for(game);
            if let Some(l) = listing_of(v.terminal).filter(|l| matches!(l.state, State::Holes | State::Extra)) {
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
    if let Some(stack) = variant(e.key) {
        lines.extend(variant_lines(&stack));
    }
    lines.push(tr_with(
        "rustcraft.terminal.count",
        "In storage: %s",
        &[e.count.to_string()],
    ));
    let book = listing.as_ref().filter(|l| l.book);
    let can = book.is_some_and(|l| orderable(l, e.key));
    if e.count == 0 && can {
        hint(lines, "rustcraft.terminal.orderable", "Can be ordered");
    }
    // OT10: «не делается на машинах: его только добывают».
    if book.is_some() && !can && e.key.1 == 0 {
        hint(lines, "rustcraft.terminal.not_made", "Not made on machines: only gathered");
    }
    if e.key.1 > 0 && e.key.1 & term::VARIANT == 0 {
        lines.push(tr_with(
            "rustcraft.terminal.worn",
            "Worn: %s",
            &[e.key.1.to_string()],
        ));
    }
    if e.limit.is_some() {
        lines.push(limit_words(&e));
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
        if !order.steps.is_empty() {
            lines.push(format!("   {}", chain_text(game, order)));
        }
    }
    hint(
        lines,
        "rustcraft.terminal.queue_hint",
        "Click: the next order · ×: cancel it",
    );
}
