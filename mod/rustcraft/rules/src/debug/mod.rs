//! The diagnostics console command `rustcraft_debug` and the event journal (no spec rule: a tool
//! for answering a player's report from the server console alone). The bundle's
//! `rustcraft_debug` gathers what the world holds; what can be decided without the game lives
//! here: the command's words, the answer's limits, the journal's line, its rotation and tail, the
//! status-change tracker, and the text forms of a status and of the fishing station's water.

pub mod flow;
pub mod journal;
pub mod text;
pub mod track;

use crate::geom::Pos;

/// The console command, for operators of level 4.
pub const COMMAND: &str = "rustcraft_debug";
/// The items `storage` lists, the most numerous first.
pub const TOP_ITEMS: usize = 40;
/// The radius `near` looks in when none is given, blocks.
pub const NEAR_RADIUS: i32 = 16;
/// The largest radius `near` takes.
pub const NEAR_MAX: i32 = 128;
/// The journal lines `log` shows when no count is given, and the most it shows.
pub const LOG_LINES: usize = 30;
pub const LOG_MAX: usize = 200;
/// An answer stays under this many lines and characters: the console's feedback is read back
/// whole by whoever asked.
pub const MAX_LINES: usize = 60;
pub const MAX_CHARS: usize = 4000;

/// What the console asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// The block at `pos`, in `dimension` or the level in place.
    At { pos: Pos, dimension: Option<String> },
    /// The machines and workers within `radius` of a player.
    Near { player: String, radius: i32 },
    /// Every worker, or only those of `owner` (a name or a uuid).
    Workers { owner: Option<String> },
    /// The storage at `pos` (a cell or a terminal, or a block on its line): its items, orders and
    /// the machines of its line.
    Storage { pos: Pos, dimension: Option<String> },
    /// The journal's last `n` lines, only those containing `filter` when one is given.
    Log { n: usize, filter: Option<String> },
}

/// The command's words after its name.
pub fn usage() -> String {
    format!(
        "usage: {COMMAND} at <x> <y> <z> [dimension] | near <player> [radius={NEAR_RADIUS}] | \
         workers [player] | storage <x> <y> <z> [dimension] | log [n={LOG_LINES}] [filter]"
    )
}

/// The words after the command's name, read; the usage line when they make no command.
pub fn parse(args: &str) -> Result<Command, String> {
    let words: Vec<&str> = args.split_whitespace().collect();
    match words.as_slice() {
        [word @ ("at" | "storage"), x, y, z, rest @ ..] if rest.len() <= 1 => {
            let n = |w: &str| w.parse::<i32>().map_err(|_| format!("not a coordinate: {w}"));
            let pos = [n(x)?, n(y)?, n(z)?];
            let dimension = rest.first().map(|d| dimension_id(d));
            Ok(if *word == "at" {
                Command::At { pos, dimension }
            } else {
                Command::Storage { pos, dimension }
            })
        }
        ["near", player, rest @ ..] if rest.len() <= 1 => {
            let radius = match rest.first() {
                Some(r) => r.parse::<i32>().map_err(|_| format!("not a radius: {r}"))?,
                None => NEAR_RADIUS,
            };
            Ok(Command::Near {
                player: (*player).to_owned(),
                radius: radius.clamp(1, NEAR_MAX),
            })
        }
        ["workers"] => Ok(Command::Workers { owner: None }),
        ["workers", owner] => Ok(Command::Workers {
            owner: Some((*owner).to_owned()),
        }),
        ["log", rest @ ..] => {
            // `log 50 furnace`, `log furnace` and `log` all read; a number first is the count.
            let (n, filter) = match rest.first().map(|w| w.parse::<usize>()) {
                Some(Ok(n)) => (n, &rest[1..]),
                _ => (LOG_LINES, rest),
            };
            Ok(Command::Log {
                n: n.clamp(1, LOG_MAX),
                filter: (!filter.is_empty()).then(|| filter.join(" ")),
            })
        }
        _ => Err(usage()),
    }
}

/// `nether` and `the_nether` as `minecraft:the_nether`; a namespaced id as it is.
pub fn dimension_id(word: &str) -> String {
    match word {
        "overworld" => "minecraft:overworld".to_owned(),
        "nether" | "the_nether" => "minecraft:the_nether".to_owned(),
        "end" | "the_end" => "minecraft:the_end".to_owned(),
        w if w.contains(':') => w.to_owned(),
        w => format!("minecraft:{w}"),
    }
}

/// A position as the journal and the answers write it: `x y z`.
pub fn pos_text(p: Pos) -> String {
    format!("{} {} {}", p[0], p[1], p[2])
}

/// Whether `p` is within `radius` of `centre` (a sphere).
pub fn within(centre: [f64; 3], p: Pos, radius: i32) -> bool {
    let d2: f64 = (0..3)
        .map(|i| {
            let d = f64::from(p[i]) + 0.5 - centre[i];
            d * d
        })
        .sum();
    d2 <= f64::from(radius) * f64::from(radius)
}

/// The `n` most numerous items, the most first (by name among equals).
pub fn top_items(items: &std::collections::BTreeMap<String, u64>, n: usize) -> Vec<(String, u64)> {
    let mut all: Vec<(String, u64)> = items
        .iter()
        .filter(|(_, c)| **c > 0)
        .map(|(k, c)| (k.clone(), *c))
        .collect();
    all.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    all.truncate(n);
    all
}

/// Short entries joined with `, ` into lines of about `width` characters, each line led by
/// `lead`: a list that would take a line an entry takes a few.
pub fn pack(lead: &str, entries: &[String], width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for e in entries {
        if !line.is_empty() && line.chars().count() + e.chars().count() + 2 > width {
            out.push(format!("{lead}{line}"));
            line.clear();
        }
        if !line.is_empty() {
            line.push_str(", ");
        }
        line.push_str(e);
    }
    if !line.is_empty() {
        out.push(format!("{lead}{line}"));
    }
    out
}

/// The answer kept under [`MAX_LINES`] and [`MAX_CHARS`]: the lines that fit, then one saying how
/// many were left out.
pub fn clip(lines: Vec<String>) -> Vec<String> {
    let total = lines.len();
    let mut out = Vec::new();
    let mut chars = 0;
    for line in lines {
        let len = line.chars().count() + 1;
        if out.len() + 1 >= MAX_LINES || chars + len > MAX_CHARS - 40 {
            break;
        }
        chars += len;
        out.push(line);
    }
    if out.len() < total {
        let left = total - out.len();
        out.push(format!("... {left} more lines not shown"));
    }
    out
}

/// `clip` for a journal's tail: the newest lines that fit (with room left for a heading), and
/// first a line saying how many older ones were left out.
pub fn clip_newest(lines: Vec<String>) -> Vec<String> {
    let total = lines.len();
    let mut kept = Vec::new();
    let mut chars = 0;
    for line in lines.into_iter().rev() {
        let len = line.chars().count() + 1;
        if kept.len() + 3 >= MAX_LINES || chars + len > MAX_CHARS - 200 {
            break;
        }
        chars += len;
        kept.push(line);
    }
    kept.reverse();
    if kept.len() < total {
        kept.insert(0, format!("... {} older lines not shown", total - kept.len()));
    }
    kept
}

/// One worker as `workers` and `near` list it.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkerRow {
    pub id: u64,
    pub owner: String,
    pub cell: Pos,
    pub tool: String,
    pub charge_percent: f64,
    pub hold_used: usize,
    pub hold_size: usize,
    pub errand: String,
    pub note_key: &'static str,
    pub loaded: bool,
}

impl WorkerRow {
    pub fn line(&self) -> String {
        format!(
            "worker #{} owner {} at {} tool {} charge {:.0}% hold {}/{} errand {} note {}{}",
            self.id,
            self.owner,
            pos_text(self.cell),
            self.tool,
            self.charge_percent,
            self.hold_used,
            self.hold_size,
            self.errand,
            self.note_key,
            if self.loaded { "" } else { " (chunk not loaded)" }
        )
    }
}

/// The 5 × 5 water under a fishing station (cells in `depths`' order: x outer, z inner), one row
/// of the answer per x: each cell its water depth, `L` where the magistral passes over it, `@` the
/// station's own column.
pub fn water_grid(depths: &[i32], line: &[bool], side: i32) -> Vec<String> {
    let side = usize::try_from(side).unwrap_or(0);
    if side == 0 || depths.len() != side * side {
        return vec![format!("water: {} cells, not {side}x{side}", depths.len())];
    }
    let middle = depths.len() / 2;
    (0..side)
        .map(|row| {
            let cells: Vec<String> = (0..side)
                .map(|col| {
                    let i = row * side + col;
                    let d = depths[i];
                    match (i == middle, line.get(i).copied().unwrap_or(false)) {
                        (true, _) => format!("@{d}"),
                        (false, true) => format!("L{d}"),
                        (false, false) => format!(" {d}"),
                    }
                })
                .collect();
            format!("  dx {:+}: {}", row as i32 - side as i32 / 2, cells.join(" "))
        })
        .collect()
}

#[cfg(test)]
mod tests;
