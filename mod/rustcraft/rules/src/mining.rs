//! Slice 8: the miner (spec 4.8 D1–D5, 18.4 MN1; guide `miner`, `mod_area`, `mod_scout`,
//! `mod_storage`), the block placer and the block breaker (spec 4.3; guide `placer`, `breaker`).
//! The miner scans the square below it layer by layer down to the bottom of the world, digs each
//! wanted ore it finds and puts back the rock round it (D2). The placer puts a block of its own
//! into the cell in front of it; the breaker breaks the block in front of it and keeps the drops.
//! The bundle reads the world and breaks and places; these rules say what, where and when.

use crate::geom::{Face, Pos};
use crate::machines::processor::WorkMode;
use crate::modules::{self, Module};
use crate::tuning::Tuning;

/// The ores the miner's window lists (D5), each one switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ore {
    Coal,
    Copper,
    Iron,
    Gold,
    Redstone,
    Lapis,
    Diamond,
    Emerald,
    /// Nether quartz and ancient debris.
    Nether,
    /// An ore of another mod: always taken.
    Other,
}

impl Ore {
    /// The window's switches, in its order.
    pub const LISTED: [Ore; 9] = [
        Ore::Coal,
        Ore::Copper,
        Ore::Iron,
        Ore::Gold,
        Ore::Redstone,
        Ore::Lapis,
        Ore::Diamond,
        Ore::Emerald,
        Ore::Nether,
    ];

    /// The ore group of `block`, none for a block that is no ore: any `…_ore`, and ancient debris.
    pub fn of(block: &str) -> Option<Ore> {
        let name = block.rsplit(':').next().unwrap_or(block);
        if name == "ancient_debris" {
            return Some(Ore::Nether);
        }
        if !name.ends_with("_ore") {
            return None;
        }
        let words = [
            ("coal", Ore::Coal),
            ("copper", Ore::Copper),
            ("iron", Ore::Iron),
            ("gold", Ore::Gold),
            ("redstone", Ore::Redstone),
            ("lapis", Ore::Lapis),
            ("diamond", Ore::Diamond),
            ("emerald", Ore::Emerald),
            ("quartz", Ore::Nether),
        ];
        Some(
            words
                .iter()
                .find(|(w, _)| name.contains(w))
                .map_or(Ore::Other, |(_, o)| *o),
        )
    }

    /// Its bit in the saved set of switched-off ores; none for `Other`.
    pub fn bit(self) -> u16 {
        Ore::LISTED
            .iter()
            .position(|o| *o == self)
            .map_or(0, |i| 1 << i)
    }

    /// The word its window line and the lang key use.
    pub fn key(self) -> &'static str {
        match self {
            Ore::Coal => "coal",
            Ore::Copper => "copper",
            Ore::Iron => "iron",
            Ore::Gold => "gold",
            Ore::Redstone => "redstone",
            Ore::Lapis => "lapis",
            Ore::Diamond => "diamond",
            Ore::Emerald => "emerald",
            Ore::Nether => "nether",
            Ore::Other => "other",
        }
    }

    /// A block of the group, to show its switch with.
    pub fn icon(self) -> &'static str {
        match self {
            Ore::Coal => "minecraft:coal_ore",
            Ore::Copper => "minecraft:copper_ore",
            Ore::Iron => "minecraft:iron_ore",
            Ore::Gold => "minecraft:gold_ore",
            Ore::Redstone => "minecraft:redstone_ore",
            Ore::Lapis => "minecraft:lapis_ore",
            Ore::Diamond => "minecraft:diamond_ore",
            Ore::Emerald => "minecraft:emerald_ore",
            Ore::Nether => "minecraft:nether_quartz_ore",
            Ore::Other => "minecraft:stone",
        }
    }
}

/// D5: whether the miner takes `block`, with the ores of `off` switched off.
pub fn wanted(block: &str, off: u16) -> bool {
    Ore::of(block).is_some_and(|o| o.bit() & off == 0)
}

/// The rocks an ore may be set in (D2).
pub const ROCKS: [&str; 12] = [
    "minecraft:stone",
    "minecraft:deepslate",
    "minecraft:netherrack",
    "minecraft:tuff",
    "minecraft:andesite",
    "minecraft:diorite",
    "minecraft:granite",
    "minecraft:calcite",
    "minecraft:blackstone",
    "minecraft:basalt",
    "minecraft:smooth_basalt",
    "minecraft:end_stone",
];

/// D2: the rock that takes an ore's place: the commonest rock among its neighbours (the first of
/// `ROCKS` on a tie), else the ore's own: deepslate for a deepslate ore, netherrack for a Nether
/// one, stone otherwise.
pub fn rock_for(ore: &str, neighbours: &[String]) -> &'static str {
    let best = ROCKS
        .iter()
        .map(|r| (*r, neighbours.iter().filter(|n| n.as_str() == *r).count()))
        .filter(|(_, n)| *n > 0)
        .fold(None::<(&str, usize)>, |best, (r, n)| match best {
            Some((_, m)) if m >= n => best,
            _ => Some((r, n)),
        });
    if let Some((r, _)) = best {
        return r;
    }
    if ore.contains("deepslate") {
        "minecraft:deepslate"
    } else if ore.contains("nether") || ore.ends_with("ancient_debris") {
        "minecraft:netherrack"
    } else {
        "minecraft:stone"
    }
}

// ---- Modules and modes ----

/// (work, energy) factors of a work mode (A1) and the speed and economy modules in `slots` (A7, A9).
pub fn factors(mode: WorkMode, slots: &[Option<Module>], t: &Tuning) -> (f64, f64) {
    let (mut work, mut energy) = mode.factors();
    for _ in 0..modules::count(slots, Module::Speed) {
        work *= t.speed_work;
        energy *= t.speed_energy;
    }
    for _ in 0..modules::count(slots, Module::Economy) {
        work *= t.economy_work;
        energy *= t.economy_energy;
    }
    (work, energy)
}

/// The side of the miner's square: 11, 22 with an area module, 33 with two.
pub fn side(slots: &[Option<Module>], t: &Tuning) -> i32 {
    let n = modules::count(slots, Module::Area) as usize;
    t.miner_sides[n.min(t.miner_sides.len() - 1)]
}

/// The scan factor of the scout modules: ×2 each (guide `mod_scout`).
pub fn scout(slots: &[Option<Module>]) -> f64 {
    2f64.powi(modules::count(slots, Module::Scout) as i32)
}

/// The miner's slots: 9, ×2 a storage module (guide `mod_storage`), at most `miner_slots_max`.
pub fn slots_of(slots: &[Option<Module>], t: &Tuning) -> usize {
    let n = modules::count(slots, Module::Storage);
    (t.miner_slots << n.min(8)).min(t.miner_slots_max)
}

// ---- The miner ----

/// The square's offsets along one axis: `side` cells round the miner, one more on the low side
/// for an even side.
fn low(side: i32) -> i32 {
    -(side / 2)
}

/// How many cells the miner at `miner` scans, from the layer under it down to `min_y`.
pub fn cells(miner: Pos, side: i32, min_y: i32) -> u64 {
    let layers = (miner[1] - min_y).max(0) as u64;
    layers * (side as u64).pow(2)
}

/// The `index`th cell the miner scans: layer by layer from the one under it, row by row.
pub fn cell(miner: Pos, side: i32, min_y: i32, index: u64) -> Option<Pos> {
    let per = (side as u64).pow(2);
    let layer = (index / per) as i32;
    let y = miner[1] - 1 - layer;
    if y < min_y || per == 0 {
        return None;
    }
    let i = (index % per) as i32;
    Some([
        miner[0] + low(side) + i % side,
        y,
        miner[2] + low(side) + i / side,
    ])
}

/// Where the miner is in its work: the next cell to scan, the ore it digs and how far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Miner {
    pub cursor: u64,
    pub target: Option<Pos>,
    pub dig: f64,
    /// The part of a cell the scan has paid for.
    pub carry: f64,
}

/// What a tick of the miner asks the bundle to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Nothing,
    /// The ore at this cell comes out now.
    Dig(Pos),
}

impl Miner {
    /// D4: the square is worked out.
    pub fn exhausted(&self, total: u64) -> bool {
        self.target.is_none() && self.cursor >= total
    }

    /// What it asks, E/s: its draw while there is work and room for the ore (D3).
    pub fn request(&self, total: u64, room: bool, energy: f64, t: &Tuning) -> f64 {
        if self.exhausted(total) || !room {
            0.0
        } else {
            t.miner_energy * energy
        }
    }

    /// One tick on `share` of its request, at `work` speed and `scout` scan factor: it scans on
    /// until it finds a cell `wanted` says yes to, then digs it. `cell_at` names the cells.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        share: f64,
        work: f64,
        scout: f64,
        total: u64,
        t: &Tuning,
        mut cell_at: impl FnMut(u64) -> Option<Pos>,
        mut wanted: impl FnMut(Pos) -> bool,
    ) -> Step {
        if share <= 0.0 {
            return Step::Nothing;
        }
        if let Some(p) = self.target {
            self.dig += share * work / t.miner_dig_ticks.max(1.0);
            if self.dig >= 1.0 - 1e-9 {
                self.dig = 0.0;
                self.target = None;
                return Step::Dig(p);
            }
            return Step::Nothing;
        }
        self.carry += t.miner_scan_per_tick * share * work * scout;
        while self.carry >= 1.0 - 1e-9 && self.cursor < total {
            self.carry -= 1.0;
            let i = self.cursor;
            self.cursor += 1;
            if let Some(p) = cell_at(i)
                && wanted(p)
            {
                self.target = Some(p);
                self.carry = 0.0;
                break;
            }
        }
        if self.cursor >= total {
            self.carry = 0.0;
        }
        Step::Nothing
    }
}

/// Ticks a miner at `share`, `work` and `scout` takes to reach and dig the `index`th cell, from a
/// fresh start: what the self-test waits for.
pub fn ticks_to_dig(index: u64, share: f64, work: f64, scout: f64, t: &Tuning) -> u64 {
    let mut m = Miner::default();
    let mut ticks = 0;
    loop {
        ticks += 1;
        let step = m.tick(
            share,
            work,
            scout,
            index + 1,
            t,
            |i| Some([i as i32, 0, 0]),
            |p| p[0] as u64 == index,
        );
        if step != Step::Nothing || ticks > 1_000_000 {
            return ticks;
        }
    }
}

// ---- The placer and the breaker ----

/// The work of a placer or a breaker: one block each time its progress fills.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cycle {
    pub progress: f64,
}

impl Cycle {
    /// A tick on `share` of its draw at `work` speed, a block taking `ticks`: whether a block is due.
    pub fn tick(&mut self, share: f64, work: f64, ticks: f64) -> bool {
        self.progress += share.max(0.0) * work / ticks.max(1.0);
        if self.progress >= 1.0 - 1e-9 {
            self.progress = 0.0;
            true
        } else {
            false
        }
    }
}

/// Blocks no breaker breaks: what a player cannot break, portals, and RustCraft's own blocks
/// (their slots and settings would be lost).
pub const UNBREAKABLE: [&str; 13] = [
    "minecraft:bedrock",
    "minecraft:barrier",
    "minecraft:light",
    "minecraft:command_block",
    "minecraft:chain_command_block",
    "minecraft:repeating_command_block",
    "minecraft:structure_block",
    "minecraft:structure_void",
    "minecraft:jigsaw",
    "minecraft:end_portal_frame",
    "minecraft:end_portal",
    "minecraft:end_gateway",
    "minecraft:reinforced_deepslate",
];

/// Cells a breaker takes as empty and a placer as free.
pub const EMPTY: [&str; 7] = [
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:void_air",
    "minecraft:water",
    "minecraft:lava",
    "minecraft:bubble_column",
    "minecraft:nether_portal",
];

/// Whether the breaker breaks `block`.
pub fn breakable(block: &str) -> bool {
    !EMPTY.contains(&block) && !UNBREAKABLE.contains(&block) && !block.starts_with("rustcraft:")
}

/// Whether the placer may put a block into a cell holding `block`.
pub fn free(block: &str) -> bool {
    EMPTY[..3].contains(&block) || block == "minecraft:water"
}

/// The face a block's `facing` looks out of (the slice-7 blocks' state).
pub fn front(facing: &str) -> Option<Face> {
    match facing {
        "north" => Some(Face::ZNeg),
        "south" => Some(Face::ZPos),
        "west" => Some(Face::XNeg),
        "east" => Some(Face::XPos),
        _ => None,
    }
}

/// What a miner, a placer or a breaker shows (A4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Working,
    Slowed(u32),
    NoEnergy,
    /// D4: «Область выработана».
    Exhausted,
    /// «Некуда складывать добытое» / «Готовое некуда отдать».
    Full,
    /// The placer: «Нет блоков».
    NoBlocks,
    /// The placer: «Место занято».
    Taken,
    /// The breaker: nothing to break in front of it.
    Nothing,
}

impl Shown {
    /// A machine with work on `share` of its draw.
    pub fn working(share: f64) -> Shown {
        if share <= 1e-9 {
            Shown::NoEnergy
        } else if share < 1.0 - 1e-9 {
            Shown::Slowed(((share * 100.0).round() as u32).clamp(1, 99))
        } else {
            Shown::Working
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ores_are_known_by_their_names() {
        assert_eq!(Ore::of("minecraft:iron_ore"), Some(Ore::Iron));
        assert_eq!(
            Ore::of("minecraft:deepslate_diamond_ore"),
            Some(Ore::Diamond)
        );
        assert_eq!(Ore::of("minecraft:nether_gold_ore"), Some(Ore::Gold));
        assert_eq!(Ore::of("minecraft:nether_quartz_ore"), Some(Ore::Nether));
        assert_eq!(Ore::of("minecraft:ancient_debris"), Some(Ore::Nether));
        assert_eq!(Ore::of("othermod:tin_ore"), Some(Ore::Other));
        assert_eq!(Ore::of("minecraft:stone"), None);
        assert_eq!(Ore::of("minecraft:raw_iron_block"), None);
        // D5: all by default; coal switched off stays.
        assert!(wanted("minecraft:coal_ore", 0));
        assert!(!wanted("minecraft:coal_ore", Ore::Coal.bit()));
        assert!(wanted("othermod:tin_ore", u16::MAX));
    }

    #[test]
    fn an_ore_gives_way_to_the_rock_round_it() {
        let n = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            rock_for(
                "minecraft:iron_ore",
                &n(&["minecraft:stone", "minecraft:air"])
            ),
            "minecraft:stone"
        );
        assert_eq!(
            rock_for(
                "minecraft:iron_ore",
                &n(&[
                    "minecraft:deepslate",
                    "minecraft:deepslate",
                    "minecraft:stone"
                ])
            ),
            "minecraft:deepslate"
        );
        // Nothing round it but air and ores: by its own name.
        assert_eq!(
            rock_for("minecraft:deepslate_iron_ore", &n(&["minecraft:air"])),
            "minecraft:deepslate"
        );
        assert_eq!(
            rock_for("minecraft:nether_quartz_ore", &[]),
            "minecraft:netherrack"
        );
        assert_eq!(rock_for("minecraft:coal_ore", &[]), "minecraft:stone");
    }

    #[test]
    fn the_square_is_11_wide_and_grows_with_area_modules() {
        let t = Tuning::default();
        assert_eq!(side(&[], &t), 11);
        assert_eq!(side(&[Some(Module::Area)], &t), 22);
        assert_eq!(side(&[Some(Module::Area); 3], &t), 33);
        assert_eq!(slots_of(&[], &t), 9);
        assert_eq!(slots_of(&[Some(Module::Storage)], &t), 18);
        assert_eq!(slots_of(&[Some(Module::Storage); 2], &t), 27);
        assert_eq!(scout(&[Some(Module::Scout); 2]), 4.0);
    }

    #[test]
    fn the_scan_goes_down_layer_by_layer_inside_the_square() {
        let m = [100, 64, 100];
        assert_eq!(cells(m, 11, -64), 128 * 121);
        assert_eq!(cell(m, 11, -64, 0), Some([95, 63, 95]));
        assert_eq!(cell(m, 11, -64, 120), Some([105, 63, 105]));
        assert_eq!(cell(m, 11, -64, 121), Some([95, 62, 95]));
        assert_eq!(cell(m, 11, -64, cells(m, 11, -64)), None);
        // An even side has one more cell on the low side.
        assert_eq!(cell(m, 22, -64, 0), Some([89, 63, 89]));
        assert_eq!(cell(m, 22, -64, 21), Some([110, 63, 89]));
    }

    #[test]
    fn the_miner_finds_an_ore_digs_it_and_works_out_its_square() {
        let t = Tuning::default();
        let miner = [0, 10, 0];
        let total = cells(miner, 11, 0);
        let ore = [2, 7, -1];
        let mut m = Miner::default();
        let mut dug = None;
        let mut ticks = 0;
        while dug.is_none() && ticks < 1000 {
            ticks += 1;
            if let Step::Dig(p) = m.tick(
                1.0,
                1.0,
                1.0,
                total,
                &t,
                |i| cell(miner, 11, 0, i),
                |p| p == ore,
            ) {
                dug = Some(p);
            }
        }
        assert_eq!(dug, Some(ore));
        // Layers 9 and 8 (242 cells), then 4 rows and 8 cells of layer 7: 294 cells at 64 a tick
        // is 5 ticks, then 20 ticks of digging.
        assert_eq!(ticks, 25);
        let index = (0..total)
            .find(|&i| cell(miner, 11, 0, i) == Some(ore))
            .unwrap();
        assert_eq!(ticks_to_dig(index, 1.0, 1.0, 1.0, &t), 25);
        // Half the energy: twice as long.
        assert_eq!(ticks_to_dig(index, 0.5, 1.0, 1.0, &t), 50);
        while !m.exhausted(total) {
            m.tick(
                1.0,
                1.0,
                1.0,
                total,
                &t,
                |i| cell(miner, 11, 0, i),
                |_| false,
            );
        }
        assert_eq!(m.request(total, true, 1.0, &t), 0.0);
        let fresh = Miner::default();
        assert_eq!(fresh.request(total, false, 1.0, &t), 0.0);
        assert_eq!(fresh.request(total, true, 1.0, &t), 8.0);
    }

    #[test]
    fn modes_and_modules_set_the_pace_and_the_draw() {
        let t = Tuning::default();
        assert_eq!(factors(WorkMode::Normal, &[], &t), (1.0, 1.0));
        let (w, e) = factors(WorkMode::Boost, &[Some(Module::Speed)], &t);
        assert!((w - 4.0).abs() < 1e-9 && (e - 7.5).abs() < 1e-9);
        let mut c = Cycle::default();
        let fired: usize = (0..40).filter(|_| c.tick(1.0, 1.0, 20.0)).count();
        assert_eq!(fired, 2);
        assert_eq!(Shown::working(0.5), Shown::Slowed(50));
        assert_eq!(Shown::working(0.0), Shown::NoEnergy);
    }

    #[test]
    fn the_breaker_spares_bedrock_portals_and_rustcraft() {
        assert!(breakable("minecraft:cobblestone"));
        assert!(!breakable("minecraft:bedrock"));
        assert!(!breakable("minecraft:air"));
        assert!(!breakable("minecraft:water"));
        assert!(!breakable("rustcraft:housing"));
        assert!(free("minecraft:air"));
        assert!(!free("minecraft:stone"));
        assert_eq!(front("east"), Some(Face::XPos));
    }
}
