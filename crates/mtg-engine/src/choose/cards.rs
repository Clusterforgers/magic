//! `ChooseCards`: a player picks cards from a list (search, reveal, "choose a creature", discard, sacrifice).
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (d)) fills in the check.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// The cards a player may choose from and how many.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardsOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The cards that may be chosen.
    pub from: Vec<ObjectId>,
    /// The fewest to choose.
    pub min: u32,
    /// The most to choose.
    pub max: u32,
    /// What the choice is for, in the words of a card ("Discard a card."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
}

/// The cards chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardsChoice(pub Vec<ObjectId>);

pub(crate) fn answer(
    _player: PlayerId,
    _options: &CardsOptions,
    _choice: CardsChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("choose-cards").into())
}
