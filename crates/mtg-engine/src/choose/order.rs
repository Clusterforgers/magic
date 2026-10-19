//! `OrderCards`: a player puts cards in an order (scry, surveil, cards put back, "in any order").
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (d)) fills in the check.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// The cards to put in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OrderOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The cards, in their current order.
    pub cards: Vec<ObjectId>,
    /// What the order is for, in the words of a card ("Put the cards on the bottom in any order."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
}

/// Every card once, in the order chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OrderChoice(pub Vec<ObjectId>);

pub(crate) fn answer(
    _player: PlayerId,
    _options: &OrderOptions,
    _choice: OrderChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("order-cards").into())
}
