//! Reports to Path-of-craft-tree (spec 4.15 X1–X9, 7.1; `docs/design/path-of-craft-tree-contract.md`):
//! what a machine did and whose it is, as a `pct-machine` console command. Path-of-craft-tree
//! decides what it is worth. The machines' rules give events; this turns them into commands, so
//! the rules know nothing of the other mod.

use crate::geom::Pos;

/// The console command Path-of-craft-tree registers.
pub const COMMAND: &str = "pct-machine";
/// The console command SoulsCraft registers for its hunter book (spec 27.1): every kill a
/// RustCraft machine makes, whether Path-of-craft-tree is there or not.
pub const SOULS_COMMAND: &str = "soulscraft-kill";

/// A kill told to SoulsCraft: what made it (`turret`; the guard of plan slice 3 will say `guard`),
/// whose it is, the victim's id and kind and its block position at death.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoulsKill {
    pub source: &'static str,
    pub owner: u128,
    pub entity: u32,
    pub kind: String,
    pub pos: Pos,
}

impl SoulsKill {
    /// The command line, without a slash.
    pub fn command(&self) -> String {
        format!(
            "{SOULS_COMMAND} {} {:032x} {} {} {} {} {}",
            self.source, self.owner, self.entity, self.kind, self.pos[0], self.pos[1], self.pos[2]
        )
    }
}

/// The block a position at death lies in.
pub fn block_of(at: [f64; 3]) -> Pos {
    at.map(|c| c.floor() as i32)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Report {
    /// A saw, a reaper (X1) broke a block and took its drops; `tool` is `axe` or `hoe`.
    Break {
        owner: u128,
        dimension: String,
        pos: Pos,
        block: String,
        tool: &'static str,
    },
    /// A machine put a block into the world (X6): the miner's rock, the placer's block. No owner:
    /// Path-of-craft-tree only marks the place.
    Place {
        dimension: String,
        pos: Pos,
        block: String,
    },
    /// A turret killed a mob (X8), sent the tick it died.
    Kill {
        owner: u128,
        entity: u32,
        kind: String,
    },
    /// Slice 11: a butcher killed a farm animal (round 26), sent the tick it died; no `kill`.
    Slaughter {
        owner: u128,
        entity: u32,
        kind: String,
    },
    /// Slice 18 (cross): the electric furnace or the alloy smelter gave its result (the
    /// contract's `smelt`).
    Smelt {
        owner: u128,
        item: String,
        count: u32,
    },
    /// Slice 12: the kitchen made a recipe (the contract's `craft`).
    Craft {
        owner: u128,
        recipe: String,
        item: String,
        count: u32,
        crafts: u32,
    },
    /// Slice 12: the contract's counted reports of spec 22: `repair`, `breed`, `shear`, `honey`,
    /// `brew`, `enchant` (levels) and `brush`; 0.21.1: `mix`, the buckets a mixer made.
    Counted {
        kind: &'static str,
        owner: u128,
        count: u32,
    },
    /// Slice 12: the fishing station's catch.
    Fish {
        owner: u128,
        item: String,
        count: u32,
    },
    /// Slice 12: the shearer milked a cow.
    Milk { owner: u128, entity: u32 },
    /// Slice 13: a cartographer marked a chunk new to its owner (EX1).
    Chunk {
        owner: u128,
        dimension: String,
        x: i32,
        z: i32,
    },
    /// Slice 13: a cartographer found a biome new to its owner (EX1).
    Biome { owner: u128, biome: String },
    /// A cartographer found a structure (its id as in `/locate structure`) new to its owner (EX1).
    Structure { owner: u128, id: String },
    /// Slice 12: the building machine placed a cell of its projector's plan (instead of `place`).
    Build {
        owner: u128,
        dimension: String,
        pos: Pos,
        block: String,
    },
}

/// Slice 18 (cross): what a machine's output slots gained between `before` and `after`, item by
/// item (counts summed per item), for its `smelt` report; nothing taken away counts.
pub fn gained<'a>(
    before: impl IntoIterator<Item = (&'a str, u32)>,
    after: impl IntoIterator<Item = (&'a str, u32)>,
) -> Vec<(String, u32)> {
    let mut had: std::collections::BTreeMap<&str, i64> = std::collections::BTreeMap::new();
    for (item, n) in before {
        *had.entry(item).or_default() -= i64::from(n);
    }
    for (item, n) in after {
        *had.entry(item).or_default() += i64::from(n);
    }
    had.into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(item, n)| (item.to_owned(), u32::try_from(n).unwrap_or(u32::MAX)))
        .collect()
}

/// The counted kinds of spec 22 Path-of-craft-tree takes; `mix` (the mixer's buckets, Alchemy)
/// since 0.21.1 and Path-of-craft-tree 0.5.2, which an older one refuses and logs.
pub const COUNTED: [&str; 8] = ["repair", "breed", "shear", "honey", "brew", "enchant", "brush", "mix"];
/// 0.21.1: the mixer's counted report.
pub const MIX: &str = "mix";

impl Report {
    /// The command line, without a slash.
    pub fn command(&self) -> String {
        match self {
            Report::Break {
                owner,
                dimension,
                pos,
                block,
                tool,
            } => format!(
                "{COMMAND} break {owner:032x} {dimension} {} {} {} {block} {tool}",
                pos[0], pos[1], pos[2]
            ),
            Report::Place {
                dimension,
                pos,
                block,
            } => format!(
                "{COMMAND} place {dimension} {} {} {} {block}",
                pos[0], pos[1], pos[2]
            ),
            Report::Kill {
                owner,
                entity,
                kind,
            } => format!("{COMMAND} kill {owner:032x} {entity} {kind}"),
            Report::Slaughter {
                owner,
                entity,
                kind,
            } => format!("{COMMAND} slaughter {owner:032x} {entity} {kind}"),
            Report::Craft {
                owner,
                recipe,
                item,
                count,
                crafts,
            } => format!("{COMMAND} craft {owner:032x} {recipe} {item} {count} {crafts}"),
            Report::Smelt { owner, item, count } => format!("{COMMAND} smelt {owner:032x} {item} {count}"),
            Report::Counted { kind, owner, count } => format!("{COMMAND} {kind} {owner:032x} {count}"),
            Report::Fish { owner, item, count } => format!("{COMMAND} fish {owner:032x} {item} {count}"),
            Report::Milk { owner, entity } => format!("{COMMAND} milk {owner:032x} {entity}"),
            Report::Chunk { owner, dimension, x, z } => format!("{COMMAND} chunk {owner:032x} {dimension} {x} {z}"),
            Report::Biome { owner, biome } => format!("{COMMAND} biome {owner:032x} {biome}"),
            Report::Structure { owner, id } => format!("{COMMAND} structure {owner:032x} {id}"),
            Report::Build {
                owner,
                dimension,
                pos,
                block,
            } => format!(
                "{COMMAND} build {owner:032x} {dimension} {} {} {} {block}",
                pos[0], pos[1], pos[2]
            ),
        }
    }

    /// Its kind's word, for the counters.
    pub fn kind(&self) -> &'static str {
        match self {
            Report::Break { .. } => "break",
            Report::Place { .. } => "place",
            Report::Kill { .. } => "kill",
            Report::Slaughter { .. } => "slaughter",
            Report::Smelt { .. } => "smelt",
            Report::Craft { .. } => "craft",
            Report::Counted { kind, .. } => kind,
            Report::Fish { .. } => "fish",
            Report::Milk { .. } => "milk",
            Report::Chunk { .. } => "chunk",
            Report::Biome { .. } => "biome",
            Report::Structure { .. } => "structure",
            Report::Build { .. } => "build",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gained_counts_only_what_came_out() {
        let iron = "minecraft:iron_ingot";
        assert_eq!(gained([(iron, 3)], [(iron, 4)]), vec![(iron.to_owned(), 1)]);
        assert!(gained([(iron, 4)], [(iron, 1)]).is_empty());
        assert_eq!(
            gained([], [(iron, 2), ("rustcraft:sparksteel", 1), (iron, 1)]),
            vec![(iron.to_owned(), 3), ("rustcraft:sparksteel".to_owned(), 1)]
        );
    }

    #[test]
    fn a_smelt_report_is_the_contracts() {
        // docs/design/path-of-craft-tree-contract.md: `smelt <владелец> <предмет> <количество>`.
        let s = Report::Smelt { owner: 0x26a5132a990442e59cfd2cccd5083fa8, item: "minecraft:iron_ingot".into(), count: 4 };
        assert_eq!(s.command(), "pct-machine smelt 26a5132a990442e59cfd2cccd5083fa8 minecraft:iron_ingot 4");
        assert_eq!(s.kind(), "smelt");
    }

    #[test]
    fn spec_22_reports_read_as_the_contract_writes_them() {
        let owner = 0x26a5132a990442e59cfd2cccd5083fa8u128;
        let o = "26a5132a990442e59cfd2cccd5083fa8";
        let c = Report::Counted { kind: "breed", owner, count: 1 };
        assert_eq!(c.command(), format!("pct-machine breed {o} 1"));
        let f = Report::Fish { owner, item: "minecraft:cod".into(), count: 1 };
        assert_eq!(f.command(), format!("pct-machine fish {o} minecraft:cod 1"));
        assert_eq!(Report::Milk { owner, entity: 9 }.command(), format!("pct-machine milk {o} 9"));
        let m = Report::Counted { kind: MIX, owner, count: 2 };
        assert_eq!(m.command(), format!("pct-machine mix {o} 2"));
        assert_eq!(m.kind(), "mix");
        assert!(COUNTED.contains(&MIX));
        let b = Report::Build {
            owner,
            dimension: "minecraft:overworld".into(),
            pos: [1, 2, -3],
            block: "minecraft:stone".into(),
        };
        assert_eq!(b.command(), format!("pct-machine build {o} minecraft:overworld 1 2 -3 minecraft:stone"));
        assert_eq!(b.kind(), "build");
        let s = Report::Structure { owner, id: "minecraft:village_plains".into() };
        assert_eq!(s.command(), format!("pct-machine structure {o} minecraft:village_plains"));
        assert_eq!(s.kind(), "structure");
    }

    #[test]
    fn a_kill_reads_as_soulscraft_takes_it() {
        let k = SoulsKill {
            source: "turret",
            owner: 0x26a5132a990442e59cfd2cccd5083fa8,
            entity: 77,
            kind: "minecraft:zombie".into(),
            pos: block_of([10.7, 64.0, -3.2]),
        };
        assert_eq!(
            k.command(),
            "soulscraft-kill turret 26a5132a990442e59cfd2cccd5083fa8 77 minecraft:zombie 10 64 -4"
        );
    }

    #[test]
    fn reports_read_as_the_contract_writes_them() {
        let owner = 0x26a5132a990442e59cfd2cccd5083fa8u128;
        let b = Report::Break {
            owner,
            dimension: "minecraft:overworld".into(),
            pos: [100, 60, -100],
            block: "minecraft:oak_log".into(),
            tool: "axe",
        };
        assert_eq!(
            b.command(),
            "pct-machine break 26a5132a990442e59cfd2cccd5083fa8 minecraft:overworld 100 60 -100 minecraft:oak_log axe"
        );
        let k = Report::Kill {
            owner: 1,
            entity: 42,
            kind: "minecraft:zombie".into(),
        };
        assert_eq!(
            k.command(),
            "pct-machine kill 00000000000000000000000000000001 42 minecraft:zombie"
        );
        assert_eq!(k.kind(), "kill");
        let p = Report::Place {
            dimension: "minecraft:overworld".into(),
            pos: [1, 2, 3],
            block: "minecraft:stone".into(),
        };
        assert_eq!(
            p.command(),
            "pct-machine place minecraft:overworld 1 2 3 minecraft:stone"
        );
        let s = Report::Slaughter {
            owner: 2,
            entity: 7,
            kind: "minecraft:cow".into(),
        };
        assert_eq!(
            s.command(),
            "pct-machine slaughter 00000000000000000000000000000002 7 minecraft:cow"
        );
        assert_eq!(s.kind(), "slaughter");
    }
}
