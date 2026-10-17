//! Scenario DSL, synthetic cards and invariant checks.
//!
//! Build a state with [`Scenario`], drive it with [`Driver`] (scripted answers by card name),
//! check engine invariants with [`InvariantChecker`], and fuzz whole games with [`play_game`].

pub mod bot;
pub mod cards;
pub mod decks;
pub mod driver;
pub mod harness;
pub mod invariants;
pub mod scenario;

pub use bot::RandomBot;
pub use decks::fuzz_setup;
pub use driver::{Driver, ScriptError};
pub use harness::{FailureKind, GameFailure, GameOutcome, PlayOptions, play_game, play_game_with};
pub use invariants::{InvariantChecker, Violation, check_state};
pub use scenario::{Scenario, ScenarioError, damaged, ready, sick, tapped};
