// Slice 12: the moving parts of the blocks (rustcraft_rules::anim, written from the guide's
// docs/showcase/js/poses.js by assets/models/export_mc.py).
//
// - Models: every moving part is a standalone model (`native_standalone_models`, API 151) the
//   block's own model leaves out: the crusher's rolls, the mixer's stirrer, the windmill's rotor,
//   flames, lamps, levels.
// - Server: a block's state for its parts (works or stands, its level, a colour) lives in its
//   saved compound; `show` changes it and tells clients once (`native_block_entity_sync`, API 52,
//   as SoulsCraft's pedestal does), never for each tick's progress.
// - Clients get it in the block entity's update tag (`native_block_entity_update_tag`, API 45) and
//   draw the parts each frame (`native_block_entity_submit`, API 71) from the game time, turned as
//   the block's `facing` turns its model. A block that stands with no part moving keeps its
//   drawing (`draw.keep()`, API 94) until the server's next change.
use crate::game::{Game, native_block_entity_draw::BlockEntityDraw};
use horizon_client::containers::ModBlockEntity;
use horizon_world::{
    nbt::{Compound, JavaString, Tag},
    registry::BlockStateId,
};
use rustcraft_rules::anim::{self, Off, Shown, Step, parts::PARTS};

const ON_KEY: &str = "anim_on";
const LEVEL_KEY: &str = "anim_lv";
const COLOUR_KEY: &str = "anim_c";
/// Keys of other modules a client also draws from (the facade's look, rustcraft_facade).
pub(super) const EXTRA_KEYS: [&str; 4] = ["facade", "facade_state", "plan", "ghost"];

/// `native_standalone_models` (API 151): every moving part.
pub(super) fn standalone_models(models: &mut Vec<String>) {
    for p in PARTS {
        if !models.iter().any(|m| m == p.model) {
            models.push(p.model.to_owned());
        }
    }
}

fn byte(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Byte(v)) => i32::from(*v),
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

/// What the block entity's compound says of its parts.
pub(super) fn read(nbt: &Compound) -> Shown {
    Shown {
        on: byte(nbt, ON_KEY) != 0,
        level: byte(nbt, LEVEL_KEY).clamp(0, 100) as u8,
        colour: byte(nbt, COLOUR_KEY) as u32 & 0xFF_FFFF,
    }
}

fn write(nbt: &mut Compound, s: &Shown) {
    nbt.insert(JavaString::from(ON_KEY), Tag::Byte(i8::from(s.on)));
    nbt.insert(JavaString::from(LEVEL_KEY), Tag::Byte(s.level.min(100) as i8));
    nbt.insert(JavaString::from(COLOUR_KEY), Tag::Int((s.colour & 0xFF_FFFF) as i32));
}

/// On the server: the block at `pos` now shows `shown`; clients hear of it only when it changed.
pub(super) fn show(game: &mut Game<'_>, pos: [i32; 3], shown: Shown) {
    let changed = game
        .native_with_mod_block_entity(pos, |e| {
            if !anim::moves(&e.id) || read(&e.nbt) == shown {
                return false;
            }
            write(&mut e.nbt, &shown);
            true
        })
        .unwrap_or(false);
    if changed {
        game.native_block_entity_sync(pos);
    }
}

/// `show` keeping the block's level and colour.
pub(super) fn show_on(game: &mut Game<'_>, pos: [i32; 3], on: bool) {
    let Some(now) = game.native_mod_block_entity(pos).map(|e| read(&e.nbt)) else {
        return;
    };
    show(game, pos, Shown { on, ..now });
}

/// `show` keeping whether it works.
pub(super) fn show_level(game: &mut Game<'_>, pos: [i32; 3], level01: f64, colour: u32) {
    let Some(now) = game.native_mod_block_entity(pos).map(|e| read(&e.nbt)) else {
        return;
    };
    let level = (level01.clamp(0.0, 1.0) * 100.0).round() as u8;
    show(game, pos, Shown { level, colour, ..now });
}

static LEVELS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<[i32; 3], (u8, u32)>>> =
    std::sync::LazyLock::new(Default::default);

/// `show_level` for a block that may change each tick (a tank, a battery): the block entity is read
/// only when the level in percent or the colour changed since the last call for `pos`. The memory
/// is by position alone; two dimensions' blocks at one position only cost extra reads.
pub(super) fn show_level_cached(game: &mut Game<'_>, pos: [i32; 3], level01: f64, colour: u32) {
    let level = (level01.clamp(0.0, 1.0) * 100.0).round() as u8;
    let mut seen = LEVELS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if seen.get(&pos) == Some(&(level, colour)) {
        return;
    }
    seen.insert(pos, (level, colour));
    drop(seen);
    show_level(game, pos, level01, colour);
}

/// `native_block_entity_update_tag` (API 45): what clients draw from.
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if !entity.id.starts_with("rustcraft:") {
        return;
    }
    let mut c = Compound::new();
    // The zipline's anchors draw their rope from theirs (rustcraft_zipline).
    let zipline = crate::game::rustcraft_zipline::KEYS;
    // Slice 18: what the crystals, panels and windmills draw (rustcraft_power).
    let power = crate::game::rustcraft_power::DRAW_KEYS;
    // Slice 18: the turret's aim (rustcraft_base).
    let aim = crate::game::rustcraft_base::AIM_KEYS;
    for key in [ON_KEY, LEVEL_KEY, COLOUR_KEY]
        .into_iter()
        .chain(EXTRA_KEYS)
        .chain(zipline)
        .chain(power)
        .chain(aim)
    {
        if let Some(v) = entity.nbt.get(&JavaString::from(key)) {
            c.insert(JavaString::from(key), v.clone());
        }
    }
    if !c.is_empty() {
        *tag = Some(c);
    }
}

/// The block's turn about the vertical, degrees, as its blockstate turns its model by `facing`
/// (vanilla's y turns clockwise seen from above; the pose turns counterclockwise).
fn facing_degrees(game: &Game<'_>, state: BlockStateId) -> f32 {
    match game.native_block_state_property(state, "facing") {
        Some("east") => -90.0,
        Some("south") => -180.0,
        Some("west") => -270.0,
        _ => 0.0,
    }
}

const AXES: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
/// Game time wraps after this many ticks before it becomes seconds, so f32 keeps its fractions.
const WRAP_TICKS: i64 = 1_200_000;

/// `native_block_entity_submit` (API 71): the block's moving parts. True when it has any.
pub(super) fn draw(
    game: &Game<'_>,
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) -> bool {
    if !anim::moves(block) {
        return false;
    }
    let shown = read(&entity.nbt);
    let t = ((draw.game_time.rem_euclid(WRAP_TICKS)) as f32 + draw.partial) / 20.0;
    let mut moving = false;
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], facing_degrees(game, state), [0.5, 0.5, 0.5]);
    for (i, p) in anim::parts_of(block) {
        moving |= shown.on || matches!(p.off, Off::Always);
        let Some(steps) = anim::pose(i, &shown, t) else {
            continue;
        };
        draw.push();
        // Slice 18: the turret's head turns to its target, the barrel tilts with it.
        if block == "rustcraft:turret" {
            crate::game::rustcraft_base::turn_to_aim(&entity.nbt, p.model, facing_degrees(game, state), draw);
        }
        for step in steps {
            match step {
                Step::Turn { axis, radians, pivot } => {
                    draw.rotate_about(AXES[axis.min(2)], radians.to_degrees(), pivot);
                }
                Step::Slide(by) => draw.translate(by),
                Step::Stretch { from, by } => {
                    draw.translate([0.0, from, 0.0]);
                    draw.scale([1.0, by, 1.0]);
                    draw.translate([0.0, -from, 0.0]);
                }
            }
        }
        let tint = if p.tinted && shown.colour != 0 { shown.colour } else { 0xFF_FFFF };
        game.native_draw_model(draw, p.model, tint);
        draw.pop();
    }
    draw.pop();
    if !moving {
        draw.keep();
    }
    true
}

/// `native_block_entity_render_bounding_box` (API 71): the windmill's rotor stands two blocks up.
pub(super) fn bounds(block: &str, bounds: &mut [f32; 6]) {
    if block == "rustcraft:windmill" {
        bounds[4] = bounds[4].max(4.0);
    }
    if block == "rustcraft:projector" {
        crate::game::rustcraft_builder::ghost_bounds(bounds);
    }
}
