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
