//! The `rustcraft:workers` note, server to every client (slice 18): each seat, its heading, tool,
//! the mount of its rail, whether its tool is at work and what it works on, so the client draws
//! the right bracket and moves the tool (`pose`).

use super::Tool;
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
}

const SIZE: usize = 4 + 4 + 1 + 1 + 1 + 12;

fn tool_byte(tool: Option<Tool>) -> u8 {
    tool.map_or(0, |t| Tool::ALL.iter().position(|x| *x == t).map_or(0, |i| i as u8 + 1))
}

fn tool_of(b: u8) -> Option<Tool> {
    Tool::ALL.get(usize::from(b).checked_sub(1)?).copied()
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
    }
    out
}

pub fn decode(data: &[u8]) -> Option<Vec<Seat>> {
    if data.len() % SIZE != 0 {
        return None;
    }
    let f32_at = |c: &[u8], i: usize| f32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]]);
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
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_reads_back() {
        let seats = vec![
            Seat { seat: 7, yaw: -90.0, tool: Some(Tool::Bore), mount: Face::XNeg, busy: true, aim: Some([1.5, 64.5, -3.5]) },
            Seat { seat: 9, yaw: 0.0, tool: None, mount: Face::YPos, busy: false, aim: None },
        ];
        assert_eq!(decode(&encode(&seats)), Some(seats));
        assert_eq!(decode(&[1, 2, 3]), None);
        for t in Tool::ALL {
            assert_eq!(tool_of(tool_byte(Some(t))), Some(t));
        }
    }
}
