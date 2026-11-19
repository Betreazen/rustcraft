//! RustCraft's game rules without the game: the bundle feeds them a snapshot of the world and
//! applies what they decide. Rule ids in comments (M3, N4…) refer to `docs/design/02-spec.md`.

pub mod anchor;
pub mod anim;
pub mod backpack;
pub mod butcher;
pub mod connectors;
pub mod cull;
// The diagnostics console command and the event journal.
pub mod debug;
pub mod druses;
pub mod explore;
pub mod gear;
pub mod geom;
pub mod grow;
pub mod handbook;
pub mod housing;
pub mod machines;
pub mod mining;
pub mod modules;
pub mod monorail;
pub mod net;
pub mod orders;
// 0.20: Path-of-craft-tree's tree on its owner's machines.
pub mod perks;
pub mod power;
pub mod ray;
pub mod reports;
pub mod selftest;
pub mod storage;
pub mod traversal;
pub mod tiers;
// 0.21: an item's description in its tooltip (spec 41).
pub mod tips;
pub mod tuning;
pub mod utility;
pub mod turret;
pub mod worker;
// Round 31: the z-fighting check over the content mod's block models (tests only).
#[cfg(test)]
mod zfight;
// Slice 18, base and gear (stream 7).
pub mod aim;
pub mod turret_owner;
