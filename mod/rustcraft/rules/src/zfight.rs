//! Round 31: the z-fighting check over every block model of the content mod. Two faces z-fight
//! when they lie in one plane, look the same way and overlap: the game draws both at the same
//! depth and flickers between them. The check reads the exported models and blockstates
//! (mod/content/assets/rustcraft) and looks for such pairs
//!
//! - within each model;
//! - between the models a blockstate shows together (the body, the tier stripe, the connectors);
//! - between a block's body and the parts its block entity draws over it (moving parts at rest,
//!   the crystal's, panel's and windmill's connectors, round 32: every machine's bottom connector);
//! - between a block and its neighbours: a full block, the same block, and a housing on the side
//!   of each connector the block shows (round 32: and below each block with a bottom connector,
//!   its arm up into the block).
//!
//! A pair is left out when the overlap is buried: something solid stands right in front of it,
//! so no eye ever sees it. A pair whose two faces look alike (one texture, one colour cell, the
//! same light) cannot flicker visibly; the report counts those apart and the test lets them be.
//!
//! The models come from Blender (assets/models/*.py through export_mc.py): a case found here is
//! fixed in those scripts and exported again, never by editing the JSON.
//!
//! `cargo test -q zfight -- --nocapture` prints the report.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::anim::{self, Shown, Step};

type V3 = [f64; 3];

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/assets/rustcraft");
/// Pixels: planes closer than this are one plane.
const PLANE_EPS: f64 = 1e-3;
/// Square pixels: overlaps smaller than this are an edge or a corner, not an area.
const AREA_EPS: f64 = 1e-3;
/// How far in front of a face the buried test looks.
const FRONT: f64 = 0.01;
/// The six faces of a model element: name, normal.
const FACES: [(&str, V3); 6] = [
    ("down", [0.0, -1.0, 0.0]),
    ("up", [0.0, 1.0, 0.0]),
    ("north", [0.0, 0.0, -1.0]),
    ("south", [0.0, 0.0, 1.0]),
    ("west", [-1.0, 0.0, 0.0]),
    ("east", [1.0, 0.0, 0.0]),
];

/// A face of a placed model: its corners in pixels (world of the scene), its normal and how it
/// looks.
#[derive(Clone, Debug)]
struct Quad {
    pts: [V3; 4],
    n: V3,
    /// The side of the block whose occluding neighbour hides it (`cullface`), as a direction.
    cull: Option<V3>,
    look: String,
    label: String,
}

/// A model placed in a scene: its faces and the boxes of its unturned elements.
#[derive(Clone, Debug)]
struct Placed {
    key: String,
    quads: Vec<Quad>,
    solids: Vec<(V3, V3)>,
}

/// One element of a model file.
#[derive(Clone, Debug)]
struct Element {
    from: V3,
    to: V3,
    rotation: Option<(usize, f64, V3)>,
    /// (face, look, cullface face).
    faces: Vec<(usize, String, Option<usize>)>,
}

#[derive(Clone, Debug, Default)]
struct Model {
    elements: Vec<Element>,
}

fn v3(v: &Value) -> V3 {
    let at = |i: usize| v[i].as_f64().unwrap_or(0.0);
    [at(0), at(1), at(2)]
}

/// The models of the content mod by their id (`rustcraft:block/<name>`), parents resolved.
fn load_models() -> BTreeMap<String, Model> {
    let mut raw = BTreeMap::new();
    for entry in std::fs::read_dir(format!("{ASSETS}/models/block")).unwrap().filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().trim_end_matches(".json").to_owned();
        let text = std::fs::read_to_string(entry.path()).unwrap();
        let json: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        raw.insert(format!("rustcraft:block/{name}"), json);
    }
    let mut out = BTreeMap::new();
    for (id, json) in &raw {
        let mut own = json.clone();
        while own.get("elements").is_none() {
            let Some(parent) = own["parent"].as_str().and_then(|p| raw.get(p)) else { break };
            let textures = own["textures"].clone();
            own = parent.clone();
            if textures.is_object() {
                own["textures"] = textures;
            }
        }
        out.insert(id.clone(), parse_model(&own));
    }
    out
}

fn parse_model(json: &Value) -> Model {
    let textures = &json["textures"];
    let resolve = |t: &str| -> String {
        let mut t = t.to_owned();
        for _ in 0..4 {
            match t.strip_prefix('#') {
                Some(key) => t = textures[key].as_str().unwrap_or(key).to_owned(),
                None => break,
            }
        }
        t
    };
    let elements = json["elements"]
        .as_array()
        .map(|els| {
            els.iter()
                .map(|e| {
                    let rotation = e.get("rotation").and_then(|r| {
                        let angle = r["angle"].as_f64()?;
                        let axis = "xyz".find(r["axis"].as_str()?)?;
                        (angle != 0.0).then(|| (axis, angle, v3(&r["origin"])))
                    });
                    let light = e["light_emission"].as_i64().unwrap_or(0);
                    let shade = e["shade"].as_bool().unwrap_or(true);
                    let faces = FACES
                        .iter()
                        .enumerate()
                        .filter_map(|(i, (name, _))| {
                            let f = e["faces"].get(*name)?;
                            let look = format!(
                                "{}|{}|{}|{light}|{shade}",
                                resolve(f["texture"].as_str().unwrap_or("")),
                                f["uv"],
                                f["tintindex"]
                            );
                            let cull = f["cullface"].as_str().and_then(|c| FACES.iter().position(|(n, _)| *n == c));
                            Some((i, look, cull))
                        })
                        .collect();
                    Element { from: v3(&e["from"]), to: v3(&e["to"]), rotation, faces }
                })
                .collect()
        })
        .unwrap_or_default();
    Model { elements }
}

// ---- Geometry ----

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `p` turned right-handed by `radians` about axis `axis` through `origin`.
fn turn(p: V3, axis: usize, radians: f64, origin: V3) -> V3 {
    let (s, c) = radians.sin_cos();
    let (i, j) = [(1, 2), (2, 0), (0, 1)][axis];
    let d = sub(p, origin);
    let mut out = d;
    out[i] = c * d[i] - s * d[j];
    out[j] = s * d[i] + c * d[j];
    add(out, origin)
}

fn turn_dir(n: V3, axis: usize, radians: f64) -> V3 {
    turn(n, axis, radians, [0.0; 3])
}

/// The corners of face `face` of the box `from`..`to`.
fn corners(face: usize, f: V3, t: V3) -> [V3; 4] {
    match face {
        0 => [[f[0], f[1], f[2]], [t[0], f[1], f[2]], [t[0], f[1], t[2]], [f[0], f[1], t[2]]],
        1 => [[f[0], t[1], f[2]], [t[0], t[1], f[2]], [t[0], t[1], t[2]], [f[0], t[1], t[2]]],
        2 => [[f[0], f[1], f[2]], [t[0], f[1], f[2]], [t[0], t[1], f[2]], [f[0], t[1], f[2]]],
        3 => [[f[0], f[1], t[2]], [t[0], f[1], t[2]], [t[0], t[1], t[2]], [f[0], t[1], t[2]]],
        4 => [[f[0], f[1], f[2]], [f[0], t[1], f[2]], [f[0], t[1], t[2]], [f[0], f[1], t[2]]],
        _ => [[t[0], f[1], f[2]], [t[0], t[1], f[2]], [t[0], t[1], t[2]], [t[0], f[1], t[2]]],
    }
}

/// A blockstate's turn, `x` then `y` quarter turns, as the game turns a variant's model (each
/// clockwise seen from the axis' positive end) about the block's middle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Turn {
    x: i64,
    y: i64,
}

impl Turn {
    fn point(self, mut p: V3) -> V3 {
        for _ in 0..self.x.rem_euclid(360) / 90 {
            p = [p[0], p[2], 16.0 - p[1]];
        }
        for _ in 0..self.y.rem_euclid(360) / 90 {
            p = [16.0 - p[2], p[1], p[0]];
        }
        p
    }

    fn dir(self, mut n: V3) -> V3 {
        for _ in 0..self.x.rem_euclid(360) / 90 {
            n = [n[0], n[2], -n[1]];
        }
        for _ in 0..self.y.rem_euclid(360) / 90 {
            n = [-n[2], n[1], n[0]];
        }
        n
    }
}

/// A part's pose from its block entity: the steps of `anim::pose`, outermost first, in pixels.
type Pose = Vec<Step>;

fn pose_point(pose: &Pose, mut p: V3) -> V3 {
    for step in pose.iter().rev() {
        p = match *step {
            Step::Turn { axis, radians, pivot } => {
                turn(p, axis, f64::from(radians), pivot.map(|v| f64::from(v) * 16.0))
            }
            Step::Slide(d) => add(p, d.map(|v| f64::from(v) * 16.0)),
            Step::Stretch { from, by } => {
                let from = f64::from(from) * 16.0;
                [p[0], from + (p[1] - from) * f64::from(by), p[2]]
            }
        };
    }
    p
}

fn pose_dir(pose: &Pose, mut n: V3) -> V3 {
    for step in pose.iter().rev() {
        if let Step::Turn { axis, radians, .. } = *step {
            n = turn_dir(n, axis, f64::from(radians));
        }
    }
    n
}

/// `model` placed with blockstate turn `turn`, block-entity pose `pose`, `offset` blocks away.
fn place(models: &BTreeMap<String, Model>, model: &str, state_turn: Turn, pose: &Pose, offset: [i32; 3]) -> Option<Placed> {
    let m = models.get(model)?;
    let shift = offset.map(|v| f64::from(v) * 16.0);
    let at = |p: V3| add(state_turn.point(pose_point(pose, p)), shift);
    let short = model.trim_start_matches("rustcraft:block/");
    let mut quads = Vec::new();
    let mut solids = Vec::new();
    for (index, e) in m.elements.iter().enumerate() {
        let tilt = |p: V3| match e.rotation {
            Some((axis, angle, origin)) => turn(p, axis, angle.to_radians(), origin),
            None => p,
        };
        for (face, look, cull) in &e.faces {
            let mut n = FACES[*face].1;
            if let Some((axis, angle, _)) = e.rotation {
                n = turn_dir(n, axis, angle.to_radians());
            }
            let n = state_turn.dir(pose_dir(pose, n));
            let pts = corners(*face, e.from, e.to).map(|p| at(tilt(p)));
            quads.push(Quad {
                pts,
                n,
                cull: cull.map(|c| state_turn.dir(FACES[c].1)),
                look: look.clone(),
                label: format!("{short}#{index}{}.{}", span(e.from, e.to), FACES[*face].0),
            });
        }
        let still = pose.iter().all(|s| matches!(s, Step::Slide(_)));
        if e.rotation.is_none() && still {
            let (a, b) = (at(e.from), at(e.to));
            solids.push(([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]));
        }
    }
    let key = format!("{short}@{},{}{}{:?}", state_turn.x, state_turn.y, if pose.is_empty() { "" } else { "~" }, offset);
    Some(Placed { key, quads, solids })
}

/// An element's box as the report names it: `[from..to]`, Minecraft axes (y up).
fn span(from: V3, to: V3) -> String {
    let n = |v: f64| format!("{}", (v * 1000.0).round() / 1000.0);
    format!("[{},{},{}..{},{},{}]", n(from[0]), n(from[1]), n(from[2]), n(to[0]), n(to[1]), n(to[2]))
}

/// A full opaque block `offset` blocks away: every face of the cell, all of one look.
fn full_block(offset: [i32; 3]) -> Placed {
    let shift = offset.map(|v| f64::from(v) * 16.0);
    let (lo, hi) = (shift, add(shift, [16.0; 3]));
    let quads = (0..6)
        .map(|face| Quad {
            pts: corners(face, lo, hi),
            n: FACES[face].1,
            cull: Some(FACES[face].1),
            look: "full block".to_owned(),
            label: format!("full block {offset:?}.{}", FACES[face].0),
        })
        .collect();
    Placed { key: format!("full{offset:?}"), quads, solids: vec![(lo, hi)] }
}

/// The plane coordinates of `p` with axis `drop` left out.
fn flat(p: V3, drop: usize) -> [f64; 2] {
    match drop {
        0 => [p[1], p[2]],
        1 => [p[2], p[0]],
        _ => [p[0], p[1]],
    }
}

fn signed_area(poly: &[[f64; 2]]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f64>() / 2.0
}

fn ccw(mut poly: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    if signed_area(&poly) < 0.0 {
        poly.reverse();
    }
    poly
}

/// `subject` clipped to the convex counter-clockwise `clip` (Sutherland–Hodgman).
fn clip(subject: Vec<[f64; 2]>, clip: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out = subject;
    for i in 0..clip.len() {
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        let side = |p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let input = std::mem::take(&mut out);
        for j in 0..input.len() {
            let (p, q) = (input[j], input[(j + 1) % input.len()]);
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                out.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                out.push([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]);
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

/// The overlap of two faces in one plane looking one way: its area in the plane's projection and
/// sample points of it, back in 3D on the plane. None when they are not in one plane, look
/// different ways or only touch.
fn overlap(a: &Quad, b: &Quad) -> Option<(f64, Vec<V3>)> {
    if dot(a.n, b.n) < 1.0 - 1e-6 || (dot(a.n, a.pts[0]) - dot(b.n, b.pts[0])).abs() > PLANE_EPS {
        return None;
    }
    let drop = (0..3).max_by(|&i, &j| a.n[i].abs().total_cmp(&a.n[j].abs()))?;
    let pa = ccw(a.pts.iter().map(|p| flat(*p, drop)).collect());
    let pb = ccw(b.pts.iter().map(|p| flat(*p, drop)).collect());
    let inter = clip(pa, &pb);
    if inter.len() < 3 {
        return None;
    }
    let area = signed_area(&inter).abs() / a.n[drop].abs();
    if area < AREA_EPS {
        return None;
    }
    // Back to 3D: the dropped coordinate from the plane equation.
    let d = dot(a.n, a.pts[0]);
    let lift = |q: [f64; 2]| -> V3 {
        let mut p = [0.0; 3];
        let (u, v) = [(1, 2), (2, 0), (0, 1)][drop];
        p[u] = q[0];
        p[v] = q[1];
        p[drop] = (d - a.n[u] * q[0] - a.n[v] * q[1]) / a.n[drop];
        p
    };
    let n = inter.len() as f64;
    let mid = [inter.iter().map(|q| q[0]).sum::<f64>() / n, inter.iter().map(|q| q[1]).sum::<f64>() / n];
    let mut samples = vec![lift(mid)];
    samples.extend(inter.iter().map(|q| lift([q[0] + (mid[0] - q[0]) * 0.25, q[1] + (mid[1] - q[1]) * 0.25])));
    Some((area, samples))
}

/// Whether every sample, a hair in front of the plane, is inside one of `solids`.
fn buried(samples: &[V3], n: V3, solids: &[&(V3, V3)]) -> bool {
    samples.iter().all(|s| {
        let p = add(*s, n.map(|v| v * FRONT));
        solids.iter().any(|(lo, hi)| (0..3).all(|k| lo[k] + 1e-6 < p[k] && p[k] < hi[k] - 1e-6))
    })
}

/// A pair of faces that z-fight.
#[derive(Clone, Debug)]
struct Fight {
    scene: String,
    a: String,
    b: String,
    area: f64,
    alike: bool,
}

/// The faces of `a` against those of `b` (or against each other when `b` is none), with the
/// solids of the whole scene for the buried test.
fn fights(scene: &str, a: &Placed, b: Option<&Placed>, solids: &[&(V3, V3)], out: &mut Vec<Fight>) {
    for (i, qa) in a.quads.iter().enumerate() {
        let others: &[Quad] = match b {
            Some(b) => &b.quads,
            None => &a.quads[i + 1..],
        };
        for qb in others {
            let Some((area, samples)) = overlap(qa, qb) else { continue };
            if buried(&samples, qa.n, solids) {
                continue;
            }
            out.push(Fight {
                scene: scene.to_owned(),
                a: qa.label.clone(),
                b: qb.label.clone(),
                area,
                alike: qa.look == qb.look,
            });
        }
    }
}

// ---- Blockstates ----

/// A part of a multipart blockstate (a variants file becomes one part per variant).
#[derive(Clone, Debug)]
struct Part {
    when: Option<Value>,
    model: String,
    turn: Turn,
}

fn apply_of(apply: &Value) -> (String, Turn) {
    let a = if apply.is_array() { &apply[0] } else { apply };
    let t = Turn { x: a["x"].as_i64().unwrap_or(0), y: a["y"].as_i64().unwrap_or(0) };
    (a["model"].as_str().unwrap_or("").to_owned(), t)
}

fn read_state(json: &Value) -> Vec<Part> {
    if let Some(variants) = json["variants"].as_object() {
        return variants
            .iter()
            .map(|(key, v)| {
                let when: serde_json::Map<String, Value> = key
                    .split(',')
                    .filter_map(|kv| kv.split_once('='))
                    .map(|(k, v)| (k.to_owned(), Value::String(v.to_owned())))
                    .collect();
                let (model, turn) = apply_of(v);
                Part { when: (!when.is_empty()).then_some(Value::Object(when)), model, turn }
            })
            .collect();
    }
    json["multipart"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .map(|p| {
                    let (model, turn) = apply_of(&p["apply"]);
                    Part { when: p.get("when").cloned(), model, turn }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn holds(when: &Value, state: &BTreeMap<String, String>) -> bool {
    let obj = when.as_object().cloned().unwrap_or_default();
    if let Some(any) = obj.get("OR").and_then(Value::as_array) {
        return any.iter().any(|w| holds(w, state));
    }
    if let Some(all) = obj.get("AND").and_then(Value::as_array) {
        return all.iter().all(|w| holds(w, state));
    }
    obj.iter().all(|(k, v)| {
        let have = state.get(k).map(String::as_str).unwrap_or("");
        v.as_str().is_some_and(|v| v.split('|').any(|x| x == have))
    })
}

fn mentioned(when: &Value, out: &mut BTreeMap<String, BTreeSet<String>>) {
    let Some(obj) = when.as_object() else { return };
    for (k, v) in obj {
        if k == "OR" || k == "AND" {
            for w in v.as_array().into_iter().flatten() {
                mentioned(w, out);
            }
            continue;
        }
        let values = out.entry(k.clone()).or_default();
        for x in v.as_str().unwrap_or("").split('|') {
            values.insert(x.to_owned());
        }
    }
}

/// Whether a `when` names only `facing` and `lit`: the part is the block's body, in every state.
fn always(when: &Value) -> bool {
    when.as_object().is_some_and(|w| w.keys().all(|k| k == "facing" || k == "lit"))
}

/// Every state the parts tell apart: the values each property takes in some `when`, booleans
/// both ways.
fn states(parts: &[Part]) -> Vec<BTreeMap<String, String>> {
    let mut props = BTreeMap::new();
    for p in parts {
        if let Some(w) = &p.when {
            mentioned(w, &mut props);
        }
    }
    for values in props.values_mut() {
        if values.iter().all(|v| v == "true" || v == "false") {
            values.extend(["true".to_owned(), "false".to_owned()]);
        }
    }
    let mut out = vec![BTreeMap::new()];
    for (k, values) in props {
        out = out
            .into_iter()
            .flat_map(|s| {
                values.iter().map(|v| {
                    let mut s = s.clone();
                    s.insert(k.clone(), v.clone());
                    s
                }).collect::<Vec<_>>()
            })
            .collect();
    }
    out
}

const SIDES: [(&str, [i32; 3]); 6] = [
    ("down", [0, -1, 0]),
    ("up", [0, 1, 0]),
    ("north", [0, 0, -1]),
    ("south", [0, 0, 1]),
    ("west", [-1, 0, 0]),
    ("east", [1, 0, 0]),
];

fn opposite(side: &str) -> &'static str {
    match side {
        "down" => "up",
        "up" => "down",
        "north" => "south",
        "south" => "north",
        "west" => "east",
        _ => "west",
    }
}

/// The poses a block entity holds part `index` in for a while: at rest, and while it works
/// when its moves stand still (a fire panel, a lamp, a lifted top). Moving poses only pass.
fn still_poses(index: usize) -> Vec<Pose> {
    let p = &anim::parts::PARTS[index];
    let mut out = Vec::new();
    let full = Shown { on: false, level: 100, colour: 0 };
    if let Some(rest) = anim::pose(index, &full, 0.0) {
        let moving = matches!(p.off, anim::Off::Always) && p.on.iter().any(|m| m.amp != 0.0);
        if !moving {
            out.push(rest);
        }
    }
    if p.on.iter().all(|m| m.amp == 0.0) && !matches!(p.off, anim::Off::Always) {
        let on = Shown { on: true, ..full };
        if let Some(pose) = (0..40).find_map(|i| anim::pose(index, &on, i as f32 * 0.05)) {
            out.push(pose);
        }
    }
    out
}

/// The blocks whose block entity draws connectors `<model>__link_<side>` on the sides a housing
/// touches (rustcraft_power::link_model).
const DRAWN_LINKS: [&str; 3] = ["rustcraft:crystal", "rustcraft:solar", "rustcraft:windmill"];

/// Where a model stands in a scene: model id, blockstate turn, pose (an index into the survey's
/// poses, 0 for none) and the cell it is in.
type Spot = (String, Turn, usize, [i32; 3]);

/// The placed models of a survey, each placed once.
struct Board<'a> {
    models: &'a BTreeMap<String, Model>,
    poses: Vec<Pose>,
    placed: BTreeMap<Spot, Option<Placed>>,
}

impl Board<'_> {
    fn get(&mut self, spot: &Spot) -> Option<&Placed> {
        if !self.placed.contains_key(spot) {
            let (model, turn, pose, offset) = spot;
            let p = place(self.models, model, *turn, &self.poses[*pose], *offset);
            self.placed.insert(spot.clone(), p);
        }
        self.placed.get(spot).and_then(Option::as_ref)
    }

    fn pose(&mut self, pose: Pose) -> usize {
        self.poses.push(pose);
        self.poses.len() - 1
    }
}

/// The housing's tiers (round 31, the net stream): each has its own core and arms, the tier's
/// colour on their collars and corner caps.
const HOUSING_TIERS: std::ops::RangeInclusive<u8> = 1..=4;

/// The models of a housing of `tier` beside a block, `offset` away from it, with its arm toward
/// the block: its core and that arm with its collar (round 31: the housing joins every machine,
/// so the arm is there and the channels run in it).
fn housing_beside(tier: u8, toward_block: &str, offset: [i32; 3]) -> [Spot; 2] {
    [
        (format!("rustcraft:block/housing_core_{tier}"), Turn::default(), 0, offset),
        (format!("rustcraft:block/housing_arm_{toward_block}_{tier}"), Turn::default(), 0, offset),
    ]
}

/// The spots a block shows in `state`: its blockstate's parts; with no `facing` or facing north,
/// the parts its block entity draws (they turn with the body); and a housing of `tier` beside each
/// side it links. Round 32: a block with a bottom connector stands on a housing in every state,
/// its block entity drawing the connector unturned (crate::connectors::bottom_model).
fn scene_of(
    board: &mut Board<'_>,
    block: &str,
    parts: &[Part],
    state: &BTreeMap<String, String>,
    ber: &[(String, usize)],
    tier: u8,
) -> Vec<Spot> {
    let mut scene: Vec<Spot> = parts
        .iter()
        .filter(|p| p.when.as_ref().is_none_or(|w| holds(w, state)))
        .map(|p| (p.model.clone(), p.turn, 0, [0; 3]))
        .collect();
    if state.get("facing").is_none_or(|f| f == "north") {
        scene.extend(ber.iter().map(|(model, pose)| (model.clone(), Turn::default(), *pose, [0; 3])));
    }
    if block == "rustcraft:housing" {
        return scene;
    }
    if let Some(model) = crate::connectors::bottom_model(block) {
        scene.push((model.to_owned(), Turn::default(), 0, [0; 3]));
        scene.extend(housing_beside(tier, "up", [0, -1, 0]));
    }
    let name = block.trim_start_matches("rustcraft:");
    for (side, offset) in SIDES {
        let link = format!("rustcraft:block/{name}__link_{side}");
        let drawn = DRAWN_LINKS.contains(&block) && board.get(&(link.clone(), Turn::default(), 0, [0; 3])).is_some();
        if drawn && state.get("facing").is_none_or(|f| f == "north") {
            scene.push((link, Turn::default(), 0, [0; 3]));
        }
        if drawn || state.get(side).is_some_and(|v| v == "true") {
            scene.extend(housing_beside(tier, opposite(side), offset));
        }
    }
    scene
}

/// `p` without the faces an occluding neighbour toward `toward` hides.
fn drawn(p: &Placed, toward: V3, neighbour_occludes: bool) -> Placed {
    let quads = p
        .quads
        .iter()
        .filter(|q| !(neighbour_occludes && q.cull.is_some_and(|c| dot(c, toward) > 0.5)))
        .cloned()
        .collect();
    Placed { key: p.key.clone(), quads, solids: p.solids.clone() }
}

/// Everything the check found, over every model, blockstate and neighbour.
fn survey() -> Vec<Fight> {
    let models = load_models();
    // A neighbour housing whose models were renamed would be left out of every scene unseen.
    for tier in HOUSING_TIERS {
        for (model, ..) in housing_beside(tier, "west", [0; 3]) {
            assert!(models.contains_key(&model), "no housing model {model}");
        }
    }
    let mut board = Board { models: &models, poses: vec![Pose::new()], placed: BTreeMap::new() };
    let mut out = Vec::new();
    // Each model alone.
    for id in models.keys() {
        let p = place(&models, id, Turn::default(), &board.poses[0], [0; 3]).unwrap();
        let solids: Vec<_> = p.solids.iter().collect();
        fights(&p.key, &p, None, &solids, &mut out);
    }
    let mut pairs = BTreeSet::new();
    let mut applied = BTreeSet::new();
    for entry in std::fs::read_dir(format!("{ASSETS}/blockstates")).unwrap().filter_map(|e| e.ok()) {
        let block = format!("rustcraft:{}", entry.file_name().to_string_lossy().trim_end_matches(".json"));
        let json: Value = serde_json::from_str(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
        let parts = read_state(&json);
        applied.extend(parts.iter().map(|p| (block.clone(), p.model.clone(), p.turn)));
        let mut ber = Vec::new();
        for (index, part) in anim::parts::PARTS.iter().enumerate().filter(|(_, p)| p.block == block) {
            for pose in still_poses(index) {
                let id = board.pose(pose);
                ber.push((part.model.to_owned(), id));
            }
        }
        // A block beside housings meets each tier of them: the tiers differ by colour only, so a
        // pair that looks alike in one tier may not in another.
        let scenes: Vec<(BTreeMap<String, String>, u8, Vec<Spot>)> = states(&parts)
            .into_iter()
            .flat_map(|state| {
                let first = scene_of(&mut board, &block, &parts, &state, &ber, 1);
                let housed = first.iter().any(|s| s.0.starts_with("rustcraft:block/housing_core_"));
                let mut out = vec![(state.clone(), 1, first)];
                if housed && block != "rustcraft:housing" {
                    for tier in HOUSING_TIERS.skip(1) {
                        out.push((state.clone(), tier, scene_of(&mut board, &block, &parts, &state, &ber, tier)));
                    }
                }
                out
            })
            .collect();
        for (state, tier, scene) in scenes {
            let label = if tier == 1 {
                format!("{block} {{{}}}", state_text(&state))
            } else {
                format!("{block} {{{}}} beside housings of tier {tier}", state_text(&state))
            };
            // The block's body, shown in every state (its parts by `facing` and `lit` alone),
            // hides what lies inside it; the parts of some states only (connectors, stripes,
            // housings) cover only their own pairs.
            let body: Vec<Spot> = parts
                .iter()
                .filter(|p| p.when.as_ref().is_none_or(|w| holds(w, &state) && always(w)))
                .map(|p| (p.model.clone(), p.turn, 0, [0; 3]))
                .collect();
            for spot in &body {
                board.get(spot);
            }
            let cover: Vec<(V3, V3)> = body
                .iter()
                .filter_map(|s| board.placed.get(s).and_then(Option::as_ref))
                .flat_map(|p| p.solids.iter().copied())
                .collect();
            for i in 0..scene.len() {
                for j in i + 1..scene.len() {
                    let (a, b) = if scene[i] <= scene[j] { (&scene[i], &scene[j]) } else { (&scene[j], &scene[i]) };
                    // One part in two poses (at rest and working) is never drawn twice at once.
                    let one_part = a.0 == b.0 && a.1 == b.1 && a.3 == b.3;
                    if one_part || !pairs.insert((a.clone(), b.clone())) {
                        continue;
                    }
                    board.get(a);
                    board.get(b);
                    let (Some(Some(pa)), Some(Some(pb))) = (board.placed.get(a), board.placed.get(b)) else { continue };
                    // Only the pair's own boxes and the body count as cover, so the first scene
                    // that holds a pair decides it for every scene: a third part present in some
                    // states only must not hide a fight the others show.
                    let solids: Vec<_> = pa.solids.iter().chain(&pb.solids).chain(&cover).collect();
                    fights(&label, pa, Some(pb), &solids, &mut out);
                }
            }
        }
    }
    // Neighbours: every model a blockstate shows, against a full block and against itself on
    // each side. A face with a cullface toward an occluding neighbour is not drawn.
    for (block, model, turn) in applied {
        let Some(here) = place(&models, &model, turn, &board.poses[0], [0; 3]) else { continue };
        let occludes = !crate::cull::see_through(&block);
        for (side, offset) in SIDES {
            let toward = offset.map(f64::from);
            let away = toward.map(|v| -v);
            let full = full_block(offset);
            let ours = drawn(&here, toward, true);
            let theirs = drawn(&full, away, occludes);
            let solids: Vec<_> = here.solids.iter().chain(&full.solids).collect();
            fights(&format!("{} beside a full block {side}", here.key), &ours, Some(&theirs), &solids, &mut out);
            let twin = place(&models, &model, turn, &board.poses[0], offset).unwrap();
            let ours = drawn(&here, toward, occludes);
            let theirs = drawn(&twin, away, occludes);
            let solids: Vec<_> = here.solids.iter().chain(&twin.solids).collect();
            fights(&format!("{} beside itself {side}", here.key), &ours, Some(&theirs), &solids, &mut out);
        }
    }
    out
}

fn state_text(state: &BTreeMap<String, String>) -> String {
    state.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")
}

fn report(fights: &[Fight], alike: bool) -> String {
    let mut lines: Vec<String> = fights
        .iter()
        .filter(|f| f.alike == alike)
        .map(|f| format!("{}: {} x {} ({:.3} px²)", f.scene, f.a, f.b, f.area))
        .collect();
    lines.sort();
    lines.dedup();
    lines.join("\n")
}

#[test]
fn no_block_model_z_fights() {
    let fights = survey();
    let visible = fights.iter().filter(|f| !f.alike).count();
    let alike = fights.len() - visible;
    println!("z-fighting: {visible} visible, {alike} between faces that look alike");
    println!("{}", report(&fights, false));
    if std::env::var_os("ZFIGHT_ALL").is_some() {
        println!("--- alike ---\n{}", report(&fights, true));
    }
    assert!(visible == 0, "{visible} pairs of faces z-fight:\n{}", report(&fights, false));
}

#[test]
fn every_blockstate_names_an_exported_model() {
    let models = load_models();
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(format!("{ASSETS}/blockstates")).unwrap().filter_map(|e| e.ok()) {
        let json: Value = serde_json::from_str(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
        for part in read_state(&json) {
            if !models.contains_key(&part.model) {
                missing.push(format!("{}: {}", entry.file_name().to_string_lossy(), part.model));
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "blockstates name models that are not exported: {missing:?}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(pts: [V3; 4], n: V3) -> Quad {
        Quad { pts, n, cull: None, look: String::new(), label: String::new() }
    }

    #[test]
    fn faces_in_one_plane_that_overlap_fight_and_touching_ones_do_not() {
        let a = quad(corners(1, [0.0, 0.0, 0.0], [4.0, 2.0, 4.0]), [0.0, 1.0, 0.0]);
        let b = quad(corners(1, [2.0, 0.0, 2.0], [6.0, 2.0, 6.0]), [0.0, 1.0, 0.0]);
        let (area, _) = overlap(&a, &b).unwrap();
        assert!((area - 4.0).abs() < 1e-9);
        let c = quad(corners(1, [4.0, 0.0, 0.0], [8.0, 2.0, 4.0]), [0.0, 1.0, 0.0]);
        assert!(overlap(&a, &c).is_none(), "an edge is no area");
        let d = quad(corners(0, [0.0, 2.0, 0.0], [4.0, 3.0, 4.0]), [0.0, -1.0, 0.0]);
        assert!(overlap(&a, &d).is_none(), "faces looking at each other do not fight");
    }

    #[test]
    fn turns_match_the_game() {
        // y 90 takes north to east; x 90 takes up to north.
        let t = Turn { x: 0, y: 90 };
        assert_eq!(t.dir([0.0, 0.0, -1.0]), [1.0, 0.0, 0.0]);
        assert_eq!(t.point([8.0, 0.0, 0.0]), [16.0, 0.0, 8.0]);
        let x = Turn { x: 90, y: 0 };
        assert_eq!(x.dir([0.0, 1.0, 0.0]), [0.0, 0.0, -1.0]);
        assert_eq!(x.point([8.0, 16.0, 8.0]), [8.0, 8.0, 0.0]);
    }

    #[test]
    fn a_face_behind_a_solid_is_buried() {
        let solid = ([0.0, 0.0, 0.0], [16.0, 4.0, 16.0]);
        assert!(buried(&[[8.0, 4.0, 8.0]], [0.0, -1.0, 0.0], &[&solid]));
        assert!(!buried(&[[8.0, 4.0, 8.0]], [0.0, 1.0, 0.0], &[&solid]));
    }
}
