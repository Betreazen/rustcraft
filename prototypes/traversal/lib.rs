// Builds the game-free modules of the prototype on their own, so `cargo test` runs in seconds.
#[path = "mixins/traversal/journal.rs"]
pub mod journal;
#[path = "mixins/traversal/measure.rs"]
pub mod measure;
#[path = "mixins/traversal/plans.rs"]
pub mod plans;
#[path = "mixins/traversal/rules.rs"]
pub mod rules;
#[path = "mixins/traversal/setbacks.rs"]
pub mod setbacks;
#[path = "mixins/traversal/wire.rs"]
pub mod wire;

/// The paths the modules use inside the game.
pub mod game {
    pub use crate::journal as trv_journal;
    pub use crate::measure as trv_measure;
    pub use crate::plans as trv_plans;
    pub use crate::rules as trv_rules;
    pub use crate::wire as trv_wire;
}

#[cfg(test)]
mod tests {
    #[test]
    fn corrections_wait_until_taken() {
        crate::setbacks::note(7, [1.0, 2.0, 3.0]);
        assert_eq!(crate::setbacks::take_all(), vec![(7, [1.0, 2.0, 3.0])]);
        assert!(crate::setbacks::take_all().is_empty());
    }
}
