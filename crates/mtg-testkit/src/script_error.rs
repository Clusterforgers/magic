//! Errors raised while running a scripted scenario.

use mtg_engine::EngineError;
use thiserror::Error;

use crate::invariants::Violation;
use crate::scenario::ScenarioError;

/// A scripted step failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScriptError {
    /// The engine rejected an answer.
    #[error("engine: {0}")]
    Engine(#[from] EngineError),
    /// An invariant broke after an answer.
    #[error("invariant: {0}")]
    Invariant(#[from] Violation),
    /// The scenario could not be built.
    #[error("scenario: {0}")]
    Scenario(#[from] ScenarioError),
    /// The script named a card that is not among the options.
    #[error("no option named {name:?} among {place}")]
    NoSuchCard {
        /// The requested card name.
        name: String,
        /// Where it was looked for.
        place: &'static str,
    },
    /// The pending decision is not the kind the script step needs.
    #[error("expected a {wanted} decision but the engine asked for {got}")]
    UnexpectedDecision {
        /// What the step needed.
        wanted: &'static str,
        /// What is pending.
        got: String,
    },
    /// An `expect_*` assertion did not hold.
    #[error("expectation failed: {0}")]
    Expectation(String),
    /// The script waited too long for a step.
    #[error("gave up waiting for {0}")]
    Stuck(String),
}
