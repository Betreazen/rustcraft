//! The handbook's animated scenes (spec H3, H4; guide docs/showcase/js/scenes.js).
//!
//! `scenes` (written by assets/models/scenes_mc.py) holds every scene of the guide as snapshots
//! of its steps: the blocks present, the moving objects, the flows and beams, the night's veil and
//! how long the step stays. A page with a scene has one sheet more, its last: the scene's panel,
//! the step's caption under it, and the step buttons in the bottom row.
//!
//! - The panel draws the blocks as block item icons in the game's own view of a block in a slot
//!   (vanilla's `gui` display: 30° down, 225° round, 10 of a slot's 16 pixels to a block edge), so
//!   neighbouring blocks' icons meet edge to edge: `project` is that view, `fit` scales one
//!   scene's every step into the panel at one scale, `drawn` lists a moment's icons back to front.
//! - Flows and beams are dots moving along their paths (`dots`), by the time since the step
//!   began; water, lava and the projector's ghost are tinted cubes (`cube_faces`).
//! - Labels (`Label`): the guide's badges (numbers and diagnoses over the scene) and name tags
//!   (one per kind of block), drawn as small plates above their points (`labels_at`,
//!   `label_box`); a tag on a moving object follows it.
//! - `Playback` steps through a scene: on after its time (`Step::dur`, player.js `stepTime`), back
//!   to the first after the last, or by the step buttons.

use super::{PREFIX, Part};
use crate::machines::panel::{Rect, inside};

pub use super::scenes::{LOOKS, SCENES};

/// What a block of a scene looks like in the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// An item's icon (`minecraft:stone`, `rustcraft:crusher`).
    Item(&'static str),
    /// A block with its moving parts, as a still (assets/models/poses_mc.py): the item, drawn
    /// with the item model (`rustcraft:hb/<block>_on` working, `_still` standing).
    Model(&'static str, &'static str),
    /// A cube of one colour (ARGB): water, lava, the projector's ghost.
    Tint(u32),
    /// A dropped item, as the guide draws one: a little cube of one colour (ARGB), `DROP` of a
    /// block's edge.
    Drop(u32),
}

/// A dropped item's cube, of a block's edge.
pub const DROP: f32 = 0.24;

/// A block standing in a cell; `gone` is when in the step it vanishes (infinity: never).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub look: u16,
    /// Its cell's corner (scene axes: x and y across, z up).
    pub at: [f32; 3],
    pub gone: f32,
}

pub const fn b(look: u16, at: [f32; 3]) -> Block {
    Block {
        look,
        at,
        gone: f32::INFINITY,
    }
}

pub const fn bg(look: u16, at: [f32; 3], gone: f32) -> Block {
    Block { look, at, gone }
}

/// How a moving object goes along its path (engine.js `move`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// To the end, and stays there.
    Once,
    /// From the start again.
    Loop,
    /// There and back.
    Ping,
}

/// An object moving along `path` (cell corners) at `speed` blocks a second, `since` seconds into
/// its move when the step begins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mover {
    pub look: u16,
    pub path: &'static [[f32; 3]],
    pub speed: f32,
    pub mode: Mode,
    pub since: f32,
}

/// What a flow carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Item,
    Fluid,
    Energy,
}

/// `dots` dots of `rgb` going round `path` (cells) at `speed` blocks a second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flow {
    pub kind: Kind,
    pub rgb: u32,
    pub dots: u8,
    pub speed: f32,
    pub path: &'static [[f32; 3]],
}

/// Where a beam ends: a point, or a moving object's body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End {
    Point([f32; 3]),
    Mover(u16),
}

/// A row of dashes running from `from` to `to` (scene points, not cells).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beam {
    pub from: [f32; 3],
    pub to: End,
    pub rgb: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// Seconds the step stays before the next.
    pub dur: f32,
    pub blocks: &'static [Block],
    pub movers: &'static [Mover],
    pub flows: &'static [Flow],
    pub beams: &'static [Beam],
    /// The night's veil over the panel (ARGB), 0 for none.
    pub tint: u32,
    /// Its badges and name tags; label `k`'s text is `Scene::label_key(step, k)`.
    pub labels: &'static [Label],
}

/// What a label says, and so its colours (player.js badge classes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// A diagnosis: the block stands.
    Bad,
    /// It works worse.
    Warn,
    /// It works, or a gain.
    Ok,
    /// A plain fact or number.
    Info,
    /// A block's name.
    Tag,
}

/// A badge or a name tag: where it stands (a scene point, or a moving object's corner raised by
/// `lift` through its middle) and its tone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Label {
    pub at: End,
    pub lift: f32,
    pub tone: Tone,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scene {
    /// The guide's entry, the page's id.
    pub id: &'static str,
    pub steps: &'static [Step],
}

impl Scene {
    /// The caption of step `step`.
    pub fn caption_key(&self, step: usize) -> String {
        format!("{PREFIX}scene.{}.{step}", self.id)
    }

    /// The text of the step's label `k`.
    pub fn label_key(&self, step: usize, k: usize) -> String {
        format!("{PREFIX}scene.{}.{step}.label.{k}", self.id)
    }
}

/// The scene of the page `id`.
pub fn find(id: &str) -> Option<&'static Scene> {
    SCENES.iter().find(|s| s.id == id)
}

pub fn look(i: u16) -> Look {
    LOOKS.get(usize::from(i)).copied().unwrap_or(Look::Tint(0))
}

// ---- The view ----

/// A slot's icon is 16 GUI pixels; a block in it is 10 to an edge (vanilla `block/block`'s gui
/// display: scale 0.625, turned 30° about x and 225° about y).
pub const ICON: f32 = 16.0;
pub const EDGE: f32 = 10.0;
/// One block along x moves its icon this far right and `DOWN` down; along y as far left and as
/// far down; one block up moves it `UP` up.
pub const ACROSS: f32 = EDGE * std::f32::consts::FRAC_1_SQRT_2;
pub const DOWN: f32 = ACROSS * 0.5;
pub const UP: f32 = EDGE * 0.866_025_4;

/// A scene point in GUI pixels at scale 1, from the scene's origin.
pub fn project(p: [f32; 3]) -> [f32; 2] {
    [(p[0] - p[1]) * ACROSS, (p[0] + p[1]) * DOWN - p[2] * UP]
}

/// The middle of a cell whose corner is `at`.
pub fn centre(at: [f32; 3]) -> [f32; 3] {
    [at[0] + 0.5, at[1] + 0.5, at[2] + 0.5]
}

/// How a scene sits in the panel: its points at `scale`, moved by `shift` (panel pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub scale: f32,
    pub shift: [f32; 2],
}

impl Fit {
    /// A scene point in panel pixels.
    pub fn place(&self, p: [f32; 3]) -> [f32; 2] {
        let q = project(p);
        [
            self.shift[0] + q[0] * self.scale,
            self.shift[1] + q[1] * self.scale,
        ]
    }

    /// An icon's side.
    pub fn icon(&self) -> f32 {
        ICON * self.scale
    }
}

/// The panel's scales: the scene no larger than this, and no smaller (a scene that needs less is
/// cut at the panel's edge); a scale is a whole number of sixteenths, so an icon is whole pixels.
pub const MAX_SCALE: f32 = 2.0;
pub const MIN_SCALE: f32 = 0.5;

/// Every cell a scene's steps put something in or move something through.
pub fn cells(scene: &Scene) -> Vec<[f32; 3]> {
    let mut out = Vec::new();
    for st in scene.steps {
        out.extend(st.blocks.iter().map(|b| b.at));
        for m in st.movers {
            out.extend_from_slice(m.path);
        }
        for f in st.flows {
            out.extend_from_slice(f.path);
        }
        // A badge's point, as the guide's camera keeps room for its labels.
        for l in st.labels {
            if let End::Point(p) = l.at {
                out.push([p[0] - 0.5, p[1] - 0.5, p[2] - 0.5]);
            }
        }
    }
    out
}

/// Where the step's labels stand `t` seconds into it: each label's index and scene point.
pub fn labels_at(step: &Step, t: f32) -> Vec<(usize, [f32; 3])> {
    step.labels
        .iter()
        .enumerate()
        .filter_map(|(k, l)| {
            let p = match l.at {
                End::Point(p) => p,
                End::Mover(i) => {
                    let c = step.movers.get(usize::from(i))?.at(t);
                    [c[0] + 0.5, c[1] + 0.5, c[2] + l.lift]
                }
            };
            Some((k, p))
        })
        .collect()
}

/// A label's text is drawn at this share of the window's font.
pub const LABEL_SCALE: f32 = 0.75;
/// The font's line, and what a plate keeps round its text.
pub const LABEL_LINE: f32 = 8.0;
pub const LABEL_PAD: f32 = 2.0;

/// The plate (panel pixels) of a label whose text is `width` wide at scale 1, standing on the
/// point `at`: centred over it, kept inside a panel of `size`.
pub fn label_box(at: [f32; 2], width: f32, size: [f32; 2]) -> Rect {
    let w = width * LABEL_SCALE + 2.0 * LABEL_PAD;
    let h = LABEL_LINE * LABEL_SCALE + 2.0 * LABEL_PAD;
    let x = (at[0] - w / 2.0).clamp(0.0, (size[0] - w).max(0.0));
    let y = (at[1] - h).clamp(0.0, (size[1] - h).max(0.0));
    [x.round(), y.round(), w, h]
}

/// A label's colours: its text (RGB, as the window's text takes it) and its plate (ARGB).
pub fn tone_colours(tone: Tone) -> (u32, u32) {
    use crate::machines::style::colour;
    const RGB: u32 = 0x00FF_FFFF;
    match tone {
        Tone::Bad => (colour::BAD_TEXT, colour::BAD_BACK),
        Tone::Warn => (colour::WARN & RGB, colour::WARN_BACK),
        Tone::Ok => (colour::OK & RGB, colour::OK_BACK),
        Tone::Info => (colour::VALUE, colour::FACE),
        Tone::Tag => (colour::LABEL, 0xC011_0D0B),
    }
}

/// The box (x0, y0, x1, y1, GUI pixels at scale 1) of every icon of the scene, over all its
/// steps, so the scene keeps its place from step to step.
pub fn bounds(scene: &Scene) -> [f32; 4] {
    let half = ICON / 2.0;
    let mut b = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for c in cells(scene) {
        let p = project(centre(c));
        b = [
            b[0].min(p[0] - half),
            b[1].min(p[1] - half),
            b[2].max(p[0] + half),
            b[3].max(p[1] + half),
        ];
    }
    if b[0] > b[2] {
        return [-half, -half, half, half];
    }
    b
}

/// The scene in a panel `size` pixels: as large as fits (between the scales), in its middle.
pub fn fit(scene: &Scene, size: [f32; 2]) -> Fit {
    let b = bounds(scene);
    let (w, h) = ((b[2] - b[0]).max(1.0), (b[3] - b[1]).max(1.0));
    let k = (size[0] / w).min(size[1] / h);
    let scale = ((k * 16.0).floor() / 16.0).clamp(MIN_SCALE, MAX_SCALE);
    let mid = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
    Fit {
        scale,
        shift: [
            size[0] / 2.0 - mid[0] * scale,
            size[1] / 2.0 - mid[1] * scale,
        ],
    }
}

pub fn path_length(path: &[[f32; 3]]) -> f32 {
    path.windows(2).map(|w| distance(w[0], w[1])).sum()
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()
}

/// The point `dist` along `path` (engine.js `pointOnPath`): its end past the end.
pub fn point_on(path: &[[f32; 3]], dist: f32) -> [f32; 3] {
    let mut left = dist.max(0.0);
    for w in path.windows(2) {
        let seg = distance(w[0], w[1]);
        if left <= seg {
            let f = if seg > 0.0 { left / seg } else { 0.0 };
            return [
                w[0][0] + (w[1][0] - w[0][0]) * f,
                w[0][1] + (w[1][1] - w[0][1]) * f,
                w[0][2] + (w[1][2] - w[0][2]) * f,
            ];
        }
        left -= seg;
    }
    path.last().copied().unwrap_or([0.0; 3])
}

impl Mover {
    /// Its cell corner `t` seconds into the step (engine.js `position`).
    pub fn at(&self, t: f32) -> [f32; 3] {
        let len = path_length(self.path);
        let dist = (self.since + t).max(0.0) * self.speed;
        if len <= 0.0 {
            return self.path.first().copied().unwrap_or([0.0; 3]);
        }
        let dist = match self.mode {
            Mode::Once => dist.min(len),
            Mode::Loop => dist % len,
            Mode::Ping => {
                let d = dist % (2.0 * len);
                if d > len { 2.0 * len - d } else { d }
            }
        };
        point_on(self.path, dist)
    }
}

/// One icon or tinted cube of a moment, in panel pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub look: Look,
    /// The middle of its icon.
    pub centre: [f32; 2],
    /// How far towards the viewer its cell is: the order it is drawn in.
    pub depth: f32,
}

impl Drawn {
    /// Its icon's top left at a side of `side`.
    pub fn corner(&self, side: f32) -> [f32; 2] {
        [self.centre[0] - side / 2.0, self.centre[1] - side / 2.0]
    }
}

/// The step's blocks and moving objects `t` seconds into it, back to front: a cell further
/// along x, y or z is nearer the viewer and is drawn later.
pub fn drawn(step: &Step, t: f32, fit: &Fit) -> Vec<Drawn> {
    let mut out: Vec<(Drawn, f32)> = Vec::new();
    let mut add = |look_id: u16, at: [f32; 3]| {
        let c = centre(at);
        let d = Drawn {
            look: look(look_id),
            centre: fit.place(c),
            depth: c[0] + c[1] + c[2],
        };
        out.push((d, c[2]));
    };
    for bl in step.blocks.iter().filter(|b| t < b.gone) {
        add(bl.look, bl.at);
    }
    for m in step.movers {
        add(m.look, m.at(t));
    }
    out.sort_by(|a, b| a.0.depth.total_cmp(&b.0.depth).then(a.1.total_cmp(&b.1)));
    out.into_iter().map(|(d, _)| d).collect()
}

/// The front-most icon at `at` (panel pixels), as an index into `drawn`'s list: its block's
/// outline in the view, near enough (a circle of 0.4 of the icon).
pub fn pick(drawn: &[Drawn], fit: &Fit, at: [f32; 2]) -> Option<usize> {
    let r = fit.icon() * 0.4;
    (0..drawn.len()).rev().find(|&i| {
        let c = drawn[i].centre;
        (c[0] - at[0]).powi(2) + (c[1] - at[1]).powi(2) <= r * r
    })
}

/// The three faces a tinted cube shows round its icon's middle `c`: the top, the +x side and
/// the +y side, each its corners in order and how light it is.
pub fn cube_faces(c: [f32; 2], scale: f32) -> [([[f32; 2]; 4], f32); 3] {
    let at = |dx: f32, dy: f32, dz: f32| {
        let p = project([dx, dy, dz]);
        [c[0] + p[0] * scale, c[1] + p[1] * scale]
    };
    let h = 0.5;
    [
        (
            [at(-h, -h, h), at(h, -h, h), at(h, h, h), at(-h, h, h)],
            1.0,
        ),
        (
            [at(h, -h, h), at(h, h, h), at(h, h, -h), at(h, -h, -h)],
            0.8,
        ),
        (
            [at(-h, h, h), at(h, h, h), at(h, h, -h), at(-h, h, -h)],
            0.6,
        ),
    ]
}

/// `argb` with its colour darkened to `light` (1 unchanged), its alpha kept.
pub fn shaded(argb: u32, light: f32) -> u32 {
    let ch = |shift: u32| ((((argb >> shift) & 0xFF) as f32 * light).round() as u32).min(255);
    (argb & 0xFF00_0000) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// A flow's or a beam's dot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dot {
    pub centre: [f32; 2],
    pub rgb: u32,
    /// A beam's dash, smaller than a flow's dot.
    pub dash: bool,
}

/// How a beam's dashes go (player.js `DASH`, `DASH_GAP`, `DASH_SPEED`): one every `DASH_EVERY`
/// blocks, running at `DASH_SPEED` blocks a second; where a beam meets a moving object's body.
pub const DASH_EVERY: f32 = 0.32;
pub const DASH_SPEED: f32 = 0.9;
pub const BEAM_BODY: f32 = 1.1;

/// The flows' dots and the beams' dashes `t` seconds into the step (player.js `drawOverlays`).
pub fn dots(step: &Step, t: f32, fit: &Fit) -> Vec<Dot> {
    let mut out = Vec::new();
    for f in step.flows {
        let len = path_length(f.path);
        let n = f.dots.max(1);
        for i in 0..n {
            let d = if len > 0.0 {
                (t * f.speed + f32::from(i) * len / f32::from(n)) % len
            } else {
                0.0
            };
            out.push(Dot {
                centre: fit.place(centre(point_on(f.path, d))),
                rgb: f.rgb,
                dash: false,
            });
        }
    }
    for bm in step.beams {
        let to = match bm.to {
            End::Point(p) => p,
            End::Mover(i) => match step.movers.get(usize::from(i)) {
                Some(m) => {
                    let p = m.at(t);
                    [p[0] + 0.5, p[1] + 0.5, p[2] + BEAM_BODY]
                }
                None => continue,
            },
        };
        let len = distance(bm.from, to);
        if len <= 0.0 {
            continue;
        }
        let mut s = (t * DASH_SPEED) % DASH_EVERY;
        while s < len {
            let f = s / len;
            let p = [
                bm.from[0] + (to[0] - bm.from[0]) * f,
                bm.from[1] + (to[1] - bm.from[1]) * f,
                bm.from[2] + (to[2] - bm.from[2]) * f,
            ];
            out.push(Dot {
                centre: fit.place(p),
                rgb: bm.rgb,
                dash: true,
            });
            s += DASH_EVERY;
        }
    }
    out
}

/// A dot's side at `scale`: a flow's dot with its dark rim, a dash without.
pub fn dot_side(dot: &Dot, scale: f32) -> f32 {
    let base = if dot.dash { 1.5 } else { 2.5 };
    (base * scale).round().max(if dot.dash { 1.0 } else { 2.0 })
}

// ---- The sheet ----

/// The scene's panel, the caption under it (at most `CAPTION_LINES` lines), and in the bottom
/// row the sheet's number squeezed left for the step buttons and the step's number.
pub const PANEL: Rect = [8.0, 17.0, 304.0, 123.0];
pub const CAPTION_TOP: f32 = 143.0;
pub const CAPTION_LINES: usize = 4;
pub const SHEET: Rect = [116.0, 191.0, 64.0, 12.0];
pub const STEP_BACK: Rect = [182.0, 191.0, 16.0, 12.0];
pub const STEP_COUNT: Rect = [198.0, 191.0, 78.0, 12.0];
pub const STEP_ON: Rect = [276.0, 191.0, 16.0, 12.0];

/// The step button at `at` (window pixels) of a scene's sheet.
pub fn part_at(at: [f32; 2]) -> Option<Part> {
    if inside(STEP_BACK, at) {
        Some(Part::StepBack)
    } else if inside(STEP_ON, at) {
        Some(Part::StepOn)
    } else {
        None
    }
}

/// A caption as the lines under the panel: wrapped, and the last cut with «…» if it is longer.
pub fn caption_lines(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines = super::wrap(text, width, measure);
    if lines.len() > CAPTION_LINES {
        let rest = lines.split_off(CAPTION_LINES - 1).join(" ");
        lines.push(super::clip(&rest, width, measure));
    }
    lines
}

// ---- Playing ----

/// After a scene's last step the scene waits this much longer, then starts again (player.js).
pub const END_HOLD: f32 = 3.0;

/// Where a page's scene is: its step, and since when (seconds of a clock the window keeps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    pub page: usize,
    pub step: usize,
    pub since: f64,
}

impl Playback {
    pub fn start(page: usize, now: f64) -> Self {
        Playback {
            page,
            step: 0,
            since: now,
        }
    }

    /// Seconds into the step.
    pub fn time(&self, now: f64) -> f32 {
        (now - self.since).max(0.0) as f32
    }

    /// On to the next step once this one's time is up; after the last, from the first again.
    pub fn update(&mut self, scene: &Scene, now: f64) {
        let n = scene.steps.len();
        if n == 0 {
            return;
        }
        self.step = self.step.min(n - 1);
        let last = self.step == n - 1;
        let hold = scene.steps[self.step].dur + if last { END_HOLD } else { 0.0 };
        if self.time(now) >= hold {
            self.step = if last { 0 } else { self.step + 1 };
            self.since = now;
        }
    }

    /// A step on or back by its button: no further than the first or the last; the step starts
    /// from its beginning.
    pub fn go(&mut self, scene: &Scene, forward: bool, now: f64) {
        let last = scene.steps.len().saturating_sub(1);
        self.step = if forward {
            (self.step + 1).min(last)
        } else {
            self.step.saturating_sub(1).min(last)
        };
        self.since = now;
    }
}

#[cfg(test)]
mod tests;
