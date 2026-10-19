//! `ChooseReplacement`: which of several replacement effects applies first (CR 616.1).
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (j)) fills in the check.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// One replacement effect that applies to the event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReplacementCandidate {
    /// The object with the effect.
    pub source: ObjectId,
    /// Its index among the source's rules.
    pub ability: usize,
}

/// The effects the affected player orders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReplacementOptions {
    /// The effects that apply, any of which may be chosen first.
    pub candidates: Vec<ReplacementCandidate>,
}

/// The effect applied first, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReplacementChoice(pub usize);

pub(crate) fn answer(
    _player: PlayerId,
    _options: &ReplacementOptions,
    _choice: ReplacementChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("choose-replacement").into())
}
