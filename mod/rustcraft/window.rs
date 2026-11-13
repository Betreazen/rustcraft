// The joint window (M7, the guide's `wrench` window): the wrench clicked on a housing (or on a
// block beside one, rustcraft_transport::on_right_click_block) opens a page per face of the
// housing that is a joint, with a row per slot: the channel, its direction, its filter (a white
// or black list of up to three items) and its priority. rustcraft_rules::net::joint_window says
// what each cell shows and what a click does; this module turns cells into item stacks and keeps
// the settings in the housing's block entity (`joints`, rustcraft_rules::net::settings::Record).
//
// The housing copies the chorus plant, which opens no menu when used: a block's own menu opens
// only for templates whose use opens one (SRC/crates/horizon-world/src/level/interact.rs:
// 430-454, `PlayerEffect::OpenMenu`, which game.rs:6059-6165 turns into a mod container's
// menu), and a mod menu type changes nothing there. So the window is a server menu, a chest of
// six rows a server mod fills with items (`native_open_menu`, API 10, SRC/docs/native-mods.md:
// 4391-4410, game/native_menus.rs:126-148): every client shows it, no click moves an item, each
// reaches `native_menu_clicked` with no player's state swapped in (native_hooks.rs:7889-7904).
// There are no phantom slots, so a filter item is the player's own: click an empty filter cell,
// then the item in the inventory below (`native_player_stacks`, native_menus.rs:227-239).
//
// The menu opens on the next tick of the player's level, not inside the click: there no player is
// swapped in and the level is in place for the housing's block entity. Clicks wait for that tick
// too.
use crate::game::{
    Game,
    rustcraft_content::{self as content, Machine},
    rustcraft_fluids as fluids,
    rustcraft_server::{self as server, LevelNet},
    rustcraft_transport as transport,
};
use horizon_client::{items::ItemStack, text::Text};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::{Face, Pos, step},
    net::{
        items::Mode,
        joint_window::{self, Action, Cell, Page, Row, Unused},
        joints::ENERGY_DEFAULT,
        settings::{HousingJoints, JointSetting, Record},
        topology::{ChannelKind, Housing, SLOTS},
    },
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

/// The housing's saved compound keeps its joints' settings under this key, as a list of
/// compounds, one per face and slot that has settings.
const JOINTS_KEY: &str = "joints";
const ROWS: u8 = joint_window::ROWS as u8;
/// `ClickType` ids a click on the page counts for: a click and a shift-click.
const PICKUP: i32 = 0;
const QUICK_MOVE: i32 = 1;
const RIGHT_BUTTON: i32 = 1;
/// The player's inventory below the page: 27 main slots, then the hotbar
/// (`native_menu_inventory_slot`, native_menus.rs:24-33).
const MAIN_SLOTS: usize = 27;
const INVENTORY_SLOTS: usize = 36;
/// The game's own empty item.
const AIR: &str = "minecraft:air";

#[derive(Clone)]
struct Session {
    dimension: String,
    housing: Pos,
    face: Face,
    armed: Option<usize>,
    window: i32,
}

struct Opening {
    uuid: u128,
    dimension: String,
    housing: Pos,
    preferred: Option<Face>,
}

struct Click {
    uuid: u128,
    window: i32,
    slot: i32,
    right: bool,
    /// The item in the inventory slot clicked, when the click was on the inventory.
    picked: Option<String>,
}

#[derive(Default)]
struct Windows {
    opening: Vec<Opening>,
    sessions: HashMap<u128, Session>,
    clicks: Vec<Click>,
    /// Each player's «Скопировать»: one face's four slots.
    clipboard: HashMap<u128, [Option<JointSetting>; SLOTS]>,
}

static WINDOWS: LazyLock<Mutex<Windows>> = LazyLock::new(Mutex::default);

fn windows() -> MutexGuard<'static, Windows> {
    WINDOWS.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- Saved settings ----

/// The housing's joints' settings as its block entity keeps them.
pub(super) fn read_joints(nbt: &Compound) -> HousingJoints {
    let Some(Tag::List(list)) = nbt.get(&JavaString::from(JOINTS_KEY)) else {
        return HousingJoints::default();
    };
    let byte = |c: &Compound, k: &str| match c.get(&JavaString::from(k)) {
        Some(Tag::Byte(b)) => Some(*b as u8),
        _ => None,
    };
    let records: Vec<Record> = list
        .iter()
        .filter_map(|tag| {
            let Tag::Compound(c) = tag else {
                return None;
            };
            let priority = match c.get(&JavaString::from("priority")) {
                Some(Tag::Int(p)) => *p,
                _ => 0,
            };
            let items = match c.get(&JavaString::from("items")) {
                Some(Tag::List(items)) => items
                    .iter()
                    .filter_map(|t| match t {
                        Tag::String(s) => Some(s.to_string_lossy()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            Some(Record {
                face: byte(c, "face")?,
                slot: byte(c, "slot")?,
                mode: byte(c, "mode")?,
                allow_only: byte(c, "allow") == Some(1),
                priority,
                items,
            })
        })
        .collect();
    HousingJoints::from_records(&records)
}

pub(super) fn write_joints(nbt: &mut Compound, joints: &HousingJoints) {
    let key = JavaString::from(JOINTS_KEY);
    let records = joints.to_records();
    if records.is_empty() {
        nbt.remove(&key);
        return;
    }
    let list = records
        .into_iter()
        .map(|r| {
            Tag::compound([
                ("face", Tag::Byte(r.face as i8)),
                ("slot", Tag::Byte(r.slot as i8)),
                ("mode", Tag::Byte(r.mode as i8)),
                ("allow", Tag::Byte(i8::from(r.allow_only))),
                ("priority", Tag::Int(r.priority)),
                (
                    "items",
                    Tag::List(
                        r.items
                            .iter()
                            .map(|i| Tag::String(JavaString::from(i.as_str())))
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();
    nbt.insert(key, Tag::List(list));
}

/// The saved compound of a housing without any setting of `slot`: its channel was taken out.
pub(super) fn forget_slot(nbt: &mut Compound, slot: usize) {
    let joints = read_joints(nbt).without_slot(slot);
    write_joints(nbt, &joints);
}

// ---- Hooks ----

/// The wrench asked for the window of `housing`, on `preferred` if that face is a joint.
pub(super) fn request_open(uuid: u128, dimension: &str, housing: Pos, preferred: Option<Face>) {
    let mut w = windows();
    w.opening.retain(|o| o.uuid != uuid);
    w.opening.push(Opening {
        uuid,
        dimension: dimension.to_owned(),
        housing,
        preferred,
    });
}

/// `native_menu_clicked`: a click on one of our windows waits for the level's tick; a click on
/// the inventory below takes note of the item there now.
pub(super) fn on_menu_clicked(
    game: &mut Game<'_>,
    uuid: u128,
    window: i32,
    slot: i32,
    button: i32,
    mode: i32,
) {
    let ours = windows()
        .sessions
        .get(&uuid)
        .is_some_and(|s| s.window == window);
    if !ours || !(mode == PICKUP || mode == QUICK_MOVE) {
        return;
    }
    let Ok(index) = usize::try_from(slot) else {
        return;
    };
    let picked = index.checked_sub(joint_window::CELLS).and_then(|n| {
        let inventory = match n {
            0..MAIN_SLOTS => n + 9,
            MAIN_SLOTS..INVENTORY_SLOTS => n - MAIN_SLOTS,
            _ => return None,
        };
        let stacks = game.native_player_stacks(uuid)?;
        let stack = stacks.get(inventory)?.as_ref()?;
        (&*stack.item != AIR).then(|| stack.item.to_string())
    });
    if index >= joint_window::CELLS && picked.is_none() {
        return;
    }
    windows().clicks.push(Click {
        uuid,
        window,
        slot,
        right: button == RIGHT_BUTTON,
        picked,
    });
}

/// `native_menu_closed`: the player closed it, or another menu took its place.
pub(super) fn on_menu_closed(uuid: u128, window: i32) {
    let mut w = windows();
    if w.sessions.get(&uuid).is_some_and(|s| s.window == window) {
        w.sessions.remove(&uuid);
        w.clicks.retain(|c| c.uuid != uuid);
    }
}

/// Each tick of a level: open the windows asked for there, then answer their clicks.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, level: &mut LevelNet) {
    let (opening, clicks) = {
        let mut guard = windows();
        // Through the guard once, so its fields borrow apart.
        let w = &mut *guard;
        if w.opening.is_empty() && w.clicks.is_empty() {
            return;
        }
        let (here, later): (Vec<Opening>, Vec<Opening>) = std::mem::take(&mut w.opening)
            .into_iter()
            .partition(|o| o.dimension == dimension);
        w.opening = later;
        let sessions = &w.sessions;
        let (here_clicks, later_clicks): (Vec<Click>, Vec<Click>) =
            std::mem::take(&mut w.clicks).into_iter().partition(|c| {
                sessions
                    .get(&c.uuid)
                    .is_some_and(|s| s.dimension == dimension)
            });
        w.clicks = later_clicks;
        (here, here_clicks)
    };
    for o in opening {
        open(game, dimension, o);
    }
    for c in clicks {
        answer(game, dimension, level, c);
    }
}

// ---- What the window shows ----

/// What the block behind one face of the housing is to its joints.
struct FaceBlock {
    block: String,
    /// The default direction of an item joint there; none without slots.
    inventory: Option<Mode>,
    /// A crystal or a machine: an energy joint works there.
    energy: bool,
    /// The default direction of a fluid joint there (a tank, a pump); none for other blocks.
    fluid: Option<Mode>,
}

/// The housing as the window needs it, read fresh from the level.
struct Context {
    slots: Housing,
    joints: HousingJoints,
    faces: [FaceBlock; 6],
}

fn context(game: &mut Game<'_>, dimension: &str, housing: Pos) -> Option<Context> {
    if game.native_block(housing) != content::HOUSING {
        return None;
    }
    let entity = game.native_mod_block_entity(housing)?;
    let slots = server::read_slots(&entity.nbt);
    let joints = read_joints(&entity.nbt);
    let saved = transport::legacy_modes(game);
    let game = &*game;
    let faces = Face::ALL.map(|face| {
        let pos = step(housing, face);
        let block = game.native_block(pos);
        let pipe = block == content::HOUSING || block == content::NODE;
        let side = transport::direction(face.opposite());
        let inventory = if pipe {
            None
        } else {
            transport::default_mode(game, dimension, pos, side, &saved)
        };
        let energy = !pipe && (block == content::CRYSTAL || Machine::of(&block).is_some());
        let fluid = if pipe {
            None
        } else {
            fluids::default_mode(&block)
        };
        FaceBlock {
            block,
            inventory,
            energy,
            fluid,
        }
    });
    Some(Context {
        slots,
        joints,
        faces,
    })
}

fn face_block(ctx: &Context, face: Face) -> &FaceBlock {
    &ctx.faces[usize::from(face.index())]
}

fn is_joint(f: &FaceBlock) -> bool {
    f.inventory.is_some() || f.energy || f.fluid.is_some()
}

/// The default direction of `kind` at a face.
fn default_for(kind: Option<ChannelKind>, f: &FaceBlock) -> Mode {
    match kind {
        Some(ChannelKind::Item) => f.inventory.unwrap_or(Mode::Into),
        Some(ChannelKind::Energy) => ENERGY_DEFAULT,
        Some(ChannelKind::Fluid) => f.fluid.unwrap_or(Mode::Both),
        None => Mode::Both,
    }
}

fn page(ctx: &Context, face: Face, armed: Option<usize>, clipboard: bool) -> Page {
    let f = face_block(ctx, face);
    let kinds: [Option<ChannelKind>; SLOTS] = ctx.slots.slots.map(|c| c.map(|c| c.kind));
    Page {
        joints: ctx.faces.each_ref().map(is_joint),
        selected: face,
        rows: kinds.map(|kind| Row {
            kind,
            here: match kind {
                Some(ChannelKind::Item) => f.inventory.is_some(),
                Some(ChannelKind::Energy) => f.energy,
                Some(ChannelKind::Fluid) => f.fluid.is_some(),
                None => false,
            },
        }),
        settings: std::array::from_fn(|slot| {
            ctx.joints.resolved(face, slot, default_for(kinds[slot], f))
        }),
        armed,
        clipboard,
    }
}

fn open(game: &mut Game<'_>, dimension: &str, o: Opening) {
    let Some(ctx) = context(game, dimension, o.housing) else {
        return;
    };
    let joints = ctx.faces.each_ref().map(is_joint);
    let Some(face) = joint_window::first_page(&joints, o.preferred) else {
        let text = server::line(
            "rustcraft.window.no_joints",
            "This housing has no joints: no block with an inventory or energy beside it",
            &[],
        );
        server::tell(game, o.uuid, text);
        return;
    };
    let clipboard = windows().clipboard.contains_key(&o.uuid);
    let p = page(&ctx, face, None, clipboard);
    let items = stacks(game, &ctx, &p);
    let title = server::line("rustcraft.window.joint", "Joints of the housing", &[]);
    let Some(window) = game.native_open_menu(o.uuid, ROWS, title, items) else {
        log::warn!(
            "rustcraft: the joint window did not open for {:032x}",
            o.uuid
        );
        return;
    };
    windows().sessions.insert(
        o.uuid,
        Session {
            dimension: dimension.to_owned(),
            housing: o.housing,
            face,
            armed: None,
            window,
        },
    );
}

fn answer(game: &mut Game<'_>, dimension: &str, level: &mut LevelNet, c: Click) {
    let (session, clip) = {
        let w = windows();
        let Some(s) = w.sessions.get(&c.uuid).filter(|s| s.window == c.window) else {
            return;
        };
        (s.clone(), w.clipboard.get(&c.uuid).cloned())
    };
    let Some(ctx) = context(game, dimension, session.housing) else {
        // The housing is gone.
        game.native_close_menu(c.uuid);
        return;
    };
    let p = page(&ctx, session.face, session.armed, clip.is_some());
    let index = usize::try_from(c.slot).unwrap_or(usize::MAX);
    let action = match &c.picked {
        Some(item) => joint_window::pick(&p, item),
        None => joint_window::click(&p, index, c.right),
    };
    let mut next = session.clone();
    let mut joints = None;
    match action {
        Action::None => return,
        Action::Select(face) => {
            next.face = face;
            next.armed = None;
        }
        Action::Arm(slot) => next.armed = slot,
        Action::Edit { slot, edit } => {
            let setting = p.settings[slot].apply(&edit);
            joints = Some(ctx.joints.with(session.face, slot, setting));
            if c.picked.is_some() {
                next.armed = None;
            }
        }
        Action::Copy => {
            let copied =
                std::array::from_fn(|slot| ctx.slots.slots[slot].map(|_| p.settings[slot].clone()));
            windows().clipboard.insert(c.uuid, copied);
        }
        Action::Paste => {
            if let Some(clip) = &clip {
                joints = Some(ctx.joints.with_face(session.face, clip));
            }
        }
    }
    if let Some(joints) = &joints {
        let written = game.native_with_mod_block_entity(session.housing, |entity| {
            write_joints(&mut entity.nbt, joints);
        });
        if written.is_none() {
            return;
        }
        level.joints.insert(session.housing, joints.clone());
        // The nets read the new settings with the next rescan, within a second.
        level.stale = true;
    }
    let ctx = Context {
        joints: joints.unwrap_or(ctx.joints),
        ..ctx
    };
    let clipboard = windows().clipboard.contains_key(&c.uuid);
    let shown = page(&ctx, next.face, next.armed, clipboard);
    let items = stacks(game, &ctx, &shown);
    game.native_set_menu_items(c.uuid, items);
    if let Some(s) = windows().sessions.get_mut(&c.uuid) {
        s.face = next.face;
        s.armed = next.armed;
    }
}

// ---- Cells as items ----

fn stacks(game: &Game<'_>, ctx: &Context, page: &Page) -> Vec<Option<ItemStack>> {
    joint_window::cells(page)
        .iter()
        .map(|cell| stack_of(game, ctx, cell))
        .collect()
}

/// A text component as an item's name or lore line: not italic, in `color`.
fn styled(text: &Text, color: &str) -> Tag {
    let mut c = text.component();
    if let Some(o) = c.as_object_mut() {
        o.insert("italic".into(), false.into());
        o.insert("color".into(), color.into());
    }
    horizon_client::lan::data::component_nbt(&c)
}

/// `item` named `name`, with `lore` under it, glinting when `glint`.
pub(super) fn named(item: &str, count: u32, name: Text, lore: &[Text], glint: bool) -> ItemStack {
    let mut stack = ItemStack::new(item, count.clamp(1, 64));
    let data = stack.data_mut();
    data.set_other("minecraft:custom_name", Some(styled(&name, "white")));
    if !lore.is_empty() {
        let lines = lore.iter().map(|l| styled(l, "gray")).collect();
        data.set_other("minecraft:lore", Some(Tag::List(lines)));
    }
    if glint {
        data.set_other("minecraft:enchantment_glint_override", Some(Tag::Byte(1)));
    }
    stack
}

fn tr(key: &str, fallback: &str) -> Text {
    server::line(key, fallback, &[])
}

fn number(n: impl ToString) -> Text {
    Text::literal(n.to_string())
}

fn face_name(face: Face) -> Text {
    match face {
        Face::XNeg => tr("rustcraft.face.west", "West"),
        Face::XPos => tr("rustcraft.face.east", "East"),
        Face::YNeg => tr("rustcraft.face.down", "Down"),
        Face::YPos => tr("rustcraft.face.up", "Up"),
        Face::ZNeg => tr("rustcraft.face.north", "North"),
        Face::ZPos => tr("rustcraft.face.south", "South"),
    }
}

fn kind_name(kind: ChannelKind) -> Text {
    match kind {
        ChannelKind::Item => tr("rustcraft.channel.item", "Item channel"),
        ChannelKind::Fluid => tr("rustcraft.channel.fluid", "Fluid channel"),
        ChannelKind::Energy => tr("rustcraft.channel.energy", "Energy channel"),
    }
}

/// A block's own name, as the game names it (`block.<ns>.<path>`).
fn block_name(block: &str) -> Text {
    Text::translate(&format!("block.{}", block.replace(':', ".")), &[])
}

fn mode_item(mode: Mode) -> &'static str {
    match mode {
        Mode::Into => "minecraft:lime_concrete",
        Mode::From => "minecraft:orange_concrete",
        Mode::Both => "minecraft:light_blue_concrete",
        Mode::Off => "minecraft:gray_concrete",
    }
}

fn stack_of(game: &Game<'_>, ctx: &Context, cell: &Cell) -> Option<ItemStack> {
    let pane = |color: &str| format!("minecraft:{color}_stained_glass_pane");
    let slot_no = |slot: &usize| number(slot + 1);
    let stack = match cell {
        Cell::Blank => return None,
        Cell::Tab {
            face,
            joint: true,
            selected,
        } => {
            let block = &face_block(ctx, *face).block;
            let item = if game.items().get(block).is_some() {
                block.as_str()
            } else {
                "minecraft:barrel"
            };
            let name = server::line(
                "rustcraft.window.tab",
                "%s: %s",
                &[face_name(*face), block_name(block)],
            );
            let lore = if *selected {
                tr("rustcraft.window.tab.open", "This face is open")
            } else {
                tr("rustcraft.window.tab.hint", "Click: open this face")
            };
            named(item, 1, name, &[lore], *selected)
        }
        Cell::Tab { face, .. } => named(
            &pane("gray"),
            1,
            server::line(
                "rustcraft.window.tab.none",
                "%s: no joint",
                &[face_name(*face)],
            ),
            &[],
            false,
        ),
        Cell::Channel {
            slot,
            kind: Some(kind),
        } => named(
            content::channel_item(*kind),
            1,
            server::line(
                "rustcraft.window.slot",
                "Slot %s: %s",
                &[slot_no(slot), kind_name(*kind)],
            ),
            &[],
            false,
        ),
        Cell::Channel { slot, kind: None } => named(
            &pane("light_gray"),
            1,
            server::line(
                "rustcraft.window.slot.empty",
                "Slot %s: empty",
                &[slot_no(slot)],
            ),
            &[],
            false,
        ),
        Cell::Direction { mode, .. } => named(
            mode_item(*mode),
            1,
            server::line(
                "rustcraft.window.direction",
                "Direction: %s",
                &[transport::mode_name(*mode)],
            ),
            &[tr(
                "rustcraft.window.direction.hint",
                "Left click: next · right click: back",
            )],
            false,
        ),
        Cell::List { allow_only, .. } => {
            let (item, name, what) = if *allow_only {
                (
                    "minecraft:white_wool",
                    tr("rustcraft.window.white", "White list"),
                    tr(
                        "rustcraft.window.white.hint",
                        "Lets through only the filter's items",
                    ),
                )
            } else {
                (
                    "minecraft:black_wool",
                    tr("rustcraft.window.black", "Black list"),
                    tr(
                        "rustcraft.window.black.hint",
                        "Lets through everything but the filter's items",
                    ),
                )
            };
            let toggle = tr("rustcraft.window.list.hint", "Click: switch");
            named(item, 1, name, &[what, toggle], false)
        }
        Cell::Filter {
            item: Some(item), ..
        } => {
            let mut stack = ItemStack::new(item, 1);
            let lore = styled(
                &tr(
                    "rustcraft.window.filter.remove",
                    "Click: take it off the filter",
                ),
                "gray",
            );
            stack
                .data_mut()
                .set_other("minecraft:lore", Some(Tag::List(vec![lore])));
            stack
        }
        Cell::Filter { armed: true, .. } => named(
            &pane("lime"),
            1,
            tr(
                "rustcraft.window.filter.armed",
                "Now click the item in your inventory",
            ),
            &[tr(
                "rustcraft.window.filter.cancel",
                "Click here again: cancel",
            )],
            true,
        ),
        Cell::Filter { .. } => named(
            &pane("light_gray"),
            1,
            tr("rustcraft.window.filter.empty", "Empty filter cell"),
            &[tr(
                "rustcraft.window.filter.hint",
                "Click, then an item in your inventory",
            )],
            false,
        ),
        Cell::PriorityDown { priority, .. } => named(
            &pane("red"),
            1,
            tr("rustcraft.window.priority.down", "Priority −1"),
            &[server::line(
                "rustcraft.window.priority.now",
                "Now: %s",
                &[number(priority)],
            )],
            false,
        ),
        Cell::PriorityUp { priority, .. } => named(
            &pane("lime"),
            1,
            tr("rustcraft.window.priority.up", "Priority +1"),
            &[server::line(
                "rustcraft.window.priority.now",
                "Now: %s",
                &[number(priority)],
            )],
            false,
        ),
        Cell::Priority { priority, .. } => {
            let (item, count) = match priority.signum() {
                1 => ("minecraft:lime_dye", priority.unsigned_abs()),
                -1 => ("minecraft:red_dye", priority.unsigned_abs()),
                _ => ("minecraft:gray_dye", 1),
            };
            named(
                item,
                count,
                server::line(
                    "rustcraft.window.priority",
                    "Priority: %s",
                    &[number(priority)],
                ),
                &[
                    tr(
                        "rustcraft.window.priority.hint",
                        "A higher priority fills first",
                    ),
                    tr(
                        "rustcraft.window.priority.click",
                        "Left click: +1 · right click: −1",
                    ),
                ],
                false,
            )
        }
        Cell::Unused { why, .. } => {
            let reason = match why {
                Unused::Empty => tr("rustcraft.window.unused.empty", "No channel in this slot"),
                Unused::NotItems => tr(
                    "rustcraft.window.unused.not_items",
                    "Only item channels have a filter and a priority",
                ),
                Unused::NotHere => tr(
                    "rustcraft.window.unused.not_here",
                    "The block on this face does not work with this channel",
                ),
            };
            named(&pane("black"), 1, Text::literal("—"), &[reason], false)
        }
        Cell::Help => named(
            "minecraft:book",
            1,
            tr("rustcraft.window.help", "How a joint works"),
            &[
                tr(
                    "rustcraft.window.help.1",
                    "Top row: the housing's faces; open one",
                ),
                tr(
                    "rustcraft.window.help.2",
                    "A row per slot: channel, direction, filter, priority",
                ),
                tr(
                    "rustcraft.window.help.3",
                    "Filter: click an empty cell, then an item below",
                ),
            ],
            false,
        ),
        Cell::Copy => named(
            "minecraft:paper",
            1,
            tr("rustcraft.window.copy", "Copy this face's settings"),
            &[],
            false,
        ),
        Cell::Paste { ready: true } => named(
            "minecraft:paper",
            1,
            tr("rustcraft.window.paste", "Paste the settings here"),
            &[],
            true,
        ),
        Cell::Paste { ready: false } => named(
            &pane("gray"),
            1,
            tr("rustcraft.window.paste.none", "Nothing copied yet"),
            &[],
            false,
        ),
    };
    Some(stack)
}

/// The settings every housing of `housings` keeps, for the rescan; housings without any are left out.
pub(super) fn all_joints(
    game: &Game<'_>,
    housings: impl Iterator<Item = Pos>,
) -> BTreeMap<Pos, HousingJoints> {
    housings
        .filter_map(|pos| {
            let entity = game.native_mod_block_entity(pos)?;
            let joints = read_joints(&entity.nbt);
            (!joints.settings.is_empty()).then_some((pos, joints))
        })
        .collect()
}
