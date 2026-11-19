//! Machines: the one-input processors (electric furnace, crusher), the fuel generator, the alloy
//! smelter, the autocrafter and the pump, with their windows' data and layouts (the tank's too).

pub mod alloy;
pub mod bucket;
pub mod catalyst;
pub mod charging;
pub mod crafter;
pub mod extras;
pub mod fluid_windows;
pub mod generator;
// 0.20: what a machine takes by itself from the magistral or a hopper.
pub mod intake;
pub mod mixer;
pub mod panel;
pub mod processor;
pub mod pump;
pub mod sheet;
pub mod queue;
// 0.21 (spec 39): the mixer's and the kitchen's recipes turned on and off.
pub mod recipes;
pub mod slots;
pub mod sockets;
pub mod style;
pub mod tiered;
pub mod windows;
