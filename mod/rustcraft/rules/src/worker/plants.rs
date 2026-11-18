//! What the reaper understands (spec 17, round 18): five groups of plants, each with its rule.
//! `plan` looks at one cell of the reaper's zone and says what to break, what to pick and what to
//! plant again; the bundle does it and the drops go to the hold.
//!
//! - Aged crops (wheat, carrots, potatoes, beetroot, nether wart, torchflower, pitcher plant):
//!   picked ripe, sown again from the hold (H3: nothing to sow leaves the cell empty).
//! - Empty soil (0.18.5, the owner's field test): farmland, or soul sand for nether wart, with air
//!   over it in the zone is sown from the hold (`sow`): the seed of the crop growing next to it,
//!   else the first seed of the hold that grows there and the filter lets through.
//! - Ground to till (0.18.5, the owner: «работники сами готовят землю»): grass, dirt or a dirt
//!   path with air over it in the zone becomes farmland and is sown in the same pass, when the
//!   hold has a seed for it; ground near water first (`hydrated`).
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

/// Whether the reaper's filter (H6, guide `tool_reaper`: «Фильтр в окне шасси оставляет только
/// нужные растения, например одни тыквы») lets it take the plant `block`: an empty filter takes
/// all; otherwise the plant's block, its crop or its seed is in it.
pub fn wanted(block: &str, filter: &[String]) -> bool {
    if filter.is_empty() {
        return true;
    }
    let crop = match block {
        "minecraft:carrots" => "minecraft:carrot",
        "minecraft:potatoes" => "minecraft:potato",
        "minecraft:beetroots" => "minecraft:beetroot",
        "minecraft:cocoa" => "minecraft:cocoa_beans",
        SWEET_BERRY_BUSH => "minecraft:sweet_berries",
        "minecraft:cave_vines" | "minecraft:cave_vines_plant" => "minecraft:glow_berries",
        "minecraft:kelp_plant" => "minecraft:kelp",
        "minecraft:pitcher_crop" => "minecraft:pitcher_plant",
        b => b,
    };
    let seed = AGED.iter().find(|a| a.block == block).map(|a| a.seed);
    filter
        .iter()
        .any(|f| f == block || f == crop || Some(f.as_str()) == seed)
}

/// What the reaper sows (0.18.5): the seed item, the crop block it becomes and the soil it needs.
const SOWN: [(&str, &str, &str); 7] = [
    ("minecraft:wheat_seeds", "minecraft:wheat", FARMLAND),
    ("minecraft:carrot", "minecraft:carrots", FARMLAND),
    ("minecraft:potato", "minecraft:potatoes", FARMLAND),
    ("minecraft:beetroot_seeds", "minecraft:beetroots", FARMLAND),
    (TORCHFLOWER_SEED, TORCHFLOWER_CROP, FARMLAND),
    ("minecraft:pitcher_pod", "minecraft:pitcher_crop", FARMLAND),
    ("minecraft:nether_wart", "minecraft:nether_wart", SOUL_SAND),
];
const SOUL_SAND: &str = "minecraft:soul_sand";

/// The seed a growing plant `block` comes from, if the reaper sows it.
fn seed_of_plant(block: &str) -> Option<&'static str> {
    if block == TORCHFLOWER {
        return Some(TORCHFLOWER_SEED);
    }
    SOWN.iter()
        .find(|(_, crop, _)| *crop == block)
        .map(|(seed, _, _)| *seed)
}

/// Whether the filter lets the reaper sow `seed` of `crop`: an empty filter sows all; otherwise
/// the seed, the plant it grows into or its crop is named (`wanted`).
fn sows(seed: &str, crop: &str, filter: &[String]) -> bool {
    filter.is_empty()
        || filter.iter().any(|f| f == seed)
        || wanted(crop, filter)
        || (crop == TORCHFLOWER_CROP && wanted(TORCHFLOWER, filter))
}

/// What the reaper finds at a cell when it sows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sowing {
    /// No empty soil there, or soil its filter sows nothing on.
    Nothing,
    /// Empty soil and no seed in the hold for it («Нечем засеять»).
    NoSeed,
    /// The crop to set there, paid with one of its seed.
    Sow(Replant),
    /// The ground under it tilled to farmland first, then the crop set.
    Till(Replant),
}

/// What the reaper tills to farmland: the ground a hoe tills in the base game, but coarse dirt.
const TILLED: [&str; 3] = ["minecraft:grass_block", "minecraft:dirt", "minecraft:dirt_path"];

/// Whether the reaper tills `block` to farmland when it has a seed for it.
pub fn tills(block: &str) -> bool {
    TILLED.contains(&block)
}

/// Whether farmland at `soil` is wet: water within 4 blocks across, on its level or one up
/// (the base game's `FarmBlock.isNearWater`). The reaper tills such ground first.
pub fn hydrated(view: &dyn BlockView, soil: Pos) -> bool {
    (-4..=4).any(|dx| {
        (-4..=4).any(|dz| {
            (0..=1).any(|dy| view.block([soil[0] + dx, soil[1] + dy, soil[2] + dz]) == "minecraft:water")
        })
    })
}

/// 0.18.5: what the reaper sows at `pos` of its zone, an air cell over farmland (or soul sand for
/// nether wart), or over grass, dirt or a dirt path, which it tills first (`Sowing::Till`).
/// `seeds` are the seed items of its hold in slot order. The seed is the one of the
/// crop growing next to `pos` (one or two cells along a row), else the first of `seeds` that grows
/// on this soil; a filter (H6) sows only what it lets through.
pub fn sow(view: &dyn BlockView, pos: Pos, seeds: &[String], filter: &[String]) -> Sowing {
    let here = view.block(pos);
    if here != "minecraft:air" && here != "minecraft:cave_air" {
        return Sowing::Nothing;
    }
    let found = view.block(step(pos, Face::YNeg));
    let till = tills(&found);
    let soil = if till { FARMLAND } else { found.as_str() };
    let fits: Vec<(&str, &str)> = SOWN
        .iter()
        .filter(|(seed, crop, on)| *on == soil && sows(seed, crop, filter))
        .map(|(seed, crop, _)| (*seed, *crop))
        .collect();
    if fits.is_empty() {
        return Sowing::Nothing;
    }
    let usable = |seed: &str| seeds.iter().any(|s| s == seed) && fits.iter().any(|f| f.0 == seed);
    let neighbour: Option<&str> = (1..=2)
        .flat_map(|d| {
            [Face::XNeg, Face::XPos, Face::ZNeg, Face::ZPos]
                .into_iter()
                .map(move |f| super::moved(pos, f.offset(), d))
        })
        .filter_map(|p| seed_of_plant(&view.block(p)))
        .find(|seed| usable(seed));
    let first = || seeds.iter().map(String::as_str).find(|s| usable(s));
    let Some(seed) = neighbour.or_else(first) else {
        return Sowing::NoSeed;
    };
    let Some(&(seed, crop)) = fits.iter().find(|f| f.0 == seed) else {
        return Sowing::NoSeed;
    };
    let r = Replant {
        pos,
        block: crop.to_owned(),
        props: vec![("age".into(), "0".into())],
        seed: seed.to_owned(),
    };
    if till { Sowing::Till(r) } else { Sowing::Sow(r) }
}

/// Whether two cells of a column are the same plant (kelp's two blocks are one plant).
fn same_column(a: &str, b: &str) -> bool {
    let kelp = |s: &str| s == "minecraft:kelp" || s == "minecraft:kelp_plant";
    a == b || (kelp(a) && kelp(b))
}

fn age(view: &dyn BlockView, pos: Pos) -> Option<u32> {
    view.prop(pos, "age")?.parse().ok()
}

/// How far down a column is followed to find its base.
const COLUMN_REACH: usize = 64;

/// What the reaper does at `pos`, a cell of its zone; `bottom` and `top` are the lowest and the
/// highest y its zone reaches (H4: columns are cut within it). None when there is nothing ripe
/// there.
pub fn plan(view: &dyn BlockView, pos: Pos, bottom: i32, top: i32) -> Option<Harvest> {
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
        // A column is planned from its top cell, or from the zone's top when it grows higher, and
        // cut top down to its base or the zone's bottom, whichever is higher: the base stays. So
        // a reaper reaches a column from any side of it: over a field, hanging over kelp from a
        // ceiling rail or riding up a bamboo on a wall rail.
        let above = view.block(step(pos, Face::YPos));
        let is_top = !same_column(&above, &block);
        if pos[1] > top || !(is_top || pos[1] == top) {
            return None;
        }
        let mut base = pos;
        for _ in 0..COLUMN_REACH {
            let under = step(base, Face::YNeg);
            if !same_column(&view.block(under), &block) {
                break;
            }
            base = under;
        }
        let lowest = (base[1] + 1).max(bottom);
        let breaks: Vec<Pos> = (lowest..=pos[1]).rev().map(|y| [pos[0], y, pos[2]]).collect();
        if breaks.is_empty() {
            return None;
        }
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
        let h = plan(&w, [0, 1, 0], 0, 3).unwrap();
        assert_eq!(h.group, Group::Aged);
        assert_eq!(h.breaks, vec![[0, 1, 0]]);
        assert_eq!(h.replant.unwrap().seed, "minecraft:wheat_seeds");
        assert_eq!(plan(&w, [1, 1, 0], 0, 3), None);
    }

    #[test]
    fn a_column_is_cut_over_its_base_top_down_within_the_zone() {
        // spec 17 example: cane 3 high gives 2, the base stays; a zone up to y 3 plans it there.
        let mut w = World::default();
        for y in 1..=4 {
            w.set([0, y, 0], "minecraft:sugar_cane", &[]);
        }
        let h = plan(&w, [0, 3, 0], 1, 3).unwrap();
        assert_eq!(h.breaks, vec![[0, 3, 0], [0, 2, 0]]);
        for y in [1, 2, 4] {
            assert_eq!(plan(&w, [0, y, 0], 1, 3), None, "only one cell plans it: y {y}");
        }
        let mut lone = World::default();
        lone.set([0, 1, 0], "minecraft:cactus", &[]);
        assert_eq!(plan(&lone, [0, 1, 0], 0, 3), None);
    }

    #[test]
    fn a_column_is_cut_from_its_top_inside_the_zone() {
        // A bamboo 6 high beside a wall rail at y 4, its zone y 3..5: the top is out of reach, so
        // the zone's top cell plans it down to the zone's bottom.
        let mut w = World::default();
        for y in 1..=6 {
            w.set([0, y, 0], "minecraft:bamboo", &[]);
        }
        assert_eq!(plan(&w, [0, 5, 0], 3, 5).unwrap().breaks, vec![[0, 5, 0], [0, 4, 0], [0, 3, 0]]);
        // Riding higher its zone holds the top: cut from it down to y 5.
        assert_eq!(plan(&w, [0, 6, 0], 5, 7).unwrap().breaks, vec![[0, 6, 0], [0, 5, 0]]);
    }

    #[test]
    fn kelp_under_a_ceiling_rail_is_reached() {
        // Kelp from the sea floor at y 0 to y 6; a reaper hanging from a ceiling rail at y 8
        // reaches y 6..8 and cuts the kelp's top block, the base far below stays.
        let mut w = World::default();
        for y in 0..6 {
            w.set([0, y, 0], "minecraft:kelp_plant", &[]);
        }
        w.set([0, 6, 0], "minecraft:kelp", &[]);
        assert_eq!(plan(&w, [0, 6, 0], 6, 8).unwrap().breaks, vec![[0, 6, 0]]);
        assert_eq!(plan(&w, [0, 6, 0], 0, 8).unwrap().breaks.len(), 6, "all over the base");
    }

    #[test]
    fn kelp_is_one_column_of_two_blocks() {
        let mut w = World::default();
        w.set([0, 1, 0], "minecraft:kelp_plant", &[]);
        w.set([0, 2, 0], "minecraft:kelp_plant", &[]);
        w.set([0, 3, 0], "minecraft:kelp", &[]);
        assert_eq!(plan(&w, [0, 3, 0], 1, 5).unwrap().breaks.len(), 2);
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
        assert_eq!(plan(&w, [1, 1, 0], 0, 3).unwrap().group, Group::Stem);
        assert_eq!(plan(&w, [0, 1, 0], 0, 3), None, "the stem stays");
        assert_eq!(plan(&w, [5, 1, 0], 0, 3), None, "a placed pumpkin stays");
    }

    #[test]
    fn berries_are_picked_and_the_bush_stays() {
        let mut w = World::default();
        w.set([0, 1, 0], SWEET_BERRY_BUSH, &[("age", "3")]);
        w.set([1, 1, 0], SWEET_BERRY_BUSH, &[("age", "1")]);
        w.set([2, 1, 0], "minecraft:cave_vines", &[("berries", "true")]);
        let h = plan(&w, [0, 1, 0], 0, 3).unwrap();
        assert!(h.breaks.is_empty());
        assert_eq!(h.picks[0].count, 2);
        assert_eq!(h.picks[0].set, ("age".into(), "1".into()));
        assert_eq!(plan(&w, [1, 1, 0], 0, 3), None);
        assert_eq!(
            plan(&w, [2, 1, 0], 0, 3).unwrap().picks[0].item,
            "minecraft:glow_berries"
        );
    }

    #[test]
    fn ripe_cocoa_is_planted_again_facing_the_same_trunk() {
        let mut w = World::default();
        w.set([0, 1, 0], COCOA, &[("age", "2"), ("facing", "west")]);
        let r = plan(&w, [0, 1, 0], 0, 3).unwrap().replant.unwrap();
        assert!(r.props.contains(&("facing".into(), "west".into())));
        assert_eq!(r.seed, "minecraft:cocoa_beans");
    }

    #[test]
    fn a_filtered_reaper_takes_only_pumpkins() {
        let pumpkins = vec!["minecraft:pumpkin".to_owned()];
        assert!(wanted("minecraft:pumpkin", &pumpkins));
        for other in ["minecraft:wheat", "minecraft:melon", "minecraft:cocoa"] {
            assert!(!wanted(other, &pumpkins), "{other}");
        }
        assert!(wanted("minecraft:wheat", &[]), "no filter: all");
        let crops = vec!["minecraft:carrot".to_owned(), "minecraft:wheat_seeds".to_owned()];
        assert!(wanted("minecraft:carrots", &crops) && wanted("minecraft:wheat", &crops));
        assert!(wanted("minecraft:kelp_plant", &["minecraft:kelp".to_owned()]));
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
            assert_eq!(plan(&w, [i as i32, 1, 0], 0, 3), None);
        }
    }

    #[test]
    fn a_torchflower_on_farmland_needs_its_seed() {
        let mut w = World::default();
        w.set([0, 0, 0], FARMLAND, &[]);
        w.set([0, 1, 0], TORCHFLOWER, &[]);
        w.set([2, 1, 0], TORCHFLOWER, &[]);
        assert_eq!(
            plan(&w, [0, 1, 0], 0, 3).unwrap().replant.unwrap().seed,
            TORCHFLOWER_SEED
        );
        assert_eq!(plan(&w, [2, 1, 0], 0, 3), None, "a wild one stays");
        assert!(is_seed(TORCHFLOWER_SEED) && is_seed("minecraft:wheat_seeds"));
    }

    fn seeds(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn sown(s: Sowing) -> Option<(String, String)> {
        match s {
            Sowing::Sow(r) => Some((r.block, r.seed)),
            _ => None,
        }
    }

    #[test]
    fn empty_farmland_is_sown_from_the_hold() {
        // The owner's field test: seeds in the hold, empty farmland beside the way.
        let mut w = World::default();
        w.set([0, 0, 0], FARMLAND, &[]);
        let hold = seeds(&["minecraft:wheat", "minecraft:wheat_seeds"]);
        assert_eq!(
            sown(sow(&w, [0, 1, 0], &hold, &[])),
            Some(("minecraft:wheat".into(), "minecraft:wheat_seeds".into()))
        );
        let Sowing::Sow(r) = sow(&w, [0, 1, 0], &hold, &[]) else { panic!() };
        assert_eq!(r.props, vec![("age".to_owned(), "0".to_owned())]);
    }

    #[test]
    fn the_seed_of_the_crop_next_to_it_goes_first() {
        let mut w = World::default();
        for x in 0..4 {
            w.set([x, 0, 0], FARMLAND, &[]);
        }
        w.set([2, 1, 0], "minecraft:carrots", &[("age", "2")]);
        w.set([3, 1, 0], "minecraft:carrots", &[("age", "2")]);
        let hold = seeds(&["minecraft:wheat_seeds", "minecraft:carrot"]);
        assert_eq!(
            sown(sow(&w, [1, 1, 0], &hold, &[])).unwrap().0,
            "minecraft:carrots",
            "a carrot row grows on"
        );
        assert_eq!(
            sown(sow(&w, [0, 1, 0], &hold, &[])).unwrap().0,
            "minecraft:carrots",
            "two cells away still counts"
        );
        // No carrot left: the first seed of the hold.
        let wheat_only = seeds(&["minecraft:beetroot_seeds", "minecraft:wheat_seeds"]);
        assert_eq!(
            sown(sow(&w, [1, 1, 0], &wheat_only, &[])).unwrap().1,
            "minecraft:beetroot_seeds"
        );
    }

    #[test]
    fn nether_wart_goes_on_soul_sand_only() {
        let mut w = World::default();
        w.set([0, 0, 0], SOUL_SAND, &[]);
        w.set([1, 0, 0], FARMLAND, &[]);
        let hold = seeds(&["minecraft:wheat_seeds", "minecraft:nether_wart"]);
        assert_eq!(
            sown(sow(&w, [0, 1, 0], &hold, &[])).unwrap().0,
            "minecraft:nether_wart"
        );
        assert_eq!(
            sown(sow(&w, [1, 1, 0], &hold, &[])).unwrap().0,
            "minecraft:wheat"
        );
        assert_eq!(
            sow(&w, [1, 1, 0], &seeds(&["minecraft:nether_wart"]), &[]),
            Sowing::NoSeed,
            "nether wart is no seed for farmland"
        );
    }

    #[test]
    fn every_seed_grows_its_crop() {
        let mut w = World::default();
        w.set([0, 0, 0], FARMLAND, &[]);
        for (seed, crop) in [
            ("minecraft:wheat_seeds", "minecraft:wheat"),
            ("minecraft:carrot", "minecraft:carrots"),
            ("minecraft:potato", "minecraft:potatoes"),
            ("minecraft:beetroot_seeds", "minecraft:beetroots"),
            (TORCHFLOWER_SEED, TORCHFLOWER_CROP),
            ("minecraft:pitcher_pod", "minecraft:pitcher_crop"),
        ] {
            assert_eq!(
                sown(sow(&w, [0, 1, 0], &seeds(&[seed]), &[])),
                Some((crop.to_owned(), seed.to_owned()))
            );
            assert!(is_seed(seed), "the unload dock keeps {seed}");
        }
    }

    #[test]
    fn nothing_to_sow_and_nothing_there() {
        let mut w = World::default();
        w.set([0, 0, 0], FARMLAND, &[]);
        w.set([1, 0, 0], "minecraft:grass_block", &[]);
        w.set([2, 0, 0], FARMLAND, &[]);
        w.set([2, 1, 0], "minecraft:wheat", &[("age", "1")]);
        assert_eq!(sow(&w, [0, 1, 0], &[], &[]), Sowing::NoSeed);
        let hold = seeds(&["minecraft:wheat_seeds"]);
        w.set([1, 1, 0], "minecraft:short_grass", &[]);
        assert_eq!(sow(&w, [1, 1, 0], &hold, &[]), Sowing::Nothing, "grass grows over the ground");
        assert_eq!(sow(&w, [2, 1, 0], &hold, &[]), Sowing::Nothing, "it grows already");
        assert_eq!(sow(&w, [0, 2, 0], &hold, &[]), Sowing::Nothing, "air over air");
    }

    #[test]
    fn a_filtered_reaper_sows_only_what_its_filter_takes() {
        let mut w = World::default();
        w.set([0, 0, 0], FARMLAND, &[]);
        let hold = seeds(&["minecraft:wheat_seeds", "minecraft:carrot"]);
        let carrots = vec!["minecraft:carrot".to_owned()];
        assert_eq!(
            sown(sow(&w, [0, 1, 0], &hold, &carrots)).unwrap().0,
            "minecraft:carrots"
        );
        let pumpkins = vec!["minecraft:pumpkin".to_owned()];
        assert_eq!(
            sow(&w, [0, 1, 0], &hold, &pumpkins),
            Sowing::Nothing,
            "a pumpkin reaper sows no wheat and lacks nothing"
        );
        let wheat = vec!["minecraft:wheat".to_owned()];
        assert_eq!(
            sow(&w, [0, 1, 0], &seeds(&["minecraft:carrot"]), &wheat),
            Sowing::NoSeed
        );
        let flowers = vec![TORCHFLOWER.to_owned()];
        assert_eq!(
            sown(sow(&w, [0, 1, 0], &seeds(&[TORCHFLOWER_SEED]), &flowers)).unwrap().0,
            TORCHFLOWER_CROP
        );
    }

    #[test]
    fn grass_beside_the_way_is_tilled_and_sown_with_a_seed_for_it() {
        // 0.18.5, the owner: «работники сами готовят землю».
        let mut w = World::default();
        for (x, ground) in [(0, "minecraft:grass_block"), (1, "minecraft:dirt"), (2, "minecraft:dirt_path")] {
            w.set([x, 0, 0], ground, &[]);
        }
        w.set([3, 0, 0], "minecraft:coarse_dirt", &[]);
        w.set([4, 0, 0], "minecraft:stone", &[]);
        let hold = seeds(&["minecraft:carrot"]);
        for x in 0..3 {
            match sow(&w, [x, 1, 0], &hold, &[]) {
                Sowing::Till(r) => assert_eq!((r.block.as_str(), r.seed.as_str()), ("minecraft:carrots", "minecraft:carrot")),
                other => panic!("x {x}: {other:?}"),
            }
        }
        assert_eq!(sow(&w, [3, 1, 0], &hold, &[]), Sowing::Nothing, "coarse dirt stays");
        assert_eq!(sow(&w, [4, 1, 0], &hold, &[]), Sowing::Nothing);
        // No seed for it: it says so and leaves the grass; nether wart grows on no farmland.
        assert_eq!(sow(&w, [0, 1, 0], &[], &[]), Sowing::NoSeed);
        assert_eq!(sow(&w, [0, 1, 0], &seeds(&["minecraft:nether_wart"]), &[]), Sowing::NoSeed);
        // A filter to pumpkins tills nothing.
        assert_eq!(sow(&w, [0, 1, 0], &hold, &["minecraft:pumpkin".to_owned()]), Sowing::Nothing);
    }

    #[test]
    fn ground_near_water_is_wet() {
        let mut w = World::default();
        w.set([4, 0, 4], "minecraft:water", &[]);
        assert!(hydrated(&w, [0, 0, 0]));
        assert!(hydrated(&w, [0, -1, 0]), "water one up");
        assert!(!hydrated(&w, [0, 1, 0]), "water below");
        assert!(!hydrated(&w, [-1, 0, 0]), "five away");
    }
}
