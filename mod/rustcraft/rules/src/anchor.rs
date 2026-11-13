//! The base anchor (spec 4.10, B1, B2, B4; round 19 B5; round 29 B6; guide `anchor`). It holds
//! the chunk it stands in loaded while its owner is on the server, anywhere; radius modules make
//! that 3×3, then 5×5 chunks. It draws 2 E/s a held chunk and holds nothing without its full
//! draw. One anchor a player: the newest one placed works, the older ones wait until it is gone.

use crate::geom::Pos;
use crate::modules::{self, Module};
use crate::tuning::Tuning;
use std::collections::{BTreeMap, BTreeSet};

pub const ANCHOR: &str = "rustcraft:anchor";

/// One anchor as the level keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub pos: Pos,
    pub owner: u128,
    /// When it was placed: a tick count, larger is newer.
    pub placed: u64,
}

/// The square of chunks it holds with `radius` radius modules.
pub fn chunks(pos: Pos, slots: &[Option<Module>], t: &Tuning) -> Vec<[i32; 2]> {
    let n = modules::count(slots, Module::Radius).min(2) as usize;
    let half = t.anchor_sides[n] / 2;
    let (cx, cz) = (pos[0] >> 4, pos[2] >> 4);
    let mut out = Vec::new();
    for x in cx - half..=cx + half {
        for z in cz - half..=cz + half {
            out.push([x, z]);
        }
    }
    out
}

/// E/s it draws holding `chunks` chunks, less with economy modules (A7).
pub fn draw(chunks: usize, slots: &[Option<Module>], t: &Tuning) -> f64 {
    let eco = modules::count(slots, Module::Economy) as i32;
    t.anchor_energy_per_chunk * chunks as f64 * t.economy_energy.powi(eco)
}

/// B6: the anchors that may work, the newest of each owner.
pub fn working(anchors: &[Anchor]) -> BTreeSet<Pos> {
    let mut newest: BTreeMap<u128, Anchor> = BTreeMap::new();
    for a in anchors {
        let keep = newest
            .get(&a.owner)
            .is_none_or(|b| (a.placed, a.pos) > (b.placed, b.pos));
        if keep {
            newest.insert(a.owner, *a);
        }
    }
    newest.values().map(|a| a.pos).collect()
}

/// What an anchor shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Holding,
    NoEnergy,
    OwnerAway,
    /// B6: its owner placed a newer one.
    Replaced,
    NoOwner,
}

/// Its state: it holds only for an owner on the server, as their one anchor, on its full draw.
pub fn state(has_owner: bool, newest: bool, owner_online: bool, share: f64) -> State {
    if !has_owner {
        State::NoOwner
    } else if !newest {
        State::Replaced
    } else if !owner_online {
        State::OwnerAway
    } else if share < 0.999 {
        State::NoEnergy
    } else {
        State::Holding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_chunk_then_3x3_then_5x5() {
        let t = Tuning::default();
        let p = [40, 70, -5];
        assert_eq!(chunks(p, &[], &t), vec![[2, -1]]);
        assert_eq!(chunks(p, &[Some(Module::Radius)], &t).len(), 9);
        let two = [
            Some(Module::Radius),
            Some(Module::Radius),
            Some(Module::Radius),
        ];
        assert_eq!(chunks(p, &two, &t).len(), 25);
        assert_eq!(draw(9, &[], &t), 18.0);
        assert!(draw(9, &[Some(Module::Economy)], &t) < 18.0);
    }

    #[test]
    fn the_newest_anchor_of_a_player_works_and_the_older_waits() {
        let a = Anchor {
            pos: [0, 0, 0],
            owner: 1,
            placed: 5,
        };
        let b = Anchor {
            pos: [99, 0, 0],
            owner: 1,
            placed: 9,
        };
        let c = Anchor {
            pos: [5, 0, 0],
            owner: 2,
            placed: 1,
        };
        let w = working(&[a, b, c]);
        assert!(w.contains(&b.pos) && w.contains(&c.pos) && !w.contains(&a.pos));
        assert_eq!(
            working(&[a, c]).len(),
            2,
            "with the newer gone the older works again"
        );
    }

    #[test]
    fn it_holds_only_on_its_full_draw_for_an_owner_on_the_server() {
        assert_eq!(state(true, true, true, 1.0), State::Holding);
        assert_eq!(state(true, true, true, 0.5), State::NoEnergy);
        assert_eq!(state(true, true, false, 1.0), State::OwnerAway);
        assert_eq!(state(true, false, true, 1.0), State::Replaced);
        assert_eq!(state(false, true, true, 1.0), State::NoOwner);
    }
}
