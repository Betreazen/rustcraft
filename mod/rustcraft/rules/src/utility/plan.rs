//! The holo-projector and the building machine (spec 22, BU1; slice 18, guide `projector`,
//! `builder`). The projector remembers the blocks of an area two wrench corners set, up to
//! 32 × 32 × 32 (9 × 9 and 8 up from it until corners are set), each with its state (a stair's
//! facing, a door's half); a plan turns and mirrors; the building machine places the missing
//! cells from the storage, and gets builder's experience only for cells of the plan, each cell
//! once for good. A plan packs into a palette of states and runs of cells, which a block entity
//! and an item keep in NBT (a JSON string would pass NBT's 64 KiB string limit for a big plan).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The default area from the projector's block: 9 × 9 across, 1..=8 up.
pub const HALF: i32 = 4;
pub const HEIGHT: i32 = 8;
/// The longest side of an area two corners set.
pub const MAX_SIDE: i32 = 32;
/// The most cells a plan keeps.
pub const MAX_CELLS: usize = (MAX_SIDE * MAX_SIDE * MAX_SIDE) as usize;
/// The block-state properties a plan keeps, those placing and turning need.
pub const PROPERTIES: [&str; 18] = [
    "facing", "half", "axis", "type", "shape", "hinge", "open", "rotation", "face", "attachment",
    "part", "north", "east", "south", "west", "up", "waterlogged", "orientation",
];

/// One block of a plan, relative to the projector.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cell {
    pub at: [i32; 3],
    /// The block's state: its id and the kept properties, `minecraft:oak_stairs[facing=east,half=top]`.
    pub block: String,
}

impl Cell {
    /// The block's id without its properties (the item the storage holds for it).
    pub fn id(&self) -> &str {
        state_id(&self.block)
    }
}

/// A remembered building.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub cells: Vec<Cell>,
}

/// An area, as offsets from the projector, both corners inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Area {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl Default for Area {
    fn default() -> Area {
        Area { min: [-HALF, 1, -HALF], max: [HALF, HEIGHT, HALF] }
    }
}

impl Area {
    /// The area between two corners (world cells) as offsets from the projector at `projector`;
    /// none when a side is longer than MAX_SIDE.
    pub fn of(a: [i32; 3], b: [i32; 3], projector: [i32; 3]) -> Option<Area> {
        let mut min = [0; 3];
        let mut max = [0; 3];
        for k in 0..3 {
            min[k] = a[k].min(b[k]) - projector[k];
            max[k] = a[k].max(b[k]) - projector[k];
            if max[k] - min[k] + 1 > MAX_SIDE {
                return None;
            }
        }
        Some(Area { min, max })
    }

    /// Its offsets bottom up, so a block stands on what is under it.
    pub fn cells(self) -> impl Iterator<Item = [i32; 3]> {
        (self.min[1]..=self.max[1]).flat_map(move |y| {
            (self.min[0]..=self.max[0]).flat_map(move |x| (self.min[2]..=self.max[2]).map(move |z| [x, y, z]))
        })
    }

    pub fn size(self) -> [i32; 3] {
        [0, 1, 2].map(|k| self.max[k] - self.min[k] + 1)
    }
}

/// The offsets of the default area, in building order.
pub fn area() -> impl Iterator<Item = [i32; 3]> {
    Area::default().cells()
}

/// A state's block id.
pub fn state_id(state: &str) -> &str {
    state.split_once('[').map_or(state, |(id, _)| id)
}

/// A state's properties, in the order written.
pub fn state_props(state: &str) -> Vec<(String, String)> {
    let Some((_, rest)) = state.split_once('[') else {
        return Vec::new();
    };
    rest.trim_end_matches(']')
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// A state string from an id and its properties.
pub fn state_string(id: &str, props: &[(String, String)]) -> String {
    if props.is_empty() {
        return id.to_owned();
    }
    let p: Vec<String> = props.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{id}[{}]", p.join(","))
}

const SIDES: [&str; 4] = ["north", "east", "south", "west"];

fn turn_side(side: &str, quarter: u8, mirror: bool) -> String {
    let Some(mut i) = SIDES.iter().position(|s| *s == side) else {
        return side.to_owned();
    };
    if mirror {
        // Mirror across x: east and west swap.
        i = [0, 3, 2, 1][i];
    }
    SIDES[(i + usize::from(quarter)) % 4].to_owned()
}

/// A state turned by `quarter` quarter turns clockwise (seen from above) after a mirror across
/// x when `mirror`.
pub fn turn_state(state: &str, quarter: u8, mirror: bool) -> String {
    let props: Vec<(String, String)> = state_props(state)
        .into_iter()
        .map(|(k, v)| {
            let v = match k.as_str() {
                "facing" => turn_side(&v, quarter, mirror),
                "axis" if quarter % 2 == 1 => match v.as_str() {
                    "x" => "z".to_owned(),
                    "z" => "x".to_owned(),
                    _ => v,
                },
                "rotation" => {
                    let r: u8 = v.parse().unwrap_or(0);
                    let r = if mirror { (16 - r) % 16 } else { r };
                    ((r + 4 * quarter) % 16).to_string()
                }
                "hinge" if mirror => if v == "left" { "right".to_owned() } else { "left".to_owned() },
                "shape" if mirror => match v.as_str() {
                    "inner_left" => "inner_right".to_owned(),
                    "inner_right" => "inner_left".to_owned(),
                    "outer_left" => "outer_right".to_owned(),
                    "outer_right" => "outer_left".to_owned(),
                    _ => v,
                },
                _ => v,
            };
            (k, v)
        })
        .collect();
    // The four sides of a fence or a wall move with the turn.
    let mut sides: BTreeMap<String, String> = BTreeMap::new();
    let mut rest = Vec::new();
    for (k, v) in props {
        if SIDES.contains(&k.as_str()) {
            sides.insert(turn_side(&k, quarter, mirror), v);
        } else {
            rest.push((k, v));
        }
    }
    let mut all = rest;
    for s in SIDES {
        if let Some(v) = sides.remove(s) {
            all.push((s.to_owned(), v));
        }
    }
    state_string(state_id(state), &all)
}

fn turn_at(at: [i32; 3], quarter: u8, mirror: bool) -> [i32; 3] {
    let [mut x, y, mut z] = at;
    if mirror {
        x = -x;
    }
    for _ in 0..quarter % 4 {
        (x, z) = (-z, x);
    }
    [x, y, z]
}

/// A packed plan: the box its cells sit in, a palette of states and runs of cells over the box in
/// y, x, z order (each run a palette place plus one, 0 for no block, and its length).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Packed {
    pub min: [i32; 3],
    pub size: [i32; 3],
    pub palette: Vec<String>,
    pub runs: Vec<i32>,
}

impl Plan {
    /// The plan of what stands in `area`: `block_at(offset)` names the block state there, air and
    /// fluids none.
    pub fn read_area(area: Area, mut block_at: impl FnMut([i32; 3]) -> Option<String>) -> Plan {
        let cells = area
            .cells()
            .filter_map(|at| block_at(at).map(|block| Cell { at, block }))
            .take(MAX_CELLS)
            .collect();
        Plan { cells }
    }

    /// The plan of the default area.
    pub fn read(block_at: impl FnMut([i32; 3]) -> Option<String>) -> Plan {
        Plan::read_area(Area::default(), block_at)
    }

    /// The cells whose block is not there now, in building order (`block_at` names the state
    /// there, compared by its id).
    pub fn missing<'a>(&'a self, mut block_at: impl FnMut([i32; 3]) -> Option<String> + 'a) -> impl Iterator<Item = &'a Cell> + 'a {
        self.cells
            .iter()
            .filter(move |c| block_at(c.at).as_deref().map(state_id) != Some(c.id()))
    }

    /// The plan turned `quarter` quarter turns about the projector and mirrored across x first
    /// when `mirror`; bottom up again.
    pub fn transformed(&self, quarter: u8, mirror: bool) -> Plan {
        let mut cells: Vec<Cell> = self
            .cells
            .iter()
            .map(|c| Cell { at: turn_at(c.at, quarter, mirror), block: turn_state(&c.block, quarter, mirror) })
            .collect();
        cells.sort_by_key(|c| (c.at[1], c.at[0], c.at[2]));
        Plan { cells }
    }

    /// How many of each block the cells `cells` take, by block id.
    pub fn counts<'a>(cells: impl Iterator<Item = &'a Cell>) -> BTreeMap<String, u32> {
        let mut out = BTreeMap::new();
        for c in cells {
            *out.entry(c.id().to_owned()).or_insert(0) += 1;
        }
        out
    }

    /// The layers (y offsets) the plan has, bottom up.
    pub fn layers(&self) -> Vec<i32> {
        self.cells.iter().map(|c| c.at[1]).collect::<BTreeSet<_>>().into_iter().collect()
    }

    pub fn pack(&self) -> Packed {
        if self.cells.is_empty() {
            return Packed::default();
        }
        let mut min = self.cells[0].at;
        let mut max = min;
        for c in &self.cells {
            for k in 0..3 {
                min[k] = min[k].min(c.at[k]);
                max[k] = max[k].max(c.at[k]);
            }
        }
        let size = [0, 1, 2].map(|k| max[k] - min[k] + 1);
        let mut palette: Vec<String> = Vec::new();
        let mut index: BTreeMap<&str, i32> = BTreeMap::new();
        let at: BTreeMap<[i32; 3], &str> = self.cells.iter().map(|c| (c.at, c.block.as_str())).collect();
        let mut runs: Vec<i32> = Vec::new();
        for y in 0..size[1] {
            for x in 0..size[0] {
                for z in 0..size[2] {
                    let v = match at.get(&[min[0] + x, min[1] + y, min[2] + z]) {
                        Some(b) => *index.entry(b).or_insert_with(|| {
                            palette.push((*b).to_owned());
                            palette.len() as i32
                        }),
                        None => 0,
                    };
                    match runs.len() {
                        n if n >= 2 && runs[n - 2] == v => runs[n - 1] += 1,
                        _ => runs.extend([v, 1]),
                    }
                }
            }
        }
        Packed { min, size, palette, runs }
    }

    /// What `pack` packed; a run past the box or the palette ends it.
    pub fn unpack(p: &Packed) -> Plan {
        let mut cells = Vec::new();
        let total = (p.size[0].max(0) * p.size[1].max(0) * p.size[2].max(0)) as usize;
        let mut i = 0usize;
        for run in p.runs.chunks_exact(2) {
            let (v, n) = (run[0], run[1].max(0) as usize);
            for _ in 0..n {
                if i >= total || i >= MAX_CELLS {
                    return Plan { cells };
                }
                if v > 0 {
                    let Some(block) = p.palette.get(v as usize - 1) else {
                        return Plan { cells };
                    };
                    let (sx, sz) = (p.size[0] as usize, p.size[2] as usize);
                    let y = i / (sx * sz);
                    let x = (i / sz) % sx;
                    let z = i % sz;
                    cells.push(Cell {
                        at: [p.min[0] + x as i32, p.min[1] + y as i32, p.min[2] + z as i32],
                        block: block.clone(),
                    });
                }
                i += 1;
            }
        }
        Plan { cells }
    }
}

/// The cells the building machine has built and been paid for (BU1): offsets from the projector,
/// kept in its block entity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Built(pub BTreeSet<[i32; 3]>);

impl Built {
    /// Whether placing the block of `at` pays builder's experience: only the first time.
    pub fn pays(&mut self, at: [i32; 3]) -> bool {
        self.0.insert(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_keeps_what_stands_and_names_what_is_missing() {
        let plan = Plan::read(|at| (at == [0, 1, 0] || at == [1, 1, 0]).then(|| "minecraft:stone".to_owned()));
        assert_eq!(plan.cells.len(), 2);
        let missing: Vec<_> = plan
            .missing(|at| (at == [0, 1, 0]).then(|| "minecraft:stone".to_owned()))
            .map(|c| c.at)
            .collect();
        assert_eq!(missing, vec![[1, 1, 0]]);
        assert_eq!(area().count(), 9 * 9 * 8);
    }

    #[test]
    fn two_corners_make_an_area_of_at_most_32() {
        let a = Area::of([10, 64, 10], [41, 70, 20], [9, 63, 9]).unwrap();
        assert_eq!(a.min, [1, 1, 1]);
        assert_eq!(a.size(), [32, 7, 11]);
        assert!(Area::of([0, 0, 0], [32, 0, 0], [0, 0, 0]).is_none(), "33 long");
    }

    #[test]
    fn a_plan_packs_into_runs_and_back() {
        let plan = Plan::read_area(Area { min: [0, 0, 0], max: [31, 3, 31] }, |at| {
            (at[1] == 0 || at == [5, 2, 7]).then(|| {
                if at[0] % 2 == 0 { "minecraft:stone".to_owned() } else { "minecraft:oak_stairs[facing=east,half=top]".to_owned() }
            })
        });
        let p = plan.pack();
        assert_eq!(p.palette.len(), 2);
        assert!(p.runs.len() < 2200, "runs, not a cell each: {}", p.runs.len());
        assert_eq!(Plan::unpack(&p), plan);
        assert_eq!(Plan::unpack(&Plan::default().pack()), Plan::default());
    }

    #[test]
    fn states_turn_and_mirror() {
        assert_eq!(turn_state("minecraft:oak_stairs[facing=north,half=top]", 1, false), "minecraft:oak_stairs[facing=east,half=top]");
        assert_eq!(turn_state("minecraft:oak_log[axis=x]", 1, false), "minecraft:oak_log[axis=z]");
        assert_eq!(turn_state("minecraft:oak_door[facing=east,hinge=left]", 0, true), "minecraft:oak_door[facing=west,hinge=right]");
        assert_eq!(turn_state("minecraft:oak_sign[rotation=2]", 1, false), "minecraft:oak_sign[rotation=6]");
        assert_eq!(
            turn_state("minecraft:oak_fence[north=true,east=false,south=false,west=false]", 1, false),
            "minecraft:oak_fence[north=false,east=true,south=false,west=false]"
        );
        let plan = Plan { cells: vec![Cell { at: [2, 1, 0], block: "minecraft:stone".to_owned() }] };
        assert_eq!(plan.transformed(1, false).cells[0].at, [0, 1, 2]);
        assert_eq!(plan.transformed(0, true).cells[0].at, [-2, 1, 0]);
        assert_eq!(state_id("minecraft:oak_stairs[facing=east]"), "minecraft:oak_stairs");
    }

    #[test]
    fn counts_and_layers() {
        let plan = Plan::read_area(Area { min: [0, 0, 0], max: [1, 1, 0] }, |_| Some("minecraft:stone".to_owned()));
        assert_eq!(Plan::counts(plan.cells.iter()).get("minecraft:stone"), Some(&4));
        assert_eq!(plan.layers(), vec![0, 1]);
    }

    #[test]
    fn each_cell_pays_once() {
        // BU1.
        let mut b = Built::default();
        assert!(b.pays([0, 1, 0]));
        assert!(!b.pays([0, 1, 0]));
    }
}
