//! `YesNo`: "you may", and whether to pay.
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (d)) fills in the check.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// A yes-or-no question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct YesNoOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
}

/// The answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct YesNoChoice(pub bool);

pub(crate) fn answer(
    _player: PlayerId,
    _options: &YesNoOptions,
    _choice: YesNoChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("yes-no").into())
}
