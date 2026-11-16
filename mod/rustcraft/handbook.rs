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
//   its texts from the content mod's lang files, and its recipes read from the game's own
//   (rustcraft_rules::handbook::recipes picks and lays them out): the crafting table's, shaped,
//   shapeless and transmuting (`Game::recipes`, world_entry.rs:175-180; `Special::Transmute`,
//   horizon-client crafting.rs:426-433), smelting (`Recipes::cooking`) and the machines' own types
//   (`native_recipes_of_type`, API 36, native_hooks.rs:11351-11356: a client sees the content
//   mod's recipes, which is where the mod's are), two to a row, a machine's over its arrow.
//   A table of the guide's (the turret's) is drawn in columns, and «см. также» is a row of page
//   icons and names: a click opens that page. A block's window is drawn as a picture before its
//   numbered legend (rustcraft_rules::handbook::window): each part's number, label and widget
//   (slots, bars, the mode switch, sockets, the status line, rows, chips) in the windows' style.
// - A left click (`native_screen_mouse_button_pressed_pre`, API 80, native_hooks.rs:3804-3838),
//   the wheel (`native_screen_mouse_scrolled_pre`, API 93, native_hooks.rs:3893-3933) and the
//   arrow keys (`native_screen_key_pressed_pre`, API 104) turn its sheets; Escape closes it.
// - A page whose guide entry has an animated scene (H3; rustcraft_rules::handbook::scene) has
//   one sheet more, its last: the scene's step as block item icons in the game's slot view of a
//   block (`native_item`, API 20, native_hooks.rs:10951-10965, which scales the icon with
//   `HudBatch::scale_since`, horizon-client-render hud.rs:1889-1894), water, lava and the ghost as
//   tinted cubes (`native_quad`, API 111, native_hooks.rs:11020-11023), flows and beams as dots
//   (`native_fill`, native_hooks.rs:10842-10846), everything cut to the panel
//   (`HudBatch::clip_since`, hud.rs:1895). The guide's badges and name tags are small plates of
//   text above their points (the text scaled down with `HudBatch::scale_since`), a dropped item a
//   little cube of its colour. A block with moving parts is drawn with a still of its model and
//   parts in the step's state (`native_item_model`, API 111, native_hooks.rs:10992-11016; the
//   stills are item models of the content mod's, assets/models/poses_mc.py). The scene moves by the window's own clock and goes on
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
    crafting::{Ingredient, Recipe, Special, Template},
    hud::HudBatch,
    items::ItemStack,
    mod_api::NativeScreenView,
    text::Text,
};
use rustcraft_rules::{
    handbook::{
        self as rules, Given, PAGES, Part, Piece, Role, Row, View,
        recipes::{self, Made, Out, Shown},
        scene::{self, Look, Playback, Scene},
        window::{self, Cell, Widget},
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

/// Every recipe of the game as a page shows it: the crafting table's (shaped, shapeless,
/// transmuting), smelting and the machines' own types.
fn all_recipes(game: &Game<'_>) -> Vec<Shown> {
    let mut out = Vec::new();
    let book = game.recipes();
    let grid = |width: usize, cells: Vec<Option<String>>, result: &Template| {
        rules::recipe_grid(width, &cells)
            .zip(result.item.as_deref().map(str::to_owned))
            .map(|(g, it)| Shown {
                made: Made::Grid(g),
                out: Out::Item(it, result.count.max(1)),
            })
    };
    for (_, recipe) in &book.crafting {
        let made = match recipe {
            Recipe::Shaped {
                width,
                cells,
                result,
                ..
            } => grid(
                *width,
                cells.iter().map(|c| c.as_ref().and_then(shown)).collect(),
                result,
            ),
            Recipe::Shapeless {
                ingredients,
                result,
            } => grid(3, ingredients.iter().map(shown).collect(), result),
            Recipe::Special(Special::Transmute {
                input,
                material,
                min,
                result,
                ..
            }) => shown(input)
                .zip(shown(material))
                .zip(result.item.as_deref().map(str::to_owned))
                .map(|((i, m), it)| recipes::transmute(i, m, *min, it, result.count)),
            Recipe::Special(_) => None,
        };
        out.extend(made);
    }
    for c in &book.cooking {
        if c.kind == recipes::SMELTING
            && let Some(i) = shown(&c.ingredient)
        {
            out.push(recipes::smelting(
                i,
                c.result.item.to_string(),
                c.result.count,
            ));
        }
    }
    let resolve = |id: &str| {
        game.native_recipe_ingredient(&serde_json::Value::String(id.to_owned()))
            .and_then(|i| shown(&i))
    };
    for kind in recipes::MACHINE_TYPES {
        for r in game.native_recipes_of_type(kind) {
            out.extend(recipes::custom(kind, &r.json, &resolve));
        }
    }
    out
}

/// The recipes page `page` shows (rustcraft_rules::handbook::recipes::for_page).
pub(super) fn page_recipes(game: &Game<'_>, page: usize) -> Vec<Shown> {
    match PAGES.get(page) {
        Some(p) if !p.makes.is_empty() => recipes::for_page(p, all_recipes(game)),
        _ => Vec::new(),
    }
}

// ---- The client: the window ----

/// The window asked for by the server, opened at the window's next tick.
static ASKED: AtomicBool = AtomicBool::new(false);
/// Where the window is: kept while it is closed, so it opens again where it was.
static VIEW: LazyLock<Mutex<View>> = LazyLock::new(Mutex::default);
/// The recipes the window has looked up since it opened, by page.
static RECIPES: LazyLock<Mutex<HashMap<&'static str, Vec<Shown>>>> = LazyLock::new(Mutex::default);

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

fn recipes() -> MutexGuard<'static, HashMap<&'static str, Vec<Shown>>> {
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

/// A page's recipes, looked up once while the window is open.
fn recipes_for(game: &Game<'_>, page: usize) -> Vec<Shown> {
    let id = PAGES[page].id;
    if let Some(r) = recipes().get(id) {
        return r.clone();
    }
    let r = page_recipes(game, page);
    recipes().insert(id, r.clone());
    r
}

/// A page laid out in the reader's language: its rows, its sheets and its recipes.
struct Laid {
    rows: Vec<Row>,
    sheets: Vec<Range<usize>>,
    recipes: Vec<Shown>,
}

fn laid(game: &Game<'_>, page: usize) -> Laid {
    let p = &PAGES[page];
    let recipes = recipes_for(game, page);
    let heights: Vec<f32> = recipes.iter().map(Shown::height).collect();
    let pieces: Vec<Piece> = rules::pieces(p, &heights, &lookup);
    let measure = |s: &str| game.native_text_width(s);
    let rows = rules::lay_out(&pieces, rules::BODY_WIDTH, &measure);
    let sheets = rules::sheets(&rows);
    Laid {
        rows,
        sheets,
        recipes,
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
    labels(game, batch, at, s, step, t, &fit);
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
        if let Some(Look::Item(id) | Look::Model(id, _)) =
            scene::pick(&drawn, &fit, over).map(|i| drawn[i].look)
        {
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
        Look::Model(id, model) => {
            let side = fit.icon();
            let corner = d.corner(side);
            game.native_item_model(
                batch,
                id,
                model,
                [at[0] + corner[0], at[1] + corner[1]],
                side,
            );
        }
        Look::Drop(argb) => {
            for (corners, light) in scene::cube_faces(c, fit.scale * scene::DROP) {
                game.native_quad(batch, corners, scene::shaded(argb, light));
            }
        }
    }
}

/// The step's badges and name tags: each a plate with its text, scaled down, above its point.
fn labels(
    game: &Game<'_>,
    batch: &mut HudBatch,
    at: [f32; 2],
    s: &Scene,
    step: usize,
    t: f32,
    fit: &scene::Fit,
) {
    let st = &s.steps[step];
    let size = [scene::PANEL[2], scene::PANEL[3]];
    for (k, p) in scene::labels_at(st, t) {
        let Some(words) = lookup(&s.label_key(step, k)) else {
            continue;
        };
        let (rgb, back) = scene::tone_colours(st.labels[k].tone);
        let b = scene::label_box(fit.place(p), game.native_text_width(&words), size);
        game.native_fill(batch, [at[0] + b[0], at[1] + b[1], b[2], b[3]], back);
        let corner = [
            at[0] + b[0] + scene::LABEL_PAD,
            at[1] + b[1] + scene::LABEL_PAD,
        ];
        let start = batch.vertices.len();
        game.native_text(batch, &words, corner, rgb, false);
        batch.scale_since(
            start,
            corner[0],
            corner[1],
            scene::LABEL_SCALE,
            scene::LABEL_SCALE,
        );
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
            Row::Recipes { first, n, .. } => {
                for k in 0..*n {
                    if let Some(r) = laid.recipes.get(first + k) {
                        let at = [x + recipes::column_x(k), y];
                        recipe(game, batch, o, at, r);
                    }
                }
            }
            Row::Cells { cells, role } => {
                for (dx, t) in cells {
                    text(game, batch, t, o, [x + dx, y + 1.0], role_colour(*role));
                }
            }
            Row::Links(pages) => {
                for (col, page) in pages.iter().enumerate() {
                    if let Some(&p) = page.as_ref() {
                        link(game, batch, o, y, col, p);
                    }
                }
            }
            Row::WindowTitle(title) => window_title(game, batch, o, y, title),
            Row::Widget {
                n,
                label,
                widget: w,
                words,
                height,
            } => widget_part(game, batch, o, y, (*n, label, w, words, *height)),
            Row::Gap => {}
        }
        y += row.height();
    }
}

/// A «см. также» link: the page's icon in a slot and its name, lit under the mouse.
fn link(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], y: f32, col: usize, page: usize) {
    let p = &PAGES[page];
    let r = rules::link_rect(y, col);
    let icon = [r[0] + 1.0, r[1] + 1.0];
    slot(game, batch, o, icon);
    item(game, batch, o, icon, p.icon(), 1);
    let measure = |s: &str| game.native_text_width(s);
    let name = rules::clip(&tr(&p.key("name"), p.id), r[2] - 22.0, &measure);
    let hot = kit::hovered(game, o, r);
    let ty = r[1] + ((r[3] - FONT_HEIGHT) / 2.0).round();
    text(
        game,
        batch,
        &name,
        o,
        [r[0] + 21.0, ty],
        if hot { VALUE } else { TEXT },
    );
    kit::hover(game, batch, o, r);
}

fn role_colour(role: Role) -> u32 {
    match role {
        Role::Lead | Role::Value => VALUE,
        Role::Text => TEXT,
        Role::Heading | Role::Key => LABEL,
        Role::Bad => colour::BAD_TEXT,
    }
}

/// A recipe's block with its corner at `at` (window pixels): a grid or a machine's inputs, the
/// arrow (the machine over it) and what it makes; a slot under the mouse names its item.
fn recipe(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], at: [f32; 2], r: &Shown) {
    let mut named: Option<Text> = None;
    let cell = |batch: &mut HudBatch, p: [f32; 2], it: &str, count: u32| {
        let p = [at[0] + p[0], at[1] + p[1]];
        slot(game, batch, o, p);
        item(game, batch, o, p, it, count);
        kit::hovered(game, o, style::slot_rect(p)).then(|| item_name(game, it))
    };
    let (arrow, result) = match &r.made {
        Made::Grid(grid) => {
            for (i, c) in grid.iter().enumerate() {
                let p = recipes::grid_cell(i);
                match c {
                    Some(it) => named = cell(batch, p, it, 1).or(named),
                    None => slot(game, batch, o, [at[0] + p[0], at[1] + p[1]]),
                }
            }
            (recipes::GRID_ARROW, recipes::GRID_RESULT)
        }
        Made::Machine { machine, inputs } => {
            for (k, (it, n)) in inputs.iter().take(recipes::INPUTS).enumerate() {
                named = cell(batch, recipes::input_at(k), it, *n).or(named);
            }
            let m = [
                at[0] + recipes::MACHINE_ICON[0],
                at[1] + recipes::MACHINE_ICON[1],
            ];
            item(game, batch, o, m, machine, 1);
            if kit::hovered(game, o, [m[0], m[1], 16.0, 16.0]) {
                named = Some(item_name(game, machine));
            }
            (recipes::MACHINE_ARROW, recipes::MACHINE_RESULT)
        }
    };
    kit::arrow(
        game,
        batch,
        o,
        [at[0] + arrow[0], at[1] + arrow[1], arrow[2], arrow[3]],
        1.0,
    );
    match &r.out {
        Out::Item(it, n) => named = cell(batch, result, it, *n).or(named),
        Out::Fluid(fluid, mb) => {
            let p = [at[0] + result[0], at[1] + result[1]];
            slot(game, batch, o, p);
            let c = [o[0] + p[0] + 1.0, o[1] + p[1] + 1.0, 16.0, 16.0];
            game.native_fill(batch, c, recipes::fluid_colour(fluid));
            if *mb != recipes::BUCKET_MB {
                let n = mb.div_ceil(recipes::BUCKET_MB).to_string();
                kit::text_right(game, batch, &n, o, p[0] + 17.0, p[1] + 9.0, VALUE);
            }
            if kit::hovered(game, o, style::slot_rect(p)) {
                named = Some(fluid_name(fluid));
            }
        }
    }
    if let Some(name) = named {
        game.native_gui_tooltip(batch, &[name], game.native_cursor());
    }
}

/// A fluid's name, as the mod's windows name it (`rustcraft.fluid.<name>.name`).
fn fluid_name(id: &str) -> Text {
    let (space, name) = id.split_once(':').unwrap_or(("minecraft", id));
    if space == "minecraft" {
        let key = format!("block.minecraft.{name}");
        return Text::translate_with_fallback(&key, name, &[]);
    }
    Text::translate_with_fallback(&format!("{space}.fluid.{name}.name"), name, &[])
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

// ---- The window's picture (rustcraft_rules::handbook::window) ----

/// The picture's title strip: the window's name on a raised strip.
fn window_title(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], y: f32, title: &str) {
    let r = [rules::BODY_X, y, rules::BODY_WIDTH, window::TITLE_HEIGHT];
    kit::fills(game, batch, o, &style::raised(r));
    let measure = |s: &str| game.native_text_width(s);
    let t = rules::clip(title, r[2] - 8.0, &measure);
    text(game, batch, &t, o, [r[0] + 4.0, y + 3.0], TEXT_ON);
}

/// One part of the picture: the dark panel behind it, its number and label, its widget.
fn widget_part(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    y: f32,
    part: (usize, &str, &Widget, &[String], f32),
) {
    let (n, label, w, words, height) = part;
    let x = rules::BODY_X;
    game.native_fill(
        batch,
        [o[0] + x, o[1] + y, rules::BODY_WIDTH, height],
        colour::WELL,
    );
    let square = [x + 2.0, y + 1.0, rules::NUMBER[0], rules::NUMBER[1]];
    kit::fills(game, batch, o, &style::raised(square));
    kit::centred(game, batch, o, square, &n.to_string(), TEXT_ON);
    if !label.is_empty() {
        text(
            game,
            batch,
            &label.to_uppercase(),
            o,
            [x + rules::INDENT + 2.0, y + 1.0],
            LABEL,
        );
    }
    let at = window::widget_at(!label.is_empty());
    let at = [x + at[0] + 2.0, y + at[1]];
    let width = rules::BODY_WIDTH - rules::INDENT - 4.0;
    widget(game, batch, o, at, width, w, words);
}

/// A mock-up slot: its frame, its item's colour, its count.
fn mock_slot(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], p: [f32; 2], c: &Cell) {
    slot(game, batch, o, p);
    if c.rgb != 0 {
        let alpha = if c.ghost { 0x6000_0000 } else { 0xFF00_0000 };
        let r = [o[0] + p[0] + 3.0, o[1] + p[1] + 3.0, 12.0, 12.0];
        game.native_fill(batch, r, alpha | c.rgb);
    }
    if !c.count.is_empty() {
        kit::text_right(game, batch, c.count, o, p[0] + 17.0, p[1] + 9.0, VALUE);
    }
}

fn word(words: &[String], k: usize) -> &str {
    words.get(k).map_or("", String::as_str)
}

/// A widget with its corner at `at` (window pixels), `width` wide.
fn widget(
    game: &Game<'_>,
    batch: &mut HudBatch,
    o: [f32; 2],
    at: [f32; 2],
    width: f32,
    w: &Widget,
    words: &[String],
) {
    let measure = |s: &str| game.native_text_width(s);
    let bar = |batch: &mut HudBatch, pct: u8, rgb: u32| {
        let r = [at[0], at[1] + 1.0, window::BAR[0], window::BAR[1]];
        kit::fills(game, batch, o, &style::well(r));
        let fill = (r[2] - 2.0) * f32::from(pct.min(100)) / 100.0;
        let f = [o[0] + r[0] + 1.0, o[1] + r[1] + 1.0, fill, r[3] - 2.0];
        game.native_fill(batch, f, 0xFF00_0000 | rgb);
        let line = rules::clip(word(words, 0), width - r[2] - 6.0, &measure);
        text(game, batch, &line, o, [at[0] + r[2] + 6.0, at[1]], LABEL);
    };
    match *w {
        Widget::Slots { cols, cells } => {
            let per = window::slot_cols(cols, width);
            for (i, c) in cells.iter().enumerate() {
                let p = [
                    at[0] + window::SLOT * (i % per) as f32,
                    at[1] + window::SLOT * (i / per) as f32,
                ];
                mock_slot(game, batch, o, p, c);
            }
        }
        Widget::Work { pct } => {
            let r = [at[0], at[1] + 1.0, 22.0, 6.0];
            kit::arrow(game, batch, o, r, f64::from(pct) / 100.0);
        }
        Widget::Bar { pct, rgb } | Widget::Tank { pct, rgb } => bar(batch, pct, rgb),
        Widget::Mode { active } => {
            let each = (width / 3.0).min(70.0);
            for k in 0..3 {
                let r = [
                    at[0] + each * k as f32,
                    at[1],
                    each - 2.0,
                    window::MODE_HEIGHT,
                ];
                let t = rules::clip(word(words, k), r[2] - 4.0, &measure);
                button(game, batch, o, r, &t, k == usize::from(active));
            }
        }
        Widget::Sockets { filled } => {
            for (k, &rgb) in filled.iter().enumerate() {
                let c = Cell {
                    rgb,
                    count: "",
                    ghost: false,
                };
                mock_slot(game, batch, o, [at[0] + window::SLOT * k as f32, at[1]], &c);
            }
        }
        Widget::Status { tone } => {
            let (rgb, back) = match tone {
                0 => (colour::OK & 0xFF_FFFF, colour::OK_BACK),
                1 => (colour::WARN & 0xFF_FFFF, colour::WARN_BACK),
                _ => (colour::BAD_TEXT, colour::BAD_BACK),
            };
            let r = [o[0] + at[0], o[1] + at[1], width, window::STATUS_HEIGHT];
            game.native_fill(batch, r, back);
            let t = rules::clip(word(words, 0), width - 6.0, &measure);
            text(game, batch, &t, o, [at[0] + 3.0, at[1] + 2.0], rgb);
        }
        Widget::Rows { n } => {
            for k in 0..usize::from(n) {
                let y = at[1] + rules::LINE * k as f32;
                let value = word(words, 2 * k + 1);
                let room = width - measure(value) - rules::PAIR_GAP;
                let key = rules::clip(word(words, 2 * k), room.max(0.0), &measure);
                text(game, batch, &key, o, [at[0], y], LABEL);
                kit::text_right(game, batch, value, o, at[0] + width, y, VALUE);
            }
        }
        Widget::Chips { on, note } => {
            let rects = window::chip_rects(words, on.len(), width, &measure);
            for (k, r) in rects.iter().enumerate() {
                let r = [at[0] + r[0], at[1] + r[1], r[2], r[3]];
                let lit = on.get(k).copied().unwrap_or(false);
                let face = if lit {
                    style::raised(r)
                } else {
                    style::well(r)
                };
                kit::fills(game, batch, o, &face);
                let t = rules::clip(word(words, k), r[2] - 2.0, &measure);
                kit::centred(game, batch, o, r, &t, if lit { TEXT_ON } else { TEXT });
            }
            if note {
                let top = rects.last().map_or(0.0, |r| r[1] + r[3]) + 2.0;
                for (i, l) in rules::wrap(word(words, on.len()), width, &measure)
                    .iter()
                    .enumerate()
                {
                    let y = at[1] + top + rules::LINE * i as f32;
                    text(game, batch, l, o, [at[0], y], LABEL);
                }
            }
        }
        Widget::Craft { from, to } => {
            for (k, c) in from.iter().enumerate() {
                mock_slot(game, batch, o, [at[0] + window::SLOT * k as f32, at[1]], c);
            }
            let ax = at[0] + window::SLOT * from.len() as f32 + 4.0;
            kit::arrow(
                game,
                batch,
                o,
                [ax, at[1] + 6.0, window::CRAFT_ARROW, 6.0],
                1.0,
            );
            let ox = at[0] + window::craft_out_x(from.len());
            for (k, c) in to.iter().enumerate() {
                mock_slot(game, batch, o, [ox + window::SLOT * k as f32, at[1]], c);
            }
        }
        Widget::Map { n } => {
            let rows = usize::from(n).div_ceil(3).max(1);
            let r = [at[0], at[1], width, rules::LINE * rows as f32 + 2.0];
            kit::fills(game, batch, o, &style::well(r));
            let col = width / 3.0;
            for k in 0..usize::from(n) {
                let t = rules::clip(word(words, k), col - 6.0, &measure);
                let p = [
                    at[0] + 3.0 + col * (k % 3) as f32,
                    at[1] + 2.0 + rules::LINE * (k / 3) as f32,
                ];
                text(game, batch, &t, o, p, VALUE);
            }
        }
        Widget::Text => {
            for (i, l) in rules::wrap(word(words, 0), width, &measure)
                .iter()
                .enumerate()
            {
                text(
                    game,
                    batch,
                    l,
                    o,
                    [at[0], at[1] + rules::LINE * i as f32],
                    TEXT,
                );
            }
        }
    }
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
    if let View::Page { page, sheet } = v.clamped(sheets_of(game, v))
        && page < PAGES.len()
    {
        let laid = laid(game, page);
        if let Some(range) = laid.sheets.get(sheet)
            && let Some(to) = rules::link_at(&laid.rows, range.clone(), sheet, at)
        {
            *view() = View::Page { page: to, sheet: 0 };
            return;
        }
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
