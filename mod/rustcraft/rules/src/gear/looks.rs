//! Slice 18: gear seen on a player (guide `jetpack`: the jetpack on the back and its flames in
//! flight; `zipline`: a hand on the handle with its rollers; `backpack`: starsteel's on the back).
//! The server sends every player of a level what each player there wears; clients draw it after
//! the player's own model (`native_mob_layer`, API 149).

/// The channel the looks go to clients on.
pub const CHANNEL: &str = "rustcraft:looks";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Look {
    pub jetpack: bool,
    pub flying: bool,
    pub handle: bool,
    pub backpack: bool,
}

impl Look {
    pub fn to_byte(self) -> u8 {
        u8::from(self.jetpack) | u8::from(self.flying) << 1 | u8::from(self.handle) << 2 | u8::from(self.backpack) << 3
    }

    pub fn from_byte(b: u8) -> Look {
        Look { jetpack: b & 1 != 0, flying: b & 2 != 0, handle: b & 4 != 0, backpack: b & 8 != 0 }
    }

    pub fn shows(self) -> bool {
        self.to_byte() != 0
    }
}

/// The wire: each player's entity id (4 bytes, little end first) and look byte.
pub fn encode(looks: &[(i32, Look)]) -> Vec<u8> {
    looks.iter().flat_map(|(id, l)| id.to_le_bytes().into_iter().chain([l.to_byte()])).collect()
}

pub fn decode(data: &[u8]) -> Vec<(i32, Look)> {
    data.chunks_exact(5)
        .map(|c| (i32::from_le_bytes([c[0], c[1], c[2], c[3]]), Look::from_byte(c[4])))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_go_over_the_wire_and_back() {
        let all = Look { jetpack: true, flying: true, handle: true, backpack: true };
        for b in 0..16 {
            assert_eq!(Look::from_byte(b).to_byte(), b);
        }
        assert!(!Look::default().shows());
        let list = vec![(42, all), (-7, Look { handle: true, ..Look::default() })];
        assert_eq!(decode(&encode(&list)), list);
        assert!(decode(&[1, 2, 3]).is_empty());
    }
}
