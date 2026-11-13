//! Reports to Path-of-craft-tree (spec 4.15 X1–X9, 7.1; `docs/design/path-of-craft-tree-contract.md`):
//! what a machine did and whose it is, as a `pct-machine` console command. Path-of-craft-tree
//! decides what it is worth. The machines' rules give events; this turns them into commands, so
//! the rules know nothing of the other mod.

use crate::geom::Pos;

/// The console command Path-of-craft-tree registers.
pub const COMMAND: &str = "pct-machine";

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
}

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
        }
    }

    /// Its kind's word, for the counters.
    pub fn kind(&self) -> &'static str {
        match self {
            Report::Break { .. } => "break",
            Report::Place { .. } => "place",
            Report::Kill { .. } => "kill",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
