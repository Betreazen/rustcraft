//! The living vein (spec 4.9, guide `vein`): a block of ore a minute grows in a free cell within 2
//! of it, for 2000 E and a stone from its slots. Which ore is chance: common ores often, rare ones
//! seldom; each rarity module (`module_rare`) doubles the chances of the rare ones. Its ore gives no
//! miner's experience (X6): the placed block is told to Path-of-craft-tree as `place`.

pub const STEP_TICKS: u64 = 1200;
pub const BLOCK_ENERGY: f64 = 2000.0;
pub const REACH: i32 = 2;

/// The ores and their chances in percent (guide `vein`); the last six are the rare ones the rarity
/// module makes twice as likely (guide `mod_rare`: gold, lapis, quartz, emerald, diamond, debris).
pub const ORES: [(&str, f64, bool); 10] = [
    ("minecraft:coal_ore", 30.0, false),
    ("minecraft:copper_ore", 25.0, false),
    ("minecraft:iron_ore", 20.0, false),
    ("minecraft:redstone_ore", 8.0, false),
    ("minecraft:lapis_ore", 6.0, true),
    ("minecraft:gold_ore", 6.0, true),
    ("minecraft:nether_quartz_ore", 3.0, true),
    ("minecraft:emerald_ore", 1.0, true),
    ("minecraft:diamond_ore", 0.8, true),
    ("minecraft:ancient_debris", 0.2, true),
];

/// Whether `item` feeds it: stone for ore, sand and gravel for the stratum module.
pub fn is_rock(item: &str) -> bool {
    matches!(
        item,
        "minecraft:cobblestone" | "minecraft:stone" | "minecraft:cobbled_deepslate" | "minecraft:deepslate" | "minecraft:sand" | "minecraft:gravel"
    )
}

/// The ores the pick module can choose, one a toggle of its window (ancient debris is not one).
pub const PICKS: [&str; 9] = [
    "minecraft:coal_ore",
    "minecraft:copper_ore",
    "minecraft:iron_ore",
    "minecraft:redstone_ore",
    "minecraft:lapis_ore",
    "minecraft:gold_ore",
    "minecraft:nether_quartz_ore",
    "minecraft:emerald_ore",
    "minecraft:diamond_ore",
];
/// The pick module's price: the block costs what it costs without it (guide `mod_pick`: the module
/// itself is the dear part; `mod_rare`: "темп роста и цена блока не меняются").
pub const PICK_FACTOR: f64 = 1.0;
/// ST3: the stratum grows a block in 120 s for 3000 E.
pub const STRATUM_TICKS: u64 = 2400;
pub const STRATUM_ENERGY: f64 = 3000.0;

/// What grows from `rock` with the stratum module: suspicious sand from sand, gravel from gravel.
pub fn stratum(rock: &str) -> Option<&'static str> {
    match rock {
        "minecraft:sand" => Some("minecraft:suspicious_sand"),
        "minecraft:gravel" => Some("minecraft:suspicious_gravel"),
        _ => None,
    }
}

/// Slice 18 (guide `mod_stratum`): with the stratum module the vein's window offers sand or gravel;
/// the vein takes the chosen one and grows its suspicious block.
pub const STRATUM_ROCKS: [&str; 2] = ["minecraft:sand", "minecraft:gravel"];

/// The rock the stratum takes for window choice `choice` (0 sand, 1 gravel; sand for any other).
pub fn stratum_rock(choice: usize) -> &'static str {
    STRATUM_ROCKS.get(choice).copied().unwrap_or(STRATUM_ROCKS[0])
}

/// Slice 18, wave 2 (guide `mod_stratum`: «и таблицу находок»): with the stratum module the
/// window's toggles are the two rocks, then the archaeology table's find tables; each group is a
/// choice of one.
pub const STRATUM_TABLES: [&str; 5] = crate::utility::ARCHEO_TABLES;

/// The rock (0 sand, 1 gravel) and the find table chosen by the stratum window's `toggles`; the
/// first of each when none is on.
pub fn stratum_choice(toggles: &[bool]) -> (usize, usize) {
    let rocks = STRATUM_ROCKS.len();
    let rock = toggles.iter().take(rocks).position(|t| *t).unwrap_or(0);
    let table = toggles
        .iter()
        .skip(rocks)
        .take(STRATUM_TABLES.len())
        .position(|t| *t)
        .unwrap_or(0);
    (rock, table)
}

/// A press on the stratum window's toggle `i`: that one on, the others of its group off.
pub fn stratum_press(toggles: &[bool], i: usize) -> Vec<bool> {
    let rocks = STRATUM_ROCKS.len();
    let n = rocks + STRATUM_TABLES.len();
    let mut t = toggles.to_vec();
    t.resize(n.max(t.len()), false);
    let group = if i < rocks { 0..rocks } else { rocks..n };
    if !group.contains(&i) {
        return t;
    }
    for k in group {
        t[k] = k == i;
    }
    t
}

/// The find table of the stratum window's choice.
pub fn stratum_table(toggles: &[bool]) -> &'static str {
    STRATUM_TABLES[stratum_choice(toggles).1]
}

/// Whether `rock` feeds ore (not the stratum).
pub fn is_stone(rock: &str) -> bool {
    is_rock(rock) && stratum(rock).is_none()
}

/// 0.21 (round 32): what the magistral brings the vein: the rock it grows from now, stone
/// without the stratum module, with it the rock its window chose (`toggles`). Before, it took
/// sand and gravel it never used without the module, and stone it never used with it. A hopper
/// and a hand still give it any rock (`is_rock`).
pub fn net_takes(stratum: bool, toggles: &[bool], item: &str) -> bool {
    if stratum {
        item == stratum_rock(stratum_choice(toggles).0)
    } else {
        is_stone(item)
    }
}

/// The ore a draw `r` (0..1) gives with `rare` rarity modules.
pub fn ore(r: f64, rare: u32) -> &'static str {
    let k = 2f64.powi(rare.min(4) as i32);
    let weight = |w: f64, is_rare: bool| if is_rare { w * k } else { w };
    let total: f64 = ORES.iter().map(|(_, w, rr)| weight(*w, *rr)).sum();
    let mut at = r.clamp(0.0, 0.999_999) * total;
    for (id, w, rr) in ORES {
        let w = weight(w, rr);
        if at < w {
            return id;
        }
        at -= w;
    }
    ORES[0].0
}

/// The cells within reach round it, nearest first.
pub fn cells() -> Vec<[i32; 3]> {
    let mut out: Vec<[i32; 3]> = (-REACH..=REACH)
        .flat_map(|x| (-REACH..=REACH).flat_map(move |y| (-REACH..=REACH).map(move |z| [x, y, z])))
        .filter(|c| *c != [0, 0, 0])
        .collect();
    out.sort_by_key(|c| c[0].abs() + c[1].abs() + c[2].abs());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_magistral_brings_the_vein_the_rock_it_grows_from() {
        // 0.21: without the stratum module it kept the sand and gravel that rode to the storage.
        for stone in ["minecraft:cobblestone", "minecraft:stone", "minecraft:deepslate"] {
            assert!(net_takes(false, &[], stone), "{stone}");
            assert!(!net_takes(true, &[], stone), "{stone}: the stratum grows from sand or gravel");
        }
        for rock in STRATUM_ROCKS {
            assert!(!net_takes(false, &[], rock), "{rock}");
            assert!(is_rock(rock), "{rock}: a hopper still gives it");
        }
        assert!(net_takes(true, &[], "minecraft:sand"), "sand is the first choice");
        assert!(!net_takes(true, &[], "minecraft:gravel"));
        let gravel = stratum_press(&[], 1);
        assert!(net_takes(true, &gravel, "minecraft:gravel"));
        assert!(!net_takes(true, &gravel, "minecraft:sand"));
        assert!(!net_takes(false, &[], "minecraft:dirt"));
    }

    #[test]
    fn the_stratum_window_chooses_a_rock_and_a_table() {
        assert_eq!(stratum_choice(&[]), (0, 0));
        let t = stratum_press(&[], 1);
        let t = stratum_press(&t, 2 + 3);
        assert_eq!(stratum_choice(&t), (1, 3));
        assert_eq!(stratum_table(&t), crate::utility::ARCHEO_TABLES[3]);
        // A table press leaves the rock; another rock leaves the table.
        let t = stratum_press(&t, 0);
        assert_eq!(stratum_choice(&t), (0, 3));
    }

    #[test]
    fn common_ores_come_first_and_rarity_shifts_the_chances() {
        assert_eq!(ore(0.0, 0), "minecraft:coal_ore");
        assert_eq!(ore(0.999, 0), "minecraft:ancient_debris");
        // Without the module a draw at 0.8 is iron; with two it lands on the rare ones.
        assert_eq!(ore(0.70, 0), "minecraft:iron_ore");
        assert!(ORES.iter().any(|(id, _, rare)| *rare && *id == ore(0.70, 2)));
        assert_eq!(cells().len(), 124);
        assert!(is_rock("minecraft:cobblestone") && !is_rock("minecraft:dirt"));
        assert_eq!(stratum("minecraft:gravel"), Some("minecraft:suspicious_gravel"));
        assert!(is_stone("minecraft:stone") && !is_stone("minecraft:sand"));
    }

    #[test]
    fn the_stratum_takes_the_rock_its_window_chose() {
        assert_eq!(stratum_rock(0), "minecraft:sand");
        assert_eq!(stratum_rock(1), "minecraft:gravel");
        assert_eq!(stratum_rock(7), "minecraft:sand");
        for r in STRATUM_ROCKS {
            assert!(stratum(r).is_some());
        }
    }

    #[test]
    fn redstone_is_not_rare_and_the_pick_costs_no_more() {
        // Guide `mod_rare` names gold, lapis, quartz, emerald, diamond and ancient debris only.
        let rare: Vec<&str> = ORES.iter().filter(|o| o.2).map(|o| o.0).collect();
        assert_eq!(
            rare,
            ["minecraft:lapis_ore", "minecraft:gold_ore", "minecraft:nether_quartz_ore", "minecraft:emerald_ore", "minecraft:diamond_ore", "minecraft:ancient_debris"]
        );
        // Guide `mod_pick`/`mod_rare`: the block's price does not change.
        assert_eq!(PICK_FACTOR, 1.0);
    }
}
