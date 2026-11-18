//! The `rustcraft:workers` note, server to every client (slice 18): each seat, its heading, tool,
//! the mount of its rail, whether its tool is at work and what it works on, so the client draws
//! the right bracket and moves the tool (`pose`); since 0.18.5 also its board (`Board`), which
//! the client draws over it in its own language.

use super::{Note, Tool};
use crate::geom::Face;

/// One worker as its clients see it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seat {
    pub seat: u32,
    pub yaw: f32,
    pub tool: Option<Tool>,
    pub mount: Face,
    pub busy: bool,
    /// The block its tool works on, world coordinates of its middle.
    pub aim: Option<[f32; 3]>,
    pub board: Board,
}

/// 0.18.5: the board over a worker (W5, guide `chassis`: «Над ним видна табличка: заряд,
/// заполнение трюма, состояние»). The server named its seat with it, a translated component the
/// game reads as no name at all (Horizon takes a mob's name only from plain `text`), so no board
/// was ever seen; the client draws it now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Board {
    /// Charge, percent of the battery.
    pub charge: u8,
    /// Hold slots used and in all.
    pub used: u16,
    pub slots: u16,
    pub note: Note,
}

impl Board {
    pub fn of(w: &super::Worker, t: &crate::tuning::Tuning) -> Board {
        let share = (w.charge / w.capacity(t).max(1.0)).clamp(0.0, 1.0);
        let clip = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        Board {
            charge: (share * 100.0).round() as u8,
            used: clip(w.hold.used()),
            slots: clip(w.hold.slots.len()),
            note: w.note,
        }
    }

    /// The board's line: «Жнец · 84% · 3/27 · Работает», with the tool's name and the state as
    /// the client's language has them.
    pub fn line(&self, tool: &str, note: &str) -> String {
        format!("{tool} · {}% · {}/{} · {note}", self.charge, self.used, self.slots)
    }
}

const SIZE: usize = 4 + 4 + 1 + 1 + 1 + 12 + 1 + 2 + 2 + 1;

fn tool_byte(tool: Option<Tool>) -> u8 {
    tool.map_or(0, |t| Tool::ALL.iter().position(|x| *x == t).map_or(0, |i| i as u8 + 1))
}

fn tool_of(b: u8) -> Option<Tool> {
    Tool::ALL.get(usize::from(b).checked_sub(1)?).copied()
}

fn note_byte(note: Note) -> u8 {
    Note::ALL.iter().position(|n| *n == note).map_or(0, |i| i as u8)
}

fn note_of(b: u8) -> Note {
    Note::ALL.get(usize::from(b)).copied().unwrap_or_default()
}

pub fn encode(seats: &[Seat]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seats.len() * SIZE);
    for s in seats {
        out.extend(s.seat.to_le_bytes());
        out.extend(s.yaw.to_le_bytes());
        out.push(tool_byte(s.tool));
        out.push(s.mount.index());
        out.push(u8::from(s.busy) | (u8::from(s.aim.is_some()) << 1));
        for v in s.aim.unwrap_or([0.0; 3]) {
            out.extend(v.to_le_bytes());
        }
        out.push(s.board.charge);
        out.extend(s.board.used.to_le_bytes());
        out.extend(s.board.slots.to_le_bytes());
        out.push(note_byte(s.board.note));
    }
    out
}

pub fn decode(data: &[u8]) -> Option<Vec<Seat>> {
    if data.len() % SIZE != 0 {
        return None;
    }
    let f32_at = |c: &[u8], i: usize| f32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]]);
    let u16_at = |c: &[u8], i: usize| u16::from_le_bytes([c[i], c[i + 1]]);
    data.chunks_exact(SIZE)
        .map(|c| {
            let flags = c[10];
            Some(Seat {
                seat: u32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                yaw: f32_at(c, 4),
                tool: tool_of(c[8]),
                mount: Face::from_index(c[9])?,
                busy: flags & 1 != 0,
                aim: (flags & 2 != 0).then(|| [f32_at(c, 11), f32_at(c, 15), f32_at(c, 19)]),
                board: Board {
                    charge: c[23],
                    used: u16_at(c, 24),
                    slots: u16_at(c, 26),
                    note: note_of(c[28]),
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_reads_back() {
        let board = Board { charge: 84, used: 3, slots: 3456, note: Note::NothingToSow };
        let seats = vec![
            Seat { seat: 7, yaw: -90.0, tool: Some(Tool::Bore), mount: Face::XNeg, busy: true, aim: Some([1.5, 64.5, -3.5]), board },
            Seat { seat: 9, yaw: 0.0, tool: None, mount: Face::YPos, busy: false, aim: None, board: Board::default() },
        ];
        assert_eq!(decode(&encode(&seats)), Some(seats));
        assert_eq!(decode(&[1, 2, 3]), None);
        for t in Tool::ALL {
            assert_eq!(tool_of(tool_byte(Some(t))), Some(t));
        }
        for (i, n) in Note::ALL.into_iter().enumerate() {
            assert_eq!(note_of(note_byte(n)), n);
            assert!(!Note::ALL[..i].contains(&n), "{n:?} listed twice");
        }
    }

    #[test]
    fn the_board_says_charge_hold_and_state() {
        // W5, guide `chassis`: «Над ним видна табличка: заряд, заполнение трюма, состояние».
        let t = crate::tuning::Tuning::default();
        let mut w = super::super::Worker::new(1, 7, [0, 0, 0], &t);
        w.tool = Some(Tool::Reaper);
        w.charge = 0.84 * w.capacity(&t);
        w.hold.add("minecraft:wheat_seeds", 10, 64);
        w.note = Note::NothingToSow;
        let b = Board::of(&w, &t);
        assert_eq!((b.charge, b.used, usize::from(b.slots)), (84, 1, t.worker_hold));
        assert_eq!(b.line("Жнец", "Нечем засеять"), format!("Жнец · 84% · 1/{} · Нечем засеять", t.worker_hold));
    }
}

/// The chassis window's panel (slice 18, guide `ui.js` `chassis`), server to the player who has
/// the window open: what the client draws beside the menu's slots.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Panel {
    pub tier: u8,
    pub tool: Option<Tool>,
    /// Sockets holding something and sockets in all.
    pub sockets: (usize, usize),
    /// Charge, 0..1, and the battery, E.
    pub charge: f64,
    pub battery: f64,
    /// Hold slots used and in all.
    pub hold: (usize, usize),
    pub note: super::Note,
    pub filter: Vec<String>,
}

/// The channel the panel goes on.
pub const PANEL: &str = "rustcraft:chassis";

impl Panel {
    pub fn of(w: &super::Worker, t: &crate::tuning::Tuning) -> Panel {
        let battery = w.capacity(t);
        Panel {
            tier: w.tier,
            tool: w.tool,
            sockets: (w.upgrades.iter().flatten().count(), w.upgrades.len()),
            charge: (w.charge / battery.max(1.0)).clamp(0.0, 1.0),
            battery,
            hold: (w.hold.used(), w.hold.slots.len()),
            note: w.note,
            filter: w.filter.clone(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn decode(data: &[u8]) -> Option<Panel> {
        serde_json::from_slice(data).ok()
    }
}

/// How a state reads in a status line: fine, waiting, or stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Fine,
    Waiting,
    Stopped,
}

/// The translation key, English fallback and mood of a worker's state (W5, A4).
pub fn note_key(note: super::Note) -> (&'static str, &'static str, Mood) {
    use super::Note as N;
    use Mood::{Fine, Stopped, Waiting};
    match note {
        N::Working => ("rustcraft.worker.working", "Working", Fine),
        N::Unloading => ("rustcraft.worker.unloading", "Unloading", Fine),
        N::Charging => ("rustcraft.worker.charging", "Charging", Fine),
        N::ToUnload => ("rustcraft.worker.to_unload", "Going to unload", Fine),
        N::ToCharge => ("rustcraft.worker.to_charge", "Going to charge", Fine),
        N::NoTool => ("rustcraft.worker.no_tool", "No tool", Stopped),
        N::NoUnloadDock => ("rustcraft.worker.no_unload", "No unload dock on the way", Stopped),
        N::NoChargeDock => ("rustcraft.worker.no_charge", "No charge dock on the way", Stopped),
        N::Flat => ("rustcraft.worker.flat", "Battery empty", Stopped),
        N::NothingToSow => ("rustcraft.worker.nothing_to_sow", "Nothing to sow", Waiting),
        N::DockFull => ("rustcraft.worker.dock_full", "Waiting: the unload dock is full", Waiting),
        N::DockEmpty => (
            "rustcraft.worker.dock_empty",
            "Waiting: the charge dock has no energy",
            Waiting,
        ),
        N::NoWay => ("rustcraft.worker.no_way", "No rail to ride", Stopped),
        N::ToLoad => ("rustcraft.worker.to_load", "Going to load", Fine),
        N::Loading => ("rustcraft.worker.loading", "Loading", Fine),
        N::NoLoadDock => ("rustcraft.worker.no_load", "No load dock on the way", Stopped),
        N::NoCargo => ("rustcraft.worker.no_cargo", "Waiting for cargo", Waiting),
        N::NoRails => ("rustcraft.worker.no_rails", "Out of rails", Stopped),
        N::LavaAhead => ("rustcraft.worker.lava", "Lava ahead and no blocks", Stopped),
        N::AllMapped => ("rustcraft.worker.all_mapped", "Everything round the way is mapped", Waiting),
        N::NothingToReap => ("rustcraft.worker.nothing_to_reap", "Nothing to reap beside the way", Waiting),
        N::NoTrees => ("rustcraft.worker.no_trees", "No trees beside the way", Waiting),
        N::NoOre => ("rustcraft.worker.no_ore", "No ore beside the way", Waiting),
        N::NoSaplings => ("rustcraft.worker.no_saplings", "No saplings", Waiting),
        N::NothingToDig => (

            "rustcraft.worker.nothing_to_dig",
            "Nothing to dig beside the way",
            Waiting,
        ),
    }
}

/// The translation key and English fallback of a worker's name by its tool.
pub fn tool_key(tool: Option<Tool>) -> (&'static str, &'static str) {
    match tool {
        Some(Tool::Saw) => ("rustcraft.worker.saw", "Lumberjack"),
        Some(Tool::Reaper) => ("rustcraft.worker.reaper", "Reaper"),
        Some(Tool::Hold) => ("rustcraft.worker.cargo", "Cargo cart"),
        Some(Tool::Grab) => ("rustcraft.worker.grab", "Collector"),
        Some(Tool::Scoop) => ("rustcraft.worker.scoop", "Digger"),
        Some(Tool::Shooter) => ("rustcraft.worker.guard", "Guard"),
        Some(Tool::Map) => ("rustcraft.worker.map", "Cartographer"),
        Some(Tool::Bore) => ("rustcraft.worker.bore", "Tunneler"),
        None => ("rustcraft.worker.bare", "Chassis"),
    }
}

/// The chassis window's title key and fallback by tier.
pub fn window_key(tier: u8) -> (&'static str, &'static str) {
    match tier {
        2 => ("rustcraft.worker.window_2", "Worker chassis · crystallite"),
        3 => ("rustcraft.worker.window_3", "Worker chassis · starsteel"),
        _ => ("rustcraft.worker.window", "Worker chassis · sparksteel"),
    }
}

#[cfg(test)]
mod panel_tests {
    use super::*;
    use crate::tuning::Tuning;
    use crate::worker::{Note, Worker};

    #[test]
    fn a_panel_reads_back_and_says_its_mood() {
        let t = Tuning::default();
        let mut w = Worker::new(1, 7, [0, 0, 0], &t);
        w.tool = Some(Tool::Reaper);
        w.charge = t.worker_battery * 0.8;
        w.filter = vec!["minecraft:pumpkin".into()];
        w.set_tier(2, &t);
        let p = Panel::of(&w, &t);
        assert_eq!(p.sockets, (0, 5));
        assert!((p.charge - 0.8).abs() < 1e-9);
        assert_eq!(Panel::decode(&p.encode()), Some(p));
        assert_eq!(note_key(Note::Working).2, Mood::Fine);
        assert_eq!(note_key(Note::DockFull).2, Mood::Waiting);
        assert_eq!(note_key(Note::NoTool).2, Mood::Stopped);
        assert_eq!(window_key(3).0, "rustcraft.worker.window_3");
    }
}
