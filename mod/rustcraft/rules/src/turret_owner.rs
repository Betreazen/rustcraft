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
