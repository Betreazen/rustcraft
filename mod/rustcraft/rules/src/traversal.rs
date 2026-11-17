//! The zipline's ropes and anchors (12.5, Z1–Z3) and the harpoon's hold on mobs (G5), without
//! the game: the bundle (rustcraft_zipline) reads the blocks and players and applies the answer.
//!
//! - Z1: the zipline aims as its use does ([`aim`]): the first block on the look within
//!   [`ZIP_RANGE`](crate::gear::ZIP_RANGE) that is not air, water or grass; the anchor goes on top
//!   of it when that block is firm and the cell over it free.
//! - Z2: a use ties a rope between a start anchor at the player's feet ([`start_cell`]) and the
//!   anchor on the target. The rope is saved ([`Ropes`]) and stays until an anchor breaks; each
//!   player keeps at most [`MAX_ROPES`], a new one takes the place of their oldest.
//! - Z3: a rope is ridden end to end without a charge, by anyone: from the end the player stands
//!   near ([`end_ride`]), or from where their look grabs it ([`grab`], [`grab_ride`]).
//! - G5 on mobs: a mob on the harpoon's line is pulled to the player; a big one pulls the player
//!   to it instead ([`hold`]).

use crate::geom::Pos;
use crate::monorail::route::{Point, Route, distance};

/// The anchor block a rope is tied to.
pub const ANCHOR: &str = "rustcraft:zip_anchor";
/// The rope runs this high over its anchors' floor: the rider's hands, so the rider's feet reach
/// the anchor's floor at either end.
pub const ROPE_HANDS: f64 = 1.9;
/// The anchor's ring, where its cord goes up to the rope (its model's top, 12 pixels).
pub const ANCHOR_TOP: f64 = 0.75;
/// Ropes a player keeps (a proposal); a new one takes the place of their oldest.
pub const MAX_ROPES: usize = 4;
/// A rope shorter than this is not tied: the player stands at its end already.
pub const MIN_ROPE: f64 = 2.0;
/// A player this close to an end of a rope (feet to the anchor's floor) rides it from there.
pub const NEAR_END: f64 = 4.0;
/// The look grabs a rope that passes within GRAB_RADIUS of it between GRAB_FROM and GRAB_REACH
/// blocks from the eye: not at the eye itself, which stands under the rope at its start anchor.
pub const GRAB_FROM: f64 = 1.0;
pub const GRAB_REACH: f64 = 5.0;
pub const GRAB_RADIUS: f64 = 0.8;
/// A grab this close to an end rides to the other end, whatever the look says.
pub const END_SNAP: f64 = 1.0;
/// G5: a mob at least this wide or tall holds the player, who is pulled to it (a proposal: an
/// iron golem, a ravager, a hoglin and bigger).
pub const BIG_WIDTH: f64 = 1.3;
pub const BIG_HEIGHT: f64 = 2.5;
/// A pulled mob stops this far in front of the player, and the player this far before a big mob
/// (both past the mob's half width).
pub const PULL_GAP: f64 = 1.0;

/// What a cell is to a rope or a harpoon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Air,
    /// Short grass: the look passes through it and an anchor may take its place.
    Grass,
    /// Water, tall grass: the look passes through, an anchor may not stand there.
    Through,
    /// An anchor of a rope.
    Anchor,
    /// A block with a collision box: an anchor may stand on it.
    Solid,
    /// Not passed by the look but with nothing to stand on: a flower, a torch.
    Thin,
}

impl Cell {
    fn passed(self) -> bool {
        matches!(self, Cell::Air | Cell::Grass | Cell::Through)
    }

    /// Whether an anchor may take this cell.
    pub fn takes_anchor(self) -> bool {
        matches!(self, Cell::Air | Cell::Grass)
    }
}

/// Where the zipline aims.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aim {
    /// Nothing to hook within range.
    Far,
    /// The look meets an anchor first.
    Anchor(Pos),
    /// The block `block` takes no anchor on top.
    NoRoom(Pos),
    /// The anchor goes into `anchor`, on top of `block`.
    Valid { block: Pos, anchor: Pos },
}

/// Slice 18: the anchor's ghost before it is set (guide `zipline`: «Виден контур якоря»): the
/// anchor model's boxes (zip_anchor.json, block sixteenths), drawn translucent.
pub const GHOST_BOXES: [([f64; 3], [f64; 3]); 3] = [
    ([5.0, 0.0, 5.0], [11.0, 1.0, 11.0]),
    ([7.0, 1.0, 7.0], [9.0, 10.0, 9.0]),
    ([6.0, 10.0, 6.0], [10.0, 12.0, 10.0]),
];

/// The ghost's boxes of an anchor at `anchor`, in world coordinates.
pub fn ghost_boxes(anchor: Pos) -> Vec<([f64; 3], [f64; 3])> {
    let base = anchor.map(f64::from);
    GHOST_BOXES
        .iter()
        .map(|(lo, hi)| {
            let at = |v: &[f64; 3]| [base[0] + v[0] / 16.0, base[1] + v[1] / 16.0, base[2] + v[2] / 16.0];
            (at(lo), at(hi))
        })
        .collect()
}

/// Z1: where the zipline aims, along `path` (the look's cells from the eye's, ray::blocks_along).
pub fn aim(path: &[Pos], cell: impl Fn(Pos) -> Cell) -> Aim {
    let Some(&hit) = path.iter().skip(1).find(|p| !cell(**p).passed()) else {
        return Aim::Far;
    };
    match cell(hit) {
        Cell::Anchor => Aim::Anchor(hit),
        Cell::Solid => {
            let anchor = up(hit);
            if cell(anchor).takes_anchor() {
                Aim::Valid { block: hit, anchor }
            } else {
                Aim::NoRoom(hit)
            }
        }
        _ => Aim::NoRoom(hit),
    }
}

fn up(p: Pos) -> Pos {
    [p[0], p[1] + 1, p[2]]
}

fn down(p: Pos) -> Pos {
    [p[0], p[1] - 1, p[2]]
}

/// Z2: the start anchor's cell for a player with feet at `feet`: the feet's cell, or the one over
/// it (a player on a slab), that takes an anchor and stands on a firm block. None in the air.
pub fn start_cell(feet: [f64; 3], cell: impl Fn(Pos) -> Cell) -> Option<Pos> {
    let c = [feet[0].floor() as i32, (feet[1] + 1e-3).floor() as i32, feet[2].floor() as i32];
    [c, up(c)]
        .into_iter()
        .find(|&p| cell(p).takes_anchor() && cell(down(p)) == Cell::Solid)
}

/// Where a rider's feet are at an anchor: the middle of its floor.
pub fn feet(p: Pos) -> [f64; 3] {
    [f64::from(p[0]) + 0.5, f64::from(p[1]), f64::from(p[2]) + 0.5]
}

/// Where the rope meets the anchor `p`: over its middle, at the rider's hands.
pub fn hands(p: Pos) -> [f64; 3] {
    let f = feet(p);
    [f[0], f[1] + ROPE_HANDS, f[2]]
}

/// A rope between two anchors.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rope {
    pub id: u64,
    /// The player who tied it, as 32 hex digits (the store is JSON).
    pub owner: String,
    pub dimension: String,
    /// The start anchor, which draws the rope (rustcraft_zipline::draw_anchor).
    pub a: Pos,
    pub b: Pos,
    /// Slice 18: when it was tied, ms of the wall clock (0: an older store's, stamped on load).
    #[serde(default)]
    pub tied_ms: u64,
}

/// Slice 18: a rope comes down by itself this long after it was tied (guide `zipline`: «Трос
/// держится 5 минут»), ms.
pub const ROPE_LIFE_MS: u64 = 5 * 60 * 1000;

/// One end of a rope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    A,
    B,
}

impl End {
    pub fn other(self) -> End {
        match self {
            End::A => End::B,
            End::B => End::A,
        }
    }
}

pub fn owner_key(owner: u128) -> String {
    format!("{owner:032x}")
}

impl Rope {
    pub fn at(&self, end: End) -> Pos {
        match end {
            End::A => self.a,
            End::B => self.b,
        }
    }

    pub fn length(&self) -> f64 {
        distance(feet(self.a), feet(self.b))
    }

    /// The end at `pos`, if one is.
    pub fn end_at(&self, pos: Pos) -> Option<End> {
        if self.a == pos {
            Some(End::A)
        } else if self.b == pos {
            Some(End::B)
        } else {
            None
        }
    }

    /// The end nearer to feet at `at`, and how far its floor is.
    pub fn nearer_end(&self, at: [f64; 3]) -> (End, f64) {
        let (da, db) = (distance(at, feet(self.a)), distance(at, feet(self.b)));
        if da <= db { (End::A, da) } else { (End::B, db) }
    }
}

/// Every rope of the world, as its file keeps them.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Ropes {
    pub next: u64,
    pub ropes: Vec<Rope>,
}

impl Ropes {
    /// Z2: ties a rope; the owner's oldest ropes beyond MAX_ROPES are untied and returned, so
    /// their anchors can go. Its id first.
    pub fn tie(&mut self, owner: u128, dimension: &str, a: Pos, b: Pos) -> (u64, Vec<Rope>) {
        self.next += 1;
        let id = self.next;
        let key = owner_key(owner);
        self.ropes.push(Rope { id, owner: key.clone(), dimension: dimension.to_owned(), a, b, tied_ms: 0 });
        let mut theirs: Vec<u64> = self.ropes.iter().filter(|r| r.owner == key).map(|r| r.id).collect();
        theirs.sort_unstable();
        let over = theirs.len().saturating_sub(MAX_ROPES);
        let gone: Vec<u64> = theirs.into_iter().take(over).collect();
        let untied = gone.iter().filter_map(|g| self.untie(*g)).collect();
        (id, untied)
    }

    pub fn get(&self, id: u64) -> Option<&Rope> {
        self.ropes.iter().find(|r| r.id == id)
    }

    pub fn untie(&mut self, id: u64) -> Option<Rope> {
        let i = self.ropes.iter().position(|r| r.id == id)?;
        Some(self.ropes.remove(i))
    }

    /// The rope with an end at `pos` of `dimension`.
    pub fn at(&self, dimension: &str, pos: Pos) -> Option<&Rope> {
        self.ropes
            .iter()
            .find(|r| r.dimension == dimension && r.end_at(pos).is_some())
    }

    pub fn in_level<'a>(&'a self, dimension: &'a str) -> impl Iterator<Item = &'a Rope> + 'a {
        self.ropes.iter().filter(move |r| r.dimension == dimension)
    }

    /// Ropes with no time yet (just tied, or from an older store) count from `now_ms`.
    pub fn stamp(&mut self, now_ms: u64) {
        for r in self.ropes.iter_mut().filter(|r| r.tied_ms == 0) {
            r.tied_ms = now_ms.max(1);
        }
    }

    /// The ropes whose time is up at `now_ms`.
    pub fn expired(&self, now_ms: u64) -> Vec<u64> {
        self.ropes
            .iter()
            .filter(|r| r.tied_ms != 0 && now_ms.saturating_sub(r.tied_ms) >= ROPE_LIFE_MS)
            .map(|r| r.id)
            .collect()
    }


    /// Wave 2: the ropes whose time is up at `now_ms` and nobody rides now (`ridden`): a rope
    /// waits for its rider to get off.
    pub fn expired_free(&self, now_ms: u64, ridden: &std::collections::BTreeSet<u64>) -> Vec<u64> {
        self.expired(now_ms).into_iter().filter(|id| !ridden.contains(id)).collect()
    }
    pub fn count_of(&self, owner: u128) -> usize {
        let key = owner_key(owner);
        self.ropes.iter().filter(|r| r.owner == key).count()
    }
}

/// What a sweep finds at a rope's ends: whether each is loaded, and whether it is an anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndSeen {
    pub loaded: bool,
    pub anchor: bool,
}

/// A rope whose loaded end lost its anchor (broken, blown up, cleared) is broken.
pub fn broken(a: EndSeen, b: EndSeen) -> bool {
    (a.loaded && !a.anchor) || (b.loaded && !b.anchor)
}

/// Z3: a ride from the end the player stands near (feet at `at`) to the other: None beyond
/// NEAR_END.
pub fn end_ride(rope: &Rope, at: [f64; 3]) -> Option<(End, End)> {
    let (end, d) = rope.nearer_end(at);
    (d <= NEAR_END).then_some((end, end.other()))
}

/// Where along the rope (0 at A, 1 at B) a look from `eye` along unit `dir` grabs it, and how
/// far from the eye: the closest approach of the look's stretch GRAB_FROM..GRAB_REACH and the
/// rope's line of hands, when within GRAB_RADIUS.
pub fn grab(eye: [f64; 3], dir: [f64; 3], rope: &Rope) -> Option<(f64, f64)> {
    let p0 = add(eye, scale(dir, GRAB_FROM));
    let d1 = scale(dir, GRAB_REACH - GRAB_FROM);
    let (q0, q1) = (hands(rope.a), hands(rope.b));
    let d2 = sub(q1, q0);
    let (s, t) = closest(p0, d1, q0, d2);
    let gap = distance(add(p0, scale(d1, s)), add(q0, scale(d2, t)));
    (gap <= GRAB_RADIUS).then_some((t, GRAB_FROM + s * (GRAB_REACH - GRAB_FROM)))
}

/// The parameters (s on p0 + s·d1, t on q0 + t·d2, both in 0..1) of the closest points of two
/// segments (Ericson, Real-Time Collision Detection, 5.1.9).
fn closest(p0: [f64; 3], d1: [f64; 3], q0: [f64; 3], d2: [f64; 3]) -> (f64, f64) {
    let r = sub(p0, q0);
    let (a, e, f) = (dot(d1, d1), dot(d2, d2), dot(d2, r));
    if a <= 1e-12 && e <= 1e-12 {
        return (0.0, 0.0);
    }
    if a <= 1e-12 {
        return (0.0, (f / e).clamp(0.0, 1.0));
    }
    let c = dot(d1, r);
    if e <= 1e-12 {
        return ((-c / a).clamp(0.0, 1.0), 0.0);
    }
    let b = dot(d1, d2);
    let denom = a * e - b * b;
    let mut s = if denom > 1e-12 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
    let mut t = (b * s + f) / e;
    if t < 0.0 {
        t = 0.0;
        s = (-c / a).clamp(0.0, 1.0);
    } else if t > 1.0 {
        t = 1.0;
        s = ((b - c) / a).clamp(0.0, 1.0);
    }
    (s, t)
}

/// Z3 from a grab at `t`: the feet's point on the rope and the end the ride goes to, the one the
/// look `dir` points along; a grab within END_SNAP of that end goes to the other.
pub fn grab_ride(rope: &Rope, t: f64, dir: [f64; 3]) -> ([f64; 3], End) {
    let (fa, fb) = (feet(rope.a), feet(rope.b));
    let along = sub(fb, fa);
    let at = add(fa, scale(along, t.clamp(0.0, 1.0)));
    let mut to = if dot(dir, along) >= 0.0 { End::B } else { End::A };
    if distance(at, feet(rope.at(to))) < END_SNAP {
        to = to.other();
    }
    (at, to)
}

/// A straight way through `points` at `speed` blocks a second, points closer than a tenth of a
/// block to the one before left out (the route's timing divides by each stretch).
pub fn way(points: &[[f64; 3]], speed: f64) -> Route {
    let mount = crate::geom::Face::YPos;
    let mut kept: Vec<[f64; 3]> = Vec::new();
    for &p in points {
        if kept.last().is_none_or(|l| distance(*l, p) >= 0.1) {
            kept.push(p);
        }
    }
    let mut times = Vec::with_capacity(kept.len());
    let mut t = 0.0;
    for (i, p) in kept.iter().enumerate() {
        if i > 0 {
            t += distance(kept[i - 1], *p) / speed;
        }
        times.push(t);
    }
    Route { points: kept.into_iter().map(|at| Point { at, mount, speed }).collect(), times }
}

/// How the harpoon holds a mob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    /// The mob is pulled to the player.
    Mob,
    /// The mob is too big: the player is pulled to it.
    Player,
}

/// G5 on mobs: a mob of `width` and `height` (blocks) is pulled, a big one pulls the player.
pub fn hold(width: f64, height: f64) -> Hold {
    if width >= BIG_WIDTH || height >= BIG_HEIGHT { Hold::Player } else { Hold::Mob }
}

/// Where a pulled mob (`width` wide, feet at `mob`) stops in front of the player (feet at
/// `player`): PULL_GAP past its half width from the player, towards where it was, at the
/// player's feet level.
pub fn mob_stop(player: [f64; 3], mob: [f64; 3], width: f64) -> [f64; 3] {
    toward(player, mob, PULL_GAP + width / 2.0)
}

/// Where the player stops before a big mob: PULL_GAP past its half width, on its side, at its
/// feet level.
pub fn player_stop(player: [f64; 3], mob: [f64; 3], width: f64) -> [f64; 3] {
    toward(mob, player, PULL_GAP + width / 2.0)
}

/// `gap` blocks from `from` flat towards `to`, at `from`'s height (`from` itself when `to` is
/// right over it).
fn toward(from: [f64; 3], to: [f64; 3], gap: f64) -> [f64; 3] {
    let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
    let flat = dx.hypot(dz);
    if flat < 1e-9 {
        return from;
    }
    let k = gap.min(flat) / flat;
    [from[0] + dx * k, from[1], from[2] + dz * k]
}

/// Points every half block from `from` to `to`, `to` last: where a pulled mob is checked for room.
pub fn samples(from: [f64; 3], to: [f64; 3]) -> Vec<[f64; 3]> {
    let n = (distance(from, to) * 2.0).ceil().max(1.0) as usize;
    (1..=n).map(|i| add(from, scale(sub(to, from), i as f64 / n as f64))).collect()
}

/// How many of `points` in a row, from the first, have room (`free`): a pulled mob goes only as
/// far as the last of them.
pub fn free_run(points: &[[f64; 3]], free: impl Fn([f64; 3]) -> bool) -> usize {
    points.iter().take_while(|p| free(**p)).count()
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A world of the given cells, air elsewhere.
    fn world(cells: &[(Pos, Cell)]) -> impl Fn(Pos) -> Cell {
        let map: HashMap<Pos, Cell> = cells.iter().copied().collect();
        move |p| map.get(&p).copied().unwrap_or(Cell::Air)
    }

    fn line(n: i32) -> Vec<Pos> {
        (0..n).map(|x| [x, 65, 0]).collect()
    }

    #[test]
    fn the_aim_puts_the_anchor_on_a_firm_block_with_room_over_it() {
        let w = world(&[([5, 65, 0], Cell::Solid)]);
        assert_eq!(aim(&line(10), &w), Aim::Valid { block: [5, 65, 0], anchor: [5, 66, 0] });
        let roofed = world(&[([5, 65, 0], Cell::Solid), ([5, 66, 0], Cell::Solid)]);
        assert_eq!(aim(&line(10), &roofed), Aim::NoRoom([5, 65, 0]));
        let grassy = world(&[([5, 65, 0], Cell::Solid), ([5, 66, 0], Cell::Grass)]);
        assert!(matches!(aim(&line(10), &grassy), Aim::Valid { .. }));
    }

    #[test]
    fn the_aim_passes_water_and_grass_and_stops_at_thin_blocks_and_anchors() {
        let w = world(&[([2, 65, 0], Cell::Through), ([3, 65, 0], Cell::Grass), ([4, 65, 0], Cell::Thin)]);
        assert_eq!(aim(&line(10), &w), Aim::NoRoom([4, 65, 0]));
        let a = world(&[([4, 65, 0], Cell::Anchor)]);
        assert_eq!(aim(&line(10), &a), Aim::Anchor([4, 65, 0]));
        assert_eq!(aim(&line(10), world(&[])), Aim::Far);
        // The eye's own cell never counts.
        let inside = world(&[([0, 65, 0], Cell::Solid)]);
        assert_eq!(aim(&line(3), &inside), Aim::Far);
    }

    #[test]
    fn the_start_anchor_stands_at_the_feet_or_over_a_slab() {
        let ground = world(&[([0, 63, 0], Cell::Solid)]);
        assert_eq!(start_cell([0.5, 64.0, 0.5], &ground), Some([0, 64, 0]));
        // On a slab the feet are in the slab's cell: the anchor goes over it.
        let slab = world(&[([0, 64, 0], Cell::Solid)]);
        assert_eq!(start_cell([0.5, 64.5, 0.5], &slab), Some([0, 65, 0]));
        // In the air, nothing.
        assert_eq!(start_cell([0.5, 70.0, 0.5], world(&[])), None);
        // Short grass gives way to it, water does not.
        let grass = world(&[([0, 63, 0], Cell::Solid), ([0, 64, 0], Cell::Grass)]);
        assert_eq!(start_cell([0.5, 64.0, 0.5], &grass), Some([0, 64, 0]));
        let wet = world(&[([0, 63, 0], Cell::Solid), ([0, 64, 0], Cell::Through)]);
        assert_eq!(start_cell([0.5, 64.0, 0.5], &wet), None);
    }

    #[test]
    fn a_rope_comes_down_five_minutes_after_it_was_tied() {
        let mut ropes = Ropes::default();
        let (id, _) = ropes.tie(5, "o", [0, 64, 0], [0, 64, 9]);
        assert!(ropes.expired(u64::MAX).is_empty(), "not stamped yet");
        ropes.stamp(1_000);
        assert!(ropes.expired(1_000 + ROPE_LIFE_MS - 1).is_empty());
        assert_eq!(ropes.expired(1_000 + ROPE_LIFE_MS), vec![id]);
        let ridden = std::collections::BTreeSet::from([id]);
        assert!(ropes.expired_free(1_000 + ROPE_LIFE_MS, &ridden).is_empty(), "a ridden rope waits for its rider");
        assert_eq!(ropes.expired_free(1_000 + ROPE_LIFE_MS, &Default::default()), vec![id]);
        ropes.stamp(9_999_999);
        assert_eq!(ropes.get(id).map(|r| r.tied_ms), Some(1_000), "a stamped rope keeps its time");
        let old: Ropes = serde_json::from_str(r#"{"next":1,"ropes":[{"id":1,"owner":"x","dimension":"o","a":[0,0,0],"b":[1,0,0]}]}"#).expect("an older store reads");
        assert_eq!(old.ropes[0].tied_ms, 0);
    }

    #[test]
    fn the_ghost_stands_where_the_anchor_goes() {
        let b = ghost_boxes([10, 64, -3]);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].0, [10.3125, 64.0, -2.6875]);
        assert_eq!(b[2].1, [10.625, 64.75, -2.375]);
    }

    #[test]
    fn a_player_keeps_four_ropes_and_the_oldest_goes() {
        let mut ropes = Ropes::default();
        let me = 7u128;
        for i in 0..MAX_ROPES as i32 {
            let (_, gone) = ropes.tie(me, "o", [i, 64, 0], [i, 64, 9]);
            assert!(gone.is_empty());
        }
        ropes.tie(8, "o", [50, 64, 0], [50, 64, 9]);
        let (id, gone) = ropes.tie(me, "o", [9, 64, 0], [9, 64, 9]);
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].a, [0, 64, 0]);
        assert_eq!(ropes.count_of(me), MAX_ROPES);
        assert_eq!(ropes.count_of(8), 1);
        assert!(ropes.get(id).is_some());
        assert!(ropes.at("o", [0, 64, 0]).is_none());
        assert_eq!(ropes.at("o", [9, 64, 9]).map(|r| r.id), Some(id));
        assert!(ropes.at("n", [9, 64, 9]).is_none());
    }

    #[test]
    fn the_store_reads_back_as_written() {
        let mut ropes = Ropes::default();
        ropes.tie(u128::MAX, "minecraft:overworld", [1, 2, 3], [4, 5, 6]);
        let json = serde_json::to_string(&ropes).expect("writes");
        assert_eq!(serde_json::from_str::<Ropes>(&json).expect("reads"), ropes);
    }

    #[test]
    fn a_rope_with_a_lost_anchor_is_broken_unless_its_chunk_is_away() {
        let fine = EndSeen { loaded: true, anchor: true };
        let away = EndSeen { loaded: false, anchor: false };
        let lost = EndSeen { loaded: true, anchor: false };
        assert!(!broken(fine, fine));
        assert!(!broken(fine, away));
        assert!(broken(lost, away));
        assert!(broken(fine, lost));
    }

    fn rope() -> Rope {
        Rope { id: 1, owner: owner_key(1), dimension: "o".into(), a: [0, 64, 0], b: [20, 70, 0], tied_ms: 0 }
    }

    #[test]
    fn a_ride_starts_at_the_nearer_end_within_reach() {
        let r = rope();
        assert_eq!(end_ride(&r, [2.0, 64.0, 0.5]), Some((End::A, End::B)));
        assert_eq!(end_ride(&r, [19.0, 70.0, 1.0]), Some((End::B, End::A)));
        assert_eq!(end_ride(&r, [10.0, 64.0, 0.5]), None);
        assert_eq!(r.end_at([20, 70, 0]), Some(End::B));
        assert_eq!(r.nearer_end([0.5, 64.0, 0.5]), (End::A, 0.0));
    }

    #[test]
    fn the_look_grabs_the_rope_and_rides_where_it_points() {
        let r = rope();
        // Halfway along, standing under it and looking up at it towards B.
        let mid = add(scale(add(feet(r.a), feet(r.b)), 0.5), [0.0, 0.0, -1.5]);
        let eye = add(mid, [0.0, 1.62, 0.0]);
        let target = add(scale(add(hands(r.a), hands(r.b)), 0.5), [0.0, 0.0, 0.0]);
        let d = sub(target, eye);
        let len = dot(d, d).sqrt();
        let dir = scale(d, 1.0 / len);
        let (t, from_eye) = grab(eye, dir, &r).expect("grabbed");
        assert!((t - 0.5).abs() < 0.05, "t {t}");
        assert!((from_eye - len).abs() < 0.2);
        let (at, to) = grab_ride(&r, t, [1.0, 0.0, 0.0]);
        assert_eq!(to, End::B);
        assert!((at[0] - 10.25).abs() < 1.0);
        assert_eq!(grab_ride(&r, t, [-1.0, 0.0, 0.0]).1, End::A);
        // Looking away grabs nothing.
        assert_eq!(grab(eye, [0.0, 0.0, -1.0], &r), None);
    }

    #[test]
    fn standing_at_the_start_the_look_grabs_only_along_the_rope() {
        let r = rope();
        let eye = add(feet(r.a), [0.0, 1.62, 0.0]);
        let along = sub(hands(r.b), hands(r.a));
        let n = dot(along, along).sqrt();
        let (t, _) = grab(eye, scale(along, 1.0 / n), &r).expect("along the rope");
        // At the start end: the ride goes to B whatever the look says.
        assert_eq!(grab_ride(&r, t.min(0.01), [-1.0, 0.0, 0.0]).1, End::B);
        assert_eq!(grab(eye, [-1.0, 0.0, 0.0], &r), None);
        assert_eq!(grab(eye, [0.0, 0.0, 1.0], &r), None);
    }

    #[test]
    fn a_way_leaves_out_points_on_top_of_each_other() {
        let w = way(&[[0.0, 64.0, 0.0], [0.0, 64.0, 0.05], [8.0, 64.0, 0.0]], 8.0);
        assert_eq!(w.points.len(), 2);
        assert!((w.seconds() - 1.0).abs() < 1e-6);
        assert_eq!(w.end(), [8.0, 64.0, 0.0]);
    }

    #[test]
    fn small_mobs_are_pulled_and_big_ones_pull_the_player() {
        assert_eq!(hold(0.6, 1.95), Hold::Mob); // a zombie
        assert_eq!(hold(0.9, 1.4), Hold::Mob); // a cow
        assert_eq!(hold(1.4, 2.7), Hold::Player); // an iron golem
        assert_eq!(hold(1.95, 2.2), Hold::Player); // a ravager
    }

    #[test]
    fn a_pulled_mob_stops_in_front_of_the_player_and_the_player_before_a_big_mob() {
        let near = |a: [f64; 3], b: [f64; 3]| distance(a, b) < 1e-9;
        let me = [0.0, 64.0, 0.0];
        let mob = [10.0, 66.0, 0.0];
        assert!(near(mob_stop(me, mob, 0.6), [1.3, 64.0, 0.0]));
        assert!(near(player_stop(me, mob, 1.4), [8.3, 66.0, 0.0]));
        // A mob closer than its gap stops at its own distance, at the player's level.
        assert!(near(mob_stop(me, [1.0, 64.0, 0.0], 0.6), [1.0, 64.0, 0.0]));
        assert!(near(mob_stop(me, [0.0, 70.0, 0.0], 0.6), me));
    }

    #[test]
    fn a_pull_goes_only_as_far_as_there_is_room() {
        let s = samples([10.0, 64.0, 0.0], [0.0, 64.0, 0.0]);
        assert_eq!(s.len(), 20);
        assert_eq!(*s.last().expect("some"), [0.0, 64.0, 0.0]);
        assert_eq!(free_run(&s, |p| p[0] > 4.9), 10);
        assert_eq!(free_run(&s, |_| true), 20);
    }
}
