//! Slice 18: the magnet's pull and the run module's burst (guide `pmodules`: «Магнит притягивает
//! лежащие предметы», «Бег +50% на 5 с после начала бега»).

/// The sprint bit of a player's input flags (`PlayerInput`).
pub const SPRINT: i32 = 64;
/// The run module: +50 % speed for 5 s after the player starts to sprint.
pub const RUN_TICKS: u64 = 100;
pub const RUN_BOOST: f64 = 0.5;
/// The magnet pulls items this far a step, one step every `MAGNET_EVERY` ticks; nearer than
/// `MAGNET_NEAR` the game's own pickup takes them.
pub const MAGNET_STEP: f64 = 0.45;
pub const MAGNET_EVERY: u64 = 2;
pub const MAGNET_NEAR: f64 = 1.0;

/// Whether sprinting began between two inputs.
pub fn sprint_started(before: i32, now: i32) -> bool {
    before & SPRINT == 0 && now & SPRINT != 0
}

/// Whether the run burst holds at `now` for a sprint that began at `since`.
pub fn run_boost(since: Option<u64>, now: u64) -> bool {
    since.is_some_and(|s| now >= s && now - s < RUN_TICKS)
}

/// The command that pulls every item lying between `MAGNET_NEAR` and `reach` blocks of `to` one
/// step toward it, in level `dimension` (each item turned to face `to` and moved forward).
pub fn pull_command(dimension: &str, to: [f64; 3], reach: f64) -> String {
    let [x, y, z] = to;
    format!(
        "execute in {dimension} positioned {x:.2} {y:.2} {z:.2} as @e[type=minecraft:item,distance={MAGNET_NEAR}..{reach}] at @s facing {x:.2} {y:.2} {z:.2} run tp @s ^ ^ ^{MAGNET_STEP}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_run_burst_lasts_five_seconds_from_the_sprint() {
        assert!(sprint_started(1, 1 | SPRINT));
        assert!(!sprint_started(SPRINT, SPRINT));
        assert!(run_boost(Some(100), 100));
        assert!(run_boost(Some(100), 199));
        assert!(!run_boost(Some(100), 200));
        assert!(!run_boost(None, 150));
    }

    #[test]
    fn the_pull_takes_items_in_reach_toward_the_player() {
        let c = pull_command("minecraft:overworld", [1.0, 64.5, -2.25], 6.0);
        assert_eq!(
            c,
            "execute in minecraft:overworld positioned 1.00 64.50 -2.25 as @e[type=minecraft:item,distance=1..6] at @s facing 1.00 64.50 -2.25 run tp @s ^ ^ ^0.45"
        );
    }
}
