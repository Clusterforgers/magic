//! `ChooseNumber`: X when a spell is announced, or "any number" as an effect resolves.
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (d)) fills in the check.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// The range of numbers a player may pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NumberOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The smallest allowed number.
    pub min: u32,
    /// The largest allowed number; no limit when absent.
    pub max: Option<u32>,
}

/// The number picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NumberChoice(pub u32);

pub(crate) fn answer(
    _player: PlayerId,
    _options: &NumberOptions,
    _choice: NumberChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("choose-number").into())
}
