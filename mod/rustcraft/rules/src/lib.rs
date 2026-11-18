//! RustCraft's game rules without the game: the bundle feeds them a snapshot of the world and
//! applies what they decide. Rule ids in comments (M3, N4…) refer to `docs/design/02-spec.md`.

pub mod anchor;
pub mod anim;
pub mod backpack;
pub mod butcher;
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
pub mod power;
pub mod ray;
pub mod reports;
pub mod selftest;
pub mod storage;
pub mod traversal;
pub mod tiers;
pub mod tuning;
pub mod utility;
pub mod turret;
pub mod worker;
// Slice 18, base and gear (stream 7).
pub mod aim;
pub mod turret_owner;
