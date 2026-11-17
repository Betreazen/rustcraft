//! Slice 18: the energy shield as the guide says (`eshield`): an axe switches it off for 5 / 3 /
//! 1.5 s by tier (vanilla's shield: 5 s), and the fourth tier sends arrows and other projectiles
//! back at whoever shot them.

/// Slice 18 (cross, guide `eshield`: «Когда заряд кончается, поле гаснет и щит не блокирует,
/// пока его не зарядят»): whether a shield with `charge` E can stay up: enough for one block's
/// least cost. Asked when it goes up and every tick it is held up.
pub fn holds_up(charge: f64) -> bool {
    charge + 1e-9 >= super::SHIELD_PER_DAMAGE
}

/// Vanilla's shield is off this long after an axe's hit, seconds.
pub const AXE_SECONDS: f32 = 5.0;

/// How long an axe switches the shield of `tier` off, seconds.
pub fn axe_seconds(tier: u8) -> f32 {
    match tier {
        0..=2 => 5.0,
        3 => 3.0,
        _ => 1.5,
    }
}

/// The `disable_cooldown_scale` of the shield's `minecraft:blocks_attacks`: its time over
/// vanilla's.
pub fn axe_disable_scale(tier: u8) -> f32 {
    axe_seconds(tier) / AXE_SECONDS
}

/// The shield's `minecraft:blocks_attacks` component: vanilla's shield with its axe time.
pub fn blocks_attacks(tier: u8) -> serde_json::Value {
    serde_json::json!({
        "block_delay_seconds": 0.25,
        "disable_cooldown_scale": axe_disable_scale(tier),
        "damage_reductions": [{ "base": 0.0, "factor": 1.0 }],
        "item_damage": { "threshold": 3.0, "base": 1.0, "factor": 1.0 },
        "bypassed_by": "#minecraft:bypasses_shield",
        "block_sound": "minecraft:item.shield.block",
        "disabled_sound": "minecraft:item.shield.break"
    })
}

/// Whether a blocking player looking along `yaw` (vanilla's: 0 south, 90 west), standing at
/// `player`, sends back a projectile of a shield of `tier` that came from `from`: the fourth tier,
/// from in front (within vanilla's 90° each side of the look).
pub fn reflects(tier: u8, yaw: f32, player: [f64; 3], from: [f64; 3]) -> bool {
    if tier < 4 {
        return false;
    }
    let r = f64::from(yaw).to_radians();
    let look = [-r.sin(), r.cos()];
    let to = [from[0] - player[0], from[2] - player[2]];
    look[0] * to[0] + look[1] * to[1] > 0.0
}

/// The motion a projectile sent back from `at` to `target` flies with, `speed` blocks a tick,
/// aimed a little up so an arrow's drop still meets its shooter.
pub fn back_motion(at: [f64; 3], target: [f64; 3], speed: f64) -> [f64; 3] {
    let d = [target[0] - at[0], target[1] - at[1], target[2] - at[2]];
    let flat = d[0].hypot(d[2]);
    let len = (flat * flat + d[1] * d[1]).sqrt().max(1e-6);
    let k = speed / len;
    [d[0] * k, d[1] * k + flat * 0.02, d[2] * k]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drained_shield_goes_down() {
        assert!(holds_up(super::super::SHIELD_PER_DAMAGE));
        assert!(holds_up(6000.0));
        assert!(!holds_up(9.0));
        assert!(!holds_up(0.0));
    }

    #[test]
    fn an_axe_switches_the_shield_off_for_its_tiers_time() {
        assert_eq!(axe_seconds(2), 5.0);
        assert_eq!(axe_seconds(3), 3.0);
        assert_eq!(axe_seconds(4), 1.5);
        assert!((axe_disable_scale(4) - 0.3).abs() < 1e-6);
        let c = blocks_attacks(3);
        assert!((c["disable_cooldown_scale"].as_f64().unwrap() - 0.6).abs() < 1e-6);
    }

    #[test]
    fn only_the_fourth_tier_sends_back_and_only_from_the_front() {
        let me = [0.0, 64.0, 0.0];
        let south = [0.0, 64.0, 8.0];
        assert!(reflects(4, 0.0, me, south), "looking south, shot from the south");
        assert!(!reflects(4, 180.0, me, south), "from behind");
        assert!(!reflects(3, 0.0, me, south), "third tier blocks, does not send back");
        let m = back_motion([0.0, 65.0, 0.0], [10.0, 65.0, 0.0], 1.5);
        assert!(m[0] > 1.4 && m[1] > 0.0 && m[2].abs() < 1e-9);
    }
}
