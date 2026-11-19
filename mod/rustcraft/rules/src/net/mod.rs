//! The magistral: housings with their three built-in channels, nodes, and the networks they form.

pub mod bottleneck;
pub mod energy;
pub mod fluids;
pub mod items;
pub mod joint_window;
pub mod joints;
// Round 31: housings saved with channels set by hand get them built in.
pub mod migrate;
// 0.21 (spec 45): the channel items of 0.19 turned into their alloy wherever the mod sees them.
pub mod old_channels;
pub mod settings;
pub mod stock;
// 0.21 (round 32): a storage's surplus to the disintegrator and the trash.
pub mod surplus;
pub mod tanks;
pub mod node_tier;
pub mod node_window;
pub mod topology;
pub mod wrench;
// s18/pipe: the joint window drawn as the guide's table.
pub mod joint_screen;
// Slice 18, wave 2: the plain item flow drawn in the housings (M5).
pub mod flow;
