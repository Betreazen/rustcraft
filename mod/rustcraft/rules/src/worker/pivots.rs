//! The pivots of the worker parts that move, written by assets/models/export_mc.py from
//! assets/models/transport.py: do not edit by hand, run the exporter.

/// (part, pivot in blocks, Minecraft axes, in the model frame).
#[rustfmt::skip]
pub const PIVOTS: &[(&str, [f64; 3])] = &[
    ("side", [0.5, 0.5, 0.5]),
    ("aim_yaw_saw", [0.90625, 0.390625, 0.5]),
    ("reach_saw", [0.90625, 0.390625, 0.5]),
    ("saw_disc", [1.25, 0.46875, 0.5]),
    ("reaper_reel", [1.0625, 0.421875, 0.5]),
    ("bore_bit", [0.96875, 0.4375, 0.5]),
    ("aim_yaw_gun", [0.6875, 0.71875, 0.5]),
    ("aim_pitch_gun", [0.75, 0.8125, 0.5]),
    ("aim_yaw_scoop", [0.90625, 0.390625, 0.5]),
    ("reach_scoop", [0.90625, 0.390625, 0.5]),
    ("scoop_bucket", [1.25, 0.40625, 0.5]),
    ("map_drum", [0.625, 0.84375, 0.5]),
];
