//! What the reaper understands (spec 17, round 18): five groups of plants, each with its rule.
//! `plan` looks at one cell of the reaper's zone and says what to break, what to pick and what to
//! plant again; the bundle does it and the drops go to the hold.
//!
//! - Aged crops (wheat, carrots, potatoes, beetroot, nether wart, torchflower, pitcher plant):
//!   picked ripe, sown again from the hold (H3: nothing to sow leaves the cell empty).
//! - Columns (sugar cane, cactus, bamboo, kelp): everything over the lowest block, top down.
//! - Fruits on a stem (pumpkin, melon): the fruit an attached stem points at; the stem stays.
//! - Berries (sweet berries, glow berries): picked, the bush or vine stays.
//! - On a wall (cocoa): the ripe pod, planted again on the same trunk.
//!
//! H2: young crops, the bases of columns, stems and bushes are left alone. H7: chorus and fungi
//! are none of these and are never touched.

use super::BlockView;
use crate::geom::{Face, Pos, step};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Aged,
    Column,
    Stem,
    Berries,
    Wall,
}

/// A block to set after the harvest, paid with one `seed` from the hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replant {
    pub pos: Pos,
    pub block: String,
    pub props: Vec<(String, String)>,
    pub seed: String,
}

/// A berry picked without breaking: the item and count it gives and the property it sets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub pos: Pos,
    pub item: String,
    pub count: u32,
    pub set: (String, String),
}

/// What the reaper does with one plant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Harvest {
    pub group: Group,
    /// Blocks broken and their drops taken, in this order (columns top down).
    pub breaks: Vec<Pos>,
    pub picks: Vec<Pick>,
    pub replant: Option<Replant>,
}

impl Harvest {
    /// How many plant blocks it takes: what its time and energy count.
    pub fn size(&self) -> usize {
        (self.breaks.len() + self.picks.len()).max(1)
    }
}

/// Aged crops: the block, the age it is ripe at, and the seed that sows it again (the block to
/// plant, if not the same).
struct Aged {
    block: &'static str,
    ripe: u32,
    seed: &'static str,
    sown: &'static str,
}

const AGED: [Aged; 6] = [
    Aged {
        block: "minecraft:wheat",
        ripe: 7,
        seed: "minecraft:wheat_seeds",
        sown: "minecraft:wheat",
    },
    Aged {
        block: "minecraft:carrots",
        ripe: 7,
        seed: "minecraft:carrot",
        sown: "minecraft:carrots",
    },
    Aged {
        block: "minecraft:potatoes",
        ripe: 7,
        seed: "minecraft:potato",
        sown: "minecraft:potatoes",
    },
    Aged {
        block: "minecraft:beetroots",
        ripe: 3,
        seed: "minecraft:beetroot_seeds",
        sown: "minecraft:beetroots",
    },
    Aged {
        block: "minecraft:nether_wart",
        ripe: 3,
        seed: "minecraft:nether_wart",
        sown: "minecraft:nether_wart",
    },
    Aged {
        block: "minecraft:pitcher_crop",
        ripe: 4,
        seed: "minecraft:pitcher_pod",
        sown: "minecraft:pitcher_crop",
    },
];
/// A torchflower is ripe once its crop turned into the flower; on farmland it is a crop's.
const TORCHFLOWER: &str = "minecraft:torchflower";
const TORCHFLOWER_SEED: &str = "minecraft:torchflower_seeds";
const TORCHFLOWER_CROP: &str = "minecraft:torchflower_crop";
const FARMLAND: &str = "minecraft:farmland";
const COLUMNS: [&str; 5] = [
    "minecraft:sugar_cane",
    "minecraft:cactus",
    "minecraft:bamboo",
    "minecraft:kelp",
    "minecraft:kelp_plant",
];
/// Fruits and the attached stem that points at each.
const STEMS: [(&str, &str); 2] = [
    ("minecraft:pumpkin", "minecraft:attached_pumpkin_stem"),
    ("minecraft:melon", "minecraft:attached_melon_stem"),
];
const SWEET_BERRY_BUSH: &str = "minecraft:sweet_berry_bush";
const CAVE_VINES: [&str; 2] = ["minecraft:cave_vines", "minecraft:cave_vines_plant"];
const COCOA: &str = "minecraft:cocoa";

/// The items the reaper keeps in its hold to sow again, which the unload dock leaves.
pub fn is_seed(item: &str) -> bool {
    AGED.iter().any(|a| a.seed == item)
        || item == TORCHFLOWER_SEED
        || item == "minecraft:cocoa_beans"
}

/// Whether two cells of a column are the same plant (kelp's two blocks are one plant).
fn same_column(a: &str, b: &str) -> bool {
    let kelp = |s: &str| s == "minecraft:kelp" || s == "minecraft:kelp_plant";
    a == b || (kelp(a) && kelp(b))
}

fn age(view: &dyn BlockView, pos: Pos) -> Option<u32> {
    view.prop(pos, "age")?.parse().ok()
}

/// What the reaper does at `pos`, a cell of its zone; `top` is the highest y its zone reaches
/// (H4: columns are cut within it). None when there is nothing ripe there.
pub fn plan(view: &dyn BlockView, pos: Pos, top: i32) -> Option<Harvest> {
    let block = view.block(pos);
    let below = view.block(step(pos, Face::YNeg));
    if let Some(a) = AGED.iter().find(|a| a.block == block) {
        if a.block == "minecraft:pitcher_crop" && view.prop(pos, "half").as_deref() == Some("upper")
        {
            return None;
        }
        if age(view, pos)? < a.ripe {
            return None;
        }
        return Some(Harvest {
            group: Group::Aged,
            breaks: vec![pos],
            picks: Vec::new(),
            replant: Some(Replant {
                pos,
                block: a.sown.into(),
                props: vec![("age".into(), "0".into())],
                seed: a.seed.into(),
            }),
        });
    }
    if block == TORCHFLOWER && below == FARMLAND {
        return Some(Harvest {
            group: Group::Aged,
            breaks: vec![pos],
            picks: Vec::new(),
            replant: Some(Replant {
                pos,
                block: TORCHFLOWER_CROP.into(),
                props: vec![("age".into(), "0".into())],
                seed: TORCHFLOWER_SEED.into(),
            }),
        });
    }
    if COLUMNS.contains(&block.as_str()) {
        // Only the base plans its column: the cell under it is not the same plant.
        if same_column(&below, &block) {
            return None;
        }
        let mut breaks = Vec::new();
        let mut p = step(pos, Face::YPos);
        while p[1] <= top && same_column(&view.block(p), &block) {
            breaks.push(p);
            p = step(p, Face::YPos);
        }
        if breaks.is_empty() {
            return None;
        }
        breaks.reverse();
        return Some(Harvest {
            group: Group::Column,
            breaks,
            picks: Vec::new(),
            replant: None,
        });
    }
    if let Some((_, stem)) = STEMS.iter().find(|(fruit, _)| *fruit == block) {
        let attached = [Face::XNeg, Face::XPos, Face::ZNeg, Face::ZPos]
            .into_iter()
            .any(|f| {
                let s = step(pos, f);
                view.block(s) == *stem
                    && view.prop(s, "facing").as_deref() == Some(facing_name(f.opposite()))
            });
        return attached.then(|| Harvest {
            group: Group::Stem,
            breaks: vec![pos],
            picks: Vec::new(),
            replant: None,
        });
    }
    if block == SWEET_BERRY_BUSH {
        let a = age(view, pos)?;
        if a < 2 {
            return None;
        }
        // The base game gives 1–2 at age 2 and 2–3 at age 3; the reaper takes the lower count.
        let count = if a >= 3 { 2 } else { 1 };
        return Some(Harvest {
            group: Group::Berries,
            breaks: Vec::new(),
            picks: vec![Pick {
                pos,
                item: "minecraft:sweet_berries".into(),
                count,
                set: ("age".into(), "1".into()),
            }],
            replant: None,
        });
    }
    if CAVE_VINES.contains(&block.as_str()) {
        if view.prop(pos, "berries").as_deref() != Some("true") {
            return None;
        }
        return Some(Harvest {
            group: Group::Berries,
            breaks: Vec::new(),
            picks: vec![Pick {
                pos,
                item: "minecraft:glow_berries".into(),
                count: 1,
                set: ("berries".into(), "false".into()),
            }],
            replant: None,
        });
    }
    if block == COCOA {
        if age(view, pos)? < 2 {
            return None;
        }
        let facing = view.prop(pos, "facing").unwrap_or_else(|| "north".into());
        return Some(Harvest {
            group: Group::Wall,
            breaks: vec![pos],
            picks: Vec::new(),
            replant: Some(Replant {
                pos,
                block: COCOA.into(),
                props: vec![("age".into(), "0".into()), ("facing".into(), facing)],
                seed: "minecraft:cocoa_beans".into(),
            }),
        });
    }
    None
}

/// The `facing` value of a horizontal face.
pub fn facing_name(face: Face) -> &'static str {
    match face {
        Face::XNeg => "west",
        Face::XPos => "east",
        Face::ZNeg => "north",
        Face::ZPos => "south",
        Face::YNeg => "down",
        Face::YPos => "up",
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::World;
    use super::*;

    #[test]
    fn ripe_wheat_is_sown_again_and_young_wheat_is_left() {
        let mut w = World::default();
        w.set([0, 0, 0], "minecraft:farmland", &[]);
        w.set([0, 1, 0], "minecraft:wheat", &[("age", "7")]);
        w.set([1, 1, 0], "minecraft:wheat", &[("age", "3")]);
        let h = plan(&w, [0, 1, 0], 3).unwrap();
        assert_eq!(h.group, Group::Aged);
        assert_eq!(h.breaks, vec![[0, 1, 0]]);
        assert_eq!(h.replant.unwrap().seed, "minecraft:wheat_seeds");
        assert_eq!(plan(&w, [1, 1, 0], 3), None);
    }

    #[test]
    fn a_column_is_cut_over_its_base_top_down_within_the_zone() {
        // spec 17 example: cane 3 high gives 2, the base stays.
        let mut w = World::default();
        for y in 1..=4 {
            w.set([0, y, 0], "minecraft:sugar_cane", &[]);
        }
        let h = plan(&w, [0, 1, 0], 3).unwrap();
        assert_eq!(h.breaks, vec![[0, 3, 0], [0, 2, 0]]);
        assert_eq!(plan(&w, [0, 2, 0], 3), None, "only the base plans");
        let mut lone = World::default();
        lone.set([0, 1, 0], "minecraft:cactus", &[]);
        assert_eq!(plan(&lone, [0, 1, 0], 3), None);
    }

    #[test]
    fn kelp_is_one_column_of_two_blocks() {
        let mut w = World::default();
        w.set([0, 1, 0], "minecraft:kelp_plant", &[]);
        w.set([0, 2, 0], "minecraft:kelp_plant", &[]);
        w.set([0, 3, 0], "minecraft:kelp", &[]);
        assert_eq!(plan(&w, [0, 1, 0], 5).unwrap().breaks.len(), 2);
    }

    #[test]
    fn a_pumpkin_goes_only_when_a_stem_points_at_it() {
        let mut w = World::default();
        w.set([1, 1, 0], "minecraft:pumpkin", &[]);
        w.set(
            [0, 1, 0],
            "minecraft:attached_pumpkin_stem",
            &[("facing", "east")],
        );
        w.set([5, 1, 0], "minecraft:pumpkin", &[]);
        assert_eq!(plan(&w, [1, 1, 0], 3).unwrap().group, Group::Stem);
        assert_eq!(plan(&w, [0, 1, 0], 3), None, "the stem stays");
        assert_eq!(plan(&w, [5, 1, 0], 3), None, "a placed pumpkin stays");
    }

    #[test]
    fn berries_are_picked_and_the_bush_stays() {
        let mut w = World::default();
        w.set([0, 1, 0], SWEET_BERRY_BUSH, &[("age", "3")]);
        w.set([1, 1, 0], SWEET_BERRY_BUSH, &[("age", "1")]);
        w.set([2, 1, 0], "minecraft:cave_vines", &[("berries", "true")]);
        let h = plan(&w, [0, 1, 0], 3).unwrap();
        assert!(h.breaks.is_empty());
        assert_eq!(h.picks[0].count, 2);
        assert_eq!(h.picks[0].set, ("age".into(), "1".into()));
        assert_eq!(plan(&w, [1, 1, 0], 3), None);
        assert_eq!(
            plan(&w, [2, 1, 0], 3).unwrap().picks[0].item,
            "minecraft:glow_berries"
        );
    }

    #[test]
    fn ripe_cocoa_is_planted_again_facing_the_same_trunk() {
        let mut w = World::default();
        w.set([0, 1, 0], COCOA, &[("age", "2"), ("facing", "west")]);
        let r = plan(&w, [0, 1, 0], 3).unwrap().replant.unwrap();
        assert!(r.props.contains(&("facing".into(), "west".into())));
        assert_eq!(r.seed, "minecraft:cocoa_beans");
    }

    #[test]
    fn chorus_and_mushrooms_are_never_touched() {
        let mut w = World::default();
        for (i, b) in [
            "minecraft:chorus_flower",
            "minecraft:red_mushroom",
            "minecraft:crimson_fungus",
        ]
        .iter()
        .enumerate()
        {
            w.set([i as i32, 1, 0], b, &[("age", "5")]);
            assert_eq!(plan(&w, [i as i32, 1, 0], 3), None);
        }
    }

    #[test]
    fn a_torchflower_on_farmland_needs_its_seed() {
        let mut w = World::default();
        w.set([0, 0, 0], FARMLAND, &[]);
        w.set([0, 1, 0], TORCHFLOWER, &[]);
        w.set([2, 1, 0], TORCHFLOWER, &[]);
        assert_eq!(
            plan(&w, [0, 1, 0], 3).unwrap().replant.unwrap().seed,
            TORCHFLOWER_SEED
        );
        assert_eq!(plan(&w, [2, 1, 0], 3), None, "a wild one stays");
        assert!(is_seed(TORCHFLOWER_SEED) && is_seed("minecraft:wheat_seeds"));
    }
}
