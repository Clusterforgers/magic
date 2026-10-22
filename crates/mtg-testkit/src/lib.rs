//! Scenario DSL, synthetic cards and invariant checks.
//!
//! Build a state with [`Scenario`], drive it with [`Driver`] (scripted answers by card name),
//! check engine invariants with [`InvariantChecker`], and fuzz whole games with [`play_game`].

pub mod alternative_cards;
pub mod announce_cards;
pub mod attach_cards;
pub mod bot;
pub mod cards;
pub mod cascade_cards;
pub mod choice_cards;
pub mod copy_cards;
pub mod corpus;
pub mod cost_cards;
pub mod counter_cards;
pub mod decks;
pub mod driver;
pub mod evasion_cards;
pub mod event_cards;
pub mod expect;
pub mod extra_cost_cards;
pub mod face_cards;
pub mod family_cards;
pub mod foretell_cards;
pub mod harness;
pub mod help_cards;
pub mod impulse_cards;
pub mod invariants;
pub mod keyword_cards;
pub mod kicker_cards;
pub mod layer_cards;
pub mod loyalty_cards;
pub mod madness_cards;
pub mod manual_cards;
pub mod manual_script;
pub mod nth_cards;
pub mod part_cards;
pub mod pay_cards;
pub mod prevent_cards;
pub mod protect_cards;
pub mod pump_cards;
pub mod replace_cards;
pub mod rule_cards;
pub mod scenario;
pub mod script_error;
pub mod small_cards;
pub mod static_cards;
pub mod trigger_cards;
pub mod windows;
pub mod x_after_cards;
pub mod x_cards;
pub mod zone_cards;

pub use alternative_cards::alternative_fuzz_setup;
pub use announce_cards::split_fuzz_setup;
pub use attach_cards::attach_fuzz_setup;
pub use bot::RandomBot;
pub use cascade_cards::cascade_fuzz_setup;
pub use choice_cards::choice_fuzz_setup;
pub use copy_cards::{copy_fuzz_setup, spell_copy_fuzz_setup};
pub use cost_cards::cost_fuzz_setup;
pub use counter_cards::counters_fuzz_setup;
pub use decks::{
    archon_fuzz_setup, fuzz_setup, keyword_fuzz_setup, library_fuzz_setup, ramp_fuzz_setup,
    removal_fuzz_setup, spell_fuzz_setup, symbol_fuzz_setup, trigger_fuzz_setup,
};
pub use driver::Driver;
pub use evasion_cards::evasion_fuzz_setup;
pub use event_cards::events_fuzz_setup;
pub use extra_cost_cards::extra_cost_fuzz_setup;
pub use face_cards::face_fuzz_setup;
pub use family_cards::families_fuzz_setup;
pub use foretell_cards::foretell_fuzz_setup;
pub use harness::{FailureKind, GameFailure, GameOutcome, PlayOptions, play_game, play_game_with};
pub use help_cards::help_fuzz_setup;
pub use impulse_cards::{impulse_fuzz_setup, impulse_spell};
pub use invariants::{InvariantChecker, Violation, check_settled, check_state};
pub use keyword_cards::keyword_triggers_fuzz_setup;
pub use kicker_cards::kicker_fuzz_setup;
pub use layer_cards::layers_fuzz_setup;
pub use loyalty_cards::loyalty_fuzz_setup;
pub use madness_cards::madness_fuzz_setup;
pub use manual_cards::manual_fuzz_setup;
pub use manual_script::ManualScript;
pub use nth_cards::nth_fuzz_setup;
pub use part_cards::parts_fuzz_setup;
pub use pay_cards::pay_fuzz_setup;
pub use prevent_cards::prevent_fuzz_setup;
pub use protect_cards::protect_fuzz_setup;
pub use pump_cards::pump_fuzz_setup;
pub use replace_cards::replace_fuzz_setup;
pub use rule_cards::rules_fuzz_setup;
pub use scenario::{
    Scenario, ScenarioError, attached_to, damaged, ready, sick, tapped, with_counters,
};
pub use script_error::ScriptError;
pub use small_cards::small_fuzz_setup;
pub use static_cards::statics_fuzz_setup;
pub use windows::{SeatWindows, Windows};
pub use x_after_cards::x_after_fuzz_setup;
pub use x_cards::x_fuzz_setup;
pub use zone_cards::zones_fuzz_setup;
