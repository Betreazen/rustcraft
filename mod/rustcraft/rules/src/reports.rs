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
    /// Slice 12: the kitchen made a recipe (the contract's `craft`).
    Craft {
        owner: u128,
        recipe: String,
        item: String,
        count: u32,
        crafts: u32,
    },
    /// Slice 12: the contract's counted reports of spec 22: `repair`, `breed`, `shear`, `honey`,
    /// `brew`, `enchant` (levels) and `brush`.
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
    /// Slice 12: the building machine placed a cell of its projector's plan (instead of `place`).
    Build {
        owner: u128,
        dimension: String,
        pos: Pos,
        block: String,
    },
}

/// The counted kinds of spec 22 Path-of-craft-tree takes.
pub const COUNTED: [&str; 7] = ["repair", "breed", "shear", "honey", "brew", "enchant", "brush"];

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
            Report::Counted { kind, owner, count } => format!("{COMMAND} {kind} {owner:032x} {count}"),
            Report::Fish { owner, item, count } => format!("{COMMAND} fish {owner:032x} {item} {count}"),
            Report::Milk { owner, entity } => format!("{COMMAND} milk {owner:032x} {entity}"),
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
            Report::Craft { .. } => "craft",
            Report::Counted { kind, .. } => kind,
            Report::Fish { .. } => "fish",
            Report::Milk { .. } => "milk",
            Report::Build { .. } => "build",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_22_reports_read_as_the_contract_writes_them() {
        let owner = 0x26a5132a990442e59cfd2cccd5083fa8u128;
        let o = "26a5132a990442e59cfd2cccd5083fa8";
        let c = Report::Counted { kind: "breed", owner, count: 1 };
        assert_eq!(c.command(), format!("pct-machine breed {o} 1"));
        let f = Report::Fish { owner, item: "minecraft:cod".into(), count: 1 };
        assert_eq!(f.command(), format!("pct-machine fish {o} minecraft:cod 1"));
        assert_eq!(Report::Milk { owner, entity: 9 }.command(), format!("pct-machine milk {o} 9"));
        let b = Report::Build {
            owner,
            dimension: "minecraft:overworld".into(),
            pos: [1, 2, -3],
            block: "minecraft:stone".into(),
        };
        assert_eq!(b.command(), format!("pct-machine build {o} minecraft:overworld 1 2 -3 minecraft:stone"));
        assert_eq!(b.kind(), "build");
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
