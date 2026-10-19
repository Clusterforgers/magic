//! `ChooseCosts`: which optional costs to pay as a spell is announced (kicker, buyback, "you may pay").
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (d)) fills in the check.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// One optional cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OptionalCost {
    /// Its index among the ability's optional costs.
    pub index: usize,
    /// The cost as the card prints it.
    pub label: String,
}

/// The optional costs a player may add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CostsOptions {
    /// The spell or ability being announced.
    pub source: ObjectId,
    /// The costs that may be paid.
    pub optional: Vec<OptionalCost>,
}

/// The optional costs chosen, by index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CostsChoice(pub Vec<usize>);

pub(crate) fn answer(
    _player: PlayerId,
    _options: &CostsOptions,
    _choice: CostsChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("choose-costs").into())
}
