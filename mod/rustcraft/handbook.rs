// The engineer's handbook (spec H2, H5, H6; guide `guide`; rustcraft_rules::handbook decides).
//
// On the server:
// - Once a second, for each player of the level who has not had it: carrying any alloy
//   (`native_player_stacks`, API 10, native_menus.rs:225-239) gives them the handbook
//   (`native_give_stack`, API 98, native_server.rs:1101-1110) and a line in their chat. A full
//   inventory tries again the next second. Who has had it is kept in the world's file
//   `data/rustcraft_handbook.json` (`native_world_dir`, native_server.rs:896-899), as the gear
//   keeps its store. The trigger is the alloy in the inventory, not the craft: coppersteel comes
//   from the crafting table and the others from the alloy smelter, by hand or through a
//   magistral, and every way ends with an alloy in a player's hands.
// - A right click with the handbook (`native_use_item`) sends the player's client a mod message
//   (`native_net_send_to_player`, API 40, native_net.rs:170-191).
//
// On the client:
// - The message (`native_net_from_server`) asks for the window; the window's next tick opens the
//   mod's own screen (`native_mod_screen_open`, API 111, native_hooks.rs:11024-11057). It is the
//   client's alone: the server keeps no menu, and nothing on it waits for the server.
// - `native_screen_render_post` (API 78, native_hooks.rs:3779-3803) draws it over the world in
//   the «Кузня душ» style (docs/design/04-window-style.md): the contents, a section's pages, a
//   page with its item's icon (`native_gui_draw_item_with`, native_client_services.rs:529-539),
//   its texts from the content mod's lang files, and the item's crafting recipe as a 3 × 3 grid
//   read from the game's own recipes (`Game::recipes`, world_entry.rs:175-180).
// - A left click (`native_screen_mouse_button_pressed_pre`, API 80, native_hooks.rs:3804-3838),
//   the wheel (`native_screen_mouse_scrolled_pre`, API 93, native_hooks.rs:3893-3933) and the
//   arrow keys (`native_screen_key_pressed_pre`, API 104) turn its sheets; Escape closes it.
// - A page whose guide entry has an animated scene (H3; rustcraft_rules::handbook::scene) has
//   one sheet more, its last: the scene's step as block item icons in the game's slot view of a
//   block (`native_item`, API 20, native_hooks.rs:10951-10965, which scales the icon with
//   `HudBatch::scale_since`, horizon-client-render hud.rs:1889-1894), water, lava and the ghost as
//   tinted cubes (`native_quad`, API 111, native_hooks.rs:11020-11023), flows and beams as dots
//   (`native_fill`, native_hooks.rs:10842-10846), everything cut to the panel
//   (`HudBatch::clip_since`, hud.rs:1895). The scene moves by the window's own clock and goes on
//   to its next step by itself; the step buttons in the bottom row step it by hand. A block
//   under the mouse names its item (`native_gui_tooltip`, native_client_services.rs:472-474).
use crate::game::{
    Game,
    rustcraft_screens::{
        self as kit, FONT_HEIGHT, LABEL, TEXT, TEXT_ON, VALUE, button, slot, text, tr, tr_with,
    },
    rustcraft_server::line,
};
use horizon_client::{
    crafting::{Ingredient, Recipe},
    hud::HudBatch,
    items::ItemStack,
    mod_api::NativeScreenView,
    text::Text,
};
use rustcraft_rules::{
    handbook::{
        self as rules, Given, PAGES, Part, Piece, Role, Row, View,
        scene::{self, Look, Playback, Scene},
    },
    machines::style::{self, colour},
};
use std::{
    collections::HashMap,
    ops::Range,
    path::PathBuf,
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

/// The mod's own screen's owner (`native_mod_screen`).
const MOD_ID: &str = "rustcraft";
const LEFT: i32 = 0;
const FORWARD_KEYS: [&str; 2] = ["key.keyboard.right", "key.keyboard.page.down"];
const BACK_KEYS: [&str; 2] = ["key.keyboard.left", "key.keyboard.page.up"];
const CONTENTS_KEY: &str = "key.keyboard.backspace";

// ---- The server ----

#[derive(Default)]
struct Server {
    path: Option<PathBuf>,
    given: Given,
    dirty: bool,
}

static SERVER: LazyLock<Mutex<Server>> = LazyLock::new(Mutex::default);

fn server() -> MutexGuard<'static, Server> {
    SERVER.lock().unwrap_or_else(PoisonError::into_inner)
}

fn load(s: &mut Server, game: &Game<'_>) {
    if s.path.is_some() {
        return;
    }
    let Some(world) = game.native_world_dir() else {
        return;
    };
    let path = world.join("data").join("rustcraft_handbook.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        match serde_json::from_str(&text) {
            Ok(given) => s.given = given,
            Err(e) => log::warn!("rustcraft: {} does not read: {e}", path.display()),
        }
    }
    s.path = Some(path);
}

fn save(s: &mut Server) {
    if !s.dirty {
        return;
    }
    let Some(path) = s.path.clone() else { return };
    let Ok(json) = serde_json::to_string(&s.given) else {
        return;
    };
    s.dirty = false;
    let fresh = path.with_extension("json.tmp");
    let done = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&fresh, json))
        .and_then(|()| std::fs::rename(&fresh, &path));
    if let Err(e) = done {
        log::warn!("rustcraft: who had the handbook was not saved: {e}");
    }
}

/// Notes that `player` has had the handbook and saves it.
fn mark(game: &Game<'_>, player: u128) {
    let mut s = server();
    load(&mut s, game);
    s.dirty |= s.given.add(player);
    save(&mut s);
}

/// `native_use_item`: a right click with the handbook opens its window on the player's client.
pub(super) fn on_use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed || &*stack.item != rules::ITEM {
        return;
    }
    let Some(player) = game.native_acting_player() else {
        return;
    };
    *claimed = true;
    if let Err(e) = game.native_net_send_to_player(player.uuid, rules::CHANNEL, &[1]) {
        log::warn!("rustcraft: the handbook's window was not asked for: {e}");
    }
}

/// Once a second for a level (rustcraft_server::on_level_tick): its players who carry an alloy
/// and have not had the handbook get it (H2).
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    let players: Vec<u128> = game
        .native_server_players()
        .into_iter()
        .filter(|p| p.dimension == dimension)
        .map(|p| p.uuid)
        .collect();
    if players.is_empty() {
        return;
    }
    let waiting: Vec<u128> = {
        let mut s = server();
        load(&mut s, game);
        players.into_iter().filter(|u| !s.given.has(*u)).collect()
    };
    for uuid in waiting {
        let Some(stacks) = game.native_player_stacks(uuid) else {
            continue;
        };
        if !rules::due(false, stacks.iter().flatten().map(|s| &*s.item)) {
            continue;
        }
        // What did not fit comes back: no room yet, so the next second tries again.
        if game
            .native_give_stack(uuid, ItemStack::new(rules::ITEM, 1))
            .is_some()
        {
            continue;
        }
        mark(game, uuid);
        let text = line(
            &rules::key("given"),
            "You have the Engineer's Handbook: a right click opens it",
            &[],
        );
        game.native_tell(uuid, text);
    }
}

/// The self-test's way in: what the grant decides for `player` carrying `carried`, noted as
/// `tick` notes it (no stack is given: the self-test's players are pretend ones).
pub(super) fn offer(game: &Game<'_>, player: u128, carried: &[&str]) -> bool {
    let had = {
        let mut s = server();
        load(&mut s, game);
        s.given.has(player)
    };
    let due = rules::due(had, carried.iter().copied());
    if due {
        mark(game, player);
    }
    due
}

/// The self-test's pretend player forgotten again.
pub(super) fn forget(game: &Game<'_>, player: u128) {
    let mut s = server();
    load(&mut s, game);
    s.dirty |= s.given.remove(player);
    save(&mut s);
}

// ---- The recipe ----

type Grid = [Option<String>; 9];

/// What an ingredient shows: its first item in the game's order.
fn shown(i: &Ingredient) -> Option<String> {
    i.order
        .first()
        .cloned()
        .or_else(|| i.items.iter().min().cloned())
}

/// The first crafting recipe of `item` in the game's recipes: its grid and how many it makes.
pub(super) fn recipe_of(game: &Game<'_>, item: &str) -> Option<(Grid, u32)> {
    for (_, recipe) in &game.recipes().crafting {
        let (width, cells, result): (usize, Vec<Option<String>>, _) = match recipe {
            Recipe::Shaped {
                width,
                cells,
                result,
                ..
            } => (
                *width,
                cells.iter().map(|c| c.as_ref().and_then(shown)).collect(),
                result,
            ),
            Recipe::Shapeless {
                ingredients,
                result,
            } => (3, ingredients.iter().map(shown).collect(), result),
            Recipe::Special(_) => continue,
        };
        if result.item.as_deref() != Some(item) {
            continue;
        }
        if let Some(grid) = rules::recipe_grid(width, &cells) {
            return Some((grid, result.count.max(1)));
        }
    }
    None
}

// ---- The client: the window ----

/// The window asked for by the server, opened at the window's next tick.
static ASKED: AtomicBool = AtomicBool::new(false);
/// Where the window is: kept while it is closed, so it opens again where it was.
static VIEW: LazyLock<Mutex<View>> = LazyLock::new(Mutex::default);
/// The recipes the window has looked up since it opened, by item.
static RECIPES: LazyLock<Mutex<HashMap<&'static str, Option<(Grid, u32)>>>> =
    LazyLock::new(Mutex::default);

/// The scene on screen: its page, step and since when (`clock`); None off a scene's sheet.
static PLAYING: LazyLock<Mutex<Option<Playback>>> = LazyLock::new(Mutex::default);
/// The window's clock, which moves the scenes.
static STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);

fn clock() -> f64 {
    STARTED.elapsed().as_secs_f64()
}

fn playing() -> MutexGuard<'static, Option<Playback>> {
    PLAYING.lock().unwrap_or_else(PoisonError::into_inner)
}

fn view() -> MutexGuard<'static, View> {
    VIEW.lock().unwrap_or_else(PoisonError::into_inner)
}

fn recipes() -> MutexGuard<'static, HashMap<&'static str, Option<(Grid, u32)>>> {
    RECIPES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_net_from_server` on a client: the server asks for the window.
pub(super) fn on_net(channel: &str, _: &[u8]) {
    if channel == rules::CHANNEL {
        ASKED.store(true, Ordering::Relaxed);
    }
}

/// The window's tick: the window asked for opens.
pub(super) fn client_tick(game: &mut Game<'_>) {
    if !ASKED.swap(false, Ordering::Relaxed) {
        return;
    }
    recipes().clear();
    if !game.native_mod_screen_open(MOD_ID) {
        log::debug!("rustcraft: the handbook did not open over the screen that is up");
    }
}

fn is_open(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(MOD_ID)
}

fn corner(screen: &NativeScreenView<'_>) -> [f32; 2] {
    let o = rules::origin([screen.rect[2], screen.rect[3]]);
    [screen.rect[0] + o[0], screen.rect[1] + o[1]]
}

fn lookup(key: &str) -> Option<String> {
    horizon_client::lang::has(key).then(|| horizon_client::lang::get_or(key, ""))
}

/// A page's recipe, looked up once while the window is open.
fn recipe_for(game: &Game<'_>, item: &'static str) -> Option<(Grid, u32)> {
    if let Some(r) = recipes().get(item) {
        return r.clone();
    }
    let r = recipe_of(game, item);
    recipes().insert(item, r.clone());
    r
}

/// A page laid out in the reader's language: its rows, its sheets and its recipe.
struct Laid {
    rows: Vec<Row>,
    sheets: Vec<Range<usize>>,
    /// The page's item and its recipe.
    item: Option<&'static str>,
    recipe: Option<(Grid, u32)>,
}

fn laid(game: &Game<'_>, page: usize) -> Laid {
    let p = &PAGES[page];
    let recipe = p.item.and_then(|item| recipe_for(game, item));
    let pieces: Vec<Piece> = rules::pieces(p, recipe.is_some(), &lookup);
    let measure = |s: &str| game.native_text_width(s);
    let rows = rules::lay_out(&pieces, rules::BODY_WIDTH, &measure);
    let sheets = rules::sheets(&rows);
    Laid {
        rows,
        sheets,
        item: p.item,
        recipe,
    }
}

fn sheets_of(game: &Game<'_>, v: View) -> usize {
    match v {
        View::Page { page, .. } if page < PAGES.len() => {
            rules::sheet_count(&PAGES[page], laid(game, page).sheets.len())
        }
        _ => 1,
    }
}

/// The scene the view shows, when it is on a page's scene sheet.
fn scene_shown(game: &Game<'_>, v: View) -> Option<&'static Scene> {
    let View::Page { page, .. } = v else {
        return None;
    };
    let p = PAGES.get(page)?;
    let text = laid(game, page).sheets.len();
    let View::Page { sheet, .. } = v.clamped(rules::sheet_count(p, text)) else {
        return None;
    };
    (rules::scene_sheet(p, text)? == sheet)
        .then(|| rules::scene_of(p))
        .flatten()
}

/// `native_screen_render_post`: the window, while it is the open screen.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !is_open(game) {
        return;
    }
    let o = corner(screen);
    let v = *view();
    kit::background(game, batch, o, [rules::WIDTH, rules::HEIGHT]);
    let title = tr(&rules::key("title"), "Engineer's Handbook");
    match v {
        View::Contents { .. } => {
            let View::Contents { section, sheet } = v.clamped(1) else {
                return;
            };
            let contents = tr(&rules::key("contents"), "Contents");
            kit::title_line(
                game,
                batch,
                o,
                &title,
                Some(contents.as_str()),
                rules::WIDTH,
            );
            *playing() = None;
            contents_part(game, batch, o, section, sheet);
            bottom(
                game,
                batch,
                o,
                true,
                false,
                sheet,
                rules::contents_sheets(section),
            );
        }
        View::Page { page, .. } => {
            let page = page.min(PAGES.len() - 1);
            let laid = laid(game, page);
            let p = &PAGES[page];
            let sheets = rules::sheet_count(p, laid.sheets.len());
            let View::Page { page, sheet } = v.clamped(sheets) else {
                return;
            };
            let on_scene = rules::scene_sheet(p, laid.sheets.len()) == Some(sheet);
            match rules::scene_of(p).filter(|_| on_scene) {
                Some(s) => {
                    let name = tr(&p.key("name"), p.id);
                    let word = tr(&rules::key("scene"), "Scene");
                    // The title's « · » and its rest fit what the title leaves of the line.
                    let measure = |x: &str| game.native_text_width(x);
                    let room = rules::WIDTH - 2.0 * rules::TITLE[0] - measure(&title);
                    let rest = format!("{name} · {word}");
                    let rest = rules::clip(&rest, room - measure(" · "), &measure);
                    kit::title_line(game, batch, o, &title, Some(rest.as_str()), rules::WIDTH);
                    scene_part(game, batch, o, page, s);
                    bottom(game, batch, o, false, true, sheet, sheets);
                }
                None => {
                    *playing() = None;
                    let section = tr(&rules::section_key(p.section), p.id);
                    kit::title_line(game, batch, o, &title, Some(section.as_str()), rules::WIDTH);
                    if sheet == 0 {
                        head(game, batch, o, page);
                    }
                    body(game, batch, o, &laid, sheet);
                    bottom(game, batch, o, false, false, sheet, sheets);
                }
            }
        }
    }
}

/// A page's scene sheet (H3): the step in the panel, its caption, the step buttons.
fn scene_part(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], page: usize, s: &Scene) {
    let now = clock();
    let (step, t) = {
        let mut play = playing();
        let p = play.get_or_insert_with(|| Playback::start(page, now));
        if p.page != page {
            *p = Playback::start(page, now);
        }
        p.update(s, now);
        (p.step.min(s.steps.len() - 1), p.time(now))
    };
    let st = &s.steps[step];
    let panel = scene::PANEL;
    kit::fills(game, batch, o, &style::well(panel));
    let at = [o[0] + panel[0], o[1] + panel[1]];
    let fit = scene::fit(s, [panel[2], panel[3]]);
    let start = batch.vertices.len();
    let drawn = scene::drawn(st, t, &fit);
    for d in &drawn {
        look(game, batch, at, d, &fit);
    }
    if st.tint != 0 {
        game.native_fill(batch, [at[0], at[1], panel[2], panel[3]], st.tint);
    }
    for dot in scene::dots(st, t, &fit) {
        let side = scene::dot_side(&dot, fit.scale);
        let c = [at[0] + dot.centre[0], at[1] + dot.centre[1]];
        if !dot.dash {
            let rim = side + 2.0;
            let r = [c[0] - rim / 2.0, c[1] - rim / 2.0, rim, rim];
            game.native_fill(batch, r, colour::OUTLINE);
        }
        let r = [c[0] - side / 2.0, c[1] - side / 2.0, side, side];
        game.native_fill(batch, r, 0xFF00_0000 | dot.rgb);
    }
    batch.clip_since(start, at[0], at[1], at[0] + panel[2], at[1] + panel[3]);
    let measure = |x: &str| game.native_text_width(x);
    if let Some(caption) = lookup(&s.caption_key(step)) {
        let lines = scene::caption_lines(&caption, rules::BODY_WIDTH, &measure);
        for (i, line) in lines.iter().enumerate() {
            let y = scene::CAPTION_TOP + rules::LINE * i as f32;
            text(game, batch, line, o, [rules::BODY_X, y + 1.0], TEXT);
        }
    }
    button(game, batch, o, scene::STEP_BACK, "◀", false);
    button(game, batch, o, scene::STEP_ON, "▶", false);
    let n = tr_with(
        &rules::key("step"),
        "Step %s of %s",
        &[(step + 1).to_string(), s.steps.len().to_string()],
    );
    kit::centred(game, batch, o, scene::STEP_COUNT, &n, LABEL);
    if kit::hovered(game, o, panel) {
        let cursor = game.native_cursor();
        let over = [cursor[0] - at[0], cursor[1] - at[1]];
        if let Some(Look::Item(id)) = scene::pick(&drawn, &fit, over).map(|i| drawn[i].look) {
            game.native_gui_tooltip(batch, &[item_name(game, id)], cursor);
        }
    }
}

/// One block of a scene: its item's icon at the scene's scale, or a tinted cube.
fn look(game: &Game<'_>, batch: &mut HudBatch, at: [f32; 2], d: &scene::Drawn, fit: &scene::Fit) {
    let c = [at[0] + d.centre[0], at[1] + d.centre[1]];
    match d.look {
        Look::Item(id) => {
            let side = fit.icon();
            let corner = d.corner(side);
            game.native_item(batch, id, [at[0] + corner[0], at[1] + corner[1]], side);
        }
        Look::Tint(argb) => {
            for (corners, light) in scene::cube_faces(c, fit.scale) {
                game.native_quad(batch, corners, scene::shaded(argb, light));
            }
        }
    }
}

/// An item's name, as a tooltip names it.
fn item_name(game: &Game<'_>, it: &str) -> Text {
    let fallback = it.rsplit(':').next().unwrap_or(it);
    let kind = if game.items().get(it).is_some_and(|i| i.block.is_some()) {
        "block"
    } else {
        "item"
    };
    Text::translate_with_fallback(&format!("{kind}.{}", it.replace(':', ".")), fallback, &[])
}

/// The contents: the sections on the left, the chosen one raised; its pages on the right.
fn contents_part(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], section: usize, sheet: usize) {
    for i in 0..rules::SECTIONS.len() {
        let r = rules::section_rect(i);
        let name = tr(&rules::section_key(i), rules::SECTIONS[i]);
        let measure = |s: &str| game.native_text_width(s);
        let name = rules::clip(&name, r[2] - 8.0, &measure);
        if i == section {
            kit::fills(game, batch, o, &style::raised(r));
            text(game, batch, &name, o, [r[0] + 4.0, r[1] + 2.0], TEXT_ON);
        } else {
            let hot = kit::hovered(game, o, r);
            text(
                game,
                batch,
                &name,
                o,
                [r[0] + 4.0, r[1] + 2.0],
                if hot { VALUE } else { TEXT },
            );
            kit::hover(game, batch, o, r);
        }
    }
    for (row, page) in rules::listed(section, sheet).into_iter().enumerate() {
        let p = &PAGES[page];
        let r = rules::entry_rect(row);
        let icon = rules::entry_icon(row);
        slot(game, batch, o, icon);
        item(game, batch, o, icon, p.icon(), 1);
        let name = tr(&p.key("name"), p.id);
        let measure = |s: &str| game.native_text_width(s);
        let name = rules::clip(&name, r[2] - 24.0, &measure);
        let hot = kit::hovered(game, o, r);
        let y = r[1] + ((r[3] - FONT_HEIGHT) / 2.0).round();
        text(
            game,
            batch,
            &name,
            o,
            [r[0] + 21.0, y],
            if hot { VALUE } else { TEXT },
        );
        kit::hover(game, batch, o, r);
    }
}

/// A page's head on its first sheet: the item's icon in a slot, the name, the tier.
fn head(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], page: usize) {
    let p = &PAGES[page];
    slot(game, batch, o, rules::ICON);
    item(game, batch, o, rules::ICON, p.icon(), 1);
    let name = tr(&p.key("name"), p.id);
    text(game, batch, &name, o, rules::NAME, VALUE);
    if p.tier > 0 {
        let tier = tr_with(
            &rules::key(&format!("tier.{}", p.tier)),
            "Tier %s",
            &[p.tier.to_string()],
        );
        text(game, batch, &tier, o, rules::UNDER_NAME, LABEL);
    }
}

/// The rows of a page's sheet.
fn body(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], laid: &Laid, sheet: usize) {
    let Some(range) = laid.sheets.get(sheet) else {
        return;
    };
    let mut y = rules::sheet_top(sheet);
    for row in &laid.rows[range.clone()] {
        let x = rules::BODY_X;
        match row {
            Row::Text {
                x: dx,
                text: t,
                role,
            } => {
                text(game, batch, t, o, [x + dx, y + 1.0], role_colour(*role));
            }
            Row::Pair { key, value } => {
                text(game, batch, key, o, [x, y + 1.0], LABEL);
                kit::text_right(game, batch, value, o, x + rules::BODY_WIDTH, y + 1.0, VALUE);
            }
            Row::Number { n, text: t, role } => {
                let square = [x, y, rules::NUMBER[0], rules::NUMBER[1]];
                kit::fills(game, batch, o, &style::raised(square));
                kit::centred(game, batch, o, square, &n.to_string(), TEXT_ON);
                text(
                    game,
                    batch,
                    t,
                    o,
                    [x + rules::INDENT, y + 1.0],
                    role_colour(*role),
                );
            }
            Row::Recipe => recipe(game, batch, o, y, laid),
            Row::Gap => {}
        }
        y += row.height();
    }
}

fn role_colour(role: Role) -> u32 {
    match role {
        Role::Lead | Role::Value => VALUE,
        Role::Text => TEXT,
        Role::Heading | Role::Key => LABEL,
        Role::Bad => colour::BAD_TEXT,
    }
}

/// The recipe's block at `y`: the grid, the arrow and what it makes; a cell under the mouse
/// names its item.
fn recipe(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], y: f32, laid: &Laid) {
    let (Some((grid, count)), Some(made)) = (laid.recipe.as_ref(), laid.item) else {
        return;
    };
    let mut named: Option<&str> = None;
    for (i, cell) in grid.iter().enumerate() {
        let p = rules::recipe_cell(y, i);
        slot(game, batch, o, p);
        if let Some(it) = cell {
            item(game, batch, o, p, it, 1);
            if kit::hovered(game, o, style::slot_rect(p)) {
                named = Some(it.as_str());
            }
        }
    }
    let a = rules::RECIPE_ARROW;
    kit::arrow(
        game,
        batch,
        o,
        [rules::BODY_X + a[0], y + a[1], a[2], a[3]],
        1.0,
    );
    let res = [
        rules::BODY_X + rules::RECIPE_RESULT[0],
        y + rules::RECIPE_RESULT[1],
    ];
    slot(game, batch, o, res);
    item(game, batch, o, res, made, *count);
    if let Some(it) = named {
        game.native_gui_tooltip(batch, &[item_name(game, it)], game.native_cursor());
    }
}

fn item(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], p: [f32; 2], id: &str, count: u32) {
    let stack = ItemStack::new(id, count.max(1));
    let shown = (count > 1).then(|| count.to_string());
    game.native_gui_draw_item_with(batch, &stack, [o[0] + p[0], o[1] + p[1]], shown.as_deref());
}

/// The bottom row: back, the contents, the sheet's number (left of the step buttons on a
/// scene's sheet), on.
fn bottom(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    contents: bool,
    on_scene: bool,
    sheet: usize,
    sheets: usize,
) {
    button(game, batch, o, rules::PREV, "<", false);
    button(game, batch, o, rules::NEXT, ">", false);
    let name = tr(&rules::key("contents"), "Contents");
    button(game, batch, o, rules::CONTENTS, &name, contents);
    let n = tr_with(
        &rules::key("sheet"),
        "%s / %s",
        &[(sheet + 1).to_string(), sheets.max(1).to_string()],
    );
    let place = if on_scene { scene::SHEET } else { rules::SHEET };
    kit::centred(game, batch, o, place, &n, LABEL);
}

/// `native_screen_mouse_button_pressed_pre`: a left click on the window's parts.
pub(super) fn on_mouse(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if *canceled || !is_open(game) {
        return;
    }
    *canceled = true;
    if button != LEFT {
        return;
    }
    let o = corner(screen);
    let at = [screen.mouse[0] - o[0], screen.mouse[1] - o[1]];
    let v = *view();
    if let Some(s) = scene_shown(game, v)
        && let Some(part) = scene::part_at(at)
    {
        if let Some(p) = playing().as_mut() {
            p.go(s, part == Part::StepOn, clock());
        }
        return;
    }
    let Some(part) = rules::part_at(v, at) else {
        return;
    };
    let next = rules::press(v, part, sheets_of(game, v));
    *view() = next;
}

/// `native_screen_mouse_scrolled_pre`: the wheel turns the sheets.
pub(super) fn on_scroll(game: &Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if *canceled || !is_open(game) {
        return;
    }
    *canceled = true;
    if scroll[1] != 0.0 {
        turn(game, scroll[1] < 0.0);
    }
}

/// `native_screen_key_pressed_pre`: the arrows and Page Up/Down turn the sheets, Backspace goes
/// to the contents; every other key (Escape among them) goes on.
pub(super) fn on_key(game: &Game<'_>, key: &str, canceled: &mut bool) {
    if *canceled || !is_open(game) {
        return;
    }
    if FORWARD_KEYS.contains(&key) || BACK_KEYS.contains(&key) {
        *canceled = true;
        turn(game, FORWARD_KEYS.contains(&key));
    } else if key == CONTENTS_KEY {
        *canceled = true;
        let v = *view();
        *view() = rules::press(v, Part::Contents, sheets_of(game, v));
    }
}

fn turn(game: &Game<'_>, forward: bool) {
    let v = *view();
    *view() = rules::turn(v, forward, sheets_of(game, v));
}
