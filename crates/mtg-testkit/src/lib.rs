//! Scenario DSL, synthetic cards and invariant checks.
//!
//! Build a state with [`Scenario`], drive it with [`Driver`] (scripted answers by card name),
//! check engine invariants with [`InvariantChecker`], and fuzz whole games with [`play_game`].

pub mod bot;
pub mod cards;
pub mod corpus;
pub mod decks;
pub mod driver;
pub mod expect;
pub mod harness;
pub mod invariants;
pub mod scenario;
pub mod script_error;
pub mod trigger_cards;
pub mod windows;

pub use bot::RandomBot;
pub use decks::{
    archon_fuzz_setup, fuzz_setup, keyword_fuzz_setup, spell_fuzz_setup, symbol_fuzz_setup,
    trigger_fuzz_setup,
};
pub use driver::Driver;
pub use harness::{FailureKind, GameFailure, GameOutcome, PlayOptions, play_game, play_game_with};
pub use invariants::{InvariantChecker, Violation, check_settled, check_state};
pub use scenario::{
    Scenario, ScenarioError, attached_to, damaged, ready, sick, tapped, with_counters,
};
pub use script_error::ScriptError;
pub use windows::{SeatWindows, Windows};
