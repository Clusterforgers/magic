//! Scenario DSL, synthetic cards and invariant checks.
//!
//! Build a state with [`Scenario`], drive it with [`Driver`] (scripted answers by card name),
//! check engine invariants with [`InvariantChecker`], and fuzz whole games with [`play_game`].

pub mod bot;
pub mod cards;
pub mod choice_cards;
pub mod corpus;
pub mod counter_cards;
pub mod decks;
pub mod driver;
pub mod expect;
pub mod harness;
pub mod invariants;
pub mod keyword_cards;
pub mod layer_cards;
pub mod loyalty_cards;
pub mod pay_cards;
pub mod pump_cards;
pub mod rule_cards;
pub mod scenario;
pub mod script_error;
pub mod small_cards;
pub mod static_cards;
pub mod trigger_cards;
pub mod windows;
pub mod zone_cards;

pub use bot::RandomBot;
pub use choice_cards::choice_fuzz_setup;
pub use counter_cards::counters_fuzz_setup;
pub use decks::{
    archon_fuzz_setup, fuzz_setup, keyword_fuzz_setup, library_fuzz_setup, ramp_fuzz_setup,
    removal_fuzz_setup, spell_fuzz_setup, symbol_fuzz_setup, trigger_fuzz_setup,
};
pub use driver::Driver;
pub use harness::{FailureKind, GameFailure, GameOutcome, PlayOptions, play_game, play_game_with};
pub use invariants::{InvariantChecker, Violation, check_settled, check_state};
pub use keyword_cards::keyword_triggers_fuzz_setup;
pub use layer_cards::layers_fuzz_setup;
pub use loyalty_cards::loyalty_fuzz_setup;
pub use pay_cards::pay_fuzz_setup;
pub use pump_cards::pump_fuzz_setup;
pub use rule_cards::rules_fuzz_setup;
pub use scenario::{
    Scenario, ScenarioError, attached_to, damaged, ready, sick, tapped, with_counters,
};
pub use script_error::ScriptError;
pub use small_cards::small_fuzz_setup;
pub use static_cards::statics_fuzz_setup;
pub use windows::{SeatWindows, Windows};
pub use zone_cards::zones_fuzz_setup;
