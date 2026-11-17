//! Slice 18: a turret's kill is its owner's (guide `turret`: «Моб, убитый турелью, даёт владельцу
//! добычу и опыт, как если бы владелец убил его сам; опыт дерева идёт ветке стрелка»).
//!
//! The shot that kills is struck by the owner when they are in the turret's level: the base game
//! then drops the mob's own experience and the loot only a player's kill gives, and
//! Path-of-craft-tree counts the owner's ranged kill itself (the shot's damage is an arrow's). No
//! fixed experience and no `kill` report follow such a kill, or it would count twice. When the
//! owner is away the turret strikes with no attacker as before.

/// The damage type of a shot the owner strikes: a ranged kill for the base game and the tree.
pub const OWNER_SHOT: &str = "minecraft:arrow";

/// Whether the shot of `damage` at a mob holding `health` is the owner's.
pub fn owner_blow(health: f32, damage: f32, owner_here: bool) -> bool {
    owner_here && health > 0.0 && damage >= health
}


/// Wave 2: the experience a mob gives its killer (`Mob.getBaseExperienceReward`, vanilla's
/// `xpReward`), for a kill the owner could not strike himself: none for a mob that gives none.
pub fn kill_experience(kind: &str) -> Option<u32> {
    let id = kind.strip_prefix("minecraft:").unwrap_or(kind);
    Some(match id {
        "blaze" | "guardian" | "elder_guardian" | "evoker" | "breeze" | "piglin_brute" => 10,
        "wither" => 50,
        "ender_dragon" => 500,
        "warden" => 5,
        "endermite" | "silverfish" | "vex" => 3,
        "slime" | "magma_cube" => 1,
        "zoglin" | "hoglin" | "ravager" | "creaking" => 5,
        "iron_golem" | "snow_golem" | "villager" | "wandering_trader" | "bat" | "allay" => return None,
        _ => 5,
    })
}

/// The loot marks' module of horizon-world (`mod/rustcraft/loot_marks.rs`), compiled here for its tests.
#[cfg(test)]
#[path = "../../loot_marks.rs"]
mod loot_marks;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_killing_shot_is_the_owners_when_they_are_here() {
        assert!(owner_blow(4.0, 6.0, true));
        assert!(owner_blow(6.0, 6.0, true));
        assert!(!owner_blow(10.0, 6.0, true), "a shot that does not kill stays the turret's");
        assert!(!owner_blow(4.0, 6.0, false), "an owner away strikes nothing");
        assert!(!owner_blow(0.0, 6.0, true), "a dead mob is nobody's");
    }
}

#[cfg(test)]
mod wave2_tests {
    use super::loot_marks as lm;
    use super::*;
    use serde_json::json;

    #[test]
    fn a_device_kill_gives_the_mobs_own_experience() {
        assert_eq!(kill_experience("minecraft:zombie"), Some(5));
        assert_eq!(kill_experience("minecraft:blaze"), Some(10));
        assert_eq!(kill_experience("minecraft:villager"), None);
    }

    #[test]
    fn every_player_kill_condition_of_a_mob_table_also_holds_for_a_marked_kill() {
        let mut t = json!({"pools": [
            {"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:string"}]},
            {"condition": {"type": "minecraft:killed_by_player"}, "rolls": 1,
             "entries": [{"type": "minecraft:item", "name": "minecraft:spider_eye"}]}
        ]});
        assert!(lm::rewrite("minecraft:entities/spider", &mut t));
        let c = &t["pools"][1]["condition"];
        assert_eq!(c["type"], "minecraft:any_of");
        assert_eq!(c["terms"][0]["type"], "minecraft:killed_by_player");
        assert_eq!(c["terms"][1]["type"], lm::KIND);
        assert_eq!(t["pools"][0]["entries"][0]["name"], "minecraft:string");
        let mut chest = json!({"pools": [{"condition": {"type": "minecraft:killed_by_player"}}]});
        assert!(!lm::rewrite("minecraft:chests/simple_dungeon", &mut chest), "only mobs' tables");
    }

    #[test]
    fn a_mark_holds_near_where_it_was_set_until_it_is_forgotten() {
        lm::mark("w2test:a", 7, [0.0, 64.0, 0.0], 90);
        lm::mark("w2test:a", 7, [10.0, 64.0, 10.0], 100);
        assert!(!lm::marked("w2test:a", [0.0, 64.0, 0.0]), "a newer mark of a mob replaces its old one");
        assert_eq!(lm::holds(lm::KIND, Some("w2test:a"), Some([11.0, 64.0, 10.5])), Some(true));
        assert_eq!(lm::holds(lm::KIND, Some("w2test:a"), Some([14.0, 64.0, 10.0])), Some(false));
        assert_eq!(lm::holds(lm::KIND, Some("w2test:b"), Some([10.0, 64.0, 10.0])), Some(false));
        assert_eq!(lm::holds("other:kind", Some("w2test:a"), Some([10.0, 64.0, 10.0])), None);
        lm::forget_before(100 + lm::LIFE + 1);
        assert!(!lm::marked("w2test:a", [10.0, 64.0, 10.0]));
    }
}
