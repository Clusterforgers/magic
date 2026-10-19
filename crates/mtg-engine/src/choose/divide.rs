//! `DivideAmount`: how a spell divides damage or counters among its targets (CR 601.2d).
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (d)) fills in the check.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Target;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// An amount to divide and the targets that share it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DivideOptions {
    /// The spell or ability being announced.
    pub source: ObjectId,
    /// The amount to divide; all of it is assigned.
    pub total: u32,
    /// The targets, each of which gets at least one.
    pub among: Vec<Target>,
}

/// What each target gets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DivideChoice(pub Vec<(Target, u32)>);

pub(crate) fn answer(
    _player: PlayerId,
    _options: &DivideOptions,
    _choice: DivideChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("divide-amount").into())
}
