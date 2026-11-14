// Slice 9: the drawn windows of the slice-7 and slice-8 blocks (docs/design/04-window-style.md):
// the turret, the base anchor, the docks, the battery, the windmill, the lava generator, the miner,
// the block placer and the block breaker. They were server menus in chest layout; since slice 9
// each block copies the furnace, so its use opens its content mod's menu type (rustcraft_content),
// and a Horizon client draws it here through `native_screen_draw` like the machines' windows.
//
// What a window shows is a sheet (rustcraft_rules::machines::sheet): rustcraft_base::sheet builds
// it on the server from the lines the server menus showed, as the game's text components, and
// this module sends it to each player who has the window open (`native_player_container_open`,
// `native_net_send_to_player`), again whenever it changes. The client reads each text in its own
// language (`horizon_client::component::to_text`, the chat's reader) and keeps no state of its
// own: a press on a socket, a mode or an ore toggle is a menu button the server answers
// (rustcraft_base::on_sheet_button), and the docks', the miner's, the placer's and the breaker's
// slots are the menu's own, so shift-click, the cursor and quick-move work as in any container.
// The solar panel copies glass for the light under it (E19), opens no menu on use, and keeps its
// server menu (rustcraft_base::windows).
use crate::game::{
    Game, rustcraft_base as base,
    rustcraft_screens::{
        self as kit, LABEL, TEXT, VALUE, label, mode_names, mode_tooltip, slot, socket, text,
        text_right, tr, tr_with,
    },
    rustcraft_server::LevelNet,
};
use horizon_client::{hud::HudBatch, mod_api::NativeScreen, text::Text};
use rustcraft_rules::{
    geom::Pos,
    machines::{
        panel::{inside, slot_frame},
        processor::WorkMode,
        sheet::{self, CHANNEL, Kind, Layout, Press, Sheet, WIDTH},
        style::{self, colour},
    },
    tuning::Tuning,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

/// A sheet goes again at least this often, in level ticks, while its window is open.
const RESEND_EVERY: u64 = 40;
/// And is looked at this often for a change.
const LOOK_EVERY: u64 = 5;
/// The veil over a slot the block does not use yet (the miner's, until a storage module).
const UNUSED: u32 = 0xB311_0D0B;
/// A fluid bar's colour: lava, as the tank's window draws it.
const LAVA: u32 = 0xFFF2_731C;

// ---- The server: who has a sheet window open, and what was sent ----

struct Viewer {
    dimension: String,
    pos: Pos,
    sent: Option<Vec<u8>>,
}

#[derive(Default)]
struct Server {
    viewers: HashMap<u128, Viewer>,
    /// Each level's ticks: the levels tick one after another.
    ticks: HashMap<String, u64>,
}

static SERVER: LazyLock<Mutex<Server>> = LazyLock::new(Mutex::default);

fn server() -> MutexGuard<'static, Server> {
    SERVER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_player_container_open`: a player who opens a sheet window is sent its sheet next.
pub(super) fn on_container_open(player: u128, dimension: &str, menu: &str, pos: Option<Pos>) {
    let Some(pos) = pos.filter(|_| Kind::of(menu).is_some()) else {
        return;
    };
    server().viewers.insert(
        player,
        Viewer {
            dimension: dimension.to_owned(),
            pos,
            sent: None,
        },
    );
}

/// `native_player_container_close`.
pub(super) fn on_container_close(player: u128, menu: &str) {
    if Kind::of(menu).is_some() {
        server().viewers.remove(&player);
    }
}

/// `native_player_logged_out`: a player who left without closing the window gets nothing more.
pub(super) fn on_logout(player: u128) {
    server().viewers.remove(&player);
}

/// One tick of the level: each open window of this level gets its sheet when it changed, and
/// every RESEND_EVERY ticks anyway.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &LevelNet, t: &Tuning) {
    let (due, again) = {
        let mut s = server();
        let ticks = {
            let t = s.ticks.entry(dimension.to_owned()).or_default();
            *t += 1;
            *t
        };
        let due: Vec<(u128, Pos)> = if ticks.is_multiple_of(LOOK_EVERY) {
            s.viewers
                .iter()
                .filter(|(_, v)| v.dimension == dimension)
                .map(|(u, v)| (*u, v.pos))
                .collect()
        } else {
            Vec::new()
        };
        (due, ticks.is_multiple_of(RESEND_EVERY))
    };
    for (player, pos) in due {
        let block = game.native_block(pos);
        let Some(sheet) = base::sheet(game, dimension, level, pos, &block, t) else {
            continue;
        };
        let bytes = sheet::encode(&sheet);
        let fresh = server()
            .viewers
            .get(&player)
            .is_some_and(|v| v.sent.as_ref() != Some(&bytes));
        if !fresh && !again {
            continue;
        }
        match game.native_net_send_to_player(player, CHANNEL, &bytes) {
            Ok(()) => {
                if let Some(v) = server().viewers.get_mut(&player) {
                    v.sent = Some(bytes);
                }
            }
            Err(e) => log::debug!("rustcraft: a sheet not sent to {player:032x}: {e}"),
        }
    }
}

// ---- The client ----

static SHEET: Mutex<Option<Sheet>> = Mutex::new(None);

/// `native_net_from_server`: the open window's sheet.
pub(super) fn on_net(channel: &str, data: &[u8]) {
    if channel != CHANNEL {
        return;
    }
    match sheet::decode(data) {
        Some(s) => *SHEET.lock().unwrap_or_else(PoisonError::into_inner) = Some(s),
        None => log::warn!("rustcraft: a window sheet that does not read"),
    }
}

/// The sheet of the window open now, once the server has sent it.
fn sheet_now(game: &Game<'_>) -> Option<Sheet> {
    let pos = game.native_screen_pos()?;
    SHEET
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .filter(|s| s.pos == pos)
        .cloned()
}

/// A text of the sheet in the player's language.
fn read(v: &Value) -> String {
    horizon_client::component::to_text(v, &mut |s| Text::literal(s)).plain()
}

/// Whether `block` has a sheet window.
pub(super) fn is_sheet(block: &str) -> bool {
    Kind::of(block).is_some()
}

fn layout_of(kind: Kind) -> Layout {
    sheet::layout(kind.shape())
}

/// `native_screen`: the window's size, its slots (the block's own, or its one hidden slot) and
/// the inventory's place.
pub(super) fn layout(block: &str) -> Option<NativeScreen> {
    let kind = Kind::of(block)?;
    let l = layout_of(kind);
    let slots = if l.slots.is_empty() {
        vec![None]
    } else {
        l.slots.iter().copied().map(Some).collect()
    };
    Some(NativeScreen {
        width: WIDTH,
        height: l.height,
        slots,
        inventory_y: l.inventory[1],
        inventory_x: l.inventory[0],
        own_labels: true,
        own_background: true,
        ..NativeScreen::default()
    })
}

fn block_name(block: &str) -> String {
    let key = format!("block.{}", block.replace(':', "."));
    tr(&key, block)
}

fn item_name(id: &str) -> String {
    let key = id.replace(':', ".");
    tr(&format!("item.{key}"), &tr(&format!("block.{key}"), id))
}

/// `native_screen_draw`.
pub(super) fn draw(game: &Game<'_>, block: &str, batch: &mut HudBatch, o: [f32; 2]) {
    let Some(kind) = Kind::of(block) else {
        return;
    };
    let l = layout_of(kind);
    kit::background(game, batch, o, [WIDTH, l.height]);
    kit::title_line(game, batch, o, &block_name(block), None, WIDTH);
    let Some(s) = sheet_now(game) else {
        let waiting = tr("rustcraft.window.waiting", "Waiting for the server…");
        text(game, batch, &waiting, o, [8.0, 18.0], LABEL);
        kit::inventory(game, batch, o, l.inventory, l.inventory_label_y);
        return;
    };
    sockets(game, batch, o, &l, &s);
    if !l.mode.is_empty() {
        let names = mode_names(game, l.mode[2][2]);
        let chosen = s
            .mode
            .and_then(|c| WorkMode::ALL.iter().position(|m| m.code() == c));
        kit::switch(game, batch, o, &l.mode, &names, chosen);
    }
    if let (Some(r), Some(support)) = (l.role, s.support) {
        kit::fills(game, batch, o, &style::role_plate(r, support));
        if let Some(t) = s.role.first() {
            let x = style::status_text_x(r);
            let line = kit::fit(game, &read(t), r[0] + r[2] - x - 2.0);
            text(game, batch, &line, o, [x, r[1] + 3.0], VALUE);
        }
    }
    rows(game, batch, o, &l, &s);
    bars(game, batch, o, &l, &s);
    toggles(game, batch, o, &l, &s);
    let usable = s.usable.map_or(l.slots.len(), usize::from);
    for (i, p) in l.slots.iter().enumerate() {
        slot(game, batch, o, *p);
        if i >= usable {
            game.native_fill(batch, kit::at(o, slot_frame(*p)), UNUSED);
        }
    }
    if let Some(st) = &s.status {
        kit::status_line(game, batch, o, l.status, st.severity(), &read(&st.text));
    }
    kit::inventory(game, batch, o, l.inventory, l.inventory_label_y);
}

fn sockets(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], l: &Layout, s: &Sheet) {
    for (k, p) in l.sockets.iter().enumerate() {
        let module = s.modules.get(k).cloned().flatten();
        socket(game, batch, o, *p, module.is_none());
        if let Some(id) = module {
            game.native_item(batch, &id, [o[0] + p[0], o[1] + p[1]], 16.0);
        }
    }
}

fn rows(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], l: &Layout, s: &Sheet) {
    let shown = s.rows.len().min(l.rows.len());
    for (i, (v, y)) in s.rows.iter().zip(&l.rows).enumerate() {
        let line = kit::fit(game, &read(v), WIDTH - 16.0);
        text(game, batch, &line, o, [8.0, *y], TEXT);
        if i + 1 < shown {
            kit::fills(game, batch, o, &[style::kv_rule(8.0, WIDTH - 8.0, y + 9.0)]);
        }
    }
}

fn bars(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], l: &Layout, s: &Sheet) {
    for (b, (y, r)) in s.bars.iter().zip(&l.bars) {
        label(game, batch, &read(&b.label), o, [r[0], *y]);
        text_right(game, batch, &read(&b.value), o, r[0] + r[2], *y, VALUE);
        let fill = if b.energy { kit::ENERGY } else { LAVA };
        kit::bar(game, batch, kit::at(o, *r), b.share, fill);
    }
}

/// The miner's ore toggles: a raised brass chip with the ore's icon when it is taken, a well with
/// the icon veiled when it is left.
fn toggles(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], l: &Layout, s: &Sheet) {
    for (t, r) in s.toggles.iter().zip(&l.toggles) {
        let face = if t.on {
            style::raised(*r)
        } else {
            style::well(*r)
        };
        kit::fills(game, batch, o, &face);
        let at = [o[0] + r[0] + 2.0, o[1] + r[1] + 2.0];
        if game.native_item(batch, &t.icon, at, 16.0) && !t.on {
            game.native_fill(batch, [at[0], at[1], 16.0, 16.0], colour::GHOST);
        }
        kit::hover(game, batch, o, *r);
    }
}

/// `native_screen_click`: the menu button of the part under `at`.
pub(super) fn on_click(block: &str, at: [f32; 2]) -> Option<i32> {
    let kind = Kind::of(block)?;
    sheet::press_at(&layout_of(kind), at).map(Press::id)
}

fn hint(lines: &mut Vec<String>, s: String) {
    lines.push(format!("§7{s}"));
}

/// `native_screen_tooltip`: what the parts under the mouse mean.
pub(super) fn tooltip(game: &Game<'_>, block: &str, at: [f32; 2], lines: &mut Vec<String>) {
    let Some(kind) = Kind::of(block) else {
        return;
    };
    let l = layout_of(kind);
    let Some(s) = sheet_now(game) else {
        return;
    };
    if let Some(k) = sheet::socket_at(&l, at) {
        lines.push(tr_with(
            "rustcraft.window.socket",
            "Socket %s of %s",
            &[(k + 1).to_string(), sheet::SOCKETS.to_string()],
        ));
        match s.modules.get(k).cloned().flatten() {
            Some(id) => {
                hint(
                    lines,
                    tr_with(
                        "rustcraft.window.socket.module",
                        "module: %s",
                        &[item_name(&id)],
                    ),
                );
                hint(
                    lines,
                    tr("rustcraft.window.module_take", "Click: take it back"),
                );
            }
            None => {
                hint(lines, tr("rustcraft.window.socket.free", "free"));
                hint(
                    lines,
                    tr(
                        "rustcraft.window.socket.put",
                        "Click with a module on the cursor, or shift-click it in the inventory",
                    ),
                );
            }
        }
        return;
    }
    if let Some(i) = l.mode.iter().position(|r| inside(*r, at)) {
        mode_tooltip(WorkMode::ALL[i], lines);
        return;
    }
    if l.role.is_some_and(|r| inside(r, at)) {
        for v in &s.role {
            lines.push(read(v));
        }
        return;
    }
    if let Some((t, _)) = s
        .toggles
        .iter()
        .zip(&l.toggles)
        .find(|(_, r)| inside(**r, at))
    {
        lines.push(read(&t.name));
        hint(lines, read(&t.hint));
        return;
    }
    if let Some((b, _)) = s
        .bars
        .iter()
        .zip(&l.bars)
        .find(|(_, (y, r))| inside([r[0], *y, r[2], r[1] + r[3] - y], at))
    {
        lines.push(read(&b.value));
        if b.energy {
            kit::energy_tooltip(lines);
        }
        return;
    }
    for (i, y) in l.rows.iter().enumerate().take(s.rows.len()) {
        if inside([8.0, *y, WIDTH - 16.0, 9.0], at) {
            lines.push(read(&s.rows[i]));
            for h in s.hints.get(i).into_iter().flatten() {
                hint(lines, read(h));
            }
            return;
        }
    }
    let usable = s.usable.map_or(l.slots.len(), usize::from);
    if l.slots
        .iter()
        .skip(usable)
        .any(|p| inside(slot_frame(*p), at))
    {
        lines.push(tr(
            "rustcraft.miner.more_slots",
            "More slots with a storage module",
        ));
        return;
    }
    if inside(l.status, at)
        && let Some(st) = &s.status
    {
        lines.push(read(&st.text));
    }
}
