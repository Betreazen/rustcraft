//! Slice 12: the moving parts of the blocks, as the guide moves them (docs/showcase/js/poses.js).
//!
//! The table [`parts::PARTS`] is written by assets/models/export_mc.py from assets/models/anim_mc.py:
//! each moving part is a standalone model the client draws every frame from its block entity
//! (`native_block_entity_submit`, `native_draw_model`), moved by these rules from the game time.
//! The server sends a block's [`Shown`] state (works or stands, its level, a colour) in its update
//! tag when it changes (`native_block_entity_sync`), never each tick.
//!
//! Numbers are Minecraft's: blocks, y up, pivots from the block's lower north-west corner. Turns
//! are radians about the axis through the part's pivot, right-handed as the pose stack turns.

pub mod parts;

/// What a motion moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// A turn about axis 0 (x), 1 (y) or 2 (z), radians.
    Rot(usize),
    /// A slide along the vector, in blocks per unit of the wave.
    Move([f32; 3]),
}

/// The wave a motion follows, of `freq * t + phase`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// The argument itself: a steady spin.
    Lin,
    Sin,
    /// max(0, sin): a lid that knocks up and rests.
    Pos,
    /// 0.5 + 0.5 sin.
    Pulse,
    /// 0.5 - 0.5 cos: out and back from rest.
    Cos01,
    /// The fraction: steam rising and starting again.
    Saw,
    /// The butcher's arm: winds up slowly over 70 % of 1.4 s, strikes over the rest.
    Chop,
    /// The block's level 0..1 (the brewer's bubbles ride on the brew).
    Level,
}

impl Shape {
    pub fn eval(self, x: f32, level: f32) -> f32 {
        match self {
            Shape::Lin => x,
            Shape::Sin => x.sin(),
            Shape::Pos => x.sin().max(0.0),
            Shape::Pulse => 0.5 + 0.5 * x.sin(),
            Shape::Cos01 => 0.5 - 0.5 * x.cos(),
            Shape::Saw => x.rem_euclid(1.0),
            Shape::Chop => {
                const PERIOD: f32 = 1.4;
                let ph = (x / PERIOD).rem_euclid(1.0);
                if ph < 0.7 {
                    -0.7 * (ph / 0.7)
                } else {
                    -0.7 + ((ph - 0.7) / 0.3) * 1.05
                }
            }
            Shape::Level => level,
        }
    }
}

/// One move of a part: `base + amp * shape(freq * t + phase)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub target: Target,
    pub base: f32,
    pub amp: f32,
    pub shape: Shape,
    pub freq: f32,
    pub phase: f32,
}

impl Motion {
    pub fn value(&self, t: f32, level: f32) -> f32 {
        self.base + self.amp * self.shape.eval(self.freq * t + self.phase, level)
    }
}

/// What a part does while its block stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Off {
    /// Drawn still in its rest place.
    Rest,
    /// Not drawn (a flame, a beam, sparks).
    Hide,
    /// Moves as if it worked: the guide's docks, portal dock and windmill always work.
    Always,
    /// Held so (the enchanter's book closed).
    Moves(&'static [Motion]),
}

/// When a part is drawn at all, besides [`Off::Hide`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Show {
    Always,
    /// While `index < floor(rate t) mod modulo`: the autocrafter's lamps light one by one.
    Below { rate: f32, modulo: u32, index: u32 },
    /// While `floor(rate t) mod modulo == index`: the docks' lamps run round.
    Eq { rate: f32, modulo: u32, index: u32 },
    /// While `sin(freq t) > threshold`: sparks.
    Sin { freq: f32, threshold: f32 },
}

impl Show {
    pub fn shown(self, t: f32) -> bool {
        let step = |rate: f32, modulo: u32| ((rate * t).floor().max(0.0) as u64 % u64::from(modulo.max(1))) as u32;
        match self {
            Show::Always => true,
            Show::Below { rate, modulo, index } => index < step(rate, modulo),
            Show::Eq { rate, modulo, index } => index == step(rate, modulo),
            Show::Sin { freq, threshold } => (freq * t).sin() > threshold,
        }
    }
}

/// A moving part of a block (one row of [`parts::PARTS`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    /// The block it belongs to (`rustcraft:crusher`).
    pub block: &'static str,
    /// Its standalone model (`rustcraft:block/crusher__roll_a`).
    pub model: &'static str,
    /// The point it turns about.
    pub pivot: [f32; 3],
    /// The bottom of a level part, which it stretches up from.
    pub bottom: f32,
    pub on: &'static [Motion],
    pub off: Off,
    pub show: Show,
    /// The row of the part it rides on, counted from this one (0 for none): it takes that
    /// part's moves after its own.
    pub carry: i32,
    /// Stretched upright from `bottom` by the block's level.
    pub level: bool,
    /// Coloured by the block's colour (the tank's fluid).
    pub tinted: bool,
}

/// What clients know of a block for its moving parts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    pub on: bool,
    /// 0..=100.
    pub level: u8,
    /// 0xRRGGBB for a tinted part.
    pub colour: u32,
}

impl Shown {
    pub fn level01(&self) -> f32 {
        f32::from(self.level.min(100)) / 100.0
    }
}

/// 0.21 (spec 40, SA2): what a storage's cells and terminals show on their sides. They work (the
/// cells' green lamps, the terminal's scrolling lines) while the storage works, its cells a solid
/// box (S8); their level is the share of the storage's slots in use, rounded up from the first
/// item, so a storage that holds anything never shows an empty slot.
pub fn storage_shown(works: bool, used_slots: u64, total_slots: u64) -> Shown {
    let level = if total_slots == 0 || used_slots == 0 {
        0
    } else {
        (used_slots.min(total_slots) * 100).div_ceil(total_slots).clamp(1, 100) as u8
    };
    Shown { on: works, level, colour: 0 }
}

/// The colour a tank shows `fluid` in (0xRRGGBB), the fluid windows' colours.
pub fn fluid_tint(fluid: &str) -> u32 {
    match fluid {
        "minecraft:water" => 0x3F76E4,
        "minecraft:lava" => 0xE8641E,
        "minecraft:milk" | "rustcraft:milk" => 0xF2F0E8,
        "rustcraft:honey" => 0xE8A33A,
        "rustcraft:experience" => 0x8FE34A,
        "rustcraft:solution" => 0x6CC24A,
        "rustcraft:accelerator" => 0xF5D547,
        "rustcraft:economizer" => 0xE39A2E,
        "rustcraft:enricher" => 0xB65FD8,
        _ => 0x9AA0AA,
    }
}

/// One step of the pose for a part, applied in order to the pose stack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Turn `radians` about axis `axis` through `pivot`.
    Turn { axis: usize, radians: f32, pivot: [f32; 3] },
    Slide([f32; 3]),
    /// Stretch y by `by` about height `from`.
    Stretch { from: f32, by: f32 },
}

/// The parts of `block`, with their rows' indices.
pub fn parts_of(block: &str) -> impl Iterator<Item = (usize, &'static Part)> + '_ {
    parts::PARTS.iter().enumerate().filter(move |(_, p)| p.block == block)
}

/// Whether `block` has moving parts (its block entity draws them).
pub fn moves(block: &str) -> bool {
    parts::PARTS.iter().any(|p| p.block == block)
}

/// The motions part `p` follows now, or none when it is not drawn.
pub fn motions(p: &Part, on: bool, t: f32) -> Option<&'static [Motion]> {
    if on || matches!(p.off, Off::Always) {
        return p.show.shown(t).then_some(p.on);
    }
    match p.off {
        Off::Rest => Some(&[]),
        Off::Hide => None,
        Off::Always => Some(p.on),
        Off::Moves(m) => Some(m),
    }
}

/// Whether a motion changes with the time: a still slide or turn, or one that follows the level,
/// holds its place.
fn changes(m: &Motion) -> bool {
    m.amp != 0.0 && m.freq != 0.0 && m.shape != Shape::Level
}

/// 0.21 (spec 40, SA6): whether part `p` looks different from frame to frame while its block
/// works (`on`) or stands. A part that only shows or hides by the state (a lamp, a fire panel, a
/// level) does not: a block whose parts all hold still keeps its drawing until the server's next
/// change, so a wall of storage cells costs nothing per frame.
pub fn animates(p: &Part, on: bool) -> bool {
    if on || matches!(p.off, Off::Always) {
        return p.on.iter().any(changes) || !matches!(p.show, Show::Always);
    }
    matches!(p.off, Off::Moves(m) if m.iter().any(changes))
}

fn steps_of(p: &Part, moves: &[Motion], t: f32, level: f32, out: &mut Vec<Step>) {
    for m in moves {
        let v = m.value(t, level);
        match m.target {
            Target::Rot(axis) => out.push(Step::Turn {
                axis,
                radians: v,
                pivot: p.pivot,
            }),
            Target::Move(d) => out.push(Step::Slide([d[0] * v, d[1] * v, d[2] * v])),
        }
    }
}

/// The steps that place row `index` now: its carrier's moves first (outer), then its own, then
/// its stretch. None when it is not drawn. `t` is seconds of game time.
pub fn pose(index: usize, shown: &Shown, t: f32) -> Option<Vec<Step>> {
    let p = parts::PARTS.get(index)?;
    let own = motions(p, shown.on, t)?;
    let level = shown.level01();
    let mut out = Vec::new();
    if p.carry != 0 {
        let carrier = usize::try_from(index as i64 + i64::from(p.carry)).ok()?;
        let c = parts::PARTS.get(carrier)?;
        if let Some(cm) = motions(c, shown.on, t) {
            steps_of(c, cm, t, level, &mut out);
        }
    }
    steps_of(p, own, t, level, &mut out);
    if p.level {
        out.push(Step::Stretch {
            from: p.bottom,
            by: level.max(0.001),
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(model: &str) -> (usize, &'static Part) {
        parts::PARTS
            .iter()
            .enumerate()
            .find(|(_, p)| p.model == model)
            .unwrap_or_else(|| panic!("no row {model}"))
    }

    #[test]
    fn the_table_names_known_blocks_and_carriers_of_the_same_block() {
        assert!(parts::PARTS.len() > 40);
        for (i, p) in parts::PARTS.iter().enumerate() {
            assert!(p.block.starts_with("rustcraft:") && p.model.starts_with("rustcraft:block/"));
            if p.carry != 0 {
                let c = &parts::PARTS[(i as i64 + i64::from(p.carry)) as usize];
                assert_eq!(c.block, p.block, "{} rides on another block's part", p.model);
            }
        }
        assert!(moves("rustcraft:crusher") && !moves("rustcraft:housing"));
    }

    #[test]
    fn a_crusher_rolls_only_while_it_works() {
        let (i, _) = row("rustcraft:block/crusher__roll_a");
        let on = Shown { on: true, ..Shown::default() };
        let still = pose(i, &Shown::default(), 2.0).unwrap();
        assert!(still.is_empty(), "at rest it stands");
        let moving = pose(i, &on, 2.0).unwrap();
        assert!(matches!(moving[0], Step::Turn { axis: 0, radians, .. } if (radians.abs() - 10.0).abs() < 1e-4));
    }

    #[test]
    fn a_flame_hides_while_the_furnace_stands_and_the_windmill_always_turns() {
        let (i, _) = row("rustcraft:block/furnace__flame");
        assert_eq!(pose(i, &Shown::default(), 1.0), None);
        let (w, _) = row("rustcraft:block/windmill__rotor");
        assert!(!pose(w, &Shown::default(), 1.0).unwrap().is_empty());
    }

    #[test]
    fn levels_stretch_and_lamps_light_in_turn() {
        let (i, p) = row("rustcraft:block/tank__level_fluid");
        assert!(p.tinted);
        let half = Shown { level: 50, ..Shown::default() };
        let steps = pose(i, &half, 0.0).unwrap();
        assert_eq!(steps.last(), Some(&Step::Stretch { from: p.bottom, by: 0.5 }));
        let (l0, _) = row("rustcraft:block/crafter__lit0");
        let on = Shown { on: true, ..Shown::default() };
        assert_eq!(pose(l0, &on, 0.1), None, "at the start of a round no lamp burns");
        assert!(pose(l0, &on, 0.3).is_some());
        assert_eq!(pose(l0, &Shown::default(), 0.3), None, "lamps are off while it stands");
    }

    #[test]
    fn the_enchanters_pages_ride_on_the_book_and_close_when_it_stands() {
        let (i, p) = row("rustcraft:block/enchanter__page_l");
        assert_ne!(p.carry, 0);
        let closed = pose(i, &Shown::default(), 0.0).unwrap();
        assert_eq!(closed.len(), 1);
        let open = pose(i, &Shown { on: true, ..Shown::default() }, 0.5).unwrap();
        assert!(open.len() >= 3, "the book's turn and bob, then the page");
    }

    /// Spec 40 (SA1–SA5): the blocks whose sides showed nothing of their work now have a part
    /// there, each one model for the four sides.
    #[test]
    fn the_side_walls_show_the_work() {
        for block in ["crusher", "mixer", "placer", "breaker", "brewer", "gen_lava", "portal_dock"] {
            let run: Vec<_> = parts_of(&format!("rustcraft:{block}"))
                .filter(|(_, p)| p.model.ends_with("__run"))
                .collect();
            assert_eq!(run.len(), 1, "{block} has one status light");
            let (i, p) = run[0];
            assert_eq!(pose(i, &Shown::default(), 1.0), None, "{block}: dark while it stands");
            let on = Shown { on: true, ..Shown::default() };
            let low = pose(i, &on, 0.0).unwrap();
            let high = pose(i, &on, 1.0).unwrap();
            assert_ne!(low, high, "{block}: the light runs up while it works");
            assert!(animates(p, true) && !animates(p, false));
        }
        assert!(moves("rustcraft:turbine") && moves("rustcraft:cell") && moves("rustcraft:terminal"));
        assert!(moves("rustcraft:feeder"));
        // The autocrafter's side lamps are the top's: still nine parts.
        assert_eq!(parts_of("rustcraft:crafter").count(), 9);
    }

    #[test]
    fn a_cell_shows_its_fill_and_its_lamp_without_redrawing() {
        let (fill, f) = row("rustcraft:block/cell__level_fill");
        let (lamp, l) = row("rustcraft:block/cell__lamp");
        let works = Shown { on: true, level: 40, colour: 0 };
        assert_eq!(pose(fill, &works, 0.0).unwrap().last(), Some(&Step::Stretch { from: f.bottom, by: 0.4 }));
        assert!(pose(lamp, &works, 0.0).is_some());
        assert_eq!(pose(lamp, &Shown { on: false, ..works }, 0.0), None, "the green lamp is out");
        assert!(!animates(f, true) && !animates(l, true), "a working cell keeps its drawing");
        let (screen, s) = row("rustcraft:block/terminal__screen");
        assert!(animates(s, true) && pose(screen, &Shown::default(), 0.0).is_none());
    }

    #[test]
    fn a_storage_shows_whether_it_works_and_how_full_it_is() {
        assert_eq!(storage_shown(true, 0, 108), Shown { on: true, level: 0, colour: 0 });
        assert_eq!(storage_shown(true, 1, 108).level, 1, "one slot is seen");
        assert_eq!(storage_shown(true, 10, 108).level, 10);
        assert_eq!(storage_shown(false, 54, 108), Shown { on: false, level: 50, colour: 0 });
        assert_eq!(storage_shown(true, 200, 108).level, 100);
        assert_eq!(storage_shown(false, 5, 0).level, 0);
    }

    #[test]
    fn what_animates() {
        assert!(!animates(row("rustcraft:block/furnace__fire").1, true), "a fire panel holds still");
        assert!(animates(row("rustcraft:block/furnace__flame").1, true));
        assert!(animates(row("rustcraft:block/crafter__lit0").1, true), "lamps in turn");
        assert!(!animates(row("rustcraft:block/crafter__lit0").1, false));
        assert!(animates(row("rustcraft:block/windmill__rotor").1, false), "it always turns");
        assert!(!animates(row("rustcraft:block/enchanter__page_l").1, false), "closed and still");
        assert!(!animates(row("rustcraft:block/tank__level_fluid").1, true));
        assert!(animates(row("rustcraft:block/brewer__bubbles").1, true));
    }

    #[test]
    fn the_chop_winds_up_then_strikes() {
        assert!(Shape::Chop.eval(0.5, 0.0) < 0.0);
        assert!(Shape::Chop.eval(1.3, 0.0) > Shape::Chop.eval(1.0, 0.0));
        assert!((Shape::Saw.eval(2.25, 0.0) - 0.25).abs() < 1e-6);
    }
}
