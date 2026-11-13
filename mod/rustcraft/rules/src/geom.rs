//! Block positions and faces.

/// A block position in the world.
pub type Pos = [i32; 3];

/// One of the six faces of a block, named by the direction it looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Face {
    XNeg,
    XPos,
    YNeg,
    YPos,
    ZNeg,
    ZPos,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::XNeg,
        Face::XPos,
        Face::YNeg,
        Face::YPos,
        Face::ZNeg,
        Face::ZPos,
    ];

    /// The step to the neighbour behind this face.
    pub fn offset(self) -> Pos {
        match self {
            Face::XNeg => [-1, 0, 0],
            Face::XPos => [1, 0, 0],
            Face::YNeg => [0, -1, 0],
            Face::YPos => [0, 1, 0],
            Face::ZNeg => [0, 0, -1],
            Face::ZPos => [0, 0, 1],
        }
    }

    /// The face's place in `ALL`, the byte a block saves it as.
    pub fn index(self) -> u8 {
        Face::ALL.iter().position(|&f| f == self).unwrap_or(0) as u8
    }

    /// The face `index` names; none past the six.
    pub fn from_index(index: u8) -> Option<Face> {
        Face::ALL.get(usize::from(index)).copied()
    }

    /// The face whose offset is `normal` (a clicked face as the game gives it).
    pub fn from_normal(normal: Pos) -> Option<Face> {
        Face::ALL.into_iter().find(|f| f.offset() == normal)
    }

    pub fn opposite(self) -> Face {
        match self {
            Face::XNeg => Face::XPos,
            Face::XPos => Face::XNeg,
            Face::YNeg => Face::YPos,
            Face::YPos => Face::YNeg,
            Face::ZNeg => Face::ZPos,
            Face::ZPos => Face::ZNeg,
        }
    }
}

/// The neighbour of `pos` behind `face`.
pub fn step(pos: Pos, face: Face) -> Pos {
    let o = face.offset();
    [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]]
}

/// The face of `from` that looks at the adjacent block `to`, if they touch by a face.
pub fn face_towards(from: Pos, to: Pos) -> Option<Face> {
    Face::ALL.into_iter().find(|&f| step(from, f) == to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_survives_its_index_and_its_normal() {
        for face in Face::ALL {
            assert_eq!(Face::from_index(face.index()), Some(face));
            assert_eq!(Face::from_normal(face.offset()), Some(face));
        }
        assert_eq!(Face::from_index(6), None);
        assert_eq!(Face::from_normal([1, 1, 0]), None);
    }
}
