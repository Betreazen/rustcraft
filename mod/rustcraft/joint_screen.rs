// Slice 18, stream 1: the joint window drawn as the guide's table (ui.js `wrench`, «Стык: …»).
//
// The client half of rustcraft_window's drawn sessions: the server sends the view of one face of
// a housing (rustcraft_rules::net::joint_screen), this opens a screen of the mod's own
// (`native_mod_screen_open`, API 111) under the id `rustcraft:joint`, draws the table «Гнездо /
// Канал / Направление / Фильтр / Приоритет» with «Скопировать настройки / Вставить», and sends a
// press back as a mod message from the client's tick (`native_net_send_to_server` takes the game
// mutably, the mouse hook only lends it). A filter's «+» takes the item in the off hand: the
// wrench is in the main hand.
use crate::game::Game;
use rustcraft_rules::{
    geom::{Face, Pos},
    net::{
        items::Mode,
        joint_screen::{self as js, JointView, Press, ViewRow},
        settings::FILTER_LIMIT,
        topology::ChannelKind,
    },
};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

/// The mod screen's id.
const SCREEN: &str = "rustcraft:joint";
const LEFT_BUTTON: i32 = 0;
const RIGHT_BUTTON: i32 = 1;

#[derive(Default)]
struct JointClient {
    view: Option<JointView>,
    open_due: bool,
    close_due: bool,
    shown: bool,
    presses: Vec<(Pos, Press)>,
}

static CLIENT: LazyLock<Mutex<JointClient>> = LazyLock::new(Mutex::default);

fn client() -> MutexGuard<'static, JointClient> {
    CLIENT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_net_from_server`: a joint window's view.
pub(super) fn on_net(channel: &str, data: &[u8]) {
    if channel != js::CHANNEL {
        return;
    }
    let Some(view) = js::decode(data) else {
        log::warn!("rustcraft: a joint window that does not read");
        return;
    };
    let mut c = client();
    if view.close {
        c.close_due = true;
        c.view = None;
        return;
    }
    c.open_due |= view.open;
    c.view = Some(view);
}

/// `native_tick` on a client: open or close the window as the server said, send the presses.
pub(super) fn client_tick(game: &mut Game<'_>) {
    let (open, close, mut presses) = {
        let mut c = client();
        (
            std::mem::take(&mut c.open_due),
            std::mem::take(&mut c.close_due),
            std::mem::take(&mut c.presses),
        )
    };
    if close && game.native_mod_screen() == Some(SCREEN) {
        game.native_mod_screen_close(SCREEN);
    }
    if open && game.native_mod_screen_open(SCREEN) {
        client().shown = true;
    }
    if client().shown && game.native_mod_screen() != Some(SCREEN) {
        let mut c = client();
        c.shown = false;
        // Closed by the player: the server ends the session.
        if let Some(v) = c.view.take() {
            presses.push((v.pos, Press::Close));
        }
    }
    for (pos, press) in presses {
        if let Err(e) = game.native_net_send_to_server(js::PRESS, &js::encode_press(pos, &press)) {
            log::warn!("rustcraft: a joint window press not sent: {e}");
        }
    }
}

fn is_open(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(SCREEN)
}

fn corner(screen: &horizon_client::mod_api::NativeScreenView<'_>) -> [f32; 2] {
    let o = js::origin([screen.rect[2], screen.rect[3]]);
    [screen.rect[0] + o[0], screen.rect[1] + o[1]]
}

/// `native_screen_mouse_pre`: a click on a tab, a button, a filter item.
pub(super) fn on_mouse(
    game: &Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if *canceled || !is_open(game) {
        return;
    }
    *canceled = true;
    if button != LEFT_BUTTON && button != RIGHT_BUTTON {
        return;
    }
    let o = corner(screen);
    let at = [screen.mouse[0] - o[0], screen.mouse[1] - o[1]];
    let mut c = client();
    let Some(press) = c.view.as_ref().and_then(|v| js::press_at(v, at, button == RIGHT_BUTTON)) else {
        return;
    };
    let Some(pos) = c.view.as_ref().map(|v| v.pos) else {
        return;
    };
    c.presses.push((pos, press));
}

fn face_name(face: Face) -> String {
    use crate::game::rustcraft_screens::tr;
    match face {
        Face::XNeg => tr("rustcraft.face.west", "West"),
        Face::XPos => tr("rustcraft.face.east", "East"),
        Face::YNeg => tr("rustcraft.face.down", "Down"),
        Face::YPos => tr("rustcraft.face.up", "Up"),
        Face::ZNeg => tr("rustcraft.face.north", "North"),
        Face::ZPos => tr("rustcraft.face.south", "South"),
    }
}

fn mode_name(mode: Mode) -> String {
    use crate::game::rustcraft_screens::tr;
    match mode {
        Mode::Into => tr("rustcraft.mode.into", "into the block"),
        Mode::From => tr("rustcraft.mode.from", "out of the block"),
        Mode::Both => tr("rustcraft.mode.both", "default"),
        Mode::Off => tr("rustcraft.mode.off", "off"),
    }
}

fn kind_name(row: &ViewRow) -> String {
    use crate::game::rustcraft_screens::tr;
    match row.channel() {
        Some(ChannelKind::Item) => tr("rustcraft.joint.window.kind.item", "Items"),
        Some(ChannelKind::Fluid) => tr("rustcraft.joint.window.kind.fluid", "Fluids"),
        Some(ChannelKind::Energy) => tr("rustcraft.joint.window.kind.energy", "Energy"),
        Some(ChannelKind::Xp) => tr("rustcraft.joint.window.kind.xp", "Experience"),
        None => tr("rustcraft.joint.window.kind.empty", "empty"),
    }
}

/// `native_screen_render_post`: the window, as ui.js draws it.
pub(super) fn draw(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    use crate::game::rustcraft_screens::{self as kit, tr, tr_with};
    if !is_open(game) {
        return;
    }
    let Some(v) = client().view.clone() else {
        return;
    };
    let o = corner(screen);
    kit::background(game, batch, o, [js::WIDTH, js::HEIGHT]);
    let block = tr(&format!("block.{}", v.block.replace(':', ".")), &v.block);
    let title = tr_with("rustcraft.joint.window.title", "Joint: %s", &[block]);
    kit::title_line(game, batch, o, &title, None, js::WIDTH);
    for (i, r) in js::tab_rects().into_iter().enumerate() {
        let Some(face) = Face::from_index(i as u8) else {
            continue;
        };
        if v.joints[i] {
            kit::button(game, batch, o, r, &face_name(face), i == usize::from(v.face));
        } else {
            kit::centred(game, batch, o, r, &face_name(face), kit::LABEL);
        }
    }
    let heads = [
        tr("rustcraft.joint.window.col.slot", "Slot"),
        tr("rustcraft.joint.window.col.channel", "Channel"),
        tr("rustcraft.joint.window.col.direction", "Direction"),
        tr("rustcraft.joint.window.col.filter", "Filter"),
        tr("rustcraft.joint.window.col.priority", "Priority"),
    ];
    for (head, x) in heads.iter().zip(js::COLUMNS) {
        kit::label(game, batch, head, o, [x, js::HEAD_Y]);
    }
    for (slot, row) in v.rows.iter().enumerate() {
        draw_row(game, batch, o, slot, row);
    }
    kit::button(game, batch, o, js::COPY, &tr("rustcraft.joint.window.copy", "Copy settings"), false);
    let paste = tr("rustcraft.joint.window.paste", "Paste");
    if v.clipboard {
        kit::button(game, batch, o, js::PASTE, &paste, false);
    } else {
        kit::centred(game, batch, o, js::PASTE, &paste, kit::LABEL);
    }
    let hint = tr(
        "rustcraft.joint.window.hint",
        "Filter «+»: the off hand's item · right click a direction: back",
    );
    let hint = kit::fit(game, &hint, js::WIDTH - 16.0);
    kit::text(game, batch, &hint, o, [8.0, js::HINT_Y], kit::LABEL);
}

fn draw_row(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    o: [f32; 2],
    slot: usize,
    row: &ViewRow,
) {
    use crate::game::rustcraft_screens::{self as kit, tr};
    let text_y = js::ROWS_Y + slot as f32 * js::ROW_H + 6.0;
    fn dash(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, o: [f32; 2], column: usize, y: f32) {
        crate::game::rustcraft_screens::text(game, batch, "—", o, [js::COLUMNS[column], y], crate::game::rustcraft_screens::LABEL);
    }
    kit::text(game, batch, &(slot + 1).to_string(), o, [js::COLUMNS[0], text_y], kit::VALUE);
    kit::text(game, batch, &kind_name(row), o, [js::COLUMNS[1], text_y], kit::TEXT);
    if row.directed() {
        kit::button(game, batch, o, js::mode_rect(slot), &mode_name(js::mode_of(row)), true);
    } else {
        dash(game, batch, o, 2, text_y);
    }
    if !row.filtered() {
        dash(game, batch, o, 3, text_y);
        dash(game, batch, o, 4, text_y);
        return;
    }
    let list = if row.allow_only {
        tr("rustcraft.joint.window.white", "white")
    } else {
        tr("rustcraft.joint.window.black", "black")
    };
    kit::button(game, batch, o, js::list_rect(slot), &list, row.allow_only);
    let fluid = row.channel() == Some(ChannelKind::Fluid);
    for (i, item) in row.items.iter().enumerate().take(FILTER_LIMIT) {
        let p = js::item_at(slot, i);
        kit::slot(game, batch, o, p);
        let shown = if fluid { rustcraft_rules::fluid_icons::filter_icon(item) } else { item.clone() };
        game.native_item(batch, &shown, [o[0] + p[0], o[1] + p[1]], 16.0);
    }
    if row.items.len() < FILTER_LIMIT {
        kit::button(game, batch, o, js::add_rect(slot), "+", false);
    }
    let [down, up] = js::priority_rects(slot);
    kit::button(game, batch, o, down, "−", false);
    kit::button(game, batch, o, up, "+", false);
    let between = [down[0] + down[2], down[1], up[0] - down[0] - down[2], down[3]];
    kit::centred(game, batch, o, between, &row.priority.to_string(), kit::VALUE);
}
