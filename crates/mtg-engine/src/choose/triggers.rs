//! `OrderTriggers`: the order in which a player puts their triggered abilities on the stack (CR 603.3b).
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (i)) fills in the check.

use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::object::ObjectRef;

/// One triggered ability waiting for the stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TriggerEntry {
    /// The object that triggered.
    pub source: ObjectRef,
    /// Its ability, by index among the source's rules.
    pub ability: usize,
}

/// The triggers to order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TriggersOptions {
    /// The triggers of one controller, in the order they triggered.
    pub triggers: Vec<TriggerEntry>,
}

/// Every trigger once, by index, in the order they go on the stack (the last is on top).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TriggersChoice(pub Vec<usize>);

pub(crate) fn answer(
    _player: PlayerId,
    _options: &TriggersOptions,
    _choice: TriggersChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("order-triggers").into())
}
